//! When there are many PCB, you have PCB instances.
//! When there are many units on a given PCB, you have PCB units.
//! Thus, you can refer to a specific PCB unit by its instance and unit.
//! To complicate things, humans prefer 1-based numbers ("the first", "the second",..), and computers prefer 0-based numbers (arrays, vectors, etc.)
//! We define types and use them, so that it's clear where indexes (0-based) vs numbers (1-based) are used.

use std::fmt::Debug;
use std::str::FromStr;

/// 0-based
pub type PcbInstanceIndex = u16;
/// 1-based
pub type PcbInstanceNumber = u16;

/// 0-based
pub type PcbUnitIndex = u16;
/// 1-based
pub type PcbUnitNumber = u16;

#[derive(
    Debug,
    serde::Serialize,
    serde::Deserialize,
    Copy,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash
)]
#[serde(rename_all = "lowercase")]
pub enum PcbSide {
    Top,
    Bottom,
}

impl FromStr for PcbSide {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "top" => Ok(PcbSide::Top),
            "bottom" => Ok(PcbSide::Bottom),
            _ => Err(()),
        }
    }
}
