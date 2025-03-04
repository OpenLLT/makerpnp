use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use std::path::PathBuf;
use egui::{ Modal, Ui, WidgetText};
use egui_extras::{Column, TableBuilder};
use egui_i18n::tr;
use egui_mobius::types::{Enqueue, Value};
use regex::Regex;
use slotmap::new_key_type;
use tracing::{debug, info};
use planner_app::{Event, ProjectView, ProjectViewRequest, Reference};
use crate::planner_app_core::PlannerCoreService;
use crate::project::phase_tab::{PhaseTab, PhaseUi};
use crate::project::project_explorer_tab::{ProjectExplorerTab, ProjectExplorerUi};
use crate::project::tabs::{ProjectTabAction, ProjectTabContext, ProjectTabUiCommand, ProjectTabs};
use crate::project::toolbar::{ProjectToolbar, ProjectToolbarAction, ProjectToolbarUiCommand};
use crate::tabs::{Tab, TabKey};
use crate::task::Task;
use crate::ui_component::{ComponentState, UiComponent};

mod project_explorer_tab;
mod phase_tab;
mod toolbar;
mod tabs;


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

pub enum ProjectAction {
    Task(ProjectKey, Task<Result<ProjectUiCommand, ProjectError>>),
}

pub struct Project {
    planner_core_service: PlannerCoreService,
    path: PathBuf,
    project_ui_state: Value<ProjectUiState>,

    modified: bool,

    // list of errors to show
    errors: Vec<String>,

    // FIXME actually persist this, currently it should be treated as 'persistable_state'.
    project_tabs: Value<ProjectTabs>,

    toolbar: ProjectToolbar,

    pub component: ComponentState<(ProjectKey, ProjectUiCommand)>,
}

impl Project {
    pub fn from_path(path: PathBuf, key: ProjectKey) -> (Self, ProjectUiCommand) {
        debug!("Creating project instance from path. path: {}", &path.display());

        let component: ComponentState<(ProjectKey, ProjectUiCommand)> = ComponentState::default();
        let component_sender = component.sender.clone();

        let mut toolbar = ProjectToolbar::default();
        toolbar.component.configure_mapper(component_sender.clone(), move |command|{
            debug!("project toolbar mapper. command: {:?}", command);
            (key, ProjectUiCommand::ToolbarCommand(command))
        });

        let project_ui_state = Value::new(ProjectUiState::new(component_sender.clone()));

        let project_tabs = Value::new(ProjectTabs::default());
        {
            let mut project_tabs = project_tabs.lock().unwrap();
            project_tabs.component.configure_mapper(component_sender,move |command|{
                debug!("project inner-tab mapper. command: {:?}", command);
                (key, ProjectUiCommand::TabCommand(command))
            });
            project_tabs.add_tab(ProjectTabKind::ProjectExplorer(ProjectExplorerTab::default()));
        }

        let core_service = PlannerCoreService::new();
        let instance = Self {
            path,
            planner_core_service: core_service,
            project_ui_state,
            modified: false,
            errors: Default::default(),
            project_tabs,
            toolbar,
            component,
        };

        (instance, ProjectUiCommand::Load)
    }

    pub fn show_explorer(&mut self) {
        let mut project_tabs = self.project_tabs.lock().unwrap();
        let result = project_tabs.show_tab(|candidate_tab|matches!(candidate_tab, ProjectTabKind::ProjectExplorer(_)));
        if result.is_err() {
            project_tabs.add_tab(ProjectTabKind::ProjectExplorer(ProjectExplorerTab::default()));
        }
    }

    pub fn show_phase(&mut self, phase: Reference) {
        let mut project_tabs = self.project_tabs.lock().unwrap();
        let tab = PhaseTab::new(phase.clone());
        project_tabs.show_tab(|candidate_tab| {
            matches!(candidate_tab, ProjectTabKind::Phase(phase_tab) if phase_tab.eq(&tab))
        }).inspect(|tab_key|{
            debug!("showing existing phase tab. phase: {:?}, tab_key: {:?}", phase, tab_key);
        }).inspect_err(|_|{
            let mut state = self.project_ui_state.lock().unwrap();
            state.phases.insert(phase.clone(), PhaseUi::new());
            let tab_key = project_tabs.add_tab(ProjectTabKind::Phase(tab));
            debug!("adding phase tab. phase: {:?}, tab_key: {:?}", phase, tab_key);
        }).ok();
    }

