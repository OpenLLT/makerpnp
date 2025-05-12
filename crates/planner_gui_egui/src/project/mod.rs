use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use std::path::PathBuf;
use std::str::FromStr;

use egui::{Ui, Widget, WidgetText};
use egui_i18n::tr;
use egui_mobius::types::{Enqueue, Value};
use planner_app::{
    AddOrRemoveAction, DesignName, Event, LoadOutSource, ObjectPath, PhaseOverview, PhaseReference, PlacementOperation,
    PlacementState, PlacementStatus, ProcessReference, ProjectOverview, ProjectView, ProjectViewRequest, Reference,
    SetOrClearAction, VariantName,
};
use regex::Regex;
use slotmap::new_key_type;
use tracing::{debug, error, info, trace};

use crate::planner_app_core::PlannerCoreService;
use crate::project::dialogs::add_pcb::{AddPcbModal, AddPcbModalAction, AddPcbModalUiCommand};
use crate::project::dialogs::add_phase::{AddPhaseModal, AddPhaseModalAction, AddPhaseModalUiCommand};
use crate::project::explorer_tab::{ExplorerTab, ExplorerUi, ExplorerUiAction, ExplorerUiCommand, ExplorerUiContext};
use crate::project::load_out_tab::{LoadOutTab, LoadOutUi, LoadOutUiAction, LoadOutUiCommand, LoadOutUiContext};
use crate::project::overview_tab::{OverviewTab, OverviewUi, OverviewUiAction, OverviewUiCommand, OverviewUiContext};
use crate::project::parts_tab::{PartsTab, PartsUi, PartsUiAction, PartsUiCommand, PartsUiContext};
use crate::project::pcb_tab::{PcbTab, PcbUi, PcbUiAction, PcbUiCommand, PcbUiContext};
use crate::project::phase_tab::{PhaseTab, PhaseUi, PhaseUiAction, PhaseUiCommand, PhaseUiContext};
use crate::project::placements_tab::{
    PlacementsTab, PlacementsUi, PlacementsUiAction, PlacementsUiCommand, PlacementsUiContext,
};
use crate::project::tabs::{ProjectTabAction, ProjectTabContext, ProjectTabUiCommand, ProjectTabs};
use crate::project::toolbar::{ProjectToolbar, ProjectToolbarAction, ProjectToolbarUiCommand};
use crate::project::unit_assignments_tab::{
    UnitAssignmentsTab, UnitAssignmentsUi, UnitAssignmentsUiAction, UnitAssignmentsUiCommand, UnitAssignmentsUiContext,
    UpdateUnitAssignmentsArgs,
};
use crate::tabs::{Tab, TabKey};
use crate::task::Task;
use crate::ui_component::{ComponentState, UiComponent};

//
// tabs
//
mod explorer_tab;
mod load_out_tab;
mod overview_tab;
mod parts_tab;
mod pcb_tab;
mod phase_tab;
mod placements_tab;
mod unit_assignments_tab;

//
// other modules
//
mod dialogs;
mod process;
mod tables;
mod tabs;
mod toolbar;

