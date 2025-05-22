use std::borrow::Cow;
use std::cmp::max;
use std::collections::BTreeMap;
use std::ops::RangeInclusive;
use std::path::PathBuf;

use eframe::epaint::{Color32, StrokeKind};
use egui::scroll_area::ScrollBarVisibility;
use egui::{Resize, TextEdit, Ui, WidgetText};
use egui_double_slider::DoubleSlider;
use egui_extras::{Column, TableBuilder};
use egui_i18n::tr;
use egui_mobius::Value;
use egui_taffy::taffy::prelude::{auto, length, percent, span};
use egui_taffy::taffy::{AlignContent, AlignItems, Display, FlexDirection, Size, Style};
use egui_taffy::{Tui, TuiBuilderLogic, tui};
use planner_app::{DesignIndex, DesignName, GerberPurpose, PcbOverview, PcbSide, PcbUnitIndex};
use tracing::{debug, trace};
use util::range_utils::{RangeIntoUsize, clamp_inclusive_range};
use validator::{Validate, ValidationError};

use crate::dialogs::manage_gerbers::{ManageGerbersModal, ManagerGerberModalAction, ManagerGerbersModalUiCommand};
use crate::forms::Form;
use crate::pcb::tabs::PcbTabContext;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Debug)]
pub struct ConfigurationUi {
    pcb_overview: Option<PcbOverview>,

    fields: Value<DesignAssignmentsFields>,
    initial_args: PcbUnitConfigurationArgs,

    manage_gerbers_modal: Option<ManageGerbersModal>,

    pub component: ComponentState<ConfigurationUiCommand>,
}

impl ConfigurationUi {
    const TABLE_DEBUG_MODE: bool = false;

    const TABLE_HEIGHT_MAX: f32 = 200.0;
    const TABLE_SCROLL_HEIGHT_MIN: f32 = 40.0;

    // IMPORTANT SYNC LAYOUT CHANGES WITH [`unit_assignments_tab.rs`]

