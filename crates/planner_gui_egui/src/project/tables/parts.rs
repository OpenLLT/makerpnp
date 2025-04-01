use std::borrow::Cow;

use egui::{Response, Ui};
use egui_data_table::RowViewer;
use egui_data_table::viewer::{CellWriteContext, TableColumnConfig};
use egui_i18n::tr;
use egui_mobius::types::Enqueue;
use planner_app::{Part, ProcessName};
use tracing::{debug, trace};

use crate::filter::Filter;
use crate::project::parts_tab::PartsUiCommand;

#[derive(Debug, Clone)]
pub struct PartStatesRow {
    pub part: Part,
    pub enabled_processes: Vec<(ProcessName, bool)>,
}

pub struct PartStatesRowViewer {
    processes: Vec<ProcessName>,
    sender: Enqueue<PartsUiCommand>,

    pub(crate) filter: Filter,
}

impl PartStatesRowViewer {
    pub fn new(sender: Enqueue<PartsUiCommand>, mut processes: Vec<ProcessName>) -> Self {
        // sorting the processes here helps to ensure that the view vs edit list of processes has the same
        // ordering.
        processes.sort();

        let mut filter = Filter::default();
        filter
            .component_state
            .configure_mapper(sender.clone(), |filter_ui_command| {
                debug!("filter ui mapper. command: {:?}", filter_ui_command);
                PartsUiCommand::FilterCommand(filter_ui_command)
            });

        Self {
            processes,
            sender,
            filter,
        }
    }
}

impl RowViewer<PartStatesRow> for PartStatesRowViewer {
    fn column_render_config(&mut self, column: usize, is_last_visible_column: bool) -> TableColumnConfig {
        let _ = column;
        if is_last_visible_column {
            TableColumnConfig::remainder()
                .at_least(24.0)
                .resizable(false)
                .auto_size_this_frame(true)
        } else {
            TableColumnConfig::auto()
                .resizable(true)
                .clip(true)
        }
    }

    fn num_columns(&mut self) -> usize {
        3
    }

    fn is_sortable_column(&mut self, column: usize) -> bool {
        [true, true, true][column]
    }

    fn is_editable_cell(&mut self, column: usize, _row: usize, _row_value: &PartStatesRow) -> bool {
        column == 2
    }

    fn allow_row_insertions(&mut self) -> bool {
        false
    }

    fn allow_row_deletions(&mut self) -> bool {
        false
    }

    fn compare_cell(&self, row_l: &PartStatesRow, row_r: &PartStatesRow, column: usize) -> std::cmp::Ordering {
        match column {
            0 => row_l
                .part
                .manufacturer
                .cmp(&row_r.part.manufacturer),
            1 => row_l.part.mpn.cmp(&row_r.part.mpn),
            2 => row_l
                .enabled_processes
                .iter()
                .cmp(&row_r.enabled_processes),
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
            0 => ui.label(&row.part.manufacturer),
            1 => ui.label(&row.part.mpn),
            2 => {
                // Note that the enabled_processes was built in the same order as self.processes.
                let processes = row
                    .enabled_processes
                    .iter()
                    .filter_map(|(name, enabled)| match enabled {
                        true => Some(name.to_string()),
                        false => None,
                    })
                    .collect::<Vec<_>>();

                let processes_label: String = processes.join(", ");
                ui.label(processes_label)
            }
            _ => unreachable!(),
        };
    }

    fn show_cell_editor(&mut self, ui: &mut Ui, row: &mut PartStatesRow, column: usize) -> Option<Response> {
        match column {
            0 => None,
            1 => None,
            2 => {
                let response = ui.add(|ui: &mut Ui| {
                    ui.horizontal_wrapped(|ui| {
                        // Note that the enabled_processes was built in the same order as self.processes.
                        // FIXME this is directly modifying state, we should be using ['Self::sender'] here and
                        //       triggering calls to [`update`], but the egui-data-table api doesn't expose the row
                        //       being edited
                        for (name, enabled) in row.enabled_processes.iter_mut() {
                            ui.checkbox(enabled, name.to_string());
                        }
                    })
                    .response
                });
                Some(response)
            }
            _ => unreachable!(),
        }
    }

    fn set_cell_value(&mut self, src: &PartStatesRow, dst: &mut PartStatesRow, column: usize) {
        match column {
            0 => dst
                .part
                .manufacturer
                .clone_from(&src.part.manufacturer),
            1 => dst.part.mpn.clone_from(&src.part.mpn),
            2 => {
                dst.enabled_processes
                    .clone_from(&src.enabled_processes);
                dst.enabled_processes
                    .clone_from(&src.enabled_processes);
            }
            _ => unreachable!(),
        }
    }

    fn new_empty_row(&mut self) -> PartStatesRow {
        let enabled_processes = self
            .processes
            .iter()
            .map(|process| (process.clone(), false))
            .collect::<Vec<(ProcessName, bool)>>();

        PartStatesRow {
            part: Part {
                manufacturer: "".to_string(),
                mpn: "".to_string(),
            },
            enabled_processes,
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

    fn on_row_updated(&mut self, row_index: usize, new_row: &PartStatesRow, old_row: &PartStatesRow) {
        trace!(
            "on_row_updated. row_index {}, old_row: {:?}, old_row: {:?}",
            row_index, new_row, old_row
        );
        self.sender
            .send(PartsUiCommand::RowUpdated {
                index: row_index,
                new_row: new_row.clone(),
                old_row: old_row.clone(),
            })
            .expect("sent");
    }

    fn on_row_inserted(&mut self, row_index: usize, row: &PartStatesRow) {
        trace!("on_row_inserted. row_index {}, row: {:?}", row_index, row);

        // should not be possible, since row insertion/deletion is prevented, this is a bug.
        unreachable!();
    }

    fn on_row_removed(&mut self, row_index: usize, row: &PartStatesRow) {
        trace!("on_row_removed. row_index {}, row: {:?}", row_index, row);

        // should not be possible, since row insertion/deletion is prevented, this is a bug.
        unreachable!();
    }

    fn filter_row(&mut self, row: &PartStatesRow) -> bool {
        let processes: String = enabled_processes_to_string(&row.enabled_processes);

        let haystack = format!(
            "manufacturer: '{}', mpn: '{}', processes: {}",
            &row.part.manufacturer, &row.part.mpn, &processes,
        );

        // "Filter single row. If this returns false, the row will be hidden."
        let result = self.filter.matches(haystack.as_str());

        trace!("row: {:?}, haystack: {}, result: {}", row, haystack, result);

        result
    }

    fn row_filter_hash(&mut self) -> &impl std::hash::Hash {
        &self.filter
    }
}

fn enabled_processes_to_string(enabled_processes: &Vec<(ProcessName, bool)>) -> String {
    format!(
        "[{}]",
        enabled_processes
            .iter()
            .filter_map(|(process, enabled)| match enabled {
                true => Some(format!("'{}'", process)),
                false => None,
            })
            .collect::<Vec<_>>()
            .join(", ")
    )
}
