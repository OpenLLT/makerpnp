use std::fmt::{Display, Formatter};
use std::ops::Deref;

use egui::{Color32, Style};

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

#[derive(Debug, Clone, PartialEq)]
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
