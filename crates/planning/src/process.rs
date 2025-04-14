//! PCBs are assembled using one or more processes.
//! Each assembly project has one or more phases.
//! Each phase uses a process.
//! Each process has one or more operations.
//! Each operation can be pending, incomplete, complete or abandoned.
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
use crate::placement::PlacementStatus;
use crate::reference::Reference;

/// e.g. `manual` or `pnp`
pub type ProcessReference = Reference;

/// The /definition/ of a process.
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq)]
pub struct ProcessDefinition {
    pub reference: ProcessReference,

    /// examples:
    /// for `Manual` = `["load-pcbs", "manually-place-and-solder"]`
    /// for `PnP` = `["load-pcbs", "place-components", "solder"]`
    ///
    pub operations: Vec<OperationDefinition>,
    
    /// examples: `["core::"]`
    pub rules: Vec<ProcessRuleReference>
}

/// A user defined (or pre-configured) process operation reference
/// e.g. "place-components"
pub type OperationReference = Reference;

/// The /definition/ of a process operation
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq)]
pub struct OperationDefinition {
    /// e.g. "manually-place-and-solder"
    pub reference: OperationReference,

    /// e.g. `["core::place_components", "core::manual_solder"]`
    /// @see [`OperationState`]
    pub tasks: Vec<TaskReference>,
}

/// a namespaced operation task reference.  e.g. "core::place_components"
pub type TaskReference = Reference;

/// a namespaced rule reference. e.g. "core::unique_feeder_ids"
pub type ProcessRuleReference = Reference;

impl ProcessDefinition {
    pub fn has_rule(&self, rule: &ProcessRuleReference) -> bool {
        self.rules.contains(rule)
    }
}

#[derive(Error, Debug)]
pub enum ProcessError {
    #[error("Undefined process. processes: {:?}, process: '{}'", processes, process)]
    UndefinedProcessError { processes: Vec<ProcessDefinition>, process: String },
}

#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Clone)]
pub struct OperationState {
    pub reference: OperationReference,
    // TODO probably build the overall status dynamically... (cache it?)
    //pub status: OperationStatus,

    pub task_states: IndexMap<TaskReference, Box<dyn SerializableTaskState>>,
}

impl OperationState {
    pub fn is_complete(&self) -> bool {
        self.task_states.iter().fold(true, |complete, (_reference, task_state)| {
            if !complete {
                return false;
            }

            complete && matches!(task_state.status(), TaskStatus::Complete)
        })
    }
}

#[typetag::serde(tag = "type")]
pub trait SerializableTaskState: TaskState + DynEq + DynClone + AsAny + Send + Sync + Debug {
}
dyn_eq::eq_trait_object!(SerializableTaskState);
dyn_clone::clone_trait_object!(SerializableTaskState);

pub trait TaskState {
    fn status(&self) -> TaskStatus;
    fn reset(&mut self);

    fn is_complete(&self) -> bool {
        matches!(self.status(), TaskStatus::Complete)
    }

    fn can_complete(&self) -> bool;

    fn set_started(&mut self);
    
    /// Will panic if not completable.
    /// 
    /// See [`Self::can_complete`]
    fn set_completed(&mut self);

    fn set_abandoned(&mut self);
    
    /// Allows callers to access this operation's placements api
    fn placements_api(&self) -> Option<&dyn PlacementsTaskAPI> { None::<&dyn PlacementsTaskAPI> }
    
    fn requires_placements(&self) -> bool {
        self.placements_api().is_some()
    }
}

pub trait PlacementsTaskAPI {
    fn on_placement_status_change(&mut self, new_status: &PlacementStatus, old_status: &PlacementStatus);

    fn set_total_placements(&mut self, total: usize);
}

/// Allowed transitions
/// 
/// Pending -> Started
/// Started -> Complete | Abandoned
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    Started,
    Complete,
    Abandoned,
}

impl Default for TaskStatus {
    fn default() -> Self {
        Self::Pending
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Default, PartialEq, Eq)]
pub struct PlacementTaskState {
    pub placed: usize,
    pub skipped: usize,
    pub total: usize,

    // FUTURE consider using some struct that wraps the status to prevent disallowed state changes
    status: TaskStatus,
}

#[typetag::serde(name = "core::placement_task_state")]
impl SerializableTaskState for PlacementTaskState {}

impl PlacementTaskState {
    pub fn are_all_placements_placed(&self) -> bool {
        (self.placed + self.skipped) == self.total
    }
}

impl TaskState for PlacementTaskState {
    fn status(&self) -> TaskStatus {
        self.status.clone()
    }

    fn reset(&mut self) {
        *self = Self::default()
    }

    /// This task can only be completed via other apis
    fn can_complete(&self) -> bool {
        false
    }

    fn set_started(&mut self) {
        self.status = TaskStatus::Started;
    }

    /// See [`Self::can_complete`]
    fn set_completed(&mut self) {
        panic!("task cannot be completed this way");
    }

    fn set_abandoned(&mut self) {
        self.status = TaskStatus::Abandoned;
    }

    fn placements_api(&self) -> Option<&dyn PlacementsTaskAPI> {
        Some(self)
    }
}

impl PlacementsTaskAPI for PlacementTaskState {
    fn on_placement_status_change(&mut self, new_status: &PlacementStatus, old_status: &PlacementStatus) {
        match old_status {
            PlacementStatus::Placed => self.placed -= 1,
            PlacementStatus::Skipped => self.skipped -= 1,
            PlacementStatus::Pending => {}
        }
        match new_status {
            PlacementStatus::Placed => self.placed += 1,
            PlacementStatus::Skipped => self.skipped += 1,
            PlacementStatus::Pending => {}
        }
    }
    
    fn set_total_placements(&mut self, total: usize) {
        self.total = total;
    }
}


#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub enum OperationAction {
    Started,
    Completed,
    Abandoned,
}

macro_rules! generic_task_impl {
    ( $name:ident, $key:literal ) => {
        #[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Default, PartialEq, Eq)]
        pub struct $name {
            status: TaskStatus,
        }
        
        #[typetag::serde(name = $key)]
        impl SerializableTaskState for $name {}
        
        
        impl TaskState for $name {
            fn status(&self) -> TaskStatus {
                self.status.clone()
            }
        
            fn reset(&mut self) {
                *self = Self::default()
            }
        
            fn can_complete(&self) -> bool {
                true
            }
        
            fn set_started(&mut self) {
                self.status = TaskStatus::Started;
            }
            
            fn set_completed(&mut self) {
                self.status = TaskStatus::Complete;
            }
            
            fn set_abandoned(&mut self) {
                self.status = TaskStatus::Abandoned;
            }
        }
    }
}

generic_task_impl!(LoadPcbsOperationState, "core::load_pcbs_task_state");
generic_task_impl!(AutomatedSolderingOperationState, "core::automated_soldering_task_state");
generic_task_impl!(ManualSolderingOperationState, "core::manual_soldering_operation_state");

