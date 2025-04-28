use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io;
use std::io::BufReader;
use std::path::PathBuf;

use earcut::Earcut;
use eframe::emath::Vec2;
use eframe::{CreationContext, NativeOptions, egui, run_native};
use egui::ahash::HashMap;
use egui::style::ScrollStyle;
use egui::{Color32, Context, Frame, Painter, Pos2, Rect, Response, Ui};
use egui_extras::{Column, TableBuilder};
use egui_taffy::taffy::Dimension::Length;
use egui_taffy::taffy::prelude::{auto, percent};
use egui_taffy::taffy::{Size, Style};
use egui_taffy::{TuiBuilderLogic, taffy};
use epaint::{Shape, Stroke, StrokeKind};
use gerber_parser::gerber_doc::GerberDoc;
use gerber_parser::parser::parse_gerber;
use gerber_types::{
    Aperture, ApertureDefinition, Command, Coordinates, ExtendedCode, FunctionCode, GCode, MacroContent, MacroDecimal,
    Operation,
};
use log::{error, info, warn};
use rfd::FileDialog;
use thiserror::Error;
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
    state: Option<GerberLayer>,
    log: Vec<AppLogItem>,
}

struct GerberLayer {
    color: Color32,
    gerber_doc: GerberDoc,
    path: PathBuf,
    view: ViewState,
    needs_initial_view: bool,
    bounding_box: BoundingBox,
    gerber_primitives: Vec<GerberPrimitive>,
    cursor_position: Option<(f64, f64)>,
}

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

#[derive(Debug)]
struct BoundingBox {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

impl Default for BoundingBox {
    fn default() -> Self {
        Self {
            min_x: f64::MAX,
            min_y: f64::MAX,
            max_x: f64::MIN,
            max_y: f64::MIN,
        }
    }
}

#[derive(Debug, Clone)]
enum Exposure {
    CutOut,
    Add,
}

impl From<bool> for Exposure {
    fn from(value: bool) -> Self {
        match value {
            true => Exposure::Add,
            false => Exposure::CutOut,
        }
    }
}

impl Exposure {
    fn to_color(&self, color: &Color32) -> Color32 {
        match self {
            Exposure::CutOut => Color32::BLACK,
            Exposure::Add => *color,
        }
    }
}

#[derive(Debug, Clone)]
enum GerberPrimitive {
    Circle {
        x: f64,
        y: f64,
        diameter: f64,
        exposure: Exposure,
    },
    Rectangle {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        exposure: Exposure,
    },
    Line {
        start: (f64, f64),
        end: (f64, f64),
        width: f64,
        exposure: Exposure,
    },
    Polygon {
        center: (f64, f64),
        /// Relative to center
        vertices: Vec<(f64, f64)>,
        exposure: Exposure,
        is_convex: bool,
        triangles: Vec<Vec<Pos2>>,
    },
}

struct GerberPolygon {
    center: (f64, f64),
    /// Relative to center
    vertices: Vec<(f64, f64)>,
    exposure: Exposure,
}

impl GerberPolygon {
    /// Checks if a polygon is convex by verifying that all cross products
    /// between consecutive edges have the same sign
    pub fn is_convex(&self) -> bool {
        if self.vertices.len() < 3 {
            return true;
        }

        let n = self.vertices.len();
        let mut sign = 0;

        for i in 0..n {
            let p1 = self.vertices[i];
            let p2 = self.vertices[(i + 1) % n];
            let p3 = self.vertices[(i + 2) % n];

            let v1_x = p2.0 - p1.0;
            let v1_y = p2.1 - p1.1;
            let v2_x = p3.0 - p2.0;
            let v2_y = p3.1 - p2.1;

            // Cross product in 2D
            let cross = v1_x * v2_y - v1_y * v2_x;

            if sign == 0 {
                sign = if cross > 0.0 { 1 } else { -1 };
            } else if (cross > 0.0 && sign < 0) || (cross < 0.0 && sign > 0) {
                return false;
            }
        }

        true
    }
}

impl GerberPrimitive {
    fn new_polygon(polygon: GerberPolygon) -> Self {
        let is_convex = polygon.is_convex();
        let mut triangles = Vec::new();

        if !is_convex {
            // Convert vertices to flat array for triangulation
            let vertices: Vec<[f64; 2]> = polygon
                .vertices
                .iter()
                .map(|(x, y)| [*x, *y])
                .collect();

            let mut indices = Vec::new();
            let mut earcut = Earcut::new();
            earcut.earcut(vertices.clone(), &[], &mut indices);

            // Convert indices back to triangle vertices
            triangles = indices
                .chunks(3)
                .map(|chunk: &[usize]| {
                    vec![
                        Pos2::new(vertices[chunk[0]][0] as f32, vertices[chunk[0]][1] as f32),
                        Pos2::new(vertices[chunk[1]][0] as f32, vertices[chunk[1]][1] as f32),
                        Pos2::new(vertices[chunk[2]][0] as f32, vertices[chunk[2]][1] as f32),
                    ]
                })
                .collect();
        }

        GerberPrimitive::Polygon {
            center: polygon.center,
            vertices: polygon.vertices,
            exposure: polygon.exposure,
            is_convex,
            triangles,
        }
    }
}

#[derive(Debug)]
enum ApertureKind<'macros> {
    Standard(Aperture),
    Macro(&'macros NamedPrimitive),
}

#[derive(Debug, Clone)]
struct NamedPrimitive {
    name: String,
    primitive: GerberPrimitive,
}

#[derive(Error, Debug)]
enum AppError {
    #[error("No file selected")]
    NoFileSelected,
    #[error("IO Error. cause: {0:?}")]
    IoError(io::Error),
}

enum AppLogItem {
    Info(String),
    Warning(String),
    Error(String),
}

impl AppLogItem {
    pub fn message(&self) -> &str {
        match self {
            AppLogItem::Info(message) => message,
            AppLogItem::Warning(message) => message,
            AppLogItem::Error(message) => message,
        }
    }

