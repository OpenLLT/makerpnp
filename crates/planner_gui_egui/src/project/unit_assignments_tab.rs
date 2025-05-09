use std::borrow::Cow;
use std::ops::RangeInclusive;
use std::path::PathBuf;

use derivative::Derivative;
use egui::{TextEdit, Ui, WidgetText};
use egui_double_slider::DoubleSlider;
use egui_extras::{Column, TableBuilder};
use egui_i18n::tr;
use egui_mobius::Value;
use egui_taffy::taffy::prelude::{auto, length, percent};
use egui_taffy::taffy::{AlignContent, AlignItems, Display, FlexDirection, Size, Style};
use egui_taffy::{Tui, TuiBuilderLogic, tui};
use planner_app::{DesignIndex, DesignName, DesignVariant, PcbOverview, PcbUnitIndex, VariantName};
use tracing::debug;
use validator::{Validate, ValidationError};

use crate::forms::Form;
use crate::forms::transforms::no_transform;
use crate::project::dialogs::PcbSideChoice;
use crate::project::dialogs::add_pcb::AddPcbValidationContext;
use crate::project::dialogs::add_phase::AddPhaseModalUiCommand;
use crate::project::dialogs::create_unit_assignment::CreateUnitAssignmentModalUiCommand;
use crate::project::tabs::ProjectTabContext;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct UnitAssignmentsUi {
    path: PathBuf,
    placements_directory: PathBuf,
    pcb_overview: Option<PcbOverview>,

    fields: Value<UnitAssignmentsFields>,

    pub component: ComponentState<UnitAssignmentsUiCommand>,
}

impl UnitAssignmentsUi {
    pub fn new(path: PathBuf) -> Self {
        let placements_directory = path
            .clone()
            .parent()
            .unwrap()
            .to_path_buf();

        Self {
            path,
            placements_directory,
            pcb_overview: None,
            fields: Default::default(),
            component: Default::default(),
        }
    }

    pub fn update_overview(&mut self, pcb_overview: PcbOverview) {
        let mut fields = self.fields.lock().unwrap();

        // TODO populate the map from the ACTUAL map, but it's not in the pcb_overview.

        fields.variant_map = (0..pcb_overview.units)
            .map(|pcb_unit_index| (0, None))
            .collect::<Vec<_>>();

        fields.pcb_unit_range = 1..=pcb_overview.units;

        self.pcb_overview = Some(pcb_overview);
    }