    fn show_form(
        &self,
        ui: &mut Ui,
        form: &Form<DesignAssignmentsFields, ConfigurationUiCommand>,
        pcb_overview: &PcbOverview,
    ) {
        let default_style = || Style {
            padding: length(2.),
            gap: length(2.),
            ..Default::default()
        };

        let container_style = || Style {
            padding: length(0.),
            margin: length(0.),
            gap: length(5.),
            ..Default::default()
        };

        tui(
            ui,
            ui.id()
                .with("configure_pcb_form"),
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
                    form.add_field_ui("units", tr!("form-configure-pcb-input-units"), tui, {
                        move |ui: &mut Ui, fields, sender| {
                            let mut units = fields.units;
                            ui.add(egui::DragValue::new(&mut units).range(1..=u16::MAX));

                            if units != fields.units {
                                sender
                                    .send(ConfigurationUiCommand::UnitsChanged(units))
                                    .expect("sent");
                            }

                            ui.response()
                        }
                    });

                    form.add_section_tui(
                        "unit_map",
                        tr!("form-configure-pcb-group-unit-map"),
                        tui,
                        move |tui: &mut Tui| {
                            //
                            // design controls row
                            //

                            form.show_fields_vertical(tui, |form, tui| {
                                tui.style(Style {
                                    flex_grow: 1.0,
                                    display: Display::Flex,
                                    align_content: Some(AlignContent::Stretch),
                                    // FIXME This `span` is only required because the `field_error` call also uses `grid_column: span(2)`, without it the width is ~50% of the horizontal space.
                                    grid_column: span(2),
                                    ..container_style()
                                })
                                    .add(|tui| {
                                        tui.style(Style {
                                            flex_grow: 0.0,
                                            ..default_style()
                                        })
                                            .label(tr!("form-configure-pcb-input-design-name"));

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

                                                let mut design_name_clone = fields.design_name.clone();
                                                TextEdit::singleline(&mut design_name_clone)
                                                    .hint_text(tr!("form-configure-pcb-input-design-name-placeholder"))
                                                    .desired_width(ui.available_width())
                                                    .show(ui);

                                                if !fields
                                                    .design_name
                                                    .eq(&design_name_clone)
                                                {
                                                    sender
                                                        .send(ConfigurationUiCommand::DesignNameChanged(design_name_clone))
                                                        .expect("sent")
                                                }
                                            });

                                        let is_design_name_ok = matches!(form.field_validation_errors("design_name"), None);

                                        if tui
                                            .style(Style {
                                                flex_grow: 0.0,
                                                ..default_style()
                                            })
                                            .enabled_ui(is_design_name_ok)
                                            .button(|tui| tui.label(tr!("form-common-button-add")))
                                            .clicked()
                                        {
                                            self.component
                                                .send(ConfigurationUiCommand::AddDesignClicked);
                                        }
                                    });

                                form.field_error(tui, "design_name");
                            });

                            //
                            // available design variants
                            //

                            tui.style(Style {
                                flex_grow: 1.0,
                                size: Size {
                                    width: percent(1.0),
                                    height: auto(),
                                },
                                ..default_style()
                            })
                                .add(|tui: &mut Tui| {
                                    tui.ui_infinite(|ui: &mut Ui| {
                                        Resize::default()
                                            .resizable([false, true])
                                            .default_size(ui.available_size())
                                            .min_width(ui.available_width())
                                            .max_width(ui.available_width())
                                            .max_height(Self::TABLE_HEIGHT_MAX)
                                            .show(ui, |ui| {
                                                // HACK: search codebase for 'HACK: table-resize-hack' for details
                                                egui::Frame::new()
                                                    .outer_margin(4.0)
                                                    .show(ui, |ui| {
                                                        ui.style_mut().interaction.selectable_labels = false;

                                                        let fields = self.fields.lock().unwrap();

                                                        let text_height = egui::TextStyle::Body
                                                            .resolve(ui.style())
                                                            .size
                                                            .max(ui.spacing().interact_size.y);

                                                        let table_response = TableBuilder::new(ui)
                                                            .striped(true)
                                                            .resizable(true)
                                                            .auto_shrink([false, false])
                                                            .min_scrolled_height(Self::TABLE_SCROLL_HEIGHT_MIN)
                                                            .scroll_bar_visibility(ScrollBarVisibility::AlwaysVisible)
                                                            .sense(egui::Sense::click())
                                                            .column(Column::auto())
                                                            .column(Column::auto())
                                                            .column(Column::remainder())
                                                            .header(20.0, |mut header| {
                                                                header.col(|ui| {
                                                                    ui.strong(tr!("table-designs-column-index"));
                                                                });
                                                                header.col(|ui| {
                                                                    ui.strong(tr!("table-designs-column-actions"));
                                                                });
                                                                header.col(|ui| {
                                                                    ui.strong(tr!("table-designs-column-name"));
                                                                });
                                                            })
                                                            .body(|mut body| {
                                                                let mut designs_selected_index = fields.designs_selected_index;
                                                                for (
                                                                    row_index,
                                                                    design_name,
                                                                ) in fields
                                                                    .designs
                                                                    .iter()
                                                                    .enumerate()
                                                                {
                                                                    body.row(text_height, |mut row| {
                                                                        let is_selected = matches!(designs_selected_index, Some(selected_index) if selected_index == row_index);
                                                                        let design_exists = pcb_overview.designs.len() > row_index;

                                                                        row.set_selected(is_selected);

                                                                        row.col(|ui| {
                                                                            ui.label((row_index + 1).to_string());
                                                                        });

                                                                        row.col(|ui| {
                                                                            ui.add_enabled_ui(design_exists, |ui|{
                                                                                if ui
                                                                                    .button(tr!("form-configure-pcb-designs-button-gerbers"))
                                                                                    .clicked()
                                                                                {
                                                                                    self.component
                                                                                        .send(ConfigurationUiCommand::ManageGerbersClicked {
                                                                                            design_index: row_index,
                                                                                        });
                                                                                }
                                                                            });
                                                                        });

                                                                        row.col(|ui| {
                                                                            ui.label(design_name.to_string());
                                                                        });

                                                                        if row.response().clicked() {
                                                                            match is_selected {
                                                                                true => designs_selected_index = None,
                                                                                false => designs_selected_index = Some(row_index),
                                                                            }
                                                                        }
                                                                    });
                                                                }
                                                                if fields.designs_selected_index != designs_selected_index {
                                                                    self.component.send(ConfigurationUiCommand::DesignVariantSelectionChanged(designs_selected_index));
                                                                }
                                                            });
                                                        if Self::TABLE_DEBUG_MODE {
                                                            ui.painter().rect_stroke(table_response.inner_rect, 0.0, (1.0, Color32::CYAN), StrokeKind::Inside);
                                                            ui.painter().rect_stroke(ui.response().rect, 0.0, (1.0, Color32::ORANGE), StrokeKind::Inside);
                                                        }
                                                    });
                                            });
                                    });
                                });

                            //
                            // unit range
                            //

                            form.add_field_tui(
                                "pcb_unit_range",
                                tr!("form-configure-pcb-input-pcb-unit-range"),
                                tui,
                                {
                                    move |tui: &mut Tui, fields, sender| {
                                        let mut pcb_unit_start = fields.pcb_unit_range.start().clone();
                                        let mut pcb_unit_end = fields.pcb_unit_range.end().clone();
                                        let range = 1..=fields.units;
                                        // FIXME due to egui/double slider bugs, a range of x..=x causes a panic, so disable the range controls in this case
                                        let is_range_valid = range.end() > range.start();
                                        //trace!("pcb_unit_start: {}, pcb_unit_end: {}, range: {:?}", pcb_unit_start, pcb_unit_start, range);

                                        tui.style(Style {
                                            display: Display::Flex,
                                            align_content: Some(AlignContent::Stretch),
                                            flex_grow: 1.0,
                                            ..container_style()
                                        })
                                            .enabled_ui(is_range_valid)
                                            .add(|tui| {
                                                tui.style(Style {
                                                    flex_grow: 1.0,
                                                    ..default_style()
                                                })
                                                    .ui(|ui|{
                                                        // always 0 the first sizing pass
                                                        let available_width = ui.available_width();
                                                        let width = if ui.is_sizing_pass() {
                                                            200.0
                                                        } else {
                                                            available_width
                                                        };
                                                        // FIXME make the width auto-size
                                                        let double_slider = DoubleSlider::new(
                                                            &mut pcb_unit_start,
                                                            &mut pcb_unit_end,
                                                            range.clone(),
                                                        )
                                                            .separation_distance(0)
                                                            .width(width);

                                                        ui.add(double_slider);
                                                    }
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
                                                            .range(pcb_unit_start..=*(range.end()))
                                                    );
                                            });

                                        let pcb_unit_range = RangeInclusive::new(pcb_unit_start, pcb_unit_end);

                                        if fields.pcb_unit_range != pcb_unit_range {
                                            sender
                                                .send(ConfigurationUiCommand::PcbUnitRangeChanged(pcb_unit_range.clone()))
                                                .expect("sent")
                                        }

                                        let assignment_range_1_based = pcb_unit_range.to_usize_range();
                                        let assignment_range = (assignment_range_1_based.start() - 1)..=(assignment_range_1_based.end() - 1);

                                        let is_design_selected = fields.designs_selected_index.is_some();

                                        if tui
                                            .style(Style {
                                                flex_grow: 0.0,
                                                ..default_style()
                                            })
                                            .enabled_ui(is_design_selected)
                                            .button(|tui| tui.label(tr!("form-common-button-apply-range")))
                                            .clicked()
                                        {
                                            self.component
                                                .send(ConfigurationUiCommand::ApplyRangeClicked(
                                                    fields.designs_selected_index.unwrap(),
                                                ));
                                        }

                                        if tui
                                            .style(Style {
                                                flex_grow: 0.0,
                                                ..default_style()
                                            })
                                            .enabled_ui(is_design_selected)
                                            .button(|tui| tui.label(tr!("form-common-button-apply-all")))
                                            .clicked()
                                        {
                                            self.component
                                                .send(ConfigurationUiCommand::ApplyAllClicked(
                                                    fields.designs_selected_index.unwrap(),
                                                ));
                                        }

                                        if tui
                                            .style(Style {
                                                flex_grow: 0.0,
                                                ..default_style()
                                            })
                                            .enabled_ui(is_design_selected)
                                            .button(|tui| tui.label(tr!("form-common-button-unassign-from-range")))
                                            .clicked()
                                        {
                                            self.component
                                                .send(ConfigurationUiCommand::UnassignFromRange(
                                                    fields.designs_selected_index.unwrap(),
                                                ));
                                        }

                                        let have_assigned_items_in_range = fields.unit_map[assignment_range].iter().any(|assignment| assignment.is_some());

                                        if tui
                                            .style(Style {
                                                flex_grow: 0.0,
                                                ..default_style()
                                            })
                                            .enabled_ui(have_assigned_items_in_range)
                                            .button(|tui| tui.label(tr!("form-common-button-unassign-range")))
                                            .clicked()
                                        {
                                            self.component
                                                .send(ConfigurationUiCommand::UnassignRange);
                                        }
                                    }
                                },
                            );

                            //
                            // design assignments
                            //

                            tui.style(Style {
                                flex_grow: 1.0,
                                size: Size {
                                    width: percent(1.0),
                                    height: auto(),
                                },
                                ..container_style()
                            })
                                .add(|tui| {
                                    tui.ui_infinite(|ui: &mut Ui| {
                                        Resize::default()
                                            .resizable([false, true])
                                            .default_size(ui.available_size())
                                            .min_width(ui.available_width())
                                            .max_height(Self::TABLE_HEIGHT_MAX)
                                            .max_width(ui.available_width())
                                            .show(ui, |ui| {
                                                // HACK: search codebase for 'HACK: table-resize-hack' for details
                                                egui::Frame::new()
                                                    .outer_margin(4.0)
                                                    .show(ui, |ui| {
                                                        ui.style_mut().interaction.selectable_labels = false;

                                                        let mut fields = self.fields.lock().unwrap();

                                                        let text_height = egui::TextStyle::Body
                                                            .resolve(ui.style())
                                                            .size
                                                            .max(ui.spacing().interact_size.y);

                                                        let table_response = TableBuilder::new(ui)
                                                            .auto_shrink([false, false])
                                                            .scroll_bar_visibility(ScrollBarVisibility::AlwaysVisible)
                                                            .striped(true)
                                                            .resizable(true)
                                                            .min_scrolled_height(Self::TABLE_SCROLL_HEIGHT_MIN)
                                                            .sense(egui::Sense::click())
                                                            .column(Column::auto())
                                                            .column(Column::remainder())
                                                            .header(20.0, |mut header| {
                                                                header.col(|ui| {
                                                                    ui.strong(tr!("table-design-assignments-column-pcb-unit"));
                                                                });
                                                                header.col(|ui| {
                                                                    ui.strong(tr!("table-design-assignments-column-design"));
                                                                });
                                                            })
                                                            .body(|mut body| {
                                                                let mut unit_map_selected_indexes = fields.unit_map_selected_indexes.clone();

                                                                // use 'take' to ensure we only display the ones that should be visible
                                                                for (pcb_unit_index, design_index) in
                                                                    fields.unit_map.iter().take(fields.units as usize).enumerate()
                                                                {
                                                                    body.row(text_height, |mut row| {
                                                                        let is_selected = unit_map_selected_indexes.contains(&pcb_unit_index);
                                                                        row.set_selected(is_selected);

                                                                        row.col(|ui| {
                                                                            ui.label((pcb_unit_index + 1).to_string());
                                                                        });

                                                                        row.col(|ui| {
                                                                            let label = design_index
                                                                                .map(|design_index| fields.designs[design_index].to_string())
                                                                                .unwrap_or(tr!("assignment-unassigned"));
                                                                            ui.label(label);
                                                                        });

                                                                        if row.response().clicked() {
                                                                            match is_selected {
                                                                                true => {
                                                                                    unit_map_selected_indexes.retain(|&x| x != pcb_unit_index)
                                                                                }
                                                                                false => unit_map_selected_indexes.push(pcb_unit_index),
                                                                            }
                                                                        }
                                                                    });
                                                                }
                                                                fields.unit_map_selected_indexes = unit_map_selected_indexes;
                                                            });

                                                        if Self::TABLE_DEBUG_MODE {
                                                            ui.painter().rect_stroke(table_response.inner_rect, 0.0, (1.0, Color32::CYAN), StrokeKind::Inside);
                                                            ui.painter().rect_stroke(ui.response().rect, 0.0, (1.0, Color32::ORANGE), StrokeKind::Inside);
                                                        }
                                                    });
                                            });

                                        ui.response()
                                    });
                                });

                            // If the unit_map was grown, then the last item was selected; then the unit count was
                            // decreased, the map will still be the size after it was grown and the selection
                            // will still contain an entry that is not visible or usable for selection operations.
                            let unit_map_visible_selected_indexes = {
                                let fields = self.fields.lock().unwrap();

                                fields.unit_map_selected_indexes
                                    .iter()
                                    .take(fields.units as usize)
                                    .cloned()
                                    .collect::<Vec<_>>()
                            };

                            //
                            // button row
                            //

                            form.show_fields_vertical(tui, |_form, tui| {
                                tui.style(Style {
                                    flex_grow: 1.0,
                                    display: Display::Flex,
                                    align_content: Some(AlignContent::Stretch),
                                    flex_direction: FlexDirection::Row,
                                    ..container_style()
                                })
                                    .add(|tui| {
                                        tui.style(Style {
                                            flex_grow: 1.0,
                                            ..container_style()
                                        })
                                            .add(|tui| {
                                                let fields = self.fields.lock().unwrap();
                                                let is_selection_ok = !unit_map_visible_selected_indexes.is_empty();
                                                let is_design_name_ok = fields.designs_selected_index.is_some();
                                                if tui
                                                    .style(Style {
                                                        flex_grow: 1.0,
                                                        ..default_style()
                                                    })
                                                    .enabled_ui(is_selection_ok && is_design_name_ok)
                                                    .button(|tui| tui.label(tr!("form-common-button-assign-selected")))
                                                    .clicked()
                                                {
                                                    self.component
                                                        .send(ConfigurationUiCommand::AssignSelection(
                                                            fields.designs_selected_index.unwrap(),
                                                            unit_map_visible_selected_indexes.clone(),
                                                        ));
                                                }

                                                let have_assigned_selection = unit_map_visible_selected_indexes.iter().any(|index|{
                                                    let assignment = &fields.unit_map[*index];
                                                    assignment.is_some()
                                                });
                                                if tui
                                                    .style(Style {
                                                        flex_grow: 1.0,
                                                        ..default_style()
                                                    })
                                                    .enabled_ui(have_assigned_selection)
                                                    .button(|tui| tui.label(tr!("form-common-button-unassign-selected")))
                                                    .clicked()
                                                {
                                                    self.component
                                                        .send(ConfigurationUiCommand::UnassignSelection(
                                                            unit_map_visible_selected_indexes.clone(),
                                                        ));
                                                }

                                                let have_assigned_items = fields.unit_map.iter().any(|assignment| assignment.is_some());
                                                if tui
                                                    .style(Style {
                                                        flex_grow: 1.0,
                                                        ..default_style()
                                                    })
                                                    .enabled_ui(have_assigned_items)
                                                    .button(|tui| tui.label(tr!("form-common-button-unassign-all")))
                                                    .clicked()
                                                {
                                                    self.component
                                                        .send(ConfigurationUiCommand::UnassignAllClicked);
                                                }

                                            });
                                    });
                            });
                        },
                    );
                });
            });
    }

    fn show_manage_gerbers_modal(&mut self, design_index: usize) {
        let Some((design_name, design_gerbers)) = self
            .pcb_overview
            .as_ref()
            .map(|pcb_overview| {
                let design_name = pcb_overview.designs[design_index].clone();
                let gerbers = pcb_overview.gerbers[design_index].clone();
                (design_name, gerbers)
            })
        else {
            return;
        };

        let mut modal = ManageGerbersModal::new(design_index, design_name.to_string(), design_gerbers);
        modal
            .component
            .configure_mapper(self.component.sender.clone(), move |command| {
                trace!("manage gerbers modal mapper. command: {:?}", command);
                ConfigurationUiCommand::ManageGerbersModalUiCommand(command)
            });

        self.manage_gerbers_modal = Some(modal);
    }
}

