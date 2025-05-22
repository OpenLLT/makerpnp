use egui::{Ui, WidgetText};
use egui_i18n::tr;
use planner_app::PcbOverview;

use crate::pcb::tabs::PcbTabContext;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Debug)]
pub struct ConfigurationUi {
    pcb_overview: Option<PcbOverview>,

    pub component: ComponentState<ConfigurationUiCommand>,
}

impl ConfigurationUi {
    pub fn new() -> Self {
        Self {
            pcb_overview: None,
            component: Default::default(),
        }
    }

    pub fn update_pcb_overview(&mut self, pcb_overview: PcbOverview) {
        self.pcb_overview.replace(pcb_overview);
    }
}

#[derive(Debug, Clone)]
pub enum ConfigurationUiCommand {
    None,
}

#[derive(Debug, Clone)]
pub enum ConfigurationUiAction {
    None,
}

#[derive(Debug, Clone, Default)]
pub struct ConfigurationUiContext {}

impl UiComponent for ConfigurationUi {
    type UiContext<'context> = ConfigurationUiContext;
    type UiCommand = ConfigurationUiCommand;
    type UiAction = ConfigurationUiAction;

    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        ui.label(tr!("pcb-configuration-header"));
        if let Some(pcb_overview) = &self.pcb_overview {
            ui.label(tr!("pcb-configuration-detail-name", { name: &pcb_overview.name }));
        }
    }

    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            ConfigurationUiCommand::None => Some(ConfigurationUiAction::None),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Default, PartialEq)]
pub struct ConfigurationTab {}

impl Tab for ConfigurationTab {
    type Context = PcbTabContext;

    fn label(&self) -> WidgetText {
        egui::widget_text::WidgetText::from(tr!("pcb-configuration-tab-label"))
    }

    fn ui<'a>(&mut self, ui: &mut Ui, _tab_key: &TabKey, context: &mut Self::Context) {
        let state = context.state.lock().unwrap();
        UiComponent::ui(&state.configuration_ui, ui, &mut ConfigurationUiContext::default());
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, _context: &mut Self::Context) -> bool {
        true
    }
}
