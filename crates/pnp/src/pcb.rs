use std::fmt::Debug;
use std::str::FromStr;

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
