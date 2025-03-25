use egui::{Color32, Id, Label, Margin, RichText, Sense, Ui};
use egui_i18n::tr;
use egui_table::{AutoSizeMode, CellInfo, HeaderCellInfo};
use planner_app::PlacementState;
use tracing::debug;

use crate::i18n::conversions::{pcb_side_to_i18n_key, placement_placed_to_i18n_key};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, PartialOrd, Ord)]
#[derive(serde::Serialize, serde::Deserialize)]
struct IsAscending(bool);
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, PartialOrd, Ord)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct ColumnIdx(pub usize);

struct PlacementStateTable<'a> {
    placements: &'a Vec<PlacementState>,
    action: Option<TableAction>,
    
    state: &'a PlacementsStateTableState,
}

#[derive(Default, Debug)]
pub struct PlacementsStateTableState {
    /// Column sorting state.
    sort: Vec<(ColumnIdx, IsAscending)>,
}

impl PlacementsStateTableState {
    pub fn on_column_header_clicked(&mut self, col: ColumnIdx) {
        let mut sort = self.sort.to_owned();
        match sort
            .iter_mut()
            .find(|(c, ..)| c == &col)
        {
            Some((_, asc)) => match asc.0 {
                true => asc.0 = false,
                false => sort.retain(|(c, ..)| c != &col),
            },
            None => {
                sort.push((col, IsAscending(true)));
            }
        }

        self.sort.clear();
        self.sort.extend(sort);
    }
}

impl<'a> PlacementStateTable<'a> {
    fn new(placements: &'a Vec<PlacementState>, state: &'a PlacementsStateTableState) -> Self {
        Self { placements, state, action: None }
    }

    pub fn cell_content_ui(&self, row_nr: u64, col_nr: usize, ui: &mut egui::Ui) {
        let placement_state = &self.placements[row_nr as usize];
        match col_nr {
            0 => {
                ui.label(format!("{}", index_to_human_readable(row_nr as usize)));
            }
            1 => {
                ui.label(&placement_state.unit_path.to_string());
            }
            2 => {
                ui.label(&placement_state.placement.ref_des);
            }
            3 => {
                let label = tr!(placement_placed_to_i18n_key(placement_state.placed));
                ui.label(label);
            }
            4 => {
                ui.label(
                    &placement_state
                        .placement
                        .part
                        .manufacturer,
                );
            }
            5 => {
                ui.label(&placement_state.placement.part.mpn);
            }
            6 => {
                ui.label(
                    placement_state
                        .placement
                        .rotation
                        .to_string(),
                );
            }
            7 => {
                ui.label(placement_state.placement.x.to_string());
            }
            8 => {
                ui.label(placement_state.placement.y.to_string());
            }
            9 => {
                let key = pcb_side_to_i18n_key(&placement_state.placement.pcb_side);
                ui.label(tr!(key));
            }
            _ => unreachable!(),
        }
    }
}

impl egui_table::TableDelegate for PlacementStateTable<'_> {
    fn header_cell_ui(&mut self, ui: &mut Ui, cell: &HeaderCellInfo) {
        let style = ui.style().clone();
        let visual = &style.visuals;

        // NOTE: unlike RED and YELLOW which can be acquirable through 'error_bg_color' and
        // 'warn_bg_color', there's no 'green' color which can be acquired from inherent theme.
        // Following logic simply gets 'green' color from current background's brightness.
        let green = if visual.window_fill.g() > 128 {
            Color32::DARK_GREEN
        } else {
            Color32::GREEN
        };

        let col = ColumnIdx(cell.group_index);

        if egui::Frame::NONE
            .inner_margin(Margin::symmetric(4, 0))
            .show(ui, |ui| {
                egui::Sides::new()
                    .height(ui.available_height())
                    .show(
                        ui,
                        |ui| {
                            let label = match cell.group_index {
                                0 => Label::new(tr!("table-placements-column-index")).selectable(false),
                                1 => Label::new(RichText::new(tr!("table-placements-column-object-path")).strong())
                                    .selectable(false),
                                2 => Label::new(RichText::new(tr!("table-placements-column-refdes")).strong())
                                    .selectable(false),
                                3 => Label::new(RichText::new(tr!("table-placements-column-placed")).strong())
                                    .selectable(false),
                                4 => Label::new(RichText::new(tr!("table-placements-column-manufacturer")).strong())
                                    .selectable(false),
                                5 => Label::new(RichText::new(tr!("table-placements-column-mpn")).strong())
                                    .selectable(false),
                                6 => Label::new(RichText::new(tr!("table-placements-column-rotation")).strong())
                                    .selectable(false),
                                7 => Label::new(RichText::new(tr!("table-placements-column-x")).strong())
                                    .selectable(false),
                                8 => Label::new(RichText::new(tr!("table-placements-column-y")).strong())
                                    .selectable(false),
                                9 => Label::new(RichText::new(tr!("table-placements-column-pcb-side")).strong())
                                    .selectable(false),
                                _ => unreachable!(),
                            };
                            //let label = label.sense(Sense::click());
                            ui.add(label)
                        },
                        |ui| {
                            if let Some(pos) = self
                                .state
                                .sort
                                .iter()
                                .position(|(c, ..)| c == &col)
                            {
                                let is_asc = self.state.sort[pos].1.0 as usize;

                                ui.colored_label(
                                    [green, Color32::RED][is_asc],
                                    RichText::new(format!("{}{}", ["↘", "↗"][is_asc], pos + 1,)).monospace(),
                                )
                            } else {
                                ui.monospace(" ")
                            }
                        },
                    );
            })
            .response
            .clicked()
        {
            debug!("clicked column-index: {}", cell.group_index);

            let action = TableAction::ColumnHeaderClicked(cell.group_index);
            self.action = Some(action);
        }
    }

    fn cell_ui(&mut self, ui: &mut Ui, cell_info: &CellInfo) {
        let egui_table::CellInfo {
            row_nr,
            col_nr,
            ..
        } = *cell_info;

        if row_nr % 2 == 1 {
            ui.painter()
                .rect_filled(ui.max_rect(), 0.0, ui.visuals().faint_bg_color);
        }

        egui::Frame::NONE
            .inner_margin(Margin::symmetric(4, 0))
            .show(ui, |ui| {
                self.cell_content_ui(row_nr, col_nr, ui);
            });
    }
}

pub enum TableAction {
    ColumnHeaderClicked(usize),
}
pub fn show_placements(ui: &mut Ui, placements: &Vec<PlacementState>, state: &PlacementsStateTableState) -> Option<TableAction> {
    let id_salt = Id::new("table_demo");
    let state_id = egui_table::Table::new()
        .id_salt(id_salt)
        .get_id(ui); // Note: must be here (in the correct outer `ui` scope) to be correct.

    let default_column = egui_table::Column::new(100.0)
        .range(10.0..=500.0)
        .resizable(true);
    
    let table = egui_table::Table::new()
        .id_salt(id_salt)
        .num_rows(placements.len() as _)
        .columns(vec![default_column; 10])
        .headers([egui_table::HeaderRow::new(20.0)])
        .auto_size_mode(AutoSizeMode::Always);

    let mut delegate = PlacementStateTable::new(placements, state);

    table.show(ui, &mut delegate);

    delegate.action
}

// TODO move this somewhere else on 2nd re-use.
pub fn index_to_human_readable(index: usize) -> usize {
    index + 1
}
