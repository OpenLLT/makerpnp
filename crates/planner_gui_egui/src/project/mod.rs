use std::path::PathBuf;
use egui_mobius::types::{Enqueue, Value};
use slotmap::new_key_type;
use crate::app_core::CoreService;
use crate::ui_commands::UiCommand;

new_key_type! {
    /// A key for a project
    pub struct ProjectKey;
}

pub struct Project {
    key: ProjectKey,
    core_service: CoreService,
    sender: Enqueue<UiCommand>,
    path: PathBuf,
    project_ui_state: Value<ProjectUiState>,
}

impl Project {
    pub fn from_path(path: PathBuf, sender: Enqueue<UiCommand>, key: ProjectKey) -> Self {
        let project_ui_state = Value::new(ProjectUiState::default());
        
        let core_service = CoreService::new(project_ui_state.clone());
        Self {
            key,
            sender,
            path,
            core_service,
            project_ui_state,
        }
        
    }
}

#[derive(Default, Debug)]
pub struct ProjectUiState {
    loaded: bool,
}