impl ConfigurationUi {
    pub fn new() -> Self {
        Self {
            pcb_overview: None,
            manage_gerbers_modal: None,
            component: Default::default(),
            fields: Value::default(),
            initial_args: Default::default(),
        }
    }

    pub fn update_pcb_overview(&mut self, pcb_overview: PcbOverview) {
        if let Some(modal) = &mut self.manage_gerbers_modal {
            modal.update_gerbers(&pcb_overview.gerbers)
        }

        let mut fields = self.fields.lock().unwrap();

        fields.update_units(pcb_overview.units);
        fields.designs = pcb_overview.designs.clone();

        // preserve the existing elements of the unit map if the new map has a smaller unit range (e.g. existing vec = 0..=10, new map = 0..=5)
        fields.unit_map = (0..max(fields.unit_map.len() as u16, pcb_overview.units))
            .map(|pcb_unit_index| {
                pcb_overview
                    .unit_map
                    .get(&pcb_unit_index)
                    .cloned()
            })
            .collect::<Vec<_>>();

        self.initial_args = fields.as_args();

        self.pcb_overview.replace(pcb_overview);
    }
}

type UnitMap = Vec<Option<DesignIndex>>;

/// The run-time values will be updated from the [`PcbOverview`] view.
#[derive(Clone, Debug, Validate, serde::Deserialize, serde::Serialize)]
#[validate(context = DesignAssignmentsValidationContext)]
pub struct DesignAssignmentsFields {
    /// the number of individual units the PCB has
    #[validate(range(min = 1, max = u16::MAX, code = "form-input-error-range"))]
    units: u16,

