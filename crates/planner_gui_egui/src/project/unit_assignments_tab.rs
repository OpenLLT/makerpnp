use std::cmp::max;
use std::collections::HashMap;
use std::ops::RangeInclusive;
use std::path::PathBuf;
use std::str::FromStr;

use derivative::Derivative;
use egui::{Response, TextEdit, Ui, Vec2, WidgetText};
use egui_double_slider::DoubleSlider;
use egui_extras::{Column, TableBuilder};
use egui_i18n::tr;
use egui_mobius::Value;
use egui_mobius::types::ValueGuard;
use egui_taffy::taffy::prelude::{auto, length, percent, span};
use egui_taffy::taffy::{AlignContent, AlignItems, AlignSelf, Display, FlexDirection, Size, Style};
use egui_taffy::{Tui, TuiBuilderLogic, TuiContainerResponse, tui};
use planner_app::{DesignIndex, DesignName, DesignVariant, PcbOverview, PcbUnitAssignments, PcbUnitIndex, VariantName};
use tracing::debug;
use validator::{Validate, ValidationError};

use crate::forms::Form;
use crate::forms::transforms::no_transform;
use crate::project::tabs::ProjectTabContext;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};

// TODO when changing the design variant selection, update the combo and text boxes with the details from the selection.
// TODO Add an 'unassign selected' button.
// TODO make the *content* of the table rows non-selectable. (the rows should still be selectable)
// TODO remove form debugging borders

