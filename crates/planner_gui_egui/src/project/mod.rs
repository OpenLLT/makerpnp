use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use std::path::PathBuf;
use egui::{ Modal, Ui, WidgetText};
use egui_dock::{DockArea, DockState, Style};
use egui_extras::{Column, TableBuilder};
use egui_i18n::tr;
use egui_mobius::slot::Slot;
use egui_mobius::types::{Enqueue, Value};
use regex::Regex;
use slotmap::new_key_type;
use tracing::{debug, info};
use planner_app::{Event, ProjectView, ProjectViewRequest, Reference};
use crate::planner_app_core::PlannerCoreService;
use crate::project::phase_tab::{PhaseTab, PhaseUi};
use crate::project::project_explorer_tab::{ProjectExplorerTab, ProjectExplorerUi};
use crate::tabs::{AppTabViewer, Tab, TabKey, Tabs};
use crate::task::Task;

mod project_explorer_tab;
mod phase_tab;


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

pub struct Project {
    planner_core_service: PlannerCoreService,
    sender: Enqueue<(ProjectKey, ProjectUiCommand)>,
    path: PathBuf,
    project_ui_state: Value<ProjectUiState>,

    // hold the project slot as long as the project exists, however we never read it so we need to avoid a warning.
    #[allow(dead_code)]
    project_slot: Slot<(ProjectKey, ProjectUiCommand)>,

    modified: bool,

    // list of errors to show
    errors: Vec<String>,

    // FIXME actually persist this, currently it should be treated as 'persistable_state'.
    persistent_state: Value<PersistentProjectUiState>,
}

pub struct PersistentProjectUiState {
    tabs: Value<Tabs<ProjectTabKind, ProjectTabContext>>,
    tree: DockState<TabKey>,
}

impl PersistentProjectUiState {
    fn add_tab(&mut self, tab_kind: ProjectTabKind) {
        let mut tabs = self.tabs.lock().unwrap();
        let tab_id = tabs.add(tab_kind);
        self.tree.push_to_focused_leaf(tab_id);
    }
    
    fn show_tab<F>(&mut self, f: F) 
    where
        F: Fn(&ProjectTabKind) -> bool
    {
        let tab = self
            .tree
            .iter_all_tabs()
            .find_map(|(_surface_and_node, tab_key)|{
                let tabs = self.tabs.lock().unwrap();
                let tab_kind = tabs.get(tab_key).unwrap();
                
                match f(tab_kind) {
                    true => Some(tab_key),
                    false => None,
                } 
            });
        
        if let Some(tab_key) = tab {
            let find_result = self.tree.find_tab(tab_key).unwrap();
            self.tree.set_active_tab(find_result);
        } 
    }
}

impl Project {
    pub fn from_path(path: PathBuf, sender: Enqueue<(ProjectKey, ProjectUiCommand)>, project_slot: Slot<(ProjectKey, ProjectUiCommand)>) -> (Self, ProjectUiCommand) {

        debug!("Creating project instance from path. path: {}", &path.display());

        let project_ui_state = Value::new(ProjectUiState::new(sender.clone()));

        let persistent_state = Value::new(PersistentProjectUiState {
            tabs: Value::new(Tabs::new()),
            tree: DockState::new(vec![]),
        });
        
        persistent_state.lock().unwrap().add_tab(ProjectTabKind::ProjectExplorer(ProjectExplorerTab::default()));
        
        let core_service = PlannerCoreService::new();
        let instance = Self {
            sender,
            path,
            planner_core_service: core_service,
            project_ui_state,
            project_slot,
            modified: false,
            errors: Default::default(),
            persistent_state,
        };

        (instance, ProjectUiCommand::Load)
    }

    /// Due to bugs in egui_dock where it doesn't call `on_close` when closing tabs, it's possible that the tabs
    /// and the dock tree are out of sync.  `on_close` should be removing elements from `self.tabs` corresponding to the
    /// tab being closed, but because it is not called there can be orphaned elements, we need to find and remove them.
    pub fn cleanup_tabs(&self, tab_context: &mut ProjectTabContext) {
        // TODO consider moving this method into `UiState`
        let pstate = self.persistent_state.lock().unwrap();

        let known_tab_keys = pstate
            .tree
            .iter_all_tabs()
            .map(|(_surface_and_node, tab_key)| tab_key.clone())
            .collect::<Vec<_>>();

        let mut tabs = pstate.tabs.lock().unwrap();
        
        tabs.retain_all(&known_tab_keys, tab_context);
    }

