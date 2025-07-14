use std::fmt::Debug;
use std::path::PathBuf;
use std::str::FromStr;

use derivative::Derivative;
use egui::{Button, Modal, TextEdit, Ui, Widget};
use egui_i18n::tr;
use egui_mobius::types::Value;
use egui_taffy::taffy::prelude::{auto, length, percent};
use egui_taffy::taffy::{AlignContent, AlignItems, Display, FlexDirection, Style};
use egui_taffy::{Tui, TuiBuilderLogic, taffy, tui};
use planner_app::{LoadOutSource, PcbSide, ProcessReference, Reference};
use taffy::Size;
use validator::{Validate, ValidationError};

use crate::file_picker::Picker;
use crate::forms::Form;
use crate::forms::transforms::no_transform;
use crate::project::dialogs::PcbSideChoice;
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct AddPhaseModal {
    fields: Value<AddPhaseFields>,
    processes: Vec<ProcessReference>,
    path: PathBuf,

    file_picker: Value<Picker>,

    pub component: ComponentState<AddPhaseModalUiCommand>,
}

impl AddPhaseModal {
    pub fn new(path: PathBuf, processes: Vec<ProcessReference>) -> Self {
        Self {
            fields: Default::default(),
            processes,
            path,
            component: Default::default(),
            file_picker: Default::default(),
        }
    }

