use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::Arc;
use eframe::emath::Vec2;
use eframe::{egui, run_native, CreationContext, NativeOptions};
use egui::style::ScrollStyle;
use egui::{Align2, Color32, Context, Frame, Painter, Pos2, Rect, Response, Ui};
use egui_extras::{Column, TableBuilder};
use egui_taffy::taffy::Dimension::Length;
use egui_taffy::taffy::prelude::{auto, percent};
use egui_taffy::taffy::{Size, Style};
use egui_taffy::{taffy, TuiBuilderLogic};
use epaint::{FontId, Mesh, Shape, Stroke, StrokeKind, Vertex};
use gerber_parser::gerber_doc::GerberDoc;
use gerber_parser::gerber_types;
use gerber_parser::gerber_types::{Aperture, ApertureDefinition, ApertureMacro, Command, Coordinates, DCode, ExtendedCode, FunctionCode, GCode, MacroContent, MacroDecimal, Operation, VariableDefinition};
use gerber_parser::parser::parse_gerber;
use log::{debug, error, info, warn};
use rand::prelude::SmallRng;
use rand::{Rng, SeedableRng};
use rfd::FileDialog;
use thiserror::Error;
use geometry::{BoundingBox, PolygonMesh};
use gerber::Exposure;
use gerber::position::deduplicate::DedupEpsilon;
use logging::AppLogItem;
use crate::gerber::{Position, Winding};
use crate::gerber_expressions::{
    evaluate_expression, macro_boolean_to_bool, macro_decimal_pair_to_f64, macro_decimal_to_f64, macro_integer_to_u32,
    ExpressionEvaluationError, MacroContext,
};

mod gerber;
mod geometry;
mod logging;

const INITIAL_GERBER_AREA_PERCENT: f32 = 0.95;

fn main() -> eframe::Result<()> {
    env_logger::init(); // Log to stderr (optional).
    let native_options = NativeOptions::default();
    run_native(
        "Gerber Viewer",
        native_options,
        Box::new(|cc| Ok(Box::new(GerberViewer::new(cc)))),
    )
}
struct GerberViewer {
    state: Option<GerberViewState>,
    log: Vec<AppLogItem>,
    coord_input: (String, String),
    use_unique_shape_colors: bool,
}

struct LayerViewState {
    enabled: bool,
    color: Color32,
}

impl LayerViewState {
    fn new(color: Color32) -> Self {
        Self {
            enabled: true,
            color,
        }
    }
}

struct GerberViewState {
    view: ViewState,
    needs_initial_view: bool,
    bounding_box: BoundingBox,
    cursor_gerber_coords: Option<Position>,
    layers: Vec<(LayerViewState, GerberLayer)>,
    center_screen_pos: Option<Vec2>,
    origin_screen_pos: Option<Vec2>,
}

impl Default for GerberViewState {
    fn default() -> Self {
        Self {
            view: Default::default(),
            needs_initial_view: true,
            bounding_box: BoundingBox::default(),
            cursor_gerber_coords: None,
            center_screen_pos: None,
            origin_screen_pos: None,
            layers: vec![],
        }
    }
}

impl GerberViewState {
    pub fn add_layer(&mut self, layer_view_state: LayerViewState, layer: GerberLayer) {
        self.layers
            .push((layer_view_state, layer));
        self.update_bbox_from_layers();
        self.request_reset();
    }

    fn update_bbox_from_layers(&mut self) {
        let mut bbox = BoundingBox::default();

        for (_, layer) in self
            .layers
            .iter()
            .filter(|(state, _)| state.enabled)
        {
            let layer_bbox = &layer.bounding_box;
            bbox.min_x = f64::min(bbox.min_x, layer_bbox.min_x);
            bbox.min_y = f64::min(bbox.min_y, layer_bbox.min_y);
            bbox.max_x = f64::max(bbox.max_x, layer_bbox.max_x);
            bbox.max_y = f64::max(bbox.max_y, layer_bbox.max_y);
        }
        debug!("view bbox: {:?}", bbox);

        self.bounding_box = bbox;
    }

    pub fn request_reset(&mut self) {
        self.needs_initial_view = true;
    }

    fn reset_view(&mut self, viewport: Rect) {
        self.update_bbox_from_layers();

        let bbox = &self.bounding_box;

        let content_width = bbox.max_x - bbox.min_x;
        let content_height = bbox.max_y - bbox.min_y;

        // Calculate scale to fit the content
        let scale = f32::min(
            viewport.width() / (content_width as f32),
            viewport.height() / (content_height as f32),
        ) * INITIAL_GERBER_AREA_PERCENT;

        // Calculate the content center in mm
        let content_center_x = (bbox.min_x + bbox.max_x) / 2.0;
        let content_center_y = (bbox.min_y + bbox.max_y) / 2.0;

        // Offset from viewport center to place content center
        self.view.translation = Vec2::new(
            viewport.center().x - (content_center_x as f32 * scale),
            viewport.center().y + (content_center_y as f32 * scale), // Note the + here since we flip Y
        );

        self.view.scale = scale;
        self.needs_initial_view = false;
    }

    pub fn update_cursor_position(&mut self, response: &Response, ui: &Ui) {
        if !response.hovered() {
            return;
        }

        if let Some(pointer_pos) = ui.input(|i| i.pointer.hover_pos()) {
            self.cursor_gerber_coords = Some(self.screen_to_gerber_coords(pointer_pos.to_vec2()));
        }
    }

    pub fn handle_panning(&mut self, response: &Response, ui: &mut Ui) {
        if response.dragged_by(egui::PointerButton::Primary) {
            let delta = response.drag_delta();
            self.view.translation += delta;
            ui.ctx().clear_animations();
        }
    }

    pub fn handle_zooming(&mut self, response: &Response, viewport: Rect, ui: &mut Ui) {
        // Only process zoom if mouse is actually over the viewport
        if !response.hovered() {
            return;
        }

        let zoom_factor = 1.1;
        let mut scroll_delta = ui.input(|i| i.raw_scroll_delta.y);
        if ui.input(|i| i.modifiers.ctrl) {
            scroll_delta *= 0.0; // Disable zoom when Ctrl is held (for text scaling)
        }

        if scroll_delta != 0.0 {
            let old_scale = self.view.scale;
            let new_scale = if scroll_delta > 0.0 {
                old_scale * zoom_factor
            } else {
                old_scale / zoom_factor
            };

            if let Some(mouse_pos) = response.hover_pos() {
                let mouse_pos = mouse_pos - viewport.min.to_vec2();
                let mouse_world = (mouse_pos - self.view.translation) / old_scale;
                self.view.translation = mouse_pos - mouse_world * new_scale;
            }

            self.view.scale = new_scale;
        }
    }

    /// Convert to gerber coordinates using view transformation
    pub fn screen_to_gerber_coords(&self, screen_pos: Vec2) -> gerber::Position {
        let gerber_pos = (screen_pos - self.view.translation) / self.view.scale;
        gerber::Position::new(gerber_pos.x as f64, gerber_pos.y as f64).invert_y()
    }

    /// Convert from gerber coordinates using view transformation
    pub fn gerber_to_screen_coords(&self, gerber_pos: gerber::Position) -> Vec2 {
        let gerber_pos = gerber_pos.invert_y().to_vec2();
        self.view.translation + (gerber_pos * self.view.scale)
    }

    /// X and Y are in GERBER units.
    pub fn move_view(&mut self, position: gerber::Position) {
        debug!("move view. x: {}, y: {}", position.x, position.y);
        debug!("view translation (before): {:?}", self.view.translation);

        let mut gerber_coords = self.screen_to_gerber_coords(self.view.translation);
        gerber_coords += position;
        debug!("gerber_coords: {:?}", self.view.translation);
        let screen_coords = self.gerber_to_screen_coords(gerber_coords);

        debug!("screen_cords: {:?}", screen_coords);

        let delta = screen_coords - self.view.translation;
        debug!("delta: {:?}", delta);

        self.view.translation -= delta;
        debug!("view translation (after): {:?}", self.view.translation);
    }

    /// X and Y are in GERBER units.
    pub fn locate_view(&mut self, x: f64, y: f64) {
        debug!("locate view. x: {}, y: {}", x, y);
        self.view.translation = Vec2::new(
            self.center_screen_pos.unwrap().x - (x as f32 * self.view.scale),
            self.center_screen_pos.unwrap().y + (y as f32 * self.view.scale),
        );
        debug!("view translation (after): {:?}", self.view.translation);
    }
}

