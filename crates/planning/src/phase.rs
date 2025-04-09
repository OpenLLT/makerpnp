use std::fmt::{Display, Formatter};

use indexmap::IndexSet;
use pnp::pcb::PcbSide;
use thiserror::Error;

use crate::placement::PlacementSortingItem;
use crate::process::{Process, ProcessName, ProcessOperationKind, ProcessOperationState};
use crate::reference::Reference;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Phase {
    pub reference: Reference,
    pub process: ProcessName,

    pub load_out_source: String,

    // TODO consider adding PCB unit + SIDE assignments to the phase instead of just a single side
    pub pcb_side: PcbSide,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub placement_orderings: Vec<PlacementSortingItem>,
}

#[derive(Error, Debug)]
pub enum PhaseError {
    #[error("Unknown phase. phase: '{0:}'")]
    UnknownPhase(Reference),

    #[error("Invalid operation for phase. phase: '{0:}', operation: {1:?}")]
    InvalidOperationForPhase(Reference, ProcessOperationKind),
    #[error("Preceding operation for phase incomplete. phase: '{0:}', preceding_operation: {1:?}")]
    PrecedingOperationIncomplete(Reference, ProcessOperationKind),
}

pub struct PhaseOrderings<'a>(pub &'a IndexSet<Reference>);

impl<'a> Display for PhaseOrderings<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "['{}']",
            self.0
                .iter()
                .map(Reference::to_string)
                .collect::<Vec<String>>()
                .join("', '")
        )
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub struct PhaseState {
    // the order of operations must be preserved.
    pub operation_state: Vec<(ProcessOperationKind, ProcessOperationState)>,
}

impl PhaseState {
    pub fn from_process(process: &Process) -> Self {
        let operation_state = process
            .operations
            .iter()
            .map(|kind| (kind.clone(), ProcessOperationState::default()))
            .collect::<Vec<_>>();

        Self {
            operation_state,
        }
    }
}
