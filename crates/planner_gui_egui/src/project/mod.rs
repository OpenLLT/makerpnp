use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use std::path::PathBuf;
use std::str::FromStr;

use egui::{Ui, WidgetText};
use egui_i18n::tr;
use egui_mobius::types::{Enqueue, Value};
use planner_app::{
    DesignName, Event, ProcessName, ProjectOverview, ProjectView, ProjectViewRequest, Reference, VariantName,
};
use regex::Regex;
use slotmap::new_key_type;
use tracing::{debug, info};

use crate::planner_app_core::PlannerCoreService;
use crate::project::dialogs::add_pcb::{AddPcbModal, AddPcbModalAction, AddPcbModalUiCommand};
use crate::project::dialogs::add_phase::{AddPhaseModal, AddPhaseModalAction, AddPhaseModalUiCommand};
use crate::project::dialogs::create_unit_assignment::{
    CreateUnitAssignmentModal, CreateUnitAssignmentModalAction, CreateUnitAssignmentModalUiCommand,
};
use crate::project::explorer_tab::{ExplorerTab, ExplorerUi, ExplorerUiAction, ExplorerUiCommand, ExplorerUiContext};
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
mod overview_tab;
mod parts_tab;
mod phase_tab;
mod placements_tab;
mod tabs;
mod toolbar;

mod tables;

mod dialogs;

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
}

pub struct Project {
    planner_core_service: PlannerCoreService,
    path: PathBuf,
    project_ui_state: Value<ProjectUiState>,

    modified: bool,

    /// list of errors to show
    errors: Vec<String>,

