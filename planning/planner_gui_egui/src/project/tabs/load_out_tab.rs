use std::path::PathBuf;

use derivative::Derivative;
use egui::{Ui, WidgetText};
use egui_dock::tab_viewer::OnCloseResponse;
use egui_mobius::types::Value;
use planner_app::{LoadOut, LoadOutSource, Part, Reference};
use tracing::{debug, error, trace};
use util::path::clip_path;

use crate::project::tables::load_out::{
    LoadOutRow, LoadOutTableUi, LoadOutTableUiAction, LoadOutTableUiCommand, LoadOutTableUiContext,
};
use crate::project::tabs::ProjectTabContext;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct LoadOutTabUi {
    phase: Reference,
    #[derivative(Debug = "ignore")]
    load_out_table_ui: LoadOutTableUi,

    pub component: ComponentState<LoadOutTabUiCommand>,
}

impl LoadOutTabUi {
    pub fn new(phase: Reference) -> Self {
        let component: ComponentState<LoadOutTabUiCommand> = Default::default();

        let mut load_out_table_ui = LoadOutTableUi::new();
        load_out_table_ui
            .component
            .configure_mapper(component.sender.clone(), |load_out_table_command| {
                trace!("phase load_out table mapper. command: {:?}", load_out_table_command);
                LoadOutTabUiCommand::LoadoutTableUiCommand(load_out_table_command)
            });

        Self {
            phase,
            load_out_table_ui,

            component,
        }
    }

    pub fn update_load_out(&mut self, load_out: LoadOut) {
        self.load_out_table_ui
            .update_loadout(load_out);
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
    LoadoutTableUiCommand(LoadOutTableUiCommand),
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
        self.load_out_table_ui.filter_ui(ui);

        ui.separator();

        self.load_out_table_ui
            .ui(ui, &mut LoadOutTableUiContext::default());
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
            LoadOutTabUiCommand::LoadoutTableUiCommand(command) => self
                .load_out_table_ui
                .update(command, &mut LoadOutTableUiContext::default())
                .map(|action| match action {
                    LoadOutTableUiAction::None => LoadOutTabUiAction::None,
                    LoadOutTableUiAction::RequestRepaint => LoadOutTabUiAction::RequestRepaint,
                }),
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
