use egui::{Ui, WidgetText};
use egui::scroll_area::ScrollBarVisibility;
use egui_extras::{Column, TableBuilder};
use egui_i18n::tr;
use tracing::debug;
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
                .scroll_bar_visibility(ScrollBarVisibility::AlwaysVisible)
                .column(Column::auto()) // index
                .column(Column::auto()) // refdes
                .column(Column::auto()) // placed
                .column(Column::auto()) // mfr
                .column(Column::remainder()) // mpn
                .column(Column::auto()) // rotation
                .column(Column::auto()) // x
                .column(Column::auto());           // y

            table.header(20.0, |mut header| {
                header.col(|ui| {
                    ui.strong(tr!("phase-placements-column-index"));
                });
                header.col(|ui| {
                    ui.strong(tr!("phase-placements-column-refdes"));
                });
                header.col(|ui| {
                    ui.strong(tr!("phase-placements-column-placed"));
                });
                header.col(|ui| {
                    ui.strong(tr!("phase-placements-column-manufacturer"));
                });
                header.col(|ui| {
                    ui.strong(tr!("phase-placements-column-mpn"));
                });
                header.col(|ui| {
                    ui.strong(tr!("phase-placements-column-rotation"));
                });
                header.col(|ui| {
                    ui.strong(tr!("phase-placements-column-x"));
                });
                header.col(|ui| {
                    ui.strong(tr!("phase-placements-column-y"));
                });
            }).body(|mut body| {

                for (index, placement_state) in phase_placements.placements.iter().enumerate() {
                    body.row(18.0, |mut row| {
                        row.col(|ui| {
                            ui.label(format!("{}", index_to_human_readable(index)));
                        });
                        row.col(|ui| {
                            ui.label(&placement_state.placement.ref_des);
                        });
                        row.col(|ui| {
                            let label = match placement_state.placed {
                                true => tr!("placement-placed"),
                                false => tr!("placement-pending"),
                            };
                            ui.label(label);
                        });
                        row.col(|ui| {
                            ui.label(&placement_state.placement.part.manufacturer);
                        });
                        row.col(|ui| {
                            ui.label(&placement_state.placement.part.mpn);
                        });   
                        row.col(|ui| {
                            ui.label(placement_state.placement.x.to_string());
                        });     
                        row.col(|ui| {
                            ui.label(placement_state.placement.y.to_string());
                        });    
                        row.col(|ui| {
                            ui.label(placement_state.placement.rotation.to_string());
                        });
                    })
                }
            });
        }
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, context: &mut Self::Context) -> bool {
        let mut state = context.state.lock().unwrap();
        if let Some(phase) = state.phases.remove(&self.phase) {
            debug!("removed orphaned phase: {:?}", &self.phase);
        }
        true
    }
}

// TODO move this someone else on 2nd re-use.
fn index_to_human_readable(index: usize) -> usize {
    index + 1
}
