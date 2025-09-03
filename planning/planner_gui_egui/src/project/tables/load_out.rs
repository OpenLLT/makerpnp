use egui::{Color32, CornerRadius, Stroke, StrokeKind, Ui};
use egui_deferred_table::{
    Action, CellIndex, DeferredTable, DeferredTableBuilder, DeferredTableDataSource, DeferredTableRenderer,
    TableDimensions,
};
use egui_i18n::tr;
use egui_mobius::Value;
use egui_mobius::types::Enqueue;
use planner_app::{LoadOut, LoadOutItem, Reference};
use tracing::{debug, info, trace};

use crate::filter::{Filter, FilterUiAction, FilterUiCommand, FilterUiContext};
use crate::project::tables::{ApplyChange, CellEditState, EditableDataSource, handle_cell_click};
use crate::ui_component::{ComponentState, UiComponent};

const SHOW_DEBUG_SHAPES: bool = false;

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
    rows: Vec<LoadOutItem>,

    // temporary implementation due to in-progress nature of egui_deferred_table
    cell: Option<CellEditState<LoadoutItemCellEditState, LoadOutItem>>,
    sender: Enqueue<LoadOutTableUiCommand>,
}

#[derive(Debug, Clone)]
pub enum LoadoutItemCellEditState {
    FeederReference(String),
}

enum LoadoutItemCellEditStateError {
    InvalidFeederReference,
}

impl ApplyChange<LoadoutItemCellEditState, LoadoutItemCellEditStateError> for LoadOutItem {
    fn apply_change(&mut self, value: LoadoutItemCellEditState) -> Result<(), LoadoutItemCellEditStateError> {
        match value {
            LoadoutItemCellEditState::FeederReference(value) => {
                if value.is_empty() {
                    self.reference = None;
                    Ok(())
                } else {
                    Reference::try_from(value)
                        .map(|reference| {
                            self.reference = Some(reference);
                        })
                        .map_err(|_| LoadoutItemCellEditStateError::InvalidFeederReference)
                }
            }
        }
    }
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
        self.rows = load_out.items.drain(..).collect();
    }
}

impl EditableDataSource for LoadOutDataSource {
    type Value = LoadOutItem;
    type ItemState = LoadoutItemCellEditState;

    fn build_edit_state(&self, cell_index: CellIndex) -> Option<(LoadoutItemCellEditState, LoadOutItem)> {
        let original_item = &self.rows[cell_index.row];

        match cell_index.column {
            FEEDER_REFERENCE_COL => Some((
                LoadoutItemCellEditState::FeederReference(
                    original_item
                        .reference
                        .as_ref()
                        .map_or("".to_string(), |value| value.to_string()),
                ),
                original_item.clone(),
            )),
            _ => None,
        }
    }

    fn on_edit_complete(&mut self, cell_index: CellIndex, edit_state: Self::ItemState, original_item: LoadOutItem) {
        self.sender
            .send(LoadOutTableUiCommand::CellEditComplete(
                cell_index,
                edit_state,
                original_item,
            ))
            .expect("sent");
    }

    fn set_edit_state(&mut self, edit_state: CellEditState<Self::ItemState, Self::Value>) {
        self.cell.replace(edit_state);
    }

    fn edit_state(&self) -> Option<&CellEditState<Self::ItemState, Self::Value>> {
        self.cell.as_ref()
    }

    fn take_state(&mut self) -> CellEditState<Self::ItemState, Self::Value> {
        self.cell.take().unwrap()
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

        let handled = match &self.cell {
            Some(CellEditState::Editing(selected_cell_index, edit, _original_item))
                if *selected_cell_index == cell_index =>
            {
                match edit {
                    LoadoutItemCellEditState::FeederReference(value) => {
                        if !value.is_empty() {
                            Reference::try_from(value.clone())
                                .inspect_err(|_error| {
                                    // FUTURE the error could be shown in a tool-tip
                                    let validation_error_stroke = Stroke::new(1.0, Color32::RED);
                                    ui.painter().rect_stroke(
                                        ui.max_rect().shrink(1.0),
                                        CornerRadius::ZERO,
                                        validation_error_stroke,
                                        StrokeKind::Inside,
                                    );
                                })
                                .ok();
                        }

                        let mut value = value.clone();
                        if ui
                            .text_edit_singleline(&mut value)
                            .changed()
                        {
                            // NOTE: if we had &mut self here, we could apply the edit state now
                            self.sender
                                .send(LoadOutTableUiCommand::ApplyCellEdit {
                                    edit: LoadoutItemCellEditState::FeederReference(value),
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
            match cell_index.column {
                FEEDER_REFERENCE_COL => ui.label(
                    row.reference
                        .as_ref()
                        .map_or("".to_string(), |value| value.to_string()),
                ),
                MANUFACTURER_COL => ui.label(&row.manufacturer),
                MPN_COL => ui.label(&row.mpn),
                _ => unreachable!(),
            };
        }

        if SHOW_DEBUG_SHAPES {
            match self.cell {
                Some(CellEditState::Pivot(selected_cell_index)) if selected_cell_index == cell_index => {
                    ui.painter()
                        .debug_rect(ui.clip_rect(), Color32::ORANGE, "Pivot");
                }
                Some(CellEditState::Editing(selected_cell_index, _, _)) if selected_cell_index == cell_index => {
                    ui.painter()
                        .debug_rect(ui.clip_rect(), Color32::RED, "Edit");
                }
                _ => {}
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

    pub fn filter_ui(&self, ui: &mut Ui) {
        self.filter
            .ui(ui, &mut FilterUiContext::default());
    }
}

#[derive(Debug, Clone)]
pub enum LoadOutTableUiCommand {
    None,
    FilterCommand(FilterUiCommand),
    ApplyCellEdit {
        edit: LoadoutItemCellEditState,
        cell_index: CellIndex,
    },
    CellEditComplete(CellIndex, LoadoutItemCellEditState, LoadOutItem),
}

#[derive(Debug, Clone)]
pub enum LoadOutTableUiAction {
    None,
    RequestRepaint,
    ItemUpdated {
        index: CellIndex,
        item: LoadOutItem,
        original_item: LoadOutItem,
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

        let (_response, actions) = DeferredTable::new(ui.make_persistent_id("load_out_table"))
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

            LoadOutTableUiCommand::ApplyCellEdit {
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
            LoadOutTableUiCommand::CellEditComplete(cell_index, edit_state, original_item) => {
                let source = &mut *self.source.lock().unwrap();
                let row = &mut source.rows[cell_index.row];

                row.apply_change(edit_state)
                    .map(|_| LoadOutTableUiAction::ItemUpdated {
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

// let haystack = format!(
//     "feeder: {}, manufacturer: '{}', mpn: '{}'",
//     &row.feeder, &row.part.manufacturer, &row.part.mpn,
// );
//
// // "Filter single row. If this returns false, the row will be hidden."
// let result = self.filter.matches(haystack.as_str());
//
// trace!("row: {:?}, haystack: {}, result: {}", row, haystack, result);