    /// allows the user to type in a design name, the current value is used when making assignments or
    /// sizing the unit_map
    #[validate(length(min = 1, code = "form-input-error-length"))]
    design_name: String,

    pcb_unit_range: RangeInclusive<u16>,

    designs: Vec<DesignName>,

    /// index of the vec is the pcb unit index (0-based)
    #[validate(custom(function = "DesignAssignmentsFields::validate_unit_map", use_context))]
    unit_map: UnitMap,

    designs_selected_index: Option<usize>,
    unit_map_selected_indexes: Vec<usize>,
}

impl Default for DesignAssignmentsFields {
    fn default() -> Self {
        const MINIMUM_UNITS: u16 = 1;

        Self {
            units: MINIMUM_UNITS,
            designs: vec![],
            // See [`Self::grow_unit_map`]
            unit_map: vec![],
            design_name: "".to_string(),

            pcb_unit_range: 1..=1,
            designs_selected_index: None,
            unit_map_selected_indexes: vec![],
        }
    }
}

pub struct DesignAssignmentsValidationContext {
    units: u16,
}

impl DesignAssignmentsFields {
    fn as_args(&self) -> PcbUnitConfigurationArgs {
        // convert from the ui-friendly vector to a space-friendly map
        let unit_map = self
            .unit_map
            .iter()
            .take(self.units as usize)
            .enumerate()
            .filter_map(|(pcb_unit_index, assignment)| {
                assignment.map(|design_index| (pcb_unit_index as PcbUnitIndex, design_index))
            })
            .collect::<BTreeMap<PcbUnitIndex, DesignIndex>>();

        PcbUnitConfigurationArgs {
            units: self.units,
            designs: self.designs.clone(),
            unit_map,
        }
    }

