use std::cmp::Ordering;
use std::fmt::{Debug, Display, Formatter};
use std::ops::{Deref, DerefMut};

use eda_units::eda_units::angle::{AngleUnit, Radians};
use eda_units::eda_units::dimension::DimensionPoint2;
use eda_units::eda_units::dimension_unit::DimensionUnitPoint2;
use lexical_sort::natural_lexical_cmp;
use rust_decimal::Decimal;

use crate::part::Part;
use crate::pcb::PcbSide;

/// Uses right-handed cartesian coordinate system
/// See https://en.wikipedia.org/wiki/Cartesian_coordinate_system
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq)]
pub struct Placement {
    pub ref_des: RefDes,
    pub part: Part,
    pub place: bool,
    pub pcb_side: PcbSide,

    /// Positive = Right
    pub x: Decimal,
    /// Positive = Up
    pub y: Decimal,
    /// Positive values indicate anti-clockwise rotation
    /// Range is >-180 to +180 degrees
    pub rotation: Decimal,
    // TODO use PlacementPosition instead of the above, note since placements are often handled together, the owner of the placements
    //      should keep the dimension unit system (inch, mm) and angle unit system (degrees, radians)
    //      we want to avoid needlessly storing the unit system information in the placement itself.
    // pub position: PlacementPosition,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct RefDes(String);

impl Ord for RefDes {
    fn cmp(&self, other: &Self) -> Ordering {
        natural_lexical_cmp(&self.0, &other.0)
    }
}

impl PartialOrd for RefDes {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Display for RefDes {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for RefDes {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl Debug for RefDes {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl From<&str> for RefDes {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl Deref for RefDes {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for RefDes {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Copy, Clone, PartialEq, Eq)]
pub struct PlacementPosition {
    /// X Positive = Right
    /// Y Positive = Up
    pub coords: DimensionPoint2,

    // TODO consider using `Degrees` instead of `Angle` to ensure type safety
    /// Degrees, positive values indicate anti-clockwise rotation
    /// Range is >-180 to +180 degrees
    pub rotation: Radians,
}

impl PlacementPosition {
    pub fn new(coords: DimensionPoint2, rotation: Radians) -> Self {
        Self {
            coords,
            rotation,
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Copy, Clone, PartialEq, Eq)]
pub struct PlacementPositionUnit {
    /// X Positive = Right
    /// Y Positive = Up
    pub coords: DimensionUnitPoint2,
    /// Degrees, positive values indicate anti-clockwise rotation
    pub rotation: AngleUnit,
}

impl PlacementPositionUnit {
    pub fn new(coords: DimensionUnitPoint2, rotation: AngleUnit) -> Self {
        Self {
            coords,
            rotation,
        }
    }
}