new_key_type! {
    /// A key for a project
    pub struct ProjectKey;
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectPath(String);

impl ProjectPath {
    pub fn new(path: String) -> Self {
        Self(path)
    }
}

impl Deref for ProjectPath {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for ProjectPath {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for ProjectPath {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[derive(Debug)]
pub enum ProjectAction {
    Task(ProjectKey, Task<ProjectAction>),
    SetModifiedState(bool),
    UiCommand(ProjectUiCommand),
    RequestRepaint,
}

pub struct Project {
    planner_core_service: PlannerCoreService,
    path: PathBuf,
    project_ui_state: Value<ProjectUiState>,

    modified: bool,

    /// list of errors to show
    errors: Vec<(chrono::DateTime<chrono::Utc>, String)>,

    /// initially empty until the OverviewView has been received and processed.
    processes: Vec<ProcessReference>,

    /// initially empty, requires fetching the PhaseOverviewView for each phase before it can be used.
    phases: Vec<PhaseOverview>,

    // FIXME actually persist this, currently it should be treated as 'persistable_state'.
    project_tabs: Value<ProjectTabs>,

    toolbar: ProjectToolbar,

    pub component: ComponentState<(ProjectKey, ProjectUiCommand)>,

    add_pcb_modal: Option<AddPcbModal>,
    add_phase_modal: Option<AddPhaseModal>,
}

impl Project {
    pub fn from_path(path: PathBuf, key: ProjectKey) -> (Self, ProjectUiCommand) {
        let instance = Self::new_inner(path, key, None);
        (instance, ProjectUiCommand::Load)
    }

    pub fn new(name: String, path: PathBuf, key: ProjectKey) -> (Self, ProjectUiCommand) {
        let instance = Self::new_inner(path, key, Some(name));
        (instance, ProjectUiCommand::Create)
    }

    fn new_inner(path: PathBuf, key: ProjectKey, name: Option<String>) -> Self {
        debug!("Creating project instance from path. path: {}", &path.display());

        let component: ComponentState<(ProjectKey, ProjectUiCommand)> = ComponentState::default();
        let component_sender = component.sender.clone();

        let mut toolbar = ProjectToolbar::default();
        toolbar
            .component
            .configure_mapper(component_sender.clone(), move |command| {
                trace!("project toolbar mapper. command: {:?}", command);
                (key, ProjectUiCommand::ToolbarCommand(command))
            });

        let project_directory = path.parent().unwrap().to_path_buf();
        let project_ui_state = Value::new(ProjectUiState::new(
            key,
            project_directory,
            name,
            component_sender.clone(),
        ));

        let project_tabs = Value::new(ProjectTabs::default());
        {
            let mut project_tabs = project_tabs.lock().unwrap();
            project_tabs
                .component
                .configure_mapper(component_sender, move |command| {
                    trace!("project inner-tab mapper. command: {:?}", command);
                    (key, ProjectUiCommand::TabCommand(command))
                });
            project_tabs.add_tab(ProjectTabKind::Explorer(ExplorerTab::default()));
        }

        let core_service = PlannerCoreService::new();
        Self {
            path,
            planner_core_service: core_service,
            project_ui_state,
            modified: false,
            errors: Default::default(),
            processes: Default::default(),
            phases: Default::default(),
            project_tabs,
            toolbar,
            component,
            add_pcb_modal: None,
            add_phase_modal: None,
        }
    }

    pub fn show_explorer(&mut self) -> Task<ProjectAction> {
        let mut project_tabs = self.project_tabs.lock().unwrap();
        let result = project_tabs.show_tab(|candidate_tab| matches!(candidate_tab, ProjectTabKind::Explorer(_)));
        if result.is_err() {
            project_tabs.add_tab(ProjectTabKind::Explorer(ExplorerTab::default()));
        }

        Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestView(
            ProjectViewRequest::ProjectTree,
        )))
    }

    pub fn show_overview(&mut self) -> Task<ProjectAction> {
        let mut project_tabs = self.project_tabs.lock().unwrap();
        let result = project_tabs.show_tab(|candidate_tab| matches!(candidate_tab, ProjectTabKind::Overview(_)));
        if result.is_err() {
            project_tabs.add_tab_to_second_leaf_or_split(ProjectTabKind::Overview(OverviewTab::default()));
        }

        Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestView(
            ProjectViewRequest::Overview,
        )))
    }

    pub fn show_parts(&mut self) -> Task<ProjectAction> {
        let mut project_tabs = self.project_tabs.lock().unwrap();
        let result = project_tabs.show_tab(|candidate_tab| matches!(candidate_tab, ProjectTabKind::Parts(_)));
        if result.is_err() {
            project_tabs.add_tab_to_second_leaf_or_split(ProjectTabKind::Parts(PartsTab::default()));
        }

        Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestView(
            ProjectViewRequest::Parts,
        )))
    }

    pub fn show_placements(&mut self) -> Task<ProjectAction> {
        let mut project_tabs = self.project_tabs.lock().unwrap();
        let result = project_tabs.show_tab(|candidate_tab| matches!(candidate_tab, ProjectTabKind::Placements(_)));
        if result.is_err() {
            project_tabs.add_tab_to_second_leaf_or_split(ProjectTabKind::Placements(PlacementsTab::default()));
        }

        Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestView(
            ProjectViewRequest::Placements,
        )))
    }

    pub fn show_phase(&mut self, key: ProjectKey, phase: Reference) -> Option<Vec<Task<ProjectAction>>> {
        let mut project_tabs = self.project_tabs.lock().unwrap();
        let tab = PhaseTab::new(phase.clone());
        
        let mut tasks = None;
        project_tabs
            .show_tab(
                |candidate_tab_kind| matches!(candidate_tab_kind, ProjectTabKind::Phase(candidate_tab) if candidate_tab.eq(&tab)),
            )
            .inspect(|tab_key| {
                debug!("showing existing phase tab. phase: {:?}, tab_key: {:?}", phase, tab_key);
            })
            .inspect_err(|_| {
                self.ensure_phase(key, &phase);
                
                tasks = Some(vec![
                    Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestView(
                        ProjectViewRequest::PhaseOverview {
                            phase: phase.clone(),
                        },
                    ))),
                    Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestView(
                        ProjectViewRequest::PhasePlacements {
                            phase: phase.clone(),
                        },
                    ))),
                ]);

                let tab_key = project_tabs.add_tab_to_second_leaf_or_split(ProjectTabKind::Phase(tab));
                debug!("adding phase tab. phase: {:?}, tab_key: {:?}", phase, tab_key);
            })
            .ok();
        
        tasks
    }

    pub fn show_loadout(&self, key: ProjectKey, phase: Reference, load_out_source: &LoadOutSource) -> Option<Task<ProjectAction>> {
        let project_directory = self.path.parent().unwrap();

        let mut project_tabs = self.project_tabs.lock().unwrap();
        let tab = LoadOutTab::new(project_directory.into(), load_out_source.clone());
        
        let mut task = None;
        
        project_tabs
            .show_tab(|candidate_tab_kind| {
                matches!(candidate_tab_kind, ProjectTabKind::LoadOut(candidate_tab) if candidate_tab.eq(&tab))
            })
            .inspect(|tab_key| {
                debug!("showing existing load-out tab. load_out_source: {:?}, tab_key: {:?}", load_out_source, tab_key);
            })
            .inspect_err(|_| {
                self.ensure_load_out(key, phase.clone(), load_out_source);

                task = Some(Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestView(
                    ProjectViewRequest::PhaseLoadOut {
                        phase,
                    },
                ))));

                let tab_key = project_tabs.add_tab_to_second_leaf_or_split(ProjectTabKind::LoadOut(tab));
                debug!("adding load-out tab. load_out_source: {:?}, tab_key: {:?}", load_out_source, tab_key);
            })
            .ok();
        
        task
    }

    pub fn show_pcb(&mut self, key: ProjectKey, pcb_index: u16) -> Option<Vec<Task<ProjectAction>>> {
        let mut project_tabs = self.project_tabs.lock().unwrap();
        let tab = PcbTab::new(pcb_index);

        let mut tasks = None;

        project_tabs
            .show_tab(|candidate_tab_kind| {
                matches!(candidate_tab_kind, ProjectTabKind::Pcb(candidate_tab) if candidate_tab.eq(&tab))
            })
            .inspect(|tab_key|{
                debug!("showing existing pcb tab. pcb: {:?}, tab_key: {:?}", pcb_index, tab_key);
            })
            .inspect_err(|_|{
                self.ensure_pcb(key, pcb_index);

                tasks = Some(vec![Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestView(
                    ProjectViewRequest::PcbOverview {
                        pcb: pcb_index.clone(),
                    },
                )))]);



                let tab_key = project_tabs.add_tab_to_second_leaf_or_split(ProjectTabKind::Pcb(tab));
                debug!("adding pcb tab. pcb_index: {:?}, tab_key: {:?}", pcb_index, tab_key);
            })
            .ok();
        
        tasks
    }

    pub fn show_unit_assignments(&mut self, key: ProjectKey, pcb_index: u16) -> Option<Vec<Task<ProjectAction>>> {
        let mut project_tabs = self.project_tabs.lock().unwrap();
        let tab = UnitAssignmentsTab::new(pcb_index);
        
        let mut tasks = None;
        
        project_tabs
            .show_tab(|candidate_tab_kind| {
                matches!(candidate_tab_kind, ProjectTabKind::UnitAssignments(candidate_tab) if candidate_tab.eq(&tab))
            })
            .inspect(|tab_key|{
                debug!("showing existing unit assignments tab. pcb: {:?}, tab_key: {:?}", pcb_index, tab_key);
            })
            .inspect_err(|_|{
                self.ensure_unit_assignments(key, pcb_index);
                tasks = Some(vec![
                    Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestView(
                        ProjectViewRequest::PcbOverview {
                            pcb: pcb_index,
                        },
                    ))),
                    Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestView(
                        ProjectViewRequest::PcbUnitAssignments {
                            pcb: pcb_index,
                        },
                    ))),
                ]);

                let tab_key = project_tabs.add_tab_to_second_leaf_or_split(ProjectTabKind::UnitAssignments(tab));
                debug!("adding unit assignments tab. pcb_index: {:?}, tab_key: {:?}", pcb_index, tab_key);
            })
            .ok();
        
        tasks
    }

    fn ensure_phase(&self, key: ProjectKey, phase: &Reference) {
        let phase = phase.clone();
        let mut state = self.project_ui_state.lock().unwrap();
        let _phase_state = state
            .phases
            .entry(phase.clone())
            .or_insert_with(|| {
                debug!("ensuring phase ui. phase: {:?}", phase);
                let mut phase_ui = PhaseUi::new();
                phase_ui
                    .component
                    .configure_mapper(self.component.sender.clone(), {
                        move |command| {
                            trace!("phase ui mapper. command: {:?}", command);
                            (key, ProjectUiCommand::PhaseUiCommand {
                                phase: phase.clone(),
                                command,
                            })
                        }
                    });

                phase_ui
            });
    }

    fn ensure_load_out(&self, key: ProjectKey, phase: Reference, load_out_source: &LoadOutSource) {
        let load_out_source = load_out_source.clone();
        let mut state = self.project_ui_state.lock().unwrap();
        let _load_out_ui = state
            .load_outs
            .entry(load_out_source.clone())
            .or_insert_with(|| {
                debug!("ensuring load out ui. source: {:?}", load_out_source);
                let mut load_out_ui = LoadOutUi::new(phase);
                load_out_ui
                    .component
                    .configure_mapper(self.component.sender.clone(), {
                        move |command| {
                            trace!("load out ui mapper. command: {:?}", command);
                            (key, ProjectUiCommand::LoadOutUiCommand {
                                load_out_source: load_out_source.clone(),
                                command,
                            })
                        }
                    });

                load_out_ui
            });
    }

    fn ensure_pcb(&self, key: ProjectKey, pcb_index: u16) {
        let mut state = self.project_ui_state.lock().unwrap();
        let _pcb_ui = state
            .pcbs
            .entry(pcb_index as usize)
            .or_insert_with(|| {
                debug!("ensuring pcb ui. pcb_index: {:?}", pcb_index);
                let mut pcb_ui = PcbUi::new(self.path.clone());
                pcb_ui
                    .component
                    .configure_mapper(self.component.sender.clone(), {
                        move |command| {
                            trace!("pcb ui mapper. command: {:?}", command);
                            (key, ProjectUiCommand::PcbUiCommand {
                                pcb_index,
                                command,
                            })
                        }
                    });

                pcb_ui
            });
    }

    fn ensure_unit_assignments(&self, key: ProjectKey, pcb_index: u16) {
        let mut state = self.project_ui_state.lock().unwrap();
        let _unit_assignments_ui = state
            .unit_assignments
            .entry(pcb_index as usize)
            .or_insert_with(|| {
                debug!("ensuring unit assignments ui. pcb_index: {:?}", pcb_index);
                let mut unit_assignments_ui = UnitAssignmentsUi::new(self.path.clone());
                unit_assignments_ui
                    .component
                    .configure_mapper(self.component.sender.clone(), {
                        move |command| {
                            trace!("pcb ui mapper. command: {:?}", command);
                            (key, ProjectUiCommand::UnitAssignmentsUiCommand {
                                pcb_index,
                                command,
                            })
                        }
                    });

                unit_assignments_ui
            });
    }

    fn navigate(&mut self, key: ProjectKey, path: ProjectPath) -> Option<ProjectAction> {
        // if the path starts with `/project/` then show/hide UI elements based on the path,
        // e.g. update a dynamic that controls a per-project-tab-bar dynamic selector
        info!("ProjectMessage::Navigate. path: {}", path);

        #[must_use]
        fn handle_root(project: &mut Project, key: &ProjectKey, path: &ProjectPath) -> Option<ProjectAction> {
            if path.eq(&"/project/".into()) {
                let task = project.show_overview();
                Some(ProjectAction::Task(*key, task))
            } else {
                None
            }
        }

        #[must_use]
        fn handle_placements(project: &mut Project, key: &ProjectKey, path: &ProjectPath) -> Option<ProjectAction> {
            if path.eq(&"/project/placements".into()) {
                let task = project.show_placements();

                Some(ProjectAction::Task(*key, task))
            } else {
                None
            }
        }

        #[must_use]
        fn handle_parts(project: &mut Project, key: &ProjectKey, path: &ProjectPath) -> Option<ProjectAction> {
            if path.eq(&"/project/parts".into()) {
                let task = project.show_parts();

                Some(ProjectAction::Task(*key, task))
            } else {
                None
            }
        }

        #[must_use]
        fn handle_phases(_project: &mut Project, key: &ProjectKey, path: &ProjectPath) -> Option<ProjectAction> {
            if path.eq(&"/project/phases".into()) {
                let task = Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestView(
                    ProjectViewRequest::Phases,
                )));

                Some(ProjectAction::Task(*key, task))
            } else {
                None
            }
        }

        #[must_use]
        fn handle_phase(project: &mut Project, key: &ProjectKey, path: &ProjectPath) -> Option<ProjectAction> {
            let phase_pattern = Regex::new(r"^/project/phases/(?<phase>[^/]*){1}$").unwrap();
            if let Some(captures) = phase_pattern.captures(&path) {
                let phase_reference: String = captures
                    .name("phase")
                    .unwrap()
                    .as_str()
                    .to_string();
                debug!("phase_reference: {}", phase_reference);

                let reference = Reference::from_raw(phase_reference);

                let tasks = project.show_phase(key.clone(), reference.clone());

                tasks.map(|tasks|ProjectAction::Task(*key, Task::batch(tasks)))
            } else {
                None
            }
        }

        #[must_use]
        fn handle_phase_loadout(_project: &mut Project, key: &ProjectKey, path: &ProjectPath) -> Option<ProjectAction> {
            let phase_pattern = Regex::new(r"^/project/phases/(?<phase>[^/]*){1}/loadout$").unwrap();
            if let Some(captures) = phase_pattern.captures(&path) {
                let phase_reference: String = captures
                    .name("phase")
                    .unwrap()
                    .as_str()
                    .to_string();
                debug!("phase_reference: {}", phase_reference);

                let tasks: Vec<_> = vec![
                    Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestView(
                        ProjectViewRequest::Phases,
                    ))),
                    Task::done(ProjectAction::UiCommand(ProjectUiCommand::ShowPhaseLoadout {
                        phase: Reference(phase_reference),
                    })),
                ];

                Some(ProjectAction::Task(*key, Task::batch(tasks)))
            } else {
                None
            }
        }

        #[must_use]
        fn handle_pcb(project: &mut Project, key: &ProjectKey, path: &ProjectPath) -> Option<ProjectAction> {
            let phase_pattern = Regex::new(r"^/project/pcbs/(?<pcb>[0-9]?){1}$").unwrap();
            if let Some(captures) = phase_pattern.captures(&path) {
                let pcb_index = captures
                    .name("pcb")
                    .unwrap()
                    .as_str()
                    .parse::<u16>()
                    .unwrap();
                debug!("pcb: {}", pcb_index);

                let tasks = project.show_pcb(key.clone(), pcb_index);

                tasks.map(|tasks|ProjectAction::Task(*key, Task::batch(tasks)))
            } else {
                None
            }
        }

        #[must_use]
        fn handle_unit_assignments(
            project: &mut Project,
            key: &ProjectKey,
            path: &ProjectPath,
        ) -> Option<ProjectAction> {
            let phase_pattern = Regex::new(r"^/project/pcbs/(?<pcb>[0-9]?){1}/units(?:.*)?$").unwrap();
            if let Some(captures) = phase_pattern.captures(&path) {
                let pcb_index = captures
                    .name("pcb")
                    .unwrap()
                    .as_str()
                    .parse::<u16>()
                    .unwrap();
                debug!("pcb: {}", pcb_index);

                let tasks = project.show_unit_assignments(*key, pcb_index);
                
                tasks.map(|tasks|ProjectAction::Task(*key, Task::batch(tasks)))
            } else {
                None
            }
        }

        let handlers = [
            handle_root,
            handle_parts,
            handle_phases,
            handle_phase_loadout,
            handle_placements,
            handle_phase,
            handle_pcb,
            handle_unit_assignments,
        ];

        handlers
            .iter()
            .find_map(|handler| handler(self, &key, &path))
    }

    pub fn update_processes(&mut self, project_overview: &ProjectOverview) {
        self.processes = project_overview.processes.clone();
    }

    #[must_use]
    fn update_placement(
        planner_core_service: &mut PlannerCoreService,
        key: ProjectKey,
        object_path: ObjectPath,
        new_placement: PlacementState,
        old_placement: PlacementState,
    ) -> (Vec<Task<ProjectAction>>, Vec<UpdatePlacementAction>) {
        let mut tasks = vec![];

        fn handle_phase(
            planner_core_service: &mut PlannerCoreService,
            key: &ProjectKey,
            object_path: &ObjectPath,
            new_placement: &PlacementState,
            old_placement: &PlacementState,
        ) -> Option<(Vec<UpdatePlacementAction>, Result<Vec<ProjectAction>, ProjectAction>)> {
            if !new_placement
                .phase
                .eq(&old_placement.phase)
            {
                let (phase, operation, update_placement_actions) = match (&new_placement.phase, &old_placement.phase) {
                    (Some(new_phase), None) => (new_phase, SetOrClearAction::Set, vec![
                        UpdatePlacementAction::RefreshPhasePlacements {
                            phase: new_phase.clone(),
                        },
                    ]),
                    (Some(new_phase), Some(old_phase)) => (new_phase, SetOrClearAction::Set, vec![
                        UpdatePlacementAction::RefreshPhasePlacements {
                            phase: new_phase.clone(),
                        },
                        UpdatePlacementAction::RefreshPhasePlacements {
                            phase: old_phase.clone(),
                        },
                    ]),
                    (None, Some(old_phase)) => (old_phase, SetOrClearAction::Clear, vec![
                        UpdatePlacementAction::RefreshPhasePlacements {
                            phase: old_phase.clone(),
                        },
                    ]),
                    _ => unreachable!(),
                };

                Some((
                    update_placement_actions,
                    planner_core_service
                        .update(key.clone(), Event::AssignPlacementsToPhase {
                            phase: phase.clone(),
                            operation,
                            placements: exact_match(&object_path.to_string()),
                        })
                        .into_actions(),
                ))
            } else {
                None
            }
        }

        fn handle_placed(
            planner_core_service: &mut PlannerCoreService,
            key: &ProjectKey,
            object_path: &ObjectPath,
            new_placement: &PlacementState,
            old_placement: &PlacementState,
        ) -> Option<(Vec<UpdatePlacementAction>, Result<Vec<ProjectAction>, ProjectAction>)> {
            if new_placement.phase.is_none() {
                error!(
                    "Attempt to place a placement that has not been assigned to a phase. placement: {:?}",
                    new_placement
                );
                return None;
            }

            if new_placement.operation_status != old_placement.operation_status {
                let operation = match new_placement.operation_status {
                    PlacementStatus::Placed => PlacementOperation::Place,
                    PlacementStatus::Skipped => PlacementOperation::Skip,
                    PlacementStatus::Pending => PlacementOperation::Reset,
                };

                Some((
                    vec![UpdatePlacementAction::RefreshPhaseOverview {
                        phase: new_placement
                            .phase
                            .as_ref()
                            .unwrap()
                            .clone(),
                    }],
                    planner_core_service
                        .update(key.clone(), Event::RecordPlacementsOperation {
                            object_path_patterns: vec![exact_match(&object_path.to_string())],
                            operation,
                        })
                        .into_actions(),
                ))
            } else {
                None
            }
        }

        #[derive(Debug)]
        enum Operation {
            AddOrRemovePhase,
            SetOrResetPlaced,
        }

        // FUTURE find a solution to keep the operation with the handler, instead of two separate arrays.
        //        a tuple was tried, but results in a compile error: "expected fn item, found a different fn item"
        let actions = [Operation::AddOrRemovePhase, Operation::SetOrResetPlaced];

        let action_handlers = [handle_phase, handle_placed];

        let mut update_placement_actions = vec![];

        for (action, handler) in actions
            .into_iter()
            .zip(action_handlers.into_iter())
        {
            debug!("update placement, operation: {:?}", action);

            if let Some((additional_update_placement_actions, core_result)) =
                handler(planner_core_service, &key, &object_path, &new_placement, &old_placement)
            {
                match core_result {
                    Ok(actions) => {
                        debug!("actions: {:?}", actions);
                        let effect_tasks: Vec<Task<ProjectAction>> = actions
                            .into_iter()
                            .map(Task::done)
                            .collect();
                        tasks.extend(effect_tasks);
                        update_placement_actions.extend(additional_update_placement_actions);
                    }
                    Err(service_error) => {
                        tasks.push(Task::done(service_error));
                        break;
                    }
                }
            }
        }

        let final_task = Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestView(
            ProjectViewRequest::Placements,
        )));
        tasks.push(final_task);

        update_placement_actions.dedup();

        (tasks, update_placement_actions)
    }

    fn handle_update_placement_actions(tasks: &mut Vec<Task<ProjectAction>>, actions: Vec<UpdatePlacementAction>) {
        for action in actions {
            if let Some(task) = match action {
                UpdatePlacementAction::RefreshPhaseOverview {
                    phase,
                } => Some(Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestView(
                    ProjectViewRequest::PhaseOverview {
                        phase,
                    },
                )))),
                UpdatePlacementAction::RefreshPhasePlacements {
                    phase,
                } => Some(Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestView(
                    ProjectViewRequest::PhasePlacements {
                        phase,
                    },
                )))),
            } {
                tasks.push(task);
            }
        }
    }
}

