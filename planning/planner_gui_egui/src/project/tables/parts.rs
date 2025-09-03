use egui::Ui;
use egui_deferred_table::{
    Action, CellIndex, DeferredTable, DeferredTableBuilder, DeferredTableDataSource, DeferredTableRenderer,
    TableDimensions,
};
use egui_i18n::tr;
use egui_mobius::Value;
use egui_mobius::types::Enqueue;
use planner_app::{PartStates, PartWithState, ProcessReference};
use tracing::{debug, info, trace};

use crate::filter::{Filter, FilterUiAction, FilterUiCommand, FilterUiContext};
use crate::project::tables::{ApplyChange, CellEditState, EditableDataSource, handle_cell_click};
use crate::ui_component::{ComponentState, UiComponent};

mod columns {
    pub const MANUFACTURER_COL: usize = 0;
    pub const MPN_COL: usize = 1;
    pub const QUANTITY_COL: usize = 2;
    pub const PROCESSES_COL: usize = 3;
    pub const REF_DES_SET_COL: usize = 4;

    /// count of columns
    pub const COLUMN_COUNT: usize = 5;
}
use columns::*;

#[derive(Debug, Clone)]
pub struct PartDataSource {
    rows: Vec<PartWithState>,

    processes: Vec<ProcessReference>,

    // temporary implementation due to in-progress nature of egui_deferred_table
    cell: Option<CellEditState<PartCellEditState, PartWithState>>,
    sender: Enqueue<PartTableUiCommand>,
}

impl PartDataSource {}

#[derive(Debug, Clone)]
pub enum PartCellEditState {
    Processes(Vec<(ProcessReference, bool)>),
}

enum PartCellEditStateError {
    #[allow(dead_code)]
    None,
}

impl ApplyChange<PartCellEditState, PartCellEditStateError> for PartWithState {
    fn apply_change(&mut self, value: PartCellEditState) -> Result<(), PartCellEditStateError> {
        match value {
            PartCellEditState::Processes(value) => {
                self.processes = value
                    .into_iter()
                    .filter_map(|(process, enabled)| if enabled { Some(process.clone()) } else { None })
                    .collect();

                Ok(())
            }
        }
    }
}

impl PartDataSource {
    pub fn new(sender: Enqueue<PartTableUiCommand>, mut processes: Vec<ProcessReference>) -> Self {
        // sorting the processes here helps to ensure that the view vs edit list of processes has the same
        // ordering.
        processes.sort();

        Self {
            rows: Default::default(),

            processes,
            cell: Default::default(),
            sender,
        }
    }

    pub fn update_parts(&mut self, mut part_states: PartStates) {
        self.rows = part_states.parts.drain(..).collect();
    }
}

impl EditableDataSource for PartDataSource {
    type Value = PartWithState;
    type ItemState = PartCellEditState;

    fn build_item_state(&self, cell_index: CellIndex) -> Option<(PartCellEditState, PartWithState)> {
        let original_item = &self.rows[cell_index.row];

        match cell_index.column {
            PROCESSES_COL => {
                let enabled_processes = self
                    .processes
                    .iter()
                    .map(|process| {
                        (
                            process.clone(),
                            original_item
                                .processes
                                .contains(process),
                        )
                    })
                    .collect::<Vec<(ProcessReference, bool)>>();

                Some((PartCellEditState::Processes(enabled_processes), original_item.clone()))
            }
            _ => None,
        }
    }

    fn on_edit_complete(&mut self, index: CellIndex, state: PartCellEditState, original_item: PartWithState) {
        self.sender
            .send(PartTableUiCommand::CellEditComplete(index, state, original_item))
            .expect("sent");
    }

    fn set_edit_state(&mut self, edit_state: CellEditState<Self::ItemState, Self::Value>) {
        self.cell.replace(edit_state);
    }

    fn edit_state(&self) -> Option<&CellEditState<Self::ItemState, Self::Value>> {
        self.cell.as_ref()
    }

    fn take_edit_state(&mut self) -> CellEditState<Self::ItemState, Self::Value> {
        self.cell.take().unwrap()
    }
}

impl DeferredTableDataSource for PartDataSource {
    fn get_dimensions(&self) -> TableDimensions {
        TableDimensions {
            row_count: self.rows.len(),
            column_count: COLUMN_COUNT,
        }
    }
}

