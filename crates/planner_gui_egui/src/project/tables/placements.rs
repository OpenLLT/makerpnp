use std::borrow::Cow;
use std::sync::Arc;

use derivative::Derivative;
use egui::{Response, Ui};
use egui_data_table::viewer::CellWriteContext;
use egui_data_table::{DataTable, RowViewer};
use egui_i18n::tr;
use egui_mobius::Value;
use egui_mobius::types::Enqueue;
use planner_app::{Part, PcbSide, Placement, PlacementState, PlacementStatus, Reference};
use tracing::{debug, trace};

use crate::filter::{Filter, FilterUiAction, FilterUiCommand, FilterUiContext};
use crate::i18n::conversions::{pcb_side_to_i18n_key, placement_place_to_i18n_key, placement_placed_to_i18n_key};
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

    pub fn update_placements(&mut self, mut placements: Vec<PlacementState>, phases: Vec<Reference>) {
        let mut part_states_table = self.placements_table.lock().unwrap();
        let table: DataTable<PlacementsRow> = {
            placements
                .drain(0..)
                .map(|placement_state| PlacementsRow {
                    placement_state,
                })
        }
        .collect();

        part_states_table.replace((PlacementsRowViewer::new(self.component.sender.clone(), phases), table));
    }
}

#[derive(Debug, Clone)]
pub enum PlacementsTableUiCommand {
    None,

    // internal
    RowUpdated(usize, PlacementsRow),
    FilterCommand(FilterUiCommand),
}

#[derive(Debug, Clone)]
pub enum PlacementsTableUiAction {
    None,
    UpdatePlacement { placement: PlacementState },
    RequestRepaint,
}

#[derive(Debug, Clone, Default)]
pub struct PlacementsTableUiContext {}

impl UiComponent for PlacementsTableUi {
    type UiContext<'context> = PlacementsTableUiContext;
    type UiCommand = PlacementsTableUiCommand;
    type UiAction = PlacementsTableUiAction;

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

