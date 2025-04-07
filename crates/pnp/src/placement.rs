use std::cmp::Ordering;
use std::fmt::{Debug, Display, Formatter};
use std::ops::{Deref, DerefMut};

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
    /// Range is >-180 to +180.
    pub rotation: Decimal,
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