impl DeferredTableRenderer for PartDataSource {
    fn render_cell(&self, ui: &mut Ui, cell_index: CellIndex) {
        let row = &self.rows[cell_index.row];

        let handled = match &self.cell {
            Some(CellEditState::Editing(selected_cell_index, edit, _original_item))
                if *selected_cell_index == cell_index =>
            {
                match edit {
                    PartCellEditState::Processes(enabled_processes) => {
                        let mut enabled_processes_mut = enabled_processes.clone();
                        let _response = ui.add(|ui: &mut Ui| {
                            // FIXME this doesn't always fit in the available space, what to do?
                            ui.horizontal(|ui| {
                                // Note that the enabled_processes was built in the same order as self.processes.
                                for (name, enabled) in enabled_processes_mut.iter_mut() {
                                    ui.checkbox(enabled, name.to_string());
                                }
                            })
                            .response
                        });

                        if enabled_processes_mut != *enabled_processes {
                            // NOTE: if we had &mut self here, we could apply the edit state now
                            self.sender
                                .send(PartTableUiCommand::ApplyCellEdit {
                                    edit: PartCellEditState::Processes(enabled_processes_mut),
                                    cell_index,
                                })
                                .expect("sent");
                        }

                        true
                    }
                }
            }
            _ => false,
        };

        if !handled {
            let _ = match cell_index.column {
                MANUFACTURER_COL => ui.label(&row.part.manufacturer),
                MPN_COL => ui.label(&row.part.mpn),
                PROCESSES_COL => {
                    // Build in the same order as self.processes, specifically not just iterating over `row.processes`
                    let processes = self
                        .processes
                        .iter()
                        .filter(|process| row.processes.contains(process))
                        .map(|process| process.to_string())
                        .collect::<Vec<String>>();

                    let processes_label: String = processes.join(", ");
                    ui.label(processes_label)
                }
                REF_DES_SET_COL => {
                    let label: String = row
                        .ref_des_set
                        .iter()
                        .cloned()
                        .map(|meh| meh.to_string())
                        .collect::<Vec<String>>()
                        .join(", ");
                    ui.label(label)
                }
                QUANTITY_COL => ui.label(format!("{}", row.quantity)),
                _ => unreachable!(),
            };
        }
    }
}

pub struct PartTableUi {
    source: Value<PartDataSource>,
    filter: Filter,

    pub component: ComponentState<PartTableUiCommand>,
}

impl PartTableUi {
    pub fn new(processes: Vec<ProcessReference>) -> Self {
        let component = ComponentState::default();

        let mut filter = Filter::default();
        filter
            .component_state
            .configure_mapper(component.sender.clone(), |filter_ui_command| {
                trace!("filter ui mapper. command: {:?}", filter_ui_command);
                PartTableUiCommand::FilterCommand(filter_ui_command)
            });

        Self {
            source: Value::new(PartDataSource::new(component.sender.clone(), processes)),
            filter,
            component,
        }
    }

    pub fn update_processes(&mut self, processes: Vec<ProcessReference>) {
        self.source.lock().unwrap().processes = processes;
    }

    pub fn update_parts(&mut self, part_states: PartStates) {
        self.source
            .lock()
            .unwrap()
            .update_parts(part_states);
    }

    pub fn filter_ui(&self, ui: &mut Ui) {
        self.filter
            .ui(ui, &mut FilterUiContext::default());
    }
}

#[derive(Debug, Clone)]
pub enum PartTableUiCommand {
    None,
    FilterCommand(FilterUiCommand),
    ApplyCellEdit {
        edit: PartCellEditState,
        cell_index: CellIndex,
    },
    CellEditComplete(CellIndex, PartCellEditState, PartWithState),
}

#[derive(Debug, Clone)]
pub enum PartTableUiAction {
    None,
    RequestRepaint,
    ItemUpdated {
        index: CellIndex,
        item: PartWithState,
        original_item: PartWithState,
    },
}

#[derive(Debug, Clone, Default)]
pub struct PartTableUiContext {}

impl UiComponent for PartTableUi {
    type UiContext<'context> = PartTableUiContext;
    type UiCommand = PartTableUiCommand;
    type UiAction = PartTableUiAction;

    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        let data_source = &mut *self.source.lock().unwrap();

