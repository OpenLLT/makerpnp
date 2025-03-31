use derivative::Derivative;
use egui::{Ui, WidgetText};
use egui_i18n::tr;
use planner_app::{PhaseOverview, PhasePlacements, Reference};
use tracing::debug;

use crate::project::placements_tab::PlacementsUiCommand;
use crate::project::tables::placements::{
    PlacementsTableUi, PlacementsTableUiAction, PlacementsTableUiCommand, PlacementsTableUiContext,
};
use crate::project::tabs::ProjectTabContext;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct PhaseUi {
    overview: Option<PhaseOverview>,
    #[derivative(Debug = "ignore")]
    placements_table_ui: PlacementsTableUi,

    pub component: ComponentState<PhaseUiCommand>,
}

impl PhaseUi {
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
            overview: None,
            placements_table_ui,
            component: Default::default(),
        }
    }

    pub fn update_overview(&mut self, phase_overview: PhaseOverview) {
        self.overview.replace(phase_overview);
    }

    pub fn update_placements(&mut self, phase_placements: PhasePlacements, phases: Vec<Reference>) {
        self.placements_table_ui
            .update_placements(phase_placements.placements, phases);
    }
}

#[derive(Debug, Clone)]
pub enum PhaseUiCommand {
    None,
    PlacementsTableUiCommand(PlacementsTableUiCommand),
}

#[derive(Debug, Clone)]
pub enum PhaseUiAction {
    None,
    RequestRepaint,
}

#[derive(Debug, Clone, Default)]
pub struct PhaseUiContext {}

impl UiComponent for PhaseUi {
    type UiContext<'context> = PhaseUiContext;
    type UiCommand = PhaseUiCommand;
    type UiAction = PhaseUiAction;

    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        ui.label(tr!("phase-placements-header"));
        self.placements_table_ui
            .ui(ui, &mut PlacementsTableUiContext::default());
    }

    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            PhaseUiCommand::None => Some(PhaseUiAction::None),
            PhaseUiCommand::PlacementsTableUiCommand(command) => {
                let action = self
                    .placements_table_ui
                    .update(command, &mut PlacementsTableUiContext::default());
                match action {
                    Some(PlacementsTableUiAction::None) => None,
                    Some(PlacementsTableUiAction::RequestRepaint) => Some(PhaseUiAction::RequestRepaint),
                    Some(PlacementsTableUiAction::UpdatePlacement {
                        placement,
                    }) => {
                        todo!()
                    }
                    None => None,
                }
            }
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq)]
pub struct PhaseTab {
    phase: Reference,
}

impl PhaseTab {
    pub fn new(phase: Reference) -> Self {
        Self {
            phase,
        }
    }
}

impl Tab for PhaseTab {
    type Context = ProjectTabContext;

    fn label(&self) -> WidgetText {
        let title = format!("{}", self.phase).to_string();
        egui::widget_text::WidgetText::from(title)
    }

    fn ui<'a>(&mut self, ui: &mut Ui, _tab_key: &TabKey, context: &mut Self::Context) {
        let state = context.state.lock().unwrap();
        let phase_ui = state.phases.get(&self.phase).unwrap();
        UiComponent::ui(phase_ui, ui, &mut PhaseUiContext::default());
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, context: &mut Self::Context) -> bool {
        let mut state = context.state.lock().unwrap();
        if let Some(_phase) = state.phases.remove(&self.phase) {
            debug!("removed orphaned phase: {:?}", &self.phase);
        }
        true
    }
}
