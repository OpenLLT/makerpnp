use std::collections::HashMap;
use std::path::PathBuf;

use derivative::Derivative;
use egui::Ui;
use egui_mobius::Value;
use egui_mobius::types::Enqueue;
use planner_app::{DesignIndex, Event, PcbView, PcbViewRequest};
use regex::Regex;
use slotmap::new_key_type;
use tabs::configuration_tab::{
    ConfigurationTab, ConfigurationTabUiAction, ConfigurationTabUiCommand, ConfigurationTabUiContext, ConfigurationUi,
};
use tabs::explorer_tab::{ExplorerTab, ExplorerTabUiCommand, ExplorerTabUiContext, ExplorerUi};
use tabs::gerber_viewer_tab::{
    GerberViewerTab, GerberViewerTabUi, GerberViewerTabUiAction, GerberViewerTabUiCommand, GerberViewerTabUiContext,
};
use tracing::{debug, error, info, trace};

use crate::pcb::core_helper::PcbCoreHelper;
use crate::pcb::tabs::explorer_tab::ExplorerTabUiAction;
use crate::pcb::tabs::panel_tab::{PanelTab, PanelTabUi, PanelTabUiAction, PanelTabUiCommand, PanelTabUiContext};
use crate::pcb::tabs::{PcbTabAction, PcbTabContext, PcbTabUiCommand, PcbTabs};
use crate::planner_app_core::{PlannerCoreService, PlannerError};
use crate::task::Task;
use crate::ui_component::{ComponentState, UiComponent};
use crate::ui_components::gerber_viewer_ui::{GerberViewerMode, GerberViewerUiInstanceArgs};
use crate::ui_util::NavigationPath;

pub mod tabs;

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

    path: PathBuf,
    modified: bool,

    pcb_tabs: Value<PcbTabs>,

    pub component: ComponentState<(PcbKey, PcbUiCommand)>,
}

impl Pcb {
    pub fn from_path(path: PathBuf, key: PcbKey, pcb_tabs: Value<PcbTabs>) -> (Self, Vec<PcbUiCommand>) {
        Self::new_inner(path, key, None, pcb_tabs, PcbUiCommand::Load)
    }

    pub fn new(
        path: PathBuf,
        key: PcbKey,
        name: String,
        units: u16,
        pcb_tabs: Value<PcbTabs>,
    ) -> (Self, Vec<PcbUiCommand>) {
        Self::new_inner(path.clone(), key, None, pcb_tabs, PcbUiCommand::Create {
            path,
            name,
            units,
        })
    }

    fn new_inner(
        path: PathBuf,
        key: PcbKey,
        name: Option<String>,
        pcb_tabs: Value<PcbTabs>,
        initial_command: PcbUiCommand,
    ) -> (Self, Vec<PcbUiCommand>) {
        debug!("Creating pcb instance from path. path: {:?}", path);

        let component: ComponentState<(PcbKey, PcbUiCommand)> = ComponentState::default();
        let component_sender = component.sender.clone();

        let pcb_ui_state = Value::new(PcbUiState::new(key, name, component_sender.clone()));

        //let pcb_tabs = Value::new(PcbTabs::default());

        let core_service = PlannerCoreService::new();

        let mut instance = Self {
            planner_core_service: core_service,
            pcb_ui_state,
            path,
            modified: false,
            component,
            pcb_tabs,
        };

        let mut commands = instance.configure_tabs(key);
        commands.insert(0, initial_command);

        (instance, commands)
    }

