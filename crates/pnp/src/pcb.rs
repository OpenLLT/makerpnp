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