    pub fn level(&self) -> &'static str {
        match self {
            AppLogItem::Info(_) => "info",
            AppLogItem::Warning(_) => "warning",
            AppLogItem::Error(_) => "error",
        }
    }
}

impl Display for AppLogItem {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AppLogItem::Info(message) => f.write_fmt(format_args!("Info: {}", message)),
            AppLogItem::Warning(message) => f.write_fmt(format_args!("Warning: {}", message)),
            AppLogItem::Error(message) => f.write_fmt(format_args!("Error: {}", message)),
        }
    }
}

impl GerberViewer {
    /// FIXME: Blocks main thread when file selector is open
    fn open_gerber_file(&mut self) {
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
        let path = FileDialog::new()
            .add_filter("Gerber Files", &["gbr", "gbl", "gbo", "gbs", "gko", "gko", "gto"])
            .pick_file()
            .ok_or(AppError::NoFileSelected)?;

        self.parse_gerber_file(path)?;

        Ok(())
    }
    pub fn parse_gerber_file(&mut self, path: PathBuf) -> Result<(), AppError> {
        let file = File::open(path.clone()).map_err(AppError::IoError)?;
        let reader = BufReader::new(file);

        let gerber_doc: GerberDoc = parse_gerber(reader);

        let log = gerber_doc
            .commands
            .iter()
            .map(|c| match c {
                Ok(command) => AppLogItem::Info(format!("{:?}", command)),
                Err(error) => AppLogItem::Error(format!("{:?}", error)),
            })
            .collect::<Vec<_>>();
        self.log.extend(log);

        let gerber_primitives = GerberLayer::build_primitives(&gerber_doc);
        let bounding_box = GerberLayer::calculate_bounding_box(&gerber_primitives);

        self.state = Some(GerberLayer {
            color: Color32::LIGHT_GRAY,
            gerber_doc,
            path,
            gerber_primitives,
            view: ViewState {
                translation: Default::default(),
                scale: 0.0,
            },
            needs_initial_view: true,
            bounding_box,
            cursor_position: None,
        });

        let message = "Gerber file parsed successfully";
        info!("{}", message);
        self.log
            .push(AppLogItem::Info(message.to_string()));

        Ok(())
    }

    pub fn close_file(&mut self) {
        self.state = None;
    }

