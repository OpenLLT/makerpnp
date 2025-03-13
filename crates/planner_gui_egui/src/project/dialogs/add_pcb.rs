use std::fmt::Debug;
use std::path::PathBuf;

use egui::{Modal, TextEdit, Ui};
use egui_i18n::tr;
use egui_mobius::types::Value;
use egui_taffy::taffy::prelude::{auto, length, percent};
use egui_taffy::taffy::{AlignItems, FlexDirection, Style};
use egui_taffy::{taffy, tui};
use taffy::Size;
use validator::Validate;

use crate::forms::Form;
use crate::project::dialogs::PcbKindChoice;
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Debug)]
pub struct AddPcbModal {
    fields: Value<AddPcbFields>,

    path: PathBuf,

    pub component: ComponentState<AddPcbModalUiCommand>,
}

impl AddPcbModal {
    pub fn new(path: PathBuf) -> Self {
        Self {
            fields: Default::default(),
            path,
            component: Default::default(),
        }
    }

    fn show_form(&self, ui: &mut Ui, form: &Form<AddPcbFields, AddPcbModalUiCommand>) {
        let default_style = || Style {
            padding: length(2.),
            gap: length(2.),
            ..Default::default()
        };

        tui(ui, ui.id().with("add_pcb_form"))
            .reserve_available_width()
            .style(Style {
                align_items: Some(AlignItems::Center),
                flex_direction: FlexDirection::Column,
                size: Size {
                    width: percent(1.),
                    height: auto(),
                },
                padding: length(8.),
                gap: length(8.),
                ..default_style()
            })
            .show(|tui| {
                form.show_fields(tui, |form, tui| {
                    form.add_field_ui("name", tr!("form-add-pcb-input-name"), tui, {
                        // NOTE text input does not resize with grid cell when using `.ui_add`, known issue - https://discord.com/channels/900275882684477440/904461220592119849/1338883750922293319
                        //      as a workaround we use `ui_add_manual` for now, with `no_transform`.
                        move |ui: &mut Ui, fields, sender| {
                            let mut name_clone = fields.name.clone();
                            let output = TextEdit::singleline(&mut name_clone)
                                .desired_width(ui.available_width())
                                .show(ui);

                            if !fields.name.eq(&name_clone) {
                                sender
                                    .send(AddPcbModalUiCommand::NameChanged(name_clone))
                                    .expect("sent")
                            }

                            output.response
                        }
                    });

                    form.add_field_ui("pcb_kind", tr!("form-common-choice-pcb-kind"), tui, {
                        move |ui: &mut Ui, fields, sender| {
                            let kind = fields.kind.clone();

                            let available_size = ui.available_size();

                            ui.add_sized(available_size, |ui: &mut Ui| {
                                let kind_id = ui.id();
                                egui::ComboBox::from_id_salt(kind_id)
                                    .width(ui.available_width())
                                    .selected_text(match kind {
                                        None => tr!("form-common-combo-default"),
                                        Some(PcbKindChoice::Single) => tr!("form-common-choice-pcb-kind-single"),
                                        Some(PcbKindChoice::Panel) => {
                                            tr!("form-common-choice-pcb-kind-panel")
                                        }
                                    })
                                    .show_ui(ui, |ui| {
                                        if ui
                                            .add(egui::SelectableLabel::new(
                                                kind == Some(PcbKindChoice::Single),
                                                tr!("form-common-choice-pcb-kind-single"),
                                            ))
                                            .clicked()
                                        {
                                            sender
                                                .send(AddPcbModalUiCommand::PcbKindChanged(PcbKindChoice::Single))
                                                .expect("sent");
                                        }
                                        if ui
                                            .add(egui::SelectableLabel::new(
                                                kind == Some(PcbKindChoice::Panel),
                                                tr!("form-common-choice-pcb-kind-panel"),
                                            ))
                                            .clicked()
                                        {
                                            sender
                                                .send(AddPcbModalUiCommand::PcbKindChanged(PcbKindChoice::Panel))
                                                .expect("sent");
                                        }
                                    })
                                    .response
                            })
                        }
                    });
                });
            });
    }
}

#[derive(Clone, Debug, Default, Validate, serde::Deserialize, serde::Serialize)]
pub struct AddPcbFields {
    #[validate(length(min = 1, code = "form-input-error-length"))]
    name: String,
    #[validate(required(code = "form-option-error-required"))]
    kind: Option<PcbKindChoice>,
}

#[derive(Debug, Clone)]
pub enum AddPcbModalUiCommand {
    Submit,
    Cancel,

    NameChanged(String),
    PcbKindChanged(PcbKindChoice),
}

#[derive(Debug, Clone)]
pub enum AddPcbModalAction {
    Submit(AddPcbArgs),
    CloseDialog,
}

/// Value object
#[derive(Debug, Clone)]
pub struct AddPcbArgs {
    pub name: String,
    pub kind: planner_app::PcbKind,
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

            let form = Form::new(&self.fields, &self.component.sender);

            self.show_form(ui, &form);

            egui::Sides::new().show(
                ui,
                |_ui| {},
                |ui| {
                    if ui
                        .button(tr!("form-button-cancel"))
                        .clicked()
                    {
                        self.component
                            .send(AddPcbModalUiCommand::Cancel);
                    }
                    if ui
                        .button(tr!("form-button-ok"))
                        .clicked()
                        && form.is_valid()
                    {
                        self.component
                            .send(AddPcbModalUiCommand::Submit);
                    }
                },
            );
        });
    }

    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            AddPcbModalUiCommand::Submit => {
                let fields = self.fields.lock().unwrap();
                let args = AddPcbArgs {
                    name: fields.name.clone(),
                    // Safety: form validation prevents kind from being None
                    kind: fields
                        .kind
                        .clone()
                        .unwrap()
                        .try_into()
                        .unwrap(),
                };
                Some(AddPcbModalAction::Submit(args))
            }
            AddPcbModalUiCommand::NameChanged(name) => {
                self.fields.lock().unwrap().name = name;
                None
            }
            AddPcbModalUiCommand::PcbKindChanged(kind) => {
                self.fields.lock().unwrap().kind = Some(kind);
                None
            }
            AddPcbModalUiCommand::Cancel => Some(AddPcbModalAction::CloseDialog),
        }
    }
}
