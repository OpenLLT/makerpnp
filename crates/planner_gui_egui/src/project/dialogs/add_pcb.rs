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

                    form.add_field_ui("units", tr!("form-add-pcb-input-units"), tui, {
                        move |ui: &mut Ui, fields, sender| {
                            let mut units = fields.units;
                            ui.add(egui::DragValue::new(&mut units).range(1..=u16::MAX));

                            if units != fields.units {
                                sender
                                    .send(AddPcbModalUiCommand::UnitsChanged(units))
                                    .expect("sent");
                            }

                            ui.response()
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

    #[validate(range(min = 1, max = 65535, code = "form-input-error-range"))]
    units: u16,
}

#[derive(Debug, Clone)]
pub enum AddPcbModalUiCommand {
    Submit,
    Cancel,

    NameChanged(String),
    UnitsChanged(u16),
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
    pub units: u16,
}

impl UiComponent for AddPcbModal {
    type UiContext<'context> = ();
    type UiCommand = AddPcbModalUiCommand;
    type UiAction = AddPcbModalAction;

    fn ui<'context>(&self, ui: &mut egui::Ui, _context: &mut Self::UiContext<'context>) {
        ui.ctx().style_mut(|style| {
            // if this is not done, text in labels/checkboxes/etc wraps
            style.wrap_mode = Some(egui::TextWrapMode::Extend);
        });

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

            let form = Form::new(&self.fields, &self.component.sender, ());

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
                        .add_enabled(form.is_valid(), egui::Button::new(tr!("form-button-ok")))
                        .clicked()
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
                    units: fields.units,
                };
                Some(AddPcbModalAction::Submit(args))
            }
            AddPcbModalUiCommand::NameChanged(name) => {
                self.fields.lock().unwrap().name = name;
                None
            }
            AddPcbModalUiCommand::UnitsChanged(units) => {
                self.fields.lock().unwrap().units = units;
                None
            }
            AddPcbModalUiCommand::Cancel => Some(AddPcbModalAction::CloseDialog),
        }
    }
}
