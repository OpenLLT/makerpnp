use egui::Ui;
use egui::scroll_area::ScrollBarVisibility;
use egui_extras::{Column, TableBuilder};
use egui_i18n::tr;
use planner_app::PlacementState;

use crate::i18n::conversions::{pcb_side_to_i18n_key, placement_placed_to_i18n_key};

pub fn show_placements(ui: &mut Ui, placements: &Vec<PlacementState>) {
    let table = TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .scroll_bar_visibility(ScrollBarVisibility::AlwaysVisible)
        .column(Column::auto()) // index
        .column(Column::auto()) // object path
        .column(Column::auto()) // refdes
        .column(Column::auto()) // placed
        .column(Column::auto()) // mfr
        .column(Column::auto()) // mpn
        .column(Column::auto()) // rotation
        .column(Column::auto()) // x
        .column(Column::auto()) // y
        .column(Column::auto()); // side

    table
        .header(20.0, |mut header| {
            header.col(|ui| {
                ui.strong(tr!("table-placements-column-index"));
            });
            header.col(|ui| {
                ui.strong(tr!("table-placements-column-object-path"));
            });
            header.col(|ui| {
                ui.strong(tr!("table-placements-column-refdes"));
            });
            header.col(|ui| {
                ui.strong(tr!("table-placements-column-placed"));
            });
            header.col(|ui| {
                ui.strong(tr!("table-placements-column-manufacturer"));
            });
            header.col(|ui| {
                ui.strong(tr!("table-placements-column-mpn"));
            });
            header.col(|ui| {
                ui.strong(tr!("table-placements-column-rotation"));
            });
            header.col(|ui| {
                ui.strong(tr!("table-placements-column-x"));
            });
            header.col(|ui| {
                ui.strong(tr!("table-placements-column-y"));
            });
            header.col(|ui| {
                ui.strong(tr!("table-placements-column-pcb-side"));
            });
        })
        .body(|body| {
            let row_count = placements.len();
            let mut placements_iter = placements.iter();
            body.rows(18.0, row_count, |mut row| {
                let index = row.index();
                let placement_state = placements_iter.next().unwrap();
                row.col(|ui| {
                    ui.label(format!("{}", index_to_human_readable(index)));
                });
                row.col(|ui| {
                    ui.label(&placement_state.unit_path.to_string());
                });
                row.col(|ui| {
                    ui.label(&placement_state.placement.ref_des);
                });
                row.col(|ui| {
                    let label = tr!(placement_placed_to_i18n_key(placement_state.placed));
                    ui.label(label);
                });
                row.col(|ui| {
                    ui.label(
                        &placement_state
                            .placement
                            .part
                            .manufacturer,
                    );
                });
                row.col(|ui| {
                    ui.label(&placement_state.placement.part.mpn);
                });
                row.col(|ui| {
                    ui.label(
                        placement_state
                            .placement
                            .rotation
                            .to_string(),
                    );
                });
                row.col(|ui| {
                    ui.label(placement_state.placement.x.to_string());
                });
                row.col(|ui| {
                    ui.label(placement_state.placement.y.to_string());
                });
                row.col(|ui| {
                    let key = pcb_side_to_i18n_key(&placement_state.placement.pcb_side);
                    ui.label(tr!(key));
                });
            })
        });
}

// TODO move this somewhere else on 2nd re-use.
pub fn index_to_human_readable(index: usize) -> usize {
    index + 1
}
