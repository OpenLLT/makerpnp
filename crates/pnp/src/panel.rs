use std::fmt::Debug;

use derivative::Derivative;
use math::ratio::ratio_of_f64;
use nalgebra::{Point2, Vector, Vector2};
use num_rational::Ratio;
use serde_with::serde_as;

#[derive(serde::Serialize, serde::Deserialize, Default, Debug, Clone, PartialEq, PartialOrd)]
pub struct Dimensions<T: Default + Debug + Clone + PartialEq + PartialOrd> {
    pub left: T,
    pub right: T,
    pub top: T,
    pub bottom: T,
}

#[derive(serde::Serialize, serde::Deserialize, Default, Debug, Clone, PartialEq)]
pub struct DesignSizing {
    pub origin: Vector2<f64>,
    pub offset: Vector2<f64>,
    pub size: Vector2<f64>,
}

#[derive(serde::Serialize, serde::Deserialize, Default, Debug, Clone, PartialEq)]
pub struct PcbUnitPositioning {
    pub offset: Vector2<f64>,
    /// clockwise positive radians
    pub rotation: f64,
}

/// Note: 'mils' unsupported here, the /storage/ is constrained by the units usable by the gerber spec, which are Inches and Millimeters.
#[derive(
    Derivative,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash
)]
#[derivative(Default)]
pub enum Unit {
    Inches,
    #[derivative(Default)]
    Millimeters,
}

#[derive(serde::Serialize, serde::Deserialize, Derivative, Debug, Clone, PartialEq)]
#[derivative(Default)]
#[serde_as]
pub struct PanelSizing {
    #[derivative(Default(value = "Unit::Millimeters"))]
    pub units: Unit,

    #[derivative(Default(value = "Point2::new(100.0, 100.0)"))]
    pub size: Point2<f64>,

    #[derivative(Default(value = "Dimensions { left: 5.0, right: 5.0, top: 5.0, bottom: 5.0 }"))]
    pub edge_rails: Dimensions<f64>,

    pub fiducials: Vec<FiducialParameters>,
    pub design_sizings: Vec<DesignSizing>,
    pub pcb_unit_positionings: Vec<PcbUnitPositioning>,
}

impl PanelSizing {
    pub fn ensure_design_sizings(&mut self, design_count: usize) {
        self.design_sizings
            .resize_with(design_count, Default::default);
    }

    pub fn ensure_unit_positionings(&mut self, unit_count: u16) {
        self.pcb_unit_positionings
            .resize_with(unit_count as usize, Default::default);
    }
}

#[derive(
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Derivative,
    Copy,
    Clone,
    PartialEq,
    PartialOrd
)]
#[derivative(Default)]
pub struct FiducialParameters {
    pub position: Vector2<f64>,
    #[derivative(Default(value = "2.0"))]
    pub mask_diameter: f64,
    #[derivative(Default(value = "1.0"))]
    pub copper_diameter: f64,
}

impl FiducialParameters {
    pub fn copper_to_mask_ratio(&self) -> Option<Ratio<i64>> {
        ratio_of_f64(self.copper_diameter, self.mask_diameter)
    }
}
