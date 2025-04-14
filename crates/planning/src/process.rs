//! PCBs are assembled using one or more processes.
//! Each assembly project has one or more phases.
//! Each phase uses a process.
//! Each process has one or more operations.
//! Each operation can be complete, or not.
//! Some operations just have a simple status, others require per-operation actions to be performed before they are
//! complete
//! 
//! Phases can be abandoned or skipped.
//! Operations can be abandoned or skipped.
//! 
//! Later operations and phases cannot be actioned unless preceding phases and actions are completed/skipped/abandoned.
use std::fmt::{Debug};
use dyn_clone::DynClone;
use dyn_eq::DynEq;
use indexmap::IndexMap;
use thiserror::Error;
use util::dynamic::as_any::AsAny;
use crate::reference::Reference;

/// e.g. `manual` or `pnp`
pub type ProcessReference = Reference;

/// The /definition/ of a process.
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq)]
pub struct Process {
    pub reference: ProcessReference,

    /// examples:
    /// for `Manual` = `["load-pcbs", "manually-place-and-solder"]`
    /// for `PnP` = `["load-pcbs", "place-components", "solder"]`
    ///
    pub operations: Vec<ProcessOperation>,
    
    /// examples: `["core::"]`
    pub rules: Vec<ProcessRuleReference>
}

/// A user defined (or pre-configured) process operation reference
/// e.g. "place-components"
pub type ProcessOperationReference = Reference;

/// The /definition/ of a process operation
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq)]
pub struct ProcessOperation {
    /// e.g. "manually-place-and-solder"
    pub reference: ProcessOperationReference,

    /// e.g. `["core::place_components", "core::manual_solder"]`
    /// @see [`ProcessOperationState`]
    pub tasks: Vec<OperationTaskReference>,
}

/// a namespaced operation task reference.  e.g. "core::place_components"
pub type OperationTaskReference = Reference;

/// a namespaced rule reference. e.g. "core::unique_feeder_ids"
pub type ProcessRuleReference = Reference;

impl Process {
    pub fn has_rule(&self, rule: &ProcessRuleReference) -> bool {
        self.rules.contains(rule)
    }
}

#[derive(Error, Debug)]
pub enum ProcessError {
    #[error("Unused process. processes: {:?}, process: '{}'", processes, process)]
    UndefinedProcessError { processes: Vec<Process>, process: String },
}

#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Clone)]
pub struct ProcessOperationState {
    pub reference: ProcessOperationReference,
    // TODO probably build the overall status dynamically... (cache it?)
    //pub status: OperationStatus,

    pub task_states: IndexMap<OperationTaskReference, Box<dyn SerializableOperationTaskState>>,
}

impl ProcessOperationState {
    pub fn is_complete(&self) -> bool {
        self.task_states.iter().fold(true, |complete, (reference, task_state)| {
            if !complete {
                return false;
            }

            complete && matches!(task_state.status(), OperationTaskStatus::Complete)
        })
    }
}

#[typetag::serde(tag = "type")]
pub trait SerializableOperationTaskState: OperationTaskState + DynEq + DynClone + AsAny + Send + Sync + Debug {
}
dyn_eq::eq_trait_object!(SerializableOperationTaskState);
dyn_clone::clone_trait_object!(SerializableOperationTaskState);

pub trait OperationTaskState {
    fn status(&self) -> OperationTaskStatus;
    fn reset(&mut self);

    fn is_complete(&self) -> bool {
        matches!(self.status(), OperationTaskStatus::Complete)
    }

    fn can_complete(&self) -> bool;

    /// Will panic if not completable.
    /// 
    /// See [`Self::can_complete`]
    fn set_completed(&mut self);

    // TODO add these, probably needed when adding/removing placements to a phase
    // fn on_placement_removed(&mut self, &Placement)
    // fn on_placement_added(&mut self, &Placement)

}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq)]
pub enum OperationTaskStatus {
    Pending,
    Incomplete,
    Complete,
    Abandoned,
}

impl Default for OperationTaskStatus {
    fn default() -> Self {
        Self::Pending
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Default, PartialEq, Eq)]
pub struct PlacementOperationState {
    pub placed: usize,
    pub skipped: usize,
    pub total: usize,

    status: OperationTaskStatus,
}

#[typetag::serde(name = "core::placement_operation_state")]
impl SerializableOperationTaskState for PlacementOperationState {}

impl PlacementOperationState {
    pub fn are_all_placements_placed(&self) -> bool {
        (self.placed + self.skipped) == self.total
    }
}

impl OperationTaskState for PlacementOperationState {
    fn status(&self) -> OperationTaskStatus {
        self.status.clone()
    }

    fn reset(&mut self) {
        *self = Self::default()
    }

    /// This task can only be completed
    fn can_complete(&self) -> bool {
        false
    }

    /// See [`Self::can_complete`]
    fn set_completed(&mut self) {
        panic!("task cannot be completed this way");
    }
}


#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub enum ProcessOperationSetItem {
    Completed,
}

macro_rules! generic_operation_impl {
    ( $name:ident, $key:literal ) => {
        #[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Default, PartialEq, Eq)]
        pub struct $name {
            status: OperationTaskStatus,
        }
        
        #[typetag::serde(name = $key)]
        impl SerializableOperationTaskState for $name {}
        
        
        impl OperationTaskState for $name {
            fn status(&self) -> OperationTaskStatus {
                self.status.clone()
            }
        
            fn reset(&mut self) {
                *self = Self::default()
            }
        
            fn can_complete(&self) -> bool {
                true
            }
        
            fn set_completed(&mut self) {
                self.status = OperationTaskStatus::Complete;
            }
        }
        
    }
}

generic_operation_impl!(LoadPcbsOperationState, "core::load_pcbs_operation_state");
generic_operation_impl!(AutomatedSolderingOperationState, "core::automated_soldering_operation_state");
generic_operation_impl!(ManualSolderingOperationState, "core::manual_soldering_operation_state");

