use std::fmt::Debug;

use derivative::Derivative;
use math::ratio::ratio_of_f64;
use nalgebra::{Point2, Vector2};
use num_rational::Ratio;
use rust_decimal::Decimal;
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
    /// x,y sizing of the design
    pub size: Vector2<f64>,

    /// In EDA tools like DipTrace, an offset can be specified when exporting gerbers, e.g. (10,5).
    /// Use negative offsets here to relocate the gerber back to (0,0), e.g. (-10, -5)
    pub gerber_offset: Vector2<f64>,

    /// In EDA tools like DipTrace, an offset can be specified when exporting placements, e.g. (10,5).
    /// Use negative offsets here to relocate the placements back to (0,0), e.g. (-10, -5)
    pub placement_offset: Vector2<f64>,

    /// For mirroring and rotation
    /// (aka Center Offset)
    ///
    /// Usually this value should be set to the center of the PCB outline's bounding box.
    ///
    /// Must not include any export offsets.
    ///
    /// Examples:
    /// * 10x10mm pcb with coordinates (0,0) - (10,10), origin = (5,5)
    /// * 10x10mm pcb with coordinates (-5,-5) - (5,5), origin = (0,0)
    pub origin: Vector2<f64>,
}

#[derive(serde::Serialize, serde::Deserialize, Default, Debug, Clone, PartialEq)]
pub struct PcbUnitPositioning {
    pub offset: Vector2<f64>,
    /// anti-clockwise positive degrees
    pub rotation: Decimal,
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

#[derive(serde::Serialize, serde::Deserialize, Derivative, Default, Debug, Clone, PartialEq)]
#[serde_as]
pub struct PanelSizing {
    pub units: Unit,

    pub size: Vector2<f64>,

    /// Any value of 0 here means no edge rail.
    /// `Option` is intentionally /not/ used here to simplify offset calculations.
    pub edge_rails: Dimensions<f64>,

    pub fiducials: Vec<FiducialParameters>,

    /// There should be one entry for each design in the PCB.
    ///
    /// See `ensure_design_sizings`
    pub design_sizings: Vec<DesignSizing>,

    /// There should be one entry for each PCB unit.
    ///
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
    Default,
    Copy,
    Clone,
    PartialEq,
    PartialOrd
)]
pub struct FiducialParameters {
    pub position: Point2<f64>,
    pub mask_diameter: f64,
    pub copper_diameter: f64,
}

impl FiducialParameters {
    pub fn copper_to_mask_ratio(&self) -> Option<Ratio<i64>> {
        ratio_of_f64(self.copper_diameter, self.mask_diameter)
    }
}
