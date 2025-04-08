use std::hash::Hash;

use egui::{Id, Ui};
use egui_taffy::taffy::prelude::{auto, percent};
use egui_taffy::taffy::{AlignContent, Display, FlexDirection, Size, Style};
use egui_taffy::{Tui, TuiBuilderLogic};
use indexmap::IndexMap;

use crate::forms::transforms::no_transform;
use crate::widgets::list_box::list_box_with_id_tui;

pub struct AugmentedListSelector {}

impl AugmentedListSelector {
    pub fn show<K: Hash + Eq + Clone, V: Clone + Send + Sync + 'static, MF, VF, AF, RF>(
        tui: &mut Tui,
        rename_this_style: fn() -> Style,
        selected_items: &IndexMap<K, V>,
        all_items: &IndexMap<K, String>,
        mapper_fn: MF,
        default_v: V,
        v_selector: VF,
        added_fn: AF,
        removed_fn: RF,
    ) where
        MF: Fn((&K, &V)) -> (K, (String, V)),
        VF: Fn(&mut Ui, V) -> Option<V>,
        AF: Fn(&K, &V),
        RF: Fn(&K, &V),
    {
        let selected_items: IndexMap<K, (String, V)> = selected_items
            .iter()
            .map(mapper_fn)
            .collect();

        let available_items: IndexMap<K, String> = all_items
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
            ..Default::default()
        })
        .add(|tui| {
            let id = tui.current_id();
            let available_item_id = id.with("available_item_index");
            let selected_item_id = id.with("selected_item_index");

            Self::left_column(tui, rename_this_style, available_item_id, &available_items);
            Self::center_column(
                tui,
                rename_this_style,
                available_item_id,
                selected_item_id,
                &available_items,
                &selected_items,
                default_v,
                v_selector,
                added_fn,
                removed_fn,
            );
            Self::right_column(tui, rename_this_style, selected_item_id, &selected_items);
            // end of row
        });
    }

    fn left_column<T>(tui: &mut Tui, rename_this_style: fn() -> Style, id: Id, available_items: &IndexMap<T, String>) {
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
                .map(|(_k, v)| v)
                .collect::<Vec<_>>();

            list_box_with_id_tui(tui, id, names);
            // end of cell
        });
    }

    fn right_column<K, V>(
        tui: &mut Tui,
        rename_this_style: fn() -> Style,
        id: Id,
        selected_items: &IndexMap<K, (String, V)>,
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

    fn center_column<K: Clone, V: Clone + Send + Sync + 'static, F, AF, RF>(
        tui: &mut Tui,
        rename_this_style: fn() -> Style,
        available_item_id: Id,
        selected_item_id: Id,
        available_items: &IndexMap<K, String>,
        selected_items: &IndexMap<K, (String, V)>,
        default_v: V,
        v_selector: F,
        added_fn: AF,
        removed_fn: RF,
    ) where
        F: Fn(&mut Ui, V) -> Option<V>,
        AF: Fn(&K, &V),
        RF: Fn(&K, &V),
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

            let mut available_index = tui.egui_ui().memory(|mem| {
                // NOTE It's CRITICAL that the correct type is specified for `get_temp`
                mem.data
                    .get_temp::<Option<usize>>(available_item_id)
                    .unwrap_or_default()
            });

            let mut selected_index = tui.egui_ui().memory(|mem| {
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
                // using 'take' ensures there's no selection after handling the button click
                let (k, _s) = available_items
                    .iter()
                    .nth(available_index.take().unwrap())
                    .unwrap();

                added_fn(k, &v);

                tui.egui_ui().memory_mut(|mem| {
                    mem.data
                        .insert_temp(available_item_id, available_index)
                });
            };
            if tui
                .enabled_ui(selected_index.is_some())
                .ui_add(egui::Button::new("<"))
                .clicked()
            {
                // using 'take' ensures there's no selection after handling the button click
                let (k, (_s, v)) = selected_items
                    .iter()
                    .nth(selected_index.take().unwrap())
                    .unwrap();

                removed_fn(k, v);

                tui.egui_ui().memory_mut(|mem| {
                    mem.data
                        .insert_temp(selected_item_id, selected_index)
                });
            };

            // end of cell
        });

        tui.egui_ui()
            .memory_mut(|mem| mem.data.insert_temp(id, v));
    }
}
