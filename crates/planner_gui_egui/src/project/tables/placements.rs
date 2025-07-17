use std::borrow::Cow;
use std::sync::Arc;

use derivative::Derivative;
use egui::scroll_area::ScrollBarVisibility;
use egui::{Response, Ui};
use egui_data_table::viewer::{CellWriteContext, TableColumnConfig};
use egui_data_table::{DataTable, RowViewer};
use egui_i18n::tr;
use egui_mobius::Value;
use egui_mobius::types::Enqueue;
use planner_app::{
    ObjectPath, Part, PcbSide, PhaseOverview, Placement, PlacementState, PlacementStatus, PlacementsItem,
    ProjectPlacementStatus, Reference,
};
use tracing::{debug, trace};

use crate::filter::{Filter, FilterUiAction, FilterUiCommand, FilterUiContext};
use crate::i18n::conversions::{
    pcb_side_to_i18n_key, placement_operation_status_to_i18n_key, placement_place_to_i18n_key,
    placement_project_status_to_i18n_key,
};
use crate::i18n::datatable_support::FluentTranslator;
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct PlacementsTableUi {
    #[derivative(Debug = "ignore")]
    placements_table: Value<Option<(PlacementsRowViewer, DataTable<PlacementsRow>)>>,

    pub component: ComponentState<PlacementsTableUiCommand>,
}

impl PlacementsTableUi {
    pub fn new() -> Self {
        Self {
            placements_table: Value::default(),
            component: Default::default(),
        }
    }

    pub fn update_placements(&mut self, placements: Vec<PlacementsItem>, phases: Vec<PhaseOverview>) {
        let mut part_states_table = self.placements_table.lock().unwrap();

        let rows = placements
            .into_iter()
            .map(
                |PlacementsItem {
                     path: object_path,
                     state: placement_state,
                     ordering,
                 }| PlacementsRow {
                    object_path,
                    placement_state,
                    ordering,
                },
            )
            .collect::<Vec<_>>();

        match &mut *part_states_table {
            None => {
                let viewer = PlacementsRowViewer::new(self.component.sender.clone(), phases);
                let table = DataTable::from_iter(rows);
                *part_states_table = Some((viewer, table));
            }
            Some((viewer, table)) => {
                viewer.phases = phases;
                table.replace(rows);
            }
        }
    }

    pub fn update_phases(&mut self, phases: Vec<PhaseOverview>) {
        if let Some((viewer, _table)) = self
            .placements_table
            .lock()
            .unwrap()
            .as_mut()
        {
            viewer.phases = phases;
        }
    }
}

#[derive(Debug, Clone)]
pub enum PlacementsTableUiCommand {
    None,

    // internal
    RowUpdated {
        index: usize,
        new_row: PlacementsRow,
        old_row: PlacementsRow,
    },
    FilterCommand(FilterUiCommand),
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
}

#[derive(Debug, Clone, Default)]
pub struct PlacementsTableUiContext {}

impl UiComponent for PlacementsTableUi {
    type UiContext<'context> = PlacementsTableUiContext;
    type UiCommand = PlacementsTableUiCommand;
    type UiAction = PlacementsTableUiAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        let mut placements_table = self.placements_table.lock().unwrap();

        if placements_table.is_none() {
            ui.spinner();
            return;
        }

        let (viewer, table) = placements_table.as_mut().unwrap();

        viewer
            .filter
            .ui(ui, &mut FilterUiContext::default());

        ui.separator();

