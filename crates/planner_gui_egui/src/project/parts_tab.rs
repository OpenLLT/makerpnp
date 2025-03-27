use std::borrow::Cow;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use derivative::Derivative;
use egui::{Response, Ui, WidgetText};
use egui_data_table::viewer::CellWriteContext;
use egui_data_table::{DataTable, RowViewer};
use egui_i18n::tr;
use egui_mobius::types::Value;
use planner_app::{Part, PartStates, ProcessName};
use tracing::{debug, trace};

use crate::i18n::datatable_support::FluentTranslator;
use crate::project::tabs::ProjectTabContext;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct PartsUi {
    #[derivative(Debug = "ignore")]
    part_states_table: Value<Option<(PartStatesRowViewer, DataTable<PartStatesRow>)>>,

    pub component: ComponentState<PartsUiCommand>,
}

impl PartsUi {
    pub fn new() -> Self {
        Self {
            part_states_table: Value::default(),

            component: Default::default(),
        }
    }

    pub fn update_part_states(&mut self, mut part_states: PartStates) {
        // TODO get this from somewhere, don't build here
        let mut processes: Vec<ProcessName> = vec![
            ProcessName::from_str("manual").unwrap(),
            ProcessName::from_str("pnp").unwrap(),
        ];

        let mut part_states_table = self.part_states_table.lock().unwrap();
        let table: DataTable<PartStatesRow> = {
            part_states
                .parts
                .drain(0..)
                .map(|part_state| {
                    let enabled_processes = processes
                        .iter()
                        .map(|process| (process.clone(), part_state.processes.contains(process)))
                        .collect::<Vec<(ProcessName, bool)>>();

                    PartStatesRow {
                        part: part_state.part,
                        enabled_processes,
                    }
                })
        }
        .collect();

        part_states_table.replace((PartStatesRowViewer::new(processes), table));
    }
}

#[derive(Debug, Clone)]
pub enum PartsUiCommand {
    None,
}

#[derive(Debug, Clone)]
pub enum PartsUiAction {
    None,
}

#[derive(Debug, Clone, Default)]
pub struct PartsUiContext {}

impl UiComponent for PartsUi {
    type UiContext<'context> = PartsUiContext;
    type UiCommand = PartsUiCommand;
    type UiAction = PartsUiAction;

    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        ui.label(tr!("project-parts-header"));
        let mut part_states_table = self.part_states_table.lock().unwrap();
        if let Some((viewer, table)) = part_states_table.as_mut() {
            let table_renderer =
                egui_data_table::Renderer::new(table, viewer).with_translator(Arc::new(FluentTranslator::default()));
            ui.add(table_renderer);
        }
    }

    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            PartsUiCommand::None => Some(PartsUiAction::None),
        }
    }
}

#[derive(Debug)]
struct PartStatesRow {
    part: Part,
    enabled_processes: Vec<(ProcessName, bool)>,
}

struct PartStatesRowViewer {
    processes: Vec<ProcessName>,
}

impl PartStatesRowViewer {
    pub fn new(mut processes: Vec<ProcessName>) -> Self {
        // sorting the processes here helps to ensure that the view vs edit list of processes has the same
        // ordering.
        processes.sort();
        Self {
            processes,
        }
    }
}

impl RowViewer<PartStatesRow> for PartStatesRowViewer {
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
                let ui = ui.add(|ui: &mut Ui| {
                    ui.horizontal_wrapped(|ui| {
                        // Note that the enabled_processes was built in the same order as self.processes.
                        for (name, enabled) in row.enabled_processes.iter_mut() {
                            ui.checkbox(enabled, name.to_string());
                        }
                    })
                    .response
                });
                Some(ui)
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

    fn on_row_updated(&mut self, row_index: usize, row: &PartStatesRow) {
        trace!("on_row_updated. row_index {}, row: {:?}", row_index, row);
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
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Default, PartialEq)]
pub struct PartsTab {}

impl Tab for PartsTab {
    type Context = ProjectTabContext;

    fn label(&self) -> WidgetText {
        egui::widget_text::WidgetText::from(tr!("project-parts-tab-label"))
    }

    fn ui<'a>(&mut self, ui: &mut Ui, _tab_key: &TabKey, context: &mut Self::Context) {
        let state = context.state.lock().unwrap();
        UiComponent::ui(&state.parts_ui, ui, &mut PartsUiContext::default());
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, _context: &mut Self::Context) -> bool {
        true
    }
}
