use derivative::Derivative;
use egui::{Ui, WidgetText};
use egui_i18n::tr;
use egui_ltreeview::TreeViewState;
use egui_mobius::Value;

use crate::pcb::tabs::PcbTabContext;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};
use crate::ui_util::NavigationPath;

#[derive(Derivative)]
#[derivative(Debug)]
pub struct GerberViewerUi {
    #[derivative(Debug = "ignore")]
    tree_view_state: Value<TreeViewState<usize>>,

    pub component: ComponentState<GerberViewerUiCommand>,
}

impl GerberViewerUi {
    pub fn new() -> Self {
        Self {
            tree_view_state: Default::default(),
            component: Default::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum GerberViewerUiCommand {
    None,
}

#[derive(Debug, Clone)]
pub enum GerberViewerUiAction {
    None,
}

#[derive(Debug, Clone, Default)]
pub struct GerberViewerUiContext {}

impl UiComponent for GerberViewerUi {
    type UiContext<'context> = GerberViewerUiContext;
    type UiCommand = GerberViewerUiCommand;
    type UiAction = GerberViewerUiAction;

    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        ui.centered_and_justified(|ui| {
            ui.spinner();
        });
    }

    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            GerberViewerUiCommand::None => None,
        }
    }
}

#[derive(Default, Debug, serde::Deserialize, serde::Serialize)]
pub struct GerberViewerTab {
    pub(crate) navigation_path: NavigationPath,
}

impl GerberViewerTab {
    pub fn new(navigation_path: NavigationPath) -> Self {
        Self {
            navigation_path,
        }
    }
}

impl Tab for GerberViewerTab {
    type Context = PcbTabContext;

    fn label(&self) -> WidgetText {
        // TODO improve the tab title
        let title = format!("{}", self.navigation_path);

        egui::widget_text::WidgetText::from(title)
    }

    fn ui<'a>(&mut self, ui: &mut Ui, _tab_key: &TabKey, context: &mut Self::Context) {
        let state = context.state.lock().unwrap();
        let instance = state
            .gerber_viewer_ui
            .get(&self.navigation_path)
            .unwrap();

        UiComponent::ui(instance, ui, &mut GerberViewerUiContext::default());
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, _context: &mut Self::Context) -> bool {
        true
    }
}
