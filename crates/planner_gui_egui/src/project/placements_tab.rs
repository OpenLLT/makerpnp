use egui::{Ui, WidgetText};
use egui_i18n::tr;
use planner_app::PlacementsList;

use crate::project::tables;
use crate::project::tabs::ProjectTabContext;
use crate::tabs::{Tab, TabKey};

#[derive(Debug)]
pub struct PlacementsUi {
    placements: Option<PlacementsList>,
}

impl PlacementsUi {
    pub fn new() -> Self {
        Self {
            placements: None,
        }
    }

    pub fn update_placements(&mut self, placements: PlacementsList) {
        self.placements.replace(placements);
    }

    pub fn ui(&self, ui: &mut Ui) {
        ui.label(tr!("project-placements-header"));
        if let Some(placements_list) = &self.placements {
            tables::show_placements(ui, &placements_list.placements);
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Default, PartialEq)]
pub struct PlacementsTab {}

impl Tab for PlacementsTab {
    type Context = ProjectTabContext;

    fn label(&self) -> WidgetText {
        egui::widget_text::WidgetText::from(tr!("project-placements-tab-label"))
    }

    fn ui<'a>(&mut self, ui: &mut Ui, _tab_key: &TabKey, context: &mut Self::Context) {
        let state = context.state.lock().unwrap();
        state.placements_ui.ui(ui);
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, _context: &mut Self::Context) -> bool {
        true
    }
}
