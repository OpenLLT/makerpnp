use planner_app::{OperationStatus, TaskAction, TaskStatus};
use tracing::{instrument, trace};

#[instrument]
pub fn build_task_actions(
    previous_operation_status: &Option<OperationStatus>,
    operation_status: &OperationStatus,
    previous_task_status: &Option<TaskStatus>,
    task_status: &TaskStatus,
    can_complete: bool,
) -> Option<Vec<TaskAction>> {
    trace!("building task actions");
    if !matches!(previous_operation_status, None | Some(OperationStatus::Complete)) {
        trace!(
            "previous operation status not complete. previous_operation_status: {:?}",
            previous_operation_status
        );
        return None;
    }

    if matches!(operation_status, OperationStatus::Complete | OperationStatus::Abandoned) {
        trace!(
            "operation status complete or abandoned. operation_status: {:?}",
            operation_status
        );
        return None;
    }

    if !matches!(previous_task_status, None | Some(TaskStatus::Complete)) {
        trace!(
            "previous task status not complete. previous_task_state: {:?}",
            previous_task_status
        );
        return None;
    }

    let mut task_actions = Vec::new();
    match task_status {
        TaskStatus::Pending => task_actions.push(TaskAction::Start),
        TaskStatus::Started => {
            if can_complete {
                task_actions.push(TaskAction::Complete);
            }
            task_actions.push(TaskAction::Abandon);
        }
        TaskStatus::Complete => {
            trace!("task status complete.");
            return None;
        }
        TaskStatus::Abandoned => {
            trace!("task status abandoned.");
            return None;
        }
    }

    trace!("task actions: {:?}", task_actions);

    Some(task_actions)
}
