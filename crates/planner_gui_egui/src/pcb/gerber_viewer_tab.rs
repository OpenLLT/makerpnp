use std::fs::File;
use std::io;
use std::io::BufReader;
use std::path::PathBuf;

use derivative::Derivative;
use eframe::emath::{Rect, Vec2};
use eframe::epaint::Color32;
use egui::{Ui, WidgetText};
use egui_i18n::tr;
use egui_ltreeview::TreeViewState;
use egui_mobius::Value;
use gerber_viewer::gerber_parser::{GerberDoc, ParseError, parse};
use gerber_viewer::position::Vector;
use gerber_viewer::{
    BoundingBox, GerberLayer, GerberRenderer, Mirroring, Transform2D, UiState, ViewState, draw_crosshair,
    generate_pastel_color,
};
use planner_app::{DesignIndex, PcbOverview};
use thiserror::Error;
use tracing::{debug, error, info};

use crate::pcb::tabs::PcbTabContext;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};
use crate::ui_util::NavigationPath;

const INITIAL_GERBER_AREA_PERCENT: f32 = 0.95;

#[derive(Derivative)]
#[derivative(Debug)]
pub struct GerberViewerUi {
    design_index: DesignIndex,

    #[derivative(Debug = "ignore")]
    gerber_state: Value<GerberViewState>,
    gerber_ui_state: Value<UiState>,

    pub component: ComponentState<GerberViewerUiCommand>,
}

impl GerberViewerUi {
    pub fn new(design_index: DesignIndex) -> Self {
        Self {
            design_index,
            gerber_state: Value::default(),
            gerber_ui_state: Value::default(),
            component: Default::default(),
        }
    }

    pub fn update_pcb_overview(&mut self, pcb_overview: PcbOverview) {
        let gerber_items = pcb_overview.gerbers[self.design_index].clone();

        // the list of gerber items may contain fewer, more or different entries and/or the same entries in a different
        // order.  for now, reset and reload everything; it would be more optimal only to reparse files that
        // need reparsing. e.g., by storing the existing gerberdoc and gerberlayer and re-using them instead of
        // regenerating them.
        self.gerber_state
            .set(GerberViewState::default());
        self.gerber_ui_state
            .set(UiState::default());

        for gerber_item in gerber_items {
            let path = gerber_item.path.clone();
            info!("Adding gerber layer: {}", path.to_str().unwrap());
            self.add_gerber_layer_from_file(path)
                .inspect_err(|e| {
                    error!("Error adding gerber layer: {}", e);
                })
                .ok();
        }
    }

    fn add_gerber_layer_from_file(&mut self, path: PathBuf) -> Result<(), GerberViewerUiError> {
        let (gerber_doc, commands) = Self::parse_gerber(&path)?;

        let mut state = self.gerber_state.lock().unwrap();

        let layer_count = state.layers.len();
        let color = generate_pastel_color(layer_count as u64);

        let layer = GerberLayer::new(commands);
        let mut layer_view_state = LayerViewState::new(color);

        layer_view_state.design_offset = state.design_offset;
        layer_view_state.design_origin = state.design_origin;

        state.add_layer(path, layer_view_state, layer, gerber_doc);

        Ok(())
    }

    fn parse_gerber(
        path: &PathBuf,
    ) -> Result<(GerberDoc, Vec<gerber_viewer::gerber_types::Command>), GerberViewerUiError> {
        let file = File::open(path.clone())
            .inspect_err(|error| {
                let message = format!(
                    "Error parsing gerber file: {}, cause: {}",
                    path.to_str().unwrap(),
                    error
                );
                error!("{}", message);
            })
            .map_err(GerberViewerUiError::IoError)?;

        let reader = BufReader::new(file);

        let gerber_doc: GerberDoc =
            parse(reader).map_err(|(_partial_doc, error)| GerberViewerUiError::ParserError(error))?;

        gerber_doc
            .commands
            .iter()
            .for_each(|c| match c {
                Ok(command) => info!("{:?}", command),
                Err(error) => error!("{:?}", error),
            });

        let message = format!("Gerber file parsed successfully. path: {}", path.to_str().unwrap());
        info!("{}", message);

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
}

struct GerberViewState {
    view: ViewState,
    needs_view_centering: bool,
    needs_bbox_update: bool,
    bounding_box: BoundingBox,
    layers: Vec<(PathBuf, LayerViewState, GerberLayer, GerberDoc)>,
    // used for mirroring and rotation, in gerber coordinates
    design_origin: Vector,

    // used for offsetting the design, in gerber coordinates
    design_offset: Vector,

    // global rotation, each layer can be offset from the global rotation
    rotation: f32,
    // global mirroring, each layer can mirrored independently of the global mirroring
    mirroring: Mirroring,
}

impl Default for GerberViewState {
    fn default() -> Self {
        Self {
            view: Default::default(),
            needs_view_centering: true,
            needs_bbox_update: true,
            bounding_box: BoundingBox::default(),
            layers: vec![],
            //design_origin: Vector::new(14.75, 6.0),
            //design_offset: Vector::new(-10.0, -10.0),
            design_origin: Vector::ZERO,
            design_offset: Vector::ZERO,
            rotation: 0.0_f32.to_radians(),
            mirroring: Mirroring::default(),
        }
    }
}

impl GerberViewState {
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
        self.request_center_view();
    }

