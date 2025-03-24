use egui::scroll_area::ScrollBarVisibility;
use egui::{Ui, WidgetText};
use egui_extras::{Column, TableBuilder};
use egui_i18n::tr;
use planner_app::PartStates;

use crate::project::tables;
use crate::project::tabs::ProjectTabContext;
use crate::tabs::{Tab, TabKey};

#[derive(Debug)]
pub struct PartsUi {
    part_states: Option<PartStates>,
}

impl PartsUi {
    pub fn new() -> Self {
        Self {
            part_states: None,
        }
    }

    pub fn update_part_states(&mut self, part_states: PartStates) {
        self.part_states.replace(part_states);
    }

    pub fn ui(&self, ui: &mut Ui) {
        ui.label(tr!("project-parts-header"));
        if let Some(part_states) = &self.part_states {
            let table = TableBuilder::new(ui)
                .striped(true)
                .resizable(true)
                .scroll_bar_visibility(ScrollBarVisibility::AlwaysVisible)
                .column(Column::auto()) // index
                .column(Column::remainder()) // mfr
                .column(Column::remainder()) // mpn
                .column(Column::remainder()); // processes

            table
                .header(20.0, |mut header| {
                    header.col(|ui| {
                        ui.strong(tr!("table-parts-column-index"));
                    });
                    header.col(|ui| {
                        ui.strong(tr!("table-parts-column-manufacturer"));
                    });
                    header.col(|ui| {
                        ui.strong(tr!("table-parts-column-mpn"));
                    });
                    header.col(|ui| {
                        ui.strong(tr!("table-parts-column-processes"));
                    });
                })
                .body(|body| {
                    let row_count = part_states.parts.len();
                    body.rows(18.0, row_count, move |mut row| {
                        let index = row.index();
                        let part_state = &part_states.parts[index];

                        row.col(|ui| {
                            ui.label(format!("{}", tables::index_to_human_readable(index)));
                        });
                        row.col(|ui| {
                            ui.label(&part_state.part.manufacturer);
                        });
                        row.col(|ui| {
                            ui.label(&part_state.part.mpn);
                        });
                        row.col(|ui| {
                            let processes: String = part_state
                                .processes
                                .iter()
                                .map(|process| process.to_string())
                                .collect::<Vec<_>>()
                                .join(",");
                            ui.label(processes);
                        });
                    })
                });
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Default, PartialEq)]
pub struct PartsTab {}

impl Tab for PartsTab {
    type Context = ProjectTabContext;

    fn label(&self) -> WidgetText {
        egui::widget_text::WidgetText::from(tr!("project-parts-tab-label"))
    }

    fn ui<'a>(&mut self, ui: &mut Ui, _tab_key: &TabKey, context: &mut Self::Context) {
        let state = context.state.lock().unwrap();
        state.parts_ui.ui(ui);
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, _context: &mut Self::Context) -> bool {
        true
    }
}