#[derive(Debug, PartialEq)]
enum UpdatePlacementAction {
    RefreshPhaseOverview { phase: PhaseReference },
    RefreshPhasePlacements { phase: PhaseReference },
}

pub struct ProjectContext {
    pub key: ProjectKey,
}

impl UiComponent for Project {
    type UiContext<'context> = ProjectContext;
    type UiCommand = (ProjectKey, ProjectUiCommand);
    type UiAction = ProjectAction;

    fn ui<'context>(&self, ui: &mut Ui, context: &mut Self::UiContext<'context>) {
        let ProjectContext {
            key,
        } = context;

        egui::TopBottomPanel::top(ui.id().with("top_panel")).show_inside(ui, |ui| {
            ui.label(tr!("project-detail-path", { path: self.path.display().to_string() }));

            let state = self.project_ui_state.lock().unwrap();
            if let Some(name) = &state.name {
                ui.label(tr!("project-detail-name", { name: name }));
            } else {
                ui.spinner();
            }

            self.toolbar.ui(ui, &mut ());
        });

        let mut tab_context = ProjectTabContext {
            state: self.project_ui_state.clone(),
        };

        let mut project_tabs = self.project_tabs.lock().unwrap();
        project_tabs.cleanup_tabs(&mut tab_context);
        project_tabs.ui(ui, &mut tab_context);

        if !self.errors.is_empty() {
            dialogs::errors::show_errors_modal(ui, *key, &self.path, &self.errors, &self.component);
        }

        //
        // Modals
        //
        if let Some(dialog) = &self.add_pcb_modal {
            dialog.ui(ui, &mut ());
        }
        if let Some(dialog) = &self.add_phase_modal {
            dialog.ui(ui, &mut ());
        }
    }

    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        let (key, command) = command;

        match command {
            ProjectUiCommand::None => None,
            ProjectUiCommand::Load => {
                debug!("Loading project from path. path: {}", self.path.display());

                self.planner_core_service
                    .update(key, Event::Load {
                        path: self.path.clone(),
                    })
                    .when_ok(|_tasks| Some(ProjectUiCommand::Loaded))
            }
            ProjectUiCommand::Loaded => {
                match self
                    .planner_core_service
                    .update(key, Event::RequestOverviewView {})
                    .into_actions()
                {
                    Ok(actions) => {
                        let mut tasks = actions
                            .into_iter()
                            .map(Task::done)
                            .collect::<Vec<Task<ProjectAction>>>();
                        
                        let task1 = self.show_explorer();
                        let task2 = self.show_overview();

                        let additional_tasks = vec![task1, task2];
                        tasks.extend(additional_tasks);

                        Some(ProjectAction::Task(key, Task::batch(tasks)))
                    }
                    Err(error_action) => Some(error_action),
                }
            }
            ProjectUiCommand::Create => {
                let state = self.project_ui_state.lock().unwrap();
                self.planner_core_service
                    .update(key, Event::CreateProject {
                        name: state.name.clone().unwrap(),
                        path: self.path.clone(),
                    })
                    .when_ok(|_| Some(ProjectUiCommand::Created))
            }
            ProjectUiCommand::Created => self
                .planner_core_service
                .update(key, Event::RequestOverviewView {})
                .when_ok(|_| Some(ProjectUiCommand::RequestView(ProjectViewRequest::ProjectTree))),
            ProjectUiCommand::Save => {
                debug!("saving project. path: {}", self.path.display());
                self.planner_core_service
                    .update(key, Event::Save)
                    .when_ok(|_| Some(ProjectUiCommand::Saved))
            }
            ProjectUiCommand::Saved => {
                debug!("saved");
                None
            }
            ProjectUiCommand::ProjectRefreshed => {
                debug!("project refreshed");
                // TODO anything that is using data from views, this requires ui components to subscribe to refresh events or something.
                None
            }
            ProjectUiCommand::RequestView(view_request) => {
                let event = match view_request {
                    ProjectViewRequest::Overview => Event::RequestOverviewView {},
                    ProjectViewRequest::Parts => Event::RequestPartStatesView {},
                    ProjectViewRequest::Placements => Event::RequestPlacementsView {},
                    ProjectViewRequest::Phases => Event::RequestPhasesView {},
                    ProjectViewRequest::ProjectTree => Event::RequestProjectTreeView {},
                    ProjectViewRequest::PhaseOverview {
                        phase,
                    } => Event::RequestPhaseOverviewView {
                        phase_reference: phase,
                    },
                    ProjectViewRequest::PhaseLoadOut {
                        phase,
                    } => Event::RequestPhaseLoadOutView {
                        phase_reference: phase,
                    },
                    ProjectViewRequest::PhasePlacements {
                        phase,
                    } => Event::RequestPhasePlacementsView {
                        phase_reference: phase,
                    },
                    ProjectViewRequest::PcbOverview {
                        pcb,
                    } => Event::RequestPcbOverviewView {
                        pcb,
                    },
                    ProjectViewRequest::PcbUnitAssignments {
                        pcb,
                    } => Event::RequestPcbUnitAssignmentsView {
                        pcb,
                    },
                };

                self.planner_core_service
                    .update(key, event)
                    .when_ok(|_| None)
            }
            ProjectUiCommand::UpdateView(view) => {
                match view {
                    ProjectView::Overview(project_overview) => {
                        trace!("project overview: {:?}", project_overview);
                        self.update_processes(&project_overview);

                        let mut state = self.project_ui_state.lock().unwrap();
                        state.name = Some(project_overview.name.clone());
                        state
                            .overview_ui
                            .update_overview(project_overview);
                    }
                    ProjectView::ProjectTree(project_tree) => {
                        trace!("project tree: {:?}", project_tree);
                        let mut state = self.project_ui_state.lock().unwrap();
                        state
                            .explorer_ui
                            .update_tree(project_tree);
                    }
                    ProjectView::PcbOverview(pcb_overview) => {
                        trace!("pcb_overview: {:?}", pcb_overview);

                        // TODO use the name to update the tab label of the pcb overview and unit_assignments tabs?
                        //      would need multiple actions...
                        //      A reactive 'Reactive<Option<PcbOverview>>' that's given to both tabs
                        //      would be perfect here to avoid needing any actions.
                        let name = pcb_overview.name.clone();

                        let mut state = self.project_ui_state.lock().unwrap();

                        if let Some(pcb_ui) = state
                            .pcbs
                            .get_mut(&(pcb_overview.index as usize))
                        {
                            pcb_ui.update_overview(pcb_overview.clone());
                        }

                        if let Some(unit_assignments_ui) = state
                            .unit_assignments
                            .get_mut(&(pcb_overview.index as usize))
                        {
                            unit_assignments_ui.update_overview(pcb_overview);
                        }
                    }
                    ProjectView::PcbUnitAssignments(pcb_unit_assignments) => {
                        trace!("pcb_unit_assignments: {:?}", pcb_unit_assignments);

                        let mut state = self.project_ui_state.lock().unwrap();

                        if let Some(unit_assignments_ui) = state
                            .unit_assignments
                            .get_mut(&(pcb_unit_assignments.index as usize))
                        {
                            unit_assignments_ui.update_unit_assignments(pcb_unit_assignments);
                        }
                    }
                    ProjectView::Placements(placements) => {
                        trace!("placements: {:?}", placements);
                        let mut state = self.project_ui_state.lock().unwrap();
                        state
                            .placements_ui
                            .update_placements(placements, self.phases.clone())
                    }
                    ProjectView::Phases(phases) => {
                        trace!("phases: {:?}", phases);
                        self.phases = phases.phases;
                    }
                    ProjectView::PhaseOverview(phase_overview) => {
                        trace!("phase overview: {:?}", phase_overview);
                        let phase = phase_overview.phase_reference.clone();

                        self.ensure_phase(key, &phase);

                        let mut state = self.project_ui_state.lock().unwrap();
                        let phase_ui = state.phases.get_mut(&phase).unwrap();

                        phase_ui.update_overview(phase_overview);
                    }
                    ProjectView::PhasePlacements(phase_placements) => {
                        trace!("phase placements: {:?}", phase_placements);
                        let phase = phase_placements.phase_reference.clone();

                        self.ensure_phase(key, &phase);

                        let mut state = self.project_ui_state.lock().unwrap();
                        let phase_ui = state.phases.get_mut(&phase).unwrap();

                        phase_ui.update_placements(phase_placements, self.phases.clone());
                    }
                    ProjectView::Process(_process) => {
                        // TODO
                    }
                    ProjectView::Parts(part_states) => {
                        trace!("parts: {:?}", part_states);
                        let mut state = self.project_ui_state.lock().unwrap();

                        state
                            .parts_ui
                            .update_part_states(part_states, self.processes.clone())
                    }
                    ProjectView::PhaseLoadOut(load_out) => {
                        trace!("load_out: {:?}", load_out);
                        let load_out_source = load_out.source.clone();

                        self.ensure_load_out(key, load_out.phase_reference.clone(), &load_out_source);

                        let mut state = self.project_ui_state.lock().unwrap();
                        let load_out_ui = state
                            .load_outs
                            .get_mut(&load_out_source)
                            .unwrap();

                        load_out_ui.update_load_out(load_out);
                    }
                }
                None
            }
            ProjectUiCommand::Error(error) => {
                match error {
                    ProjectError::CoreError(message) => {
                        self.errors.push(message);
                    }
                }
                None
            }
            ProjectUiCommand::ClearErrors => {
                self.errors.clear();
                None
            }
            ProjectUiCommand::SetModifiedState(modified_state) => {
                self.modified = modified_state;
                Some(ProjectAction::SetModifiedState(modified_state))
            }
            ProjectUiCommand::ExplorerUiCommand(command) => {
                let context = &mut ExplorerUiContext::default();
                let explorer_ui_action = self
                    .project_ui_state
                    .lock()
                    .unwrap()
                    .explorer_ui
                    .update(command, context);
                match explorer_ui_action {
                    Some(ExplorerUiAction::Navigate(path)) => self.navigate(key, path),
                    None => None,
                }
            }
            ProjectUiCommand::OverviewUiCommand(command) => {
                let context = &mut OverviewUiContext::default();
                let overview_ui_action = self
                    .project_ui_state
                    .lock()
                    .unwrap()
                    .overview_ui
                    .update(command, context);
                match overview_ui_action {
                    Some(OverviewUiAction::None) => None,
                    None => None,
                }
            }
            ProjectUiCommand::PartsUiCommand(command) => {
                let context = &mut PartsUiContext::default();
                let parts_ui_action = self
                    .project_ui_state
                    .lock()
                    .unwrap()
                    .parts_ui
                    .update(command, context);
                match parts_ui_action {
                    Some(PartsUiAction::None) => None,
                    None => None,
                    Some(PartsUiAction::UpdateProcessesForPart {
                        part,
                        processes,
                    }) => {
                        let mut tasks = vec![];
                        for (process, enabled) in processes {
                            let operation = match enabled {
                                true => AddOrRemoveAction::Add,
                                false => AddOrRemoveAction::Remove,
                            };

                            match self
                                .planner_core_service
                                .update(key, Event::AssignProcessToParts {
                                    process,
                                    operation,
                                    manufacturer: exact_match(part.manufacturer.as_str()),
                                    mpn: exact_match(part.mpn.as_str()),
                                })
                                .into_actions()
                            {
                                Ok(actions) => {
                                    debug!("actions: {:?}", actions);
                                    let effect_tasks: Vec<Task<ProjectAction>> = actions
                                        .into_iter()
                                        .map(Task::done)
                                        .collect();
                                    tasks.extend(effect_tasks);
                                }
                                Err(service_error) => {
                                    tasks.push(Task::done(service_error));
                                    break;
                                }
                            }
                        }

                        let final_task = Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestView(
                            ProjectViewRequest::Parts,
                        )));
                        tasks.push(final_task);

                        let action = ProjectAction::Task(key, Task::batch(tasks));

                        Some(action)
                    }
                    Some(PartsUiAction::RequestRepaint) => Some(ProjectAction::RequestRepaint),
                }
            }
            ProjectUiCommand::PhaseUiCommand {
                phase,
                command,
            } => {
                let mut state = self.project_ui_state.lock().unwrap();
                let phase_ui = state.phases.get_mut(&phase).unwrap();

                let context = &mut PhaseUiContext::default();
                let phase_ui_action = phase_ui.update(command, context);

                match phase_ui_action {
                    None => None,
                    Some(PhaseUiAction::None) => None,
                    Some(PhaseUiAction::RequestRepaint) => Some(ProjectAction::RequestRepaint),
                    Some(PhaseUiAction::UpdatePlacement {
                        object_path,
                        new_placement,
                        old_placement,
                    }) => {
                        let (mut tasks, actions) = Self::update_placement(
                            &mut self.planner_core_service,
                            key,
                            object_path,
                            new_placement,
                            old_placement,
                        );
                        Self::handle_update_placement_actions(&mut tasks, actions);

                        Some(ProjectAction::Task(key, Task::batch(tasks)))
                    }
                    Some(PhaseUiAction::AddPartsToLoadout {
                        phase,
                        manufacturer_pattern,
                        mpn_pattern,
                    }) => self
                        .planner_core_service
                        .update(key, Event::AddPartsToLoadout {
                            phase,
                            manufacturer: manufacturer_pattern,
                            mpn: mpn_pattern,
                        })
                        .when_ok(|_| None),
                    Some(PhaseUiAction::SetPlacementOrderings(args)) => self
                        .planner_core_service
                        .update(key, Event::SetPlacementOrdering {
                            phase: phase.clone(),
                            placement_orderings: args.orderings,
                        })
                        .when_ok(|_| Some(ProjectUiCommand::RefreshPhase(phase))),
                    Some(PhaseUiAction::TaskAction {
                        phase,
                        operation,
                        task,
                        action,
                    }) => self
                        .planner_core_service
                        .update(key, Event::RecordPhaseOperation {
                            phase: phase.clone(),
                            operation,
                            task,
                            action,
                        })
                        .when_ok(|_| Some(ProjectUiCommand::RefreshPhase(phase))),
                }
            }
            ProjectUiCommand::LoadOutUiCommand {
                load_out_source,
                command,
            } => {
                let mut state = self.project_ui_state.lock().unwrap();
                let load_out_ui = state
                    .load_outs
                    .get_mut(&load_out_source)
                    .unwrap();

                let context = &mut LoadOutUiContext::default();
                let phase_ui_action = load_out_ui.update(command, context);

                match phase_ui_action {
                    Some(LoadOutUiAction::None) => None,
                    Some(LoadOutUiAction::RequestRepaint) => Some(ProjectAction::RequestRepaint),
                    None => None,
                    Some(LoadOutUiAction::UpdateFeederForPart {
                        phase,
                        part,
                        feeder,
                    }) => {
                        debug!(
                            "update feeder. phase: {:?}, part: {:?}, feeder: {}",
                            phase, part, feeder
                        );
                        self.planner_core_service
                            .update(key, Event::AssignFeederToLoadOutItem {
                                phase,
                                feeder_reference: feeder,
                                manufacturer: exact_match(&part.manufacturer),
                                mpn: exact_match(&part.mpn),
                            })
                            .when_ok(|_| None)
                    }
                }
            }
            ProjectUiCommand::PlacementsUiCommand(command) => {
                let context = &mut PlacementsUiContext::default();
                let placements_ui_action = self
                    .project_ui_state
                    .lock()
                    .unwrap()
                    .placements_ui
                    .update(command, context);
                match placements_ui_action {
                    Some(PlacementsUiAction::None) => None,
                    Some(PlacementsUiAction::RequestRepaint) => Some(ProjectAction::RequestRepaint),
                    Some(PlacementsUiAction::UpdatePlacement {
                        object_path,
                        new_placement,
                        old_placement,
                    }) => {
                        let (mut tasks, actions) = Self::update_placement(
                            &mut self.planner_core_service,
                            key,
                            object_path,
                            new_placement,
                            old_placement,
                        );
                        Self::handle_update_placement_actions(&mut tasks, actions);

                        Some(ProjectAction::Task(key, Task::batch(tasks)))
                    }
                    None => None,
                }
            }
            ProjectUiCommand::ToolbarCommand(toolbar_command) => {
                let action = self
                    .toolbar
                    .update(toolbar_command, &mut ());
                match action {
                    Some(ProjectToolbarAction::ShowProjectExplorer) => {
                        let task = self.show_explorer();
                        Some(ProjectAction::Task(key, task))
                    }
                    Some(ProjectToolbarAction::GenerateArtifacts) => self
                        .planner_core_service
                        .update(key, Event::GenerateArtifacts)
                        .when_ok(|_| None),
                    Some(ProjectToolbarAction::RefreshFromDesignVariants) => self
                        .planner_core_service
                        .update(key, Event::RefreshFromDesignVariants)
                        .when_ok(|_| Some(ProjectUiCommand::ProjectRefreshed)),
                    Some(ProjectToolbarAction::RemoveUnusedPlacements) => self
                        .planner_core_service
                        .update(key, Event::RemoveUsedPlacements {
                            phase: None,
                        })
                        .when_ok(|_| Some(ProjectUiCommand::ProjectRefreshed)),
                    Some(ProjectToolbarAction::ShowAddPcbDialog) => {
                        let mut modal = AddPcbModal::new(self.path.clone());
                        modal
                            .component
                            .configure_mapper(self.component.sender.clone(), move |command| {
                                trace!("add pcb modal mapper. command: {:?}", command);
                                (key, ProjectUiCommand::AddPcbModalCommand(command))
                            });

                        self.add_pcb_modal = Some(modal);
                        None
                    }
                    Some(ProjectToolbarAction::ShowAddPhaseDialog) => {
                        let mut modal = AddPhaseModal::new(self.path.clone(), self.processes.clone());
                        modal
                            .component
                            .configure_mapper(self.component.sender.clone(), move |command| {
                                trace!("add phase modal mapper. command: {:?}", command);
                                (key, ProjectUiCommand::AddPhaseModalCommand(command))
                            });

                        self.add_phase_modal = Some(modal);
                        None
                    }
                    None => None,
                }
            }
            ProjectUiCommand::TabCommand(tab_command) => {
                let mut project_tabs = self.project_tabs.lock().unwrap();

                let mut tab_context = ProjectTabContext {
                    state: self.project_ui_state.clone(),
                };

                let action = project_tabs.update(tab_command, &mut tab_context);
                match action {
                    None => {}
                    Some(ProjectTabAction::None) => {
                        debug!("ProjectTabAction::None");
                    }
                }
                None
            }
            ProjectUiCommand::AddPcbModalCommand(command) => {
                if let Some(modal) = &mut self.add_pcb_modal {
                    let action = modal.update(command, &mut ());
                    match action {
                        None => None,
                        Some(AddPcbModalAction::Submit(args)) => {
                            self.add_pcb_modal.take();
                            self.planner_core_service
                                .update(key, Event::AddPcb {
                                    name: args.name,
                                    units: args.units,
                                    unit_map: args.unit_map,
                                })
                                .when_ok(|_| Some(ProjectUiCommand::RequestView(ProjectViewRequest::ProjectTree)))
                        }
                        Some(AddPcbModalAction::CloseDialog) => {
                            self.add_pcb_modal.take();
                            None
                        }
                    }
                } else {
                    None
                }
            }
            ProjectUiCommand::AddPhaseModalCommand(command) => {
                if let Some(modal) = &mut self.add_phase_modal {
                    let action = modal.update(command, &mut ());
                    match action {
                        None => None,
                        Some(AddPhaseModalAction::Submit(args)) => {
                            self.add_phase_modal.take();

                            match self
                                .planner_core_service
                                .update(key, Event::CreatePhase {
                                    process: args.process,
                                    reference: args.reference,
                                    load_out: args.load_out,
                                    pcb_side: args.pcb_side,
                                })
                                .into_actions()
                            {
                                Ok(actions) => {
                                    let mut tasks = actions
                                        .into_iter()
                                        .map(Task::done)
                                        .collect::<Vec<Task<ProjectAction>>>();

                                    let additional_tasks = vec![
                                        Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestView(
                                            ProjectViewRequest::ProjectTree,
                                        ))),
                                        Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestView(
                                            ProjectViewRequest::Phases,
                                        ))),
                                    ];
                                    tasks.extend(additional_tasks);

                                    Some(ProjectAction::Task(key, Task::batch(tasks)))
                                }
                                Err(error_action) => Some(error_action),
                            }
                        }
                        Some(AddPhaseModalAction::CloseDialog) => {
                            self.add_phase_modal.take();
                            None
                        }
                    }
                } else {
                    None
                }
            }
            ProjectUiCommand::ShowPhaseLoadout {
                phase,
            } => {
                let phase_overview = self
                    .phases
                    .iter()
                    .find(|phase_overview| {
                        phase_overview
                            .phase_reference
                            .eq(&phase)
                    });

                if let Some(phase_overview) = phase_overview {
                    let task = self.show_loadout(
                        key,
                        phase_overview.phase_reference.clone(),
                        &phase_overview.load_out_source,
                    );

                    task.map(|task|ProjectAction::Task(key, task))
                } else {
                    None
                }
            }
            ProjectUiCommand::ShowPcbUnitAssignments(pcb_index) => {
                let tasks = self.show_unit_assignments(key, pcb_index);
                tasks.map(|tasks|ProjectAction::Task(key, Task::batch(tasks)))
            }
            ProjectUiCommand::RefreshPhase(phase) => {
                let tasks = vec![
                    Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestView(
                        ProjectViewRequest::PhaseOverview {
                            phase: phase.clone(),
                        },
                    ))),
                    Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestView(
                        ProjectViewRequest::PhasePlacements {
                            phase: phase.clone(),
                        },
                    ))),
                ];
                Some(ProjectAction::Task(key, Task::batch(tasks)))
            }
            ProjectUiCommand::PcbUiCommand {
                pcb_index,
                command,
            } => {
                let mut state = self.project_ui_state.lock().unwrap();
                let pcb_ui = state
                    .pcbs
                    .get_mut(&(pcb_index as usize))
                    .unwrap();

                let context = &mut PcbUiContext::default();
                let pcb_ui_action = pcb_ui.update(command, context);
                match pcb_ui_action {
                    None => None,
                    Some(PcbUiAction::None) => None,
                    Some(PcbUiAction::AddGerberFiles {
                        pcb_index,
                        design,
                        files,
                    }) => {
                        match self
                            .planner_core_service
                            .update(key, Event::AddGerberFiles {
                                design,
                                files,
                            })
                            .into_actions()
                        {
                            Ok(actions) => {
                                let mut tasks = actions
                                    .into_iter()
                                    .map(Task::done)
                                    .collect::<Vec<Task<ProjectAction>>>();

                                let additional_tasks = vec![Task::done(ProjectAction::UiCommand(
                                    ProjectUiCommand::RequestView(ProjectViewRequest::PcbOverview {
                                        pcb: pcb_index,
                                    }),
                                ))];
                                tasks.extend(additional_tasks);

                                Some(ProjectAction::Task(key, Task::batch(tasks)))
                            }
                            Err(error_action) => Some(error_action),
                        }
                    }
                    Some(PcbUiAction::RemoveGerberFiles {
                        pcb_index,
                        design,
                        files,
                    }) => {
                        match self
                            .planner_core_service
                            .update(key, Event::RemoveGerberFiles {
                                design,
                                files,
                            })
                            .into_actions()
                        {
                            Ok(actions) => {
                                let mut tasks = actions
                                    .into_iter()
                                    .map(Task::done)
                                    .collect::<Vec<Task<ProjectAction>>>();

                                let additional_tasks = vec![Task::done(ProjectAction::UiCommand(
                                    ProjectUiCommand::RequestView(ProjectViewRequest::PcbOverview {
                                        pcb: pcb_index,
                                    }),
                                ))];
                                tasks.extend(additional_tasks);

                                Some(ProjectAction::Task(key, Task::batch(tasks)))
                            }
                            Err(error_action) => Some(error_action),
                        }
                    }
                    Some(PcbUiAction::ShowUnitAssignments(pcb_index)) => {
                        Some(ProjectAction::Task(key, Task::done(ProjectAction::UiCommand(ProjectUiCommand::ShowPcbUnitAssignments(pcb_index)))))
                    }
                }
            }
            ProjectUiCommand::UnitAssignmentsUiCommand {
                pcb_index,
                command,
            } => {
                let mut state = self.project_ui_state.lock().unwrap();
                let unit_assignment_ui = state
                    .unit_assignments
                    .get_mut(&(pcb_index as usize))
                    .unwrap();

                let context = &mut UnitAssignmentsUiContext::default();
                let unit_assignment_ui_action = unit_assignment_ui.update(command, context);
                match unit_assignment_ui_action {
                    None => None,
                    Some(UnitAssignmentsUiAction::None) => None,
                    Some(UnitAssignmentsUiAction::UpdateUnitAssignments(UpdateUnitAssignmentsArgs {
                        pcb_index,
                        variant_map,
                    })) => {
                        let mut events = vec![];

                        for (pcb_unit_index, variant_name) in variant_map.iter().enumerate() {
                            let mut object_path = ObjectPath::default();
                            object_path.set_pcb_instance(pcb_index + 1);
                            object_path.set_pcb_unit(pcb_unit_index as u16 + 1);

                            events.push(Event::AssignVariantToUnit {
                                variant: variant_name.clone(),
                                unit: object_path,
                            });
                        }

                        let mut tasks = vec![];
                        for event in events {
                            match self
                                .planner_core_service
                                .update(key, event)
                                .into_actions()
                            {
                                Ok(actions) => {
                                    let effect_tasks: Vec<Task<ProjectAction>> = actions
                                        .into_iter()
                                        .map(Task::done)
                                        .collect();
                                    tasks.extend(effect_tasks);
                                }
                                Err(service_error) => {
                                    tasks.push(Task::done(service_error));
                                    break;
                                }
                            }
                        }

                        let final_task = Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestView(
                            ProjectViewRequest::ProjectTree,
                        )));
                        tasks.push(final_task);

                        let action = ProjectAction::Task(key, Task::batch(tasks));

                        Some(action)
                    }
                }
            }
        }
    }
}

