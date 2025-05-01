use std::fs::File;
use std::io;
use std::io::BufReader;
use std::path::PathBuf;
use eframe::emath::Vec2;
use eframe::{egui, run_native, CreationContext, NativeOptions};
use egui::style::ScrollStyle;
use egui::{Color32, Context, Frame, Id, Modal, Painter, Pos2, Rect, Response, RichText, Ui};
use egui_extras::{Column, TableBuilder};
use egui_taffy::taffy::Dimension::Length;
use egui_taffy::taffy::prelude::{auto, length, percent};
use egui_taffy::taffy::{Size, Style};
use egui_taffy::{taffy, tui, TuiBuilderLogic};
use epaint::{FontFamily, Stroke};
use gerber_parser::gerber_doc::GerberDoc;
use gerber_parser::gerber_types;
use gerber_parser::gerber_types::Command;
use gerber_parser::parser::parse_gerber;
use log::{error, info, trace};
use rfd::FileDialog;
use thiserror::Error;
use gerber::color;
use gerber::geometry::BoundingBox;
use logging::AppLogItem;
use crate::gerber::Position;
use gerber::layer::{GerberLayer, ViewState};

mod gerber;
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
    use_polygon_numbering: bool,
    
    is_about_modal_open: bool,
}

impl GerberViewer {
    fn show_about_modal(&mut self) {
        if self.is_about_modal_open {
            return
        }

        self.is_about_modal_open = true;
    }
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
    layers: Vec<(LayerViewState, GerberLayer, GerberDoc)>,
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
    pub fn add_layer(&mut self, layer_view_state: LayerViewState, layer: GerberLayer, gerber_doc: GerberDoc) {
        self.layers
            .push((layer_view_state, layer, gerber_doc));
        self.update_bbox_from_layers();
        self.request_reset();
    }

    fn update_bbox_from_layers(&mut self) {
        let mut bbox = BoundingBox::default();

        for (_, layer, _) in self
            .layers
            .iter()
            .filter(|(state, _, _)| state.enabled)
        {
            let layer_bbox = &layer.bounding_box();
            bbox.min_x = f64::min(bbox.min_x, layer_bbox.min_x);
            bbox.min_y = f64::min(bbox.min_y, layer_bbox.min_y);
            bbox.max_x = f64::max(bbox.max_x, layer_bbox.max_x);
            bbox.max_y = f64::max(bbox.max_y, layer_bbox.max_y);
        }
        trace!("view bbox: {:?}", bbox);

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
        trace!("move view. x: {}, y: {}", position.x, position.y);
        trace!("view translation (before): {:?}", self.view.translation);

        let mut gerber_coords = self.screen_to_gerber_coords(self.view.translation);
        gerber_coords += position;
        trace!("gerber_coords: {:?}", self.view.translation);
        let screen_coords = self.gerber_to_screen_coords(gerber_coords);

        trace!("screen_cords: {:?}", screen_coords);

        let delta = screen_coords - self.view.translation;
        trace!("delta: {:?}", delta);

        self.view.translation -= delta;
        trace!("view translation (after): {:?}", self.view.translation);
    }

