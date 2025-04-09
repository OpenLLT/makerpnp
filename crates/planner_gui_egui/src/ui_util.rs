use egui::{Color32, Style};

pub fn green_orange_red_from_style(style: &Style) -> (Color32, Color32, Color32) {
    let visual = &style.visuals;

    // Credit: following snippet from egui-data-tables
    // Following logic simply gets 'green' color from current background's brightness.
    let green = if visual.window_fill.g() > 128 {
        Color32::DARK_GREEN
    } else {
        Color32::GREEN
    };

    (green, Color32::ORANGE, Color32::RED)
}
