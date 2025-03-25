use egui::{Ui, WidgetText};
use egui_i18n::tr;
use planner_app::PlacementsList;

use crate::project::tables;
use crate::project::tabs::ProjectTabContext;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Debug)]
pub struct PlacementsUi {
    placements: Option<PlacementsList>,

    pub component: ComponentState<PlacementsUiCommand>,
}

impl PlacementsUi {
    pub fn new() -> Self {
        Self {
            placements: None,
            component: Default::default(),
        }
    }

    pub fn update_placements(&mut self, placements: PlacementsList) {
        self.placements.replace(placements);
    }
}

#[derive(Debug, Clone)]
pub enum PlacementsUiCommand {
    None,
}

#[derive(Debug, Clone)]
pub enum PlacementsUiAction {
    None,
}

#[derive(Debug, Clone, Default)]
pub struct PlacementsUiContext {}

impl UiComponent for PlacementsUi {
    type UiContext<'context> = PlacementsUiContext;
    type UiCommand = PlacementsUiCommand;
    type UiAction = PlacementsUiAction;

    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        ui.label(tr!("project-placements-header"));
        if let Some(placements_list) = &self.placements {
            tables::show_placements(ui, &placements_list.placements);
        }
    }

    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            PlacementsUiCommand::None => Some(PlacementsUiAction::None),
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
        UiComponent::ui(&state.placements_ui, ui, &mut PlacementsUiContext::default());
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, _context: &mut Self::Context) -> bool {
        true
    }
}