    fn show_errors_modal(&self, ui: &mut Ui, key: ProjectKey) {
        let errors_modal_id = ui.id().with("errors");

        Modal::new(errors_modal_id)
            .show(ui.ctx(), |ui| {
                ui.set_width(ui.available_width() * 0.8);
                let file_name = self.path.file_name().unwrap().to_str().unwrap();
                ui.heading(tr!("modal-errors-title", {file: file_name}));

                let table = TableBuilder::new(ui)
                    .striped(true)
                    .resizable(true)
                    .column(Column::auto())
                    .column(Column::remainder());

                table.header(20.0, |mut header| {
                    header.col(|ui| {
                        ui.strong(tr!("modal-errors-column-errors"));
                    });
                }).body(|mut body| {
                    for (index, error) in self.errors.iter().enumerate() {
                        body.row(18.0, |mut row| {
                            row.col(|ui| {
                                ui.label(format!("{}", index));
                            });
                            row.col(|ui| {
                                ui.label(error);
                            });
                        })
                    }
                });


                egui::Sides::new().show(
                    ui,
                    |_ui| {},
                    |ui| {
                        if ui.button(tr!("form-button-ok")).clicked() {
                            self.component.send((key, ProjectUiCommand::ClearErrors))
                        }
                    },
                );
            });
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
            key
        } = context;

        egui::TopBottomPanel::top(ui.id().with("top_panel")).show_inside(ui, |ui| {
            ui.label(format!("Project.  path: {}", self.path.display()));

            let state = self.project_ui_state.lock().unwrap();
            if let Some(name) = &state.name {
                ui.label(format!("name: {}", name));
            } else {
                ui.spinner();
            }

            self.toolbar.ui(ui, &mut ());
        });

        let mut tab_context = ProjectTabContext {
            key: *key,
            state: self.project_ui_state.clone(),
        };


        let mut project_tabs = self.project_tabs.lock().unwrap();
        project_tabs.cleanup_tabs(&mut tab_context);
        project_tabs.ui(ui, &mut tab_context);

        if !self.errors.is_empty() {
            self.show_errors_modal(ui, *key);
        }

    }

    fn update<'context>(&mut self, command: Self::UiCommand, _context: &mut Self::UiContext<'context>) -> Option<Self::UiAction> {

        let (key, command) = command;

        match command {
            ProjectUiCommand::None => {
                None
            }
            ProjectUiCommand::Load => {
                debug!("Loading project from path. path: {}", self.path.display());

                let task = self.planner_core_service.update(Event::Load {
                    path: self.path.clone(),
                })
                    .map(|result| {
                        result.map(|_| ProjectUiCommand::Loaded)
                    });
                Some(ProjectAction::Task(key, task))
            }
            ProjectUiCommand::Loaded => {
                let mut state = self.project_ui_state.lock().unwrap();
                state.loaded = true;
                let task = self
                        .planner_core_service
                        .update(Event::RequestOverviewView {})
                        .chain(Task::done(Ok(ProjectUiCommand::RequestView(ProjectViewRequest::ProjectTree))));
                Some(ProjectAction::Task(key, task))
            }
            ProjectUiCommand::RequestView(view_request) => {
                let event = match view_request {
                    ProjectViewRequest::Overview => Event::RequestOverviewView {},
                    ProjectViewRequest::ProjectTree => Event::RequestProjectTreeView {},
                    ProjectViewRequest::PhaseOverview { phase} => Event::RequestPhaseOverviewView { phase_reference: phase.into() },
                    ProjectViewRequest::PhasePlacements { phase } => Event::RequestPhasePlacementsView { phase_reference: phase.into() },
                };

                let task = self.planner_core_service.update(event);
                Some(ProjectAction::Task(key, task))
            }
            ProjectUiCommand::UpdateView(view) => {
                match view {
                    ProjectView::Overview(project_overview) => {
                        debug!("project overview: {:?}", project_overview);
                        let mut state = self.project_ui_state.lock().unwrap();
                        state.name = Some(project_overview.name);
                    }
                    ProjectView::ProjectTree(project_tree) => {
                        debug!("project tree: {:?}", project_tree);
                        let mut state = self.project_ui_state.lock().unwrap();
                        state.project_tree.update_tree(project_tree)
                    }
                    ProjectView::Placements(placements) => {
                        todo!()
                    }
                    ProjectView::PhaseOverview(phase_overview) => {
                        debug!("phase overview: {:?}", phase_overview);
                        let phase = phase_overview.phase_reference.clone();
                        let mut state = self.project_ui_state.lock().unwrap();
                        let phase_state = state.phases.entry(phase.clone()).or_insert(PhaseUi::new());
                        phase_state.update_overview(phase_overview);
                    }
                    ProjectView::PhasePlacements(phase_placements) => {
                        debug!("phase placements: {:?}", phase_placements);
                        let phase = phase_placements.phase_reference.clone();
                        let mut state = self.project_ui_state.lock().unwrap();
                        let phase_state = state.phases.entry(phase.clone()).or_insert(PhaseUi::new());
                        phase_state.update_placements(phase_placements);
                    }
                    ProjectView::PhasePlacementOrderings(_) => {}
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
                // TODO return an action so that the UI can mark the project's tab with an indicator
                None
            }
            ProjectUiCommand::Navigate(path) => {
                {
                    let mut state = self.project_ui_state.lock().unwrap();
                    state.project_tree.select_path(&path);
                }

                // if the path starts with `/project/` then show/hide UI elements based on the path,
                // e.g. update a dynamic that controls a per-project-tab-bar dynamic selector
                info!("ProjectMessage::Navigate. path: {}", path);

                let phase_pattern = Regex::new(r"/project/phases/(?<phase>.*){1}").unwrap();
                if let Some(captures) = phase_pattern.captures(&path) {
                    let phase_reference: String = captures
                        .name("phase")
                        .unwrap()
                        .as_str()
                        .to_string();
                    debug!("phase_reference: {}", phase_reference);

                    self.show_phase(phase_reference.clone().into());

                    let tasks: Vec<_> = vec![
                        Task::done(Ok(ProjectUiCommand::RequestView(ProjectViewRequest::PhaseOverview {
                            phase: phase_reference.clone(),
                        }))),

                        Task::done(Ok(ProjectUiCommand::RequestView(ProjectViewRequest::PhasePlacements {
                            phase: phase_reference.clone(),
                        }))),
                    ];

                    Some(ProjectAction::Task(key, Task::batch(tasks)))
                } else {
                    None
                }
            }
            ProjectUiCommand::ToolbarCommand(toolbar_command) => {
                let action = self.toolbar.update(toolbar_command, &mut ());
                match action {
                    None => {}
                    Some(ProjectToolbarAction::ShowProjectExplorer) => {
                        self.show_explorer();
                    }
                }
                None
            }
            ProjectUiCommand::TabCommand(tab_command) => {
                let mut project_tabs = self.project_tabs.lock().unwrap();
                
                let mut context = ProjectTabContext {
                    key,
                    state: self.project_ui_state.clone(),
                };
                
                let action = project_tabs.update(tab_command, &mut context);
                match action {
                    None => {}
                    Some(ProjectTabAction::None) => {
                        debug!("ProjectTabAction::None");
                    }
                }
                None
            }
        }
    }
}

