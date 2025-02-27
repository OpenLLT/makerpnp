use planner_app::{Event};
use egui_mobius::types::{Enqueue, Value};
use crate::app_core::CoreService;

#[derive(Debug, Clone)]
pub enum UiCommand {
    #[allow(dead_code)]
    None,
}

pub fn handle_command(
    command: UiCommand,
    core_service: Value<CoreService>,
    command_sender: Enqueue<UiCommand>,
) {
    match command {
        UiCommand::None => {
            let mut core_service = core_service.lock().unwrap();
            core_service.update(Event::None, command_sender.clone());
        }
    }
}