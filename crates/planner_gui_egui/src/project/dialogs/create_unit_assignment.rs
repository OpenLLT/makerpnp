use std::fmt::Debug;
use std::path::PathBuf;

use derivative::Derivative;
use egui::{Modal, TextEdit, Ui};
use egui_i18n::tr;
use egui_mobius::types::Value;
use egui_taffy::taffy::prelude::{auto, length, percent};
use egui_taffy::taffy::{AlignItems, FlexDirection, Style};
use egui_taffy::{taffy, tui};
use planner_app::ObjectPath;
use taffy::Size;
use tracing::debug;
use validator::{Validate, ValidationError};

use crate::forms::Form;
use crate::project::dialogs::PcbKindChoice;
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Debug)]
pub struct CreateUnitAssignmentModal {
    fields: Value<CreateUnitAssignmentFields>,

    path: PathBuf,
    placements_directory: PathBuf,

    pub component: ComponentState<CreateUnitAssignmentModalUiCommand>,
}

impl CreateUnitAssignmentModal {
    pub fn new(path: PathBuf) -> Self {
        let placements_directory = path
            .clone()
            .parent()
            .unwrap()
            .to_path_buf();
        Self {
            fields: Default::default(),
            path,
            placements_directory,
            component: Default::default(),
        }
    }

    fn show_form(&self, ui: &mut Ui, form: &Form<CreateUnitAssignmentFields, CreateUnitAssignmentModalUiCommand>) {
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
                    form.add_field_ui(
                        "design_name",
                        tr!("form-create-unit-assignment-input-design-name"),
                        tui,
                        {
                            move |ui: &mut Ui, fields, sender| {
                                let mut design_name_clone = fields.design_name.clone();
                                let output = TextEdit::singleline(&mut design_name_clone)
                                    // TODO add placeholder hint
                                    .desired_width(ui.available_width())
                                    .show(ui);

                                if !fields
                                    .design_name
                                    .eq(&design_name_clone)
                                {
                                    sender
                                        .send(CreateUnitAssignmentModalUiCommand::DesignNameChanged(design_name_clone))
                                        .expect("sent")
                                }

                                output.response
                            }
                        },
                    );
                    form.add_field_ui(
                        "variant_name",
                        tr!("form-create-unit-assignment-input-variant-name"),
                        tui,
                        {
                            move |ui: &mut Ui, fields, sender| {
                                let mut variant_name_clone = fields.variant_name.clone();
                                let output = TextEdit::singleline(&mut variant_name_clone)
                                    // TODO add placeholder hint
                                    .desired_width(ui.available_width())
                                    .show(ui);

                                if !fields
                                    .variant_name
                                    .eq(&variant_name_clone)
                                {
                                    sender
                                        .send(CreateUnitAssignmentModalUiCommand::VariantNameChanged(
                                            variant_name_clone,
                                        ))
                                        .expect("sent")
                                }

                                output.response
                            }
                        },
                    );

                    form.add_field_ui("pcb_kind", tr!("form-common-choice-pcb-kind"), tui, {
                        move |ui: &mut Ui, fields, sender| {
                            let kind = fields.pcb_kind.clone();

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
                                                .send(CreateUnitAssignmentModalUiCommand::PcbKindChanged(
                                                    PcbKindChoice::Single,
                                                ))
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
                                                .send(CreateUnitAssignmentModalUiCommand::PcbKindChanged(
                                                    PcbKindChoice::Panel,
                                                ))
                                                .expect("sent");
                                        }
                                    })
                                    .response
                            })
                        }
                    });

                    form.add_field_ui(
                        "placements_directory",
                        tr!("form-create-unit-assignment-input-placements-directory"),
                        tui,
                        {
                            move |ui: &mut Ui, _fields, _sender| {
                                ui.label(
                                    self.placements_directory
                                        .as_path()
                                        .to_str()
                                        .unwrap(),
                                )
                            }
                        },
                    );

                    form.add_field_ui(
                        "placements_filename",
                        tr!("form-create-unit-assignment-input-placements-filename"),
                        tui,
                        move |ui: &mut Ui, fields, _sender| ui.label(&fields.placements_filename),
                    );

                    form.add_field_ui(
                        "pcb_instance",
                        tr!("form-create-unit-assignment-input-pcb-instance"),
                        tui,
                        {
                            move |ui: &mut Ui, fields, sender| {
                                let mut pcb_instance_clone = fields.pcb_instance;
                                let enabled = fields.pcb_kind.is_some();
                                let response = ui.add_enabled(
                                    enabled,
                                    egui::DragValue::new(&mut pcb_instance_clone).range(1..=i16::MAX),
                                );
                                if !fields
                                    .pcb_instance
                                    .eq(&pcb_instance_clone)
                                {
                                    sender
                                        .send(CreateUnitAssignmentModalUiCommand::PcbInstanceChanged(
                                            pcb_instance_clone,
                                        ))
                                        .expect("sent")
                                }
                                response
                            }
                        },
                    );

                    form.add_field_ui("pcb_unit", tr!("form-create-unit-assignment-input-pcb-unit"), tui, {
                        move |ui: &mut Ui, fields, sender| {
                            let mut pcb_unit_clone = fields.pcb_unit;
                            let enabled = matches!(fields.pcb_kind, Some(PcbKindChoice::Panel));

                            let response =
                                ui.add_enabled(enabled, egui::DragValue::new(&mut pcb_unit_clone).range(1..=i16::MAX));

                            if !fields.pcb_unit.eq(&pcb_unit_clone) {
                                sender
                                    .send(CreateUnitAssignmentModalUiCommand::PcbUnitChanged(pcb_unit_clone))
                                    .expect("sent")
                            }
                            response
                        }
                    });
                });
            });
    }
}