    pub fn clear_log(&mut self) {
        self.log.clear();
    }
}

impl GerberLayer {
    fn calculate_bounding_box(primitives: &Vec<GerberPrimitive>) -> BoundingBox {
        let mut bbox = BoundingBox::default();

        // Calculate bounding box
        for primitive in primitives {
            match primitive {
                GerberPrimitive::Circle {
                    x,
                    y,
                    diameter,
                    ..
                } => {
                    let radius = diameter / 2.0;
                    bbox.min_x = bbox.min_x.min(*x - radius);
                    bbox.min_y = bbox.min_y.min(*y - radius);
                    bbox.max_x = bbox.max_x.max(*x + radius);
                    bbox.max_y = bbox.max_y.max(*y + radius);
                }
                GerberPrimitive::Rectangle {
                    x,
                    y,
                    width,
                    height,
                    ..
                } => {
                    bbox.min_x = bbox.min_x.min(*x);
                    bbox.min_y = bbox.min_y.min(*y);
                    bbox.max_x = bbox.max_x.max(*x + width);
                    bbox.max_y = bbox.max_y.max(*y + height);
                }
                GerberPrimitive::Line {
                    start,
                    end,
                    width,
                    ..
                } => {
                    let radius = width / 2.0;
                    for &(x, y) in &[start, end] {
                        bbox.min_x = bbox.min_x.min(x - radius);
                        bbox.min_y = bbox.min_y.min(y - radius);
                        bbox.max_x = bbox.max_x.max(x + radius);
                        bbox.max_y = bbox.max_y.max(y + radius);
                    }
                }
                GerberPrimitive::Polygon {
                    center,
                    vertices,
                    ..
                } => {
                    for &(dx, dy) in vertices {
                        let x = center.0 + dx;
                        let y = center.1 + dy;
                        bbox.min_x = bbox.min_x.min(x);
                        bbox.min_y = bbox.min_y.min(y);
                        bbox.max_x = bbox.max_x.max(x);
                        bbox.max_y = bbox.max_y.max(y);
                    }
                }
            }
        }

        bbox
    }

