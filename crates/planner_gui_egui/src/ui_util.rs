use std::fmt::{Display, Formatter};
use std::ops::Deref;

use eframe::emath::{Rect, Vec2};
use egui::{Color32, Style, Ui};
use egui_taffy::Tui;

pub fn green_orange_red_grey_from_style(style: &Style) -> (Color32, Color32, Color32, Color32) {
    let visual = &style.visuals;

    // Credit: following snippet from egui-data-tables
    // Following logic simply gets 'green' color from current background's brightness.
    let green = if visual.window_fill.g() > 128 {
        Color32::DARK_GREEN
    } else {
        Color32::GREEN
    };

    (green, Color32::ORANGE, Color32::RED, Color32::LIGHT_GRAY)
}

// FIXME this is a best-effort attempt to make the table resize smaller and larger with the window
//       ideally the min size should be set based on the parent rect, but after hours of struggling
//       a solution was not found, so we use the clip-rect instead, and hope this is good enough for
//       the current use-cases.
pub fn tui_container_size(tui: &mut Tui) -> Vec2 {
    let parent_rect = tui.taffy_container().parent_rect();
    let container_rect = tui
        .taffy_container()
        .full_container_without_border_and_padding();

    let ui = tui.egui_ui_mut();
    let clip_rect = ui.clip_rect();
    let size_rect = container_rect.intersect(clip_rect);

    debug_rect(ui, parent_rect, Color32::RED);
    debug_rect(ui, container_rect, Color32::MAGENTA);
    debug_rect(ui, clip_rect, Color32::YELLOW);
    debug_rect(ui, size_rect, Color32::GREEN);

    size_rect.size()
}

#[cfg(feature = "layout_debugging")]
pub fn debug_rect(ui: &mut Ui, rect: Rect, debug_color: Color32) {
    let debug_stroke = egui::Stroke::new(1.0, debug_color);
    ui.painter().rect(
        rect,
        CornerRadius::ZERO,
        Color32::TRANSPARENT,
        debug_stroke,
        egui::StrokeKind::Outside,
    );
}

#[cfg(not(feature = "layout_debugging"))]
#[inline]
pub fn debug_rect(_ui: &mut Ui, _rect: Rect, _debug_color: Color32) {}

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, PartialEq, Eq, Hash)]
pub struct NavigationPath(String);

impl NavigationPath {
    pub fn new(path: String) -> Self {
        Self(path)
    }
}

impl Deref for NavigationPath {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for NavigationPath {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for NavigationPath {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl Default for NavigationPath {
    fn default() -> Self {
        Self::new("/".to_string())
    }
}