        let table_renderer = egui_data_table::Renderer::new(table, viewer)
            .with_style_modify(|style| {
                style.auto_shrink = [false, false].into();
                style.scroll_bar_visibility = ScrollBarVisibility::AlwaysVisible;
            })
            .with_translator(Arc::new(FluentTranslator::default()));
        ui.add(table_renderer);
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            PlacementsTableUiCommand::None => Some(PlacementsTableUiAction::None),
            PlacementsTableUiCommand::RowUpdated {
                index: _index,
                new_row,
                old_row,
            } => Some(PlacementsTableUiAction::UpdatePlacement {
                object_path: old_row.object_path,
                new_placement: new_row.placement_state,
                old_placement: old_row.placement_state,
            }),
            PlacementsTableUiCommand::FilterCommand(command) => {
                let mut table = self.placements_table.lock().unwrap();
                if let Some((viewer, _table)) = &mut *table {
                    let action = viewer
                        .filter
                        .update(command, &mut FilterUiContext::default());
                    debug!("filter action: {:?}", action);
                    match action {
                        Some(FilterUiAction::ApplyFilter) => Some(PlacementsTableUiAction::RequestRepaint),
                        None => None,
                    }
                } else {
                    None
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlacementsRow {
    pub object_path: ObjectPath,
    pub placement_state: PlacementState,
    pub ordering: usize,
}

pub struct PlacementsRowViewer {
    phases: Vec<PhaseOverview>,
    sender: Enqueue<PlacementsTableUiCommand>,
    pub(crate) filter: Filter,
}

impl PlacementsRowViewer {
    pub fn new(sender: Enqueue<PlacementsTableUiCommand>, mut phases: Vec<PhaseOverview>) -> Self {
        phases.sort_by(|a, b| {
            a.phase_reference
                .cmp(&b.phase_reference)
        });

        let mut filter = Filter::default();
        filter
            .component_state
            .configure_mapper(sender.clone(), |filter_ui_command| {
                trace!("filter ui mapper. command: {:?}", filter_ui_command);
                PlacementsTableUiCommand::FilterCommand(filter_ui_command)
            });

        Self {
            phases,
            sender,
            filter,
        }
    }
}

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

impl RowViewer<PlacementsRow> for PlacementsRowViewer {
    fn on_highlight_change(&mut self, highlighted: &[&PlacementsRow], _unhighlighted: &[&PlacementsRow]) {
        debug!("highlight change: {:?}", highlighted);
    }

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
        COLUMN_COUNT
    }

    fn is_sortable_column(&mut self, _column: usize) -> bool {
        true
    }

    fn is_editable_cell(&mut self, column: usize, _row: usize, _row_value: &PlacementsRow) -> bool {
        match column {
            PHASE_COL => true,
            // FIXME also check that the phase state is appropriate
            PLACED_COL => _row_value
                .placement_state
                .phase
                .is_some(),
            _ => false,
        }
    }

    fn allow_row_insertions(&mut self) -> bool {
        false
    }

    fn allow_row_deletions(&mut self) -> bool {
        false
    }

    fn compare_cell(&self, row_l: &PlacementsRow, row_r: &PlacementsRow, column: usize) -> std::cmp::Ordering {
        match column {
            OBJECT_PATH_COL => row_l
                .object_path
                .cmp(&row_r.object_path),
            REF_DES_COL => row_l
                .placement_state
                .placement
                .ref_des
                .cmp(&row_r.placement_state.placement.ref_des),
            PLACE_COL => row_l
                .placement_state
                .placement
                .place
                .cmp(&row_r.placement_state.placement.place),
            MANUFACTURER_COL => row_l
                .placement_state
                .placement
                .part
                .manufacturer
                .cmp(
                    &row_r
                        .placement_state
                        .placement
                        .part
                        .manufacturer,
                ),
            MPN_COL => row_l
                .placement_state
                .placement
                .part
                .mpn
                .cmp(&row_r.placement_state.placement.part.mpn),
            ROTATION_COL => row_l
                .placement_state
                .unit_position
                .rotation
                .cmp(&row_r.placement_state.placement.rotation),
            X_COL => row_l
                .placement_state
                .unit_position
                .x
                .cmp(&row_r.placement_state.placement.x),
            Y_COL => row_l
                .placement_state
                .unit_position
                .y
                .cmp(&row_r.placement_state.placement.y),
            PCB_SIDE_COL => row_l
                .placement_state
                .placement
                .pcb_side
                .cmp(&row_r.placement_state.placement.pcb_side),
            PHASE_COL => row_l
                .placement_state
                .phase
                .cmp(&row_r.placement_state.phase),
            PLACED_COL => row_l
                .placement_state
                .operation_status
                .cmp(&row_r.placement_state.operation_status),
            STATUS_COL => row_l
                .placement_state
                .project_status
                .cmp(&row_r.placement_state.project_status),
            ORDERING_COL => row_l.ordering.cmp(&row_r.ordering),
            _ => unreachable!(),
        }
    }

    fn column_name(&mut self, column: usize) -> Cow<'static, str> {
        match column {
            OBJECT_PATH_COL => tr!("table-placements-column-object-path"),
            REF_DES_COL => tr!("table-placements-column-refdes"),
            PLACE_COL => tr!("table-placements-column-place"),
            MANUFACTURER_COL => tr!("table-placements-column-manufacturer"),
            MPN_COL => tr!("table-placements-column-mpn"),
            ROTATION_COL => tr!("table-placements-column-rotation"),
            X_COL => tr!("table-placements-column-x"),
            Y_COL => tr!("table-placements-column-y"),
            PCB_SIDE_COL => tr!("table-placements-column-pcb-side"),
            PHASE_COL => tr!("table-placements-column-phase"),
            PLACED_COL => tr!("table-placements-column-placed"),
            STATUS_COL => tr!("table-placements-column-status"),
            ORDERING_COL => tr!("table-placements-column-ordering"),
            _ => unreachable!(),
        }
        .into()
    }

    #[profiling::function]
    fn show_cell_view(&mut self, ui: &mut Ui, row: &PlacementsRow, column: usize) {
        let _ = match column {
            OBJECT_PATH_COL => ui.label(&row.object_path.to_string()),
            REF_DES_COL => ui.label(
                row.placement_state
                    .placement
                    .ref_des
                    .to_string(),
            ),
            PLACE_COL => {
                let label = tr!(placement_place_to_i18n_key(row.placement_state.placement.place));
                ui.label(label)
            }
            MANUFACTURER_COL => ui.label(
                &row.placement_state
                    .placement
                    .part
                    .manufacturer,
            ),
            MPN_COL => ui.label(&row.placement_state.placement.part.mpn),
            ROTATION_COL => ui.label(format!(
                "{}",
                &row.placement_state
                    .unit_position
                    .rotation
            )),
            X_COL => ui.label(format!("{}", &row.placement_state.unit_position.x)),
            Y_COL => ui.label(format!("{}", &row.placement_state.unit_position.y)),
            PCB_SIDE_COL => {
                let key = pcb_side_to_i18n_key(&row.placement_state.placement.pcb_side);
                ui.label(tr!(key))
            }
            PHASE_COL => {
                let phase = &row
                    .placement_state
                    .phase
                    .clone()
                    .map(|reference: Reference| reference.to_string())
                    .unwrap_or_default();
                ui.label(phase)
            }
            PLACED_COL => {
                let label = tr!(placement_operation_status_to_i18n_key(
                    &row.placement_state.operation_status
                ));
                ui.label(label)
            }
            STATUS_COL => {
                let label = tr!(placement_project_status_to_i18n_key(
                    &row.placement_state.project_status
                ));
                ui.label(label)
            }
            ORDERING_COL => ui.label(row.ordering.to_string()),

            _ => unreachable!(),
        };
    }

    fn show_cell_editor(&mut self, ui: &mut Ui, row: &mut PlacementsRow, column: usize) -> Option<Response> {
        match column {
            PHASE_COL => {
                let response = ui.add(|ui: &mut Ui| {
                    egui::ComboBox::from_id_salt(ui.id().with("phase").with(column))
                        .width(ui.available_width())
                        .selected_text(match &row.placement_state.phase {
                            None => tr!("form-common-combo-none"),
                            Some(phase) => phase.to_string(),
                        })
                        .show_ui(ui, |ui| {
                            // Note: with the arguments to this method, there is no command we can send that will be able
                            //       to do anything useful with the row as there is probably no API to access the
                            //       underlying row instance that is being edited; so we HAVE to edit-in-place here.
                            if ui
                                .add(egui::Button::selectable(
                                    row.placement_state.phase.is_none(),
                                    tr!("form-common-combo-none")
                                ))
                                .clicked()
                            {
                                row.placement_state.phase = None;
                            }

                            for phase in self.phases.iter()
                                .filter(|phase|row.placement_state.placement.pcb_side.eq(&phase.pcb_side))
                            {
                                if ui
                                    .add(egui::Button::selectable(
                                        matches!(&row.placement_state.phase, Some(other_phase_reference) if other_phase_reference.eq(&phase.phase_reference)),
                                        phase.phase_reference.to_string(),
                                    ))
                                    .clicked()
                                {
                                    row.placement_state.phase = Some(phase.phase_reference.clone());
                                }
                            }
                        }).response
                });

                Some(response)
            }
            PLACED_COL => {
                ui.radio_value(
                    &mut row.placement_state.operation_status,
                    PlacementStatus::Pending,
                    tr!(placement_operation_status_to_i18n_key(&PlacementStatus::Pending)),
                );
                ui.radio_value(
                    &mut row.placement_state.operation_status,
                    PlacementStatus::Placed,
                    tr!(placement_operation_status_to_i18n_key(&PlacementStatus::Placed)),
                );
                ui.radio_value(
                    &mut row.placement_state.operation_status,
                    PlacementStatus::Skipped,
                    tr!(placement_operation_status_to_i18n_key(&PlacementStatus::Skipped)),
                );

                Some(ui.response())
            }
            _ => None,
        }
    }

    fn set_cell_value(&mut self, src: &PlacementsRow, dst: &mut PlacementsRow, column: usize) {
        match column {
            OBJECT_PATH_COL => dst
                .object_path
                .clone_from(&src.object_path),
            REF_DES_COL => dst
                .placement_state
                .placement
                .ref_des
                .clone_from(&src.placement_state.placement.ref_des),
            PLACE_COL => dst
                .placement_state
                .placement
                .place
                .clone_from(&src.placement_state.placement.place),
            MANUFACTURER_COL => dst
                .placement_state
                .placement
                .part
                .manufacturer
                .clone_from(
                    &src.placement_state
                        .placement
                        .part
                        .manufacturer,
                ),
            MPN_COL => dst
                .placement_state
                .placement
                .part
                .mpn
                .clone_from(&src.placement_state.placement.part.mpn),
            ROTATION_COL => dst
                .placement_state
                .unit_position
                .rotation
                .clone_from(&src.placement_state.placement.rotation),
            X_COL => dst
                .placement_state
                .unit_position
                .x
                .clone_from(&src.placement_state.placement.x),
            Y_COL => dst
                .placement_state
                .unit_position
                .y
                .clone_from(&src.placement_state.placement.y),
            PCB_SIDE_COL => dst
                .placement_state
                .placement
                .pcb_side
                .clone_from(&src.placement_state.placement.pcb_side),
            PHASE_COL => dst
                .placement_state
                .phase
                .clone_from(&src.placement_state.phase),
            PLACED_COL => dst
                .placement_state
                .operation_status
                .clone_from(&src.placement_state.operation_status),
            STATUS_COL => dst
                .placement_state
                .project_status
                .clone_from(&src.placement_state.project_status),
            ORDERING_COL => dst.ordering.clone_from(&src.ordering),
            _ => unreachable!(),
        }
    }

    fn new_empty_row(&mut self) -> PlacementsRow {
        PlacementsRow {
            object_path: Default::default(),
            placement_state: PlacementState {
                unit_path: Default::default(),
                placement: Placement {
                    ref_des: "".into(),
                    part: Part {
                        manufacturer: "".to_string(),
                        mpn: "".to_string(),
                    },
                    place: false,
                    pcb_side: PcbSide::Top,
                    x: Default::default(),
                    y: Default::default(),
                    rotation: Default::default(),
                },
                unit_position: Default::default(),
                operation_status: PlacementStatus::Pending,
                project_status: ProjectPlacementStatus::Unused,
                phase: None,
            },
            ordering: Default::default(),
        }
    }

    fn confirm_cell_write_by_ui(
        &mut self,
        _current: &PlacementsRow,
        _next: &PlacementsRow,
        column: usize,
        _context: CellWriteContext,
    ) -> bool {
        debug!(
            "confirm cell write by ui. column: {}, current: {:?}, next: {:?}, context: {:?}",
            column, _current, _next, _context
        );
        match column {
            PHASE_COL => true,
            PLACED_COL => true,
            _ => false,
        }
    }

    fn confirm_row_deletion_by_ui(&mut self, _row: &PlacementsRow) -> bool {
        false
    }

    fn on_row_updated(&mut self, row_index: usize, new_row: &PlacementsRow, old_row: &PlacementsRow) {
        trace!(
            "on_row_updated. row_index {}, old_row: {:?}, old_row: {:?}",
            row_index, new_row, old_row
        );
        self.sender
            .send(PlacementsTableUiCommand::RowUpdated {
                index: row_index,
                new_row: new_row.clone(),
                old_row: old_row.clone(),
            })
            .expect("sent");
    }

    fn on_row_inserted(&mut self, row_index: usize, row: &PlacementsRow) {
        trace!("on_row_inserted. row_index {}, row: {:?}", row_index, row);

        // should not be possible, since row insertion/deletion is prevented, this is a bug.
        unreachable!();
    }

    fn on_row_removed(&mut self, row_index: usize, row: &PlacementsRow) {
        trace!("on_row_removed. row_index {}, row: {:?}", row_index, row);

        // should not be possible, since row insertion/deletion is prevented, this is a bug.
        unreachable!();
    }

    #[profiling::function]
    fn filter_row(&mut self, row: &PlacementsRow) -> bool {
        let haystack = format!(
            "object_path: '{}', refdes: '{}', manufacturer: '{}', mpn: '{}', place: {}, placed: {}, side: {}, phase: '{}', status: '{}'",
            &row.object_path,
            &row.placement_state.placement.ref_des,
            &row.placement_state
                .placement
                .part
                .manufacturer,
            &row.placement_state.placement.part.mpn,
            &tr!(placement_place_to_i18n_key(row.placement_state.placement.place)),
            &tr!(placement_operation_status_to_i18n_key(
                &row.placement_state.operation_status
            )),
            &tr!(pcb_side_to_i18n_key(&row.placement_state.placement.pcb_side)),
            &row.placement_state
                .phase
                .as_ref()
                .map(|phase| phase.to_string())
                .unwrap_or_default(),
            &tr!(placement_project_status_to_i18n_key(
                &row.placement_state.project_status
            )),
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