    fn update_bbox_from_layers(&mut self) {
        let mut bbox = BoundingBox::default();

        for (layer_index, (_, layer_view_state, layer, _)) in self.layers.iter().enumerate() {
            let layer_bbox = &layer.bounding_box();

            let origin = self.design_origin - self.design_offset;

            let transform = Transform2D {
                rotation_radians: self.rotation + layer_view_state.rotation,
                mirroring: self.mirroring ^ layer_view_state.mirroring,
                origin,
                offset: self.design_offset,
            };

            let layer_bbox = layer_bbox.apply_transform(transform);

            debug!("layer bbox: {:?}", layer_bbox);
            bbox.min.x = f64::min(bbox.min.x, layer_bbox.min.x);
            bbox.min.y = f64::min(bbox.min.y, layer_bbox.min.y);
            bbox.max.x = f64::max(bbox.max.x, layer_bbox.max.x);
            bbox.max.y = f64::max(bbox.max.y, layer_bbox.max.y);
            debug!("view bbox after layer. layer: {}, bbox: {:?}", layer_index, bbox);
        }

        self.bounding_box = bbox;
        self.needs_bbox_update = false;
    }

    pub fn request_center_view(&mut self) {
        self.needs_view_centering = true;
    }

    fn reset_view(&mut self, viewport: Rect) {
        self.update_bbox_from_layers();

        let bbox = &self.bounding_box;

        let content_width = bbox.max.x - bbox.min.x;
        let content_height = bbox.max.y - bbox.min.y;

        // Calculate scale to fit the content
        let scale = f32::min(
            viewport.width() / (content_width as f32),
            viewport.height() / (content_height as f32),
        ) * INITIAL_GERBER_AREA_PERCENT;

        // Calculate the content center in mm
        let content_center_x = (bbox.min.x + bbox.max.x) / 2.0;
        let content_center_y = (bbox.min.y + bbox.max.y) / 2.0;

        // Offset from viewport center to place content center
        self.view.translation = Vec2::new(
            viewport.center().x - (content_center_x as f32 * scale),
            viewport.center().y + (content_center_y as f32 * scale), // Note the + here since we flip Y
        );

        self.view.scale = scale;
        self.needs_view_centering = false;
    }
}

#[derive(Error, Debug)]
enum GerberViewerUiError {
    #[error("IO Error. cause: {0:?}")]
    IoError(io::Error),

    #[error("Parser error. cause: {0:?}")]
    ParserError(ParseError),
}

struct LayerViewState {
    color: Color32,
    // in radians, positive = clockwise
    rotation: f32,
    mirroring: Mirroring,
    // the center for rotation/mirroring in gerber units
    design_origin: Vector,
    // in gerber units
    design_offset: Vector,
}

impl LayerViewState {
    fn new(color: Color32) -> Self {
        Self {
            color,
            mirroring: Mirroring::default(),
            rotation: 0.0_f32.to_radians(),
            design_origin: Vector::ZERO,
            design_offset: Vector::ZERO,
        }
    }
}

#[derive(Debug, Clone)]
pub enum GerberViewerUiCommand {
    None,
}

#[derive(Debug, Clone)]
pub enum GerberViewerUiAction {
    None,
}

#[derive(Debug, Clone, Default)]
pub struct GerberViewerUiContext {}

impl UiComponent for GerberViewerUi {
    type UiContext<'context> = GerberViewerUiContext;
    type UiCommand = GerberViewerUiCommand;
    type UiAction = GerberViewerUiAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        let mut state = self.gerber_state.lock().unwrap();

        let response = ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::drag());
        let viewport = response.rect;

        if state.needs_bbox_update {
            state.update_bbox_from_layers();
        }

        if state.needs_view_centering {
            state.reset_view(viewport);
        }

        let mut ui_state = self.gerber_ui_state.lock().unwrap();
        ui_state.update(ui, &viewport, &response, &mut state.view);

        let painter = ui.painter().with_clip_rect(viewport);
        for (_, layer_state, layer, _doc) in state.layers.iter() {
            GerberRenderer::default().paint_layer(
                &painter,
                state.view,
                layer,
                layer_state.color,
                false,
                false,
                state.rotation + layer_state.rotation,
                state.mirroring ^ layer_state.mirroring,
                layer_state.design_origin,
                layer_state.design_offset,
            );
        }

        // Draw origin crosshair
        draw_crosshair(&painter, ui_state.origin_screen_pos, Color32::BLUE);
        draw_crosshair(&painter, ui_state.center_screen_pos, Color32::LIGHT_GRAY);
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            GerberViewerUiCommand::None => None,
        }
    }
}

#[derive(Default, Debug, serde::Deserialize, serde::Serialize)]
pub struct GerberViewerTab {
    pub(crate) navigation_path: NavigationPath,
}

impl GerberViewerTab {
    pub fn new(navigation_path: NavigationPath) -> Self {
        Self {
            navigation_path,
        }
    }
}

impl Tab for GerberViewerTab {
    type Context = PcbTabContext;

    fn label(&self) -> WidgetText {
        // TODO improve the tab title
        let title = format!("{}", self.navigation_path);

        egui::widget_text::WidgetText::from(title)
    }

    fn ui<'a>(&mut self, ui: &mut Ui, _tab_key: &TabKey, context: &mut Self::Context) {
        let state = context.state.lock().unwrap();
        let instance = state
            .gerber_viewer_ui
            .get(&self.navigation_path)
            .unwrap();

        UiComponent::ui(instance, ui, &mut GerberViewerUiContext::default());
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, _context: &mut Self::Context) -> bool {
        true
    }
}