    fn build_primitives(doc: &GerberDoc) -> Vec<GerberPrimitive> {
        #[derive(Debug, Default)]
        struct MacroContext {
            variables: HashMap<u32, f64>,
        }

        fn macro_decimal_to_f64(macro_decimal: &MacroDecimal, context: &MacroContext) -> Option<f64> {
            match macro_decimal {
                MacroDecimal::Value(value) => Some(*value),
                MacroDecimal::Variable(id) => context.variables.get(id).copied(),
            }
        }

        fn macro_decimal_pair_to_f64(
            input: &(MacroDecimal, MacroDecimal),
            context: &MacroContext,
        ) -> Option<(f64, f64)> {
            let (x, y) = (
                macro_decimal_to_f64(&input.0, context)?,
                macro_decimal_to_f64(&input.1, context)?,
            );
            Some((x, y))
        }

        let mut macro_definitions = HashMap::default();

        // TODO get the macro context from the GerberDoc
        let macro_context = MacroContext::default();

        // First pass: collect aperture macros
        for cmd in doc
            .commands
            .iter()
            .filter_map(|result| result.as_ref().ok())
        {
            if let Command::ExtendedCode(ExtendedCode::ApertureMacro(macro_def)) = cmd {
                for content in &macro_def.content {
                    // TODO add logging context which includes the macro name
                    fn build_primitive(
                        content: &MacroContent,
                        macro_context: &MacroContext,
                    ) -> Option<GerberPrimitive> {
                        match content {
                            MacroContent::Circle(circle) => {
                                let Some(diameter) = macro_decimal_to_f64(&circle.diameter, &macro_context) else {
                                    return None;
                                };

                                let Some((center_x, center_y)) =
                                    macro_decimal_pair_to_f64(&circle.center, &macro_context)
                                else {
                                    return None;
                                };

                                // Get rotation angle and convert to radians
                                let rotation_radians = if let Some(angle) = &circle.angle {
                                    macro_decimal_to_f64(angle, &macro_context)? * std::f64::consts::PI / 180.0
                                } else {
                                    0.0
                                };

                                // Apply rotation to center coordinates around macro origin (0,0)
                                let (sin_theta, cos_theta) = rotation_radians.sin_cos();
                                let rotated_x = center_x * cos_theta - center_y * sin_theta;
                                let rotated_y = center_x * sin_theta + center_y * cos_theta;

                                Some(GerberPrimitive::Circle {
                                    x: rotated_x,
                                    y: rotated_y,
                                    diameter,
                                    exposure: circle.exposure.into(),
                                })
                            }
                            MacroContent::VectorLine(vector_line) => {
                                // Get start and end points
                                let Some((start_x, start_y)) =
                                    macro_decimal_pair_to_f64(&vector_line.start, &macro_context)
                                else {
                                    return None;
                                };
                                let Some((end_x, end_y)) = macro_decimal_pair_to_f64(&vector_line.end, &macro_context)
                                else {
                                    return None;
                                };
                                let width = macro_decimal_to_f64(&vector_line.width, &macro_context)?;

                                // Get rotation and prepare rotation matrix
                                let rotation_angle = macro_decimal_to_f64(&vector_line.angle, &macro_context)?;
                                let rotation_radians = rotation_angle * std::f64::consts::PI / 180.0;
                                let (sin_theta, cos_theta) = rotation_radians.sin_cos();

                                // First rotate start and end points around (0,0)
                                let rotated_start_x = start_x * cos_theta - start_y * sin_theta;
                                let rotated_start_y = start_x * sin_theta + start_y * cos_theta;
                                let rotated_end_x = end_x * cos_theta - end_y * sin_theta;
                                let rotated_end_y = end_x * sin_theta + end_y * cos_theta;

                                // Calculate center point and length after rotation
                                let center_x = (rotated_start_x + rotated_end_x) / 2.0;
                                let center_y = (rotated_start_y + rotated_end_y) / 2.0;
                                let dx = rotated_end_x - rotated_start_x;
                                let dy = rotated_end_y - rotated_start_y;
                                let length = (dx * dx + dy * dy).sqrt();

                                Some(GerberPrimitive::Rectangle {
                                    x: center_x,
                                    y: center_y,
                                    width: length,
                                    height: width, // height is the line width
                                    exposure: vector_line.exposure.into(),
                                })
                            }
                            MacroContent::CenterLine(center_line) => {
                                // Get center point and dimensions
                                let Some((center_x, center_y)) =
                                    macro_decimal_pair_to_f64(&center_line.center, &macro_context)
                                else {
                                    return None;
                                };
                                let Some((width, height)) =
                                    macro_decimal_pair_to_f64(&center_line.dimensions, &macro_context)
                                else {
                                    return None;
                                };

                                // Get rotation and prepare rotation matrix
                                let rotation_angle = macro_decimal_to_f64(&center_line.angle, &macro_context)?;
                                let rotation_radians = rotation_angle * std::f64::consts::PI / 180.0;
                                let (sin_theta, cos_theta) = rotation_radians.sin_cos();

                                // Rotate center point around macro origin (0,0)
                                let rotated_center_x = center_x * cos_theta - center_y * sin_theta;
                                let rotated_center_y = center_x * sin_theta + center_y * cos_theta;

                                Some(GerberPrimitive::Rectangle {
                                    x: rotated_center_x,
                                    y: rotated_center_y,
                                    width,
                                    height,
                                    exposure: center_line.exposure.into(),
                                })
                            }
                            MacroContent::Outline(outline) => {
                                // Need at least 3 points to form a polygon
                                if outline.points.len() < 3 {
                                    warn!("Outline with less than 3 points. outline: {:?}", outline);
                                    return None;
                                }

                                // Get vertices - points are already relative to (0,0)
                                let mut vertices: Vec<(f64, f64)> = outline
                                    .points
                                    .iter()
                                    .filter_map(|point| macro_decimal_pair_to_f64(point, &macro_context))
                                    .collect();

                                // Get rotation angle and convert to radians
                                let rotation_degrees = macro_decimal_to_f64(&outline.angle, &macro_context)?;
                                let rotation_radians = rotation_degrees * std::f64::consts::PI / 180.0;

                                // If there's rotation, apply it to all vertices around (0,0)
                                if rotation_radians != 0.0 {
                                    let (sin_theta, cos_theta) = rotation_radians.sin_cos();
                                    vertices = vertices
                                        .into_iter()
                                        .map(|(x, y)| {
                                            let rotated_x = x * cos_theta - y * sin_theta;
                                            let rotated_y = x * sin_theta + y * cos_theta;
                                            (rotated_x, rotated_y)
                                        })
                                        .collect();
                                }

                                Some(GerberPrimitive::new_polygon(GerberPolygon {
                                    center: (0.0, 0.0), // The flash operation will move this to final position
                                    vertices,
                                    exposure: outline.exposure.into(),
                                }))
                            }
                            MacroContent::Polygon(polygon) => {
                                let Some(center) = macro_decimal_pair_to_f64(&polygon.center, &macro_context) else {
                                    return None;
                                };

                                let vertices_count = polygon.vertices as usize;
                                let diameter = macro_decimal_to_f64(&polygon.diameter, &macro_context)?;
                                let rotation_degrees = macro_decimal_to_f64(&polygon.angle, &macro_context)?;
                                let rotation_radians = rotation_degrees * std::f64::consts::PI / 180.0;

                                // First generate vertices around (0,0)
                                let radius = diameter / 2.0;
                                let mut vertices = Vec::with_capacity(vertices_count);
                                for i in 0..vertices_count {
                                    let angle = (2.0 * std::f64::consts::PI * i as f64) / vertices_count as f64;
                                    let x = radius * angle.cos();
                                    let y = radius * angle.sin();

                                    // Apply rotation around macro origin (0,0)
                                    let (sin_theta, cos_theta) = rotation_radians.sin_cos();
                                    let rotated_x = x * cos_theta - y * sin_theta;
                                    let rotated_y = x * sin_theta + y * cos_theta;

                                    vertices.push((rotated_x, rotated_y));
                                }

                                // Rotate center point around macro origin
                                let (sin_theta, cos_theta) = rotation_radians.sin_cos();
                                let rotated_center_x = center.0 * cos_theta - center.1 * sin_theta;
                                let rotated_center_y = center.0 * sin_theta + center.1 * cos_theta;

                                Some(GerberPrimitive::new_polygon(GerberPolygon {
                                    center: (rotated_center_x, rotated_center_y),
                                    vertices,
                                    exposure: polygon.exposure.into(),
                                }))
                            }
                            MacroContent::Moire(_) => None,
                            MacroContent::Thermal(_) => None,
                            MacroContent::VariableDefinition(_) => None,
                            MacroContent::Comment(_) => None,
                        }
                    }

                    let primitive = build_primitive(content, &macro_context);

                    if let Some(primitive) = primitive {
                        let old_definition = macro_definitions.insert(macro_def.name.clone(), NamedPrimitive {
                            name: macro_def.name.clone(),
                            primitive,
                        });

                        if old_definition.is_some() {
                            warn!(
                                "Unsupported macro definition: {}, only one primitive currently supported. Overriding previous definition",
                                macro_def.name
                            );
                        }
                    }
                }
            }
        }

        // Second pass - collect aperture definitions

        let mut apertures = HashMap::default();

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
                    Aperture::Other(macro_name) => {
                        // Handle macro-based apertures
                        if let Some(primitive_def) = macro_definitions.get(macro_name) {
                            apertures.insert(*code, ApertureKind::Macro(primitive_def));
                        }
                    }
                    _ => {
                        apertures.insert(*code, ApertureKind::Standard(aperture.clone()));
                    }
                }
            }
        }

        // Third pass: collect all primitives, handle regions

        let mut primitives = Vec::new();
        let mut current_aperture = None;
        let mut current_pos = (0.0, 0.0);

        // regions are a special case - they are defined by aperture codes
        let mut current_region_vertices: Vec<(f64, f64)> = Vec::new();
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
                                .map(|(x, _)| *x)
                                .fold(f64::INFINITY, f64::min);
                            let max_x = current_region_vertices
                                .iter()
                                .map(|(x, _)| *x)
                                .fold(f64::NEG_INFINITY, f64::max);
                            let min_y = current_region_vertices
                                .iter()
                                .map(|(_, y)| *y)
                                .fold(f64::INFINITY, f64::min);
                            let max_y = current_region_vertices
                                .iter()
                                .map(|(_, y)| *y)
                                .fold(f64::NEG_INFINITY, f64::max);

                            // Calculate center from bounding box
                            let center_x = (min_x + max_x) / 2.0;
                            let center_y = (min_y + max_y) / 2.0;

                            // Make vertices relative to center
                            let relative_vertices: Vec<(f64, f64)> = current_region_vertices
                                .iter()
                                .map(|(x, y)| (x - center_x, y - center_y))
                                .collect();

                            let polygon = GerberPrimitive::new_polygon(GerberPolygon {
                                center: (center_x, center_y),
                                vertices: relative_vertices,
                                exposure: Exposure::Add,
                            });
                            primitives.push(polygon);
                            in_region = false;
                        }
                    }
                }

                Command::FunctionCode(FunctionCode::DCode(gerber_types::DCode::SelectAperture(code))) => {
                    current_aperture = apertures.get(&code);
                }
                Command::FunctionCode(FunctionCode::DCode(gerber_types::DCode::Operation(operation))) => {
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
                                        primitives.push(GerberPrimitive::Line {
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
                                        ApertureKind::Macro(named_primitive) => {
                                            let mut primitive = named_primitive.primitive.clone();
                                            // Update the primitive's position based on flash coordinates
                                            match &mut primitive {
                                                GerberPrimitive::Polygon {
                                                    center, ..
                                                } => {
                                                    *center = current_pos;
                                                }
                                                GerberPrimitive::Circle {
                                                    x,
                                                    y,
                                                    ..
                                                } => {
                                                    *x = current_pos.0;
                                                    *y = current_pos.1;
                                                }
                                                GerberPrimitive::Rectangle {
                                                    x,
                                                    y,
                                                    ..
                                                } => {
                                                    *x = current_pos.0;
                                                    *y = current_pos.1;
                                                }
                                                _ => {
                                                    warn!(
                                                        "macro uses a primitive that is not supported.  named_primitive: {:?}",
                                                        named_primitive
                                                    );
                                                }
                                            }
                                            primitives.push(primitive);
                                        }
                                        ApertureKind::Standard(aperture) => {
                                            match aperture {
                                                Aperture::Circle(circle) => {
                                                    primitives.push(GerberPrimitive::Circle {
                                                        x: current_pos.0,
                                                        y: current_pos.1,
                                                        diameter: circle.diameter,
                                                        exposure: Exposure::Add,
                                                    });
                                                }
                                                Aperture::Rectangle(rect) => {
                                                    primitives.push(GerberPrimitive::Rectangle {
                                                        x: current_pos.0 - rect.x / 2.0,
                                                        y: current_pos.1 - rect.y / 2.0,
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
                                                        let (final_x, final_y) =
                                                            if let Some(rotation) = polygon.rotation {
                                                                let rot_rad = rotation * std::f64::consts::PI / 180.0;
                                                                let (sin_rot, cos_rot) = rot_rad.sin_cos();
                                                                (x * cos_rot - y * sin_rot, x * sin_rot + y * cos_rot)
                                                            } else {
                                                                (x, y)
                                                            };

                                                        vertices.push((final_x, final_y));
                                                    }

                                                    primitives.push(GerberPrimitive::new_polygon(GerberPolygon {
                                                        center: (current_pos.0, current_pos.1),
                                                        vertices,
                                                        exposure: Exposure::Add,
                                                    }));
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
                                                    primitives.push(GerberPrimitive::Rectangle {
                                                        x: current_pos.0 - rect_width / 2.0,
                                                        y: current_pos.1 - rect_height / 2.0,
                                                        width: rect_width,
                                                        height: rect_height,
                                                        exposure: Exposure::Add,
                                                    });

                                                    // Add the end circles
                                                    let circle_radius = rect.x.min(rect.y) / 2.0;
                                                    for (dx, dy) in circle_centers {
                                                        primitives.push(GerberPrimitive::Circle {
                                                            x: current_pos.0 + dx,
                                                            y: current_pos.1 + dy,
                                                            diameter: circle_radius * 2.0,
                                                            exposure: Exposure::Add,
                                                        });
                                                    }
                                                }
                                                Aperture::Other(code) => {
                                                    // if the aperture referred to a macro, and the macro was supported, it will have been handled by the `ApertureKind::Macro` handling.
                                                    warn!("Unsupported aperture: {:?}, code: {}", aperture, code);
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

        primitives
    }

    fn update_position(current_pos: &mut (f64, f64), coords: &Coordinates) {
        *current_pos = (
            coords
                .x
                .map(|value| value.into())
                .unwrap_or(current_pos.0),
            coords
                .y
                .map(|value| value.into())
                .unwrap_or(current_pos.1),
        )
    }

    fn calculate_initial_view(&mut self, viewport: Rect) {
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
            self.cursor_position = Some(self.screen_to_gerber_coords(pointer_pos.to_vec2()));
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
    pub fn screen_to_gerber_coords(&self, screen_pos: Vec2) -> (f64, f64) {
        let gerber_pos = (screen_pos - self.view.translation) / self.view.scale;
        (gerber_pos.x as f64, gerber_pos.y as f64)
    }

    /// Convert from gerber coordinates using view transformation
    pub fn gerber_to_screen_coords(&self, gerber_pos: (f64, f64)) -> Vec2 {
        let gerber_pos = Vec2::new(gerber_pos.0 as f32, gerber_pos.1 as f32);
        let origin_screen_pos = self.view.translation + (gerber_pos * self.view.scale);
        origin_screen_pos
    }

    pub fn paint_gerber(&self, painter: Painter) {
        for primitive in &self.gerber_primitives {
            match primitive {
                GerberPrimitive::Circle {
                    x,
                    y,
                    diameter,
                    exposure,
                } => {
                    let color = exposure.to_color(&self.color);

                    let center = self.view.translation + Vec2::new(*x as f32, -(*y as f32)) * self.view.scale;
                    let radius = (*diameter as f32 / 2.0) * self.view.scale;
                    painter.circle(center.to_pos2(), radius, color, Stroke::NONE);
                }
                GerberPrimitive::Rectangle {
                    x,
                    y,
                    width,
                    height,
                    exposure,
                } => {
                    let color = exposure.to_color(&self.color);

                    // Calculate center-based position
                    let center = self.view.translation
                        + Vec2::new(
                            *x as f32 + *width as f32 / 2.0,     // Add half width to get center
                            -(*y as f32 + *height as f32 / 2.0), // Flip Y and add half height
                        ) * self.view.scale;

                    let size = Vec2::new(*width as f32, *height as f32) * self.view.scale;
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
                    let color = exposure.to_color(&self.color);

                    let start_position =
                        self.view.translation + Vec2::new(start.0 as f32, -(start.1 as f32)) * self.view.scale;
                    let end_position =
                        self.view.translation + Vec2::new(end.0 as f32, -(end.1 as f32)) * self.view.scale;
                    painter.line_segment(
                        [start_position.to_pos2(), end_position.to_pos2()],
                        Stroke::new((*width as f32) * self.view.scale, color),
                    );
                    // Draw circles at either end of the line.
                    let radius = (*width as f32 / 2.0) * self.view.scale;
                    painter.circle(start_position.to_pos2(), radius, color, Stroke::NONE);
                    painter.circle(end_position.to_pos2(), radius, color, Stroke::NONE);
                }
                GerberPrimitive::Polygon {
                    center,
                    vertices,
                    exposure,
                    is_convex,
                    triangles,
                } => {
                    let color = exposure.to_color(&self.color);

                    let screen_center =
                        self.view.translation + Vec2::new(center.0 as f32, -(center.1 as f32)) * self.view.scale;

                    // Draw the polygon
                    match is_convex {
                        true => {
                            // Convert vertices to screen space
                            let screen_vertices: Vec<Pos2> = vertices
                                .iter()
                                .map(|(dx, dy)| {
                                    let screen_pos =
                                        screen_center + Vec2::new(*dx as f32, -(*dy as f32)) * self.view.scale;
                                    screen_pos.to_pos2()
                                })
                                .collect();

                            painter.add(Shape::convex_polygon(screen_vertices, color, Stroke::NONE));
                        }
                        false => {
                            // Transform stored triangles to screen space and draw them
                            for triangle in triangles {
                                let screen_triangle: Vec<Pos2> = triangle
                                    .iter()
                                    .map(|pos| (screen_center + Vec2::new(pos.x, -pos.y) * self.view.scale).to_pos2())
                                    .collect();
                                painter.add(Shape::convex_polygon(screen_triangle, color, Stroke::NONE));
                            }
                        }
                    };
                }
            }
        }

        // Draw origin crosshair
        let origin_screen_pos = self.gerber_to_screen_coords((0.0, 0.0));
        Self::draw_origin_crosshair(painter, origin_screen_pos);
    }

    fn draw_origin_crosshair(painter: Painter, origin_screen_pos: Vec2) {
        // Calculate viewport bounds to extend lines across entire view
        let viewport = painter.clip_rect();

        // Draw a horizontal line (extending across viewport)
        painter.line_segment(
            [
                Pos2::new(viewport.min.x, origin_screen_pos.y),
                Pos2::new(viewport.max.x, origin_screen_pos.y),
            ],
            Stroke::new(1.0, Color32::BLUE),
        );

        // Draw a vertical line (extending across viewport)
        painter.line_segment(
            [
                Pos2::new(origin_screen_pos.x, viewport.min.y),
                Pos2::new(origin_screen_pos.x, viewport.max.y),
            ],
            Stroke::new(1.0, Color32::BLUE),
        );
    }
}

impl GerberViewer {
    pub fn new(_cc: &CreationContext) -> Self {
        _cc.egui_ctx
            .style_mut(|style| style.spacing.scroll = ScrollStyle::solid());
        Self {
            state: None,
            log: Vec::new(),
        }
    }

    fn handle_quit(&self, ctx: &egui::Context) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
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
                    if ui.button("Open").clicked() {
                        self.open_gerber_file();
                    }
                    ui.add_enabled_ui(self.state.is_some(), |ui| {
                        if ui.button("Close").clicked() {
                            self.close_file();
                        }
                    });
                    if ui.button("Quit").clicked() {
                        self.handle_quit(ui.ctx());
                    }
                });
            });

            ui.horizontal(|ui| {
                if ui.button("Open Gerber File").clicked() {
                    self.open_gerber_file();
                }
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
                                                row.col(|ui| {
                                                    ui.label(log_item.level());
                                                });
                                                row.col(|ui| {
                                                    // FIXME the width of this column expands when rows with longer messages are scrolled-to.
                                                    //       the issue is apparent after loading a gerber file, and then expanding the window horizontally
                                                    //       you'll see that table's scrollbar is not on the right of the panel, but somewhere in the middle.
                                                    //       if you then scroll the table, the scrollbar will move to the right.
                                                    ui.label(log_item.message());
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
                                        let unit_text = match state.gerber_doc.units {
                                            Some(gerber_types::Unit::Millimeters) => "MM",
                                            Some(gerber_types::Unit::Inches) => "Inches",
                                            None => "Unknown Units",
                                        };
                                        ui.label(format!("Units: {}", unit_text));

                                        ui.separator();

                                        if let Some((x, y)) = state.cursor_position {
                                            ui.label(format!("X: {:.3} Y: {:.3} {}", x, y, unit_text));
                                        } else {
                                            ui.label("X: -- Y: --");
                                        }
                                    } else {
                                        ui.label("No file loaded");
                                    }
                                });
                            });
                        });
                    });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            let response = ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::drag());
            let viewport = response.rect;

            if let Some(state) = &mut self.state {
                if state.needs_initial_view {
                    state.calculate_initial_view(viewport);
                }

                state.update_cursor_position(&response, ui);
                state.handle_panning(&response, ui);
                state.handle_zooming(&response, viewport, ui);

                let painter = ui.painter().with_clip_rect(viewport);
                state.paint_gerber(painter);
            }
        });
    }
}
