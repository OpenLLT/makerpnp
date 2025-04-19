use std::collections::HashMap;
use std::fmt::Debug;

use dyn_clone::DynClone;
use dyn_eq::DynEq;
use planning::placement::PlacementOperation;
use planning::process::{OperationReference, TaskReference, TaskStatus};
use pnp::object_path::ObjectPath;
use serde_json::Value;
use serde_with::serde_as;
use serde_with::DisplayFromStr;
use time::serde::rfc3339;
use time::OffsetDateTime;

#[derive(Debug, Clone, serde::Deserialize, PartialEq)]
pub enum TestOperationHistoryPlacementOperation {
    Placed,
}

#[typetag::serde(tag = "type")]
pub trait TestOperationHistoryKind: DynClone + DynEq + Debug {}
dyn_eq::eq_trait_object!(TestOperationHistoryKind);
dyn_clone::clone_trait_object!(TestOperationHistoryKind);

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, PartialEq, Eq)]
pub struct TestLoadPcbsOperationTaskHistoryKind {
    pub(crate) status: TaskStatus,
}

#[typetag::serde(name = "load_pcbs_operation")]
impl TestOperationHistoryKind for TestLoadPcbsOperationTaskHistoryKind {}

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, PartialEq, Eq)]
pub struct TestPlaceComponentsOperationTaskHistoryKind {
    pub(crate) status: TaskStatus,
}

#[typetag::serde(name = "place_components_operation")]
impl TestOperationHistoryKind for TestPlaceComponentsOperationTaskHistoryKind {}

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, PartialEq, Eq)]
pub struct TestManualSolderingOperationTaskHistoryKind {
    pub(crate) status: TaskStatus,
}

#[typetag::serde(name = "manual_soldering_operation")]
impl TestOperationHistoryKind for TestManualSolderingOperationTaskHistoryKind {}

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, PartialEq, Eq)]
pub struct TestAutomatedSolderingOperationTaskHistoryKind {
    pub(crate) status: TaskStatus,
}

#[typetag::serde(name = "automated_soldering_operation")]
impl TestOperationHistoryKind for TestAutomatedSolderingOperationTaskHistoryKind {}

#[serde_as]
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, PartialEq, Eq)]
pub struct TestPlacementOperationHistoryKind {
    #[serde_as(as = "DisplayFromStr")]
    pub object_path: ObjectPath,
    pub operation: PlacementOperation,
}

#[typetag::serde(name = "placement_operation")]
impl TestOperationHistoryKind for TestPlacementOperationHistoryKind {}

#[derive(Debug, serde::Deserialize, Clone)]
pub struct TestOperationHistoryItem {
    #[serde(with = "rfc3339")]
    pub date_time: OffsetDateTime,
    pub phase: String,

    pub operation_reference: OperationReference,
    pub task_reference: TaskReference,
    pub task_history: Box<dyn TestOperationHistoryKind>,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}
