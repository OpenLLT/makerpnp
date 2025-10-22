use std::path::PathBuf;

use eda_units::eda_units::dimension_unit::{DimensionUnit, DimensionUnitPoint2Ext, DimensionUnitVector2Ext};
use eda_units::eda_units::unit_system::UnitSystem;
use eframe::emath::{Pos2, Rect, Vec2};
use egui::Color32;
use gerber_viewer::gerber_parser::GerberDoc;
use gerber_viewer::{BoundingBox, GerberLayer, GerberTransform, Invert, Mirroring, ToPos2, UiState, ViewState};
use log::{debug, info, trace};
use nalgebra::{Point2, Vector2};

use crate::{INITIAL_GERBER_AREA_PERCENT, Position, VECTOR_ZERO};

pub struct GerberViewState {
    pub(super) view: ViewState,
    pub(super) needs_view_fitting: bool,
    pub(super) needs_view_centering: bool,
    pub(super) needs_bbox_update: bool,
    bounding_box: BoundingBox,
    pub(super) bounding_box_vertices: Vec<Position>,
    pub(super) layers: Vec<(PathBuf, LayerViewState, GerberLayer, GerberDoc)>,
    pub(super) ui_state: UiState,
    pub(super) transform: GerberTransform,
    pub(super) target_unit_system: UnitSystem,
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
            target_unit_system: UnitSystem::Millimeters,
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
        if self.layers.is_empty() {
            self.target_unit_system = UnitSystem::from_gerber_unit(&gerber_doc.units);

            info!("target_unit_system: {:?}", self.target_unit_system);
        }

        self.layers
            .push((path, layer_view_state, layer, gerber_doc));
        self.update_bbox_from_layers();
        self.request_fit_view();
    }

    pub fn update_bbox_from_layers(&mut self) {
        let mut bbox = BoundingBox::default();

        for (layer_index, (_, layer_view_state, layer, _)) in self
            .layers
            .iter()
            .enumerate()
            .filter(|(_index, (_path, view_state, layer, _))| view_state.enabled && !layer.is_empty())
        {
            let layer_bbox = &layer.bounding_box();

            let mut unit_aligned_layer_transform = layer_view_state.transform;
            unit_aligned_layer_transform.scale *= layer_view_state.unit_system_scale_factor;

            let image_transform_matrix = layer.image_transform().to_matrix();
            let render_transform_matrix = self.transform.to_matrix();
            let layer_matrix = unit_aligned_layer_transform.to_matrix();

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

    pub fn fit_view(&mut self, viewport: Rect) {
        self.update_bbox_from_layers();
        self.view
            .fit_view(viewport, &self.bounding_box, INITIAL_GERBER_AREA_PERCENT);
        self.needs_view_fitting = false;
    }

    pub fn center_view(&mut self, viewport: Rect) {
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

    /// X and Y are in dimension units.
    pub fn move_view(&mut self, position: Vector2<DimensionUnit>) {
        trace!("move view. x: {}, y: {}", position.x, position.y);
        trace!("view translation (before): {:?}", self.view.translation);

        let mut gerber_coords = self.screen_to_gerber_coords(self.view.translation.to_pos2());
        gerber_coords += position.to_vector2(self.target_unit_system);

        trace!("gerber_coords: {:?}", gerber_coords);
        let screen_coords = self.gerber_to_screen_coords(gerber_coords);

        trace!("screen_cords: {:?}", screen_coords);

        let delta = screen_coords - self.view.translation;
        trace!("delta: {:?}", delta);

        self.view.translation -= delta.to_vec2();
        trace!("view translation (after): {:?}", self.view.translation);
    }

    /// X and Y are in dimension units.
    pub fn locate_view(&mut self, point: Point2<DimensionUnit>) {
        trace!("locate view. x: {}, y: {}", point.x, point.y);
        let gerber_coords: Point2<DimensionUnit> = point.in_unit_system(self.target_unit_system);
        trace!("gerber_coords: {:?}", gerber_coords);

        let (x, y) = (gerber_coords.x.value_f64(), gerber_coords.y.value_f64());

        self.view.translation = Vec2::new(
            self.ui_state.center_screen_pos.x - (x as f32 * self.view.scale),
            self.ui_state.center_screen_pos.y + (y as f32 * self.view.scale),
        );
        trace!("view translation (after): {:?}", self.view.translation);
    }
}

pub struct LayerViewState {
    pub enabled: bool,
    pub color: Color32,
    pub transform: GerberTransform,
    /// always 1.0 for the first layer
    pub unit_system_scale_factor: f64,
}

impl LayerViewState {
    pub fn new(color: Color32, unit_system_scale_factor: f64) -> Self {
        Self {
            enabled: true,
            color,
            transform: GerberTransform::default(),
            unit_system_scale_factor,
        }
    }
}