    fn configure_tabs(&mut self, key: PcbKey) -> Vec<PcbUiCommand> {
        let component_sender = self.component.sender.clone();
        let mut pcb_tabs = self.pcb_tabs.lock().unwrap();

        pcb_tabs
            .component
            .configure_mapper(component_sender, move |command| {
                trace!("pcb inner-tab mapper. command: {:?}", command);
                (key, PcbUiCommand::TabCommand(command))
            });

        pcb_tabs.filter_map(|(_key, tab)| {
            let command = match tab {
                PcbTabKind::Explorer(_tab) => PcbUiCommand::ShowExplorer,
                PcbTabKind::Configuration(_) => PcbUiCommand::ShowConfiguration,
                PcbTabKind::Panel(_tab) => PcbUiCommand::ShowPanel,
                PcbTabKind::GerberViewer(tab) => PcbUiCommand::ShowGerberViewer(tab.args.clone()),
            };

            Some(command)
        })
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

    pub fn show_panel(&mut self, path: PathBuf) -> Task<PcbAction> {
        let mut pcb_tabs = self.pcb_tabs.lock().unwrap();
        let result = pcb_tabs.show_tab(|candidate_tab| matches!(candidate_tab, PcbTabKind::Panel(_)));
        if result.is_err() {
            pcb_tabs.add_tab(PcbTabKind::Panel(PanelTab::default()));
        }

        Task::done(PcbAction::UiCommand(PcbUiCommand::RequestPcbView(
            PcbViewRequest::Panel {
                path,
            },
        )))
    }

    fn ensure_gerber_viewer(&self, key: PcbKey, args: GerberViewerUiInstanceArgs) -> bool {
        let mut state = self.pcb_ui_state.lock().unwrap();
        let mut created = false;
        let _gerber_viewer_ui_state = state
            .gerber_viewer_tab_uis
            .entry(args.clone())
            .or_insert_with(|| {
                created = true;
                debug!("ensuring gerber viewer tab ui. args: {:?}", args);
                let mut gerber_viewer_ui = GerberViewerTabUi::new(args.clone());
                gerber_viewer_ui
                    .component
                    .configure_mapper(self.component.sender.clone(), {
                        move |command| {
                            trace!("gerber viewer tab ui mapper. command: {:?}", command);
                            (key, PcbUiCommand::GerberViewerTabUiCommand {
                                args: args.clone(),
                                command,
                            })
                        }
                    });

                gerber_viewer_ui
            });

        created
    }

    pub fn show_gerber_viewer(&mut self, key: PcbKey, args: GerberViewerUiInstanceArgs) -> Option<Task<PcbAction>> {
        let mut pcb_tabs = self.pcb_tabs.lock().unwrap();
        pcb_tabs
            .show_tab(|candidate_tab| matches!(candidate_tab, PcbTabKind::GerberViewer(tab) if tab.args.eq(&args)))
            .inspect_err(|_| {
                pcb_tabs.add_tab_to_second_leaf_or_split(PcbTabKind::GerberViewer(GerberViewerTab::new(args.clone())));
            })
            .ok();

        match self.ensure_gerber_viewer(key, args) {
            false => None,
            true => Some(Task::done(PcbAction::UiCommand(PcbUiCommand::RequestPcbView(
                PcbViewRequest::Overview {
                    path: self.path.clone(),
                },
            )))),
        }
    }

    fn navigate(&mut self, key: &PcbKey, navigation_path: NavigationPath) -> Option<PcbAction> {
        // if the path starts with `/pcb/` then show/hide UI elements based on the path,
        info!("pcb::navigate. navigation_path: {}", navigation_path);

        #[must_use]
        fn handle_root(key: &PcbKey, navigation_path: &NavigationPath) -> Option<PcbAction> {
            if navigation_path.eq(&"/pcb/".into()) {
                // Show the configuration, in lieu of anything else.
                Some(PcbAction::Task(
                    *key,
                    Task::done(PcbAction::UiCommand(PcbUiCommand::ShowConfiguration)),
                ))
            } else {
                None
            }
        }

        #[must_use]
        fn handle_configuration(key: &PcbKey, navigation_path: &NavigationPath) -> Option<PcbAction> {
            if navigation_path.eq(&"/pcb/configuration".into()) {
                Some(PcbAction::Task(
                    *key,
                    Task::done(PcbAction::UiCommand(PcbUiCommand::ShowConfiguration)),
                ))
            } else {
                None
            }
        }

        #[must_use]
        fn handle_panel(key: &PcbKey, navigation_path: &NavigationPath) -> Option<PcbAction> {
            if navigation_path.eq(&"/pcb/panel".into()) {
                Some(PcbAction::Task(
                    *key,
                    Task::done(PcbAction::UiCommand(PcbUiCommand::ShowPanel)),
                ))
            } else {
                None
            }
        }

        #[must_use]
        fn handle_pcb(key: &PcbKey, navigation_path: &NavigationPath) -> Option<PcbAction> {
            if navigation_path.eq(&"/pcb/pcb".into()) {
                let args = GerberViewerUiInstanceArgs {
                    mode: GerberViewerMode::Panel,
                };

                Some(PcbAction::Task(
                    *key,
                    Task::done(PcbAction::UiCommand(PcbUiCommand::ShowGerberViewer(args))),
                ))
            } else {
                None
            }
        }

        #[must_use]
        fn handle_pcb_design(key: &PcbKey, navigation_path: &NavigationPath) -> Option<PcbAction> {
            let design_pattern = Regex::new(r"^/pcb/designs/(?<design>\d*){1}$").unwrap();
            if let Some(captures) = design_pattern.captures(&navigation_path) {
                let design_index: DesignIndex = captures
                    .name("design")
                    .unwrap()
                    .as_str()
                    .parse::<DesignIndex>()
                    .unwrap();
                debug!("design_index: {}", design_index);

                let args = GerberViewerUiInstanceArgs {
                    mode: GerberViewerMode::Design(design_index),
                };

                Some(PcbAction::Task(
                    *key,
                    Task::done(PcbAction::UiCommand(PcbUiCommand::ShowGerberViewer(args))),
                ))
            } else {
                None
            }
        }

        let handlers = [
            handle_root,
            handle_configuration,
            handle_panel,
            handle_pcb,
            handle_pcb_design,
        ];

        handlers
            .iter()
            .find_map(|handler| handler(key, &navigation_path))
    }
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct PcbUiState {
    name: Option<String>,

    key: PcbKey,
    explorer_tab_ui: ExplorerUi,
    configuration_tab_ui: ConfigurationUi,
    gerber_viewer_tab_uis: HashMap<GerberViewerUiInstanceArgs, GerberViewerTabUi>,
    panel_tab_ui: PanelTabUi,
}

impl PcbUiState {
    pub fn new(key: PcbKey, name: Option<String>, sender: Enqueue<(PcbKey, PcbUiCommand)>) -> Self {
        let mut instance = Self {
            name,
            key,
            explorer_tab_ui: ExplorerUi::new(),
            configuration_tab_ui: ConfigurationUi::new(),
            gerber_viewer_tab_uis: HashMap::new(),
            panel_tab_ui: PanelTabUi::new(),
        };

        instance
            .explorer_tab_ui
            .component
            .configure_mapper(sender.clone(), move |command| {
                trace!("explorer tab ui mapper. command: {:?}", command);
                (key, PcbUiCommand::ExplorerTabUiCommand(command))
            });

        instance
            .configuration_tab_ui
            .component
            .configure_mapper(sender.clone(), move |command| {
                trace!("configuration tab ui mapper. command: {:?}", command);
                (key, PcbUiCommand::ConfigurationTabUiCommand(command))
            });

        instance
            .panel_tab_ui
            .component
            .configure_mapper(sender.clone(), move |command| {
                trace!("panel tab ui mapper. command: {:?}", command);
                (key, PcbUiCommand::PanelTabUiCommand(command))
            });

        instance
    }
}

// these should not contain state
#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub enum PcbTabKind {
    Explorer(ExplorerTab),
    Configuration(ConfigurationTab),
    GerberViewer(GerberViewerTab),
    Panel(PanelTab),
}

#[derive(Debug, Clone)]
pub enum PcbUiCommand {
    None,

