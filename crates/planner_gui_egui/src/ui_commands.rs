use std::path::PathBuf;
use egui_mobius::types::{Enqueue, Value};
use tracing::trace;
use crate::project::{ProjectKey, ProjectUiCommand};
use crate::task::Task;
use crate::ui_app::{AppState, PersistentUiState};

#[derive(Debug, Clone)]
pub enum UiCommand {
    #[allow(dead_code)]
    None,
    ShowHomeTab,
    CloseAllTabs,
    OpenFile(PathBuf),
    OpenClicked,
    ProjectClosed(ProjectKey),
    ProjectCommand { key: ProjectKey, command: ProjectUiCommand },
}

pub fn handle_command(
    app_state: Value<AppState>,
    ui_state: Value<PersistentUiState>,
    command: UiCommand,
) -> Task<UiCommand>{
    trace!("Handling command: {:?}", command);
    
    match command {
        UiCommand::None => {
            Task::none()
        }
        UiCommand::ShowHomeTab => {
            let mut ui_state = ui_state.lock().unwrap();
            ui_state.show_home_tab();
            Task::none()
        }
        UiCommand::CloseAllTabs => {
            let mut ui_state = ui_state.lock().unwrap();
            ui_state.close_all_tabs();
            Task::none()
        }
        UiCommand::OpenFile(picked_file) => {
            let mut app_state = app_state.lock().unwrap();
            app_state.open_file(picked_file, ui_state);
            Task::none()
        }
        UiCommand::OpenClicked => {
            let mut app_state = app_state.lock().unwrap();
            app_state.pick_file();
            Task::none()
        }
        UiCommand::ProjectClosed(project_key) => {
            let mut app_state = app_state.lock().unwrap();
            app_state.close_project(project_key);
            Task::none()
        }
        UiCommand::ProjectCommand { key, command} => {
            // TODO find project, call `handle_command` on it with the command
            let app_state = app_state.lock().unwrap();
            let mut guard = app_state.projects.lock().unwrap();
            let project = guard.get_mut(key).unwrap();
            project.update(key, command)
                .map(move |(key, command)|{
                    UiCommand::ProjectCommand { key, command }
                })
        }
    }
}