use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use std::path::PathBuf;
use egui::{ Modal, Ui, WidgetText};
use egui_dock::{DockArea, DockState, Style, TabViewer};
use egui_extras::{Column, TableBuilder};
use egui_i18n::{tr, translate_fluent};
use egui_mobius::slot::Slot;
use egui_mobius::types::{Enqueue, Value, ValueGuard};
use petgraph::Graph;
use petgraph::prelude::NodeIndex;
use regex::Regex;
use slotmap::new_key_type;
use tracing::{debug, info};
use i18n::fluent_argument_helpers::planner_app::build_fluent_args;
use planner_app::{Event, PhaseOverview, PhasePlacements, ProjectTreeItem, ProjectTreeView, ProjectView, ProjectViewRequest, Reference};
use crate::planner_app_core::PlannerCoreService;
use crate::task::Task;

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

    // tree wrapped in a value because `ui` gives `&self` not `&mut self` and dock needs to modify itself. 
    tree: Value<DockState<ProjectTab>>,
}

impl Project {
    pub fn from_path(path: PathBuf, sender: Enqueue<(ProjectKey, ProjectUiCommand)>, project_slot: Slot<(ProjectKey, ProjectUiCommand)>) -> (Self, ProjectUiCommand) {

        debug!("Creating project instance from path. path: {}", &path.display());

        let project_ui_state = Value::new(ProjectUiState::default());

        let core_service = PlannerCoreService::new();
        let instance = Self {
            sender,
            path,
            planner_core_service: core_service,
            project_ui_state,
            project_slot,
            modified: false,
            errors: Default::default(),
            tree: Value::new(DockState::new(vec![ProjectTab::ProjectExplorer])),
        };

        (instance, ProjectUiCommand::Load)
    }



    pub fn ui(&self, ui: &mut Ui, key: ProjectKey) {
        let state = self.project_ui_state.lock().unwrap();

        egui::TopBottomPanel::top("top_panel").show_inside(ui, |ui| {
            ui.label(format!("Project.  path: {}", self.path.display()));

            if let Some(name) = &state.name {
                ui.label(format!("name: {}", name));
            } else {
                ui.spinner();
            }
        });

        let mut project_tab_viewer = ProjectTabViewer {
            state: &state,
            sender: self.sender.clone(),
            key
        };

        let ctx = ui.ctx();

        let mut tree = self.tree.lock().unwrap();

        DockArea::new(&mut tree)
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

                        self.update_tree(project_tree)
                    }
                    ProjectView::Placements(placements) => {
                        todo!()
                    }
                    ProjectView::PhaseOverview(phase_overview) => {
                        debug!("phase overview: {:?}", phase_overview);
                        let phase = phase_overview.phase_reference.clone();
                        let mut state = self.project_ui_state.lock().unwrap();
                        let phase_state = state.phases.entry(phase).or_insert(PhaseUiState::default());
                        phase_state.overview.replace(phase_overview);
                    }
                    ProjectView::PhasePlacements(phase_placements) => {
                        debug!("phase placements: {:?}", phase_placements);
                        let phase = phase_placements.phase_reference.clone();
                        let mut state = self.project_ui_state.lock().unwrap();
                        let phase_state = state.phases.entry(phase).or_insert(PhaseUiState::default());
                        phase_state.placements.replace(phase_placements);
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

                let mut state = self.project_ui_state.lock().unwrap();

                Self::update_project_tree_view(&path, &mut state);

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

    fn update_project_tree_view(project_path: &ProjectPath, state: &mut ValueGuard<ProjectUiState>) {
        // it's an error to be given a path without a corresponding tree view node that has the same path
        let path = view_path_from_project_path(project_path).unwrap();

        let graph = &state.project_tree_view.as_mut().unwrap().tree;

        if let Some(node) = graph.node_indices().find(|&index| {
            graph[index].path.eq(&path)
        }) {
            state.project_tree_state.clear();
            state.project_tree_state.insert(node, true);
        } else {
            unreachable!()
        }
    }

    fn update_tree(&mut self, project_tree_view: ProjectTreeView) {
        let mut state = self.project_ui_state.lock().unwrap();
        state.project_tree_view.replace(project_tree_view);
    }
}

struct ProjectTabViewer<'a> {
    state: &'a ProjectUiState,
    sender: Enqueue<(ProjectKey, ProjectUiCommand)>,
    key: ProjectKey,
}

impl<'a> TabViewer for ProjectTabViewer<'a> {
    type Tab = ProjectTab;

    fn title(&mut self, tab: &mut Self::Tab) -> WidgetText {
        let title = match tab {
            ProjectTab::ProjectExplorer => tr!("project-explorer-tab-label"),
            ProjectTab::Phase(reference) => format!("{}", reference).to_string(),
        };

        egui::widget_text::WidgetText::from(title)
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut Self::Tab) {
        let state = self.state;
        match &tab {
            ProjectTab::ProjectExplorer => {
                if let Some(tree) = &state.project_tree_view {
                    self.show_project_tree(ui, &tree.tree, NodeIndex::new(0), &state.project_tree_state, self.key);
                }
            }
            ProjectTab::Phase(_) => {}
        }
    }

    fn closeable(&mut self, _tab: &mut Self::Tab) -> bool {
        match _tab {
            ProjectTab::ProjectExplorer => false,
            ProjectTab::Phase(_) => true,
        }
    }
}

impl<'a> ProjectTabViewer<'a> {
    fn show_project_tree(&self, ui: &mut egui::Ui, graph: &Graph<ProjectTreeItem, ()>, node: NodeIndex, selection_state: &HashMap<NodeIndex, bool>, project_key: ProjectKey) {
        let item = &graph[node];

        let path = project_path_from_view_path(&item.path);

        let key = format!("project-explorer-node-{}", item.key);
        let args = build_fluent_args(&item.args);

        let label = translate_fluent(&key, &args);

        let mut is_selected = if let Some(value) = selection_state.get(&node) {
            *value
        } else {
            false
        };

        let id = ui.make_persistent_id(node);

        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true)
            .show_header(ui, |ui| {
                if ui.toggle_value(&mut is_selected, label).clicked() {
                    self.sender.send((project_key, ProjectUiCommand::Navigate(path))).expect("sent");
                }
            })
            .body(|ui| {
                for neighbor in graph.neighbors(node) {
                    self.show_project_tree(ui, graph, neighbor, selection_state, project_key);
                }
            });
    }

}

#[derive(Debug)]
pub struct ProjectUiState {
    loaded: bool,
    name: Option<String>,
    project_tree_view: Option<ProjectTreeView>,
    project_tree_state: HashMap<NodeIndex, bool>,
    phases: HashMap<Reference, PhaseUiState>,
}

impl Default for ProjectUiState {
    fn default() -> Self {
        Self {
            loaded: false,
            name: None,
            project_tree_view: None,
            project_tree_state: HashMap::new(),
            phases: HashMap::default(),
        }
    }
}

#[derive(Debug)]
enum ProjectTab {
    ProjectExplorer,
    Phase(Reference),
}

#[derive(Default, Debug)]
pub struct PhaseUiState {
    overview: Option<PhaseOverview>,
    placements: Option<PhasePlacements>,
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