    fn show_form(
        &self,
        ui: &mut Ui,
        form: &Form<UnitAssignmentsFields, UnitAssignmentsUiCommand>,
        pcb_overview: &PcbOverview,
    ) {
        let default_style = || Style {
            padding: length(2.),
            gap: length(2.),
            ..Default::default()
        };

        tui(
            ui,
            ui.id()
                .with("create_unit_assignment_form"),
        )
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

                form.add_section_tui(
                    "variant_map",
                    "Variant Map".to_string(), // TODO translate
                    tui,
                    move |tui: &mut Tui| {
                        //
                        // variant controls row
                        //

                        form.show_fields_vertical(tui, |form, tui| {
                            tui.style(Style {
                                flex_grow: 1.0,
                                display: Display::Flex,
                                align_content: Some(AlignContent::Stretch),
                                flex_direction: FlexDirection::Row,
                                ..default_style()
                            })
                            .add_with_border(|tui| {
                                tui.style(Style {
                                    flex_grow: 1.0,
                                    ..default_style()
                                })
                                .label(tr!("form-create-unit-assignment-input-design-name"));

                                tui.style(Style {
                                    flex_grow: 1.0,
                                    ..default_style()
                                })
                                .ui(|ui| {
                                    let fields = self.fields.lock().unwrap();
                                    let sender = self.component.sender.clone();

                                    let design_name = fields.design_name.as_ref();

                                    egui::ComboBox::from_id_salt(ui.id().with("design_name"))
                                        // TODO do we need a row here?
                                        .width(ui.available_width())
                                        .selected_text(match design_name {
                                            None => tr!("form-common-combo-select"),
                                            Some(design_name) => design_name.to_string(),
                                        })
                                        .show_ui(ui, |ui| {
                                            for available_design_name in &pcb_overview.designs {
                                                if ui
                                                    .add(egui::SelectableLabel::new(
                                                        matches!(design_name.as_ref(), Some(available_design_name)),
                                                        available_design_name.to_string(),
                                                    ))
                                                    .clicked()
                                                {
                                                    sender
                                                        .send(UnitAssignmentsUiCommand::DesignNameChanged(
                                                            available_design_name.clone(),
                                                        ))
                                                        .expect("sent");
                                                }
                                            }
                                        })
                                        .response
                                });

                                tui.style(Style {
                                    flex_grow: 1.0,
                                    ..default_style()
                                })
                                .label(tr!("form-create-unit-assignment-input-variant-name"));

                                tui.style(Style {
                                    flex_grow: 1.0,
                                    ..default_style()
                                })
                                .ui(|ui| {
                                    let fields = self.fields.lock().unwrap();
                                    let sender = self.component.sender.clone();

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
                                            .send(UnitAssignmentsUiCommand::VariantNameChanged(variant_name_clone))
                                            .expect("sent")
                                    }
                                });

                                if tui
                                    .style(Style {
                                        flex_grow: 1.0,
                                        ..default_style()
                                    })
                                    .button(|tui| tui.label("Add"))
                                    .clicked()
                                {
                                    self.component
                                        .send(UnitAssignmentsUiCommand::ApplyRangeClicked);
                                }
                            });

                            form.field_error(tui, "variant_name");
                        });

                        form.show_fields_vertical(tui, |form, tui| {
                            form.add_field_ui(
                                "placements_filename",
                                tr!("form-create-unit-assignment-input-placements-filename"),
                                tui,
                                move |ui: &mut Ui, fields, _sender| {
                                    let label = fields
                                        .placements_filename
                                        .as_deref()
                                        .unwrap_or("<enter variant name>"); // TODO translate
                                    ui.label(label)
                                },
                            );
                        });

                        //
                        // available design variants
                        //

                        tui.style(Style {
                            flex_grow: 1.0,
                            ..default_style()
                        })
                        .ui(|ui: &mut Ui| {
                            let fields = self.fields.lock().unwrap();

                            let text_height = egui::TextStyle::Body
                                .resolve(ui.style())
                                .size
                                .max(ui.spacing().interact_size.y);

                            TableBuilder::new(ui)
                                .striped(true)
                                .resizable(true)
                                .sense(egui::Sense::click())
                                .column(Column::auto())
                                .column(Column::remainder())
                                .header(20.0, |mut header| {
                                    header.col(|ui| {
                                        ui.strong("Design Name"); // TODO translate
                                    });
                                    header.col(|ui| {
                                        ui.strong("Variant Name"); // TODO translate
                                    });
                                })
                                .body(|mut body| {
                                    for (
                                        index,
                                        DesignVariant {
                                            design_name,
                                            variant_name,
                                        },
                                    ) in fields
                                        .design_variants
                                        .iter()
                                        .enumerate()
                                    {
                                        body.row(text_height, |mut row| {
                                            // TODO use selection
                                            // row.set_selected(self.selection.contains(&row_index));

                                            row.col(|ui| {
                                                ui.label(design_name.to_string());
                                            });

                                            row.col(|ui| {
                                                ui.label(variant_name.to_string());
                                            });

                                            if row.response().clicked() {
                                                // TODO /CHANGE/ to selection (single select)
                                            }
                                        });
                                    }
                                });
                        });

                        //
                        // unit range
                        //

                        form.add_field_tui(
                            "pcb_unit_range",
                            tr!("form-create-unit-assignment-input-pcb-unit-range"),
                            tui,
                            {
                                move |tui: &mut Tui, fields, sender| {
                                    let mut pcb_unit_start = fields.pcb_unit_range.start().clone();
                                    let mut pcb_unit_end = fields.pcb_unit_range.end().clone();

                                    let enabled = true;

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
                                                            1..=pcb_overview.units,
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
                                                egui::DragValue::new(&mut pcb_unit_end)
                                                    .range(pcb_unit_start..=pcb_overview.units),
                                            );
                                        });
                                    });

                                    let pcb_unit_range = RangeInclusive::new(pcb_unit_start, pcb_unit_end);