impl Tab for ProjectTabKind {
    type Context = ProjectTabContext;

    fn label(&self) -> WidgetText {
        match self {
            ProjectTabKind::Explorer(tab) => tab.label(),
            ProjectTabKind::Overview(tab) => tab.label(),
            ProjectTabKind::Parts(tab) => tab.label(),
            ProjectTabKind::Placements(tab) => tab.label(),
            ProjectTabKind::Phase(tab) => tab.label(),
            ProjectTabKind::LoadOut(tab) => tab.label(),
            ProjectTabKind::Pcb(tab) => tab.label(),
            ProjectTabKind::UnitAssignments(tab) => tab.label(),
        }
    }

    fn ui<'a>(&mut self, ui: &mut Ui, tab_key: &TabKey, context: &mut Self::Context) {
        match self {
            ProjectTabKind::Explorer(tab) => tab.ui(ui, tab_key, context),
            ProjectTabKind::Overview(tab) => tab.ui(ui, tab_key, context),
            ProjectTabKind::Parts(tab) => tab.ui(ui, tab_key, context),
            ProjectTabKind::Placements(tab) => tab.ui(ui, tab_key, context),
            ProjectTabKind::Phase(tab) => tab.ui(ui, tab_key, context),
            ProjectTabKind::LoadOut(tab) => tab.ui(ui, tab_key, context),
            ProjectTabKind::Pcb(tab) => tab.ui(ui, tab_key, context),
            ProjectTabKind::UnitAssignments(tab) => tab.ui(ui, tab_key, context),
        }
    }

    fn on_close<'a>(&mut self, tab_key: &TabKey, context: &mut Self::Context) -> bool {
        match self {
            ProjectTabKind::Explorer(tab) => tab.on_close(tab_key, context),
            ProjectTabKind::Overview(tab) => tab.on_close(tab_key, context),
            ProjectTabKind::Parts(tab) => tab.on_close(tab_key, context),
            ProjectTabKind::Placements(tab) => tab.on_close(tab_key, context),
            ProjectTabKind::Phase(tab) => tab.on_close(tab_key, context),
            ProjectTabKind::LoadOut(tab) => tab.on_close(tab_key, context),
            ProjectTabKind::Pcb(tab) => tab.on_close(tab_key, context),
            ProjectTabKind::UnitAssignments(tab) => tab.on_close(tab_key, context),
        }
    }
}