    pub fn ui(&self, ui: &mut Ui, key: ProjectKey) {

        egui::TopBottomPanel::top(ui.id().with("top_panel")).show_inside(ui, |ui| {
            ui.label(format!("Project.  path: {}", self.path.display()));

            let state = self.project_ui_state.lock().unwrap();
            if let Some(name) = &state.name {
                ui.label(format!("name: {}", name));
            } else {
                ui.spinner();
            }
        });

        let mut tab_context = ProjectTabContext {
            key,
            state: self.project_ui_state.clone(),
        };

        self.cleanup_tabs(&mut tab_context);

        let mut pstate = self.persistent_state.lock().unwrap();

        let mut project_tab_viewer = AppTabViewer {
            tabs: pstate.tabs.clone(),
            context: &mut tab_context,
        };

        let ctx = ui.ctx();
        
        DockArea::new(&mut pstate.tree)
            .id(ui.id().with("project-tabs"))
            .style(Style::from_egui(ctx.style().as_ref()))
            .show_inside(ui, &mut project_tab_viewer);

        if !self.errors.is_empty() {
            self.show_errors_modal(ui, key);
        }

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
                            self.sender.send((key, ProjectUiCommand::ClearErrors)).expect("sent")
                        }
                    },
                );
            });
    }

    pub fn update(&mut self, key: ProjectKey, command: ProjectUiCommand) -> Task<Result<(ProjectKey, ProjectUiCommand), ProjectError>>{
        match command {
            ProjectUiCommand::None => {
                Task::none()
            }
            ProjectUiCommand::Load => {
                debug!("Loading project from path. path: {}", self.path.display());

                self.planner_core_service.update(Event::Load {
                    path: self.path.clone(),
                }, key)
                    .map(|result| {
                        result.map(|(key, _)| (key, ProjectUiCommand::Loaded))
                    })
            }
            ProjectUiCommand::Loaded => {
                let mut state = self.project_ui_state.lock().unwrap();
                state.loaded = true;
                self
                    .planner_core_service
                    .update(Event::RequestOverviewView {}, key)
                    .chain(Task::done(Ok((key, ProjectUiCommand::RequestView(ProjectViewRequest::ProjectTree)))))
            }
            ProjectUiCommand::RequestView(view_request) => {
                let event = match view_request {
                    ProjectViewRequest::Overview => Event::RequestOverviewView {},
                    ProjectViewRequest::ProjectTree => Event::RequestProjectTreeView {},
                    ProjectViewRequest::PhaseOverview { phase} => Event::RequestPhaseOverviewView { phase_reference: phase.into() },
                    ProjectViewRequest::PhasePlacements { phase } => Event::RequestPhasePlacementsView { phase_reference: phase.into() },
                };

                self.planner_core_service.update(event, key)
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
                Task::none()
            }
            ProjectUiCommand::Error(error) => {
                match error {
                    ProjectError::CoreError(message) => {
                        self.errors.push(message);
                    }
                }
                Task::none()
            }
            ProjectUiCommand::ClearErrors => {
                self.errors.clear();
                Task::none()
            }
            ProjectUiCommand::SetModifiedState(modified_state) => {
                self.modified = modified_state;
                Task::none()
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
                        Task::done(Ok((key, ProjectUiCommand::RequestView(ProjectViewRequest::PhaseOverview {
                            phase: phase_reference.clone(),
                        })))),

                        Task::done(Ok((key, ProjectUiCommand::RequestView(ProjectViewRequest::PhasePlacements {
                            phase: phase_reference.clone(),
                        })))),
                    ];

                    Task::batch(tasks)
                } else {
                    Task::none()
                }
            }
        }
    }

    pub fn show_phase(&mut self, phase: Reference) {
        let mut state = self.project_ui_state.lock().unwrap();

        let (_entry, is_new) = match state.phases.entry(phase.clone()) {
            Entry::Occupied(entry) => {
                debug!("phase previously shown. phase: {:?}", phase);
                (entry.into_mut(), false)
            }
            Entry::Vacant(entry) => {
                debug!("phase not previously shown. phase: {:?}", phase);

                (entry.insert(PhaseUi::new()), true)
            }
        };

        let mut pstate = self.persistent_state.lock().unwrap();
        let tab = PhaseTab::new(phase);
        if is_new {
            pstate.add_tab(ProjectTabKind::Phase(tab));
        } else {
            pstate.show_tab(|candidate_tab| {
                matches!(candidate_tab, ProjectTabKind::Phase(phase_tab) if phase_tab.eq(&tab))
            });
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
#[derive(Debug)]
enum ProjectTabKind {
    ProjectExplorer(ProjectExplorerTab),
    Phase(PhaseTab),
}

pub struct ProjectTabContext {
    key: ProjectKey,
    state: Value<ProjectUiState>,
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
