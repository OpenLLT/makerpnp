use egui::Id;
use egui::widget_text::WidgetText;
use egui_taffy::taffy::{AlignItems, FlexDirection, Style};
use egui_taffy::{Tui, TuiBuilderLogic};

/// Returns a flag indicating if the selection changed, and the optional selection
pub fn list_box_with_id_tui<I, S>(tui: &mut Tui, id: Id, items: I) -> (bool, Option<usize>)
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

    let mut is_changed = false;

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
                is_changed = true;
                selected_index = Some(index);
                tui.egui_ui()
                    .memory_mut(|mem| mem.data.insert_temp(id, selected_index));
            }
        }
    });

    // Return the current selection
    (is_changed, selected_index)
}

/// Returns a flag indicating if the selection changed, and the selection, which may be empty
pub fn list_box_with_id_multi_tui<I, S>(tui: &mut Tui, id: Id, items: I) -> (bool, Vec<usize>)
where
    I: IntoIterator<Item = S>,
    S: Into<WidgetText>,
{
    let mut selected_indexes = tui.egui_ui().memory(|mem| {
        // NOTE It's CRITICAL that the correct type is specified for `get_temp`
        mem.data
            .get_temp::<Vec<usize>>(id)
            .unwrap_or_default()
    });

    let mut changed = false;

    tui.style(Style {
        flex_direction: FlexDirection::Column,
        align_items: Some(AlignItems::Stretch),
        flex_grow: 1.0,
        ..Default::default()
    })
    .add(|tui| {
        for (index, item) in items.into_iter().enumerate() {
            let is_selected = selected_indexes.contains(&index);

            let response = tui
                .style(Style {
                    ..Default::default()
                })
                .selectable(is_selected, |tui| {
                    tui.label(item);
                });

            if response.clicked() {
                changed |= true;
                if is_selected {
                    selected_indexes.retain(|i| *i != index);
                } else {
                    selected_indexes.push(index);
                }
            }
        }
    });

    tui.egui_ui().memory_mut(|mem| {
        mem.data
            .insert_temp(id, selected_indexes.clone())
    });

    (changed, selected_indexes)
}
