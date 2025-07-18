use std::fs::File;
use std::io;
use std::io::BufReader;
use std::path::PathBuf;

use eframe::emath::Vec2;
use eframe::{CreationContext, NativeOptions, egui, run_native};
use egui::style::ScrollStyle;
use egui::{Color32, Context, Frame, Id, Modal, Pos2, Rect, RichText, Ui};
use egui_extras::{Column, TableBuilder};
use egui_taffy::taffy::Dimension::Length;
use egui_taffy::taffy::prelude::{auto, length, percent};
use egui_taffy::taffy::{Size, Style};
use egui_taffy::{TuiBuilderLogic, taffy, tui};
use epaint::FontFamily;
use gerber_viewer::gerber_parser::parse;
use gerber_viewer::gerber_parser::{GerberDoc, ParseError};
use gerber_viewer::gerber_types::Unit;
use gerber_viewer::{
    BoundingBox, DisplayInfo, GerberLayer, GerberRenderer, GerberTransform, Invert, Mirroring, RenderConfiguration,
    ToPos2, ToVector, UiState, ViewState, draw_crosshair, draw_outline, generate_pastel_color,
};
use log::{debug, error, info, trace};
use logging::AppLogItem;
use nalgebra::{Point2, Vector2};
use rfd::FileDialog;
use thiserror::Error;

mod logging;

type Vector = Vector2<f64>;
type Position = Point2<f64>;

const VECTOR_ZERO: Vector = Vector::new(0.0, 0.0);

const INITIAL_GERBER_AREA_PERCENT: f32 = 0.95;
const DEFAULT_STEP: f64 = 0.05;
const STEP_SPEED: f64 = 0.05;
const STEP_SCALE: f64 = 0.5;

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

    enable_bounding_box_outline: bool,

    is_about_modal_open: bool,
    step: f64,
    config: RenderConfiguration,
    display_info: DisplayInfo,
}

impl GerberViewer {
    fn show_about_modal(&mut self) {
        if self.is_about_modal_open {
            return;
        }

        self.is_about_modal_open = true;
    }
}

struct LayerViewState {
    enabled: bool,
    color: Color32,
    transform: GerberTransform,
}

impl LayerViewState {
    fn new(color: Color32) -> Self {
        Self {
            enabled: true,
            color,
            transform: GerberTransform::default(),
        }
    }
}

struct GerberViewState {
    view: ViewState,
    needs_view_fitting: bool,
    needs_view_centering: bool,
    needs_bbox_update: bool,
    bounding_box: BoundingBox,
    bounding_box_vertices: Vec<Position>,
    layers: Vec<(PathBuf, LayerViewState, GerberLayer, GerberDoc)>,
    ui_state: UiState,
    transform: GerberTransform,
}

impl Default for GerberViewState {
    fn default() -> Self {
        Self {
            view: Default::default(),
            needs_view_fitting: true,
            needs_view_centering: false,
            needs_bbox_update: true,
            bounding_box: BoundingBox::default(),
            bounding_box_vertices: vec![],
            layers: vec![],
            transform: GerberTransform::default(),
            ui_state: Default::default(),
        }
    }
}

impl GerberViewState {
    pub fn reset(&mut self) {
        self.needs_bbox_update = true;
        self.needs_view_fitting = true;
        self.needs_view_centering = false;

        self.transform = GerberTransform {
            rotation: 0.0,
            mirroring: Mirroring::default(),
            origin: VECTOR_ZERO,
            offset: VECTOR_ZERO,
            scale: 1.0,
        };

        for (_, layer_view_state, _, _) in self.layers.iter_mut() {
            layer_view_state.transform = GerberTransform::default();
            layer_view_state.enabled = true;
        }
    }

    pub fn add_layer(
        &mut self,
        path: PathBuf,
        layer_view_state: LayerViewState,
        layer: GerberLayer,
        gerber_doc: GerberDoc,
    ) {
        self.layers
            .push((path, layer_view_state, layer, gerber_doc));
        self.update_bbox_from_layers();
        self.request_fit_view();
    }