    fn validate_unit_map(
        unit_map: &UnitMap,
        context: &DesignAssignmentsValidationContext,
    ) -> Result<(), ValidationError> {
        // code elsewhere should grow the vector to the amount of units, but never shorten it
        // we take the elements we need when it is submitted
        let len = unit_map.len();
        if len < context.units as usize {
            let mut error = ValidationError::new("form-input-error-map-incorrect-entry-count");
            error.add_param(Cow::from("required"), &context.units);
            error.add_param(Cow::from("actual"), &len);

            return Err(error);
        }

        if Self::is_unit_map_fully_populated_inner(unit_map, context.units as usize) {
            return Err(ValidationError::new("form-input-error-map-unassigned-entries"));
        }

        Ok(())
    }

    fn is_unit_map_fully_populated_inner(unit_map: &UnitMap, units: usize) -> bool {
        unit_map
            .iter()
            .take(units)
            .any(|design| design.is_none())
    }

    /// Grow the unit map but never shrink it.  If we shrink it, we will lose the mappings already
    /// created if the user changes the number of units.
    fn grow_unit_map(&mut self, new_size: usize) {
        if self.unit_map.len() < new_size {
            self.unit_map.resize(new_size, None);
        }
    }

    fn update_units(&mut self, units: u16) {
        let old_limits = 1..=self.units;
        let new_limits = 1..=units;

        let old_range = self.pcb_unit_range.clone();

        self.pcb_unit_range = clamp_inclusive_range(&old_limits, &new_limits, &self.pcb_unit_range);
        debug!(
            "clamped pcb unit range. old_limits: {:?}, new_limits: {:?}, old_range: {:?}, result: {:?}",
            old_limits, new_limits, old_range, self.pcb_unit_range
        );

        self.units = units;

        self.grow_unit_map(units as usize);
    }
}