                                    if fields.pcb_unit_range != pcb_unit_range {
                                        sender
                                            .send(UnitAssignmentsUiCommand::PcbUnitRangeChanged(pcb_unit_range))
                                            .expect("sent")
                                    }
                                }
                            },
                        );

                        //
                        // design variant assignments
                        //

                        tui.style(Style {
                            flex_grow: 1.0,
                            ..default_style()
                        })
                        .ui(|ui: &mut Ui| {
                            let fields = self.fields.lock().unwrap();

                            let text_height = egui::TextStyle::Body
                                .resolve(ui.style())
                                .size
                                .max(ui.spacing().interact_size.y);

                            TableBuilder::new(ui)
                                .striped(true)
                                .resizable(true)
                                .sense(egui::Sense::click())
                                .column(Column::auto())
                                .column(Column::auto())
                                .column(Column::remainder())
                                .column(Column::auto())
                                .header(20.0, |mut header| {
                                    header.col(|ui| {
                                        ui.strong("PCB Unit"); // TODO translate
                                    });
                                    header.col(|ui| {
                                        ui.strong("Design Name"); // TODO translate
                                    });
                                    header.col(|ui| {
                                        ui.strong("Variant Name"); // TODO translate
                                    });
                                })
                                .body(|mut body| {
                                    for (pcb_unit_index, (design_index, assigned_variant_name)) in
                                        fields.variant_map.iter().enumerate()
                                    {
                                        body.row(text_height, |mut row| {
                                            // TODO use selection
                                            // row.set_selected(self.selection.contains(&row_index));

                                            row.col(|ui| {
                                                ui.label(pcb_unit_index.to_string());
                                            });

                                            row.col(|ui| {
                                                let label = &pcb_overview.designs[*design_index as usize].to_string();
                                                ui.label(label);
                                            });

                                            row.col(|ui| {
                                                let label = assigned_variant_name
                                                    .as_deref()
                                                    .unwrap_or("<unassigned>"); // TODO translate
                                                ui.label(label);
                                            });

                                            if row.response().clicked() {
                                                // TODO add to selection (multi-select)
                                            }
                                        });
                                    }
                                });
                        });

                        if tui
                            .button(|tui| tui.label("Apply"))
                            .clicked()
                        {
                            self.component
                                .send(UnitAssignmentsUiCommand::ApplyUnitAssignmentsClicked);
                        }
                    },
                );
            });
        });
    }
}

#[derive(Clone, Debug, Validate, serde::Deserialize, serde::Serialize)]
#[validate(context = UnitAssignmentsValidationContext)]
pub struct UnitAssignmentsFields {
    #[validate(length(min = 1, code = "form-input-error-length"))]
    variant_name: String,

    #[validate(custom(function = "UnitAssignmentsFields::validate_placements_filename", use_context))]
    placements_filename: Option<String>,

    pcb_unit_range: RangeInclusive<u16>,

    /// drop-down box of all designs used by the PCB
    design_name: Option<DesignName>,

    design_variants: Vec<DesignVariant>,

    /// index of the vec is the pcb unit index (0-based)
    #[validate(custom(function = "UnitAssignmentsFields::validate_variant_map"))]
    variant_map: Vec<(DesignIndex, Option<VariantName>)>,
}

impl Default for UnitAssignmentsFields {
    fn default() -> Self {
        Self {
            variant_name: "".to_string(),
            placements_filename: None,
            pcb_unit_range: 1..=1,
            design_name: None,
            design_variants: Vec::new(),
            variant_map: Vec::new(),
        }
    }
}

pub struct UnitAssignmentsValidationContext {
    placements_directory: PathBuf,
}

impl UnitAssignmentsFields {
    fn validate_variant_map(variant_map: &Vec<(DesignIndex, Option<VariantName>)>) -> Result<(), ValidationError> {
        if Self::is_variant_map_fully_populated_inner(variant_map) {
            return Err(ValidationError::new("form-input-error-map-unassigned-entries"));
        }

        Ok(())
    }

    pub fn is_variant_map_fully_populated_inner(variant_map: &Vec<(DesignIndex, Option<VariantName>)>) -> bool {
        variant_map
            .iter()
            .any(|(_design_index, assigned_variant)| assigned_variant.is_none())
    }

    fn update_placements_filename(&mut self) {
        self.placements_filename = self
            .design_name
            .as_ref()
            .map(|design_name| format!("{}_{}_placements.csv", design_name, self.variant_name).to_string());
    }

