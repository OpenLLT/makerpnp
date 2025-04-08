use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};

use pnp::object_path::ObjectPath;
use pnp::part::Part;
use pnp::placement::Placement;
use serde_with::serde_as;
use serde_with::DisplayFromStr;
use thiserror::Error;
use util::sorting::SortOrder;

use crate::design::DesignVariant;
use crate::reference::Reference;

#[serde_as]
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub struct PlacementState {
    #[serde_as(as = "DisplayFromStr")]
    pub unit_path: ObjectPath,
    pub placement: Placement,
    pub placed: bool,
    pub status: PlacementStatus,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub phase: Option<Reference>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PlacementStatus {
    Known,
    Unknown,
}

impl Display for PlacementStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PlacementStatus::Known => f.write_str("Known"),
            PlacementStatus::Unknown => f.write_str("Unknown"),
        }
    }
}

#[derive(
    Debug,
    Clone,
    serde::Serialize,
    serde::Deserialize,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash
)]
pub enum PlacementSortingMode {
    FeederReference,
    PcbUnit,
    RefDes,
    // FUTURE add other modes, such as COST, PART, AREA, HEIGHT, REFDES, ANGLE, DESIGN_X, DESIGN_Y, PANEL_X, PANEL_Y, DESCRIPTION
}

impl Display for PlacementSortingMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FeederReference => write!(f, "FeederReference"),
            Self::PcbUnit => write!(f, "PcbUnit"),
            Self::RefDes => write!(f, "RefDes"),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct PlacementSortingItem {
    pub mode: PlacementSortingMode,
    pub sort_order: SortOrder,
}

#[derive(Error, Debug)]
pub enum PlacementSortingError {
    #[error("Invalid placement sorting path. value: '{0:}'")]
    Invalid(String),
}

pub fn build_unique_parts(design_variant_placement_map: &BTreeMap<DesignVariant, Vec<Placement>>) -> Vec<Part> {
    let mut unique_parts: Vec<Part> = vec![];
    for placements in design_variant_placement_map.values() {
        for record in placements {
            if !unique_parts.contains(&record.part) {
                unique_parts.push(record.part.clone());
            }
        }
    }

    unique_parts
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq)]
pub enum PlacementOperation {
    Placed,
}
