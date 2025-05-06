use derivative::Derivative;
use egui::{Ui, WidgetText};
use egui_i18n::tr;
use planner_app::PcbOverview;
use tracing::debug;

use crate::project::tabs::ProjectTabContext;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct PcbUi {
    pcb_overview: Option<PcbOverview>,

    pub component: ComponentState<PcbUiCommand>,
}

impl PcbUi {
    pub fn new() -> Self {
        Self {
            pcb_overview: None,
            component: Default::default(),
        }
    }

    pub fn update_overview(&mut self, pcb_overview: PcbOverview) {
        self.pcb_overview = Some(pcb_overview);
    }
}

#[derive(Debug, Clone)]
pub enum PcbUiCommand {
    None,
}

#[derive(Debug, Clone)]
pub enum PcbUiAction {
    None,
}

#[derive(Debug, Clone, Default)]
pub struct PcbUiContext {}

impl UiComponent for PcbUi {
    type UiContext<'context> = PcbUiContext;
    type UiCommand = PcbUiCommand;
    type UiAction = PcbUiAction;

    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        ui.label(tr!("project-pcb-header"));
        let Some(pcb_overview) = &self.pcb_overview else {
            ui.spinner();
            return;
        };

        ui.label(&pcb_overview.name);
    }

    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            PcbUiCommand::None => Some(PcbUiAction::None),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq)]
pub struct PcbTab {
    pcb_index: usize,
}

impl PcbTab {
    pub fn new(pcb_index: usize) -> Self {
        Self {
            pcb_index,
        }
    }
}

impl Tab for PcbTab {
    type Context = ProjectTabContext;

    fn label(&self) -> WidgetText {
        let title = format!("{}", self.pcb_index).to_string();
        egui::widget_text::WidgetText::from(title)
    }

    fn ui<'a>(&mut self, ui: &mut Ui, _tab_key: &TabKey, context: &mut Self::Context) {
        let state = context.state.lock().unwrap();
        let pcb_ui = state.pcbs.get(&self.pcb_index).unwrap();
        UiComponent::ui(pcb_ui, ui, &mut PcbUiContext::default());
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, context: &mut Self::Context) -> bool {
        let mut state = context.state.lock().unwrap();
        if let Some(_pcb_ui) = state.pcbs.remove(&self.pcb_index) {
            debug!("removed orphaned pcb ui. pcb_index: {}", self.pcb_index);
        }
        true
    }
}
