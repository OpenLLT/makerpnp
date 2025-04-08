use egui::{Id, Modal, Ui, WidgetText};
use egui_i18n::tr;
use egui_mobius::Value;
use egui_taffy::taffy::prelude::{auto, length, percent};
use egui_taffy::taffy::{AlignContent, AlignItems, Display, FlexDirection, Size, Style};
use egui_taffy::{Tui, TuiBuilderLogic, tui};
use planner_app::{PlacementSortingItem, Reference};
use util::sorting::SortOrder;
use validator::Validate;

use crate::forms::Form;
use crate::forms::transforms::no_transform;
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Debug)]
pub struct PlacementOrderingsModal {
    phase_reference: Reference,

    fields: Value<PlacementOrderingFields>,

    pub component: ComponentState<PlacementOrderingsModalUiCommand>,
}

impl PlacementOrderingsModal {
    pub fn new(phase_reference: Reference) -> Self {
        Self {
            phase_reference,
            fields: Value::default(),
            component: ComponentState::default(),
        }
    }

    fn show_form(&self, ui: &mut Ui, form: &Form<PlacementOrderingFields, PlacementOrderingsModalUiCommand>) {
        let default_style = || Style {
            padding: length(2.),
            gap: length(2.),
            ..Default::default()
        };

        tui(ui, ui.id().with("placement_orderings_form"))
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
                    form.add_field_tui(
                        "orderings",
                        tr!("form-phase-placement-orderings-input-orderings"),
                        tui,
                        {
                            move |tui: &mut Tui, fields, sender| {
                                tui.style(Style {
                                    display: Display::Flex,
                                    align_content: Some(AlignContent::Stretch),
                                    flex_grow: 1.0,
                                    ..default_style()
                                })
                                .add(|tui| {
                                    for column_index in 0..3 {
                                        match column_index {
                                            0 => Self::left_column(tui, default_style),
                                            1 => Self::center_column(tui, default_style),
                                            2 => Self::right_column(tui, default_style),
                                            _ => unreachable!(),
                                        }
                                        // end of column
                                    }
                                    // end of row
                                });

                                // end of field
                            }
                        },
                    );
                    // end of fields
                });
                // end of form
            });
    }

    fn right_column(tui: &mut Tui, rename_this_style: fn() -> Style) {
        tui.style(Style {
            flex_grow: 1.0,
            min_size: Size {
                width: percent(0.4),
                height: auto(),
            },
            ..rename_this_style()
        })
        .with_border_style_from_egui_style()
        .add_with_border(|tui: &mut Tui| {
            let id = tui.current_id().with("selected");
            list_box_with_id_tui(tui, id, vec!["4", "5", "6"]);
            // end of cell
        })
    }

    fn center_column(tui: &mut Tui, rename_this_style: fn() -> Style) {
        let id = tui.current_id().with("sort_order");

        let mut sort_order = tui.egui_ui().memory(|mem| {
            // NOTE It's CRITICAL that the correct type is specified for `get_temp`
            mem.data
                .get_temp::<SortOrder>(id)
                .unwrap_or(SortOrder::Asc)
        });

        tui.style(Style {
            flex_grow: 1.0,
            flex_direction: FlexDirection::Column,
            min_size: Size {
                width: percent(0.2),
                height: auto(),
            },
            ..rename_this_style()
        })
        .add(|tui: &mut Tui| {
            tui.ui_add_manual(
                |ui| {
                    ui.vertical_centered(|ui| {
                        // TODO translations
                        if ui
                            .add(egui::RadioButton::new(sort_order == SortOrder::Asc, "Ascending"))
                            .clicked()
                        {
                            sort_order = SortOrder::Asc;
                        }
                        if ui
                            .add(egui::RadioButton::new(sort_order == SortOrder::Desc, "Descending"))
                            .clicked()
                        {
                            sort_order = SortOrder::Desc;
                        }
                    });

                    ui.response()
                },
                no_transform,
            );
            tui.ui_add(egui::Button::new(">"));
            tui.ui_add(egui::Button::new("<"));

            // end of cell
        });

        tui.egui_ui()
            .memory_mut(|mem| mem.data.insert_temp(id, sort_order));
    }

    fn left_column(tui: &mut Tui, rename_this_style: fn() -> Style) {
        tui.style(Style {
            flex_grow: 1.0,
            min_size: Size {
                width: percent(0.4),
                height: auto(),
            },
            ..rename_this_style()
        })
        .with_border_style_from_egui_style()
        .add_with_border(|tui: &mut Tui| {
            let id = tui.current_id().with("available");
            list_box_with_id_tui(tui, id, vec!["1", "2", "3"]);
            // end of cell
        })
    }
}