struct GerberLayer {
    path: PathBuf,
    gerber_doc: GerberDoc,
    gerber_primitives: Vec<GerberPrimitive>,
    bounding_box: BoundingBox,
}

impl GerberLayer {
    fn new(gerber_doc: GerberDoc, path: PathBuf) -> Self {
        let gerber_primitives = GerberLayer::build_primitives(&gerber_doc);
        let bounding_box = GerberLayer::calculate_bounding_box(&gerber_primitives);

        Self {
            path,
            gerber_doc,
            gerber_primitives,
            bounding_box,
        }
    }
}

#[derive(Debug, Copy, Clone)]
struct ViewState {
    translation: Vec2,
    scale: f32,
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            translation: Vec2::ZERO,
            scale: 1.0,
        }
    }
}

#[derive(Debug, Clone)]
enum GerberPrimitive {
    Circle {
        center: Position,
        diameter: f64,
        exposure: Exposure,
    },
    Rectangle {
        origin: Position,
        width: f64,
        height: f64,
        exposure: Exposure,
    },
    Line {
        start: Position,
        end: Position,
        width: f64,
        exposure: Exposure,
    },
    Polygon {
        center: Position,
        exposure: Exposure,
        geometry: Arc<PolygonGeometry>
    },
}

#[derive(Debug, Clone)]
struct PolygonGeometry {
    relative_vertices: Vec<Position>,  // Relative to center
    tessellation: Option<PolygonMesh>, // Precomputed tessellation data
    is_convex: bool,
}

#[derive(Debug)]
struct GerberPolygon {
    center: Position,
    /// Relative to center
    vertices: Vec<Position>,
    exposure: Exposure,
}

impl GerberPolygon {
    /// Checks if a polygon is convex by verifying that all cross products
    /// between consecutive edges have the same sign
    pub fn is_convex(&self) -> bool {
        geometry::is_convex(&self.vertices)
    }
}

impl GerberPrimitive {

    fn new_polygon(polygon: GerberPolygon) -> Self {
        debug!("new_polygon: {:?}", polygon);
        let is_convex = polygon.is_convex();
        let mut relative_vertices = polygon.vertices;

        // Calculate and fix winding order
        let winding = gerber::calculate_winding(&relative_vertices);
        if matches!(winding, Winding::Clockwise) {
            relative_vertices.reverse();
        }

        // Deduplicate adjacent vertices with geometric tolerance
        let epsilon = 1e-6; // 1 nanometer in mm units
        let relative_vertices = relative_vertices.dedup_with_epsilon(epsilon);

        // Precompute tessellation for concave polygons
        let tessellation = if !is_convex {
            Some(geometry::tessellate_polygon(&relative_vertices))
        } else {
            None
        };

        let polygon = GerberPrimitive::Polygon {
            center: polygon.center,
            exposure: polygon.exposure,
            geometry: Arc::new(PolygonGeometry {
                relative_vertices,
                tessellation,
                is_convex,
            }),
        };

        debug!("polygon: {:?}", polygon);

        polygon
    }
}

#[derive(Debug)]
enum ApertureKind {
    Standard(Aperture),
    Macro(Vec<GerberPrimitive>),
}

#[derive(Error, Debug)]
enum AppError {
    #[error("No file selected")]
    NoFileSelected,
    #[error("IO Error. cause: {0:?}")]
    IoError(io::Error),
}

impl GerberViewer {
    /// FIXME: Blocks main thread when file selector is open
    fn add_layer_files(&mut self) {
        self.open_gerber_file_inner()
            .inspect_err(|e| {
                let message = format!("Error opening file: {:?}", e);
                error!("{}", message);
                self.log
                    .push(AppLogItem::Error(message.to_string()));
            })
            .ok();
    }

    fn open_gerber_file_inner(&mut self) -> Result<(), AppError> {
        let paths = FileDialog::new()
            .add_filter("Gerber Files", &["gbr", "gbl", "gbo", "gbs", "gko", "gko", "gto"])
            .pick_files()
            .ok_or(AppError::NoFileSelected)?;

        for path in paths {
            self.add_gerber_layer_from_file(path)?;
        }

        Ok(())
    }

    pub fn add_gerber_layer_from_file(&mut self, path: PathBuf) -> Result<(), AppError> {
        let gerber_doc = Self::parse_gerber(&mut self.log, &path)?;

        let state = self.state.get_or_insert_default();

        let layer_count = state.layers.len();
        let color = generate_pastel_color(layer_count as u64);

        let layer = GerberLayer::new(gerber_doc, path.clone());
        let layer_view_state = LayerViewState::new(color);

        state.add_layer(layer_view_state, layer);

        Ok(())
    }

    fn parse_gerber(log: &mut Vec<AppLogItem>, path: &PathBuf) -> Result<GerberDoc, AppError> {
        let file = File::open(path.clone())
            .inspect_err(|error| {
                let message = format!(
                    "Error parsing gerber file: {}, cause: {}",
                    path.to_str().unwrap(),
                    error
                );
                error!("{}", message);
                log.push(AppLogItem::Error(message.to_string()));
            })
            .map_err(AppError::IoError)?;

        let reader = BufReader::new(file);

        let gerber_doc: GerberDoc = parse_gerber(reader);

        let log_entries = gerber_doc
            .commands
            .iter()
            .map(|c| match c {
                Ok(command) => AppLogItem::Info(format!("{:?}", command)),
                Err(error) => AppLogItem::Error(format!("{:?}", error)),
            })
            .collect::<Vec<_>>();
        log.extend(log_entries);

        let message = format!("Gerber file parsed successfully. path: {}", path.to_str().unwrap());
        info!("{}", message);
        log.push(AppLogItem::Info(message.to_string()));

        Ok(gerber_doc)
    }

    pub fn reload_all_layer_files(&mut self) {
        let Some(state) = &mut self.state else { return };

        for (_state, layer) in state.layers.iter_mut() {
            let path = layer.path.clone();

            if let Ok(gerber_doc) = Self::parse_gerber(&mut self.log, &path) {
                *layer = GerberLayer::new(gerber_doc, path.clone());
            }
        }
    }

    pub fn close_all(&mut self) {
        self.state = None;
    }

    pub fn clear_log(&mut self) {
        self.log.clear();
    }
}

impl GerberLayer {
    fn update_position(current_pos: &mut Position, coords: &Coordinates) {
        *current_pos = (
            coords
                .x
                .map(|value| value.into())
                .unwrap_or(current_pos.x),
            coords
                .y
                .map(|value| value.into())
                .unwrap_or(current_pos.y),
        )
            .into()
    }

    fn calculate_bounding_box(primitives: &Vec<GerberPrimitive>) -> BoundingBox {
        let mut bbox = BoundingBox::default();

        // Calculate bounding box
        for primitive in primitives {
            match primitive {
                GerberPrimitive::Circle {
                    center,
                    diameter,
                    ..
                } => {
                    let radius = diameter / 2.0;
                    bbox.min_x = bbox.min_x.min(center.x - radius);
                    bbox.min_y = bbox.min_y.min(center.y - radius);
                    bbox.max_x = bbox.max_x.max(center.x + radius);
                    bbox.max_y = bbox.max_y.max(center.y + radius);
                }
                GerberPrimitive::Rectangle {
                    origin,
                    width,
                    height,
                    ..
                } => {
                    bbox.min_x = bbox.min_x.min(origin.x);
                    bbox.min_y = bbox.min_y.min(origin.y);
                    bbox.max_x = bbox.max_x.max(origin.x + width);
                    bbox.max_y = bbox.max_y.max(origin.y + height);
                }
                GerberPrimitive::Line {
                    start,
                    end,
                    width,
                    ..
                } => {
                    let radius = width / 2.0;
                    for &Position {
                        x,
                        y,
                    } in &[start, end]
                    {
                        bbox.min_x = bbox.min_x.min(x - radius);
                        bbox.min_y = bbox.min_y.min(y - radius);
                        bbox.max_x = bbox.max_x.max(x + radius);
                        bbox.max_y = bbox.max_y.max(y + radius);
                    }
                }
                GerberPrimitive::Polygon {
                    center,
                    geometry,
                    ..
                } => {
                    for &Position {
                        x: dx,
                        y: dy,
                    } in geometry.relative_vertices.iter()
                    {
                        let x = center.x + dx;
                        let y = center.y + dy;
                        bbox.min_x = bbox.min_x.min(x);
                        bbox.min_y = bbox.min_y.min(y);
                        bbox.max_x = bbox.max_x.max(x);
                        bbox.max_y = bbox.max_y.max(y);
                    }
                }
            }
        }
        
        debug!("layer bbox: {:?}", bbox);

        bbox
    }

