use derivative::Derivative;
use egui::{Ui, WidgetText};
use egui_dock::tab_viewer::OnCloseResponse;
use egui_i18n::tr;
use nalgebra::Vector2;
use planner_app::{ObjectPath, PcbSide, PhaseOverview, PlacementState, PlacementsList};
use rust_decimal::Decimal;
use tracing::trace;

use crate::project::tables::placements::{
    PlacementsTableUi, PlacementsTableUiAction, PlacementsTableUiCommand, PlacementsTableUiContext,
};
use crate::project::tabs::ProjectTabContext;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct PlacementsTabUi {
    #[derivative(Debug = "ignore")]
    placements_table_ui: PlacementsTableUi,

    pub component: ComponentState<PlacementsTabUiCommand>,
}

impl PlacementsTabUi {
    pub fn new() -> Self {
        let component: ComponentState<PlacementsTabUiCommand> = Default::default();

        let mut placements_table_ui = PlacementsTableUi::new();
        placements_table_ui
            .component
            .configure_mapper(component.sender.clone(), |placements_table_command| {
                trace!("placements table mapper. command: {:?}", placements_table_command);
                PlacementsTabUiCommand::PlacementsTableUiCommand(placements_table_command)
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
    pub fn update_phases(&mut self, phases: Vec<PhaseOverview>) {
        self.placements_table_ui
            .update_phases(phases);
    }
}

#[derive(Debug, Clone)]
pub enum PlacementsTabUiCommand {
    None,
    PlacementsTableUiCommand(PlacementsTableUiCommand),
}

#[derive(Debug, Clone)]
pub enum PlacementsTabUiAction {
    None,
    RequestRepaint,
    UpdatePlacement {
        object_path: ObjectPath,
        new_placement: PlacementState,
        old_placement: PlacementState,
    },
    LocatePlacement {
        /// Full object path of the component
        object_path: ObjectPath,
        pcb_side: PcbSide,
        placement_coordinate: Vector2<Decimal>,
        unit_coordinate: Vector2<Decimal>,
    },
}

#[derive(Debug, Clone, Default)]
pub struct PlacementsTabUiContext {}

impl UiComponent for PlacementsTabUi {
    type UiContext<'context> = PlacementsTabUiContext;
    type UiCommand = PlacementsTabUiCommand;
    type UiAction = PlacementsTabUiAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        ui.label(tr!("project-placements-header"));
        self.placements_table_ui
            .ui(ui, &mut PlacementsTableUiContext::default())
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            PlacementsTabUiCommand::None => Some(PlacementsTabUiAction::None),
            PlacementsTabUiCommand::PlacementsTableUiCommand(command) => {
                let action = self
                    .placements_table_ui
                    .update(command, &mut PlacementsTableUiContext::default());
                match action {
                    Some(PlacementsTableUiAction::None) => None,
                    Some(PlacementsTableUiAction::RequestRepaint) => Some(PlacementsTabUiAction::RequestRepaint),
                    Some(PlacementsTableUiAction::UpdatePlacement {
                        object_path,
                        new_placement,
                        old_placement,
                    }) => Some(PlacementsTabUiAction::UpdatePlacement {
                        object_path,
                        new_placement,
                        old_placement,
                    }),
                    Some(PlacementsTableUiAction::LocatePlacement {
                        object_path,
                        pcb_side,
                        placement_coordinate,
                        unit_coordinate,
                    }) => Some(PlacementsTabUiAction::LocatePlacement {
                        object_path,
                        pcb_side,
                        placement_coordinate,
                        unit_coordinate,
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
        UiComponent::ui(&state.placements_ui, ui, &mut PlacementsTabUiContext::default());
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, _context: &mut Self::Context) -> OnCloseResponse {
        OnCloseResponse::Close
    }
}