    fn validate_placements_filename(
        placements_filename: &String,
        context: &UnitAssignmentsValidationContext,
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

/// Value object
#[derive(Debug, Clone)]
pub struct UpdateUnitAssignmentsArgs {
    pub pcb_index: u16,
    /// vector index = pcb unit index
    /// value = (design index, variant name)
    pub variant_map: Vec<VariantName>,
}

#[derive(Debug, Clone)]
pub enum UnitAssignmentsUiCommand {
    None,
    ApplyUnitAssignmentsClicked,
    VariantNameChanged(String),
    DesignNameChanged(DesignName),
    PcbUnitRangeChanged(RangeInclusive<u16>),
    ApplyRangeClicked,
}

#[derive(Debug, Clone)]
pub enum UnitAssignmentsUiAction {
    None,
    UpdateUnitAssignments(UpdateUnitAssignmentsArgs),
}

#[derive(Debug, Clone, Default)]
pub struct UnitAssignmentsUiContext {}

impl UiComponent for UnitAssignmentsUi {
    type UiContext<'context> = UnitAssignmentsUiContext;
    type UiCommand = UnitAssignmentsUiCommand;
    type UiAction = UnitAssignmentsUiAction;

    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        ui.label(tr!("project-unit-assignments-header"));
        let Some(pcb_overview) = &self.pcb_overview else {
            ui.spinner();
            return;
        };

        let validation_context = UnitAssignmentsValidationContext {
            placements_directory: self.placements_directory.clone(),
        };

        let form = Form::new(&self.fields, &self.component.sender, &validation_context);

        self.show_form(ui, &form, pcb_overview);
    }

    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            UnitAssignmentsUiCommand::None => Some(UnitAssignmentsUiAction::None),

            UnitAssignmentsUiCommand::ApplyRangeClicked => None,
            // TODO remove the apply button, combine with ApplyRangeClicked
            UnitAssignmentsUiCommand::ApplyUnitAssignmentsClicked => {
                let fields = self.fields.lock().unwrap();

                let variant_map = fields
                    .variant_map
                    .iter()
                    .filter(|(_, variant_name)| variant_name.is_some())
                    .map(|(_design_index, variant_name)| variant_name.clone().unwrap())
                    .collect::<Vec<_>>();

                let args = UpdateUnitAssignmentsArgs {
                    pcb_index: 0,
                    variant_map,
                };

                debug!("update unit assignments. args: {:?}", args);
                Some(UnitAssignmentsUiAction::UpdateUnitAssignments(args))
            }
            UnitAssignmentsUiCommand::VariantNameChanged(value) => {
                let mut fields = self.fields.lock().unwrap();
                fields.variant_name = value;
                fields.update_placements_filename();
                None
            }
            UnitAssignmentsUiCommand::PcbUnitRangeChanged(value) => {
                self.fields
                    .lock()
                    .unwrap()
                    .pcb_unit_range = value;
                None
            }

            UnitAssignmentsUiCommand::DesignNameChanged(design_name) => {
                let mut fields = self.fields.lock().unwrap();
                fields.design_name = Some(design_name);
                fields.update_placements_filename();
                None
            }
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq)]
pub struct UnitAssignmentsTab {
    pcb_index: u16,
}

impl UnitAssignmentsTab {
    pub fn new(pcb_index: u16) -> Self {
        Self {
            pcb_index,
        }
    }
}

impl Tab for UnitAssignmentsTab {
    type Context = ProjectTabContext;

    fn label(&self) -> WidgetText {
        let pcb = format!("{}", self.pcb_index).to_string();
        egui::widget_text::WidgetText::from(tr!("project-unit-assignments-tab-label", {pcb: pcb}))
    }

    fn ui<'a>(&mut self, ui: &mut Ui, _tab_key: &TabKey, context: &mut Self::Context) {
        let state = context.state.lock().unwrap();
        let unit_assignments_ui = state
            .unit_assignments
            .get(&(self.pcb_index as usize))
            .unwrap();
        UiComponent::ui(unit_assignments_ui, ui, &mut UnitAssignmentsUiContext::default());
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, context: &mut Self::Context) -> bool {
        let mut state = context.state.lock().unwrap();
        if let Some(_unit_assignments_ui) = state
            .unit_assignments
            .remove(&(self.pcb_index as usize))
        {
            debug!("removed orphaned unit assignments ui. pcb_index: {}", self.pcb_index);
        }
        true
    }
}
