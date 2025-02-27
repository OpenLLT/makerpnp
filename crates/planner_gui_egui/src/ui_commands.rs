use planner_app::{Event};
use egui_mobius::types::{Enqueue, Value};
use tracing::trace;
use crate::app_core::CoreService;
use crate::ui_app::{AppState, UiApp, UiState};

#[derive(Debug, Clone)]
pub enum UiCommand {
    #[allow(dead_code)]
    None,
    ShowHomeTab,
    CloseAllTabs,
}

pub fn handle_command(
    ui_state: Value<UiState>,
    command: UiCommand,
    core_service: Value<CoreService>,
    command_sender: Enqueue<UiCommand>,
) {
    let mut ui_state = ui_state.lock().unwrap();
    
    trace!("Handling command: {:?}", command);
    
    match command {
        UiCommand::None => {
            let mut core_service = core_service.lock().unwrap();
            core_service.update(Event::None, command_sender.clone());
        }
        UiCommand::ShowHomeTab => {
            ui_state.show_home_tab();
        }
        UiCommand::CloseAllTabs => {
            ui_state.close_all_tabs();
        }
    }
}