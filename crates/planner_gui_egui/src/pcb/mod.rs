use std::collections::HashMap;
use std::path::PathBuf;

use derivative::Derivative;
use egui::Ui;
use egui_ltreeview::{NodeBuilder, TreeView, TreeViewState};
use egui_mobius::Value;
use egui_mobius::types::Enqueue;
use planner_app::{
    Event, FileReference, LoadOutSource, PcbOverview, PcbSide, PcbUnitIndex, PcbView, PcbViewRequest,
    ProjectViewRequest, Reference,
};
use slotmap::new_key_type;
use tracing::{debug, error, info, trace};

use crate::pcb::configuration_tab::{
    ConfigurationTab, ConfigurationUi, ConfigurationUiAction, ConfigurationUiCommand, ConfigurationUiContext,
};
use crate::pcb::core_helper::PcbCoreHelper;
use crate::pcb::explorer_tab::{ExplorerTab, ExplorerUi, ExplorerUiCommand, ExplorerUiContext};
use crate::pcb::tabs::{PcbTabAction, PcbTabContext, PcbTabUiCommand, PcbTabs};
use crate::planner_app_core::{PlannerCoreService, PlannerError};
use crate::project::{ProjectAction, ProjectKey, ProjectUiCommand, ProjectUiState};
use crate::task::Task;
use crate::ui_component::{ComponentState, UiComponent};
use crate::ui_util::NavigationPath;

mod configuration_tab;
mod explorer_tab;
mod tabs;

new_key_type! {
    /// A key for a pcb
    pub struct PcbKey;
}

#[derive(Debug)]
pub enum PcbAction {
    Task(PcbKey, Task<PcbAction>),
    SetModifiedState(bool),
    UiCommand(PcbUiCommand),
    RequestRepaint,
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Pcb {
    #[derivative(Debug = "ignore")]
    planner_core_service: PlannerCoreService,

    pcb_ui_state: Value<PcbUiState>,

    path: Option<PathBuf>,
    modified: bool,

    // FIXME actually persist this, currently it should be treated as 'persistable_state'.
    #[derivative(Debug = "ignore")]
    pcb_tabs: Value<tabs::PcbTabs>,

    pub component: ComponentState<(PcbKey, PcbUiCommand)>,
}

impl Pcb {
    pub fn from_path(path: PathBuf, key: PcbKey) -> (Self, PcbUiCommand) {
        let instance = Self::new_inner(Some(path), key, None);
        (instance, PcbUiCommand::Load)
    }

    pub fn new(key: PcbKey) -> (Self, PcbUiCommand) {
        let instance = Self::new_inner(None, key, None);
        (instance, PcbUiCommand::Load)
    }

    fn new_inner(path: Option<PathBuf>, key: PcbKey, name: Option<String>) -> Self {
        debug!("Creating pcb instance from path. path: {:?}", path);

        let component: ComponentState<(PcbKey, PcbUiCommand)> = ComponentState::default();
        let component_sender = component.sender.clone();

        let pcb_ui_state = Value::new(PcbUiState::new(key, name, component_sender.clone()));

        let pcb_tabs = Value::new(PcbTabs::default());
        {
            let mut pcb_tabs = pcb_tabs.lock().unwrap();
            pcb_tabs
                .component
                .configure_mapper(component_sender, move |command| {
                    trace!("pcb inner-tab mapper. command: {:?}", command);
                    (key, PcbUiCommand::TabCommand(command))
                });
            pcb_tabs.add_tab(PcbTabKind::Explorer(ExplorerTab::default()));
        }

        let core_service = PlannerCoreService::new();

        Self {
            planner_core_service: core_service,
            pcb_ui_state,
            path,
            modified: false,
            component,
            pcb_tabs,
        }
    }

    pub fn show_explorer(&mut self, path: PathBuf) -> Task<PcbAction> {
        let mut pcb_tabs = self.pcb_tabs.lock().unwrap();
        let result = pcb_tabs.show_tab(|candidate_tab| matches!(candidate_tab, PcbTabKind::Explorer(_)));
        if result.is_err() {
            pcb_tabs.add_tab(PcbTabKind::Explorer(ExplorerTab::default()));
        }

        Task::done(PcbAction::UiCommand(PcbUiCommand::RequestPcbView(
            PcbViewRequest::Overview {
                path,
            },
        )))
    }

    pub fn show_configuration(&mut self, path: PathBuf) -> Task<PcbAction> {
        let mut pcb_tabs = self.pcb_tabs.lock().unwrap();
        let result = pcb_tabs.show_tab(|candidate_tab| matches!(candidate_tab, PcbTabKind::Configuration(_)));
        if result.is_err() {
            pcb_tabs.add_tab_to_second_leaf_or_split(PcbTabKind::Configuration(ConfigurationTab::default()));
        }

        Task::done(PcbAction::UiCommand(PcbUiCommand::RequestPcbView(
            PcbViewRequest::Overview {
                path,
            },
        )))
    }

