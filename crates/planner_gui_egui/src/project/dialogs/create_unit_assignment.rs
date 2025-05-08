use std::collections::HashMap;
use std::fmt::Debug;
use std::ops::RangeInclusive;
use std::path::PathBuf;

use derivative::Derivative;
use egui::{Modal, TextEdit, Ui};
use egui_double_slider::DoubleSlider;
use egui_i18n::tr;
use egui_mobius::types::Value;
use egui_taffy::taffy::prelude::{auto, length, percent};
use egui_taffy::taffy::{AlignContent, AlignItems, Display, FlexDirection, Style};
use egui_taffy::{Tui, TuiBuilderLogic, taffy, tui};
use planner_app::{DesignName, PcbUnitIndex, VariantName};
use taffy::Size;
use tracing::debug;
use validator::{Validate, ValidationError};

use crate::forms::Form;
use crate::forms::transforms::no_transform;
use crate::project::dialogs::PcbKindChoice;
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Debug)]
pub struct CreateUnitAssignmentModal {
    fields: Value<CreateUnitAssignmentFields>,

    /// The instance of the PCB
    pcb_index: u16,

    /// The number of units in the PCB
    units: u16,

    path: PathBuf,
    placements_directory: PathBuf,

    pub component: ComponentState<CreateUnitAssignmentModalUiCommand>,
}

impl CreateUnitAssignmentModal {
    pub fn new(path: PathBuf, pcb_index: u16, units: u16) -> Self {
        let placements_directory = path
            .clone()
            .parent()
            .unwrap()
            .to_path_buf();
        Self {
            pcb_index,
            units,
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
                        "pcb_instance",
                        tr!("form-create-unit-assignment-input-pcb-instance"),
                        tui,
                        {
                            move |ui: &mut Ui, fields, sender| {
                                let mut pcb_instance_clone = fields.pcb_instance.to_string();
                                let output = TextEdit::singleline(&mut pcb_instance_clone)
                                    .interactive(false)
                                    .desired_width(ui.available_width())
                                    .show(ui);

                                output.response
                            }
                        },
                    );

                    form.add_field_ui(
                        "design_name",
                        tr!("form-create-unit-assignment-input-design-name"),
                        tui,
                        {
                            move |ui: &mut Ui, fields, sender| {
                                let mut design_name_clone = fields.design_name.clone();
                                let output = TextEdit::singleline(&mut design_name_clone)
                                    .interactive(false)
                                    .desired_width(ui.available_width())
                                    .show(ui);

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

                    form.add_field_tui(
                        "pcb_unit_range",
                        tr!("form-create-unit-assignment-input-pcb-unit-range"),
                        tui,
                        {
                            move |tui: &mut Tui, fields, sender| {
                                let mut pcb_unit_start = fields.pcb_unit_range.start().clone();
                                let mut pcb_unit_end = fields.pcb_unit_range.end().clone();
                                let enabled = matches!(fields.pcb_kind, Some(PcbKindChoice::Panel));

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
                                            ui.horizontal_centered(|ui| {
                                                // FIXME make the width auto-size
                                                ui.add_enabled(
                                                    enabled,
                                                    DoubleSlider::new(
                                                        &mut pcb_unit_start,
                                                        &mut pcb_unit_end,
                                                        1..=self.units,
                                                    )
                                                    .separation_distance(0)
                                                    .width(400.0),
                                                )
                                            })
                                            .response
                                        },
                                        no_transform,
                                    );

                                    tui.style(Style {
                                        flex_grow: 0.0,
                                        ..default_style()
                                    })
                                    .ui(|ui| {
                                        ui.add_enabled(
                                            enabled,
                                            egui::DragValue::new(&mut pcb_unit_start).range(1..=pcb_unit_end),
                                        );
                                    });

                                    tui.style(Style {
                                        flex_grow: 0.0,
                                        ..default_style()
                                    })
                                    .ui(|ui| {
                                        ui.add_enabled(
                                            enabled,
                                            egui::DragValue::new(&mut pcb_unit_end).range(pcb_unit_start..=self.units),
                                        );
                                    });
                                });

                                let pcb_unit_range = RangeInclusive::new(pcb_unit_start, pcb_unit_end);

                                if fields.pcb_unit_range != pcb_unit_range {
                                    sender
                                        .send(CreateUnitAssignmentModalUiCommand::PcbUnitRangeChanged(pcb_unit_range))
                                        .expect("sent")
                                }
                            }
                        },
                    );
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

    #[validate(custom(function = "CreateUnitAssignmentFields::validate_placements_filename", use_context))]
    placements_filename: String,

    // object path
    #[validate(required(code = "form-option-error-required"))]
    pcb_kind: Option<PcbKindChoice>,

    #[derivative(Default(value = "1"))]
    pcb_instance: u16,

    #[derivative(Default(value = "1..=6"))]
    pcb_unit_range: RangeInclusive<u16>,
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

    VariantNameChanged(String),
    PcbUnitRangeChanged(RangeInclusive<u16>),
}

#[derive(Debug, Clone)]
pub enum CreateUnitAssignmentModalAction {
    Submit(CreateUnitAssignmentArgs),
    CloseDialog,
}

/// Value object
#[derive(Debug, Clone)]
pub struct CreateUnitAssignmentArgs {
    pub pcb_index: u16,
    pub variant_map: HashMap<VariantName, RangeInclusive<u16>>,
}

impl UiComponent for CreateUnitAssignmentModal {
    type UiContext<'context> = ();
    type UiCommand = CreateUnitAssignmentModalUiCommand;
    type UiAction = CreateUnitAssignmentModalAction;

    fn ui<'context>(&self, ui: &mut egui::Ui, _context: &mut Self::UiContext<'context>) {
        ui.ctx().style_mut(|style| {
            // if this is not done, text in labels/checkboxes/etc wraps
            style.wrap_mode = Some(egui::TextWrapMode::Extend);
        });

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
                        .add_enabled(form.is_valid(), egui::Button::new(tr!("form-button-ok")))
                        .clicked()
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

                // TODO
                let variant_map = HashMap::new();

                let args = CreateUnitAssignmentArgs {
                    pcb_index: self.pcb_index,
                    variant_map,
                };
                Some(CreateUnitAssignmentModalAction::Submit(args))
            }
            CreateUnitAssignmentModalUiCommand::VariantNameChanged(value) => {
                let mut fields = self.fields.lock().unwrap();
                fields.variant_name = value;
                fields.update_placements_filename();
                None
            }
            CreateUnitAssignmentModalUiCommand::PcbUnitRangeChanged(value) => {
                self.fields
                    .lock()
                    .unwrap()
                    .pcb_unit_range = value;
                None
            }
            CreateUnitAssignmentModalUiCommand::Cancel => Some(CreateUnitAssignmentModalAction::CloseDialog),
        }
    }
}