        let table_renderer =
            egui_data_table::Renderer::new(table, viewer).with_translator(Arc::new(FluentTranslator::default()));
        ui.add(table_renderer);
    }

    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            PlacementsTableUiCommand::None => Some(PlacementsTableUiAction::None),
            PlacementsTableUiCommand::RowUpdated(_row_index, row) => Some(PlacementsTableUiAction::UpdatePlacement {
                placement: row.placement_state,
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
    pub placement_state: PlacementState,
}

pub struct PlacementsRowViewer {
    phases: Vec<Reference>,
    sender: Enqueue<PlacementsTableUiCommand>,
    pub(crate) filter: Filter,
}

impl PlacementsRowViewer {
    pub fn new(sender: Enqueue<PlacementsTableUiCommand>, mut phases: Vec<Reference>) -> Self {
        phases.sort();

        let mut filter = Filter::default();
        filter
            .component_state
            .configure_mapper(sender.clone(), |filter_ui_command| {
                debug!("filter ui mapper. command: {:?}", filter_ui_command);
                PlacementsTableUiCommand::FilterCommand(filter_ui_command)
            });

        Self {
            phases,
            sender,
            filter,
        }
    }
}

impl RowViewer<PlacementsRow> for PlacementsRowViewer {
    fn num_columns(&mut self) -> usize {
        11
    }

    fn is_sortable_column(&mut self, _column: usize) -> bool {
        true
    }

    fn is_editable_cell(&mut self, column: usize, _row: usize, _row_value: &PlacementsRow) -> bool {
        // TODO make more things editable
        false
    }

    fn allow_row_insertions(&mut self) -> bool {
        false
    }

    fn allow_row_deletions(&mut self) -> bool {
        false
    }

    fn compare_cell(&self, row_l: &PlacementsRow, row_r: &PlacementsRow, column: usize) -> std::cmp::Ordering {
        match column {
            0 => row_l
                .placement_state
                .unit_path
                .cmp(&row_r.placement_state.unit_path),
            1 => row_l
                .placement_state
                .placement
                .ref_des
                .cmp(&row_r.placement_state.placement.ref_des),
            2 => row_l
                .placement_state
                .placed
                .cmp(&row_r.placement_state.placement.place),
            3 => row_l
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
            4 => row_l
                .placement_state
                .placement
                .part
                .mpn
                .cmp(&row_r.placement_state.placement.part.mpn),
            5 => row_l
                .placement_state
                .placement
                .rotation
                .cmp(&row_r.placement_state.placement.rotation),
            6 => row_l
                .placement_state
                .placement
                .x
                .cmp(&row_r.placement_state.placement.x),
            7 => row_l
                .placement_state
                .placement
                .y
                .cmp(&row_r.placement_state.placement.y),
            8 => row_l
                .placement_state
                .placement
                .pcb_side
                .cmp(&row_r.placement_state.placement.pcb_side),
            9 => row_l
                .placement_state
                .phase
                .cmp(&row_r.placement_state.phase),
            10 => row_l
                .placement_state
                .placed
                .cmp(&row_r.placement_state.placed),
            _ => unreachable!(),
        }
    }

    fn column_name(&mut self, column: usize) -> Cow<'static, str> {
        match column {
            0 => tr!("table-placements-column-object-path"),
            1 => tr!("table-placements-column-refdes"),
            2 => tr!("table-placements-column-place"),
            3 => tr!("table-placements-column-manufacturer"),
            4 => tr!("table-placements-column-mpn"),
            5 => tr!("table-placements-column-rotation"),
            6 => tr!("table-placements-column-x"),
            7 => tr!("table-placements-column-y"),
            8 => tr!("table-placements-column-pcb-side"),
            9 => tr!("table-placements-column-phase"),
            10 => tr!("table-placements-column-placed"),
            _ => unreachable!(),
        }
        .into()
    }

    fn show_cell_view(&mut self, ui: &mut Ui, row: &PlacementsRow, column: usize) {
        let _ = match column {
            0 => ui.label(
                &row.placement_state
                    .unit_path
                    .to_string(),
            ),
            1 => ui.label(&row.placement_state.placement.ref_des),
            2 => {
                let label = tr!(placement_place_to_i18n_key(row.placement_state.placement.place));
                ui.label(label)
            }
            3 => ui.label(
                &row.placement_state
                    .placement
                    .part
                    .manufacturer,
            ),
            4 => ui.label(&row.placement_state.placement.part.mpn),
            5 => ui.label(format!("{}", &row.placement_state.placement.rotation)),
            6 => ui.label(format!("{}", &row.placement_state.placement.x)),
            7 => ui.label(format!("{}", &row.placement_state.placement.y)),
            8 => {
                let key = pcb_side_to_i18n_key(&row.placement_state.placement.pcb_side);
                ui.label(tr!(key))
            }
            9 => {
                let phase = &row
                    .placement_state
                    .phase
                    .clone()
                    .map(|reference: Reference| reference.to_string())
                    .unwrap_or_default();
                ui.label(phase)
            }
            10 => {
                let label = tr!(placement_placed_to_i18n_key(row.placement_state.placed));
                ui.label(label)
            }

            _ => unreachable!(),
        };
    }

    fn show_cell_editor(&mut self, ui: &mut Ui, row: &mut PlacementsRow, column: usize) -> Option<Response> {
        match column {
            0 => None,
            1 => None,
            2 => None,
            3 => None,
            4 => None,
            5 => None,
            6 => None,
            7 => None,
            8 => None,
            9 => None,
            10 => None,
            _ => unreachable!(),
        }
    }

    fn set_cell_value(&mut self, src: &PlacementsRow, dst: &mut PlacementsRow, column: usize) {
        match column {
            0 => dst
                .placement_state
                .unit_path
                .clone_from(&src.placement_state.unit_path),
            1 => dst
                .placement_state
                .placement
                .ref_des
                .clone_from(&src.placement_state.placement.ref_des),
            2 => dst
                .placement_state
                .placement
                .place
                .clone_from(&src.placement_state.placement.place),
            3 => dst
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
            4 => dst
                .placement_state
                .placement
                .part
                .mpn
                .clone_from(&src.placement_state.placement.part.mpn),
            5 => dst
                .placement_state
                .placement
                .rotation
                .clone_from(&src.placement_state.placement.rotation),
            6 => dst
                .placement_state
                .placement
                .x
                .clone_from(&src.placement_state.placement.x),
            7 => dst
                .placement_state
                .placement
                .y
                .clone_from(&src.placement_state.placement.y),
            8 => dst
                .placement_state
                .placement
                .pcb_side
                .clone_from(&src.placement_state.placement.pcb_side),
            9 => dst
                .placement_state
                .phase
                .clone_from(&src.placement_state.phase),
            10 => dst
                .placement_state
                .placed
                .clone_from(&src.placement_state.placed),
            _ => unreachable!(),
        }
    }

    fn new_empty_row(&mut self) -> PlacementsRow {
        PlacementsRow {
            placement_state: PlacementState {
                unit_path: Default::default(),
                placement: Placement {
                    ref_des: "".to_string(),
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
                placed: false,
                status: PlacementStatus::Unknown,
                phase: None,
            },
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
            _ => false,
            //_ => unreachable!(),
        }
    }

    fn confirm_row_deletion_by_ui(&mut self, _row: &PlacementsRow) -> bool {
        false
    }

    fn on_row_updated(&mut self, row_index: usize, row: &PlacementsRow) {
        trace!("on_row_updated. row_index {}, row: {:?}", row_index, row);
        self.sender
            .send(PlacementsTableUiCommand::RowUpdated(row_index, row.clone()))
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

    fn filter_row(&mut self, row: &PlacementsRow) -> bool {
        let manufacturer_matched = self.filter.matches(
            &row.placement_state
                .placement
                .part
                .manufacturer,
        );
        let mpn_matched = self
            .filter
            .matches(&row.placement_state.placement.part.mpn);
        let refdes_matched = self
            .filter
            .matches(&row.placement_state.placement.ref_des);
        let unit_path_matched = self.filter.matches(
            &row.placement_state
                .unit_path
                .to_string(),
        );

        // "Filter single row. If this returns false, the row will be hidden."
        let result = manufacturer_matched || mpn_matched || refdes_matched || unit_path_matched;

        trace!(
            "row: {:?}, manufacturer_matched: {}, mpn_matched: {}, refdes_matched: {} unit_path_matched: {}, result: {}",
            row, manufacturer_matched, mpn_matched, refdes_matched, unit_path_matched, result
        );

        result
    }

    fn row_filter_hash(&mut self) -> &impl std::hash::Hash {
        &self.filter
    }
}
