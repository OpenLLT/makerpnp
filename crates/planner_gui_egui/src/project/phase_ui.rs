use egui::Ui;
use planner_app::{PhaseOverview, PhasePlacements, Reference};
use crate::project::ProjectKey;

#[derive(Debug)]
pub struct PhaseUi {
    phase: Reference,
    overview: Option<PhaseOverview>,
    placements: Option<PhasePlacements>,
}

impl PhaseUi {

    pub fn new(phase: Reference) -> Self {
        Self {
            phase,
            overview: None,
            placements: None,
        }
    }

    pub fn update_overview(&mut self, phase_overview: PhaseOverview) {
        self.overview.replace(phase_overview);
    }

    pub fn update_placements(&mut self, phase_placements: PhasePlacements) {
        self.placements.replace(phase_placements);
    }

    pub fn ui(&self, ui: &mut Ui, key: &ProjectKey) {
        ui.label(format!("phase: {:?}, key: {:?}", self.phase, key));
    }
}
