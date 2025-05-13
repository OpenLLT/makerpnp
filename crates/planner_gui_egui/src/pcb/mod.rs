use std::path::PathBuf;

use egui::Ui;
use egui_mobius::Value;
use egui_mobius::types::Enqueue;
use slotmap::new_key_type;
use tracing::debug;

use crate::task::Task;
use crate::ui_component::{ComponentState, UiComponent};

new_key_type! {
    /// A key for a pcb
    pub struct PcbKey;
}

#[derive(Debug)]
pub enum PcbAction {
    Task(PcbKey, Task<PcbAction>),
    SetModifiedState(bool),
    UiCommand(PcbUiCommand),
    RequestRepaint,
}

pub struct Pcb {
    pcb_ui_state: Value<PcbUiState>,
    path: Option<PathBuf>,

    modified: bool,

    pub component: ComponentState<(PcbKey, PcbUiCommand)>,
}

impl Pcb {
    pub fn from_path(path: PathBuf, key: PcbKey) -> (Self, PcbUiCommand) {
        let instance = Self::new_inner(Some(path), key, None);
        (instance, PcbUiCommand::Load)
    }

    pub fn new(key: PcbKey) -> (Self, PcbUiCommand) {
        let instance = Self::new_inner(None, key, None);
        (instance, PcbUiCommand::Load)
    }

    fn new_inner(path: Option<PathBuf>, key: PcbKey, name: Option<String>) -> Self {
        debug!("Creating pcb instance from path. path: {:?}", path);

        let component: ComponentState<(PcbKey, PcbUiCommand)> = ComponentState::default();
        let component_sender = component.sender.clone();

        let pcb_ui_state = Value::new(PcbUiState::new(key, name, component_sender.clone()));

        Self {
            pcb_ui_state,
            path,
            modified: false,
            component,
        }
    }
}

#[derive(Debug)]
pub struct PcbUiState {
    name: Option<String>,

    key: PcbKey,
    sender: Enqueue<(PcbKey, PcbUiCommand)>,
}

impl PcbUiState {
    pub fn new(key: PcbKey, name: Option<String>, sender: Enqueue<(PcbKey, PcbUiCommand)>) -> Self {
        Self {
            name,
            key,
            sender,
        }
    }
}

#[derive(Debug, Clone)]
pub enum PcbUiCommand {
    None,
    DebugMarkModified,
    Load,
    Save,
}

pub struct PcbContext {
    pub key: PcbKey,
}

impl UiComponent for Pcb {
    type UiContext<'context> = PcbContext;
    type UiCommand = (PcbKey, PcbUiCommand);
    type UiAction = PcbAction;

    fn ui<'context>(&self, ui: &mut Ui, context: &mut Self::UiContext<'context>) {
        let PcbContext {
            key,
        } = context;

        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.label("TODO: PCB UI"); // TODO

            if ui.button("mark modified").clicked() {
                self.component
                    .send((*key, PcbUiCommand::DebugMarkModified))
            }
        });
    }

    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        let (_key, command) = command;

        match command {
            PcbUiCommand::None => None,
            PcbUiCommand::DebugMarkModified => {
                self.modified = true;
                Some(PcbAction::SetModifiedState(self.modified))
            }
            PcbUiCommand::Load => {
                debug!("Loading pcb. path: {:?}", self.path);
                None
            }
            PcbUiCommand::Save => {
                debug!("Saving pcb. path: {:?}", self.path);
                None
            }
        }
    }
}