    fn update_bbox_from_layers(&mut self) {
        let mut bbox = BoundingBox::default();

        for (layer_index, (_, layer_view_state, layer, _)) in self
            .layers
            .iter()
            .enumerate()
            .filter(|(_index, (_path, view_state, layer, _))| view_state.enabled && !layer.is_empty())
        {
            let layer_bbox = &layer.bounding_box();

            let image_transform_matrix = layer.image_transform().to_matrix();
            let render_transform_matrix = self.transform.to_matrix();
            let layer_matrix = layer_view_state.transform.to_matrix();

            let matrix = image_transform_matrix * render_transform_matrix * layer_matrix;

            let layer_bbox = layer_bbox.apply_transform_matrix(&matrix);

            debug!("layer bbox: {:?}", layer_bbox);
            bbox.min.x = f64::min(bbox.min.x, layer_bbox.min.x);
            bbox.min.y = f64::min(bbox.min.y, layer_bbox.min.y);
            bbox.max.x = f64::max(bbox.max.x, layer_bbox.max.x);
            bbox.max.y = f64::max(bbox.max.y, layer_bbox.max.y);
            debug!("view bbox after layer. layer: {}, bbox: {:?}", layer_index, bbox);
        }

        self.bounding_box_vertices = bbox.vertices();
        debug!("view vertices: {:?}", self.bounding_box_vertices);

        self.bounding_box = bbox;
        self.needs_bbox_update = false;
    }

    pub fn request_bbox_reset(&mut self) {
        self.needs_bbox_update = true;
    }

    pub fn request_fit_view(&mut self) {
        self.needs_view_fitting = true;
    }

    pub fn request_center_view(&mut self) {
        self.needs_view_centering = true;
    }

    fn fit_view(&mut self, viewport: Rect) {
        self.update_bbox_from_layers();
        self.view
            .fit_view(viewport, &self.bounding_box, INITIAL_GERBER_AREA_PERCENT);
        self.needs_view_fitting = false;
    }

    fn center_view(&mut self, viewport: Rect) {
        self.view
            .center_view(viewport, &self.bounding_box);
        self.needs_view_centering = false;
    }

    /// Convert to gerber coordinates using view transformation
    pub fn screen_to_gerber_coords(&self, screen_pos: Pos2) -> Position {
        let gerber_pos = (screen_pos - self.view.translation) / self.view.scale;
        Position::new(gerber_pos.x as f64, gerber_pos.y as f64).invert_y()
    }

    /// Convert from gerber coordinates using view transformation
    pub fn gerber_to_screen_coords(&self, gerber_pos: Position) -> Pos2 {
        let gerber_pos = gerber_pos.invert_y();
        (gerber_pos * self.view.scale as f64).to_pos2() + self.view.translation
    }

    /// X and Y are in GERBER units.
    pub fn move_view(&mut self, position: Position) {
        trace!("move view. x: {}, y: {}", position.x, position.y);
        trace!("view translation (before): {:?}", self.view.translation);

        let mut gerber_coords = self.screen_to_gerber_coords(self.view.translation.to_pos2());
        gerber_coords += position.to_vector();
        trace!("gerber_coords: {:?}", self.view.translation);
        let screen_coords = self.gerber_to_screen_coords(gerber_coords);

        trace!("screen_cords: {:?}", screen_coords);

        let delta = screen_coords - self.view.translation;
        trace!("delta: {:?}", delta);

        self.view.translation -= delta.to_vec2();
        trace!("view translation (after): {:?}", self.view.translation);
    }

    /// X and Y are in GERBER units.
    pub fn locate_view(&mut self, x: f64, y: f64) {
        trace!("locate view. x: {}, y: {}", x, y);
        self.view.translation = Vec2::new(
            self.ui_state.center_screen_pos.x - (x as f32 * self.view.scale),
            self.ui_state.center_screen_pos.y + (y as f32 * self.view.scale),
        );
        trace!("view translation (after): {:?}", self.view.translation);
    }
}

#[derive(Error, Debug)]
enum AppError {
    #[error("No file selected")]
    NoFileSelected,
    #[error("IO Error. cause: {0:?}")]
    IoError(io::Error),

    #[error("Parser error. cause: {0:?}")]
    ParserError(ParseError),
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
            .add_filter("Gerber Files", &[
                "gbr", "gbl", "gbo", "gbs", "gko", "gto", "gdl", "gtl", "gtp", "gts",
            ])
            .add_filter("All Files", &["*"])
            .pick_files()
            .ok_or(AppError::NoFileSelected)?;

        for path in paths {
            self.add_gerber_layer_from_file(path)?;
        }