    fn navigate(&mut self, path: NavigationPath) -> Option<PcbAction> {
        // if the path starts with `/pcb/` then show/hide UI elements based on the path,
        // e.g. update a dynamic that controls a per-pcb-tab-bar dynamic selector
        info!("pcb::navigate. path: {}", path);

        // TODO

        None
    }
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct PcbUiState {
    name: Option<String>,

    key: PcbKey,
    explorer_ui: ExplorerUi,
    configuration_ui: ConfigurationUi,
}

impl PcbUiState {
    pub fn new(key: PcbKey, name: Option<String>, sender: Enqueue<(PcbKey, PcbUiCommand)>) -> Self {
        let mut instance = Self {
            name,
            key,
            explorer_ui: ExplorerUi::new(),
            configuration_ui: ConfigurationUi::new(),
        };

        instance
            .explorer_ui
            .component
            .configure_mapper(sender.clone(), move |command| {
                trace!("explorer ui mapper. command: {:?}", command);
                (key, PcbUiCommand::ExplorerUiCommand(command))
            });

        instance
            .configuration_ui
            .component
            .configure_mapper(sender.clone(), move |command| {
                trace!("configuration ui mapper. command: {:?}", command);
                (key, PcbUiCommand::ConfigurationUiCommand(command))
            });

        instance
    }
}

// these should not contain state
#[derive(serde::Deserialize, serde::Serialize, Debug)]
enum PcbTabKind {
    Explorer(ExplorerTab),
    Configuration(ConfigurationTab),
}

#[derive(Debug, Clone)]
pub enum PcbUiCommand {
    None,
    DebugMarkModified,
    Load,
    Save,
    Error(PlannerError),
    PcbView(PcbView),
    // FIXME don't care about projects, don't care ablout all pcbs, care about *this* PCB.
    SetModifiedState {
        project_modified: bool,
        pcbs_modified: bool,
    },
    Loaded,
    ExplorerUiCommand(ExplorerUiCommand),
    ConfigurationUiCommand(ConfigurationUiCommand),
    TabCommand(PcbTabUiCommand),
    RequestPcbView(PcbViewRequest),
}

pub struct PcbContext {
    pub key: PcbKey,
}

impl UiComponent for Pcb {
    type UiContext<'context> = PcbContext;
    type UiCommand = (PcbKey, PcbUiCommand);
    type UiAction = PcbAction;

    fn ui<'context>(&self, ui: &mut Ui, context: &mut Self::UiContext<'context>) {
        ui.ctx().style_mut(|style| {
            // if this is not done, text in labels/checkboxes/etc wraps when using taffy
            style.wrap_mode = Some(egui::TextWrapMode::Extend);
        });

        let PcbContext {
            key,
        } = context;

        //
        // Tabs
        //
        let mut tab_context = PcbTabContext {
            state: self.pcb_ui_state.clone(),
        };

        let mut pcb_tabs = self.pcb_tabs.lock().unwrap();
        pcb_tabs.cleanup_tabs(&mut tab_context);
        pcb_tabs.ui(ui, &mut tab_context);
    }

    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        let (key, command) = command;

        match command {
            PcbUiCommand::None => None,
            PcbUiCommand::DebugMarkModified => {
                self.modified = true;
                Some(PcbAction::SetModifiedState(self.modified))
            }
            PcbUiCommand::Load => {
                debug!("Loading pcb. path: {:?}", self.path);

                // Safety: can't 'Load' without a path, do not attempt to load without a path.
                let path = self.path.clone().unwrap();

                let pcb_file = FileReference::Absolute(path.clone());

                self.planner_core_service
                    .update(Event::LoadPcb {
                        pcb_file,
                        root: None,
                    })
                    .when_ok(key, |_| Some(PcbUiCommand::Loaded))
            }
            PcbUiCommand::Save => {
                debug!("Saving pcb. path: {:?}", self.path);
                // TODO
                None
            }
            PcbUiCommand::Error(error) => {
                error!("PCB error. error: {:?}", error);
                // TODO show a dialog
                None
            }
            PcbUiCommand::PcbView(view) => {
                match view {
                    PcbView::PcbOverview(pcb_overview) => {
                        debug!("Received pcb overview.");

                        let mut pcb_ui_state = self.pcb_ui_state.lock().unwrap();
                        pcb_ui_state
                            .configuration_ui
                            .update_pcb_overview(pcb_overview.clone());
                        pcb_ui_state
                            .explorer_ui
                            .update_pcb_overview(pcb_overview);
                    }
                }
                None
            }
            PcbUiCommand::SetModifiedState {
                project_modified,
                pcbs_modified,
            } => {
                // FIXME we want to know if *THIS* pcb is modified, not any pcb.
                self.modified = pcbs_modified;
                Some(PcbAction::SetModifiedState(pcbs_modified))
            }
            PcbUiCommand::Loaded => {
                debug!("Loaded pcb. path: {:?}", self.path);

                // Safety: can't be 'Loaded' without a path.
                let path = self.path.clone().unwrap();

                match self
                    .planner_core_service
                    .update(Event::RequestPcbOverviewView {
                        path: path.clone(),
                    })
                    .into_actions()
                {
                    Ok(actions) => {
                        let mut tasks = actions
                            .into_iter()
                            .map(Task::done)
                            .collect::<Vec<Task<PcbAction>>>();

                        let task1 = self.show_explorer(path.clone());
                        // TODO change this to show the PCB by default
                        let task2 = self.show_configuration(path.clone());

                        let additional_tasks = vec![task1, task2];
                        tasks.extend(additional_tasks);

                        Some(PcbAction::Task(key, Task::batch(tasks)))
                    }
                    Err(error_action) => Some(error_action),
                }
            }
            PcbUiCommand::RequestPcbView(view_request) => {
                let event = match view_request {
                    PcbViewRequest::Overview {
                        path,
                    } => Event::RequestPcbOverviewView {
                        path,
                    },
                };
                self.planner_core_service
                    .update(event)
                    .when_ok(key, |_| None)
            }
            PcbUiCommand::TabCommand(tab_command) => {
                let mut pcb_tabs = self.pcb_tabs.lock().unwrap();

                let mut tab_context = PcbTabContext {
                    state: self.pcb_ui_state.clone(),
                };

                let action = pcb_tabs.update(tab_command, &mut tab_context);
                match action {
                    None => {}
                    Some(PcbTabAction::None) => {
                        debug!("PcbTabAction::None");
                    }
                }
                None
            }
            PcbUiCommand::ExplorerUiCommand(command) => {
                let context = &mut ExplorerUiContext::default();
                let explorer_ui_action = self
                    .pcb_ui_state
                    .lock()
                    .unwrap()
                    .explorer_ui
                    .update(command, context);
                match explorer_ui_action {
                    Some(explorer_tab::ExplorerUiAction::Navigate(path)) => self.navigate(path),
                    None => None,
                }
            }
            PcbUiCommand::ConfigurationUiCommand(command) => {
                let context = &mut ConfigurationUiContext::default();
                let configuration_ui_action = self
                    .pcb_ui_state
                    .lock()
                    .unwrap()
                    .configuration_ui
                    .update(command, context);
                match configuration_ui_action {
                    Some(ConfigurationUiAction::None) => None,
                    None => None,
                }
            }
        }
    }
}

