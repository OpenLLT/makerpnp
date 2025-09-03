use std::path::PathBuf;
use std::sync::Arc;

use derivative::Derivative;
use egui::scroll_area::ScrollBarVisibility;
use egui::{Ui, WidgetText};
use egui_data_table::DataTable;
use egui_dock::tab_viewer::OnCloseResponse;
use egui_i18n::tr;
use egui_mobius::types::Value;
use planner_app::{LoadOut, LoadOutSource, Part, Reference};
use tracing::debug;
use util::path::clip_path;

use crate::filter::{FilterUiAction, FilterUiCommand, FilterUiContext};
use crate::i18n::datatable_support::FluentTranslator;
use crate::project::tables::load_out::{LoadOutRow, LoadOutRowViewer};
use crate::project::tabs::ProjectTabContext;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct LoadOutTabUi {
    phase: Reference,
    #[derivative(Debug = "ignore")]
    load_out_table: Value<Option<(LoadOutRowViewer, DataTable<LoadOutRow>)>>,

    pub component: ComponentState<LoadOutTabUiCommand>,
}

impl LoadOutTabUi {
    pub fn new(phase: Reference) -> Self {
        Self {
            phase,
            load_out_table: Value::default(),

            component: Default::default(),
        }
    }

    pub fn update_load_out(&mut self, mut load_out: LoadOut) {
        let mut load_out_table = self.load_out_table.lock().unwrap();

        let rows = load_out
            .items
            .drain(0..)
            .map(|item| LoadOutRow {
                part: Part::new(item.manufacturer, item.mpn),
                feeder: item
                    .reference
                    .map_or_else(|| "".to_string(), |reference| reference.to_string()),
            })
            .collect();

        let (_viewer, table) = load_out_table.get_or_insert_with(|| {
            let viewer = LoadOutRowViewer::new(self.component.sender.clone());
            let table = DataTable::new();

            (viewer, table)
        });

        table.replace(rows);
    }
}

#[derive(Debug, Clone)]
pub enum LoadOutTabUiCommand {
    None,

    // internal
    RowUpdated {
        index: usize,
        new_row: LoadOutRow,
        old_row: LoadOutRow,
    },
    FilterCommand(FilterUiCommand),
}

#[derive(Debug, Clone)]
pub enum LoadOutTabUiAction {
    None,
    UpdateFeederForPart {
        phase: Reference,
        part: Part,
        feeder: Option<Reference>,
    },
    RequestRepaint,
}

#[derive(Debug, Clone, Default)]
pub struct LoadOutTabUiContext {}

impl UiComponent for LoadOutTabUi {
    type UiContext<'context> = LoadOutTabUiContext;
    type UiCommand = LoadOutTabUiCommand;
    type UiAction = LoadOutTabUiAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        ui.label(tr!("project-load-out-header"));
        let mut load_out_table = self.load_out_table.lock().unwrap();

        if load_out_table.is_none() {
            ui.spinner();
            return;
        }

        let (viewer, table) = load_out_table.as_mut().unwrap();

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
            LoadOutTabUiCommand::None => Some(LoadOutTabUiAction::None),
            LoadOutTabUiCommand::RowUpdated {
                index,
                new_row,
                old_row,
            } => {
                let (_, _) = (index, old_row);

                Some(LoadOutTabUiAction::UpdateFeederForPart {
                    phase: self.phase.clone(),
                    part: new_row.part,
                    feeder: Reference::try_from(new_row.feeder).ok(),
                })
            }
            LoadOutTabUiCommand::FilterCommand(command) => {
                let mut table = self.load_out_table.lock().unwrap();
                if let Some((viewer, _table)) = &mut *table {
                    let action = viewer
                        .filter
                        .update(command, &mut FilterUiContext::default())
                        .inspect(|action| debug!("filter action: {:?}", action));

                    match action {
                        Some(FilterUiAction::ApplyFilter) => Some(LoadOutTabUiAction::RequestRepaint),
                        None => None,
                    }
                } else {
                    None
                }
            }
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq)]
pub struct LoadOutTab {
    pub phase: Reference,
    pub load_out_source: LoadOutSource,

    tab_label: String,
}

impl LoadOutTab {
    pub fn new(project_directory: PathBuf, phase: Reference, load_out_source: LoadOutSource) -> Self {
        let load_out_source_path = PathBuf::from(load_out_source.to_string());

        let clipped_load_out_source = clip_path(project_directory, load_out_source_path, None);

        Self {
            phase,
            load_out_source,
            tab_label: clipped_load_out_source,
        }
    }
}

impl Tab for LoadOutTab {
    type Context = ProjectTabContext;

    fn label(&self) -> WidgetText {
        egui::widget_text::WidgetText::from(&self.tab_label)
    }

    fn ui<'a>(&mut self, ui: &mut Ui, _tab_key: &TabKey, context: &mut Self::Context) {
        let state = context.state.lock().unwrap();
        let Some(load_out_ui) = state
            .load_out_tab_uis
            .get(&self.load_out_source)
        else {
            ui.spinner();
            return;
        };
        UiComponent::ui(load_out_ui, ui, &mut LoadOutTabUiContext::default());
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, context: &mut Self::Context) -> OnCloseResponse {
        let mut state = context.state.lock().unwrap();
        if let Some(_load_out_ui) = state
            .load_out_tab_uis
            .remove(&self.load_out_source)
        {
            debug!(
                "removed orphaned load out ui. load_out_source: {:?}",
                &self.load_out_source
            );
        }
        OnCloseResponse::Close
    }
}
