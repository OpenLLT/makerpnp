use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use std::path::PathBuf;
use std::str::FromStr;

use egui::{Ui, WidgetText};
use egui_i18n::tr;
use egui_mobius::types::{Enqueue, Value};
use planner_app::{
    AddOrRemoveAction, DesignName, Event, LoadOutSource, ObjectPath, PcbKind, PhaseOverview, PlacementOperation,
    PlacementState, PlacementStatus, ProcessReference, ProjectOverview, ProjectView, ProjectViewRequest, Reference,
    SetOrClearAction, VariantName,
};
use regex::Regex;
use slotmap::new_key_type;
use tracing::{debug, info, trace};

use crate::planner_app_core::PlannerCoreService;
use crate::project::dialogs::add_pcb::{AddPcbModal, AddPcbModalAction, AddPcbModalUiCommand};
use crate::project::dialogs::add_phase::{AddPhaseModal, AddPhaseModalAction, AddPhaseModalUiCommand};
use crate::project::dialogs::create_unit_assignment::{
    CreateUnitAssignmentModal, CreateUnitAssignmentModalAction, CreateUnitAssignmentModalUiCommand,
    UnitAssignmentPcbKind,
};
use crate::project::explorer_tab::{ExplorerTab, ExplorerUi, ExplorerUiAction, ExplorerUiCommand, ExplorerUiContext};
use crate::project::load_out_tab::{LoadOutTab, LoadOutUi, LoadOutUiAction, LoadOutUiCommand, LoadOutUiContext};
use crate::project::overview_tab::{OverviewTab, OverviewUi, OverviewUiAction, OverviewUiCommand, OverviewUiContext};
use crate::project::parts_tab::{PartsTab, PartsUi, PartsUiAction, PartsUiCommand, PartsUiContext};
use crate::project::phase_tab::{PhaseTab, PhaseUi, PhaseUiAction, PhaseUiCommand, PhaseUiContext};
use crate::project::placements_tab::{
    PlacementsTab, PlacementsUi, PlacementsUiAction, PlacementsUiCommand, PlacementsUiContext,
};
use crate::project::tabs::{ProjectTabAction, ProjectTabContext, ProjectTabUiCommand, ProjectTabs};
use crate::project::toolbar::{ProjectToolbar, ProjectToolbarAction, ProjectToolbarUiCommand};
use crate::tabs::{Tab, TabKey};
use crate::task::Task;
use crate::ui_component::{ComponentState, UiComponent};

mod explorer_tab;
mod load_out_tab;
mod overview_tab;
mod parts_tab;
mod phase_tab;
mod placements_tab;
mod tabs;
mod toolbar;

mod tables;

mod dialogs;