#[derive(Derivative)]
#[derivative(Debug)]
pub struct UnitAssignmentsUi {
    path: PathBuf,
    placements_directory: PathBuf,
    pcb_overview: Option<PcbOverview>,
    pcb_unit_assignments: Option<PcbUnitAssignments>,

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
            pcb_unit_assignments: None,
            fields: Default::default(),
            component: Default::default(),
        }
    }

    pub fn update_overview(&mut self, pcb_overview: PcbOverview) {
        // block to limit the scope of the borrow
        {
            let mut fields = self.fields.lock().unwrap();

            fields.pcb_unit_range = 1..=pcb_overview.units;
        }

        self.pcb_overview = Some(pcb_overview);
        self.update_map();
    }

    pub fn update_unit_assignments(&mut self, pcb_unit_assignments: PcbUnitAssignments) {
        self.pcb_unit_assignments = Some(pcb_unit_assignments);
        self.update_map();
    }

    /// we need both the pcb_overview and the pcb_unit_assignments, but the methods that provide them
    /// could be called in any order, so they both need to call this
    fn update_map(&mut self) {
        let (Some(pcb_overview), Some(pcb_unit_assignments)) = (&self.pcb_overview, &self.pcb_unit_assignments) else {
            return;
        };

        let mut fields = self.fields.lock().unwrap();

        fields.variant_map = (0..pcb_overview.units)
            .map(|pcb_unit_index| {
                (
                    pcb_overview
                        .unit_map
                        .get(&pcb_unit_index)
                        .cloned(),
                    pcb_unit_assignments
                        .unit_assignments
                        .get(&pcb_unit_index)
                        .cloned(),
                )
            })
            .collect::<Vec<_>>();
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
                                // FIXME This `span` is only required because the `field_error` call also uses `grid_column: span(2)`, without it the width is ~50% of the horizontal space.
                                grid_column: span(2),
                                ..default_style()
                            })
                            .add_with_border(|tui| {
                                tui.style(Style {
                                    flex_grow: 0.0,
                                    ..default_style()
                                })
                                .label(tr!("form-create-unit-assignment-input-design-name"));

                                tui.style(Style {
                                    flex_grow: 0.4,
                                    min_size: Size {
                                        width: length(100.0),
                                        height: auto(),
                                    },
                                    ..default_style()
                                })
                                .ui(|ui| {
                                    let fields = self.fields.lock().unwrap();
                                    let sender = self.component.sender.clone();

                                    let design_name = fields.design_name.as_ref();

                                    egui::ComboBox::from_id_salt(ui.id().with("design_name"))
                                        .width(ui.available_width())
                                        .selected_text(match design_name {
                                            None => tr!("form-common-combo-select"),
                                            Some(design_name) => design_name.to_string(),
                                        })
                                        .show_ui(ui, |ui| {
                                            for available_design_name in &pcb_overview.designs {
                                                if ui
                                                    .add(egui::SelectableLabel::new(
                                                        matches!(design_name.as_ref(), Some(design_name) if design_name.eq(&available_design_name)),
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
                                    flex_grow: 0.0,
                                    ..default_style()
                                })
                                .label(tr!("form-create-unit-assignment-input-variant-name"));

                                tui.style(Style {
                                    flex_grow: 0.6,
                                    min_size: Size {
                                        width: length(100.0),
                                        height: auto(),
                                    },
                                    ..default_style()
                                })
                                .ui(|ui| {
                                    let fields = self.fields.lock().unwrap();
                                    let sender = self.component.sender.clone();

                                    let mut variant_name_clone = fields.variant_name.clone();
                                    TextEdit::singleline(&mut variant_name_clone)
                                        .hint_text("e.g. 'Production'") // TODO translate
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

                                let is_design_variant_ok = {
                                    let fields = self.fields.lock().unwrap();
                                    // enable the button if the design is Some and the `placements_filename` field is ok
                                    matches!((&fields.design_name, form.field_validation_errors("placements_filename")), (Some(_), None))
                                };
                                if tui
                                    .style(Style {
                                        flex_grow: 0.0,
                                        ..default_style()
                                    })
                                    .enabled_ui(is_design_variant_ok)
                                    .button(|tui| tui.label("Add"))
                                    .clicked()
                                {
                                    self.component
                                        .send(UnitAssignmentsUiCommand::AddDesignVariantClicked);
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
                            let mut fields = self.fields.lock().unwrap();

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
                                    let mut design_variant_selected_index = fields.design_variant_selected_index;
                                    for (
                                        row_index,
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
                                            let is_selected = matches!(design_variant_selected_index, Some(selected_index) if selected_index == row_index);

                                            row.set_selected(is_selected);

                                            row.col(|ui| {
                                                ui.label(design_name.to_string());
                                            });

                                            row.col(|ui| {
                                                ui.label(variant_name.to_string());
                                            });

                                            if row.response().clicked() {
                                                match is_selected {
                                                    true => design_variant_selected_index = None,
                                                    false => design_variant_selected_index = Some(row_index),
                                                }
                                            }
                                        });
                                    }
                                    fields.design_variant_selected_index = design_variant_selected_index;
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

                                    pub fn transform_fail(mut value: TuiContainerResponse<Response>, _ui: &Ui) -> TuiContainerResponse<Response> {
                                        // The code below has no effect, the control resized to fill available space, but never shrinks.
                                        // value.min_size.x = 200.0;
                                        // value.intrinsic_size = Some(Vec2::new(value.min_size.x, value.min_size.y));
                                        // value.infinite = egui::Vec2b { x: true, y: false };
                                        value
                                    }

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
                                                // always 0 the first sizing pass
                                                let available_width = ui.available_width();
                                                let width = if available_width == 0.0 {
                                                    200.0
                                                } else {
                                                    available_width
                                                };
                                                // FIXME make the width auto-size
                                                let double_slider = DoubleSlider::new(
                                                    &mut pcb_unit_start,
                                                    &mut pcb_unit_end,
                                                    1..=pcb_overview.units,
                                                )
                                                    .separation_distance(0)
                                                    .width(width);
                                                    //.width(200.0);

                                                ui.add(double_slider)
                                            },
                                            transform_fail,
                                        );

                                        tui.style(Style {
                                            flex_grow: 0.0,
                                            min_size: Size {
                                                width: length(50.0),
                                                height: auto(),
                                            },
                                            ..default_style()
                                        })
                                        .ui_add(
                                            egui::DragValue::new(&mut pcb_unit_start).range(1..=pcb_unit_end)
                                        );

                                        tui.style(Style {
                                            flex_grow: 0.0,
                                            min_size: Size {
                                                width: length(50.0),
                                                height: auto(),
                                            },
                                            ..default_style()
                                        })
                                        .ui_add(
                                            egui::DragValue::new(&mut pcb_unit_end)
                                                .range(pcb_unit_start..=pcb_overview.units)
                                        );
                                    });

                                    let pcb_unit_range = RangeInclusive::new(pcb_unit_start, pcb_unit_end);

                                    if fields.pcb_unit_range != pcb_unit_range {
                                        sender
                                            .send(UnitAssignmentsUiCommand::PcbUnitRangeChanged(pcb_unit_range))
                                            .expect("sent")
                                    }

                                    let is_design_selected = fields.design_variant_selected_index.is_some();

                                    if tui
                                        .style(Style {
                                            flex_grow: 0.0,
                                            ..default_style()
                                        })
                                        .enabled_ui(is_design_selected)
                                        .button(|tui| tui.label("Apply range"))
                                        .clicked()
                                    {
                                        self.component
                                            .send(UnitAssignmentsUiCommand::ApplyRangeClicked(
                                                fields.design_variant_selected_index.unwrap(),
                                            ));
                                    }

                                    if tui
                                        .style(Style {
                                            flex_grow: 0.0,
                                            ..default_style()
                                        })
                                        .enabled_ui(is_design_selected)
                                        .button(|tui| tui.label("Apply all"))
                                        .clicked()
                                    {
                                        self.component
                                            .send(UnitAssignmentsUiCommand::ApplyAllClicked(
                                                fields.design_variant_selected_index.unwrap(),
                                            ));
                                    }

                                    if tui
                                        .style(Style {
                                            flex_grow: 0.0,
                                            ..default_style()
                                        })
                                        .enabled_ui(is_design_selected)
                                        .button(|tui| tui.label("Unassign range"))
                                        .clicked()
                                    {
                                        self.component
                                            .send(UnitAssignmentsUiCommand::UnassignRange(
                                                fields.design_variant_selected_index.unwrap(),
                                            ));
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
                            let mut fields = self.fields.lock().unwrap();

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
                                    let mut variant_map_selected_indexes = fields.variant_map_selected_indexes.clone();
                                    for (pcb_unit_index, (design_index, assigned_variant_name)) in
                                        fields.variant_map.iter().enumerate()
                                    {
                                        body.row(text_height, |mut row| {
                                            let is_selected = variant_map_selected_indexes.contains(&pcb_unit_index);
                                            row.set_selected(is_selected);

                                            row.col(|ui| {
                                                ui.label((pcb_unit_index + 1).to_string());
                                            });

                                            row.col(|ui| {
                                                let label = design_index
                                                    .map(|design_index|pcb_overview.designs[design_index].to_string())
                                                    .unwrap_or("<unassigned>".to_string()); // TODO translate
                                                ui.label(label);
                                            });

                                            row.col(|ui| {
                                                let label = assigned_variant_name
                                                    .as_deref()
                                                    .unwrap_or("<unassigned>"); // TODO translate
                                                ui.label(label);
                                            });

                                            if row.response().clicked() {
                                                match is_selected {
                                                    true => {
                                                        variant_map_selected_indexes.retain(|&x| x != pcb_unit_index)
                                                    }
                                                    false => variant_map_selected_indexes.push(pcb_unit_index),
                                                }
                                            }
                                        });
                                    }
                                    fields.variant_map_selected_indexes = variant_map_selected_indexes;
                                });
                        });

                        //
                        // button row
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
                                .add(|tui| {
                                    if tui
                                        .style(Style {
                                            flex_grow: 1.0,
                                            ..default_style()
                                        })
                                        .button(|tui| tui.label("Unassign all"))
                                        .clicked()
                                    {
                                        self.component
                                            .send(UnitAssignmentsUiCommand::UnassignAllClicked);
                                    }

                                    if tui
                                        .style(Style {
                                            flex_grow: 1.0,
                                            ..default_style()
                                        })
                                        .button(|tui| tui.label("Unassign selected"))
                                        .clicked()
                                    {
                                        self.component
                                            .send(UnitAssignmentsUiCommand::UnassignSelection(
                                                self.fields.lock().unwrap().variant_map_selected_indexes.clone(),
                                            ));
                                    }
                                });
                            });
                        });
                    },
                );
            });
        });
    }

    fn apply_variant_map(
        fields: ValueGuard<UnitAssignmentsFields>,
        pcb_index: PcbUnitIndex,
    ) -> Option<UnitAssignmentsUiAction> {
        let variant_map = fields
            .variant_map
            .iter()
            .filter(|(_, variant_name)| variant_name.is_some())
            .map(|(_design_index, variant_name)| variant_name.clone().unwrap())
            .collect::<Vec<_>>();

        let args = UpdateUnitAssignmentsArgs {
            pcb_index,
            variant_map,
        };

        debug!("update unit assignments. args: {:?}", args);
        Some(UnitAssignmentsUiAction::UpdateUnitAssignments(args))
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

    // TODO make this Vec<(DesignIndex, DesignVariant)> to avoid having to look-up the design index again
    design_variants: Vec<DesignVariant>,

    /// index of the vec is the pcb unit index (0-based)
    variant_map: Vec<(Option<DesignIndex>, Option<VariantName>)>,

    design_variant_selected_index: Option<usize>,
    variant_map_selected_indexes: Vec<usize>,
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
            design_variant_selected_index: None,
            variant_map_selected_indexes: vec![],
        }
    }
}

