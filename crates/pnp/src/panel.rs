use std::fmt::Debug;

use derivative::Derivative;
use math::ratio::ratio_of_f64;
use nalgebra::{Point2, Vector2};
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
    /// For mirroring and rotation
    /// (aka Center Offset)
    ///
    /// Usually this value should be set to the center of the PCB outline's bounding box.
    pub origin: Vector2<f64>,

    /// In EDA tools like DipTrace, a gerber offset can be specified when exporting gerbers, e.g. (10,5).
    /// Use negative offsets here to relocate the gerber back to (0,0), e.g. (-10, -5)
    pub offset: Vector2<f64>,

    /// x,y sizing of the design
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

    #[derivative(Default(value = "Vector2::new(100.0, 100.0)"))]
    pub size: Vector2<f64>,

    /// Any value of 0 here means no edge rail.
    /// `Option` is intentionally /not/ used here to simplify offset calculations.
    #[derivative(Default(value = "Dimensions { left: 5.0, right: 5.0, top: 5.0, bottom: 5.0 }"))]
    pub edge_rails: Dimensions<f64>,

    pub fiducials: Vec<FiducialParameters>,

    ///
    /// See `ensure_design_sizings`
    pub design_sizings: Vec<DesignSizing>,

    /// See `ensure_unit_positionings`
    pub pcb_unit_positionings: Vec<PcbUnitPositioning>,
}

impl PanelSizing {
    /// Call this if the PCB's design count changes
    pub fn ensure_design_sizings(&mut self, design_count: usize) {
        self.design_sizings
            .resize_with(design_count, Default::default);
    }

    /// Call this if the PCB's unit count changes
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
    pub position: Point2<f64>,
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
