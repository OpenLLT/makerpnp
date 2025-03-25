use egui::{Ui, WidgetText};
use egui_i18n::tr;
use planner_app::{PhaseOverview, PhasePlacements, Reference};
use tracing::debug;

use crate::project::tables;
use crate::project::tables::{ColumnIdx, PlacementsStateTableState, TableAction};
use crate::project::tabs::ProjectTabContext;
use crate::project::{ProjectKey, ProjectUiCommand, tables};
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Debug)]
pub struct PhaseUi {
    overview: Option<PhaseOverview>,
    placements: Option<PhasePlacements>,

    table_state: PlacementsStateTableState,

    pub component: ComponentState<PhaseUiCommand>,
}

impl PhaseUi {
    pub fn new() -> Self {
        Self {
            overview: None,
            placements: None,
            table_state: Default::default(),
            component: Default::default(),
        }
    }

    pub fn update_overview(&mut self, phase_overview: PhaseOverview) {
        self.overview.replace(phase_overview);
    }

    pub fn update_placements(&mut self, phase_placements: PhasePlacements) {
        self.placements
            .replace(phase_placements);
    }
}

#[derive(Debug, Clone)]
pub enum PhaseUiCommand {
    PhasePlacementsTableColumnHeaderClicked { column: usize },
    None,
}

#[derive(Debug, Clone)]
pub enum PhaseUiAction {
    None,
}

#[derive(Debug, Clone, Default)]
pub struct PhaseUiContext {
}

impl UiComponent for PhaseUi {
    type UiContext<'context> = PhaseUiContext;
    type UiCommand = PhaseUiCommand;
    type UiAction = PhaseUiAction;

    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        ui.label(tr!("phase-placements-header"));

        if let Some(phase_placements) = &self.placements {
            let table_action = tables::show_placements(ui, &phase_placements.placements, &self.table_state);
            match table_action {
                Some(TableAction::ColumnHeaderClicked(index)) => {
                    self.component
                        .send(PhaseUiCommand::PhasePlacementsTableColumnHeaderClicked { column: index });
                }
                None => {}
            }
        }
    }

    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            PhaseUiCommand::None => Some(PhaseUiAction::None),
            PhaseUiCommand::PhasePlacementsTableColumnHeaderClicked { column } => {
                self.table_state.on_column_header_clicked(ColumnIdx(column));

                None
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
