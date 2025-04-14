use std::collections::HashMap;
use std::fmt::Debug;
use std::fs::File;
use std::path::PathBuf;

use anyhow::Error;
use as_any::AsAny;
use pnp::object_path::ObjectPath;
use serde::Serialize;
use serde_json::Value;
use serde_with::serde_as;
use serde_with::DisplayFromStr;
use time::serde::rfc3339;
use time::OffsetDateTime;
use tracing::info;

use crate::placement::PlacementOperation;
use crate::process::{OperationReference, TaskReference, TaskStatus};
use crate::reference::Reference;

#[typetag::serde(tag = "type")]
pub trait OperationHistoryKind: AsAny + Debug {}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct LoadPcbsOperationTaskHistoryKind {
    pub(crate) status: TaskStatus,
}

#[typetag::serde(name = "load_pcbs_operation")]
impl OperationHistoryKind for LoadPcbsOperationTaskHistoryKind {}

#[serde_as]
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct PlacementOperationHistoryKind {
    #[serde_as(as = "DisplayFromStr")]
    pub object_path: ObjectPath,
    pub operation: PlacementOperation,
}

#[typetag::serde(name = "placement_operation")]
impl OperationHistoryKind for PlacementOperationHistoryKind {}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct OperationHistoryItem {
    #[serde(with = "rfc3339")]
    pub date_time: OffsetDateTime,
    pub phase: Reference,

    pub operation_reference: OperationReference,
    pub task_reference: TaskReference,
    pub task_history: Box<dyn OperationHistoryKind>,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

pub fn write(phase_log_path: PathBuf, operation_history: &Vec<OperationHistoryItem>) -> Result<(), Error> {
    // TODO use a context for better error messages
    let is_new = !phase_log_path.exists();

    let file = File::create(phase_log_path.clone())?;

    let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
    let mut ser = serde_json::Serializer::with_formatter(file, formatter);
    operation_history.serialize(&mut ser)?;

    match is_new {
        true => info!("Created operation history file. path: {:?}\n", phase_log_path),
        false => info!("Updated operation history file. path: {:?}\n", phase_log_path),
    }

    Ok(())
}

pub fn read_or_default(phase_log_path: &PathBuf) -> Result<Vec<OperationHistoryItem>, Error> {
    let is_new = !phase_log_path.exists();
    if is_new {
        return Ok(Default::default());
    }

    // TODO use a context for better error messages
    let file = File::open(phase_log_path.clone())?;

    let operation_history = serde_json::from_reader(file)?;

    Ok(operation_history)
}