    /// initially empty until the OverviewView has been received and processed.
    processes: Vec<ProcessName>,

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
                debug!("project toolbar mapper. command: {:?}", command);
                (key, ProjectUiCommand::ToolbarCommand(command))
            });

        let project_ui_state = Value::new(ProjectUiState::new(key, name, component_sender.clone()));

        let project_tabs = Value::new(ProjectTabs::default());
        {
            let mut project_tabs = project_tabs.lock().unwrap();
            project_tabs
                .component
                .configure_mapper(component_sender, move |command| {
                    debug!("project inner-tab mapper. command: {:?}", command);
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
            .show_tab(|candidate_tab| matches!(candidate_tab, ProjectTabKind::Phase(phase_tab) if phase_tab.eq(&tab)))
            .inspect(|tab_key| {
                debug!("showing existing phase tab. phase: {:?}, tab_key: {:?}", phase, tab_key);
            })
            .inspect_err(|_| {
                self.ensure_phase(key, phase.clone());

                let tab_key = project_tabs.add_tab_to_second_leaf_or_split(ProjectTabKind::Phase(tab));
                debug!("adding phase tab. phase: {:?}, tab_key: {:?}", phase, tab_key);
            })
            .ok();
    }

    fn ensure_phase(&self, key: ProjectKey, phase: Reference) {
        let mut state = self.project_ui_state.lock().unwrap();
        let _phase_state = state
            .phases
            .entry(phase.clone())
            .or_insert_with(|| {
                let mut phase_ui = PhaseUi::new();
                phase_ui
                    .component
                    .configure_mapper(self.component.sender.clone(), {
                        move |command| {
                            debug!("placements ui mapper. command: {:?}", command);
                            (key, ProjectUiCommand::PhaseUiCommand {
                                phase: phase.clone(),
                                command,
                            })
                        }
                    });

                phase_ui
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
        fn handle_phase(project: &mut Project, key: &ProjectKey, path: &ProjectPath) -> Option<ProjectAction> {
            let phase_pattern = Regex::new(r"/project/phases/(?<phase>.*){1}").unwrap();
            if let Some(captures) = phase_pattern.captures(&path) {
                let phase_reference: String = captures
                    .name("phase")
                    .unwrap()
                    .as_str()
                    .to_string();
                debug!("phase_reference: {}", phase_reference);

                project.show_phase(key.clone(), phase_reference.clone().into());

                let tasks: Vec<_> = vec![
                    Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestView(
                        ProjectViewRequest::PhaseOverview {
                            phase: phase_reference.clone(),
                        },
                    ))),
                    Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestView(
                        ProjectViewRequest::PhasePlacements {
                            phase: phase_reference.clone(),
                        },
                    ))),
                ];

                Some(ProjectAction::Task(*key, Task::batch(tasks)))
            } else {
                None
            }
        }

        let handlers = [handle_root, handle_parts, handle_placements, handle_phase];

        handlers
            .iter()
            .find_map(|handler| handler(self, &key, &path))
    }

    pub fn update_processes(&mut self, project_overview: &ProjectOverview) {
        self.processes = project_overview.processes.clone();
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
                    .when_ok(|| ProjectAction::UiCommand(ProjectUiCommand::Loaded))
            }
            ProjectUiCommand::Loaded => self
                .planner_core_service
                .update(key, Event::RequestOverviewView {})
                .when_ok(|| ProjectAction::UiCommand(ProjectUiCommand::RequestView(ProjectViewRequest::ProjectTree))),
            ProjectUiCommand::Create => {
                let state = self.project_ui_state.lock().unwrap();
                self.planner_core_service
                    .update(key, Event::CreateProject {
                        name: state.name.clone().unwrap(),
                        path: self.path.clone(),
                    })
                    .when_ok(|| ProjectAction::UiCommand(ProjectUiCommand::Created))
            }
            ProjectUiCommand::Created => self
                .planner_core_service
                .update(key, Event::RequestOverviewView {})
                .when_ok(|| ProjectAction::UiCommand(ProjectUiCommand::RequestView(ProjectViewRequest::ProjectTree))),
            ProjectUiCommand::Save => {
                debug!("saving project. path: {}", self.path.display());
                self.planner_core_service
                    .update(key, Event::Save)
                    .when_ok(|| ProjectAction::UiCommand(ProjectUiCommand::Saved))
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
                    ProjectViewRequest::ProjectTree => Event::RequestProjectTreeView {},
                    ProjectViewRequest::PhaseOverview {
                        phase,
                    } => Event::RequestPhaseOverviewView {
                        phase_reference: phase.into(),
                    },
                    ProjectViewRequest::PhasePlacements {
                        phase,
                    } => Event::RequestPhasePlacementsView {
                        phase_reference: phase.into(),
                    },
                };

                self.planner_core_service
                    .update(key, event)
                    .into_action()
            }
            ProjectUiCommand::UpdateView(view) => {
                match view {
                    ProjectView::Overview(project_overview) => {
                        debug!("project overview: {:?}", project_overview);
                        self.update_processes(&project_overview);

                        let mut state = self.project_ui_state.lock().unwrap();
                        state.name = Some(project_overview.name.clone());
                        state
                            .overview_ui
                            .update_overview(project_overview);
                    }
                    ProjectView::ProjectTree(project_tree) => {
                        debug!("project tree: {:?}", project_tree);
                        let mut state = self.project_ui_state.lock().unwrap();
                        state
                            .explorer_ui
                            .update_tree(project_tree);
                    }
                    ProjectView::Placements(placements) => {
                        debug!("placements: {:?}", placements);
                        let mut state = self.project_ui_state.lock().unwrap();
                        state
                            .placements_ui
                            .update_placements(placements)
                    }
                    ProjectView::PhaseOverview(phase_overview) => {
                        debug!("phase overview: {:?}", phase_overview);
                        let phase = phase_overview.phase_reference.clone();

                        self.ensure_phase(key.clone(), phase.clone());

                        let mut state = self.project_ui_state.lock().unwrap();
                        let phase_state = state.phases.get_mut(&phase).unwrap();

                        phase_state.update_overview(phase_overview);
                    }
                    ProjectView::PhasePlacements(phase_placements) => {
                        debug!("phase placements: {:?}", phase_placements);
                        let phase = phase_placements.phase_reference.clone();

                        self.ensure_phase(key.clone(), phase.clone());

                        let mut state = self.project_ui_state.lock().unwrap();
                        let phase_state = state.phases.get_mut(&phase).unwrap();

                        phase_state.update_placements(phase_placements);
                    }
                    ProjectView::PhasePlacementOrderings(_phase_placement_orderings) => {
                        // TODO
                    }
                    ProjectView::Process(_process) => {
                        // TODO
                    }
                    ProjectView::Parts(part_states) => {
                        debug!("parts: {:?}", part_states);
                        let mut state = self.project_ui_state.lock().unwrap();
                        state
                            .parts_ui
                            .update_part_states(part_states)
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
                    None => None,
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
                    Some(ProjectToolbarAction::RefreshFromDesignVariants) => self
                        .planner_core_service
                        .update(key, Event::RefreshFromDesignVariants)
                        .when_ok(|| ProjectAction::UiCommand(ProjectUiCommand::ProjectRefreshed)),
                    Some(ProjectToolbarAction::ShowAddPcbDialog) => {
                        let mut modal = AddPcbModal::new(self.path.clone());
                        modal
                            .component
                            .configure_mapper(self.component.sender.clone(), move |command| {
                                debug!("add pcb modal mapper. command: {:?}", command);
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
                                debug!("add phase modal mapper. command: {:?}", command);
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
                                debug!("create unit assignment modal mapper. command: {:?}", command);
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
                                .when_ok(|| {
                                    ProjectAction::UiCommand(ProjectUiCommand::RequestView(
                                        ProjectViewRequest::ProjectTree,
                                    ))
                                })
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
                            self.planner_core_service
                                .update(key, Event::CreatePhase {
                                    process: args.process,
                                    reference: args.reference,
                                    load_out: args.load_out,
                                    pcb_side: args.pcb_side,
                                })
                                .when_ok(|| {
                                    ProjectAction::UiCommand(ProjectUiCommand::RequestView(
                                        ProjectViewRequest::ProjectTree,
                                    ))
                                })
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
                            self.planner_core_service
                                .update(key, Event::AssignVariantToUnit {
                                    design: DesignName::from_str(&args.design_name).unwrap(),
                                    variant: VariantName::from_str(&args.variant_name).unwrap(),
                                    unit: args.object_path,
                                })
                                .when_ok(|| {
                                    ProjectAction::UiCommand(ProjectUiCommand::RequestView(
                                        ProjectViewRequest::ProjectTree,
                                    ))
                                })
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
        }
    }

    fn ui<'a>(&mut self, ui: &mut Ui, tab_key: &TabKey, context: &mut Self::Context) {
        match self {
            ProjectTabKind::Explorer(tab) => tab.ui(ui, tab_key, context),
            ProjectTabKind::Overview(tab) => tab.ui(ui, tab_key, context),
            ProjectTabKind::Parts(tab) => tab.ui(ui, tab_key, context),
            ProjectTabKind::Placements(tab) => tab.ui(ui, tab_key, context),
            ProjectTabKind::Phase(tab) => tab.ui(ui, tab_key, context),
        }
    }

    fn on_close<'a>(&mut self, tab_key: &TabKey, context: &mut Self::Context) -> bool {
        match self {
            ProjectTabKind::Explorer(tab) => tab.on_close(tab_key, context),
            ProjectTabKind::Overview(tab) => tab.on_close(tab_key, context),
            ProjectTabKind::Parts(tab) => tab.on_close(tab_key, context),
            ProjectTabKind::Placements(tab) => tab.on_close(tab_key, context),
            ProjectTabKind::Phase(tab) => tab.on_close(tab_key, context),
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
}

impl ProjectUiState {
    pub fn new(key: ProjectKey, name: Option<String>, sender: Enqueue<(ProjectKey, ProjectUiCommand)>) -> Self {
        let mut instance = Self {
            name,
            phases: HashMap::default(),
            overview_ui: OverviewUi::new(),
            placements_ui: PlacementsUi::new(),
            parts_ui: PartsUi::new(),
            explorer_ui: ExplorerUi::new(),
        };

        instance
            .explorer_ui
            .component
            .configure_mapper(sender.clone(), move |command| {
                debug!("explorer ui mapper. command: {:?}", command);
                (key, ProjectUiCommand::ExplorerUiCommand(command))
            });

        instance
            .overview_ui
            .component
            .configure_mapper(sender.clone(), move |command| {
                debug!("overview ui mapper. command: {:?}", command);
                (key, ProjectUiCommand::OverviewUiCommand(command))
            });

        instance
            .parts_ui
            .component
            .configure_mapper(sender.clone(), move |command| {
                debug!("parts ui mapper. command: {:?}", command);
                (key, ProjectUiCommand::PartsUiCommand(command))
            });

        instance
            .placements_ui
            .component
            .configure_mapper(sender.clone(), move |command| {
                debug!("placements ui mapper. command: {:?}", command);
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
    PhaseUiCommand { phase: Reference, command: PhaseUiCommand },
}

#[derive(Debug, Clone)]
pub enum ProjectError {
    CoreError(String),
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