    /// X and Y are in GERBER units.
    pub fn locate_view(&mut self, x: f64, y: f64) {
        trace!("locate view. x: {}, y: {}", x, y);
        self.view.translation = Vec2::new(
            self.center_screen_pos.unwrap().x - (x as f32 * self.view.scale),
            self.center_screen_pos.unwrap().y + (y as f32 * self.view.scale),
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
        let (gerber_doc, commands) = Self::parse_gerber(&mut self.log, &path)?;

        let state = self.state.get_or_insert_default();

        let layer_count = state.layers.len();
        let color = color::generate_pastel_color(layer_count as u64);
        
        let layer = GerberLayer::new(commands, path.clone());
        let layer_view_state = LayerViewState::new(color);
        
        state.add_layer(layer_view_state, layer, gerber_doc);

        Ok(())
    }

    fn parse_gerber(log: &mut Vec<AppLogItem>, path: &PathBuf) -> Result<(GerberDoc, Vec<Command>), AppError> {
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

        let commands = gerber_doc
            .commands
            .iter().filter_map(|c| {
            match c {
                Ok(command) => Some(command.clone()),
                Err(_) => None
            }
        })
            .collect();

        Ok((gerber_doc, commands))
    }

    pub fn reload_all_layer_files(&mut self) {
        let Some(state) = &mut self.state else { return };

        for (_state, layer, doc) in state.layers.iter_mut() {
            let path = layer.path().clone();

            if let Ok((gerber_doc, commands)) = Self::parse_gerber(&mut self.log, &path) {
                *layer = GerberLayer::new(commands, path.clone());
                *doc = gerber_doc;
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

impl GerberViewer {
    pub fn new(_cc: &CreationContext) -> Self {
        _cc.egui_ctx
            .style_mut(|style| style.spacing.scroll = ScrollStyle::solid());
        Self {
            state: None,
            log: Vec::new(),
            coord_input: ("0.0".to_string(), "0.0".to_string()),
            use_unique_shape_colors: false,
            use_polygon_numbering: false,
            
            is_about_modal_open: false,
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
                    if ui.button("🗁 Add layers...").clicked() {
                        ui.close_menu();
                        self.add_layer_files();
                    }
                    ui.add_enabled_ui(self.state.is_some(), |ui| {
                        if ui.button("🔃 Reload all layers").clicked() {
                            ui.close_menu();
                            self.reload_all_layer_files();
                        }
                        if ui.button("Close all").clicked() {
                            ui.close_menu();
                            self.close_all();
                        }
                    });
                    if ui.button("Quit").clicked() {
                        ui.close_menu();
                        self.handle_quit(ui.ctx());
                    }
                });
                ui.menu_button("View", |ui| {
                    ui.checkbox(&mut self.use_unique_shape_colors, "🎉 Unique shape colors");
                    ui.checkbox(&mut self.use_polygon_numbering, "＃ Polygon numbering");
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("About").clicked() {
                        ui.close_menu();
                        self.show_about_modal();
                    }
                })
            });

            ui.horizontal(|ui| {
                if ui.button("🗁").clicked() {
                    self.add_layer_files();
                }
                if ui.button("🔃").clicked() {
                    self.reload_all_layer_files();
                }

                ui.separator();
                
                ui.toggle_value(&mut self.use_unique_shape_colors, "🎉");
                ui.toggle_value(&mut self.use_polygon_numbering, "＃");
                
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
                                .move_view((*x, *y).into());
                        }
                    });

                    ui.separator();

                    if ui.button("Reset").clicked() {
                        self.state
                            .as_mut()
                            .unwrap()
                            .request_reset();
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
                                        let unit_text = match state
                                            .layers
                                            .first()
                                            .unwrap()
                                            .2
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
                    for (layer_view_state, layer, _doc) in state.layers.iter_mut() {
                        ui.horizontal(|ui| {
                            ui.color_edit_button_srgba(&mut layer_view_state.color);
                            ui.checkbox(
                                &mut layer_view_state.enabled,
                                layer
                                    .path()
                                    .file_stem()
                                    .unwrap()
                                    .to_string_lossy()
                                    .to_string(),
                            );
                        });
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

                if state.needs_initial_view {
                    state.reset_view(viewport);
                }

                state.center_screen_pos = Some(viewport.center().to_vec2());
                state.origin_screen_pos = Some(state.gerber_to_screen_coords(gerber::position::ZERO));

                state.update_cursor_position(&response, ui);
                state.handle_panning(&response, ui);
                state.handle_zooming(&response, viewport, ui);

                trace!("view: {:?}, view bbox scale: {}, viewport_center: {}, origin_screen_pos: {}", state.view, INITIAL_GERBER_AREA_PERCENT, state.center_screen_pos.unwrap(), state.origin_screen_pos.unwrap());

                let painter = ui.painter().with_clip_rect(viewport);
                for (layer_state, layer, _doc) in state.layers.iter() {
                    if layer_state.enabled {
                        layer.paint_gerber(&painter, state.view, layer_state.color, self.use_unique_shape_colors, self.use_polygon_numbering);
                    }
                }

                // Draw origin crosshair
                if let Some(position) = state.origin_screen_pos {
                    Self::draw_crosshair(&painter, position, Color32::BLUE);
                }
                if let Some(position) = state.center_screen_pos {
                    Self::draw_crosshair(&painter, position, Color32::LIGHT_GRAY);
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
                                        .family(FontFamily::Proportional)
                                );
                                tui.label(
                                    RichText::new("Gerber Viewer")
                                        .size(32.0)
                                        .family(FontFamily::Proportional)
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
                        ui.hyperlink_to(format!("{GITHUB} gerber-types-rs"), "https://github.com/dbrgn/gerber-types-rs");
                    });
                    ui.horizontal(|ui| {
                        ui.label("Gerber parser: ");
                        ui.hyperlink_to(format!("{GITHUB} gerber_parser"), "https://github.com/NemoAndrea/gerber-parser");
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

