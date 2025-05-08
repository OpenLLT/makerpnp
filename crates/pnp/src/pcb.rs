use std::fmt::Display;

/// 0-based
pub type PcbUnitIndex = u16;
/// 1-based
pub type PcbUnitNumber = u16;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum PcbSide {
    Top,
    Bottom,
}

#[deprecated(note = "Will be removed.")]
#[derive(
    Debug,
    serde::Serialize,
    serde::Deserialize,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash
)]
#[serde(rename_all = "lowercase")]
pub enum PcbKind {
    Single,
    Panel,
}

impl TryFrom<&String> for PcbKind {
    type Error = ();

    fn try_from(value: &String) -> Result<Self, Self::Error> {
        match value.to_lowercase().as_str() {
            "single" => Ok(PcbKind::Single),
            "panel" => Ok(PcbKind::Panel),
            _ => Err(()),
        }
    }
}

impl Display for PcbKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PcbKind::Single => f.write_str("single"),
            PcbKind::Panel => f.write_str("panel"),
        }
    }
}
