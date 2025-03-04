use std::path::PathBuf;
use egui_mobius::types::Value;
use tracing::trace;
use crate::project::{ProjectKey, ProjectUiCommand};
use crate::task::Task;
use crate::toolbar::{ToolbarAction, ToolbarUiCommand};
use crate::ui_app::{AppState, PersistentUiState};
use crate::ui_component::UiComponent;

#[derive(Debug, Clone)]
pub enum UiCommand {
    #[allow(dead_code)]
    None,
    ToolbarCommand(ToolbarUiCommand),
    OpenFile(PathBuf),
    ProjectClosed(ProjectKey),
    ProjectCommand { key: ProjectKey, command: ProjectUiCommand },
}

pub fn handle_command(
    app_state: Value<AppState>,
    ui_state: Value<PersistentUiState>,
    command: UiCommand,
) -> Task<UiCommand> {
    trace!("Handling command: {:?}", command);
    
    match command {
        UiCommand::None => {
            Task::none()
        }
        UiCommand::ToolbarCommand(command) => {
            let toolbar_action = app_state.lock().unwrap().toolbar.update(command);

            let task = handle_toolbar_action(toolbar_action, &app_state, &ui_state);
            task
        }
        UiCommand::OpenFile(picked_file) => {
            let mut app_state = app_state.lock().unwrap();
            app_state.open_file(picked_file, ui_state);
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
                .map(move |result|{
                    match result {
                        Ok((key, command)) => {
                            UiCommand::ProjectCommand { key, command }
                        }
                        Err(error) => {
                            UiCommand::ProjectCommand { key, command: ProjectUiCommand::Error(error) }
                        }
                    }
                    
                })
        }
    }
}

fn handle_toolbar_action(toolbar_action: Option<ToolbarAction>, app_state: &Value<AppState>, ui_state: &Value<PersistentUiState>) -> Task<UiCommand> {
    let Some(toolbar_action) = toolbar_action else { 
        return Task::none() 
    };
    
    match toolbar_action {
        ToolbarAction::ShowHomeTab => {
            let mut ui_state = ui_state.lock().unwrap();
            ui_state.show_home_tab();
            Task::none()
        }
        ToolbarAction::CloseAllTabs => {
            let mut ui_state = ui_state.lock().unwrap();
            ui_state.close_all_tabs();
            Task::none()
        }
        ToolbarAction::PickFile => {
            let mut app_state = app_state.lock().unwrap();
            app_state.pick_file();
            Task::none()
        }
    }
}