#[derive(Debug)]
pub struct ProjectUiState {
    /// initially unknown until the project is loaded
    /// always known for newly created projects.
    name: Option<String>,

    explorer_ui: ExplorerUi,
    load_outs: HashMap<LoadOutSource, LoadOutUi>,
    parts_ui: PartsUi,
    pcbs: HashMap<usize, PcbUi>,
    phases: HashMap<Reference, PhaseUi>,
    placements_ui: PlacementsUi,
    overview_ui: OverviewUi,
    unit_assignments: HashMap<usize, UnitAssignmentsUi>,
}

impl ProjectUiState {
    pub fn new(
        key: ProjectKey,
        project_directory: PathBuf,
        name: Option<String>,
        sender: Enqueue<(ProjectKey, ProjectUiCommand)>,
    ) -> Self {
        let mut instance = Self {
            name,
            explorer_ui: ExplorerUi::new(project_directory),
            load_outs: HashMap::default(),
            parts_ui: PartsUi::new(),
            pcbs: HashMap::default(),
            phases: HashMap::default(),
            placements_ui: PlacementsUi::new(),
            overview_ui: OverviewUi::new(),
            unit_assignments: HashMap::default(),
        };

        instance
            .explorer_ui
            .component
            .configure_mapper(sender.clone(), move |command| {
                trace!("explorer ui mapper. command: {:?}", command);
                (key, ProjectUiCommand::ExplorerUiCommand(command))
            });

        instance
            .overview_ui
            .component
            .configure_mapper(sender.clone(), move |command| {
                trace!("overview ui mapper. command: {:?}", command);
                (key, ProjectUiCommand::OverviewUiCommand(command))
            });

        instance
            .parts_ui
            .component
            .configure_mapper(sender.clone(), move |command| {
                trace!("parts ui mapper. command: {:?}", command);
                (key, ProjectUiCommand::PartsUiCommand(command))
            });

        instance
            .placements_ui
            .component
            .configure_mapper(sender.clone(), move |command| {
                trace!("placements ui mapper. command: {:?}", command);
                (key, ProjectUiCommand::PlacementsUiCommand(command))
            });

        instance
    }
}