#[derive(Clone, Derivative, Debug, Validate, serde::Deserialize, serde::Serialize)]
#[derivative(Default)]
#[validate(context = CreateUnitAssignmentValidationContext)]
pub struct CreateUnitAssignmentFields {
    #[validate(length(min = 1, code = "form-input-error-length"))]
    design_name: String,
    #[validate(length(min = 1, code = "form-input-error-length"))]
    variant_name: String,

    // TODO validate placements file exists for the design_name and variant_name
    #[validate(custom(function = "CreateUnitAssignmentFields::validate_placements_filename", use_context))]
    placements_filename: String,

    // object path
    #[validate(required(code = "form-option-error-required"))]
    pcb_kind: Option<PcbKindChoice>,

    // TODO should be a number > 0 (?)
    #[derivative(Default(value = "1"))]
    pcb_instance: i16,

    // TODO only required when kind is 'panel'
    #[derivative(Default(value = "1"))]
    pcb_unit: i16,
}

pub struct CreateUnitAssignmentValidationContext {
    placements_directory: PathBuf,
}

impl CreateUnitAssignmentFields {
    fn update_placements_filename(&mut self) {
        let filename = format!("{}_{}_placements.csv", self.design_name, self.variant_name).to_string();
        self.placements_filename = filename;
    }

    fn validate_placements_filename(
        placements_filename: &String,
        context: &CreateUnitAssignmentValidationContext,
    ) -> Result<(), ValidationError> {
        let mut placements_directory = context.placements_directory.clone();

        placements_directory.push(placements_filename);
        if !placements_directory.exists() {
            debug!("placements file does not exist. filename: {:?}", placements_directory);
            Err(ValidationError::new("form-file-not-found"))
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone)]
pub enum CreateUnitAssignmentModalUiCommand {
    Submit,
    Cancel,

    DesignNameChanged(String),
    VariantNameChanged(String),
    PcbKindChanged(PcbKindChoice),
    PcbInstanceChanged(i16),
    PcbUnitChanged(i16),
}

#[derive(Debug, Clone)]
pub enum CreateUnitAssignmentModalAction {
    Submit(CreateUnitAssignmentArgs),
    CloseDialog,
}

/// Value object
#[derive(Debug, Clone)]
pub struct CreateUnitAssignmentArgs {
    pub design_name: String,
    pub variant_name: String,
    pub object_path: ObjectPath,
}

impl UiComponent for CreateUnitAssignmentModal {
    type UiContext<'context> = ();
    type UiCommand = CreateUnitAssignmentModalUiCommand;
    type UiAction = CreateUnitAssignmentModalAction;

    fn ui<'context>(&self, ui: &mut egui::Ui, _context: &mut Self::UiContext<'context>) {
        let modal_id = ui
            .id()
            .with("create_unit_assignment_modal");

        Modal::new(modal_id).show(ui.ctx(), |ui| {
            ui.set_width(ui.available_width() * 0.8);

            let file_name = self
                .path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap();
            ui.heading(tr!("modal-create-unit-assignment-title", {file: file_name}));

            let validation_context = CreateUnitAssignmentValidationContext {
                placements_directory: self.placements_directory.clone(),
            };

            let form = Form::new(&self.fields, &self.component.sender, &validation_context);

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
                            .send(CreateUnitAssignmentModalUiCommand::Cancel);
                    }
                    if ui
                        .button(tr!("form-button-ok"))
                        .clicked()
                        && form.is_valid()
                    {
                        self.component
                            .send(CreateUnitAssignmentModalUiCommand::Submit);
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
            CreateUnitAssignmentModalUiCommand::Submit => {
                let fields = self.fields.lock().unwrap();

                let mut object_path = ObjectPath::default();

                let pcb_kind = fields.pcb_kind.as_ref().unwrap();
                object_path.set_pcb_kind_and_instance(pcb_kind.clone().into(), fields.pcb_instance as usize);
                match pcb_kind {
                    PcbKindChoice::Single => {}
                    PcbKindChoice::Panel => {
                        object_path.set_pcb_unit(fields.pcb_unit as usize);
                    }
                }

                let args = CreateUnitAssignmentArgs {
                    design_name: fields.design_name.clone(),
                    variant_name: fields.variant_name.clone(),
                    object_path,
                };
                Some(CreateUnitAssignmentModalAction::Submit(args))
            }
            CreateUnitAssignmentModalUiCommand::DesignNameChanged(value) => {
                let mut fields = self.fields.lock().unwrap();
                fields.design_name = value;
                fields.update_placements_filename();
                None
            }
            CreateUnitAssignmentModalUiCommand::VariantNameChanged(value) => {
                let mut fields = self.fields.lock().unwrap();
                fields.variant_name = value;
                fields.update_placements_filename();
                None
            }
            CreateUnitAssignmentModalUiCommand::PcbKindChanged(value) => {
                self.fields.lock().unwrap().pcb_kind = Some(value);
                None
            }
            CreateUnitAssignmentModalUiCommand::PcbInstanceChanged(value) => {
                self.fields.lock().unwrap().pcb_instance = value;
                None
            }
            CreateUnitAssignmentModalUiCommand::PcbUnitChanged(value) => {
                self.fields.lock().unwrap().pcb_unit = value;
                None
            }
            CreateUnitAssignmentModalUiCommand::Cancel => Some(CreateUnitAssignmentModalAction::CloseDialog),
        }
    }
}
