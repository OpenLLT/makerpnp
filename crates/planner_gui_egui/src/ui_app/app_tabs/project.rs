use egui::{Checkbox, FontFamily, RichText, Ui, WidgetText};
use egui_i18n::tr;
use egui_material_icons::icons::ICON_HOME;
use egui_taffy::taffy::prelude::{length, percent};
use egui_taffy::taffy::Style;
use egui_taffy::{taffy, tui, TuiBuilderLogic};
use serde::{Deserialize, Serialize};
use crate::project::ProjectKey;
use crate::tabs::{Tab, TabKey};
use crate::ui_app::app_tabs::TabContext;

#[derive(Clone, Default, Debug, Deserialize, Serialize)]
pub struct ProjectTab {
    pub project_key: ProjectKey,

    pub label: String,
    pub modified: bool,
}

impl ProjectTab {
    pub fn new(label: String, project_key: ProjectKey) -> Self {
        Self {
            label,
            project_key,
            modified: false,
        }
    }
}

impl Tab for ProjectTab {
    type Context<'a> = TabContext<'a>;

    fn label(&self) -> WidgetText {
        egui::widget_text::WidgetText::from(self.label.clone())
    }

    fn ui(&mut self, ui: &mut Ui, _tab_key: &mut TabKey, context: &mut Self::Context<'_>) {
        ui.label("project tab");
    }
}
