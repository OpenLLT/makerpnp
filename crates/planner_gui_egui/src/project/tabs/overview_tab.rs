use egui::{Ui, WidgetText};
use egui_i18n::tr;
use planner_app::ProjectOverview;

use crate::project::tabs::ProjectTabContext;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Debug)]
pub struct OverviewTabUi {
    overview: Option<ProjectOverview>,

    pub component: ComponentState<OverviewTabUiCommand>,
}

impl OverviewTabUi {
    pub fn new() -> Self {
        Self {
            overview: None,
            component: Default::default(),
        }
    }

    pub fn update_overview(&mut self, project_overview: ProjectOverview) {
        self.overview.replace(project_overview);
    }
}

#[derive(Debug, Clone)]
pub enum OverviewTabUiCommand {
    None,
}

#[derive(Debug, Clone)]
pub enum OverviewTabUiAction {
    None,
}

#[derive(Debug, Clone, Default)]
pub struct OverviewTabUiContext {}

impl UiComponent for OverviewTabUi {
    type UiContext<'context> = OverviewTabUiContext;
    type UiCommand = OverviewTabUiCommand;
    type UiAction = OverviewTabUiAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        ui.label(tr!("project-overview-header"));
        if let Some(overview) = &self.overview {
            ui.label(tr!("project-overview-detail-name", { name: &overview.name }));
        }
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            OverviewTabUiCommand::None => Some(OverviewTabUiAction::None),
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
        UiComponent::ui(&state.overview_ui, ui, &mut OverviewTabUiContext::default());
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, _context: &mut Self::Context) -> bool {
        true
    }
}
