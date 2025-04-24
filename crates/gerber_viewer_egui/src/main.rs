use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io;
use std::io::BufReader;
use std::path::PathBuf;

use eframe::emath::Vec2;
use eframe::{CreationContext, Frame, NativeOptions, egui, run_native};
use egui::ahash::HashMap;
use egui::scroll_area::ScrollBarVisibility;
use egui::style::ScrollStyle;
use egui::{Color32, Context, Painter, Pos2, Rect, Response, Ui};
use epaint::{Shape, Stroke, StrokeKind};
use gerber_parser::gerber_doc::GerberDoc;
use gerber_parser::parser::parse_gerber;
use gerber_types::{Aperture, ApertureDefinition, Command, Coordinates, FunctionCode, Operation, Rectangular};
use log::{error, info};
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

#[derive(Debug)]
enum GerberPrimitive {
    Circle {
        x: f64,
        y: f64,
        diameter: f64,
    },
    Rectangle {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
    Line {
        start: (f64, f64),
        end: (f64, f64),
        width: f64,
    },
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
        });

        let message = "Gerber file parsed successfully";
        info!("{}", message);
        self.log
            .push(AppLogItem::Info(message.to_string()));

        Ok(())
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
                } => {
                    let radius = width / 2.0;
                    for &(x, y) in &[start, end] {
                        bbox.min_x = bbox.min_x.min(x - radius);
                        bbox.min_y = bbox.min_y.min(y - radius);
                        bbox.max_x = bbox.max_x.max(x + radius);
                        bbox.max_y = bbox.max_y.max(y + radius);
                    }
                }
            }
        }

        bbox
    }

    fn build_primitives(doc: &GerberDoc) -> Vec<GerberPrimitive> {
        let mut primitives = Vec::new();
        let mut apertures = HashMap::default();
        let mut current_aperture = None;
        let mut current_pos = (0.0, 0.0);

        for cmd in doc
            .commands
            .iter()
            .filter_map(|result| result.as_ref().ok())
        {
            match cmd {
                Command::ExtendedCode(gerber_types::ExtendedCode::ApertureDefinition(ApertureDefinition {
                    code,
                    aperture,
                })) => {
                    apertures.insert(code, aperture);
                }
                Command::FunctionCode(FunctionCode::DCode(gerber_types::DCode::SelectAperture(code))) => {
                    current_aperture = apertures.get(&code).cloned();
                }
                Command::FunctionCode(FunctionCode::DCode(gerber_types::DCode::Operation(operation))) => {
                    match operation {
                        Operation::Move(coords) => {
                            current_pos = (
                                coords.x.unwrap_or(0_i32.into()).into(),
                                coords.y.unwrap_or(0_i32.into()).into(),
                            );
                        }
                        Operation::Interpolate(coords, ..) => {
                            if let Some(aperture) = &current_aperture {
                                let end: (f64, f64) = (
                                    coords.x.unwrap_or(0_i32.into()).into(),
                                    coords.y.unwrap_or(0_i32.into()).into(),
                                );
                                match aperture {
                                    Aperture::Circle(gerber_types::Circle {
                                        diameter, ..
                                    }) => {
                                        primitives.push(GerberPrimitive::Line {
                                            start: current_pos,
                                            end: (end.0.into(), end.1.into()),
                                            width: *diameter,
                                        });
                                    }
                                    _ => {
                                        // TODO support more Apertures (rectangle, obround, etc)
                                    }
                                }
                                current_pos = end;
                            }
                        }
                        Operation::Flash(coords, ..) => {
                            if let Coordinates {
                                x: Some(x),
                                y: Some(y),
                                ..
                            } = coords
                            {
                                current_pos = ((*x).into(), (*y).into());
                            }
                            if let Some(aperture) = &current_aperture {
                                match aperture {
                                    Aperture::Circle(gerber_types::Circle {
                                        diameter, ..
                                    }) => {
                                        primitives.push(GerberPrimitive::Circle {
                                            x: current_pos.0,
                                            y: current_pos.1,
                                            diameter: *diameter,
                                        });
                                    }
                                    Aperture::Rectangle(Rectangular {
                                        x,
                                        y,
                                        ..
                                    }) => {
                                        primitives.push(GerberPrimitive::Rectangle {
                                            x: current_pos.0 - x / 2.0,
                                            y: current_pos.1 - y / 2.0,
                                            width: *x,
                                            height: *y,
                                        });
                                    }
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        primitives
    }

    fn calculate_initial_view(&mut self, viewport: Rect) {
        let bbox = &self.bounding_box;

        let content_width = bbox.max_x - bbox.min_x;
        let content_height = bbox.max_y - bbox.min_y;

        let scale_x = viewport.width() as f64 / content_width;
        let scale_y = viewport.height() as f64 / content_height;

        let scale = scale_x.min(scale_y) as f32 * INITIAL_GERBER_AREA_PERCENT;

        let center_x = (bbox.min_x + bbox.max_x) / 2.0;
        let center_y = (bbox.min_y + bbox.max_y) / 2.0;

        self.view.translation = Vec2::new(
            (viewport.width() / 2.0) - (center_x as f32 * scale),
            (viewport.height() / 2.0) - (center_y as f32 * scale),
        );
        self.view.scale = scale;
        self.needs_initial_view = false;
    }

    pub fn handle_panning(&mut self, response: &Response, ui: &mut Ui) {
        if response.dragged_by(egui::PointerButton::Primary) {
            let delta = response.drag_delta();
            self.view.translation += delta;
            ui.ctx().clear_animations();
        }
    }

    pub fn handle_zooming(&mut self, response: &Response, viewport: Rect, ui: &mut Ui) {
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

    pub fn paint_gerber(&self, painter: Painter) {
        for primitive in &self.gerber_primitives {
            match primitive {
                GerberPrimitive::Circle {
                    x,
                    y,
                    diameter,
                } => {
                    let center = self.view.translation + Vec2::new(*x as f32, *y as f32) * self.view.scale;
                    let radius = (*diameter as f32 / 2.0) * self.view.scale;
                    painter.circle(center.to_pos2(), radius, self.color, Stroke::NONE);
                }
                GerberPrimitive::Rectangle {
                    x,
                    y,
                    width,
                    height,
                } => {
                    let top_left = self.view.translation + Vec2::new(*x as f32, *y as f32) * self.view.scale;
                    let size = Vec2::new(*width as f32, *height as f32) * self.view.scale;
                    painter.rect(
                        Rect::from_min_size(top_left.to_pos2(), size),
                        0.0,
                        self.color,
                        Stroke::NONE,
                        StrokeKind::Middle, // verify this is correct
                    );
                }
                GerberPrimitive::Line {
                    start,
                    end,
                    width,
                } => {
                    let start_position =
                        self.view.translation + Vec2::new(start.0 as f32, start.1 as f32) * self.view.scale;
                    let end_position = self.view.translation + Vec2::new(end.0 as f32, end.1 as f32) * self.view.scale;
                    painter.line_segment(
                        [start_position.to_pos2(), end_position.to_pos2()],
                        Stroke::new((*width as f32) * self.view.scale, Color32::LIGHT_GREEN),
                    );
                    // Draw circles at either end of the line.
                    let radius = (*width as f32 / 2.0) * self.view.scale;
                    painter.circle(start_position.to_pos2(), radius, self.color, Stroke::NONE);
                    painter.circle(end_position.to_pos2(), radius, self.color, Stroke::NONE);
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
        }
    }
}

impl eframe::App for GerberViewer {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        egui::TopBottomPanel::bottom("bottom_panel")
            .resizable(true)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Clear").clicked() {
                        self.clear_log();
                    }
                });

                egui::ScrollArea::vertical()
                    .scroll_bar_visibility(ScrollBarVisibility::AlwaysVisible)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for error in self.log.iter() {
                            ui.label(format!("{}", error));
                        }
                    })
            });

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            if ui.button("Open Gerber File").clicked() {
                self.open_gerber_file();
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let response = ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::drag());
            let viewport = response.rect;

            if let Some(state) = &mut self.state {
                if state.needs_initial_view {
                    state.calculate_initial_view(viewport);
                }

                state.handle_panning(&response, ui);
                state.handle_zooming(&response, viewport, ui);

                let painter = ui.painter().with_clip_rect(viewport);
                state.paint_gerber(painter);
            }
        });
    }
}
