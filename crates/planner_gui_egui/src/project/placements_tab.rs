use derivative::Derivative;
use egui::{Ui, WidgetText};
use egui_i18n::tr;
use planner_app::{ObjectPath, PhaseOverview, PlacementState, PlacementsList};
use tracing::debug;

use crate::project::tables::placements::{
    PlacementsTableUi, PlacementsTableUiAction, PlacementsTableUiCommand, PlacementsTableUiContext,
};
use crate::project::tabs::ProjectTabContext;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct PlacementsUi {
    #[derivative(Debug = "ignore")]
    placements_table_ui: PlacementsTableUi,

    pub component: ComponentState<PlacementsUiCommand>,
}

impl PlacementsUi {
    pub fn new() -> Self {
        let component: ComponentState<PlacementsUiCommand> = Default::default();

        let mut placements_table_ui = PlacementsTableUi::new();
        placements_table_ui
            .component
            .configure_mapper(component.sender.clone(), |placements_table_command| {
                debug!("placements table mapper. command: {:?}", placements_table_command);
                PlacementsUiCommand::PlacementsTableUiCommand(placements_table_command)
            });

        Self {
            placements_table_ui,
            component,
        }
    }

    pub fn update_placements(&mut self, placements: PlacementsList, phases: Vec<PhaseOverview>) {
        self.placements_table_ui
            .update_placements(placements.placements, phases);
    }
}

#[derive(Debug, Clone)]
pub enum PlacementsUiCommand {
    None,
    PlacementsTableUiCommand(PlacementsTableUiCommand),
}

#[derive(Debug, Clone)]
pub enum PlacementsUiAction {
    None,
    RequestRepaint,
    UpdatePlacement {
        object_path: ObjectPath,
        new_placement: PlacementState,
        old_placement: PlacementState,
    },
}

#[derive(Debug, Clone, Default)]
pub struct PlacementsUiContext {}

impl UiComponent for PlacementsUi {
    type UiContext<'context> = PlacementsUiContext;
    type UiCommand = PlacementsUiCommand;
    type UiAction = PlacementsUiAction;

    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        ui.label(tr!("project-placements-header"));
        self.placements_table_ui
            .ui(ui, &mut PlacementsTableUiContext::default())
    }

    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            PlacementsUiCommand::None => Some(PlacementsUiAction::None),
            PlacementsUiCommand::PlacementsTableUiCommand(command) => {
                let action = self
                    .placements_table_ui
                    .update(command, &mut PlacementsTableUiContext::default());
                match action {
                    Some(PlacementsTableUiAction::None) => None,
                    Some(PlacementsTableUiAction::RequestRepaint) => Some(PlacementsUiAction::RequestRepaint),
                    Some(PlacementsTableUiAction::UpdatePlacement {
                        object_path,
                        new_placement,
                        old_placement,
                    }) => Some(PlacementsUiAction::UpdatePlacement {
                        object_path,
                        new_placement,
                        old_placement,
                    }),
                    None => None,
                }
            }
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