mod process;

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
    create_unit_assignment_modal: Option<CreateUnitAssignmentModal>,
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
            create_unit_assignment_modal: None,
        }
    }

    pub fn show_explorer(&mut self) {
        let mut project_tabs = self.project_tabs.lock().unwrap();
        let result = project_tabs.show_tab(|candidate_tab| matches!(candidate_tab, ProjectTabKind::Explorer(_)));
        if result.is_err() {
            project_tabs.add_tab(ProjectTabKind::Explorer(ExplorerTab::default()));
        }
    }

    pub fn show_overview(&mut self) {
        let mut project_tabs = self.project_tabs.lock().unwrap();
        let result = project_tabs.show_tab(|candidate_tab| matches!(candidate_tab, ProjectTabKind::Overview(_)));
        if result.is_err() {
            project_tabs.add_tab_to_second_leaf_or_split(ProjectTabKind::Overview(OverviewTab::default()));
        }
    }

    pub fn show_parts(&mut self) {
        let mut project_tabs = self.project_tabs.lock().unwrap();
        let result = project_tabs.show_tab(|candidate_tab| matches!(candidate_tab, ProjectTabKind::Parts(_)));
        if result.is_err() {
            project_tabs.add_tab_to_second_leaf_or_split(ProjectTabKind::Parts(PartsTab::default()));
        }
    }

    pub fn show_placements(&mut self) {
        let mut project_tabs = self.project_tabs.lock().unwrap();
        let result = project_tabs.show_tab(|candidate_tab| matches!(candidate_tab, ProjectTabKind::Placements(_)));
        if result.is_err() {
            project_tabs.add_tab_to_second_leaf_or_split(ProjectTabKind::Placements(PlacementsTab::default()));
        }
    }

    pub fn show_phase(&mut self, key: ProjectKey, phase: Reference) {
        let mut project_tabs = self.project_tabs.lock().unwrap();
        let tab = PhaseTab::new(phase.clone());
        project_tabs
            .show_tab(
                |candidate_tab| matches!(candidate_tab, ProjectTabKind::Phase(candidate_tab) if candidate_tab.eq(&tab)),
            )
            .inspect(|tab_key| {
                debug!("showing existing phase tab. phase: {:?}, tab_key: {:?}", phase, tab_key);
            })
            .inspect_err(|_| {
                self.ensure_phase(key, &phase);

                let tab_key = project_tabs.add_tab_to_second_leaf_or_split(ProjectTabKind::Phase(tab));
                debug!("adding phase tab. phase: {:?}, tab_key: {:?}", phase, tab_key);
            })
            .ok();
    }

    pub fn show_loadout(&self, key: ProjectKey, phase: Reference, load_out_source: &LoadOutSource) {
        let project_directory = self.path.parent().unwrap();

        let mut project_tabs = self.project_tabs.lock().unwrap();
        let tab = LoadOutTab::new(project_directory.into(), load_out_source.clone());
        project_tabs
            .show_tab(|candidate_tab| {
                matches!(candidate_tab, ProjectTabKind::LoadOut(candidate_tab) if candidate_tab.eq(&tab))
            })
            .inspect(|tab_key| {
                debug!("showing existing load-out tab. load_out_source: {:?}, tab_key: {:?}", load_out_source, tab_key);
            })
            .inspect_err(|_| {
                self.ensure_load_out(key, phase, &load_out_source);

                let tab_key = project_tabs.add_tab_to_second_leaf_or_split(ProjectTabKind::LoadOut(tab));
                debug!("adding load-out tab. phase: {:?}, tab_key: {:?}", load_out_source, tab_key);
            })
            .ok();
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

    fn navigate(&mut self, key: ProjectKey, path: ProjectPath) -> Option<ProjectAction> {
        // if the path starts with `/project/` then show/hide UI elements based on the path,
        // e.g. update a dynamic that controls a per-project-tab-bar dynamic selector
        info!("ProjectMessage::Navigate. path: {}", path);

        #[must_use]
        fn handle_root(project: &mut Project, key: &ProjectKey, path: &ProjectPath) -> Option<ProjectAction> {
            if path.eq(&"/project/".into()) {
                project.show_overview();
                let task = Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestView(
                    ProjectViewRequest::Overview,
                )));

                Some(ProjectAction::Task(*key, task))
            } else {
                None
            }
        }

        #[must_use]
        fn handle_placements(project: &mut Project, key: &ProjectKey, path: &ProjectPath) -> Option<ProjectAction> {
            if path.eq(&"/project/placements".into()) {
                project.show_placements();
                let task = Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestView(
                    ProjectViewRequest::Placements,
                )));

                Some(ProjectAction::Task(*key, task))
            } else {
                None
            }
        }

        #[must_use]
        fn handle_parts(project: &mut Project, key: &ProjectKey, path: &ProjectPath) -> Option<ProjectAction> {
            if path.eq(&"/project/parts".into()) {
                project.show_parts();
                let task = Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestView(
                    ProjectViewRequest::Parts,
                )));

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

                project.show_phase(key.clone(), reference.clone());

                let tasks: Vec<_> = vec![
                    Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestView(
                        ProjectViewRequest::PhaseOverview {
                            phase: reference.clone(),
                        },
                    ))),
                    Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestView(
                        ProjectViewRequest::PhasePlacements {
                            phase: reference.clone(),
                        },
                    ))),
                ];

                Some(ProjectAction::Task(*key, Task::batch(tasks)))
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

        let handlers = [
            handle_root,
            handle_parts,
            handle_phases,
            handle_phase_loadout,
            handle_placements,
            handle_phase,
        ];

        handlers
            .iter()
            .find_map(|handler| handler(self, &key, &path))
    }

    pub fn update_processes(&mut self, project_overview: &ProjectOverview) {
        self.processes = project_overview.processes.clone();
    }

    fn update_placement(
        planner_core_service: &mut PlannerCoreService,
        key: ProjectKey,
        object_path: ObjectPath,
        new_placement: PlacementState,
        old_placement: PlacementState,
    ) -> Option<ProjectAction> {
        let mut tasks = vec![];

        fn handle_phase(
            planner_core_service: &mut PlannerCoreService,
            key: &ProjectKey,
            object_path: &ObjectPath,
            new_placement: &PlacementState,
            old_placement: &PlacementState,
        ) -> Option<Result<Vec<ProjectAction>, ProjectAction>> {
            if !new_placement
                .phase
                .eq(&old_placement.phase)
            {
                let (phase, operation) = match (&new_placement.phase, &old_placement.phase) {
                    (Some(new_phase), None) => (new_phase, SetOrClearAction::Set),
                    (Some(new_phase), Some(_old_phase)) => (new_phase, SetOrClearAction::Set),
                    (None, Some(old_phase)) => (old_phase, SetOrClearAction::Clear),
                    _ => unreachable!(),
                };

                Some(
                    planner_core_service
                        .update(key.clone(), Event::AssignPlacementsToPhase {
                            phase: phase.clone(),
                            operation,
                            placements: exact_match(&object_path.to_string()),
                        })
                        .into_actions(),
                )
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
        ) -> Option<Result<Vec<ProjectAction>, ProjectAction>> {
            if new_placement.operation_status != old_placement.operation_status {
                let operation = match new_placement.operation_status {
                    PlacementStatus::Placed => PlacementOperation::Place,
                    PlacementStatus::Skipped => PlacementOperation::Skip,
                    PlacementStatus::Pending => PlacementOperation::Place,
                };

                Some(
                    planner_core_service
                        .update(key.clone(), Event::RecordPlacementsOperation {
                            object_path_patterns: vec![exact_match(&object_path.to_string())],
                            operation,
                        })
                        .into_actions(),
                )
            } else {
                None
            }
        }

        #[derive(Debug)]
        enum Actions {
            AddOrRemovePhase,
            SetOrResetPlaced,
        }

        // FUTURE find a solution to keep the operation with the handler, instead of two separate arrays.
        //        a tuple was tried, but results in a compile error: "expected fn item, found a different fn item"
        let actions = [Actions::AddOrRemovePhase, Actions::SetOrResetPlaced];

        let action_handlers = [handle_phase, handle_placed];

        for (action, handler) in actions
            .into_iter()
            .zip(action_handlers.into_iter())
        {
            debug!("update placement, action: {:?}", action);

            if let Some(core_result) = handler(planner_core_service, &key, &object_path, &new_placement, &old_placement)
            {
                match core_result {
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
        }

        let final_task = Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestView(
            ProjectViewRequest::Parts,
        )));
        tasks.push(final_task);

        let action = ProjectAction::Task(key, Task::batch(tasks));

        Some(action)
    }
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

        if let Some(dialog) = &self.add_pcb_modal {
            dialog.ui(ui, &mut ());
        }
        if let Some(dialog) = &self.add_phase_modal {
            dialog.ui(ui, &mut ());
        }
        if let Some(dialog) = &self.create_unit_assignment_modal {
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

                        self.ensure_phase(key.clone(), &phase);

                        let mut state = self.project_ui_state.lock().unwrap();
                        let phase_ui = state.phases.get_mut(&phase).unwrap();

                        phase_ui.update_overview(phase_overview);
                    }
                    ProjectView::PhasePlacements(phase_placements) => {
                        trace!("phase placements: {:?}", phase_placements);
                        let phase = phase_placements.phase_reference.clone();

                        self.ensure_phase(key.clone(), &phase);

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

                        self.ensure_load_out(key.clone(), load_out.phase_reference.clone(), &load_out_source);

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
                    Some(PhaseUiAction::None) => None,
                    Some(PhaseUiAction::RequestRepaint) => Some(ProjectAction::RequestRepaint),
                    None => None,
                    Some(PhaseUiAction::UpdatePlacement {
                        object_path,
                        new_placement,
                        old_placement,
                    }) => Self::update_placement(
                        &mut self.planner_core_service,
                        key,
                        object_path,
                        new_placement,
                        old_placement,
                    ),
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
                    }) => Self::update_placement(
                        &mut self.planner_core_service,
                        key,
                        object_path,
                        new_placement,
                        old_placement,
                    ),
                    None => None,
                }
            }
            ProjectUiCommand::ToolbarCommand(toolbar_command) => {
                let action = self
                    .toolbar
                    .update(toolbar_command, &mut ());
                match action {
                    Some(ProjectToolbarAction::ShowProjectExplorer) => {
                        self.show_explorer();
                        None
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
                    Some(ProjectToolbarAction::ShowCreateUnitAssignmentDialog) => {
                        let mut modal = CreateUnitAssignmentModal::new(self.path.clone());
                        modal
                            .component
                            .configure_mapper(self.component.sender.clone(), move |command| {
                                trace!("create unit assignment modal mapper. command: {:?}", command);
                                (key, ProjectUiCommand::CreateUnitAssignmentModalCommand(command))
                            });

                        self.create_unit_assignment_modal = Some(modal);
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
                                    kind: args.kind,
                                    name: args.name,
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
            ProjectUiCommand::CreateUnitAssignmentModalCommand(command) => {
                if let Some(modal) = &mut self.create_unit_assignment_modal {
                    let action = modal.update(command, &mut ());
                    match action {
                        None => None,
                        Some(CreateUnitAssignmentModalAction::Submit(args)) => {
                            self.create_unit_assignment_modal.take();

                            let mut events = vec![];

                            match args.kind {
                                UnitAssignmentPcbKind::Single {
                                    instance,
                                } => {
                                    let mut object_path = ObjectPath::default();
                                    object_path.set_pcb_kind_and_instance(PcbKind::Single, instance);
                                    object_path.set_pcb_unit(1);

                                    events.push(Event::AssignVariantToUnit {
                                        design: DesignName::from_str(&args.design_name).unwrap(),
                                        variant: VariantName::from_str(&args.variant_name).unwrap(),
                                        unit: object_path,
                                    });
                                }
                                UnitAssignmentPcbKind::Panel {
                                    instance,
                                    unit_range,
                                } => {
                                    for unit in unit_range {
                                        let mut object_path = ObjectPath::default();
                                        object_path.set_pcb_kind_and_instance(PcbKind::Panel, instance);
                                        object_path.set_pcb_unit(unit);

                                        events.push(Event::AssignVariantToUnit {
                                            design: DesignName::from_str(&args.design_name).unwrap(),
                                            variant: VariantName::from_str(&args.variant_name).unwrap(),
                                            unit: object_path,
                                        });
                                    }
                                }
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
                        Some(CreateUnitAssignmentModalAction::CloseDialog) => {
                            self.create_unit_assignment_modal.take();
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
                    self.show_loadout(
                        key,
                        phase_overview.phase_reference.clone(),
                        &phase_overview.load_out_source,
                    );

                    let task = Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestView(
                        ProjectViewRequest::PhaseLoadOut {
                            phase: phase_overview.phase_reference.clone(),
                        },
                    )));

                    Some(ProjectAction::Task(key, task))
                } else {
                    None
                }
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
        }
    }
}

#[derive(Debug)]
pub struct ProjectUiState {
    /// initially unknown until the project is loaded
    /// always known for newly created projects.
    name: Option<String>,

    overview_ui: OverviewUi,
    placements_ui: PlacementsUi,
    parts_ui: PartsUi,
    explorer_ui: ExplorerUi,
    phases: HashMap<Reference, PhaseUi>,
    load_outs: HashMap<LoadOutSource, LoadOutUi>,
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
            phases: HashMap::default(),
            load_outs: HashMap::default(),
            overview_ui: OverviewUi::new(),
            placements_ui: PlacementsUi::new(),
            parts_ui: PartsUi::new(),
            explorer_ui: ExplorerUi::new(project_directory),
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
    CreateUnitAssignmentModalCommand(CreateUnitAssignmentModalUiCommand),
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
    RefreshPhase(Reference),
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
