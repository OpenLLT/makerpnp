use egui::{Color32, Ui};
use egui_deferred_table::{
    Action, CellIndex, DeferredTable, DeferredTableBuilder, DeferredTableDataSource, DeferredTableRenderer,
    TableDimensions,
};
use egui_i18n::tr;
use egui_mobius::Value;
use egui_mobius::types::Enqueue;
use planner_app::{LoadOut, Part};
use tracing::{debug, info, trace};

use crate::filter::{Filter, FilterUiAction, FilterUiCommand, FilterUiContext};
use crate::ui_component::{ComponentState, UiComponent};

const SHOW_DEBUG_SHAPES: bool = false;

#[derive(Debug, Clone)]
pub struct LoadOutRow {
    pub feeder: String,
    pub part: Part,
}

mod columns {
    pub const FEEDER_REFERENCE_COL: usize = 0;
    pub const MANUFACTURER_COL: usize = 1;
    pub const MPN_COL: usize = 2;

    /// count of columns
    pub const COLUMN_COUNT: usize = 3;
}
use columns::*;

#[derive(Debug, Clone)]
pub struct LoadOutDataSource {
    rows: Vec<LoadOutRow>,

    // temporary implementation due to in-progress nature of egui_deferred_table
    cell: Option<CellEditState>,
    sender: Enqueue<LoadOutTableUiCommand>,
}

#[derive(Debug, Clone)]
enum CellEditState {
    // the pivot point for selections, etc.
    Pivot(CellIndex),
    Editing(CellIndex),
}

impl LoadOutDataSource {
    pub fn new(sender: Enqueue<LoadOutTableUiCommand>) -> Self {
        Self {
            rows: Default::default(),
            cell: Default::default(),
            sender,
        }
    }

    pub fn update_loadout(&mut self, mut load_out: LoadOut) {
        self.rows = load_out
            .items
            .drain(0..)
            .map(|item| LoadOutRow {
                part: Part::new(item.manufacturer, item.mpn),
                feeder: item
                    .reference
                    .map_or_else(|| "".to_string(), |reference| reference.to_string()),
            })
            .collect();
    }
}

impl DeferredTableDataSource for LoadOutDataSource {
    fn get_dimensions(&self) -> TableDimensions {
        TableDimensions {
            row_count: self.rows.len(),
            column_count: COLUMN_COUNT,
        }
    }
}

impl DeferredTableRenderer for LoadOutDataSource {
    fn render_cell(&self, ui: &mut Ui, cell_index: CellIndex) {
        let (selected, editing) = match self.cell {
            Some(CellEditState::Pivot(selected_cell_index)) if selected_cell_index == cell_index => (true, false),
            Some(CellEditState::Editing(selected_cell_index)) if selected_cell_index == cell_index => (false, true),
            _ => (false, false),
        };

        let row = &self.rows[cell_index.row];

        let handled = editing && {
            match cell_index.column {
                FEEDER_REFERENCE_COL => {
                    let mut value = row.feeder.clone();
                    if ui
                        .text_edit_singleline(&mut value)
                        .changed()
                    {
                        self.sender
                            .send(LoadOutTableUiCommand::FeederReferenceChanged {
                                value,
                                cell_index,
                            })
                            .expect("sent");
                    }
                    true
                }
                _ => false,
            }
        };

        if !handled {
            match cell_index.column {
                FEEDER_REFERENCE_COL => ui.label(&row.feeder.to_string()),
                MANUFACTURER_COL => ui.label(&row.part.manufacturer),
                MPN_COL => ui.label(&row.part.mpn),
                _ => unreachable!(),
            };
        }

        if SHOW_DEBUG_SHAPES {
            if editing {
                ui.painter()
                    .debug_rect(ui.clip_rect(), Color32::RED, "Edit");
            } else if selected {
                ui.painter()
                    .debug_rect(ui.clip_rect(), Color32::ORANGE, "Pivot");
            }
        }
    }
}

pub struct LoadOutTableUi {
    source: Value<LoadOutDataSource>,
    filter: Filter,

    pub component: ComponentState<LoadOutTableUiCommand>,
}

impl LoadOutTableUi {
    pub fn new() -> Self {
        let component = ComponentState::default();

        let mut filter = Filter::default();
        filter
            .component_state
            .configure_mapper(component.sender.clone(), |filter_ui_command| {
                trace!("filter ui mapper. command: {:?}", filter_ui_command);
                LoadOutTableUiCommand::FilterCommand(filter_ui_command)
            });

        Self {
            source: Value::new(LoadOutDataSource::new(component.sender.clone())),
            filter,
            component,
        }
    }

    pub fn update_loadout(&mut self, load_out: LoadOut) {
        self.source
            .lock()
            .unwrap()
            .update_loadout(load_out);
    }

