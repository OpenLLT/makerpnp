use std::fs::File;
use std::hash::Hash;
use std::io;
use std::io::BufReader;
use std::path::PathBuf;

use derivative::Derivative;
use eframe::emath::{Rect, Vec2};
use eframe::epaint::Color32;
use egui::{Ui, WidgetText};
use egui_mobius::Value;
use gerber_viewer::gerber_parser::{GerberDoc, ParseError, parse};
use gerber_viewer::position::Vector;
use gerber_viewer::{
    BoundingBox, GerberLayer, GerberRenderer, Mirroring, Transform2D, UiState, ViewState, draw_crosshair,
    generate_pastel_color,
};
use indexmap::IndexMap;
use indexmap::map::Entry;
use planner_app::{DesignIndex, PcbOverview};
use thiserror::Error;
use tracing::{debug, error, info};

use crate::pcb::tabs::PcbTabContext;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};

const INITIAL_GERBER_AREA_PERCENT: f32 = 0.95;

#[derive(Derivative)]
#[derivative(Debug)]
pub struct GerberViewerUi {
    args: GerberViewerUiInstanceArgs,

    #[derivative(Debug = "ignore")]
    gerber_state: Value<GerberViewState>,
    gerber_ui_state: Value<UiState>,

    pub component: ComponentState<GerberViewerUiCommand>,
}

impl GerberViewerUi {
    pub fn new(args: GerberViewerUiInstanceArgs) -> Self {
        Self {
            args,
            gerber_state: Value::default(),
            gerber_ui_state: Value::default(),
            component: Default::default(),
        }
    }

    pub fn update_pcb_overview(&mut self, pcb_overview: PcbOverview) {
        let gerber_items = match self.args.mode {
            GerberViewerMode::Panel => pcb_overview.pcb_gerbers.clone(),
            GerberViewerMode::Design(design_index) => pcb_overview.design_gerbers[design_index].clone(),
        };

        // The new list of gerber items may contain fewer, more or different entries and/or the same entries in a different
        // order.  Only to reparse files that need reparsing, use the layer order defined by the gerber items collection.

        let mut gerber_state = self.gerber_state.lock().unwrap();

        // FUTURE move this, e.g. `GerberLayerState::sync_layers` and give it the `FBuild` and `FKey` closures as arguments.
        {
            let design_origin = gerber_state.design_origin;
            let design_offset = gerber_state.design_offset;

            let mut layers = gerber_state.layers.split_off(0);

            let errors = sync_indexmap(
                &mut layers,
                &gerber_items,
                |index, key, _content| Self::build_gerber_layer_from_file(index, key, design_origin, design_offset),
                |content| content.path.clone(),
                |_existing_entry, _gerber_item| true,
            );

            for (index, error) in errors {
                error!(
                    "Error adding gerber layer. path: {:?}, error: {}",
                    gerber_items[index].path, error
                );
            }

            gerber_state.layers = layers;
            gerber_state.request_center_view();
        }

        /// Synchronizes an index map with a source-of-truth vector using a fallible builder.
        /// Removes old items if not present in the vector.
        /// index map entry ordering will match the ordering of items in the vector.
        pub fn sync_indexmap<K, T, S, E, FBuild, FKey, FReuse>(
            map: &mut IndexMap<K, T>,
            source: &[S],
            build_fn: FBuild,
            extract_key: FKey,
            reuse_check: FReuse,
        ) -> Vec<(usize, E)>
        where
            K: Eq + Hash + Clone,
            // build the new entry
            FBuild: Fn(usize, &K, &S) -> Result<T, E>,
            // return the key to use for the new entry
            FKey: Fn(&S) -> K,
            // return true to keep the existing entry, false to replace it with the new one
            FReuse: Fn(&T, &S) -> bool,
        {
            let mut new_map: IndexMap<K, T> = IndexMap::with_capacity(source.len());
            let mut errors = Vec::new();

            for (i, item) in source.iter().enumerate() {
                let key = extract_key(item);
                match map.entry(key.clone()) {
                    Entry::Occupied(entry) if reuse_check(entry.get(), &item) => {
                        let existing_entry = entry.shift_remove();
                        new_map.insert(key, existing_entry);
                    }
                    _ => match build_fn(new_map.len(), &key, item) {
                        Ok(new_val) => {
                            new_map.insert(key, new_val);
                        }
                        Err(err) => {
                            errors.push((i, err));
                        }
                    },
                }
            }

            *map = new_map;

            errors
        }
    }

    fn build_gerber_layer_from_file(
        index: usize,
        path: &PathBuf,
        design_origin: Vector,
        design_offset: Vector,
    ) -> Result<(LayerViewState, GerberLayer, GerberDoc), GerberViewerUiError> {
        let (gerber_doc, commands) = Self::parse_gerber(path)?;

        let color = generate_pastel_color(index as u64);

        let layer = GerberLayer::new(commands);
        let mut layer_view_state = LayerViewState::new(color);

        layer_view_state.design_offset = design_offset;
        layer_view_state.design_origin = design_origin;

        Ok((layer_view_state, layer, gerber_doc))
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
    layers: IndexMap<PathBuf, (LayerViewState, GerberLayer, GerberDoc)>,
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
            layers: Default::default(),
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
            .insert(path, (layer_view_state, layer, gerber_doc));
        self.update_bbox_from_layers();
        self.request_center_view();
    }

    fn update_bbox_from_layers(&mut self) {
        let mut bbox = BoundingBox::default();

        for (layer_index, (_path, (layer_view_state, layer, _))) in self
            .layers
            .iter()
            .enumerate()
            .filter(|(_, (_path, (_, layer, _)))| !layer.is_empty())
        {
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
        for (_path, (layer_state, layer, _doc)) in state.layers.iter() {
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
            GerberViewerUiCommand::None => Some(GerberViewerUiAction::None),
        }
    }
}

/// The arguments required to create a new instance or reference an existing instance of the UI component.
/// Value object
#[derive(
    Default,
    Debug,
    serde::Deserialize,
    serde::Serialize,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash
)]
pub struct GerberViewerUiInstanceArgs {
    pub mode: GerberViewerMode,
    // TODO add other args, like pcb_side: Option<PcbSide> or tags: Option<Vec<Tag>>
}

#[derive(
    Derivative,
    Debug,
    serde::Deserialize,
    serde::Serialize,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash
)]
#[derivative(Default)]
pub enum GerberViewerMode {
    #[derivative(Default)]
    Panel,
    Design(DesignIndex),
}

#[derive(Default, Debug, serde::Deserialize, serde::Serialize)]
pub struct GerberViewerTab {
    pub(crate) args: GerberViewerUiInstanceArgs,
}

impl GerberViewerTab {
    pub fn new(args: GerberViewerUiInstanceArgs) -> Self {
        Self {
            args,
        }
    }
}

impl Tab for GerberViewerTab {
    type Context = PcbTabContext;

    fn label(&self) -> WidgetText {
        let title = match self.args.mode {
            GerberViewerMode::Panel => "Panel".to_string(),
            // TODO improve the tab title
            GerberViewerMode::Design(design_index) => format!("Design ({})", design_index),
        };

        egui::widget_text::WidgetText::from(title)
    }

    fn ui<'a>(&mut self, ui: &mut Ui, _tab_key: &TabKey, context: &mut Self::Context) {
        let state = context.state.lock().unwrap();
        let Some(instance) = state.gerber_viewer_ui.get(&self.args) else {
            ui.spinner();
            return;
        };

        UiComponent::ui(instance, ui, &mut GerberViewerUiContext::default());
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, _context: &mut Self::Context) -> bool {
        true
    }
}
