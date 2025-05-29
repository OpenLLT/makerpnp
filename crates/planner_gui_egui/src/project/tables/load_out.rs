use std::borrow::Cow;

use egui::{Response, Ui};
use egui_data_table::RowViewer;
use egui_data_table::viewer::{CellWriteContext, TableColumnConfig};
use egui_i18n::tr;
use egui_mobius::types::Enqueue;
use planner_app::Part;
use tracing::{debug, trace};

use crate::filter::Filter;
use crate::project::load_out_tab::LoadOutTabUiCommand;

#[derive(Debug, Clone)]
pub struct LoadOutRow {
    pub feeder: String,
    pub part: Part,
}

pub struct LoadOutRowViewer {
    sender: Enqueue<LoadOutTabUiCommand>,

    pub(crate) filter: Filter,
}

impl LoadOutRowViewer {
    pub fn new(sender: Enqueue<LoadOutTabUiCommand>) -> Self {
        let mut filter = Filter::default();
        filter
            .component_state
            .configure_mapper(sender.clone(), |filter_ui_command| {
                trace!("filter ui mapper. command: {:?}", filter_ui_command);
                LoadOutTabUiCommand::FilterCommand(filter_ui_command)
            });

        Self {
            sender,
            filter,
        }
    }
}

impl RowViewer<LoadOutRow> for LoadOutRowViewer {
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

    fn is_editable_cell(&mut self, column: usize, _row: usize, _row_value: &LoadOutRow) -> bool {
        column == 0
    }

    fn allow_row_insertions(&mut self) -> bool {
        false
    }

    fn allow_row_deletions(&mut self) -> bool {
        false
    }

    fn compare_cell(&self, row_l: &LoadOutRow, row_r: &LoadOutRow, column: usize) -> std::cmp::Ordering {
        match column {
            0 => row_l.feeder.cmp(&row_r.feeder),
            1 => row_l
                .part
                .manufacturer
                .cmp(&row_r.part.manufacturer),
            2 => row_l.part.mpn.cmp(&row_r.part.mpn),
            _ => unreachable!(),
        }
    }

    fn column_name(&mut self, column: usize) -> Cow<'static, str> {
        match column {
            0 => tr!("table-load-out-column-reference"),
            1 => tr!("table-load-out-column-manufacturer"),
            2 => tr!("table-load-out-column-mpn"),
            _ => unreachable!(),
        }
        .into()
    }

    fn show_cell_view(&mut self, ui: &mut Ui, row: &LoadOutRow, column: usize) {
        let _ = match column {
            0 => ui.label(&row.feeder.to_string()),
            1 => ui.label(&row.part.manufacturer),
            2 => ui.label(&row.part.mpn),
            _ => unreachable!(),
        };
    }

    fn show_cell_editor(&mut self, ui: &mut Ui, row: &mut LoadOutRow, column: usize) -> Option<Response> {
        match column {
            0 => {
                // FIXME this is directly modifying state
                let mut reference = row.feeder.clone().to_string();
                if ui
                    .text_edit_singleline(&mut reference)
                    .changed()
                {
                    row.feeder = reference;
                }

                Some(ui.response())
            }
            1 => None,
            2 => None,
            _ => unreachable!(),
        }
    }

    fn set_cell_value(&mut self, src: &LoadOutRow, dst: &mut LoadOutRow, column: usize) {
        match column {
            0 => dst.feeder.clone_from(&src.feeder),
            1 => dst
                .part
                .manufacturer
                .clone_from(&src.part.manufacturer),
            2 => dst.part.mpn.clone_from(&src.part.mpn),
            _ => unreachable!(),
        }
    }

    fn new_empty_row(&mut self) -> LoadOutRow {
        LoadOutRow {
            part: Part {
                manufacturer: "".to_string(),
                mpn: "".to_string(),
            },
            feeder: "".to_string(),
        }
    }

    fn confirm_cell_write_by_ui(
        &mut self,
        _current: &LoadOutRow,
        _next: &LoadOutRow,
        column: usize,
        _context: CellWriteContext,
    ) -> bool {
        debug!(
            "confirm cell write by ui. column: {}, current: {:?}, next: {:?}, context: {:?}",
            column, _current, _next, _context
        );
        match column {
            0 => {
                // TODO validate the feeder reference here?
                true
            }
            1 => false,
            2 => false,
            _ => unreachable!(),
        }
    }

    fn confirm_row_deletion_by_ui(&mut self, _row: &LoadOutRow) -> bool {
        false
    }

    fn on_row_updated(&mut self, row_index: usize, new_row: &LoadOutRow, old_row: &LoadOutRow) {
        trace!(
            "on_row_updated. row_index {}, old_row: {:?}, old_row: {:?}",
            row_index, new_row, old_row
        );
        self.sender
            .send(LoadOutTabUiCommand::RowUpdated {
                index: row_index,
                new_row: new_row.clone(),
                old_row: old_row.clone(),
            })
            .expect("sent");
    }

    fn on_row_inserted(&mut self, row_index: usize, row: &LoadOutRow) {
        trace!("on_row_inserted. row_index {}, row: {:?}", row_index, row);

        // should not be possible, since row insertion/deletion is prevented, this is a bug.
        unreachable!();
    }

    fn on_row_removed(&mut self, row_index: usize, row: &LoadOutRow) {
        trace!("on_row_removed. row_index {}, row: {:?}", row_index, row);

        // should not be possible, since row insertion/deletion is prevented, this is a bug.
        unreachable!();
    }

    fn filter_row(&mut self, row: &LoadOutRow) -> bool {
        let haystack = format!(
            "feeder: {}, manufacturer: '{}', mpn: '{}'",
            &row.feeder, &row.part.manufacturer, &row.part.mpn,
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
