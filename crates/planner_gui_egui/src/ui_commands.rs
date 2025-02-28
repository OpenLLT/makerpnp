use std::path::PathBuf;
use planner_app::{Event};
use egui_mobius::types::{Enqueue, Value};
use tracing::{debug, trace};
use crate::planner_app_core::PlannerCoreService;
use crate::project::ProjectKey;
use crate::ui_app::{AppState, UiApp, PersistentUiState};

#[derive(Debug, Clone)]
pub enum UiCommand {
    #[allow(dead_code)]
    None,
    ShowHomeTab,
    CloseAllTabs,
    OpenFile(PathBuf),
    OpenClicked,
    ProjectClosed(ProjectKey),
}

pub fn handle_command(
    app_state: Value<AppState>,
    ui_state: Value<PersistentUiState>,
    command: UiCommand,
    command_sender: Enqueue<UiCommand>,
) {
    trace!("Handling command: {:?}", command);
    
    match command {
        UiCommand::None => {
        }
        UiCommand::ShowHomeTab => {
            let mut ui_state = ui_state.lock().unwrap();
            ui_state.show_home_tab();
        }
        UiCommand::CloseAllTabs => {
            let mut ui_state = ui_state.lock().unwrap();
            ui_state.close_all_tabs();
        }
        UiCommand::OpenFile(picked_file) => {
            let mut app_state = app_state.lock().unwrap();
            app_state.open_file(picked_file, ui_state);
        }
        UiCommand::OpenClicked => {
            let mut app_state = app_state.lock().unwrap();
            app_state.pick_file();
        }
        UiCommand::ProjectClosed(project_key) => {
            let mut app_state = app_state.lock().unwrap();
            app_state.close_project(project_key);
        }
    }
}