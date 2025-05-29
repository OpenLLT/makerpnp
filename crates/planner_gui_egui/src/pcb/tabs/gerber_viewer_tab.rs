use derivative::Derivative;
use egui::{Ui, WidgetText};
use planner_app::PcbOverview;
use tracing::trace;

use crate::pcb::tabs::PcbTabContext;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};
use crate::ui_components::gerber_viewer_ui::{
    GerberViewerMode, GerberViewerUi, GerberViewerUiAction, GerberViewerUiCommand, GerberViewerUiContext,
    GerberViewerUiInstanceArgs,
};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct GerberViewerTabUi {
    #[derivative(Debug = "ignore")]
    gerber_viewer_ui: GerberViewerUi,

    pub component: ComponentState<GerberViewerTabUiCommand>,
}

impl GerberViewerTabUi {
    pub fn new(args: GerberViewerUiInstanceArgs) -> Self {
        let component: ComponentState<GerberViewerTabUiCommand> = Default::default();

        let mut gerber_viewer_ui = GerberViewerUi::new(args);
        gerber_viewer_ui
            .component
            .configure_mapper(component.sender.clone(), |gerber_viewer_command| {
                trace!("gerber_viewer mapper. command: {:?}", gerber_viewer_command);
                GerberViewerTabUiCommand::GerberViewerUiCommand(gerber_viewer_command)
            });

        Self {
            gerber_viewer_ui,
            component,
        }
    }

    pub fn update_pcb_overview(&mut self, pcb_overview: PcbOverview) {
        self.gerber_viewer_ui
            .update_pcb_overview(pcb_overview);
    }
}

#[derive(Debug, Clone)]
pub enum GerberViewerTabUiCommand {
    None,
    GerberViewerUiCommand(GerberViewerUiCommand),
}

#[derive(Debug, Clone)]
pub enum GerberViewerTabUiAction {
    None,
}

#[derive(Debug, Clone, Default)]
pub struct GerberViewerTabUiContext {}

impl UiComponent for GerberViewerTabUi {
    type UiContext<'context> = GerberViewerTabUiContext;
    type UiCommand = GerberViewerTabUiCommand;
    type UiAction = GerberViewerTabUiAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        egui::SidePanel::left(
            ui.id()
                .with("gerber_viewer_tab_left_panel"),
        )
        .resizable(true)
        .show_inside(ui, |ui| {
            ui.label("Gerber Viewer Tab");
        });
        egui::CentralPanel::default().show_inside(ui, |ui| {
            self.gerber_viewer_ui
                .ui(ui, &mut GerberViewerUiContext {})
        });
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            GerberViewerTabUiCommand::None => Some(GerberViewerTabUiAction::None),
            GerberViewerTabUiCommand::GerberViewerUiCommand(command) => {
                let action = self
                    .gerber_viewer_ui
                    .update(command, &mut GerberViewerUiContext {});
                match action {
                    None => None,
                    Some(GerberViewerUiAction::None) => Some(GerberViewerTabUiAction::None),
                }
            }
        }
    }
}

#[derive(Default, Debug, serde::Deserialize, serde::Serialize)]
pub struct GerberViewerTab {
    pub(crate) args: GerberViewerUiInstanceArgs,
}

impl GerberViewerTab {
    pub fn new(args: GerberViewerUiInstanceArgs) -> Self {
        Self {
            args,
        }
    }
}

impl Tab for GerberViewerTab {
    type Context = PcbTabContext;

    fn label(&self) -> WidgetText {
        let title = match self.args.mode {
            GerberViewerMode::Panel => "Panel".to_string(),
            // TODO improve the tab title
            GerberViewerMode::Design(design_index) => format!("Design ({})", design_index),
        };

        egui::widget_text::WidgetText::from(title)
    }

    fn ui<'a>(&mut self, ui: &mut Ui, _tab_key: &TabKey, context: &mut Self::Context) {
        let state = context.state.lock().unwrap();
        let Some(instance) = state
            .gerber_viewer_tab_uis
            .get(&self.args)
        else {
            ui.spinner();
            return;
        };

        UiComponent::ui(instance, ui, &mut GerberViewerTabUiContext::default());
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, _context: &mut Self::Context) -> bool {
        true
    }
}