    pub fn filter_ui(&self, ui: &mut egui::Ui) {
        self.filter
            .ui(ui, &mut FilterUiContext::default());
    }
}

#[derive(Debug, Clone)]
pub enum LoadOutTableUiCommand {
    None,
    FilterCommand(FilterUiCommand),
    FeederReferenceChanged { value: String, cell_index: CellIndex },
    CellEditComplete(CellIndex),
}

#[derive(Debug, Clone)]
pub enum LoadOutTableUiAction {
    None,
    RequestRepaint,
    RowUpdated {
        index: CellIndex,
        new_row: LoadOutRow,
        old_row: LoadOutRow,
    },
}

#[derive(Debug, Clone, Default)]
pub struct LoadOutTableUiContext {}

impl UiComponent for LoadOutTableUi {
    type UiContext<'context> = LoadOutTableUiContext;
    type UiCommand = LoadOutTableUiCommand;
    type UiAction = LoadOutTableUiAction;

    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        let data_source = &mut *self.source.lock().unwrap();

        let (_response, actions) = DeferredTable::new(ui.make_persistent_id("table_1"))
            .min_size((400.0, 400.0).into())
            .show(
                ui,
                data_source,
                |builder: &mut DeferredTableBuilder<'_, LoadOutDataSource>| {
                    builder.header(|header_builder| {
                        header_builder.column(FEEDER_REFERENCE_COL, tr!("table-load-out-column-reference"));
                        header_builder
                            .column(MANUFACTURER_COL, tr!("table-load-out-column-manufacturer"))
                            .default_width(200.0);
                        header_builder
                            .column(MPN_COL, tr!("table-load-out-column-mpn"))
                            .default_width(200.0);
                    })
                },
            );

        for action in actions {
            match action {
                Action::CellClicked(cell_index) => {
                    info!("Cell clicked. cell: {:?}", cell_index);

                    // click once to select, click again to edit

                    match data_source.cell.as_mut() {
                        None => {
                            // change selection
                            data_source
                                .cell
                                .replace(CellEditState::Pivot(cell_index));
                        }
                        Some(CellEditState::Pivot(pivot_cell_index)) if *pivot_cell_index == cell_index => {
                            debug!("clicked in selected cell");

                            // change mode to edit
                            data_source
                                .cell
                                .replace(CellEditState::Editing(cell_index));
                        }
                        Some(CellEditState::Pivot(_)) => {
                            debug!("clicked in different cell");

                            // change selection
                            data_source
                                .cell
                                .replace(CellEditState::Pivot(cell_index));
                        }
                        Some(CellEditState::Editing(editing_cell_index)) if *editing_cell_index == cell_index => {
                            debug!("clicked in cell while editing");

                            // nothing to do
                        }
                        Some(CellEditState::Editing(editing_cell_index)) => {
                            debug!("clicked in a different cell while editing");

                            // apply edited value
                            self.component
                                .sender
                                .send(LoadOutTableUiCommand::CellEditComplete(*editing_cell_index))
                                .expect("sent");

                            // change selection
                            data_source
                                .cell
                                .replace(CellEditState::Pivot(cell_index));
                        }
                    }
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
            LoadOutTableUiCommand::None => Some(LoadOutTableUiAction::None),
            LoadOutTableUiCommand::FilterCommand(command) => {
                let action = self
                    .filter
                    .update(command, &mut FilterUiContext::default())
                    .inspect(|action| debug!("filter action: {:?}", action));

                match action {
                    Some(FilterUiAction::ApplyFilter) => Some(LoadOutTableUiAction::RequestRepaint),
                    None => None,
                }
            }
            LoadOutTableUiCommand::FeederReferenceChanged {
                value,
                cell_index,
            } => {
                let source = &mut *self.source.lock().unwrap();
                let row = &mut source.rows[cell_index.row];
                row.feeder = value;
                debug!("feeder reference changed. row: {:?}", row);
                None
            }
            LoadOutTableUiCommand::CellEditComplete(cell_index) => {
                let source = &mut *self.source.lock().unwrap();
                let row = &mut source.rows[cell_index.row];

                Some(LoadOutTableUiAction::RowUpdated {
                    index: cell_index,
                    new_row: row.clone(),
                    old_row: row.clone(),
                })
            }
        }
    }
}

//
// Snippets of code remaining to be ported.
//

// let haystack = format!(
//     "feeder: {}, manufacturer: '{}', mpn: '{}'",
//     &row.feeder, &row.part.manufacturer, &row.part.mpn,
// );
//
// // "Filter single row. If this returns false, the row will be hidden."
// let result = self.filter.matches(haystack.as_str());
//
// trace!("row: {:?}, haystack: {}, result: {}", row, haystack, result);
