use std::collections::HashMap;
use std::sync::Arc;

use derivative::Derivative;
use egui::scroll_area::ScrollBarVisibility;
use egui::{Ui, WidgetText};
use egui_data_table::DataTable;
use egui_i18n::tr;
use egui_mobius::types::Value;
use planner_app::{Part, PartStates, ProcessReference};
use tracing::debug;

use crate::filter::{FilterUiAction, FilterUiCommand, FilterUiContext};
use crate::i18n::datatable_support::FluentTranslator;
use crate::project::tables::parts::{PartStatesRow, PartStatesRowViewer};
use crate::project::tabs::ProjectTabContext;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct PartsTabUi {
    #[derivative(Debug = "ignore")]
    part_states_table: Value<Option<(PartStatesRowViewer, DataTable<PartStatesRow>)>>,

    pub component: ComponentState<PartsTabUiCommand>,
}

impl PartsTabUi {
    pub fn new() -> Self {
        Self {
            part_states_table: Value::default(),

            component: Default::default(),
        }
    }

    pub fn update_part_states(&mut self, mut part_states: PartStates, processes: Vec<ProcessReference>) {
        let mut part_states_table = self.part_states_table.lock().unwrap();

        let rows = part_states
            .parts
            .drain(0..)
            .map(|part_state| {
                let enabled_processes = processes
                    .iter()
                    .map(|process| (process.clone(), part_state.processes.contains(process)))
                    .collect::<Vec<(ProcessReference, bool)>>();

                PartStatesRow {
                    part: part_state.part,
                    enabled_processes,
                    ref_des_set: part_state.ref_des_set,
                    quantity: part_state.quantity,
                }
            })
            .collect();

        let (_viewer, table) = part_states_table.get_or_insert_with(|| {
            let viewer = PartStatesRowViewer::new(self.component.sender.clone(), processes);
            let table = DataTable::new();

            (viewer, table)
        });

        table.replace(rows);
    }
}

#[derive(Debug, Clone)]
pub enum PartsTabUiCommand {
    None,

    // internal
    RowUpdated {
        index: usize,
        new_row: PartStatesRow,
        old_row: PartStatesRow,
    },
    FilterCommand(FilterUiCommand),
}

#[derive(Debug, Clone)]
pub enum PartsTabUiAction {
    None,
    UpdateProcessesForPart {
        part: Part,
        processes: HashMap<ProcessReference, bool>,
    },
    RequestRepaint,
}

#[derive(Debug, Clone, Default)]
pub struct PartsTabUiContext {}

impl UiComponent for PartsTabUi {
    type UiContext<'context> = PartsTabUiContext;
    type UiCommand = PartsTabUiCommand;
    type UiAction = PartsTabUiAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        ui.label(tr!("project-parts-header"));
        let mut part_states_table = self.part_states_table.lock().unwrap();

        if part_states_table.is_none() {
            ui.spinner();
            return;
        }

        let (viewer, table) = part_states_table.as_mut().unwrap();

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
            PartsTabUiCommand::None => Some(PartsTabUiAction::None),
            PartsTabUiCommand::RowUpdated {
                index,
                new_row,
                old_row,
            } => {
                let (_, _) = (index, old_row);
                let processes: HashMap<ProcessReference, bool> = new_row
                    .enabled_processes
                    .into_iter()
                    .collect();

                Some(PartsTabUiAction::UpdateProcessesForPart {
                    part: new_row.part,
                    processes,
                })
            }
            PartsTabUiCommand::FilterCommand(command) => {
                let mut table = self.part_states_table.lock().unwrap();
                if let Some((viewer, _table)) = &mut *table {
                    let action = viewer
                        .filter
                        .update(command, &mut FilterUiContext::default());
                    debug!("filter action: {:?}", action);
                    match action {
                        Some(FilterUiAction::ApplyFilter) => Some(PartsTabUiAction::RequestRepaint),
                        None => None,
                    }
                } else {
                    None
                }
            }
        }
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
        UiComponent::ui(&state.parts_tab_ui, ui, &mut PartsTabUiContext::default());
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, _context: &mut Self::Context) -> bool {
        true
    }
}
