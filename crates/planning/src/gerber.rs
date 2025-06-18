use std::path::PathBuf;

use pnp::pcb::PcbSide;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct GerberFile {
    pub file: PathBuf,

    pub purpose: GerberPurpose,
    pub pcb_side: Option<PcbSide>,
}

/// For Pick-and-Place planning, we only care about a subset of the gerber files.
/// See `gerber-types::FileFunction` for the full list.
#[derive(Debug, serde::Serialize, serde::Deserialize, Copy, Clone, PartialEq, Eq, Hash)]
pub enum GerberPurpose {
    Assembly,
    Component,
    Copper,
    Legend,
    Paste,
    Solder,

    Other,
}
