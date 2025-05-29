use egui::{Ui, WidgetText};
use egui_i18n::tr;

use crate::pcb::tabs::PcbTabContext;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Debug)]
pub struct PanelTabUi {
    pub component: ComponentState<PanelTabUiCommand>,
}

impl PanelTabUi {
    pub fn new() -> Self {
        Self {
            component: Default::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum PanelTabUiCommand {
    None,
}

#[derive(Debug, Clone)]
pub enum PanelTabUiAction {
    None,
}

#[derive(Debug, Clone, Default)]
pub struct PanelTabUiContext {}

impl UiComponent for PanelTabUi {
    type UiContext<'context> = PanelTabUiContext;
    type UiCommand = PanelTabUiCommand;
    type UiAction = PanelTabUiAction;

    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        ui.spinner();
    }

    fn update<'context>(
        &mut self,
        _command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match _command {
            PanelTabUiCommand::None => Some(PanelTabUiAction::None),
        }
    }
}

#[derive(Default, Debug, serde::Deserialize, serde::Serialize)]
pub struct PanelTab {}

impl Tab for PanelTab {
    type Context = PcbTabContext;

    fn label(&self) -> WidgetText {
        let title = tr!("pcb-panel-tab-label");

        egui::widget_text::WidgetText::from(title)
    }

    fn ui<'a>(&mut self, ui: &mut Ui, _tab_key: &TabKey, context: &mut Self::Context) {
        let state = context.state.lock().unwrap();
        UiComponent::ui(&state.panel_tab_ui, ui, &mut PanelTabUiContext::default());
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, _context: &mut Self::Context) -> bool {
        true
    }
}
