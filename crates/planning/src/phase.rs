use std::fmt::{Display, Formatter};

use indexmap::{IndexMap, IndexSet};
use pnp::pcb::PcbSide;
use pnp::reference::Reference;
use thiserror::Error;

use crate::placement::PlacementSortingItem;
#[cfg(test)]
use crate::process::TestTaskState;
use crate::process::{
    AutomatedSolderingTaskState, LoadPcbsTaskState, ManualSolderingTaskState, OperationReference, OperationState,
    OperationStatus, PlacementTaskState, ProcessDefinition, ProcessReference, SerializableTaskState, TaskReference,
};

pub type PhaseReference = Reference;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Phase {
    pub reference: PhaseReference,
    pub process: ProcessReference,

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

    #[error("Invalid operation for phase. phase: '{}', operation: '{}', possible_operations: {:?}", .0, .1, .2.iter().map(|reference|reference.to_string()).collect::<Vec<_>>())]
    InvalidOperationForPhase(Reference, OperationReference, Vec<OperationReference>),
    #[error("Invalid task for operation. phase: '{}', operation: '{}', task: '{}', possible_tasks: {:?}", .0, .1, .2, .3.iter().map(|reference|reference.to_string()).collect::<Vec<_>>())]
    InvalidTaskForOperation(Reference, OperationReference, TaskReference, Vec<TaskReference>),
    // #[error("Preceding operation for phase incomplete. phase: '{0:}', preceding_operation: {1:?}")]
    // PrecedingOperationIncomplete(Reference, OperationReference),
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
    pub operation_states: Vec<OperationState>,
}

impl PhaseState {
    // Safety: all process must be valid
    pub fn from_process(process: &ProcessDefinition) -> Self {
        let operation_states = process
            .operations
            .iter()
            .map(|process_operation| {
                let task_states = process_operation
                    .tasks
                    .iter()
                    .map(|task_reference| {
                        let task_state = make_task_state(task_reference);

                        (task_reference.clone(), task_state)
                    })
                    .collect::<IndexMap<_, _>>();

                OperationState {
                    reference: process_operation.reference.clone(),
                    task_states,
                }
            })
            .collect::<Vec<_>>();

        Self {
            operation_states,
        }
    }

    pub fn reset(&mut self) {
        for state in self.operation_states.iter_mut() {
            for (_task_reference, task_state) in state.task_states.iter_mut() {
                task_state.reset()
            }
        }
    }

    pub fn is_pending(&self) -> bool {
        self.operation_states
            .iter()
            .all(|os| os.status() == OperationStatus::Pending)
    }

    pub fn is_complete(&self) -> bool {
        self.operation_states
            .iter()
            .all(|os| os.status() == OperationStatus::Complete)
    }
}

pub(crate) fn make_task_state(task_reference: &TaskReference) -> Box<dyn SerializableTaskState> {
    let task_state = if task_reference.eq(&TaskReference::from_raw_str("core::load_pcbs")) {
        Box::new(LoadPcbsTaskState::default()) as Box<dyn SerializableTaskState>
    } else if task_reference.eq(&TaskReference::from_raw_str("core::place_components")) {
        Box::new(PlacementTaskState::default()) as Box<dyn SerializableTaskState>
    } else if task_reference.eq(&TaskReference::from_raw_str("core::automated_soldering")) {
        Box::new(AutomatedSolderingTaskState::default()) as Box<dyn SerializableTaskState>
    } else if task_reference.eq(&TaskReference::from_raw_str("core::manual_soldering")) {
        Box::new(ManualSolderingTaskState::default()) as Box<dyn SerializableTaskState>
    } else {
        #[cfg(test)]
        if task_reference.eq(&TaskReference::from_raw_str("core::test_task")) {
            return Box::new(TestTaskState::default()) as Box<dyn SerializableTaskState>;
        }

        panic!("unknown task reference {:?}", task_reference);
    };
    task_state
}