#[derive(Debug, Clone)]
pub enum ConfigurationUiCommand {
    None,
    ManageGerbersClicked {
        design_index: usize,
    },
    ManageGerbersModalUiCommand(ManagerGerbersModalUiCommand),

    UnitsChanged(u16),
    DesignNameChanged(String),

    PcbUnitRangeChanged(RangeInclusive<u16>),

    /// apply the selected design, specified by the index, and the pcb unit range to the unit map
    ApplyRangeClicked(usize),
    UnassignFromRange(usize),
    ApplyAllClicked(usize),

    UnassignAllClicked,

    UnassignSelection(Vec<usize>),
    AssignSelection(usize, Vec<usize>),
    DesignVariantSelectionChanged(Option<usize>),

    AddDesignClicked,
    UnassignRange,
    Reset,
    Apply,
}

#[derive(Debug, Clone)]
pub enum ConfigurationUiAction {
    None,
    AddGerberFiles {
        path: PathBuf,
        design: DesignName,
        files: Vec<(PathBuf, Option<PcbSide>, GerberPurpose)>,
    },
    RemoveGerberFiles {
        path: PathBuf,
        design: DesignName,
        files: Vec<PathBuf>,
    },
    Reset,
    Apply(PcbUnitConfigurationArgs),
}

#[derive(Debug, Clone, Default)]
pub struct ConfigurationUiContext {}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PcbUnitConfigurationArgs {
    pub units: u16,
    pub designs: Vec<DesignName>,
    pub unit_map: BTreeMap<PcbUnitIndex, DesignIndex>,
}