    // FIXME don't care about projects, don't care about /all/ pcbs, care about *this* PCB.
    SetModifiedState {
        project_modified: bool,
        pcbs_modified: bool,
    },

    Create {
        path: PathBuf,
        name: String,
        units: u16,
    },
    Created {
        path: PathBuf,
    },
    Load,
    Loaded,
    Save,
    Saved,

    //
    // errors
    //
    Error(PlannerError),

    //
    // views
    //
    PcbView(PcbView),
    RequestPcbView(PcbViewRequest),

    //
    // tabs
    //
    TabCommand(PcbTabUiCommand),

    ShowExplorer,
    ExplorerTabUiCommand(ExplorerTabUiCommand),

    ShowConfiguration,
    ConfigurationTabUiCommand(ConfigurationTabUiCommand),

    ShowGerberViewer(GerberViewerUiInstanceArgs),
    GerberViewerTabUiCommand {
        args: GerberViewerUiInstanceArgs,
        command: GerberViewerTabUiCommand,
    },

    ShowPanel,
    PanelTabUiCommand(PanelTabUiCommand),
}

pub struct PcbContext {
    pub key: PcbKey,
}

impl UiComponent for Pcb {
    type UiContext<'context> = PcbContext;
    type UiCommand = (PcbKey, PcbUiCommand);
    type UiAction = PcbAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        ui.ctx().style_mut(|style| {
            // if this is not done, text in labels/checkboxes/etc wraps when using taffy
            style.wrap_mode = Some(egui::TextWrapMode::Extend);
        });

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

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        let (key, command) = command;

