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
use crate::pcb::UnitPlacementPosition;
use crate::phase::PhaseReference;

#[serde_as]
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub struct PlacementState {
    // FUTURE consider removing `unit_path`, it may be redundant since the map that holds the stats has the path
    //        it's also a common source of confusion in the test and leads to additional code and debug output
    #[serde_as(as = "DisplayFromStr")]
    pub unit_path: ObjectPath,
    pub placement: Placement,
    #[serde(default)]
    pub unit_position: UnitPlacementPosition,
    pub operation_status: PlacementStatus,
    /// Status of the placement in the project
    pub project_status: ProjectPlacementStatus,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub phase: Option<PhaseReference>,
}

#[cfg(test)]
impl Default for PlacementState {
    fn default() -> Self {
        Self {
            unit_path: Default::default(),
            placement: Default::default(),
            unit_position: Default::default(),
            operation_status: PlacementStatus::Pending,
            project_status: ProjectPlacementStatus::Used,
            phase: None,
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProjectPlacementStatus {
    Used,
    Unused,
}

impl Default for ProjectPlacementStatus {
    fn default() -> Self {
        Self::Used
    }
}

impl Display for ProjectPlacementStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ProjectPlacementStatus::Used => f.write_str("Used"),
            ProjectPlacementStatus::Unused => f.write_str("Unused"),
        }
    }
}

/// Sorting modes, these are meant to be combined.
///
/// example scenario: sort by: Pcb, then PcbUnitYX, then Feeder Reference, then RefDes.
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
    /// The pcb instance
    Pcb,
    /// Just the pcb unit number without the pcb instance
    PcbUnit,
    /// Up then across (ignores pcb instance and pcb unit)
    PcbUnitXY,
    /// Right then up (ignores pcb instance and pcb unit)
    PcbUnitYX,
    RefDes,
    // FUTURE add other modes, such as COST, PART, AREA, HEIGHT, REFDES, ANGLE, PANEL_XY, PANEL_YX, DESCRIPTION
    //        Note: PANEL_XY and PANEL_YX are for when you have a job with multiple PCBs.
}

impl Display for PlacementSortingMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FeederReference => write!(f, "FeederReference"),
            Self::Pcb => write!(f, "Pcb"),
            Self::PcbUnit => write!(f, "PcbUnit"),
            Self::PcbUnitXY => write!(f, "PcbUnitXY"),
            Self::PcbUnitYX => write!(f, "PcbUnitYX"),
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

impl From<(&PlacementSortingMode, &SortOrder)> for PlacementSortingItem {
    fn from(value: (&PlacementSortingMode, &SortOrder)) -> Self {
        Self {
            mode: value.0.clone(),
            sort_order: value.1.clone(),
        }
    }
}

impl From<(PlacementSortingMode, SortOrder)> for PlacementSortingItem {
    fn from(value: (PlacementSortingMode, SortOrder)) -> Self {
        Self {
            mode: value.0,
            sort_order: value.1,
        }
    }
}

pub fn build_unique_parts_from_design_variant_placement_map(
    design_variant_placement_map: &BTreeMap<DesignVariant, Vec<Placement>>,
) -> Vec<&Part> {
    let mut unique_parts: Vec<&Part> = vec![];
    for placements in design_variant_placement_map.values() {
        for record in placements {
            if !unique_parts.contains(&&record.part) {
                unique_parts.push(&record.part);
            }
        }
    }

    unique_parts
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum PlacementStatus {
    Pending,
    Placed,
    Skipped,
}

impl Default for PlacementStatus {
    fn default() -> Self {
        Self::Pending
    }
}

impl Display for PlacementStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PlacementStatus::Placed => f.write_str("Placed"),
            PlacementStatus::Skipped => f.write_str("Skipped"),
            PlacementStatus::Pending => f.write_str("Pending"),
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum PlacementOperation {
    Place,
    Skip,
    Reset,
}

impl Display for PlacementOperation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PlacementOperation::Place => f.write_str("Place"),
            PlacementOperation::Skip => f.write_str("Skip"),
            PlacementOperation::Reset => f.write_str("Reset"),
        }
    }
}
