use derivative::Derivative;
use egui::{Color32, CornerRadius, Stroke, StrokeKind, Ui};
use egui_deferred_table::{
    Action, AxisParameters, CellIndex, DeferredTable, DeferredTableDataSource, DeferredTableRenderer, TableDimensions,
    apply_reordering,
};
use egui_i18n::tr;
use egui_mobius::Value;
use egui_mobius::types::Enqueue;
use planner_app::{LoadOut, LoadOutItem, Reference};
use tracing::{debug, info, trace};

use crate::filter::{Filter, FilterUiAction, FilterUiCommand, FilterUiContext};
use crate::project::tables::{ApplyChange, CellEditState, EditableTableRenderer, handle_cell_click};
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
}

#[derive(Debug)]
struct LoadOutRenderer {
    rows_to_filter: Vec<usize>,
    row_ordering: Option<Vec<usize>>,
    column_ordering: Option<Vec<usize>>,

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
    pub fn new() -> Self {
        Self {
            rows: Default::default(),
        }
    }

    pub fn update_loadout(&mut self, mut load_out: LoadOut) {
        self.rows = load_out.items.drain(..).collect();
    }
}

impl LoadOutRenderer {
    pub fn new(sender: Enqueue<LoadOutTableUiCommand>) -> Self {
        Self {
            rows_to_filter: Default::default(),
            row_ordering: None,
            column_ordering: None,
            cell: Default::default(),
            sender,
        }
    }
}

impl EditableTableRenderer<LoadOutDataSource> for LoadOutRenderer {
    type Value = LoadOutItem;
    type ItemState = LoadoutItemCellEditState;

    fn build_item_state(
        &self,
        cell_index: CellIndex,
        source: &mut LoadOutDataSource,
    ) -> Option<(LoadoutItemCellEditState, LoadOutItem)> {
        let original_item = &source.rows[cell_index.row];

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

    fn on_edit_complete(
        &mut self,
        cell_index: CellIndex,
        edit_state: Self::ItemState,
        original_item: LoadOutItem,
        source: &mut LoadOutDataSource,
    ) {
        let _ = source;

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

    fn take_edit_state(&mut self) -> CellEditState<Self::ItemState, Self::Value> {
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

impl DeferredTableRenderer<LoadOutDataSource> for LoadOutRenderer {
    fn render_cell(&self, ui: &mut Ui, cell_index: CellIndex, source: &LoadOutDataSource) {
        let row = &source.rows[cell_index.row];

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

    fn rows_to_filter(&self) -> Option<&[usize]> {
        Some(self.rows_to_filter.as_slice())
    }

    fn row_ordering(&self) -> Option<&[usize]> {
        self.row_ordering
            .as_ref()
            .map(|v| v.as_slice())
    }

    fn column_ordering(&self) -> Option<&[usize]> {
        self.column_ordering
            .as_ref()
            .map(|v| v.as_slice())
    }
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct LoadOutTableUi {
    table_state: Value<(LoadOutDataSource, LoadOutRenderer)>,
    #[derivative(Debug = "ignore")]
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
            table_state: Value::new((LoadOutDataSource::new(), LoadOutRenderer::new(component.sender.clone()))),
            filter,
            component,
        }
    }

    pub fn update_loadout(&mut self, load_out: LoadOut) {
        let (source, _renderer) = &mut *self.table_state.lock().unwrap();

        source.update_loadout(load_out);
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
        let (source, renderer) = &mut *self.table_state.lock().unwrap();

        let (_response, actions) = DeferredTable::new(ui.make_persistent_id("load_out_table"))
            .min_size((400.0, 400.0).into())
            .column_parameters(&vec![
                AxisParameters::default()
                    .default_dimension(200.0)
                    .name(tr!("table-load-out-column-manufacturer")),
                AxisParameters::default()
                    .default_dimension(200.0)
                    .name(tr!("table-load-out-column-mpn")),
            ])
            .show(ui, source, renderer);

        for action in actions {
            match action {
                // TODO we need double-click to edit cells, not single-click, then single-click again
                Action::CellClicked(cell_index) => {
                    info!("Cell clicked. cell: {:?}", cell_index);

                    handle_cell_click(source, renderer, cell_index);
                }
                Action::ColumnReorder {
                    from,
                    to,
                } => {
                    apply_reordering(&mut renderer.column_ordering, from, to);
                }
                Action::RowReorder {
                    from,
                    to,
                } => {
                    apply_reordering(&mut renderer.column_ordering, from, to);
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
                    Some(FilterUiAction::ApplyFilter) => {
                        let (source, renderer) = &mut *self.table_state.lock().unwrap();

                        renderer.rows_to_filter = source
                            .rows
                            .iter()
                            .enumerate()
                            .filter_map(|(id, row)| {
                                let haystack = format!(
                                    "feeder: '{}', manufacturer: '{}', mpn: '{}'",
                                    row.reference
                                        .as_ref()
                                        .map_or("".to_string(), |it| it.to_string()),
                                    row.manufacturer,
                                    row.mpn,
                                );

                                // "Filter single row. If this returns false, the row will be hidden."
                                let result = self.filter.matches(haystack.as_str());

                                trace!("row: {:?}, haystack: {}, result: {}", row, haystack, result);

                                if !result { Some(id) } else { None }
                            })
                            .collect::<Vec<usize>>();

                        Some(LoadOutTableUiAction::RequestRepaint)
                    }
                    None => None,
                }
            }

            LoadOutTableUiCommand::ApplyCellEdit {
                edit: new_edit_state,
                cell_index,
            } => {
                let (_source, renderer) = &mut *self.table_state.lock().unwrap();
                match renderer.cell.as_mut() {
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
                let (source, _renderer) = &mut *self.table_state.lock().unwrap();
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
