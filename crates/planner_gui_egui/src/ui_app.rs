use egui_mobius::types::{Enqueue, Value};
use crate::ui_commands::UiCommand;

#[derive(Default)]
pub struct UiState {
}

pub struct UiApp {
    pub ui_state: Value<UiState>,
    pub command_sender: Enqueue<UiCommand>,
}

impl eframe::App for UiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
    }
}