// these should not contain state
#[derive(serde::Deserialize, serde::Serialize, Debug)]
enum ProjectTabKind {
    Explorer(ExplorerTab),
    Overview(OverviewTab),
    Phase(PhaseTab),
    Placements(PlacementsTab),
    Parts(PartsTab),
    LoadOut(LoadOutTab),
    Pcb(PcbTab),
    UnitAssignments(UnitAssignmentsTab),
}

#[derive(Debug, Clone)]
pub enum ProjectUiCommand {
    None,
    Load,
    Loaded,
    Save,
    Saved,
    UpdateView(ProjectView),
    Error(ProjectError),
    SetModifiedState(bool),
    RequestView(ProjectViewRequest),
    ClearErrors,
    ToolbarCommand(ProjectToolbarUiCommand),
    TabCommand(ProjectTabUiCommand),
    AddPcbModalCommand(AddPcbModalUiCommand),
    AddPhaseModalCommand(AddPhaseModalUiCommand),
    Create,
    Created,
    ProjectRefreshed,
    ExplorerUiCommand(ExplorerUiCommand),
    PartsUiCommand(PartsUiCommand),
    OverviewUiCommand(OverviewUiCommand),
    PlacementsUiCommand(PlacementsUiCommand),
    PhaseUiCommand {
        phase: Reference,
        command: PhaseUiCommand,
    },
    ShowPhaseLoadout {
        phase: Reference,
    },
    LoadOutUiCommand {
        load_out_source: LoadOutSource,
        command: LoadOutUiCommand,
    },
    PcbUiCommand {
        pcb_index: u16,
        command: PcbUiCommand,
    },
    RefreshPhase(Reference),
    UnitAssignmentsUiCommand {
        pcb_index: u16,
        command: UnitAssignmentsUiCommand,
    },
    ShowPcbUnitAssignments(u16),
}

#[derive(Debug, Clone)]
pub enum ProjectError {
    CoreError((chrono::DateTime<chrono::Utc>, String)),
}

fn project_path_from_view_path(view_path: &String) -> ProjectPath {
    let project_path = ProjectPath(format!("/project{}", view_path).to_string());
    project_path
}

fn view_path_from_project_path(project_path: &ProjectPath) -> Option<String> {
    let view_path = project_path
        .to_string()
        .split("/project")
        .collect::<Vec<&str>>()
        .get(1)?
        .to_string();
    Some(view_path)
}

fn exact_match(value: &str) -> Regex {
    Regex::new(format!("^{}$", regex::escape(value).as_str()).as_str()).unwrap()
}
