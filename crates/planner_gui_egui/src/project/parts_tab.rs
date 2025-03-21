use std::borrow::Cow;

use derivative::Derivative;
use egui::{Response, Ui, WidgetText};
use egui_data_table::viewer::CellWriteContext;
use egui_data_table::{DataTable, RowViewer};
use egui_i18n::tr;
use egui_mobius::types::Value;
use planner_app::{Part, PartStates, PartWithState};
use tracing::debug;

use crate::project::tabs::ProjectTabContext;
use crate::tabs::{Tab, TabKey};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct PartsUi {
    #[derivative(Debug = "ignore")]
    part_states_table: Value<Option<(PartStatesRowViewer, DataTable<PartStatesRow>)>>,
}

impl PartsUi {
    pub fn new() -> Self {
        Self {
            //part_states: None,
            part_states_table: Value::default(),
        }
    }

    pub fn update_part_states(&mut self, mut part_states: PartStates) {
        //self.part_states.replace(part_states);

        let mut part_states_table = self.part_states_table.lock().unwrap();
        let table: DataTable<PartStatesRow> = {
            part_states
                .parts
                .drain(0..)
                .map(|part_state| {
                    // todo, build dynamically
                    let processes = vec![(true, "Manual".into()), (false, "PnP".into())];
                    PartStatesRow {
                        part_state,
                        processes,
                    }
                })
        }
        .collect();

        part_states_table.replace((PartStatesRowViewer::default(), table));
    }

    pub fn ui(&self, ui: &mut Ui) {
        ui.label(tr!("project-parts-header"));
        let mut part_states_table = self.part_states_table.lock().unwrap();
        if let Some((viewer, table)) = part_states_table.as_mut() {
            ui.add(egui_data_table::Renderer::new(table, viewer));
        }
    }
}

#[derive(Debug)]
struct PartStatesRow {
    part_state: PartWithState,
    processes: Vec<(bool, String)>,
}

#[derive(Default)]
struct PartStatesRowViewer;

impl RowViewer<PartStatesRow> for PartStatesRowViewer {
    fn num_columns(&mut self) -> usize {
        3
    }

    fn is_sortable_column(&mut self, column: usize) -> bool {
        [true, true, true][column]
    }

    fn compare_cell(&self, row_l: &PartStatesRow, row_r: &PartStatesRow, column: usize) -> std::cmp::Ordering {
        match column {
            0 => row_l
                .part_state
                .part
                .manufacturer
                .cmp(&row_r.part_state.part.manufacturer),
            1 => row_l
                .part_state
                .part
                .mpn
                .cmp(&row_r.part_state.part.mpn),
            2 => row_l
                .part_state
                .processes
                .cmp(&row_r.part_state.processes),
            _ => unreachable!(),
        }
    }

    fn column_name(&mut self, column: usize) -> Cow<'static, str> {
        match column {
            0 => tr!("table-parts-column-manufacturer"),
            1 => tr!("table-parts-column-mpn"),
            2 => tr!("table-parts-column-processes"),
            _ => unreachable!(),
        }
        .into()
    }

    fn show_cell_view(&mut self, ui: &mut Ui, row: &PartStatesRow, column: usize) {
        let _ = match column {
            0 => ui.label(&row.part_state.part.manufacturer),
            1 => ui.label(&row.part_state.part.mpn),
            2 => {
                let processes: String = row
                    .processes
                    .iter()
                    .filter_map(|(enabled, name)| match enabled {
                        true => Some(name.to_string()),
                        false => None,
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                ui.label(processes)
            }
            _ => unreachable!(),
        };
    }

    fn show_cell_editor(&mut self, ui: &mut Ui, row: &mut PartStatesRow, column: usize) -> Option<Response> {
        match column {
            0 => None,
            1 => None,
            2 => {
                let ui = ui.add(|ui: &mut Ui| {
                    ui.horizontal_wrapped(|ui| {
                        for (enabled, name) in row.processes.iter_mut() {
                            ui.checkbox(enabled, name.clone());
                        }
                    })
                    .response
                });
                Some(ui)
            }
            _ => unreachable!(),
        }
    }

    fn set_cell_value(&mut self, src: &PartStatesRow, dst: &mut PartStatesRow, column: usize) {
        match column {
            0 => dst
                .part_state
                .part
                .manufacturer
                .clone_from(&src.part_state.part.manufacturer),
            1 => dst
                .part_state
                .part
                .mpn
                .clone_from(&src.part_state.part.mpn),
            2 => {
                dst.part_state
                    .processes
                    .clone_from(&src.part_state.processes);
                dst.processes.clone_from(&src.processes);
            }
            _ => unreachable!(),
        }
    }

    fn new_empty_row(&mut self) -> PartStatesRow {
        // FIXME why do we need to implement this?
        PartStatesRow {
            part_state: PartWithState {
                part: Part {
                    manufacturer: "".to_string(),
                    mpn: "".to_string(),
                },
                processes: vec![],
            },
            processes: vec![],
        }
    }

    fn confirm_cell_write_by_ui(
        &mut self,
        _current: &PartStatesRow,
        _next: &PartStatesRow,
        column: usize,
        _context: CellWriteContext,
    ) -> bool {
        debug!(
            "confirm cell write by ui. column: {}, current: {:?}, next: {:?}, context: {:?}",
            column, _current, _next, _context
        );
        match column {
            0 => false,
            1 => false,
            2 => true,
            _ => unreachable!(),
        }
    }

    fn confirm_row_deletion_by_ui(&mut self, _row: &PartStatesRow) -> bool {
        false
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