impl Tab for ProjectTabKind {
    type Context = ProjectTabContext;

    fn label(&self) -> WidgetText {
        let title = match self {
            ProjectTabKind::ProjectExplorer(tab) => tab.label(),
            ProjectTabKind::Phase(tab) => tab.label(),
        };

        egui::widget_text::WidgetText::from(title)
    }

    fn ui<'a>(&mut self, ui: &mut Ui, tab_key: &TabKey, context: &mut Self::Context) {
        match self {
            ProjectTabKind::ProjectExplorer(tab) => tab.ui(ui, tab_key, context),
            ProjectTabKind::Phase(tab) => tab.ui(ui, tab_key, context),
        }
    }

    fn on_close<'a>(&mut self, tab_key: &TabKey, context: &mut Self::Context) -> bool {
        match self {
            ProjectTabKind::ProjectExplorer(tab) => tab.on_close(tab_key, context),
            ProjectTabKind::Phase(tab) => tab.on_close(tab_key, context),
        }
    }
}

#[derive(Debug)]
pub struct ProjectUiState {
    loaded: bool,
    name: Option<String>,
    project_tree: ProjectExplorerUi,
    phases: HashMap<Reference, PhaseUi>,
}

impl ProjectUiState {
    pub fn new(sender: Enqueue<(ProjectKey, ProjectUiCommand)>) -> Self {
        Self {
            loaded: false,
            name: None,
            phases: HashMap::default(),
            project_tree: ProjectExplorerUi::new(sender),
        }
    }
}

// these should not contain state
#[derive(serde::Deserialize, serde::Serialize, Debug)]
enum ProjectTabKind {
    ProjectExplorer(ProjectExplorerTab),
    Phase(PhaseTab),
}

#[derive(Debug, Clone)]
pub enum ProjectUiCommand {
    None,
    Load,
    Loaded,
    UpdateView(ProjectView),
    Error(ProjectError),
    SetModifiedState(bool),
    RequestView(ProjectViewRequest),
    ClearErrors,
    Navigate(ProjectPath),
    ToolbarCommand(ProjectToolbarUiCommand),
    TabCommand(ProjectTabUiCommand),
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
    let view_path = project_path.to_string().split("/project").collect::<Vec<&str>>().get(1)?.to_string();
    Some(view_path)
}
