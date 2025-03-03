use egui::{Ui, WidgetText};
use egui_extras::{Column, TableBuilder};
use egui_i18n::tr;
use planner_app::{PhaseOverview, PhasePlacements, Reference};
use crate::project::{ProjectTabContext};
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
        self.placements.replace(phase_placements);
    }
}

#[derive(Debug, PartialEq)]
pub struct PhaseTab {
    phase: Reference,
}

impl PhaseTab {
    pub fn new(phase: Reference) -> Self {
        Self {
            phase
        }
    }
}

impl Tab for PhaseTab {
    type Context = ProjectTabContext;

    fn label(&self) -> WidgetText {
        let title = format!("{}", self.phase).to_string();
        egui::widget_text::WidgetText::from(title)
    }

    fn ui<'a>(&mut self, ui: &mut Ui, tab_key: &TabKey, context: &mut Self::Context) {
        ui.label(format!("phase: {:?}, key: {:?}", self.phase, context.key));

        let state = context.state.lock().unwrap();
        let phase = state.phases.get(&self.phase).unwrap();
        if let Some(phase_placements) = &phase.placements {

            let table = TableBuilder::new(ui)
                .striped(true)
                .resizable(true)
                .column(Column::auto())
                .column(Column::remainder());

            table.header(20.0, |mut header| {
                header.col(|ui| {
                    ui.strong(tr!("phase-placements-header"));
                });
            }).body(|mut body| {

                for (index, placement_state) in phase_placements.placements.iter().enumerate() {
                    body.row(18.0, |mut row| {
                        row.col(|ui| {
                            ui.label(format!("{}", index));
                        });
                        row.col(|ui| {
                            ui.label(&placement_state.placement.ref_des);
                        });
                    })
                }
            });
        }

    }
}