use egui::{Ui, WidgetText};
use egui_i18n::tr;
use planner_app::{PhaseOverview, PhasePlacements, Reference};
use tracing::debug;

use crate::project::tables;
use crate::project::tabs::ProjectTabContext;
use crate::tabs::{Tab, TabKey};

#[derive(Debug)]
pub struct PhaseUi {
    overview: Option<PhaseOverview>,
    placements: Option<PhasePlacements>,
}

impl PhaseUi {
    pub fn new() -> Self {
        Self {
            overview: None,
            placements: None,
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
        let phase = state.phases.get(&self.phase).unwrap();
        if let Some(phase_placements) = &phase.placements {
            ui.label(tr!("phase-placements-header"));
            tables::show_placements(ui, &phase_placements.placements)
        }
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, context: &mut Self::Context) -> bool {
        let mut state = context.state.lock().unwrap();
        if let Some(_phase) = state.phases.remove(&self.phase) {
            debug!("removed orphaned phase: {:?}", &self.phase);
        }
        true
    }
}