impl UiComponent for ConfigurationUi {
    type UiContext<'context> = ConfigurationUiContext;
    type UiCommand = ConfigurationUiCommand;
    type UiAction = ConfigurationUiAction;

    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        ui.ctx().style_mut(|style| {
            // if this is not done, text in labels/checkboxes/etc wraps
            style.wrap_mode = Some(egui::TextWrapMode::Extend);
        });

        ui.label(tr!("pcb-configuration-header"));

        let Some(pcb_overview) = &self.pcb_overview else {
            ui.spinner();
            return;
        };

        ui.label(tr!("pcb-configuration-detail-name", { name: &pcb_overview.name }));

        ui.separator();

        //
        // form
        //

        let validation_context = DesignAssignmentsValidationContext {
            units: self.fields.lock().unwrap().units,
        };

        let form = Form::new(&self.fields, &self.component.sender, &validation_context);

        self.show_form(ui, &form, pcb_overview);

        let is_changed = self.fields.lock().unwrap().as_args() != self.initial_args;

        egui::Sides::new().show(
            ui,
            |ui| {
                if ui
                    .add_enabled(is_changed, egui::Button::new(tr!("form-button-reset")))
                    .clicked()
                {
                    self.component
                        .send(ConfigurationUiCommand::Reset);
                }

                if ui
                    .add_enabled(
                        is_changed && form.is_valid(),
                        egui::Button::new(tr!("form-button-apply")),
                    )
                    .clicked()
                {
                    self.component
                        .send(ConfigurationUiCommand::Apply);
                }
            },
            |_ui| {},
        );

