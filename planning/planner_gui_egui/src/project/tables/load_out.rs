use egui::Ui;
use egui_deferred_table::{
    Action, CellIndex, DeferredTable, DeferredTableBuilder, DeferredTableDataSource, DeferredTableRenderer,
    TableDimensions,
};
use egui_i18n::tr;
use egui_mobius::Value;
use planner_app::{LoadOut, Part};
use tracing::{debug, info, trace};

use crate::filter::{Filter, FilterUiAction, FilterUiCommand, FilterUiContext};
use crate::ui_component::{ComponentState, UiComponent};

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

#[derive(Debug, Clone, Default)]
pub struct LoadOutDataSource {
    rows: Vec<LoadOutRow>,
}

impl LoadOutDataSource {
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
        let row = &self.rows[cell_index.row];
        match cell_index.column {
            FEEDER_REFERENCE_COL => ui.label(&row.feeder.to_string()),
            MANUFACTURER_COL => ui.label(&row.part.manufacturer),
            MPN_COL => ui.label(&row.part.mpn),
            _ => unreachable!(),
        };
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
            source: Value::new(LoadOutDataSource::default()),
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
}

#[derive(Debug, Clone)]
pub enum LoadOutTableUiAction {
    None,
    RequestRepaint,
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
                    info!("Cell clicked. cell: {:?}", cell_index)
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
        }
    }
}

//
// Snippets of code remaining to be ported.
//

// 0 => {
//     // FIXME this is directly modifying state
//     let mut reference = row.feeder.clone().to_string();
//     if ui
//         .text_edit_singleline(&mut reference)
//         .changed()
//     {
//         row.feeder = reference;
//     }
//
//     Some(ui.response())
// }
// 1 => None,
// 2 => None,

// trace!(
//     "on_row_updated. row_index {}, old_row: {:?}, old_row: {:?}",
//     row_index, new_row, old_row
// );
// self.sender
//     .send(LoadOutTabUiCommand::RowUpdated {
//         index: row_index,
//         new_row: new_row.clone(),
//         old_row: old_row.clone(),
//     })
//     .expect("sent");

// let haystack = format!(
//     "feeder: {}, manufacturer: '{}', mpn: '{}'",
//     &row.feeder, &row.part.manufacturer, &row.part.mpn,
// );
//
// // "Filter single row. If this returns false, the row will be hidden."
// let result = self.filter.matches(haystack.as_str());
//
// trace!("row: {:?}, haystack: {}, result: {}", row, haystack, result);
