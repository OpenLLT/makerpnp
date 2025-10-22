use std::collections::BTreeMap;

use derivative::Derivative;
use eda_units::eda_units::angle::AngleUnit;
use eda_units::eda_units::dimension_unit::{DimensionUnitPoint2, DimensionUnitPoint2Ext};
use eda_units::eda_units::unit_system::UnitSystem;
use egui::Ui;
use egui_deferred_table::{
    Action, ApplyChange, AxisParameters, CellEditState, CellIndex, DeferredTable, DeferredTableDataSource,
    DeferredTableRenderer, EditableTableRenderer, TableDimensions, apply_reordering, handle_editable_cell_click,
};
use egui_i18n::tr;
use egui_mobius::Value;
use egui_mobius::types::Enqueue;
use planner_app::{
    ObjectPath, PcbSide, PhaseOverview, PhaseReference, PlacementPositionUnit, PlacementState, PlacementStatus,
    PlacementsItem, Reference,
};
use tracing::{debug, info, trace};

use crate::filter::{Filter, FilterUiAction, FilterUiCommand, FilterUiContext};
use crate::i18n::conversions::{
    pcb_side_to_i18n_key, placement_operation_status_to_i18n_key, placement_place_to_i18n_key,
    placement_project_status_to_i18n_key,
};
use crate::ui_component::{ComponentState, UiComponent};

mod columns {
    pub const OBJECT_PATH_COL: usize = 0;
    pub const ORDERING_COL: usize = 1;
    pub const REF_DES_COL: usize = 2;
    pub const PLACE_COL: usize = 3;
    pub const MANUFACTURER_COL: usize = 4;
    pub const MPN_COL: usize = 5;
    pub const ROTATION_COL: usize = 6;
    pub const X_COL: usize = 7;
    pub const Y_COL: usize = 8;
    pub const PCB_SIDE_COL: usize = 9;
    pub const PHASE_COL: usize = 10;
    pub const PLACED_COL: usize = 11;
    pub const STATUS_COL: usize = 12;

    /// count of columns
    pub const COLUMN_COUNT: usize = 13;
}
use columns::*;

#[derive(Debug, Clone)]
pub struct PlacementsDataSource {
    rows: Vec<PlacementsItem>,
}

#[derive(Debug, Clone)]
pub struct PlacementsRenderer {
    phases: Vec<PhaseOverview>,

    rows_to_filter: Vec<usize>,
    row_ordering: Option<Vec<usize>>,
    column_ordering: Option<Vec<usize>>,

    // a cache to allow easy lookup for the 'is_editable_cell' for the 'placed' cell
    phase_placements_editability_map: BTreeMap<PhaseReference, bool>,
    all_phases_pending: bool,

    edit_state: Option<CellEditState<PlacementsItemCellEditState, PlacementsItem>>,
    sender: Enqueue<PlacementsTableUiCommand>,
}

#[derive(Debug, Clone)]
pub enum PlacementsItemCellEditState {
    Placed(PlacementStatus),
    Phase(Option<PhaseReference>),
}

enum PlacementsItemCellEditStateError {
    #[allow(dead_code)]
    None,
}

impl ApplyChange<PlacementsItemCellEditState, PlacementsItemCellEditStateError> for PlacementsItem {
    fn apply_change(&mut self, value: PlacementsItemCellEditState) -> Result<(), PlacementsItemCellEditStateError> {
        match value {
            PlacementsItemCellEditState::Placed(value) => {
                self.state.operation_status = value;
                Ok(())
            }
            PlacementsItemCellEditState::Phase(value) => {
                self.state.phase = value;
                Ok(())
            }
        }
    }
}

impl PlacementsDataSource {
    pub fn new() -> Self {
        Self {
            rows: Default::default(),
        }
    }

    pub fn update_placements(&mut self, placements: Vec<PlacementsItem>) {
        self.rows = placements;
    }
}

impl PlacementsRenderer {
    pub fn update_phases(&mut self, mut phases: Vec<PhaseOverview>) {
        phases.sort_by(|a, b| {
            a.phase_reference
                .cmp(&b.phase_reference)
        });

        self.phase_placements_editability_map = BTreeMap::from_iter(phases.iter().map(|phase| {
            (
                phase.phase_reference.clone(),
                phase
                    .state
                    .can_modify_placements()
                    .is_ok(),
            )
        }));

        self.all_phases_pending = phases
            .iter()
            .all(|phase| phase.state.is_pending());
        self.phases = phases;

        debug!(
            "phases: {:?}, phase_placements_editability_map: {:?}",
            self.phases, self.phase_placements_editability_map
        );
    }
}