        let (_response, actions) = DeferredTable::new(ui.make_persistent_id("parts_table"))
            .min_size((400.0, 400.0).into())
            .show(
                ui,
                data_source,
                |builder: &mut DeferredTableBuilder<'_, PartDataSource>| {
                    builder.header(|header_builder| {
                        header_builder
                            .column(MANUFACTURER_COL, tr!("table-parts-column-manufacturer"))
                            .default_width(200.0);
                        header_builder
                            .column(MPN_COL, tr!("table-parts-column-mpn"))
                            .default_width(200.0);
                        header_builder
                            .column(QUANTITY_COL, tr!("table-parts-column-quantity"))
                            .default_width(100.0);
                        header_builder
                            .column(PROCESSES_COL, tr!("table-parts-column-processes"))
                            .default_width(100.0);
                        header_builder
                            .column(REF_DES_SET_COL, tr!("table-parts-column-ref-des-set"))
                            .default_width(100.0);
                    })
                },
            );

        for action in actions {
            match action {
                // TODO we need double-click to edit cells, not single-click, then single-click again
                Action::CellClicked(cell_index) => {
                    info!("Cell clicked. cell: {:?}", cell_index);

                    handle_cell_click(data_source, cell_index);
                }
            }
        }
    }

    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            PartTableUiCommand::None => Some(PartTableUiAction::None),
            PartTableUiCommand::FilterCommand(command) => {
                let action = self
                    .filter
                    .update(command, &mut FilterUiContext::default())
                    .inspect(|action| debug!("filter action: {:?}", action));

                match action {
                    Some(FilterUiAction::ApplyFilter) => Some(PartTableUiAction::RequestRepaint),
                    None => None,
                }
            }

            PartTableUiCommand::ApplyCellEdit {
                edit: new_edit_state,
                cell_index,
            } => {
                let source = &mut *self.source.lock().unwrap();
                match source.cell.as_mut() {
                    Some(CellEditState::Editing(current_cell_index, current_edit_state, _original_item))
                        if *current_cell_index == cell_index =>
                    {
                        debug!("edit state changed. cell: {:?}, edit: {:?}", cell_index, new_edit_state);
                        *current_edit_state = new_edit_state;
                    }
                    _ => {}
                }
                None
            }
            PartTableUiCommand::CellEditComplete(cell_index, edit_state, original_item) => {
                let source = &mut *self.source.lock().unwrap();
                let row = &mut source.rows[cell_index.row];

                row.apply_change(edit_state)
                    .map(|_| PartTableUiAction::ItemUpdated {
                        index: cell_index,
                        item: row.clone(),
                        original_item,
                    })
                    .ok()
            }
        }
    }
}

//
// Snippets of code remaining to be ported.
//
// fn filter_row(&mut self, row: &PartStatesRow) -> bool {
//     let processes: String = enabled_processes_to_string(&row.enabled_processes);
//
//     let haystack = format!(
//         "manufacturer: '{}', mpn: '{}', processes: {}",
//         &row.part.manufacturer, &row.part.mpn, &processes,
//     );
//
//     // "Filter single row. If this returns false, the row will be hidden."
//     let result = self.filter.matches(haystack.as_str());
//
//     trace!("row: {:?}, haystack: {}, result: {}", row, haystack, result);
//
//     result
// }
//
// fn on_highlight_change(&mut self, highlighted: &[&PartStatesRow], unhighlighted: &[&PartStatesRow]) {
//     trace!(
//             "on_highlight_change. highlighted: {:?}, unhighlighted: {:?}",
//             highlighted, unhighlighted
//         );
//
//     // NOTE: for this to work, this PR is required: https://github.com/kang-sw/egui-data-table/pull/51
//     //       without it, when making a multi-select, this only ever seems to return a slice with one element, perhaps
//     //       egui_data_tables is broken, or perhaps our expectations of how the API is supposed to work are incorrect...
//
//     // FIXME this is extremely expensive, perhaps we could just send some identifier for the selected parts instead
//     //       of the parts themselves?
//
//     let parts = highlighted
//         .iter()
//         .map(|row| row.part.clone())
//         .collect::<Vec<_>>();
//     self.sender
//         .send(PartsTabUiCommand::NewSelection(parts))
//         .expect("sent");
// }
