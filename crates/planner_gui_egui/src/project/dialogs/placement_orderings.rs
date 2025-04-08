use std::collections::BTreeMap;
use std::ops::Index;

use egui::{Id, Modal, Ui, WidgetText};
use egui_i18n::tr;
use egui_mobius::Value;
use egui_taffy::taffy::prelude::{auto, length, percent};
use egui_taffy::taffy::{AlignContent, AlignItems, Display, FlexDirection, Size, Style};
use egui_taffy::{Tui, TuiBuilderLogic, tui};
use planner_app::{PlacementSortingItem, PlacementSortingMode, Reference};
use tracing::debug;
use util::sorting::SortOrder;
use validator::Validate;

use crate::forms::Form;
use crate::forms::transforms::no_transform;
use crate::i18n::conversions::{placement_place_to_i18n_key, placement_sorting_mode_to_i18n_key};
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
                                let all_items = BTreeMap::from([
                                    (
                                        PlacementSortingMode::PcbUnit,
                                        tr!(placement_sorting_mode_to_i18n_key(&PlacementSortingMode::PcbUnit)),
                                    ),
                                    (
                                        PlacementSortingMode::FeederReference,
                                        tr!(placement_sorting_mode_to_i18n_key(
                                            &PlacementSortingMode::FeederReference
                                        )),
                                    ),
                                    (
                                        PlacementSortingMode::RefDes,
                                        tr!(placement_sorting_mode_to_i18n_key(&PlacementSortingMode::RefDes)),
                                    ),
                                ]);

                                fn selected_item_mapper(
                                    (k, v): (&PlacementSortingMode, &SortOrder),
                                ) -> (PlacementSortingMode, (String, SortOrder)) {
                                    let label =
                                        format!("{} - {:?}", tr!(placement_sorting_mode_to_i18n_key(k)), v).to_string();
                                    (k.clone(), (label, v.clone()))
                                }

                                let mut selected_items: BTreeMap<PlacementSortingMode, (String, SortOrder)> = fields
                                    .ordering
                                    .iter()
                                    .map(selected_item_mapper)
                                    .collect();

                                let available_items: BTreeMap<PlacementSortingMode, String> = all_items
                                    .iter()
                                    .filter_map(|(k, v)| {
                                        if !selected_items.contains_key(k) {
                                            Some((k.clone(), v.clone()))
                                        } else {
                                            None
                                        }
                                    })
                                    .collect();

                                tui.style(Style {
                                    display: Display::Flex,
                                    align_content: Some(AlignContent::Stretch),
                                    flex_grow: 1.0,
                                    ..default_style()
                                })
                                .add(|tui| {
                                    let id = tui.current_id();
                                    let available_item_id = id.with("available_item_index");
                                    let selected_item_id = id.with("selected_item_index");

                                    for column_index in 0..3 {
                                        match column_index {
                                            0 => Self::left_column(
                                                tui,
                                                default_style,
                                                available_item_id,
                                                &available_items,
                                            ),
                                            1 => Self::center_column(
                                                tui,
                                                default_style,
                                                available_item_id,
                                                selected_item_id,
                                                &available_items,
                                                &selected_items,
                                                SortOrder::Asc,
                                                Self::sort_order_buttons,
                                                |k, v| {
                                                    debug!("add. k: {:?}, v: {:?}", k, v);
                                                },
                                                |k, v| {
                                                    debug!("remove. k: {:?}, v: {:?}", k, v);
                                                },
                                            ),
                                            2 => Self::right_column(
                                                tui,
                                                default_style,
                                                selected_item_id,
                                                &selected_items,
                                            ),
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

    fn sort_order_buttons(ui: &mut Ui, v: SortOrder) -> Option<SortOrder> {
        let mut result = None;

        // TODO translations
        if ui
            .add(egui::RadioButton::new(v == SortOrder::Asc, "Ascending"))
            .clicked()
        {
            result = Some(SortOrder::Asc);
        }
        if ui
            .add(egui::RadioButton::new(v == SortOrder::Desc, "Descending"))
            .clicked()
        {
            result = Some(SortOrder::Desc);
        }

        result
    }

    fn left_column<T>(tui: &mut Tui, rename_this_style: fn() -> Style, id: Id, available_items: &BTreeMap<T, String>) {
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
            let names = available_items
                .iter()
                .map(|(k, v)| v)
                .collect::<Vec<_>>();

            list_box_with_id_tui(tui, id, names);
            // end of cell
        });
    }

    fn right_column<K, V>(
        tui: &mut Tui,
        rename_this_style: fn() -> Style,
        id: Id,
        selected_items: &BTreeMap<K, (String, V)>,
    ) {
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
            let items = selected_items
                .iter()
                .map(|(_k, (s, _v))| s)
                .collect::<Vec<_>>();
            list_box_with_id_tui(tui, id, items);
            // end of cell
        })
    }

    fn center_column<K: Ord + Clone, V: Clone + Send + Sync + 'static, F>(
        tui: &mut Tui,
        rename_this_style: fn() -> Style,
        available_item_id: Id,
        selected_item_id: Id,
        available_items: &BTreeMap<K, String>,
        selected_items: &BTreeMap<K, (String, V)>,
        default_v: V,
        v_selector: F,
        added_fn: fn(&K, &V),
        removed_fn: fn(&K, &V),
    ) where
        F: Fn(&mut Ui, V) -> Option<V>,
    {
        let id = tui.current_id().with("v");

        let mut v = tui.egui_ui().memory(|mem| {
            // NOTE It's CRITICAL that the correct type is specified for `get_temp`
            mem.data
                .get_temp::<V>(id)
                .unwrap_or(default_v)
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
                        if let Some(new_v) = v_selector(ui, v.clone()) {
                            v = new_v;
                        }
                    });

                    ui.response()
                },
                no_transform,
            );

            let available_index = tui.egui_ui().memory(|mem| {
                // NOTE It's CRITICAL that the correct type is specified for `get_temp`
                mem.data
                    .get_temp::<Option<usize>>(available_item_id)
                    .unwrap_or_default()
            });

            let selected_index = tui.egui_ui().memory(|mem| {
                // NOTE It's CRITICAL that the correct type is specified for `get_temp`
                mem.data
                    .get_temp::<Option<usize>>(selected_item_id)
                    .unwrap_or_default()
            });

            if tui
                .enabled_ui(available_index.is_some())
                .ui_add(egui::Button::new(">"))
                .clicked()
            {
                let (k, s) = available_items
                    .iter()
                    .nth(available_index.unwrap())
                    .unwrap();

                debug!(">, {:?}", available_index);
                added_fn(k, &v);
            };
            if tui
                .enabled_ui(selected_index.is_some())
                .ui_add(egui::Button::new("<"))
                .clicked()
            {
                let (k, (s, v)) = selected_items
                    .iter()
                    .nth(available_index.unwrap())
                    .unwrap();
                debug!("<, {:?}", selected_index);
                removed_fn(k, v);
            };

            // end of cell
        });

        tui.egui_ui()
            .memory_mut(|mem| mem.data.insert_temp(id, v));
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
pub struct PlacementOrderingFields {
    ordering: BTreeMap<PlacementSortingMode, SortOrder>,
}

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