impl PlacementsRenderer {
    pub fn new(sender: Enqueue<PlacementsTableUiCommand>) -> Self {
        Self {
            phases: Default::default(),
            rows_to_filter: Default::default(),
            row_ordering: None,
            column_ordering: None,
            all_phases_pending: false,
            phase_placements_editability_map: BTreeMap::new(),
            edit_state: Default::default(),
            sender,
        }
    }
}

impl EditableTableRenderer<PlacementsDataSource> for PlacementsRenderer {
    type Value = PlacementsItem;
    type ItemState = PlacementsItemCellEditState;

    fn build_item_state(
        &self,
        cell_index: CellIndex,
        source: &mut PlacementsDataSource,
    ) -> Option<(PlacementsItemCellEditState, PlacementsItem)> {
        let original_item = &source.rows[cell_index.row];

        match cell_index.column {
            PLACED_COL => {
                let editable = original_item
                    .state
                    .phase
                    .as_ref()
                    .is_some_and(|phase_reference| {
                        self.phase_placements_editability_map
                            .get(phase_reference)
                            .copied()
                            .unwrap_or(false)
                    });

                if !editable {
                    return None;
                }

                original_item
                    .state
                    .phase
                    .as_ref()
                    .map(|_phase| {
                        (
                            PlacementsItemCellEditState::Placed(
                                original_item
                                    .state
                                    .operation_status
                                    .clone(),
                            ),
                            original_item.clone(),
                        )
                    })
            }
            PHASE_COL => {
                let editable = self.all_phases_pending;

                if !editable {
                    return None;
                }

                Some((
                    PlacementsItemCellEditState::Phase(original_item.state.phase.clone()),
                    original_item.clone(),
                ))
            }
            _ => None,
        }
    }

    fn on_edit_complete(
        &mut self,
        cell_index: CellIndex,
        edit_state: Self::ItemState,
        original_item: PlacementsItem,
        source: &mut PlacementsDataSource,
    ) {
        let _ = source;

        self.sender
            .send(PlacementsTableUiCommand::CellEditComplete(
                cell_index,
                edit_state,
                original_item,
            ))
            .expect("sent");
    }

    fn set_edit_state(&mut self, edit_state: CellEditState<Self::ItemState, Self::Value>) {
        self.edit_state.replace(edit_state);
    }

    fn edit_state(&self) -> Option<&CellEditState<Self::ItemState, Self::Value>> {
        self.edit_state.as_ref()
    }

    fn take_edit_state(&mut self) -> CellEditState<Self::ItemState, Self::Value> {
        self.edit_state.take().unwrap()
    }
}

impl DeferredTableDataSource for PlacementsDataSource {
    fn get_dimensions(&self) -> TableDimensions {
        TableDimensions {
            row_count: self.rows.len(),
            column_count: COLUMN_COUNT,
        }
    }
}