mod core_helper {
    use tracing::warn;

    use crate::pcb::{PcbAction, PcbKey, PcbUiCommand};
    use crate::planner_app_core::{PlannerAction, PlannerError};
    use crate::task::Task;

    #[must_use]
    fn when_ok_inner<F>(
        result: Result<Vec<PlannerAction>, PlannerError>,
        project_key: PcbKey,
        f: F,
    ) -> Option<PcbAction>
    where
        F: FnOnce(&mut Vec<Task<PcbAction>>) -> Option<PcbUiCommand>,
    {
        match result {
            Ok(actions) => {
                let mut tasks = vec![];
                let effect_tasks: Vec<Task<PcbAction>> = actions
                    .into_iter()
                    .map(|planner_action| {
                        let project_action = into_project_action(planner_action);
                        Task::done(project_action)
                    })
                    .collect();

                tasks.extend(effect_tasks);

                if let Some(command) = f(&mut tasks) {
                    let final_task = Task::done(PcbAction::UiCommand(command));
                    tasks.push(final_task);
                }

                let action = PcbAction::Task(project_key, Task::batch(tasks));

                Some(action)
            }
            Err(error) => Some(PcbAction::UiCommand(PcbUiCommand::Error(error))),
        }
    }

    fn into_actions_inner(result: Result<Vec<PlannerAction>, PlannerError>) -> Result<Vec<PcbAction>, PcbAction> {
        match result {
            Ok(actions) => Ok(actions
                .into_iter()
                .map(into_project_action)
                .collect()),
            Err(error) => Err(PcbAction::UiCommand(PcbUiCommand::Error(error))),
        }
    }

    fn into_project_action(action: PlannerAction) -> PcbAction {
        match action {
            PlannerAction::SetModifiedState {
                project_modified,
                pcbs_modified,
            } => PcbAction::UiCommand(PcbUiCommand::SetModifiedState {
                project_modified,
                pcbs_modified,
            }),
            PlannerAction::ProjectView(_project_view) => {
                warn!("pcb received project view action. ignoring.");
                PcbAction::UiCommand(PcbUiCommand::None)
            }
            PlannerAction::PcbView(pcb_view) => PcbAction::UiCommand(PcbUiCommand::PcbView(pcb_view)),
        }
    }

    pub trait PcbCoreHelper {
        fn into_actions(self) -> Result<Vec<PcbAction>, PcbAction>;
        fn when_ok<F>(self, project_key: PcbKey, f: F) -> Option<PcbAction>
        where
            F: FnOnce(&mut Vec<Task<PcbAction>>) -> Option<PcbUiCommand>;
    }

    impl PcbCoreHelper for Result<Vec<PlannerAction>, PlannerError> {
        fn into_actions(self) -> Result<Vec<PcbAction>, PcbAction> {
            into_actions_inner(self)
        }

        fn when_ok<F>(self, project_key: PcbKey, f: F) -> Option<PcbAction>
        where
            F: FnOnce(&mut Vec<Task<PcbAction>>) -> Option<PcbUiCommand>,
        {
            when_ok_inner(self, project_key, f)
        }
    }
}
