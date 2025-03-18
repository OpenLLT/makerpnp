use egui::{Ui, WidgetText};
use egui_i18n::tr;
use planner_app::ProjectOverview;

use crate::project::tabs::ProjectTabContext;
use crate::tabs::{Tab, TabKey};

#[derive(Debug)]
pub struct OverviewUi {
    overview: Option<ProjectOverview>,
}

impl OverviewUi {
    pub fn new() -> Self {
        Self {
            overview: None,
        }
    }

    pub fn update_overview(&mut self, project_overview: ProjectOverview) {
        self.overview.replace(project_overview);
    }

    pub fn ui(&self, ui: &mut Ui) {
        ui.label(tr!("project-overview-header"));
        if let Some(overview) = &self.overview {
            ui.label(tr!("project-overview-detail-name", { name: &overview.name }));
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Default, PartialEq)]
pub struct OverviewTab {}

impl Tab for OverviewTab {
    type Context = ProjectTabContext;

    fn label(&self) -> WidgetText {
        egui::widget_text::WidgetText::from(tr!("project-overview-tab-label"))
    }

    fn ui<'a>(&mut self, ui: &mut Ui, _tab_key: &TabKey, context: &mut Self::Context) {
        let state = context.state.lock().unwrap();
        state.overview_ui.ui(ui);
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, _context: &mut Self::Context) -> bool {
        true
    }
}