impl DeferredTableRenderer<PlacementsDataSource> for PlacementsRenderer {
    fn render_cell(&self, ui: &mut Ui, cell_index: CellIndex, source: &PlacementsDataSource) {
        let row = &source.rows[cell_index.row];

        let handled = match &self.edit_state {
            Some(CellEditState::Editing(selected_cell_index, edit, _original_item))
                if *selected_cell_index == cell_index =>
            {
                match edit {
                    PlacementsItemCellEditState::Placed(value) => {
                        let mut value_mut = value.clone();
                        ui.radio_value(
                            &mut value_mut,
                            PlacementStatus::Pending,
                            tr!(placement_operation_status_to_i18n_key(&PlacementStatus::Pending)),
                        );
                        ui.radio_value(
                            &mut value_mut,
                            PlacementStatus::Placed,
                            tr!(placement_operation_status_to_i18n_key(&PlacementStatus::Placed)),
                        );
                        ui.radio_value(
                            &mut value_mut,
                            PlacementStatus::Skipped,
                            tr!(placement_operation_status_to_i18n_key(&PlacementStatus::Skipped)),
                        );

                        if value_mut != *value {
                            // NOTE: if we had &mut self here, we could apply the edit state now
                            self.sender
                                .send(PlacementsTableUiCommand::ApplyCellEdit {
                                    edit: PlacementsItemCellEditState::Placed(value_mut),
                                    cell_index,
                                })
                                .expect("sent");
                        }

                        true
                    }
                    PlacementsItemCellEditState::Phase(value) => {
                        let mut value_mut = value.clone();

                        ui.add(|ui: &mut Ui| {
                            egui::ComboBox::from_id_salt(ui.id().with("phase").with(cell_index.row))
                                .width(ui.available_width())
                                .selected_text(match &value_mut {
                                    None => tr!("form-common-combo-none"),
                                    Some(phase) => phase.to_string(),
                                })
                                .show_ui(ui, |ui| {
                                    // Note: with the arguments to this method, there is no command we can send that will be able
                                    //       to do anything useful with the row as there is probably no API to access the
                                    //       underlying row instance that is being edited; so we HAVE to edit-in-place here.
                                    if ui
                                        .add(egui::Button::selectable(
                                            value_mut.is_none(),
                                            tr!("form-common-combo-none")
                                        ))
                                        .clicked()
                                    {
                                        value_mut = None;
                                    }

                                    for phase in self.phases.iter()
                                        .filter(|phase| row.state.placement.pcb_side.eq(&phase.pcb_side))
                                    {
                                        if ui
                                            .add(egui::Button::selectable(
                                                matches!(&value_mut, Some(other_phase_reference) if other_phase_reference.eq(&phase.phase_reference)),
                                                phase.phase_reference.to_string(),
                                            ))
                                            .clicked()
                                        {
                                            value_mut = Some(phase.phase_reference.clone());
                                        }
                                    }
                                }).response
                        });

                        if value_mut != *value {
                            // NOTE: if we had &mut self here, we could apply the edit state now
                            self.sender
                                .send(PlacementsTableUiCommand::ApplyCellEdit {
                                    edit: PlacementsItemCellEditState::Phase(value_mut),
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
                OBJECT_PATH_COL => ui.label(&row.path.to_string()),
                REF_DES_COL => ui.label(row.state.placement.ref_des.to_string()),
                PLACE_COL => {
                    let label = tr!(placement_place_to_i18n_key(row.state.placement.place));
                    ui.label(label)
                }
                MANUFACTURER_COL => ui.label(&row.state.placement.part.manufacturer),
                MPN_COL => ui.label(&row.state.placement.part.mpn),
                ROTATION_COL => ui.label(format!("{}", &row.state.unit_position.rotation)),
                X_COL => ui.label(format!("{}", &row.state.unit_position.x)),
                Y_COL => ui.label(format!("{}", &row.state.unit_position.y)),
                PCB_SIDE_COL => {
                    let key = pcb_side_to_i18n_key(&row.state.placement.pcb_side);
                    ui.label(tr!(key))
                }
                PHASE_COL => {
                    let phase = &row
                        .state
                        .phase
                        .clone()
                        .map(|reference: Reference| reference.to_string())
                        .unwrap_or_default();
                    ui.label(phase)
                }
                PLACED_COL => {
                    let label = tr!(placement_operation_status_to_i18n_key(&row.state.operation_status));
                    ui.label(label)
                }
                STATUS_COL => {
                    let label = tr!(placement_project_status_to_i18n_key(&row.state.project_status));
                    ui.label(label)
                }
                ORDERING_COL => ui.label(row.ordering.to_string()),

                _ => unreachable!(),
            };
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
pub struct PlacementsTableUi {
    source: Value<(PlacementsDataSource, PlacementsRenderer)>,
    #[derivative(Debug = "ignore")]
    pub(crate) filter: Filter,

    pub component: ComponentState<PlacementsTableUiCommand>,
}

impl PlacementsTableUi {
    pub fn new() -> Self {
        let component = ComponentState::default();

        let mut filter = Filter::default();
        filter
            .component_state
            .configure_mapper(component.sender.clone(), |filter_ui_command| {
                trace!("filter ui mapper. command: {:?}", filter_ui_command);
                PlacementsTableUiCommand::FilterCommand(filter_ui_command)
            });

        Self {
            source: Value::new((
                PlacementsDataSource::new(),
                PlacementsRenderer::new(component.sender.clone()),
            )),
            filter,

            component,
        }
    }

    pub fn update_placements(&mut self, placements: Vec<PlacementsItem>, phases: Vec<PhaseOverview>) {
        let (source, renderer) = &mut *self.source.lock().unwrap();

        source.update_placements(placements);
        renderer.update_phases(phases);
    }

    pub fn update_phases(&mut self, phases: Vec<PhaseOverview>) {
        let (_source, renderer) = &mut *self.source.lock().unwrap();

        renderer.update_phases(phases);
    }

    pub fn filter_ui(&self, ui: &mut Ui) {
        self.filter
            .ui(ui, &mut FilterUiContext::default());
    }
}

#[derive(Debug, Clone)]
pub enum PlacementsTableUiCommand {
    None,

    // internal
    FilterCommand(FilterUiCommand),
    ApplyCellEdit {
        edit: PlacementsItemCellEditState,
        cell_index: CellIndex,
    },
    CellEditComplete(CellIndex, PlacementsItemCellEditState, PlacementsItem),
    LocatePlacement {
        /// Full object path of the component
        object_path: ObjectPath,
        pcb_side: PcbSide,
        design_position: PlacementPositionUnit,
        unit_position: PlacementPositionUnit,
    },
    NewSelection(Vec<PlacementsItem>),
}

#[derive(Debug, Clone)]
pub enum PlacementsTableUiAction {
    None,
    UpdatePlacement {
        object_path: ObjectPath,
        new_placement: PlacementState,
        old_placement: PlacementState,
    },
    RequestRepaint,
    LocatePlacement {
        /// Full object path of the component
        object_path: ObjectPath,
        pcb_side: PcbSide,
        design_position: PlacementPositionUnit,
        unit_position: PlacementPositionUnit,
    },
    NewSelection(Vec<PlacementsItem>),
}

#[derive(Debug, Clone, Default)]
pub struct PlacementsTableUiContext {}

impl UiComponent for PlacementsTableUi {
    type UiContext<'context> = PlacementsTableUiContext;
    type UiCommand = PlacementsTableUiCommand;
    type UiAction = PlacementsTableUiAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        let (source, renderer) = &mut *self.source.lock().unwrap();

        let (_response, actions) = DeferredTable::new(ui.make_persistent_id("placements_table"))
            .min_size((400.0, 400.0).into())
            .column_parameters(&vec![
                AxisParameters::default()
                    .name(tr!("table-placements-column-object-path"))
                    .default_dimension(200.0),
                AxisParameters::default().name(tr!("table-placements-column-ordering")),
                AxisParameters::default()
                    .name(tr!("table-placements-column-refdes"))
                    .default_dimension(50.0),
                AxisParameters::default().name(tr!("table-placements-column-place")),
                AxisParameters::default()
                    .name(tr!("table-placements-column-manufacturer"))
                    .default_dimension(200.0),
                AxisParameters::default()
                    .name(tr!("table-placements-column-mpn"))
                    .default_dimension(200.0),
                AxisParameters::default().name(tr!("table-placements-column-rotation")),
                AxisParameters::default().name(tr!("table-placements-column-x")),
                AxisParameters::default().name(tr!("table-placements-column-y")),
                AxisParameters::default().name(tr!("table-placements-column-pcb-side")),
                AxisParameters::default().name(tr!("table-placements-column-phase")),
                AxisParameters::default().name(tr!("table-placements-column-placed")),
                AxisParameters::default().name(tr!("table-placements-column-status")),
            ])
            .show(ui, source, renderer);

        for action in actions {
            match action {
                // TODO we need double-click to edit cells, not single-click, then single-click again
                Action::CellClicked(cell_index) => {
                    info!("Cell clicked. cell: {:?}", cell_index);

                    handle_editable_cell_click(source, renderer, cell_index);

                    // FUTURE only do this if a *different* cell is clicked, requires tracking the current cell

                    let row = &source.rows[cell_index.row];

                    self.component
                        .send(PlacementsTableUiCommand::LocatePlacement {
                            object_path: row.path.clone(),
                            pcb_side: row.state.placement.pcb_side.clone(),
                            // FIXME hard-coded use of UnitSystem::Millimeters
                            design_position: PlacementPositionUnit::new(
                                DimensionUnitPoint2::new_dim_decimal(
                                    row.state.placement.x,
                                    row.state.placement.y,
                                    UnitSystem::Millimeters,
                                ),
                                AngleUnit::new_degrees_decimal(row.state.placement.rotation),
                            ),
                            // FIXME hard-coded use of UnitSystem::Millimeters
                            unit_position: PlacementPositionUnit::new(
                                DimensionUnitPoint2::new_dim_decimal(
                                    row.state.unit_position.x,
                                    row.state.unit_position.y,
                                    UnitSystem::Millimeters,
                                ),
                                AngleUnit::new_degrees_decimal(row.state.unit_position.rotation),
                            ),
                        });
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

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            PlacementsTableUiCommand::None => Some(PlacementsTableUiAction::None),
            PlacementsTableUiCommand::FilterCommand(command) => {
                let action = self
                    .filter
                    .update(command, &mut FilterUiContext::default())
                    .inspect(|action| debug!("filter action: {:?}", action));

                match action {
                    Some(FilterUiAction::ApplyFilter) => {
                        let (source, renderer) = &mut *self.source.lock().unwrap();

                        renderer.rows_to_filter = source.rows.iter().enumerate().filter_map(|(id, row)|{

                            let haystack = format!(
                                "object_path: '{}', refdes: '{}', manufacturer: '{}', mpn: '{}', place: {}, placed: {}, side: {}, phase: '{}', status: '{}'",
                                &row.path,
                                &row.state.placement.ref_des,
                                &row.state
                                    .placement
                                    .part
                                    .manufacturer,
                                &row.state.placement.part.mpn,
                                &tr!(placement_place_to_i18n_key(row.state.placement.place)),
                                &tr!(placement_operation_status_to_i18n_key(
                                        &row.state.operation_status
                                    )),
                                &tr!(pcb_side_to_i18n_key(&row.state.placement.pcb_side)),
                                &row.state
                                    .phase
                                    .as_ref()
                                    .map(|phase| phase.to_string())
                                    .unwrap_or_default(),
                                &tr!(placement_project_status_to_i18n_key(
                                        &row.state.project_status
                                    )),
                            );

                            // "Filter single row. If this returns false, the row will be hidden."
                            let result = self.filter.matches(haystack.as_str());

                            trace!("row: {:?}, haystack: {}, result: {}", row, haystack, result);

                            if !result {
                                Some(id)
                            } else {
                                None
                            }
                        }).collect::<Vec<usize>>();

                        Some(PlacementsTableUiAction::RequestRepaint)
                    }
                    None => None,
                }
            }

            PlacementsTableUiCommand::ApplyCellEdit {
                edit: new_edit_state,
                cell_index,
            } => {
                let (_source, renderer) = &mut *self.source.lock().unwrap();
                match renderer.edit_state.as_mut() {
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
            PlacementsTableUiCommand::CellEditComplete(cell_index, edit_state, original_item) => {
                let (source, _renderer) = &mut *self.source.lock().unwrap();
                let row = &mut source.rows[cell_index.row];

                row.apply_change(edit_state)
                    .map(|_| PlacementsTableUiAction::UpdatePlacement {
                        object_path: original_item.path,
                        new_placement: row.state.clone(),
                        old_placement: original_item.state,
                    })
                    .ok()
            }

            PlacementsTableUiCommand::LocatePlacement {
                object_path,
                pcb_side,
                design_position,
                unit_position,
            } => Some(PlacementsTableUiAction::LocatePlacement {
                object_path,
                pcb_side,
                design_position,
                unit_position,
            }),
            PlacementsTableUiCommand::NewSelection(selection) => Some(PlacementsTableUiAction::NewSelection(selection)),
        }
    }
}

//
// Snippets of code remaining to be ported.
//

// fn on_highlight_change(&mut self, highlighted: &[&PlacementsRow], unhighlighted: &[&PlacementsRow]) {
//     trace!(
//             "on_highlight_change. highlighted: {:?}, unhighlighted: {:?}",
//             highlighted, unhighlighted
//         );
//
//     // NOTE: for this to work, this PR is required: https://github.com/kang-sw/egui-data-table/pull/51
//     //       without it, when making a multi-select, this only ever seems to return a slice with one element, perhaps
//     //       egui_data_tables is broken, or perhaps our expectations of how the API is supposed to work are incorrect...
//
//     // FIXME this is extremely expensive, perhaps we could just send some identifier for the selected placements instead
//     //       of the placements themselves?
//
//     let selection = highlighted
//         .iter()
//         .map(|row| PlacementsItem {
//             path: row.object_path.clone(),
//             state: row.placement_state.clone(),
//             ordering: row.ordering,
//         })
//         .collect::<Vec<_>>();
//     self.sender
//         .send(PlacementsTableUiCommand::NewSelection(selection))
//         .expect("sent");
// }