    fn show_form(&self, ui: &mut Ui, form: &Form<AddPhaseFields, AddPhaseModalUiCommand>) {
        let default_style = || Style {
            padding: length(2.),
            gap: length(2.),
            ..Default::default()
        };

        tui(ui, ui.id().with("add_phase_form"))
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
                form.show_fields_vertical(tui, |form, tui| {
                    form.add_field_ui("reference", tr!("form-common-input-phase-reference"), tui, {
                        // NOTE text input does not resize with grid cell when using `.ui_add`, known issue - https://discord.com/channels/900275882684477440/904461220592119849/1338883750922293319
                        //      as a workaround we use `ui_add_manual` for now, with `no_transform`.
                        move |ui: &mut Ui, fields, sender| {
                            let mut reference_clone = fields.reference.clone();
                            let output = TextEdit::singleline(&mut reference_clone)
                                .desired_width(ui.available_width())
                                .show(ui);

                            if !fields.reference.eq(&reference_clone) {
                                sender
                                    .send(AddPhaseModalUiCommand::ReferenceChanged(reference_clone))
                                    .expect("sent")
                            }

                            output.response
                        }
                    });

                    form.add_field_tui("load_out", tr!("form-common-input-load-out-source"), tui, {
                        move |tui: &mut Tui, fields, sender| {
                            tui.style(Style {
                                display: Display::Flex,
                                align_content: Some(AlignContent::Stretch),
                                flex_grow: 1.0,
                                ..default_style()
                            })
                            .add(|tui| {
                                tui.style(Style {
                                    flex_grow: 1.0,
                                    ..default_style()
                                })
                                .ui_add_manual(
                                    |ui| {
                                        // NOTE text input does not resize with grid cell when using `.ui_add`, known issue - https://discord.com/channels/900275882684477440/904461220592119849/1338883750922293319
                                        //      as a workaround we use `ui_add_manual` for now, with `no_transform`.
                                        let mut chosen_path = fields
                                            .load_out
                                            .clone()
                                            .unwrap_or_default();

                                        // FUTURE consider making this interactive since loadout sources do not have to be files. (in the future they could be urls, etc)
                                        TextEdit::singleline(&mut chosen_path)
                                            .desired_width(ui.available_width())
                                            .interactive(false)
                                            .ui(ui)
                                    },
                                    no_transform,
                                );

                                if tui
                                    .style(Style {
                                        flex_grow: 0.0,
                                        ..default_style()
                                    })
                                    .ui_add(Button::new("..."))
                                    .clicked()
                                {
                                    sender
                                        .send(AddPhaseModalUiCommand::PickLoadoutSourceClicked)
                                        .expect("sent");
                                }
                            })
                        }
                    });

                    form.add_field_ui("pcb_side", tr!("form-common-choice-pcb-side"), tui, {
                        move |ui: &mut Ui, fields, sender| {
                            let side = fields.pcb_side.clone();

                            let available_size = ui.available_size();

                            ui.add_sized(available_size, |ui: &mut Ui| {
                                egui::ComboBox::from_id_salt(ui.id().with("pcb_side"))
                                    .width(ui.available_width())
                                    .selected_text(match side {
                                        None => tr!("form-common-combo-select"),
                                        Some(PcbSideChoice::Top) => tr!("form-common-choice-pcb-side-top"),
                                        Some(PcbSideChoice::Bottom) => tr!("form-common-choice-pcb-side-bottom"),
                                    })
                                    .show_ui(ui, |ui| {
                                        if ui
                                            .add(egui::Button::selectable(
                                                side == Some(PcbSideChoice::Top),
                                                tr!("form-common-choice-pcb-side-top"),
                                            ))
                                            .clicked()
                                        {
                                            sender
                                                .send(AddPhaseModalUiCommand::PcbSideChanged(PcbSideChoice::Top))
                                                .expect("sent");
                                        }
                                        if ui
                                            .add(egui::Button::selectable(
                                                side == Some(PcbSideChoice::Bottom),
                                                tr!("form-common-choice-pcb-side-bottom"),
                                            ))
                                            .clicked()
                                        {
                                            sender
                                                .send(AddPhaseModalUiCommand::PcbSideChanged(PcbSideChoice::Bottom))
                                                .expect("sent");
                                        }
                                    })
                                    .response
                            })
                        }
                    });

                    form.add_field_ui("process", tr!("form-common-choice-process"), tui, {
                        move |ui: &mut Ui, fields, sender| {
                            let process = fields.process.clone();

                            let available_size = ui.available_size();

                            ui.add_sized(available_size, |ui: &mut Ui| {
                                egui::ComboBox::from_id_salt(ui.id().with("process"))
                                    .width(ui.available_width())
                                    .selected_text(process.clone().map_or_else(
                                        || tr!("form-common-combo-select"),
                                        |process_name| process_name.to_string(),
                                    ))
                                    .show_ui(ui, move |ui| {
                                        for selectable_process in self.processes.iter() {
                                            if ui
                                                .add(egui::Button::selectable(
                                                    process
                                                        .as_ref()
                                                        .is_some_and(|selected_process| {
                                                            selectable_process.eq(selected_process)
                                                        }),
                                                    selectable_process.to_string(),
                                                ))
                                                .clicked()
                                            {
                                                sender
                                                    .send(AddPhaseModalUiCommand::ProcessChanged(
                                                        selectable_process.clone(),
                                                    ))
                                                    .expect("sent");
                                            }
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
pub struct AddPhaseFields {
    // FUTURE could also validate that the reference is not already used
    #[validate(length(min = 1, code = "form-input-error-length"))]
    #[validate(custom(function = "CommonValidation::validate_reference"))]
    reference: String,

    #[validate(required(code = "form-option-error-required"))]
    pcb_side: Option<PcbSideChoice>,

    #[validate(required(code = "form-option-error-required"))]
    #[validate(custom(function = "CommonValidation::validate_optional_process_reference"))]
    process: Option<ProcessReference>,

    #[validate(required(code = "form-option-error-required"))]
    #[validate(custom(function = "CommonValidation::validate_optional_loadout_source"))]
    load_out: Option<String>,
}

struct CommonValidation {}
impl CommonValidation {
    pub fn validate_reference(reference: &String) -> Result<(), ValidationError> {
        Reference::from_str(&reference)
            .map_err(|_e| ValidationError::new("form-input-error-reference-invalid"))
            .map(|_| ())
    }

    pub fn validate_optional_process_reference(process_reference: &ProcessReference) -> Result<(), ValidationError> {
        match process_reference.is_valid() {
            true => Ok(()),
            false => Err(ValidationError::new("form-input-error-process-reference-invalid")),
        }
    }

    pub fn validate_optional_loadout_source(load_out_source: &String) -> Result<(), ValidationError> {
        LoadOutSource::from_str(&load_out_source)
            .map_err(|_e| ValidationError::new("form-input-error-loadout-source-invalid"))
            .map(|_loadout_source| ())
    }
}

#[derive(Debug, Clone)]
pub enum AddPhaseModalUiCommand {
    Submit,
    Cancel,

    ReferenceChanged(String),
    PcbSideChanged(PcbSideChoice),
    PickLoadoutSourceClicked,
    ProcessChanged(ProcessReference),
    LoadoutSourcePicked(String),
}

#[derive(Debug, Clone)]
pub enum AddPhaseModalAction {
    Submit(AddPhaseArgs),
    CloseDialog,
}

/// Value object
#[derive(Debug, Clone)]
pub struct AddPhaseArgs {
    pub process: ProcessReference,
    pub reference: Reference,
    pub load_out: LoadOutSource,
    pub pcb_side: PcbSide,
}

impl UiComponent for AddPhaseModal {
    type UiContext<'context> = ();
    type UiCommand = AddPhaseModalUiCommand;
    type UiAction = AddPhaseModalAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut egui::Ui, _context: &mut Self::UiContext<'context>) {
        if let Ok(picked_file) = self
            .file_picker
            .lock()
            .unwrap()
            .picked()
        {
            self.component
                .send(AddPhaseModalUiCommand::LoadoutSourcePicked(
                    picked_file
                        .as_path()
                        .to_str()
                        .unwrap()
                        .to_string(),
                ));
        }

        ui.ctx().style_mut(|style| {
            // if this is not done, text in labels/checkboxes/etc wraps
            style.wrap_mode = Some(egui::TextWrapMode::Extend);
        });

        let modal_id = ui.id().with("add_phase_modal");

        Modal::new(modal_id).show(ui.ctx(), |ui| {
            ui.set_width(ui.available_width() * 0.8);

            let file_name = self
                .path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap();
            ui.heading(tr!("modal-add-phase-title", {file: file_name}));

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
                            .send(AddPhaseModalUiCommand::Cancel);
                    }

                    if ui
                        .add_enabled(form.is_valid(), egui::Button::new(tr!("form-button-ok")))
                        .clicked()
                    {
                        self.component
                            .send(AddPhaseModalUiCommand::Submit);
                    }
                },
            );
        });
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            AddPhaseModalUiCommand::Submit => {
                let fields = self.fields.lock().unwrap();
                let args = AddPhaseArgs {
                    // TODO Safety: form validation prevents process from being None
                    process: fields.process.clone().unwrap(),
                    // TODO Safety: form validation prevents reference from being invalid
                    reference: Reference::from_str(fields.reference.as_ref()).unwrap(),
                    // TODO Safety: form validation prevents load_out from being invalid or None
                    load_out: LoadOutSource::from_absolute_path(PathBuf::from(fields.load_out.as_ref().unwrap()))
                        .unwrap(),
                    // Safety: form validation prevents kind from being None
                    pcb_side: fields
                        .pcb_side
                        .clone()
                        .unwrap()
                        .try_into()
                        .unwrap(),
                };
                Some(AddPhaseModalAction::Submit(args))
            }
            AddPhaseModalUiCommand::ReferenceChanged(reference) => {
                self.fields.lock().unwrap().reference = reference;
                None
            }
            AddPhaseModalUiCommand::PcbSideChanged(pcb_side) => {
                self.fields.lock().unwrap().pcb_side = Some(pcb_side);
                None
            }
            AddPhaseModalUiCommand::ProcessChanged(process) => {
                self.fields.lock().unwrap().process = Some(process);
                None
            }
            AddPhaseModalUiCommand::Cancel => Some(AddPhaseModalAction::CloseDialog),
            AddPhaseModalUiCommand::PickLoadoutSourceClicked => {
                self.file_picker
                    .lock()
                    .unwrap()
                    .pick_file();
                None
            }
            AddPhaseModalUiCommand::LoadoutSourcePicked(path) => {
                self.fields.lock().unwrap().load_out = Some(path);
                None
            }
        }
    }
}
