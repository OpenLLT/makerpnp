use std::path::PathBuf;

use pnp::pcb::PcbSide;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct GerberFile {
    pub file: PathBuf,

    pub purpose: GerberPurpose,
    pub pcb_side: Option<PcbSide>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Copy, Clone, PartialEq, Eq, Hash)]
pub enum GerberPurpose {
    Other,
    Legend,
    PasteMask,
    SolderMask,
    Assembly,
    Copper,
}
