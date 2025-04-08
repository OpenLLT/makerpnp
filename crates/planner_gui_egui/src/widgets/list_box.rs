use egui::Id;
use egui::widget_text::WidgetText;
use egui_taffy::taffy::{AlignItems, FlexDirection, Style};
use egui_taffy::{Tui, TuiBuilderLogic};

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