        Ok(())
    }

    pub fn add_gerber_layer_from_file(&mut self, path: PathBuf) -> Result<(), AppError> {
        let (gerber_doc, commands) = Self::parse_gerber(&mut self.log, &path)?;

        let state = self.state.get_or_insert_default();

        let layer_count = state.layers.len();
        let color = generate_pastel_color(layer_count as u64);

        let layer = GerberLayer::new(commands);
        let layer_view_state = LayerViewState::new(color);

        state.add_layer(path, layer_view_state, layer, gerber_doc);

        Ok(())
    }

    fn parse_gerber(
        log: &mut Vec<AppLogItem>,
        path: &PathBuf,
    ) -> Result<(GerberDoc, Vec<gerber_viewer::gerber_types::Command>), AppError> {
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

        let gerber_doc: GerberDoc = parse(reader).map_err(|(_partial_doc, error)| AppError::ParserError(error))?;

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

        let commands = gerber_doc
            .commands
            .iter()
            .filter_map(|c| match c {
                Ok(command) => Some(command.clone()),
                Err(_) => None,
            })
            .collect::<Vec<gerber_viewer::gerber_parser::gerber_types::Command>>();

        Ok((gerber_doc, commands))
    }

    pub fn reload_all_layer_files(&mut self) {
        let Some(state) = &mut self.state else { return };

        for (path, _state, layer, doc) in state.layers.iter_mut() {
            if let Ok((gerber_doc, commands)) = Self::parse_gerber(&mut self.log, &path) {
                *layer = GerberLayer::new(commands);
                *doc = gerber_doc;
            }
        }
        state.request_bbox_reset();
    }

    pub fn close_all(&mut self) {
        self.state = None;
    }

    pub fn clear_log(&mut self) {
        self.log.clear();
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
            config: RenderConfiguration::default(),
            enable_bounding_box_outline: true,

            is_about_modal_open: false,
            step: DEFAULT_STEP,

            // TODO update the display information based on the current monitor
            display_info: DisplayInfo::new()
                // Example based on an ACER Predator 37" monitor
                .with_dpi(3840.0 / 37.0, 2160.0 / 20.875),
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
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("🗁 Add layers...").clicked() {
                        self.add_layer_files();
                    }
                    ui.add_enabled_ui(self.state.is_some(), |ui| {
                        if ui
                            .button("🔃 Reload all layers")
                            .clicked()
                        {
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
                ui.menu_button("View", |ui| {
                    ui.checkbox(&mut self.config.use_unique_shape_colors, "🎉 Unique shape colors");
                    ui.checkbox(&mut self.config.use_shape_numbering, "＃ Shape numbering");
                    ui.checkbox(&mut self.config.use_vertex_numbering, "＃ Vertex numbering (polygons)");
                    ui.checkbox(&mut self.enable_bounding_box_outline, "☐ Draw bounding box");
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("About").clicked() {
                        self.show_about_modal();
                    }
                })
            });

            ui.horizontal(|ui| {
                if ui.button("🗁").clicked() {
                    self.add_layer_files();
                }
                ui.add_enabled_ui(self.state.is_some(), |ui| {
                    if ui.button("🔃").clicked() {
                        self.reload_all_layer_files();
                    }
                });

                ui.separator();

                ui.toggle_value(&mut self.config.use_unique_shape_colors, "🎉");
                ui.toggle_value(&mut self.config.use_shape_numbering, "＃");
                ui.toggle_value(&mut self.config.use_vertex_numbering, "＃");
                ui.toggle_value(&mut self.enable_bounding_box_outline, "☐");

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

                    let enabled = x.is_ok() && y.is_ok();

                    ui.add_enabled_ui(enabled, |ui| {
                        if ui.button("⛶ Go To").clicked() {
                            // Safety: ui is disabled unless x and y are `Result::ok`
                            let (x, y) = (x.as_ref().unwrap(), y.as_ref().unwrap());
                            self.state
                                .as_mut()
                                .unwrap()
                                .locate_view(*x, *y);
                        }
                        if ui.button("➡ Move by").clicked() {
                            // Safety: ui is disabled unless x and y are `Result::ok`
                            let (x, y) = (x.as_ref().unwrap(), y.as_ref().unwrap());
                            self.state
                                .as_mut()
                                .unwrap()
                                .move_view(Position::new(*x, *y));
                        }
                    });

                    ui.separator();

                    let mut changed = false;

                    ui.label("Zoom:");
                    let zoom: Option<(f32, Unit)> = self
                        .state
                        .as_mut()
                        .map(|state| {
                            state
                                .layers
                                .first()
                                .unwrap()
                                .3
                                .units
                                .map(|units| (state, units))
                        })
                        .flatten()
                        .map(|(state, units)| {
                            (
                                state
                                    .view
                                    .zoom_level_percent(units, &self.display_info),
                                units,
                            )
                        });

                    let mut zoom_level = zoom.map_or(100.0, |(zoom, _)| zoom);

                    changed |= ui
                        .add(egui::DragValue::new(&mut zoom_level))
                        .changed();

                    let mut translation = self
                        .state
                        .as_ref()
                        .map_or(Vec2::ZERO, |state| state.view.translation);
                    ui.label("X:");
                    changed |= ui
                        .add(egui::DragValue::new(&mut translation.x))
                        .changed();

                    ui.label("Y:");
                    changed |= ui
                        .add(egui::DragValue::new(&mut translation.y))
                        .changed();

                    ui.separator();

                    ui.label("Step:");
                    ui.add(
                        egui::DragValue::new(&mut self.step)
                            .fixed_decimals(2)
                            .range(0.0..=100.0)
                            .speed(STEP_SPEED * STEP_SCALE),
                    );

                    let mut design_origin = self
                        .state
                        .as_ref()
                        .map_or(VECTOR_ZERO, |state| state.transform.origin + state.transform.offset);

                    ui.label("Rotation/Mirror Origin X:");
                    changed |= ui
                        .add(
                            egui::DragValue::new(&mut design_origin.x)
                                .fixed_decimals(2)
                                .speed(self.step * STEP_SCALE),
                        )
                        .changed();

                    ui.label("Y:");
                    changed |= ui
                        .add(
                            egui::DragValue::new(&mut design_origin.y)
                                .fixed_decimals(2)
                                .speed(self.step * STEP_SCALE),
                        )
                        .changed();

                    let mut rotation = self
                        .state
                        .as_ref()
                        .map_or(0.0, |state| state.transform.rotation);
                    changed |= ui.drag_angle(&mut rotation).changed();

                    ui.separator();

                    let mut design_offset = self
                        .state
                        .as_ref()
                        .map_or(VECTOR_ZERO, |state| state.transform.offset);
                    ui.label("Design Offset X:");
                    changed |= ui
                        .add(
                            egui::DragValue::new(&mut design_offset.x)
                                .fixed_decimals(2)
                                .speed(self.step * STEP_SCALE),
                        )
                        .changed();

                    ui.label("Y:");
                    changed |= ui
                        .add(
                            egui::DragValue::new(&mut design_offset.y)
                                .fixed_decimals(2)
                                .speed(self.step * STEP_SCALE),
                        )
                        .changed();

                    ui.separator();

                    ui.label("Mirror");
                    let mut mirroring = self
                        .state
                        .as_ref()
                        .map_or(Mirroring::default(), |state| state.transform.mirroring);
                    changed |= ui
                        .toggle_value(&mut mirroring.x, "X")
                        .changed();
                    changed |= ui
                        .toggle_value(&mut mirroring.y, "Y")
                        .changed();

                    if changed {
                        if let Some(state) = &mut self.state {
                            match zoom {
                                Some((initial_zoom_level, units)) if zoom_level != initial_zoom_level => {
                                    state
                                        .view
                                        .set_zoom_level_percent(zoom_level, units, &self.display_info);
                                    state.needs_view_centering = true;
                                }
                                _ => {}
                            }

                            state.view.translation = translation;
                            state.transform.offset = design_offset;
                            state.transform.origin = design_origin - design_offset;
                            state.transform.rotation = rotation;
                            state.transform.mirroring = mirroring;

                            state.request_bbox_reset();
                        }

                        ctx.request_repaint();
                    }

                    if ui.button("Reset").clicked() {
                        self.state.as_mut().unwrap().reset();
                        self.step = DEFAULT_STEP;
                        ctx.request_repaint();
                    }

                    ui.separator();

                    if ui.button("Fit").clicked() {
                        self.state
                            .as_mut()
                            .unwrap()
                            .request_fit_view();
                        ctx.request_repaint();
                    }

                    if ui.button("Center").clicked() {
                        self.state
                            .as_mut()
                            .unwrap()
                            .request_center_view();
                        ctx.request_repaint();
                    }
                });
                ui.separator();
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
                        .add(|tui| {
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
                                        let unit_text = match state.layers.first().unwrap().3.units {
                                            Some(gerber_viewer::gerber_parser::gerber_types::Unit::Millimeters) => "MM",
                                            Some(gerber_viewer::gerber_parser::gerber_types::Unit::Inches) => "Inches",
                                            None => "Unknown Units",
                                        };
                                        ui.label(format!("Layer units: {}", unit_text));

                                        ui.separator();

                                        let (x, y) = state
                                            .ui_state
                                            .cursor_gerber_coords
                                            .map(|position| {
                                                fn format_coord(coord: f64) -> String {
                                                    format!("{:.3}", coord)
                                                }
                                                (format_coord(position.x), format_coord(position.y))
                                            })
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
                    let mut request_bbox_reset = false;
                    for (path, layer_view_state, _layer, _doc) in state.layers.iter_mut() {
                        ui.horizontal(|ui| {
                            ui.color_edit_button_srgba(&mut layer_view_state.color);
                            let height = ui.min_size().y;

                            let mut changed = false;
                            let mut origin = layer_view_state.transform.origin + layer_view_state.transform.offset;

                            changed |= ui
                                .add_sized([50.0, height], |ui: &mut Ui| {
                                    ui.drag_angle(&mut layer_view_state.transform.rotation)
                                })
                                .changed();

                            changed |= ui
                                .toggle_value(&mut layer_view_state.transform.mirroring.x, "X")
                                .changed();

                            changed |= ui
                                .add_sized([50.0, height], |ui: &mut Ui| {
                                    ui.add(egui::DragValue::new(&mut origin.x)
                                        .fixed_decimals(4)
                                        .speed(STEP_SPEED * STEP_SCALE))
                                })
                                .changed();

                            changed |= ui
                                .add_sized([50.0, height], |ui: &mut Ui| {
                                    ui.add(egui::DragValue::new(&mut layer_view_state.transform.offset.x)
                                        .fixed_decimals(4)
                                        .speed(STEP_SPEED * STEP_SCALE)
                                    )
                                })
                                .changed();

                            changed |= ui
                                .toggle_value(&mut layer_view_state.transform.mirroring.y, "Y")
                                .changed();

                            changed |= ui
                                .add_sized([50.0, height], |ui: &mut Ui| {
                                    ui.add(egui::DragValue::new(&mut origin.y)
                                        .fixed_decimals(4)
                                        .speed(STEP_SPEED * STEP_SCALE)
                                    )
                                })
                                .changed();

                            changed |= ui
                                .add_sized([50.0, height], |ui: &mut Ui| {
                                    ui.add(egui::DragValue::new(&mut layer_view_state.transform.offset.y)
                                        .fixed_decimals(4)
                                        .speed(STEP_SPEED * STEP_SCALE)
                                    )
                                })
                                .changed();

                            changed |= ui
                                .add_sized([50.0, height], |ui: &mut Ui| {
                                    ui.add(egui::DragValue::new(&mut layer_view_state.transform.scale)
                                        .fixed_decimals(4)
                                        .range(0.0..=100.0)
                                        .speed(STEP_SPEED * STEP_SCALE))
                                })
                                .changed();

                            changed |= ui
                                .checkbox(
                                    &mut layer_view_state.enabled,
                                    path.file_stem()
                                        .unwrap()
                                        .to_string_lossy()
                                        .to_string(),
                                )
                                .clicked();

                            if changed {
                                layer_view_state.transform.origin = origin - layer_view_state.transform.offset;

                                request_bbox_reset = true;
                            }
                        });
                    }

                    if request_bbox_reset {
                        state.request_bbox_reset();
                        ctx.request_repaint();
                    }
                } else {
                    ui.centered_and_justified(|ui| {
                        Frame::default().show(ui, |ui| {
                            ui.label("Add layer files...");
                        });
                    });
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(state) = &mut self.state {
                let response = ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::drag());
                let viewport = response.rect;

                if state.needs_bbox_update {
                    state.update_bbox_from_layers();
                }

                if state.needs_view_fitting {
                    state.fit_view(viewport);
                }

                if state.needs_view_centering {
                    state.center_view(viewport);
                }

                state
                    .ui_state
                    .update(ui, &viewport, &response, &mut state.view);

                let bbox_screen_vertices = state
                    .bounding_box_vertices
                    .iter()
                    .map(|position| state.gerber_to_screen_coords(*position))
                    .collect::<Vec<_>>();

                trace!(
                    "view bbox screen vertices: {:?}, offset: {:?}, scale: {:?}, translation: {:?}",
                    bbox_screen_vertices, state.transform.origin, state.view.scale, state.view.translation
                );

                trace!(
                    "view: {:?}, view bbox scale: {}, viewport_center: {}, origin_screen_pos: {}",
                    state.view,
                    INITIAL_GERBER_AREA_PERCENT,
                    state.ui_state.center_screen_pos,
                    state.ui_state.origin_screen_pos
                );

                let painter = ui.painter().with_clip_rect(viewport);
                for (_, layer_view_state, layer, _doc) in state.layers.iter() {
                    if layer_view_state.enabled {
                        let layer_transform = layer_view_state
                            .transform
                            .combine(&state.transform);

                        GerberRenderer::default().paint_layer(
                            &painter,
                            state.view,
                            layer,
                            layer_view_state.color,
                            &self.config,
                            &layer_transform,
                        );
                    }
                }

                // Draw origin crosshair
                draw_crosshair(&painter, state.ui_state.origin_screen_pos, Color32::BLUE);
                draw_crosshair(&painter, state.ui_state.center_screen_pos, Color32::LIGHT_GRAY);

                if self.enable_bounding_box_outline && !bbox_screen_vertices.is_empty() {
                    draw_outline(&painter, bbox_screen_vertices, Color32::RED);
                }
            } else {
                let default_style = || Style {
                    padding: length(8.),
                    gap: length(8.),
                    ..Default::default()
                };

                tui(ui, ui.id().with("no-file-loaded-panel"))
                    .reserve_available_space()
                    .style(Style {
                        justify_content: Some(taffy::JustifyContent::Center),
                        align_items: Some(taffy::AlignItems::Center),
                        flex_direction: taffy::FlexDirection::Column,
                        size: Size {
                            width: percent(1.),
                            height: percent(1.),
                        },
                        ..default_style()
                    })
                    .show(|tui| {
                        tui.style(Style {
                            flex_direction: taffy::FlexDirection::Column,
                            ..default_style()
                        })
                        .add(|tui| {
                            tui.label(
                                RichText::new("MakerPnP")
                                    .size(48.0)
                                    .family(FontFamily::Proportional),
                            );
                            tui.label(
                                RichText::new("Gerber Viewer")
                                    .size(32.0)
                                    .family(FontFamily::Proportional),
                            );
                        });
                    });
            }
        });

        //
        // modals
        //

        if self.is_about_modal_open {
            let modal = Modal::new(Id::new("About")).show(ctx, |ui| {
                use egui::special_emojis::GITHUB;

                ui.set_width(250.0);

                ui.vertical_centered(|ui| {
                    ui.heading("About");
                    ui.separator();
                    ui.label("MakerPnP - Gerber Viewer");
                    ui.label("Written by Dominic Clifton");
                    ui.hyperlink_to(format!("💰 ko-fi"), "https://ko-fi.com/dominicclifton");
                    ui.separator();
                    ui.separator();
                    ui.label("A pure-rust Gerber Viewer.");
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label("Source:");
                        ui.hyperlink_to(format!("{GITHUB} MakerPnP"), "https://github.com/MakerPnP/makerpnp");
                    });
                    ui.separator();
                    ui.label("Acknowledgements:");
                    ui.horizontal(|ui| {
                        ui.label("UI framework: ");
                        ui.hyperlink_to(format!("{GITHUB} egui"), "https://github.com/emilk/egui");
                    });
                    ui.horizontal(|ui| {
                        ui.label("Gerber types: ");
                        ui.hyperlink_to(
                            format!("{GITHUB} gerber-types-rs"),
                            "https://github.com/dbrgn/gerber-types-rs",
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("Gerber parser: ");
                        ui.hyperlink_to(
                            format!("{GITHUB} gerber_parser"),
                            "https://github.com/NemoAndrea/gerber-parser",
                        );
                    });

                    ui.separator();

                    if ui.button("Ok").clicked() {
                        self.is_about_modal_open = false;
                    }
                });
            });

            if modal.should_close() {
                self.is_about_modal_open = false;
            }
        }
    }
}
