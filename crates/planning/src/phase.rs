use std::fmt::{Display, Formatter};

use indexmap::{IndexMap, IndexSet};
use pnp::pcb::PcbSide;
use thiserror::Error;

use crate::placement::PlacementSortingItem;
use crate::process::{Process, ProcessReference, ProcessOperationReference, ProcessOperationState, PlacementOperationState, SerializableOperationTaskState, OperationTaskReference, LoadPcbsOperationState, AutomatedSolderingOperationState, ManualSolderingOperationState};
use crate::reference::Reference;

// TODO
//pub type PhaseReference = Reference;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Phase {
    // TODO
    //pub reference: PhaseReference,
    pub reference: Reference,
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

    #[error("Invalid operation for phase. phase: '{0:}', operation: {1:?}")]
    InvalidOperationForPhase(Reference, ProcessOperationReference),
    #[error("Preceding operation for phase incomplete. phase: '{0:}', preceding_operation: {1:?}")]
    PrecedingOperationIncomplete(Reference, ProcessOperationReference),
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
    pub operation_states: Vec<ProcessOperationState>,
}

impl PhaseState {
    
    // Safety: all process must be valid
    pub fn from_process(process: &Process) -> Self {
        let operation_states = process
            .operations
            .iter()
            .map(|process_operation| {

                let task_states = process_operation.tasks.iter().map(|task_reference|{

                    let mut task_state: Option<Box<dyn SerializableOperationTaskState>> = None;
                    if task_reference.eq(&OperationTaskReference::from_raw_str("core::load_pcbs")) {
                        task_state = Some(Box::new(LoadPcbsOperationState::default()) as Box<dyn SerializableOperationTaskState>)
                    } else if task_reference.eq(&OperationTaskReference::from_raw_str("core::place_components")) {
                        task_state = Some(Box::new(PlacementOperationState::default()) as Box<dyn SerializableOperationTaskState>)
                    } else if task_reference.eq(&OperationTaskReference::from_raw_str("core::automated_soldering")) {
                        task_state = Some(Box::new(AutomatedSolderingOperationState::default()) as Box<dyn SerializableOperationTaskState>)
                    } else if task_reference.eq(&OperationTaskReference::from_raw_str("core::manual_soldering")) {
                        task_state = Some(Box::new(ManualSolderingOperationState::default()) as Box<dyn SerializableOperationTaskState>)
                    } else {
                        panic!("unknown task reference {:?}", task_reference);
                    }
                    (task_reference.clone(), task_state.unwrap())
                    
                }).collect::<IndexMap<_, _>>();
                
                ProcessOperationState {
                    reference: process_operation.reference.clone(),
                    task_states
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
}
