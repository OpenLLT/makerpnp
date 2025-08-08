use derivative::Derivative;
use egui::{Ui, WidgetText};
use egui_dock::tab_viewer::OnCloseResponse;
use egui_i18n::tr;
use planner_app::{ProcessDefinition, ProcessReference, Reference};
use tracing::debug;

use crate::project::tabs::ProjectTabContext;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct ProcessTabUi {
    process_definition: Option<ProcessDefinition>,

    pub component: ComponentState<ProcessTabUiCommand>,
}

impl ProcessTabUi {
    pub fn new() -> Self {
        let component: ComponentState<ProcessTabUiCommand> = Default::default();

        Self {
            process_definition: None,
            component,
        }
    }

    pub fn update_definition(&mut self, process_definition: ProcessDefinition) {
        self.process_definition = Some(process_definition)
    }
}

#[derive(Debug, Clone)]
pub enum ProcessTabUiCommand {
    None,
}

#[derive(Debug, Clone)]
pub enum ProcessTabUiAction {
    None,
}

#[derive(Debug, Clone, Default)]
pub struct ProcessTabUiContext {}

impl UiComponent for ProcessTabUi {
    type UiContext<'context> = ProcessTabUiContext;
    type UiCommand = ProcessTabUiCommand;
    type UiAction = ProcessTabUiAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        ui.label(tr!("project-process-header"));

        let Some(process_definition) = &self.process_definition else {
            ui.spinner();
            return;
        };

        ui.label(format!("{:?}", process_definition));
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            ProcessTabUiCommand::None => Some(ProcessTabUiAction::None),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq)]
pub struct ProcessTab {
    pub process: ProcessReference,
}

impl ProcessTab {
    pub fn new(process: Reference) -> Self {
        Self {
            process,
        }
    }
}

impl Tab for ProcessTab {
    type Context = ProjectTabContext;

    fn label(&self) -> WidgetText {
        let title = tr!("project-process-tab-label", {process: self.process.to_string()});
        egui::widget_text::WidgetText::from(title)
    }

    fn ui<'a>(&mut self, ui: &mut Ui, _tab_key: &TabKey, context: &mut Self::Context) {
        let state = context.state.lock().unwrap();
        let Some(process_ui) = state.process_tab_uis.get(&self.process) else {
            ui.spinner();
            return;
        };
        UiComponent::ui(process_ui, ui, &mut ProcessTabUiContext::default());
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, context: &mut Self::Context) -> OnCloseResponse {
        let mut state = context.state.lock().unwrap();
        if let Some(_process_ui) = state
            .process_tab_uis
            .remove(&self.process)
        {
            debug!("removed orphaned process ui. process: {:?}", &self.process);
        }
        OnCloseResponse::Close
    }
}