    fn build_primitives(doc: &GerberDoc) -> Vec<GerberPrimitive> {
        let mut macro_definitions: HashMap<String, &ApertureMacro> = HashMap::default();

        // First pass: collect aperture macros
        for cmd in doc
            .commands
            .iter()
            .filter_map(|result| result.as_ref().ok())
        {
            if let Command::ExtendedCode(ExtendedCode::ApertureMacro(macro_def)) = cmd {
                macro_definitions.insert(macro_def.name.clone(), macro_def);
            }
        }

        // Second pass - collect aperture definitions, build their primitives (using supplied args)

        let mut apertures: HashMap<i32, ApertureKind> = HashMap::default();

        for cmd in doc
            .commands
            .iter()
            .filter_map(|result| result.as_ref().ok())
        {
            if let Command::ExtendedCode(ExtendedCode::ApertureDefinition(ApertureDefinition {
                code,
                aperture,
            })) = cmd
            {
                match aperture {
                    Aperture::Macro(macro_name, args) => {
                        // Handle macro-based apertures

                        if let Some(macro_def) = macro_definitions.get(macro_name) {
                            //
                            // build a unique name based on the macro name and args
                            //
                            let macro_name_and_args = match args {
                                None => macro_name,
                                Some(args) => {
                                    let args_str = args
                                        .iter()
                                        .map(|arg| {
                                            let meh = match arg {
                                                MacroDecimal::Value(value) => value.to_string(),
                                                MacroDecimal::Variable(variable) => format!("${}", variable),
                                                MacroDecimal::Expression(expression) => expression.clone(),
                                            };

                                            meh
                                        })
                                        .collect::<Vec<_>>()
                                        .join("X");

                                    &format!("{}_{}", macro_name, args_str)
                                }
                            };
                            debug!("macro_name_and_args: {}", macro_name_and_args);

                            let mut macro_context = MacroContext::default();

                            //
                            // populate the macro_context from the args.
                            //
                            if let Some(args) = args {
                                for (index, arg) in args.iter().enumerate() {
                                    let arg_number = (index + 1) as u32;

                                    match arg {
                                        MacroDecimal::Value(value) => {
                                            macro_context
                                                .put(arg_number, *value)
                                                .inspect_err(|error| {
                                                    error!("Error setting variable {}: {}", arg_number, error);
                                                })
                                                .ok();
                                        }
                                        MacroDecimal::Variable(variable) => {
                                            macro_context
                                                .put(arg_number, macro_context.get(variable))
                                                .inspect_err(|error| {
                                                    error!("Error setting variable {}: {}", arg_number, error);
                                                })
                                                .ok();
                                        }
                                        MacroDecimal::Expression(expression) => {
                                            evaluate_expression(&expression, &macro_context)
                                                .map(|value| {
                                                    macro_context
                                                        .put(arg_number, value)
                                                        .inspect_err(|error| {
                                                            error!("Error setting variable {}: {}", arg_number, error);
                                                        })
                                                        .ok();
                                                })
                                                .inspect_err(|error| {
                                                    error!("Error evaluating expression {}: {}", expression, error);
                                                })
                                                .ok();
                                        }
                                    }
                                }
                            }

                            debug!("macro_context: {:?}", macro_context);

                            let mut primitive_defs = vec![];

                            for content in &macro_def.content {
                                debug!("content: {:?}", content);

                                fn process_content(
                                    content: &MacroContent,
                                    macro_context: &mut MacroContext,
                                ) -> Result<Option<GerberPrimitive>, ExpressionEvaluationError>
                                {
                                    match content {
                                        MacroContent::Circle(circle) => {
                                            let diameter = macro_decimal_to_f64(&circle.diameter, macro_context)?;
                                            let (center_x, center_y) =
                                                macro_decimal_pair_to_f64(&circle.center, macro_context)?;

                                            // Get rotation angle and convert to radians
                                            let rotation_radians = if let Some(angle) = &circle.angle {
                                                macro_decimal_to_f64(angle, macro_context)? * std::f64::consts::PI
                                                    / 180.0
                                            } else {
                                                0.0
                                            };

                                            // Apply rotation to center coordinates around macro origin (0,0)
                                            let (sin_theta, cos_theta) = rotation_radians.sin_cos();
                                            let rotated_x = center_x * cos_theta - center_y * sin_theta;
                                            let rotated_y = center_x * sin_theta + center_y * cos_theta;

                                            Ok(Some(GerberPrimitive::Circle {
                                                center: (rotated_x, rotated_y).into(),
                                                diameter,
                                                exposure: macro_boolean_to_bool(&circle.exposure, macro_context)?
                                                    .into(),
                                            }))
                                        }
                                        MacroContent::VectorLine(vector_line) => {
                                            // Get parameters
                                            let (start_x, start_y) =
                                                macro_decimal_pair_to_f64(&vector_line.start, macro_context)?;
                                            let (end_x, end_y) =
                                                macro_decimal_pair_to_f64(&vector_line.end, macro_context)?;
                                            let width = macro_decimal_to_f64(&vector_line.width, macro_context)?;
                                            let rotation_angle =
                                                macro_decimal_to_f64(&vector_line.angle, macro_context)?;
                                            let rotation_radians = rotation_angle.to_radians();
                                            let (sin_theta, cos_theta) = rotation_radians.sin_cos();

                                            // Rotate start and end points
                                            let rotated_start_x = start_x * cos_theta - start_y * sin_theta;
                                            let rotated_start_y = start_x * sin_theta + start_y * cos_theta;
                                            let rotated_end_x = end_x * cos_theta - end_y * sin_theta;
                                            let rotated_end_y = end_x * sin_theta + end_y * cos_theta;

                                            // Calculate direction vector
                                            let dx = rotated_end_x - rotated_start_x;
                                            let dy = rotated_end_y - rotated_start_y;
                                            let length = (dx * dx + dy * dy).sqrt();

                                            if length == 0.0 {
                                                return Ok(None);
                                            }

                                            // Calculate perpendicular direction
                                            let ux = dx / length;
                                            let uy = dy / length;
                                            let perp_x = -uy;
                                            let perp_y = ux;

                                            // Calculate width offsets
                                            let half_width = width / 2.0;
                                            let hw_perp_x = perp_x * half_width;
                                            let hw_perp_y = perp_y * half_width;

                                            // Calculate corners in absolute coordinates
                                            let corners = [
                                                (rotated_start_x - hw_perp_x, rotated_start_y - hw_perp_y),
                                                (rotated_start_x + hw_perp_x, rotated_start_y + hw_perp_y),
                                                (rotated_end_x + hw_perp_x, rotated_end_y + hw_perp_y),
                                                (rotated_end_x - hw_perp_x, rotated_end_y - hw_perp_y),
                                            ];

                                            // Calculate center point
                                            let center_x = (rotated_start_x + rotated_end_x) / 2.0;
                                            let center_y = (rotated_start_y + rotated_end_y) / 2.0;

                                            // Convert to relative vertices
                                            let vertices = corners
                                                .iter()
                                                .map(|&(x, y)| Position::new(x - center_x, y - center_y))
                                                .collect();

                                            Ok(Some(GerberPrimitive::new_polygon(GerberPolygon {
                                                center: Position::new(center_x, center_y),
                                                vertices,
                                                exposure: macro_boolean_to_bool(&vector_line.exposure, macro_context)?
                                                    .into(),
                                            })))
                                        }
                                        MacroContent::CenterLine(center_line) => {
                                            // Get parameters
                                            let (center_x, center_y) =
                                                macro_decimal_pair_to_f64(&center_line.center, macro_context)?;
                                            let (length, width) =
                                                macro_decimal_pair_to_f64(&center_line.dimensions, macro_context)?;
                                            let rotation_angle =
                                                macro_decimal_to_f64(&center_line.angle, macro_context)?;
                                            let rotation_radians = rotation_angle.to_radians();
                                            let (sin_theta, cos_theta) = rotation_radians.sin_cos();

                                            // Calculate half dimensions
                                            let half_length = length / 2.0;
                                            let half_width = width / 2.0;

                                            // Define unrotated vertices relative to center
                                            let unrotated_vertices = [
                                                Position::new(half_length, half_width),
                                                Position::new(-half_length, half_width),
                                                Position::new(-half_length, -half_width),
                                                Position::new(half_length, -half_width),
                                            ];

                                            // Rotate each vertex relative to the center
                                            let vertices = unrotated_vertices
                                                .iter()
                                                .map(|pos| {
                                                    let x = pos.x * cos_theta - pos.y * sin_theta;
                                                    let y = pos.x * sin_theta + pos.y * cos_theta;
                                                    Position::new(x, y)
                                                })
                                                .collect();

                                            Ok(Some(GerberPrimitive::new_polygon(GerberPolygon {
                                                center: Position::new(center_x, center_y),
                                                vertices,
                                                exposure: macro_boolean_to_bool(&center_line.exposure, macro_context)?
                                                    .into(),
                                            })))
                                        }
                                        MacroContent::Outline(outline) => {
                                            // Need at least 3 points to form a polygon
                                            if outline.points.len() < 3 {
                                                warn!("Outline with less than 3 points. outline: {:?}", outline);
                                                return Ok(None);
                                            }

                                            // Get vertices - points are already relative to (0,0)
                                            let mut vertices: Vec<Position> = outline
                                                .points
                                                .iter()
                                                .filter_map(|point| {
                                                    macro_decimal_pair_to_f64(point, macro_context)
                                                        .map(|d| d.into())
                                                        .inspect_err(|err| {
                                                            error!("Error building vertex: {}", err);
                                                        })
                                                        .ok()
                                                })
                                                .collect::<Vec<_>>();

                                            // Get rotation angle and convert to radians
                                            let rotation_degrees = macro_decimal_to_f64(&outline.angle, macro_context)?;
                                            let rotation_radians = rotation_degrees * std::f64::consts::PI / 180.0;

                                            // If there's rotation, apply it to all vertices around (0,0)
                                            if rotation_radians != 0.0 {
                                                let (sin_theta, cos_theta) = rotation_radians.sin_cos();
                                                vertices = vertices
                                                    .into_iter()
                                                    .map(
                                                        |Position {
                                                             x,
                                                             y,
                                                         }| {
                                                            let rotated_x = x * cos_theta - y * sin_theta;
                                                            let rotated_y = x * sin_theta + y * cos_theta;
                                                            (rotated_x, rotated_y).into()
                                                        },
                                                    )
                                                    .collect();
                                            }

                                            Ok(Some(GerberPrimitive::new_polygon(GerberPolygon {
                                                center: (0.0, 0.0).into(), // The flash operation will move this to final position
                                                vertices,
                                                exposure: macro_boolean_to_bool(&outline.exposure, macro_context)?
                                                    .into(),
                                            })))
                                        }
                                        MacroContent::Polygon(polygon) => {
                                            let center = macro_decimal_pair_to_f64(&polygon.center, macro_context)?;

                                            let vertices_count =
                                                macro_integer_to_u32(&polygon.vertices, macro_context)? as usize;
                                            let diameter = macro_decimal_to_f64(&polygon.diameter, macro_context)?;
                                            let rotation_degrees = macro_decimal_to_f64(&polygon.angle, macro_context)?;
                                            let rotation_radians = rotation_degrees * std::f64::consts::PI / 180.0;

                                            // First generate vertices around (0,0)
                                            let radius = diameter / 2.0;
                                            let mut vertices = Vec::with_capacity(vertices_count);
                                            for i in 0..vertices_count {
                                                let angle =
                                                    (2.0 * std::f64::consts::PI * i as f64) / vertices_count as f64;
                                                let x = radius * angle.cos();
                                                let y = radius * angle.sin();

                                                // Apply rotation around macro origin (0,0)
                                                let (sin_theta, cos_theta) = rotation_radians.sin_cos();
                                                let rotated_x = x * cos_theta - y * sin_theta;
                                                let rotated_y = x * sin_theta + y * cos_theta;

                                                vertices.push((rotated_x, rotated_y).into());
                                            }

                                            // Rotate center point around macro origin
                                            let (sin_theta, cos_theta) = rotation_radians.sin_cos();
                                            let rotated_center_x = center.0 * cos_theta - center.1 * sin_theta;
                                            let rotated_center_y = center.0 * sin_theta + center.1 * cos_theta;

                                            Ok(Some(GerberPrimitive::new_polygon(GerberPolygon {
                                                center: (rotated_center_x, rotated_center_y).into(),
                                                vertices,
                                                exposure: macro_boolean_to_bool(&polygon.exposure, macro_context)?
                                                    .into(),
                                            })))
                                        }
                                        MacroContent::Moire(_) => {
                                            error!("Moire not supported");
                                            Ok(None)
                                        }
                                        MacroContent::Thermal(_) => {
                                            error!("Moire not supported");
                                            Ok(None)
                                        }
                                        MacroContent::VariableDefinition(VariableDefinition {
                                            number,
                                            expression,
                                        }) => {
                                            let result = evaluate_expression(&expression, macro_context);
                                            match result {
                                                Ok(value) => {
                                                    macro_context
                                                        .put(*number, value)
                                                        .inspect_err(|error| {
                                                            error!("Error setting variable {}: {}", number, error);
                                                        })
                                                        .ok();
                                                }
                                                Err(cause) => {
                                                    error!("Error evaluating expression {}: {}", expression, cause);
                                                }
                                            };
                                            Ok(None)
                                        }
                                        MacroContent::Comment(_) => {
                                            // Nothing to do
                                            Ok(None)
                                        }
                                    }
                                }

                                let result = process_content(content, &mut macro_context);
                                match result {
                                    Err(cause) => {
                                        error!("Error processing macro content: {:?}, cause: {}", content, cause);
                                    }
                                    Ok(Some(primitive)) => primitive_defs.push(primitive),
                                    Ok(None) => {}
                                }
                            }

                            debug!("primitive_defs: {:?}", primitive_defs);

                            apertures.insert(*code, ApertureKind::Macro(primitive_defs));
                        } else {
                            error!(
                                "Aperture definition references unknown macro. macro_name: {}",
                                macro_name
                            );
                        }
                    }
                    _ => {
                        apertures.insert(*code, ApertureKind::Standard(aperture.clone()));
                    }
                }
            }
        }

        // Third pass: collect all primitives, handle regions

        let mut layer_primitives = Vec::new();
        let mut current_aperture = None;
        let mut current_pos = gerber::position::ZERO;

        // regions are a special case - they are defined by aperture codes
        let mut current_region_vertices: Vec<Position> = Vec::new();
        let mut in_region = false;

        for cmd in doc
            .commands
            .iter()
            .filter_map(|result| result.as_ref().ok())
        {
            match cmd {
                Command::FunctionCode(FunctionCode::GCode(GCode::RegionMode(enabled))) => {
                    if *enabled {
                        // G36 - Begin Region
                        in_region = true;
                        current_region_vertices.clear();
                    } else {
                        // G37 - End Region
                        if in_region && current_region_vertices.len() >= 3 {
                            // Find bounding box
                            let min_x = current_region_vertices
                                .iter()
                                .map(
                                    |Position {
                                         x, ..
                                     }| *x,
                                )
                                .fold(f64::INFINITY, f64::min);
                            let max_x = current_region_vertices
                                .iter()
                                .map(
                                    |Position {
                                         x, ..
                                     }| *x,
                                )
                                .fold(f64::NEG_INFINITY, f64::max);
                            let min_y = current_region_vertices
                                .iter()
                                .map(
                                    |Position {
                                         y, ..
                                     }| *y,
                                )
                                .fold(f64::INFINITY, f64::min);
                            let max_y = current_region_vertices
                                .iter()
                                .map(
                                    |Position {
                                         y, ..
                                     }| *y,
                                )
                                .fold(f64::NEG_INFINITY, f64::max);

                            // Calculate center from bounding box
                            let center_x = (min_x + max_x) / 2.0;
                            let center_y = (min_y + max_y) / 2.0;

                            let center = Position::new(center_x, center_y);

                            // Make vertices relative to center
                            let relative_vertices: Vec<Position> = current_region_vertices
                                .iter()
                                .map(|position| *position - center)
                                .collect();

                            let polygon = GerberPrimitive::new_polygon(GerberPolygon {
                                center: (center_x, center_y).into(),
                                vertices: relative_vertices,
                                exposure: Exposure::Add,
                            });
                            layer_primitives.push(polygon);
                            in_region = false;
                        }
                    }
                }

                Command::FunctionCode(FunctionCode::DCode(DCode::SelectAperture(code))) => {
                    current_aperture = apertures.get(&code);
                }
                Command::FunctionCode(FunctionCode::DCode(DCode::Operation(operation))) => {
                    match operation {
                        Operation::Move(coords) => {
                            let mut end = current_pos;
                            Self::update_position(&mut end, coords);
                            if in_region {
                                // In a region, a move operation starts a new path segment
                                // If we already have vertices, close the current segment
                                if !current_region_vertices.is_empty() {
                                    current_region_vertices.push(*current_region_vertices.first().unwrap());
                                }
                                // Start new segment
                                //current_region_vertices.push(end);
                            }
                            current_pos = end;
                        }
                        Operation::Interpolate(coords, ..) => {
                            let mut end = current_pos;
                            Self::update_position(&mut end, coords);
                            if in_region {
                                // Add vertex to current region
                                current_region_vertices.push(end);
                            } else if let Some(aperture) = current_aperture {
                                match aperture {
                                    ApertureKind::Standard(Aperture::Circle(gerber_types::Circle {
                                        diameter,
                                        ..
                                    })) => {
                                        layer_primitives.push(GerberPrimitive::Line {
                                            start: current_pos,
                                            end,
                                            width: *diameter,
                                            exposure: Exposure::Add,
                                        });
                                    }
                                    _ => {
                                        // TODO support more Apertures (rectangle, obround, etc)
                                    }
                                }
                            }
                            current_pos = end;
                        }
                        Operation::Flash(coords, ..) => {
                            if in_region {
                                warn!("Flash operation found within region - ignoring");
                            } else {
                                Self::update_position(&mut current_pos, coords);

                                if let Some(aperture) = current_aperture {
                                    match aperture {
                                        ApertureKind::Macro(macro_primitives) => {
                                            for primitive in macro_primitives {
                                                let mut primitive = primitive.clone();
                                                // Update the primitive's position based on flash coordinates
                                                match &mut primitive {
                                                    GerberPrimitive::Polygon {
                                                        center, ..
                                                    } => {
                                                        *center += current_pos;
                                                    }
                                                    GerberPrimitive::Circle {
                                                        center, ..
                                                    } => {
                                                        *center += current_pos;
                                                    }
                                                    GerberPrimitive::Rectangle {
                                                        origin, ..
                                                    } => {
                                                        *origin += current_pos;
                                                    }
                                                    GerberPrimitive::Line {
                                                        start,
                                                        end,
                                                        ..
                                                    } => {
                                                        *start += current_pos;
                                                        *end += current_pos;
                                                    }
                                                }
                                                debug!("flashing macro primitive: {:?}", primitive);
                                                layer_primitives.push(primitive);
                                            }
                                        }
                                        ApertureKind::Standard(aperture) => {
                                            match aperture {
                                                Aperture::Circle(circle) => {
                                                    layer_primitives.push(GerberPrimitive::Circle {
                                                        center: current_pos,
                                                        diameter: circle.diameter,
                                                        exposure: Exposure::Add,
                                                    });
                                                }
                                                Aperture::Rectangle(rect) => {
                                                    layer_primitives.push(GerberPrimitive::Rectangle {
                                                        origin: Position::new(
                                                            current_pos.x - rect.x / 2.0,
                                                            current_pos.y - rect.y / 2.0,
                                                        ),
                                                        width: rect.x,
                                                        height: rect.y,
                                                        exposure: Exposure::Add,
                                                    });
                                                }
                                                Aperture::Polygon(polygon) => {
                                                    let radius = polygon.diameter / 2.0;
                                                    let vertices_count = polygon.vertices as usize;
                                                    let mut vertices = Vec::with_capacity(vertices_count);

                                                    // For standard aperture polygon, we need to generate vertices
                                                    // starting at angle 0 and moving counterclockwise
                                                    for i in 0..vertices_count {
                                                        let angle = (2.0 * std::f64::consts::PI * i as f64)
                                                            / vertices_count as f64;
                                                        let x = radius * angle.cos();
                                                        let y = radius * angle.sin();

                                                        // Apply rotation if specified
                                                        let final_position = if let Some(rotation) = polygon.rotation {
                                                            let rot_rad = rotation * std::f64::consts::PI / 180.0;
                                                            let (sin_rot, cos_rot) = rot_rad.sin_cos();
                                                            (x * cos_rot - y * sin_rot, x * sin_rot + y * cos_rot)
                                                                .into()
                                                        } else {
                                                            (x, y).into()
                                                        };

                                                        vertices.push(final_position);
                                                    }

                                                    layer_primitives.push(GerberPrimitive::new_polygon(
                                                        GerberPolygon {
                                                            center: current_pos,
                                                            vertices,
                                                            exposure: Exposure::Add,
                                                        },
                                                    ));
                                                }
                                                Aperture::Obround(rect) => {
                                                    // For an obround, we need to:
                                                    // 1. Create a rectangle for the center part
                                                    // 2. Add two circles (one at each end)
                                                    // The longer dimension determines which way the semicircles go

                                                    let (rect_width, rect_height, circle_centers) = if rect.x > rect.y {
                                                        // Horizontal obround
                                                        let rect_width = rect.x - rect.y; // Subtract circle diameter
                                                        let circle_offset = rect_width / 2.0;
                                                        (rect_width, rect.y, [
                                                            (circle_offset, 0.0),
                                                            (-circle_offset, 0.0),
                                                        ])
                                                    } else {
                                                        // Vertical obround
                                                        let rect_height = rect.y - rect.x; // Subtract circle diameter
                                                        let circle_offset = rect_height / 2.0;
                                                        (rect.x, rect_height, [
                                                            (0.0, circle_offset),
                                                            (0.0, -circle_offset),
                                                        ])
                                                    };

                                                    // Add the center rectangle
                                                    layer_primitives.push(GerberPrimitive::Rectangle {
                                                        origin: Position::new(
                                                            current_pos.x - rect_width / 2.0,
                                                            current_pos.y - rect_height / 2.0,
                                                        ),
                                                        width: rect_width,
                                                        height: rect_height,
                                                        exposure: Exposure::Add,
                                                    });

                                                    // Add the end circles
                                                    let circle_radius = rect.x.min(rect.y) / 2.0;
                                                    for (dx, dy) in circle_centers {
                                                        layer_primitives.push(GerberPrimitive::Circle {
                                                            center: current_pos + (dx, dy).into(),
                                                            diameter: circle_radius * 2.0,
                                                            exposure: Exposure::Add,
                                                        });
                                                    }
                                                }
                                                Aperture::Macro(code, _args) => {
                                                    // if the aperture referred to a macro, and the macro was supported, it will have been handled by the `ApertureKind::Macro` handling.
                                                    warn!("Unsupported macro aperture: {:?}, code: {}", aperture, code);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        layer_primitives
    }

    pub fn paint_gerber(&self, painter: &Painter, view: ViewState, base_color: Color32, use_unique_shape_colors: bool) {
        for (index, primitive) in self
            .gerber_primitives
            .iter()
            .enumerate()
        {
            let color = match use_unique_shape_colors {
                true => generate_pastel_color(index as u64),
                false => base_color,
            };

            match primitive {
                GerberPrimitive::Circle {
                    center,
                    diameter,
                    exposure,
                } => {
                    let color = exposure.to_color(&color);

                    let center = view.translation + center.invert_y().to_vec2() * view.scale;
                    let radius = (*diameter as f32 / 2.0) * view.scale;
                    painter.circle(center.to_pos2(), radius, color, Stroke::NONE);
                }
                GerberPrimitive::Rectangle {
                    origin,
                    width,
                    height,
                    exposure,
                } => {
                    let color = exposure.to_color(&color);

                    // Calculate center-based position
                    let center = view.translation
                        + Vec2::new(
                            origin.x as f32 + *width as f32 / 2.0,     // Add half width to get center
                            -(origin.y as f32 + *height as f32 / 2.0), // Flip Y and add half height
                        ) * view.scale;

                    let size = Vec2::new(*width as f32, *height as f32) * view.scale;
                    let top_left = center - size / 2.0; // Calculate top-left from center

                    painter.rect(
                        Rect::from_min_size(top_left.to_pos2(), size),
                        0.0,
                        color,
                        Stroke::NONE,
                        StrokeKind::Middle,
                    );
                }
                GerberPrimitive::Line {
                    start,
                    end,
                    width,
                    exposure,
                } => {
                    let color = exposure.to_color(&color);

                    let start_position = view.translation + Vec2::new(start.x as f32, -(start.y as f32)) * view.scale;
                    let end_position = view.translation + Vec2::new(end.x as f32, -(end.y as f32)) * view.scale;
                    painter.line_segment(
                        [start_position.to_pos2(), end_position.to_pos2()],
                        Stroke::new((*width as f32) * view.scale, color),
                    );
                    // Draw circles at either end of the line.
                    let radius = (*width as f32 / 2.0) * view.scale;
                    painter.circle(start_position.to_pos2(), radius, color, Stroke::NONE);
                    painter.circle(end_position.to_pos2(), radius, color, Stroke::NONE);
                }
                GerberPrimitive::Polygon {
                    center,
                    exposure,
                    geometry,
                } => {
                    let color = exposure.to_color(&color);
                    let screen_center = Vec2::new(
                        view.translation.x + (center.x as f32) * view.scale,
                        view.translation.y - (center.y as f32) * view.scale
                    );

                    if geometry.is_convex {
                        // Direct convex rendering
                        let screen_vertices: Vec<Pos2> = geometry.relative_vertices.iter()
                            .map(|v| {
                                (screen_center + Vec2::new(
                                    v.x as f32 * view.scale,
                                    -v.y as f32 * view.scale
                                )).to_pos2()
                            })
                            .collect();

                        painter.add(Shape::convex_polygon(screen_vertices, color, Stroke::NONE));
                    } else if let Some(tess) = &geometry.tessellation {
                        // Transform tessellated geometry
                        let vertices: Vec<Vertex> = tess.vertices.iter()
                            .map(|[x, y]| Vertex {
                                pos: (screen_center + Vec2::new(*x * view.scale, -*y * view.scale)).to_pos2(),
                                uv: egui::epaint::WHITE_UV,
                                color,
                            })
                            .collect();

                        painter.add(Shape::Mesh(Arc::new(Mesh {
                            vertices,
                            indices: tess.indices.clone(),
                            texture_id: egui::TextureId::default(),
                        })));
                    }

                    // Debug visualization
                    let debug_vertices: Vec<Pos2> = geometry.relative_vertices.iter()
                        .map(|v| {
                            let point = screen_center + Vec2::new(
                                v.x as f32 * view.scale,
                                -v.y as f32 * view.scale
                            );
                            point.to_pos2()

                        })
                        .collect();

                    for (i, pos) in debug_vertices.iter().enumerate() {
                        painter.text(
                            *pos,
                            Align2::CENTER_CENTER,
                            format!("{}", i),
                            FontId::monospace(8.0),
                            Color32::RED,
                        );
                    }
                }
            }
        }
    }
}

impl GerberViewer {
    pub fn new(_cc: &CreationContext) -> Self {
        _cc.egui_ctx
            .style_mut(|style| style.spacing.scroll = ScrollStyle::solid());
        Self {
            state: None,
            log: Vec::new(),
            coord_input: ("0.0".to_string(), "0.0".to_string()),
            use_unique_shape_colors: false,
        }
    }

    fn handle_quit(&self, ctx: &egui::Context) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    fn draw_crosshair(painter: &Painter, origin_screen_pos: Vec2, color: Color32) {
        // Calculate viewport bounds to extend lines across entire view
        let viewport = painter.clip_rect();

        // Draw a horizontal line (extending across viewport)
        painter.line_segment(
            [
                Pos2::new(viewport.min.x, origin_screen_pos.y),
                Pos2::new(viewport.max.x, origin_screen_pos.y),
            ],
            Stroke::new(1.0, color),
        );

        // Draw a vertical line (extending across viewport)
        painter.line_segment(
            [
                Pos2::new(origin_screen_pos.x, viewport.min.y),
                Pos2::new(origin_screen_pos.x, viewport.max.y),
            ],
            Stroke::new(1.0, color),
        );
    }
}

impl eframe::App for GerberViewer {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Disable text wrapping
        //
        // egui text layouting tries to utilize minimal width possible
        ctx.style_mut(|style| {
            style.wrap_mode = Some(egui::TextWrapMode::Extend);
        });

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Add layers...").clicked() {
                        self.add_layer_files();
                    }
                    ui.add_enabled_ui(self.state.is_some(), |ui| {
                        if ui.button("Reload all layers").clicked() {
                            self.reload_all_layer_files();
                        }
                        if ui.button("Close all").clicked() {
                            self.close_all();
                        }
                    });
                    if ui.button("Quit").clicked() {
                        self.handle_quit(ui.ctx());
                    }
                });
            });

            ui.horizontal(|ui| {
                if ui.button("Open").clicked() {
                    self.add_layer_files();
                }

                ui.separator();

                ui.add_enabled_ui(self.state.is_some(), |ui| {
                    let x_is_valid = self
                        .coord_input
                        .0
                        .parse::<f64>()
                        .is_ok();
                    let mut x_editor = egui::TextEdit::singleline(&mut self.coord_input.0)
                        .desired_width(50.0)
                        .hint_text("X");
                    if !x_is_valid {
                        x_editor = x_editor.background_color(Color32::DARK_RED);
                    }
                    ui.add(x_editor);

                    let y_is_valid = self
                        .coord_input
                        .1
                        .parse::<f64>()
                        .is_ok();
                    let mut y_editor = egui::TextEdit::singleline(&mut self.coord_input.1)
                        .desired_width(50.0)
                        .hint_text("Y");
                    if !y_is_valid {
                        y_editor = y_editor.background_color(Color32::DARK_RED);
                    }
                    ui.add(y_editor);

                    let x = self.coord_input.0.parse::<f64>();
                    let y = self.coord_input.1.parse::<f64>();

                    let enabled = x.is_ok() && y.is_ok() && self.state.is_some();

                    ui.add_enabled_ui(enabled, |ui| {
                        if ui.button("Locate").clicked() {
                            // Safety: ui is disabled unless x and y are `Result::ok`
                            let (x, y) = (x.as_ref().unwrap(), y.as_ref().unwrap());
                            self.state
                                .as_mut()
                                .unwrap()
                                .locate_view(*x, *y);
                        }
                        if ui.button("Move").clicked() {
                            // Safety: ui is disabled unless x and y are `Result::ok`
                            let (x, y) = (x.as_ref().unwrap(), y.as_ref().unwrap());
                            self.state
                                .as_mut()
                                .unwrap()
                                .move_view((*x, *y).into());
                        }
                    });

                    if ui.button("Reset").clicked() {
                        self.state
                            .as_mut()
                            .unwrap()
                            .request_reset();
                    }

                    ui.separator();

                    ui.toggle_value(&mut self.use_unique_shape_colors, "Unique shape colors");
                });
            })
        });

        let panel_fill_color = ctx.style().visuals.panel_fill;
        let light_panel_fill = ctx
            .style()
            .visuals
            .widgets
            .inactive
            .bg_fill;
        // We just want to get rid of the margin in the panel, but we have to find the right color too...
        let panel_frame = Frame::default()
            .inner_margin(0.0)
            .fill(panel_fill_color);

        egui::TopBottomPanel::bottom("bottom_panel")
            .resizable(true)
            .default_height(150.0)
            .min_height(80.0)
            .frame(panel_frame)
            .show(ctx, |ui| {
                let cell_style = Style {
                    ..Style::default()
                };

                egui_taffy::tui(ui, ui.id().with("bottom_panel_content"))
                    .reserve_available_space()
                    .style(Style {
                        flex_direction: taffy::FlexDirection::Column,
                        size: percent(1.),
                        justify_items: Some(taffy::AlignItems::FlexStart),
                        align_items: Some(taffy::AlignItems::Stretch),
                        ..Style::default()
                    })
                    .show(|tui| {
                        tui.style(Style {
                            flex_grow: 0.0,
                            ..cell_style.clone()
                        })
                        .add(|tui| {
                            tui.ui(|ui| {
                                ui.horizontal(|ui| {
                                    if ui.button("Clear").clicked() {
                                        self.clear_log();
                                    }
                                });
                            });
                        });

                        tui.separator();

                        tui.style(Style {
                            flex_grow: 1.0,
                            min_size: Size {
                                width: auto(),
                                height: Length(100.0),
                            },
                            ..cell_style.clone()
                        })
                        .add_with_border(|tui| {
                            tui.ui(|ui| {
                                let text_height = egui::TextStyle::Body
                                    .resolve(ui.style())
                                    .size
                                    .max(ui.spacing().interact_size.y);

                                let text_color = ui.style().visuals.text_color();

                                TableBuilder::new(ui)
                                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                                    .min_scrolled_height(80.0)
                                    .column(Column::auto())
                                    .column(Column::remainder())
                                    .striped(true)
                                    .resizable(true)
                                    .auto_shrink([false, false])
                                    .stick_to_bottom(true)
                                    .header(20.0, |mut header| {
                                        header.col(|ui| {
                                            ui.strong("Log Level");
                                        });
                                        header.col(|ui| {
                                            ui.strong("Message");
                                        });
                                    })
                                    .body(|body| {
                                        body.rows(text_height, self.log.len(), |mut row| {
                                            if let Some(log_item) = self.log.get(row.index()) {
                                                let color = match log_item {
                                                    AppLogItem::Error(_) => Color32::LIGHT_RED,
                                                    AppLogItem::Info(_) => text_color,
                                                    AppLogItem::Warning(_) => Color32::LIGHT_YELLOW,
                                                };

                                                row.col(|ui| {
                                                    ui.colored_label(color, log_item.level());
                                                });
                                                row.col(|ui| {
                                                    // FIXME the width of this column expands when rows with longer messages are scrolled-to.
                                                    //       the issue is apparent after loading a gerber file, and then expanding the window horizontally
                                                    //       you'll see that table's scrollbar is not on the right of the panel, but somewhere in the middle.
                                                    //       if you then scroll the table, the scrollbar will move to the right.
                                                    ui.colored_label(color, log_item.message());
                                                });
                                            }
                                        });
                                    });
                            })
                        });

                        let style = tui.egui_style_mut();
                        style.visuals.panel_fill = light_panel_fill;

                        // Status bar
                        tui.style(Style {
                            flex_grow: 0.0,
                            ..cell_style.clone()
                        })
                        .add_with_background_color(|tui| {
                            tui.ui(|ui| {
                                ui.horizontal(|ui| {
                                    if let Some(state) = &self.state {
                                        let unit_text = match state
                                            .layers
                                            .first()
                                            .unwrap()
                                            .1
                                            .gerber_doc
                                            .units
                                        {
                                            Some(gerber_types::Unit::Millimeters) => "MM",
                                            Some(gerber_types::Unit::Inches) => "Inches",
                                            None => "Unknown Units",
                                        };
                                        ui.label(format!("Layer units: {}", unit_text));

                                        ui.separator();

                                        let (x, y) = state
                                            .cursor_gerber_coords
                                            .map(
                                                |Position {
                                                     x,
                                                     y,
                                                 }| {
                                                    fn format_coord(coord: f64) -> String {
                                                        format!("{:.3}", coord)
                                                    }
                                                    (format_coord(x), format_coord(y))
                                                },
                                            )
                                            .unwrap_or(("N/A".to_string(), "N/A".to_string()));

                                        ui.label(format!("Cursor: X={} Y={} {}", x, y, unit_text));
                                    } else {
                                        ui.label("No file loaded");
                                    }
                                });
                            });
                        });
                    });
            });

        egui::SidePanel::left("left_panel")
            .resizable(true)
            .show(ctx, |ui| {
                if let Some(state) = &mut self.state {
                    for (layer_view_state, layer) in state.layers.iter_mut() {
                        ui.horizontal(|ui| {
                            ui.color_edit_button_srgba(&mut layer_view_state.color);
                            ui.checkbox(
                                &mut layer_view_state.enabled,
                                layer
                                    .path
                                    .file_stem()
                                    .unwrap()
                                    .to_string_lossy()
                                    .to_string(),
                            );
                        });
                    }
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            let response = ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::drag());
            let viewport = response.rect;
            if let Some(state) = &mut self.state {
                if state.needs_initial_view {
                    state.reset_view(viewport);
                }

                state.center_screen_pos = Some(viewport.center().to_vec2());
                state.origin_screen_pos = Some(state.gerber_to_screen_coords(gerber::position::ZERO));

                state.update_cursor_position(&response, ui);
                state.handle_panning(&response, ui);
                state.handle_zooming(&response, viewport, ui);

                debug!("view: {:?}, view bbox scale: {}, viewport_center: {}, origin_screen_pos: {}", state.view, INITIAL_GERBER_AREA_PERCENT, state.center_screen_pos.unwrap(), state.origin_screen_pos.unwrap());

                let painter = ui.painter().with_clip_rect(viewport);
                for (layer_state, layer) in state.layers.iter() {
                    if layer_state.enabled {
                        layer.paint_gerber(&painter, state.view, layer_state.color, self.use_unique_shape_colors);
                    }
                }

                // Draw origin crosshair
                if let Some(position) = state.origin_screen_pos {
                    Self::draw_crosshair(&painter, position, Color32::BLUE);
                }
                if let Some(position) = state.center_screen_pos {
                    Self::draw_crosshair(&painter, position, Color32::LIGHT_GRAY);
                }
            }
        });
    }
}

mod gerber_expressions {
    use std::collections::hash_map::Entry;
    use std::str::Chars;

    use egui::ahash::HashMap;
    use gerber_parser::gerber_types::{MacroBoolean, MacroDecimal, MacroInteger};
    use thiserror::Error;

    /// Gerber spec 2024.05 - 4.5.4.3 - "The undefined variables are 0".
    #[derive(Debug, Default)]
    pub struct MacroContext {
        variables: HashMap<u32, f64>,
    }

    impl MacroContext {
        pub fn get(&self, variable: &u32) -> f64 {
            self.variables
                .get(&variable)
                .copied()
                .unwrap_or(0.0)
        }

        pub fn put(&mut self, variable: u32, decimal: f64) -> Result<&mut f64, MacroContextError> {
            match self.variables.entry(variable) {
                Entry::Occupied(_) => Err(MacroContextError::AlreadyDefined(variable)),
                Entry::Vacant(entry) => Ok(entry.insert(decimal)),
            }
        }
    }

    #[derive(Error, Debug)]
    pub enum MacroContextError {
        /// Gerber spec (2024.05) - 4.5.4.3 - "Macro variables cannot be redefined"
        #[error("Already defined. variable: {0}")]
        AlreadyDefined(u32),
    }

    pub fn macro_decimal_to_f64(
        macro_decimal: &MacroDecimal,
        context: &MacroContext,
    ) -> Result<f64, ExpressionEvaluationError> {
        match macro_decimal {
            MacroDecimal::Value(value) => Ok(*value),
            MacroDecimal::Variable(id) => Ok(context.get(id)),
            MacroDecimal::Expression(args) => evaluate_expression(args, context),
        }
    }

    pub fn macro_boolean_to_bool(
        macro_boolean: &MacroBoolean,
        context: &MacroContext,
    ) -> Result<bool, ExpressionEvaluationError> {
        match macro_boolean {
            MacroBoolean::Value(value) => Ok(*value),
            MacroBoolean::Variable(id) => Ok(context.get(id) == 1.0),
            MacroBoolean::Expression(args) => evaluate_expression(args, context).map(|value| value != 0.0),
        }
    }

    pub fn macro_integer_to_u32(
        macro_integer: &MacroInteger,
        context: &MacroContext,
    ) -> Result<u32, ExpressionEvaluationError> {
        match macro_integer {
            MacroInteger::Value(value) => Ok(*value),
            MacroInteger::Variable(id) => Ok(context.get(id) as u32),
            MacroInteger::Expression(args) => evaluate_expression(args, context).map(|value| value as u32),
        }
    }

    pub fn macro_decimal_pair_to_f64(
        input: &(MacroDecimal, MacroDecimal),
        context: &MacroContext,
    ) -> Result<(f64, f64), ExpressionEvaluationError> {
        let (x, y) = (
            macro_decimal_to_f64(&input.0, context)?,
            macro_decimal_to_f64(&input.1, context)?,
        );
        Ok((x, y))
    }

    #[derive(Error, Debug)]
    pub enum ExpressionEvaluationError {
        #[error("Unexpected character: {0}")]
        UnexpectedChar(char),
        #[error("Unexpected end of input")]
        UnexpectedEnd,
        #[error("Invalid number")]
        InvalidNumber,
    }

    /// Evaluates a Gerber macro expression using a recursive descent parser.
    pub fn evaluate_expression(expr: &String, ctx: &MacroContext) -> Result<f64, ExpressionEvaluationError> {
        let mut parser = Parser::new(expr, ctx);
        let result = parser.parse_expression()?;
        if parser.peek().is_some() {
            Err(ExpressionEvaluationError::UnexpectedChar(parser.peek().unwrap()))
        } else {
            Ok(result)
        }
    }

    /// Tokenizer and Parser
    ///
    /// Initially Generated via ChatGPT - AI: https://chatgpt.com/share/68124813-8ec4-800f-ad20-797f57d6af18
    struct Parser<'a> {
        chars: Chars<'a>,
        lookahead: Option<char>,
        ctx: &'a MacroContext,
    }

    impl<'a> Parser<'a> {
        fn new(expr: &'a str, ctx: &'a MacroContext) -> Self {
            let mut chars = expr.chars();
            let lookahead = chars.next();
            Self {
                chars,
                lookahead,
                ctx,
            }
        }

        fn peek(&self) -> Option<char> {
            self.lookahead
        }

        fn bump(&mut self) -> Option<char> {
            let curr = self.lookahead;
            self.lookahead = self.chars.next();
            curr
        }

        fn eat_whitespace(&mut self) {
            while let Some(c) = self.peek() {
                if c.is_whitespace() {
                    self.bump();
                } else {
                    break;
                }
            }
        }

        fn parse_expression(&mut self) -> Result<f64, ExpressionEvaluationError> {
            let mut value = self.parse_term()?;
            loop {
                self.eat_whitespace();
                match self.peek() {
                    Some('+') => {
                        self.bump();
                        value += self.parse_term()?;
                    }
                    Some('-') => {
                        self.bump();
                        value -= self.parse_term()?;
                    }
                    _ => break,
                }
            }
            Ok(value)
        }

        fn parse_term(&mut self) -> Result<f64, ExpressionEvaluationError> {
            let mut value = self.parse_factor()?;
            loop {
                self.eat_whitespace();
                match self.peek() {
                    Some('*') => {
                        self.bump();
                        value *= self.parse_factor()?;
                    }
                    Some('/') => {
                        self.bump();
                        value /= self.parse_factor()?;
                    }
                    // gerber spec uses 'x' for multiplication (why Camco, why...)
                    Some('x') => {
                        self.bump();
                        value *= self.parse_factor()?;
                    }
                    _ => break,
                }
            }
            Ok(value)
        }

        fn parse_factor(&mut self) -> Result<f64, ExpressionEvaluationError> {
            self.eat_whitespace();
            match self.peek() {
                Some('(') => {
                    self.bump(); // consume '('
                    let value = self.parse_expression()?;
                    self.eat_whitespace();
                    if self.bump() != Some(')') {
                        return Err(ExpressionEvaluationError::UnexpectedEnd);
                    }
                    Ok(value)
                }
                Some('$') => self.parse_variable(),
                Some(c) if c.is_ascii_digit() || c == '.' || c == '-' => self.parse_number(),
                Some(c) => Err(ExpressionEvaluationError::UnexpectedChar(c)),
                None => Err(ExpressionEvaluationError::UnexpectedEnd),
            }
        }

        fn parse_number(&mut self) -> Result<f64, ExpressionEvaluationError> {
            let mut s = String::new();
            if self.peek() == Some('-') {
                s.push('-');
                self.bump();
            }

            while let Some(c) = self.peek() {
                if c.is_ascii_digit() || c == '.' {
                    s.push(c);
                    self.bump();
                } else {
                    break;
                }
            }

            s.parse::<f64>()
                .map_err(|_| ExpressionEvaluationError::InvalidNumber)
        }

        fn parse_variable(&mut self) -> Result<f64, ExpressionEvaluationError> {
            self.bump(); // consume '$'
            let mut s = String::new();

            while let Some(c) = self.peek() {
                if c.is_ascii_digit() {
                    s.push(c);
                    self.bump();
                } else {
                    break;
                }
            }

            let id: u32 = s
                .parse()
                .map_err(|_| ExpressionEvaluationError::InvalidNumber)?;
            Ok(self.ctx.get(&id))
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_addition_same_variable() {
            let mut ctx = MacroContext::default();
            ctx.put(1, 5.0).unwrap();

            let expr = "$1+$1".to_string();
            let result = evaluate_expression(&expr, &ctx).unwrap();
            assert_eq!(result, 10.0);
        }

        #[test]
        fn test_division_two_variables() {
            let mut ctx = MacroContext::default();
            ctx.put(1, 5.0).unwrap();
            ctx.put(2, 2.0).unwrap();

            let expr = "$1/$2".to_string();
            let result = evaluate_expression(&expr, &ctx).unwrap();
            assert_eq!(result, 2.5);
        }

        #[test]
        fn test_multiplication_two_variables_using_x() {
            let mut ctx = MacroContext::default();
            ctx.put(1, 5.0).unwrap();
            ctx.put(2, 2.0).unwrap();

            let expr = "$1x$2".to_string();
            let result = evaluate_expression(&expr, &ctx).unwrap();
            assert_eq!(result, 10.0);
        }

        #[test]
        fn test_multiplication_two_variables_using_asterix() {
            let mut ctx = MacroContext::default();
            ctx.put(1, 5.0).unwrap();
            ctx.put(2, 2.0).unwrap();

            let expr = "$1*$2".to_string();
            let result = evaluate_expression(&expr, &ctx).unwrap();
            assert_eq!(result, 10.0);
        }

        #[test]
        fn test_subtraction_and_division() {
            let mut ctx = MacroContext::default();
            ctx.put(1, 5.0).unwrap();
            ctx.put(2, 2.0).unwrap();

            let expr = "$1-$2/$2".to_string(); // 5 - (2 / 2) = 4
            let result = evaluate_expression(&expr, &ctx).unwrap();
            assert_eq!(result, 4.0);
        }

        #[test]
        fn test_parentheses_with_sub_and_div() {
            let mut ctx = MacroContext::default();
            ctx.put(1, 5.0).unwrap();
            ctx.put(2, 2.0).unwrap();

            let expr = "($1-$2)/$2".to_string(); // (5 - 2) / 2 = 1.5
            let result = evaluate_expression(&expr, &ctx).unwrap();
            assert_eq!(result, 1.5);
        }
    }
}

pub fn generate_pastel_color(index: u64) -> Color32 {
    let mut rng = SmallRng::seed_from_u64(index);

    let hue = rng.random_range(0.0..360.0);
    let saturation = rng.random_range(0.2..0.3);
    let value = rng.random_range(0.8..1.0);

    let (r, g, b) = hsv_to_rgb(hue, saturation, value);
    Color32::from_rgb(r, g, b)
}

fn hsv_to_rgb(hue: f32, saturation: f32, value: f32) -> (u8, u8, u8) {
    let hue = hue % 360.0;
    let chroma = value * saturation;
    let x = chroma * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let m = value - chroma;

    let sector = (hue / 60.0) as u8;
    let (r1, g1, b1) = match sector {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        5 => (chroma, 0.0, x),
        _ => (0.0, 0.0, 0.0), // Unreachable due to modulus
    };

    // Calculate each RGB component and clamp to valid range
    let red = ((r1 + m) * 255.0)
        .round()
        .clamp(0.0, 255.0) as u8;
    let green = ((g1 + m) * 255.0)
        .round()
        .clamp(0.0, 255.0) as u8;
    let blue = ((b1 + m) * 255.0)
        .round()
        .clamp(0.0, 255.0) as u8;

    (red, green, blue)
}
