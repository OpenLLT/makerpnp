use std::collections::BTreeMap;
use std::fmt::{Debug, Display, Formatter};
use std::str::FromStr;
use dyn_clone::DynClone;
use dyn_eq::DynEq;
use indexmap::IndexMap;
use thiserror::Error;
use util::dynamic::as_any::AsAny;
use crate::reference::Reference;

/// e.g. `manual` or `pnp`
pub type ProcessReference = Reference;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq)]
pub struct Process {
    pub reference: ProcessReference,

    /// examples:
    /// for `Manual` = `["load-pcbs", "manually-place-and-solder"]`
    /// for `PnP` = `["load-pcbs", "place-components", "solder"]`
    ///
    pub operations: Vec<ProcessOperation>,
    pub rules: Vec<ProcessRuleReference>
}

/// A user defined (or pre-configured) process operation reference
/// e.g. "place-components"
pub type ProcessOperationReference = Reference;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq)]
pub struct ProcessOperation {
    /// e.g. "manually-place-and-solder"
    pub reference: ProcessOperationReference,

    /// e.g. `["core::place_components", "core::manual_solder"]`
    /// @see [`ProcessOperationState`]
    pub tasks: Vec<OperationTaskReference>,
}

/// e.g. "core::place-components"
pub type OperationTaskReference = Reference;

/// e.g. "core::unique-feeder-ids"
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

#[typetag::serde(name = "placement_operation_state")]
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

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Default, PartialEq, Eq)]
pub struct LoadPcbsOperationState {
    status: OperationTaskStatus,
}

#[typetag::serde(name = "load_pcbs_operation_state")]
impl SerializableOperationTaskState for LoadPcbsOperationState {}


impl OperationTaskState for LoadPcbsOperationState {
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