pub fn list_box_with_id_tui<I, S>(tui: &mut Tui, id: Id, items: I) -> Option<usize>
where
    I: IntoIterator<Item = S>,
    S: Into<WidgetText>,
{
    let mut selected_index = tui.egui_ui().memory(|mem| {
        // NOTE It's CRITICAL that the correct type is specified for `get_temp`
        mem.data
            .get_temp::<Option<usize>>(id)
            .unwrap_or_default()
    });

    tui.style(Style {
        flex_direction: FlexDirection::Column,
        align_items: Some(AlignItems::Stretch),
        flex_grow: 1.0,
        ..Default::default()
    })
    .add(|tui| {
        for (index, item) in items.into_iter().enumerate() {
            let is_selected = selected_index == Some(index);

            let response = tui
                .style(Style {
                    ..Default::default()
                })
                .selectable(is_selected, |tui| {
                    tui.label(item);
                });

            if response.clicked() {
                selected_index = Some(index);
                tui.egui_ui()
                    .memory_mut(|mem| mem.data.insert_temp(id, selected_index));
            }
        }
    });

    // Return the current selection
    selected_index
}

#[derive(Clone, Debug, Default, Validate, serde::Deserialize, serde::Serialize)]
pub struct PlacementOrderingFields {}

#[derive(Debug, Clone)]
pub enum PlacementOrderingsModalUiCommand {
    Submit,
    Cancel,
}

#[derive(Debug, Clone)]
pub enum PlacementOrderingsModalAction {
    Submit(PlacementOrderingsArgs),
    CloseDialog,
}

/// Value object
#[derive(Debug, Clone)]
pub struct PlacementOrderingsArgs {
    pub orderings: Vec<PlacementSortingItem>,
}

impl UiComponent for PlacementOrderingsModal {
    type UiContext<'context> = ();
    type UiCommand = PlacementOrderingsModalUiCommand;
    type UiAction = PlacementOrderingsModalAction;

    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        ui.ctx().style_mut(|style| {
            // if this is not done, text in labels/checkboxes/etc wraps
            style.wrap_mode = Some(egui::TextWrapMode::Extend);
        });

        let modal_id = ui
            .id()
            .with("phase_placement_orderings_modal");

        Modal::new(modal_id).show(ui.ctx(), |ui| {
            ui.set_min_width(400.0);
            ui.set_max_width(ui.ctx().screen_rect().width() * 0.5);

            ui.heading(tr!("modal-phase-placement-orderings-title", { phase: self.phase_reference.to_string() }));

            let form = Form::new(&self.fields, &self.component.sender, ());

            self.show_form(ui, &form);

            let is_form_valid = form.is_valid();

            egui::Sides::new().show(
                ui,
                |_ui| {},
                |ui| {
                    if ui
                        .button(tr!("form-button-cancel"))
                        .clicked()
                    {
                        self.component
                            .send(PlacementOrderingsModalUiCommand::Cancel);
                    }
                    if ui
                        .button(tr!("form-button-ok"))
                        .clicked()
                        && is_form_valid
                    {
                        self.component
                            .send(PlacementOrderingsModalUiCommand::Cancel);
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
            PlacementOrderingsModalUiCommand::Submit => {
                let args = PlacementOrderingsArgs {
                    // TODO build the orderings from the UI
                    orderings: vec![],
                };
                Some(PlacementOrderingsModalAction::Submit(args))
            }
            PlacementOrderingsModalUiCommand::Cancel => Some(PlacementOrderingsModalAction::CloseDialog),
        }
    }
}