pub struct UnitAssignmentsValidationContext {
    placements_directory: PathBuf,
}

impl UnitAssignmentsFields {
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
    pub variant_map: Vec<VariantName>,
}

#[derive(Debug, Clone)]
pub enum UnitAssignmentsUiCommand {
    None,

    DesignNameChanged(DesignName),
    VariantNameChanged(String),

    AddDesignVariantClicked,

    PcbUnitRangeChanged(RangeInclusive<u16>),

    /// apply the selected design variant, specified by the index, and the pcb unit range to the variant map
    ApplyRangeClicked(usize),
    UnassignRange(usize),
    ApplyAllClicked(usize),

    UnassignAllClicked,

    UnassignSelection(Vec<usize>),
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
            UnitAssignmentsUiCommand::AddDesignVariantClicked => {
                let mut fields = self.fields.lock().unwrap();
                let variant_name = VariantName::from_str(&fields.variant_name).unwrap();

                if let Some(design_name) = fields.design_name.clone() {
                    fields
                        .design_variants
                        .push(DesignVariant {
                            design_name,
                            variant_name,
                        });

                    // de-duplicate in case the user pressed the button multiple times.
                    fields.design_variants.dedup();
                }

                None
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

            UnitAssignmentsUiCommand::ApplyRangeClicked(design_variant_index) => {
                if let Some(pcb_overview) = &self.pcb_overview {
                    let mut fields = self.fields.lock().unwrap();
                    let pcb_unit_range = fields.pcb_unit_range.clone();
                    let design_variant = fields.design_variants[design_variant_index].clone();
                    let design_index = pcb_overview
                        .designs
                        .iter()
                        .position(|design_name| design_name.eq(&design_variant.design_name))
                        .map(|index| index as DesignIndex);

                    for (_pcb_unit_index, (_design_index, assigned_variant_name)) in fields
                        .variant_map
                        .iter_mut()
                        .enumerate()
                        .filter(|(pcb_unit_index, _)| {
                            pcb_unit_range.contains(&(*pcb_unit_index as u16 + 1))
                        })
                        .filter(|(pcb_unit_index, (candidate_design_index, _))| {
                            matches!((candidate_design_index, design_index), (Some(cdi), Some(di)) if *cdi == di)
                        })
                    {
                        *assigned_variant_name = Some(design_variant.variant_name.clone());
                    }
                    Self::apply_variant_map(fields, pcb_overview.index)
                } else {
                    None
                }
            }

            UnitAssignmentsUiCommand::UnassignRange(design_variant_index) => {
                if let Some(pcb_overview) = &self.pcb_overview {
                    let mut fields = self.fields.lock().unwrap();
                    let pcb_unit_range = fields.pcb_unit_range.clone();
                    let design_variant = fields.design_variants[design_variant_index].clone();
                    let design_index = pcb_overview
                        .designs
                        .iter()
                        .position(|design_name| design_name.eq(&design_variant.design_name));

                    for (_pcb_unit_index, (_design_index, assigned_variant_name)) in fields
                        .variant_map
                        .iter_mut()
                        .enumerate()
                        .filter(|(pcb_unit_index, _)| {
                            pcb_unit_range.contains(&(*pcb_unit_index as u16 + 1))
                        })
                        .filter(|(pcb_unit_index, (candidate_design_index, _))| {
                            matches!((candidate_design_index, design_index), (Some(cdi), Some(di)) if *cdi == di)
                        })                    {
                        *assigned_variant_name = None;
                    }
                    Self::apply_variant_map(fields, pcb_overview.index)
                } else {
                    None
                }
            }
            UnitAssignmentsUiCommand::ApplyAllClicked(design_variant_index) => {
                if let Some(pcb_overview) = &self.pcb_overview {
                    let mut fields = self.fields.lock().unwrap();
                    let design_variant = fields.design_variants[design_variant_index].clone();
                    let design_index = pcb_overview
                        .designs
                        .iter()
                        .position(|design_name| design_name.eq(&design_variant.design_name));

                    for (_design_index, assigned_variant_name) in
                        fields
                            .variant_map
                            .iter_mut()
                            .filter(|(candidate_design_index, b)| {
                                matches!((candidate_design_index, design_index), (Some(cdi), Some(di)) if *cdi == di)
                            },
                            )
                    {
                        *assigned_variant_name = Some(design_variant.variant_name.clone());
                    }
                    Self::apply_variant_map(fields, pcb_overview.index)
                } else {
                    None
                }
            }
            UnitAssignmentsUiCommand::UnassignAllClicked => {
                if let Some(pcb_overview) = &self.pcb_overview {
                    let mut fields = self.fields.lock().unwrap();
                    for (_design_index, assigned_variant_name) in fields.variant_map.iter_mut() {
                        *assigned_variant_name = None;
                    }

                    Self::apply_variant_map(fields, pcb_overview.index)
                } else {
                    None
                }
            }
            UnitAssignmentsUiCommand::UnassignSelection(variant_map_selected_indexes) => {
                if let Some(pcb_overview) = &self.pcb_overview {
                    let mut fields = self.fields.lock().unwrap();
                    for (_index, (_design_index, assigned_variant_name)) in fields
                        .variant_map
                        .iter_mut()
                        .enumerate()
                        .filter(|(index, _)| variant_map_selected_indexes.contains(index))
                    {
                        *assigned_variant_name = None;
                    }

                    Self::apply_variant_map(fields, pcb_overview.index)
                } else {
                    None
                }
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