        match command {
            PcbUiCommand::None => None,
            PcbUiCommand::Create {
                path,
                name,
                units,
            } => {
                debug!("Creating pcb. path: {:?}", self.path);
                self.planner_core_service
                    .update(Event::CreatePcb {
                        name,
                        units,
                        path: path.clone(),
                    })
                    .when_ok(key, |_| {
                        Some(PcbUiCommand::Created {
                            path,
                        })
                    })
            }
            PcbUiCommand::Created {
                path,
            } => {
                let task1 = self.show_explorer(path.clone());
                let task2 = self.show_configuration(path);
                let tasks = vec![task1, task2];
                Some(PcbAction::Task(key, Task::batch(tasks)))
            }
            PcbUiCommand::Load => {
                debug!("Loading pcb. path: {:?}", self.path);

                let path = self.path.clone();

                self.planner_core_service
                    .update(Event::LoadPcb {
                        path,
                    })
                    .when_ok(key, |_| Some(PcbUiCommand::Loaded))
            }
            PcbUiCommand::Loaded => {
                debug!("Loaded pcb. path: {:?}", self.path);

                let task1 = self.show_explorer(self.path.clone());
                let task2 = self.show_configuration(self.path.clone());
                let tasks = vec![task1, task2];
                Some(PcbAction::Task(key, Task::batch(tasks)))
            }
            PcbUiCommand::Save => {
                debug!("Saving pcb. path: {:?}", self.path);

                let path = self.path.clone();

                self.planner_core_service
                    .update(Event::SavePcb {
                        path,
                    })
                    .when_ok(key, |_| Some(PcbUiCommand::Saved))
            }
            PcbUiCommand::Saved => {
                debug!("Saved pcb. path: {:?}", self.path);
                None
            }
            PcbUiCommand::Error(error) => {
                error!("PCB error. error: {:?}", error);
                // TODO show a dialog for PCB errors
                None
            }
            PcbUiCommand::PcbView(view) => {
                match view {
                    PcbView::PcbOverview(pcb_overview) => {
                        debug!("Received pcb overview.");

                        let mut pcb_ui_state = self.pcb_ui_state.lock().unwrap();

                        for gerber_viewer_ui in pcb_ui_state
                            .gerber_viewer_tab_uis
                            .values_mut()
                        {
                            gerber_viewer_ui.update_pcb_overview(pcb_overview.clone());
                        }

                        pcb_ui_state
                            .configuration_tab_ui
                            .update_pcb_overview(pcb_overview.clone());
                        pcb_ui_state
                            .explorer_tab_ui
                            .update_pcb_overview(pcb_overview.clone());
                        pcb_ui_state
                            .panel_tab_ui
                            .update_pcb_overview(pcb_overview);
                    }
                    PcbView::PanelSizing(panel_sizing) => {
                        debug!("Received panel sizing.");

                        let mut pcb_ui_state = self.pcb_ui_state.lock().unwrap();

                        pcb_ui_state
                            .panel_tab_ui
                            .update_panel_sizing(panel_sizing);
                    }
                }
                None
            }
            PcbUiCommand::SetModifiedState {
                pcbs_modified, ..
            } => {
                // FIXME we want to know if *THIS* pcb is modified, not any pcb.
                self.modified = pcbs_modified;
                Some(PcbAction::SetModifiedState(pcbs_modified))
            }
            PcbUiCommand::RequestPcbView(view_request) => {
                let event = match view_request {
                    PcbViewRequest::Overview {
                        path,
                    } => Some(Event::RequestPcbOverviewView {
                        path,
                    }),
                    PcbViewRequest::Panel {
                        path,
                    } => Some(Event::RequestPcbPanelSizingView {
                        path,
                    }),
                };

                // TODO remove the 'if let'
                if let Some(event) = event {
                    self.planner_core_service
                        .update(event)
                        .when_ok(key, |_| None)
                } else {
                    None
                }
            }

            //
            // tabs
            //
            PcbUiCommand::ShowExplorer => {
                let task = self.show_explorer(self.path.clone());
                Some(PcbAction::Task(key, task))
            }
            PcbUiCommand::ShowConfiguration => {
                let task = self.show_configuration(self.path.clone());
                Some(PcbAction::Task(key, task))
            }
            PcbUiCommand::ShowGerberViewer(args) => {
                let task = self.show_gerber_viewer(key, args);

                task.map(|task| PcbAction::Task(key, task))
            }
            PcbUiCommand::ShowPanel => {
                let task = self.show_panel(self.path.clone());
                Some(PcbAction::Task(key, task))
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
            PcbUiCommand::ExplorerTabUiCommand(command) => {
                let context = &mut ExplorerTabUiContext::default();
                let explorer_ui_action = self
                    .pcb_ui_state
                    .lock()
                    .unwrap()
                    .explorer_tab_ui
                    .update(command, context);
                match explorer_ui_action {
                    Some(ExplorerTabUiAction::Navigate(path)) => self.navigate(&key, path),
                    None => None,
                }
            }
            PcbUiCommand::ConfigurationTabUiCommand(command) => {
                let context = &mut ConfigurationTabUiContext::default();
                let configuration_ui_action = self
                    .pcb_ui_state
                    .lock()
                    .unwrap()
                    .configuration_tab_ui
                    .update(command, context);
                match configuration_ui_action {
                    None => None,
                    Some(ConfigurationTabUiAction::None) => None,

                    //
                    // form
                    //
                    Some(ConfigurationTabUiAction::Reset) => self
                        .planner_core_service
                        .update(Event::RequestPcbOverviewView {
                            path: self.path.clone(),
                        })
                        .when_ok(key, |_| None),
                    Some(ConfigurationTabUiAction::Apply(args)) => self
                        .planner_core_service
                        .update(Event::ApplyPcbUnitConfiguration {
                            path: self.path.clone(),
                            units: args.units,
                            designs: args.designs,
                            unit_map: args.unit_map,
                        })
                        .when_ok(key, |_| {
                            Some(PcbUiCommand::RequestPcbView(PcbViewRequest::Overview {
                                path: self.path.clone(),
                            }))
                        }),

                    //
                    // gerber file management
                    //
                    Some(ConfigurationTabUiAction::AddGerberFiles {
                        path,
                        design,
                        files,
                    }) => {
                        match self
                            .planner_core_service
                            .update(Event::AddGerberFiles {
                                path: path.clone(),
                                design,
                                files,
                            })
                            .into_actions()
                        {
                            Ok(actions) => {
                                let mut tasks = actions
                                    .into_iter()
                                    .map(Task::done)
                                    .collect::<Vec<_>>();

                                let additional_tasks = vec![Task::done(PcbAction::UiCommand(
                                    PcbUiCommand::RequestPcbView(PcbViewRequest::Overview {
                                        path,
                                    }),
                                ))];
                                tasks.extend(additional_tasks);

                                Some(PcbAction::Task(key, Task::batch(tasks)))
                            }
                            Err(error_action) => Some(error_action),
                        }
                    }
                    Some(ConfigurationTabUiAction::RemoveGerberFiles {
                        path,
                        design,
                        files,
                    }) => {
                        match self
                            .planner_core_service
                            .update(Event::RemoveGerberFiles {
                                path: path.clone(),
                                design,
                                files,
                            })
                            .into_actions()
                        {
                            Ok(actions) => {
                                let mut tasks = actions
                                    .into_iter()
                                    .map(Task::done)
                                    .collect::<Vec<_>>();

                                let additional_tasks = vec![Task::done(PcbAction::UiCommand(
                                    PcbUiCommand::RequestPcbView(PcbViewRequest::Overview {
                                        path,
                                    }),
                                ))];
                                tasks.extend(additional_tasks);

                                Some(PcbAction::Task(key, Task::batch(tasks)))
                            }
                            Err(error_action) => Some(error_action),
                        }
                    }
                }
            }
            PcbUiCommand::PanelTabUiCommand(command) => {
                let context = &mut PanelTabUiContext::default();
                let panel_tab_ui_action = self
                    .pcb_ui_state
                    .lock()
                    .unwrap()
                    .panel_tab_ui
                    .update(command, context);
                match panel_tab_ui_action {
                    Some(PanelTabUiAction::None) => None,
                    Some(PanelTabUiAction::Apply(panel_sizing)) => self
                        .planner_core_service
                        .update(Event::ApplyPanelSizing {
                            path: self.path.clone(),
                            panel_sizing,
                        })
                        .when_ok(key, |_| {
                            Some(PcbUiCommand::PanelTabUiCommand(PanelTabUiCommand::PanelSizingSaved))
                        }),
                    None => None,
                }
            }
            PcbUiCommand::GerberViewerTabUiCommand {
                args,
                command,
            } => {
                let context = &mut GerberViewerTabUiContext::default();

                let gerber_viewer_ui_action = self
                    .pcb_ui_state
                    .lock()
                    .unwrap()
                    .gerber_viewer_tab_uis
                    .get_mut(&args)
                    .unwrap()
                    .update(command, context);

                match gerber_viewer_ui_action {
                    Some(GerberViewerTabUiAction::None) => None,
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
        fn when_ok<F>(self, pcb_key: PcbKey, f: F) -> Option<PcbAction>
        where
            F: FnOnce(&mut Vec<Task<PcbAction>>) -> Option<PcbUiCommand>;
    }

    impl PcbCoreHelper for Result<Vec<PlannerAction>, PlannerError> {
        fn into_actions(self) -> Result<Vec<PcbAction>, PcbAction> {
            into_actions_inner(self)
        }

        fn when_ok<F>(self, pcb_key: PcbKey, f: F) -> Option<PcbAction>
        where
            F: FnOnce(&mut Vec<Task<PcbAction>>) -> Option<PcbUiCommand>,
        {
            when_ok_inner(self, pcb_key, f)
        }
    }
}

pub(crate) fn make_tabs(key: PcbKey) -> Value<PcbTabs> {
    debug!("Initializing pcb tabs for tab. key: {:?}", key);

    Value::new(PcbTabs::default())
}
