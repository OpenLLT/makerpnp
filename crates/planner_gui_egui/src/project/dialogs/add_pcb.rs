use std::fmt::Debug;
use std::path::PathBuf;
use egui::Modal;
use egui_i18n::tr;
use egui_mobius::types::Value;
use crate::project::dialogs::PcbKind;
use crate::project::ProjectKey;
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Debug)]
pub struct AddPcbModal {
    fields: Value<AddPcbFields>,
    
    path: PathBuf,
    key: ProjectKey,
    
    pub component: ComponentState<AddPcbModalUiCommand>
}

impl AddPcbModal {
    pub fn new(path: PathBuf, key: ProjectKey) -> Self {
        Self {
            fields: Default::default(),
            path,
            key,
            component: Default::default(),
        }
    }
}

#[derive(Debug, Default)]
pub struct AddPcbFields {
    name: String,
    kind: PcbKind,
}

#[derive(Debug, Clone)]
pub enum AddPcbModalUiCommand {
    Submit,
}

#[derive(Debug, Clone)]
pub enum AddPcbModalAction {
    CloseDialog,
}

impl UiComponent for AddPcbModal {
    type UiContext<'context> = ();
    type UiCommand = AddPcbModalUiCommand;
    type UiAction = AddPcbModalAction;

    fn ui<'context>(&self, ui: &mut egui::Ui, _context: &mut Self::UiContext<'context>) {
        let modal_id = ui.id().with("add_pcb_modal");

        Modal::new(modal_id).show(ui.ctx(), |ui| {
            ui.set_width(ui.available_width() * 0.8);

            let file_name = self
                .path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap();
            ui.heading(tr!("modal-add-pcb-title", {file: file_name}));


            egui::Sides::new().show(
                ui,
                |_ui| {},
                |ui| {
                    if ui
                        .button(tr!("form-button-ok"))
                        .clicked()
                    {
                        self.component
                            .send(AddPcbModalUiCommand::Submit);
                    }
                },
            );

        });
    }

    fn update<'context>(&mut self, _command: Self::UiCommand, _context: &mut Self::UiContext<'context>) -> Option<Self::UiAction> {
        match _command {
            AddPcbModalUiCommand::Submit => {
                // todo validation, etc...
                Some(AddPcbModalAction::CloseDialog)
            }
        }
    }
}