        //
        // Modals
        //
        if let Some(dialog) = &self.manage_gerbers_modal {
            dialog.ui(ui, &mut ());
        }
    }

    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            ConfigurationUiCommand::None => Some(ConfigurationUiAction::None),

            //
            // fields
            //
            ConfigurationUiCommand::UnitsChanged(units) => {
                let mut fields = self.fields.lock().unwrap();

                fields.update_units(units);

                None
            }
            ConfigurationUiCommand::DesignNameChanged(name) => {
                self.fields.lock().unwrap().design_name = name;
                None
            }

            ConfigurationUiCommand::PcbUnitRangeChanged(value) => {
                self.fields
                    .lock()
                    .unwrap()
                    .pcb_unit_range = value;
                None
            }

            //
            // design assignments
            //
            ConfigurationUiCommand::DesignVariantSelectionChanged(designs_selected_index) => {
                let mut fields = self.fields.lock().unwrap();
                fields.designs_selected_index = designs_selected_index;

                if let Some(designs_selected_index) = designs_selected_index {
                    // update the design name field with the name from the selected entry
                    let design_name = &fields.designs[designs_selected_index].clone();

                    fields.design_name = design_name.to_string();
                }
                None
            }

            ConfigurationUiCommand::ApplyRangeClicked(design_index) => {
                let mut fields = self.fields.lock().unwrap();
                let assignment_range_1_based = fields.pcb_unit_range.to_usize_range();
                let assignment_range = (assignment_range_1_based.start() - 1)..=(assignment_range_1_based.end() - 1);

                for assigned_design_index in &mut fields.unit_map[assignment_range] {
                    *assigned_design_index = Some(design_index)
                }
                None
            }
            ConfigurationUiCommand::UnassignFromRange(design_index) => {
                let mut fields = self.fields.lock().unwrap();
                let assignment_range_1_based = fields.pcb_unit_range.to_usize_range();
                let assignment_range = (assignment_range_1_based.start() - 1)..=(assignment_range_1_based.end() - 1);

                for assigned_design_index in fields
                    .unit_map
                    [assignment_range]
                    .iter_mut()
                    .filter(|assignment|matches!(assignment, Some(assigned_design_index) if *assigned_design_index == design_index))
                {
                    *assigned_design_index = None
                }
                None
            }
            ConfigurationUiCommand::UnassignRange => {
                let mut fields = self.fields.lock().unwrap();
                let assignment_range_1_based = fields.pcb_unit_range.to_usize_range();
                let assignment_range = (assignment_range_1_based.start() - 1)..=(assignment_range_1_based.end() - 1);

                for assigned_design_index in fields.unit_map[assignment_range].iter_mut() {
                    *assigned_design_index = None
                }
                None
            }
            ConfigurationUiCommand::ApplyAllClicked(design_index) => {
                let mut fields = self.fields.lock().unwrap();

                for assigned_design_index in fields.unit_map.iter_mut() {
                    *assigned_design_index = Some(design_index)
                }
                None
            }
            ConfigurationUiCommand::UnassignAllClicked => {
                let mut fields = self.fields.lock().unwrap();

                for assigned_design_index in fields.unit_map.iter_mut() {
                    *assigned_design_index = None
                }
                None
            }
            ConfigurationUiCommand::UnassignSelection(unit_map_selected_indexes) => {
                let mut fields = self.fields.lock().unwrap();

                for (_pcb_unit_index, assigned_design_index) in fields
                    .unit_map
                    .iter_mut()
                    .enumerate()
                    .filter(|(pcb_unit_index, _)| unit_map_selected_indexes.contains(pcb_unit_index))
                {
                    *assigned_design_index = None
                }
                None
            }
            ConfigurationUiCommand::AssignSelection(design_index, unit_map_selected_indexes) => {
                let mut fields = self.fields.lock().unwrap();

                for (_pcb_unit_index, assigned_design_index) in fields
                    .unit_map
                    .iter_mut()
                    .enumerate()
                    .filter(|(pcb_unit_index, _)| unit_map_selected_indexes.contains(pcb_unit_index))
                {
                    *assigned_design_index = Some(design_index)
                }
                None
            }
            ConfigurationUiCommand::AddDesignClicked => {
                let mut fields = self.fields.lock().unwrap();

                let design_name = DesignName::from(fields.design_name.clone().trim());
                if !fields.designs.contains(&design_name) {
                    fields.designs.push(design_name)
                }
                None
            }

            //
            // form submission
            //
            ConfigurationUiCommand::Apply => {
                let fields = self.fields.lock().unwrap();

                let args = fields.as_args();

                Some(ConfigurationUiAction::Apply(args))
            }
            ConfigurationUiCommand::Reset => Some(ConfigurationUiAction::Reset),

            //
            // gerber management
            //
            ConfigurationUiCommand::ManageGerbersClicked {
                design_index,
            } => {
                self.show_manage_gerbers_modal(design_index);
                None
            }
            ConfigurationUiCommand::ManageGerbersModalUiCommand(command) => {
                if let Some(modal) = &mut self.manage_gerbers_modal {
                    match modal.update(command, &mut ()) {
                        None => None,
                        Some(ManagerGerberModalAction::CloseDialog) => {
                            self.manage_gerbers_modal = None;
                            None
                        }
                        Some(ManagerGerberModalAction::RemoveGerberFiles {
                            design_index,
                            files,
                        }) => {
                            debug!(
                                "removing gerber file. design_index: {}, files: {:?}",
                                design_index, files
                            );
                            if let Some(pcb_overview) = &mut self.pcb_overview {
                                let design = pcb_overview.designs[design_index].clone();
                                Some(ConfigurationUiAction::RemoveGerberFiles {
                                    path: pcb_overview.path.clone(),
                                    design,
                                    files,
                                })
                            } else {
                                None
                            }
                        }
                        Some(ManagerGerberModalAction::AddGerberFiles {
                            design_index,
                            files,
                        }) => {
                            debug!(
                                "gerber files picked. design_index: {}, picked: {:?}",
                                design_index, files
                            );
                            if let Some(pcb_overview) = &mut self.pcb_overview {
                                let design = pcb_overview.designs[design_index].clone();
                                let files = files
                                    .into_iter()
                                    .map(|file| (file, None, GerberPurpose::Other))
                                    .collect();
                                Some(ConfigurationUiAction::AddGerberFiles {
                                    path: pcb_overview.path.clone(),
                                    design,
                                    files,
                                })
                            } else {
                                None
                            }
                        }
                    }
                } else {
                    None
                }
            }
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Default, PartialEq)]
pub struct ConfigurationTab {}

impl Tab for ConfigurationTab {
    type Context = PcbTabContext;

    fn label(&self) -> WidgetText {
        egui::widget_text::WidgetText::from(tr!("pcb-configuration-tab-label"))
    }

    fn ui<'a>(&mut self, ui: &mut Ui, _tab_key: &TabKey, context: &mut Self::Context) {
        let state = context.state.lock().unwrap();
        UiComponent::ui(&state.configuration_ui, ui, &mut ConfigurationUiContext::default());
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, _context: &mut Self::Context) -> bool {
        true
    }
}
