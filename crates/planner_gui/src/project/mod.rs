use std::fmt::{Display, Formatter};
use std::ops::Deref;
use std::path::PathBuf;
use cushy::value::{Destination, Dynamic, Source};
use cushy::widget::{MakeWidget, WidgetInstance};
use cushy::widgets::label::Displayable;
use cushy::widgets::pile::{Pile, PiledWidget};
use slotmap::new_key_type;
use tracing::{debug, info, trace};
use planner_app::{Event, ProjectTreeView, ProjectView, Reference};
use planner_gui::action::Action;
use crate::app_core::CoreService;
use planner_gui::task::Task;
use cushy::widgets::tree::{Tree, TreeNodeKey};
use petgraph::visit::{depth_first_search, Control, DfsEvent};
use regex::Regex;

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

#[derive(Debug, Clone)]
pub enum ProjectMessage {
    None,
    
    //
    // User interactions
    //
    
    Load,
    Navigate(ProjectPath),
    
    //
    // Internal messages
    //
    Error(String),
    UpdateView(ProjectView),
    Loaded,
    Create,
    Created,
    RequestView(ProjectViewRequest),
}

#[derive(Debug, Clone)]
pub enum ProjectViewRequest {
    Overview,
    ProjectTree,
    PhaseOverview { phase: String },
}


#[derive(Default)]
pub enum ProjectAction {
    #[default]
    None,
    Task(Task<ProjectMessage>),
    ShowError(String),
    NameChanged(String),
}

pub struct Project {
    pub(crate) name: Dynamic<Option<String>>,
    pub(crate) path: PathBuf,
    core_service: CoreService,
    project_tree: Dynamic<Tree>,
    project_tree_path: Dynamic<ProjectPath>,
    message: Dynamic<ProjectMessage>,
    
    phase_overview_widget: Option<PiledWidget>,
    
    default_content_handle: Option<PiledWidget>
}

impl Project {
    pub fn new(name: String, path: PathBuf, project_message: Dynamic<ProjectMessage>) -> (Self, ProjectMessage) {
        let project_tree = Dynamic::new(Tree::default());
        
        let core_service = CoreService::new();
        let instance = Self {
            name: Dynamic::new(Some(name)),
            path,
            core_service,
            project_tree,
            project_tree_path: Dynamic::new(ProjectPath("/".to_string())),
            message: project_message,
            phase_overview_widget: None,
            default_content_handle: None,
        };

        (instance, ProjectMessage::Create)
    }

    pub fn from_path(path: PathBuf, project_message: Dynamic<ProjectMessage>) -> (Self, ProjectMessage) {
        let project_tree = Dynamic::new(Tree::default());
        let core_service = CoreService::new();
        let instance = Self {
            name: Dynamic::default(),
            path,
            core_service,
            project_tree,
            project_tree_path: Dynamic::new(ProjectPath("/".to_string())),
            message: project_message,
            phase_overview_widget: None,
            default_content_handle: None,
        };

        (instance, ProjectMessage::Load)
    }

    pub fn make_widget(&mut self) -> WidgetInstance {
        
        let project_tree_widget = self.project_tree.lock().make_widget();
        let project_explorer = "Project Explorer".contain()
            .and(project_tree_widget.contain())
            .into_rows()
            .contain()
            .make_widget();

        let content_pile = Pile::default();

        
        let default_content = "content-pane"
            .to_label()
            .centered()
            .and(self.project_tree_path.to_label().centered())
            .into_rows();
        
        let default_content_handle = content_pile
            .push(default_content);
        
        // TODO improve this workaround for https://github.com/khonsulabs/cushy/issues/231
        //      we have to store the default_content_handle otherwise the widget
        //      will be dropped at the end of this method, and when it does the widget will be removed from the pile
        //      so we have to hold on to the handle, but doing so required changing this method to accept `&mut self` instead of `&self`
        self.default_content_handle.replace(default_content_handle);
        
        let content_pane = content_pile
            .expand()
            .contain();
                
        project_explorer
            .and(content_pane)
            .into_columns()
            .expand_horizontally()
            .make_widget()
    }

    pub fn update(&mut self, message: ProjectMessage) -> Action<ProjectAction> {
        let action = match message {
            ProjectMessage::None => {
                ProjectAction::None
            }
            ProjectMessage::Load => {
                let task = self.core_service
                    .update(Event::Load { path: self.path.clone() })
                    .chain(Task::done(ProjectMessage::Loaded));
                ProjectAction::Task(task)
            },
            ProjectMessage::Loaded => {
                let task = self.core_service
                    .update(Event::RequestOverviewView {})
                    .chain(Task::done(ProjectMessage::RequestView(ProjectViewRequest::ProjectTree)));
                ProjectAction::Task(task)
            }
            ProjectMessage::Create => {
                let task = self.core_service
                    .update(Event::CreateProject { name: self.name.get().unwrap(), path: self.path.clone() })
                    .chain(Task::done(ProjectMessage::Created));
                ProjectAction::Task(task)
            },
            ProjectMessage::Created => {
                let task = self.core_service
                    .update(Event::RequestOverviewView {})
                    .chain(Task::done(ProjectMessage::RequestView(ProjectViewRequest::ProjectTree)));
                ProjectAction::Task(task)
            },
            ProjectMessage::RequestView(view) => {
                let event = match view {
                    ProjectViewRequest::Overview => Event::RequestOverviewView {},
                    ProjectViewRequest::ProjectTree => Event::RequestProjectTreeView {},
                    ProjectViewRequest::PhaseOverview{ phase } => Event::RequestPhaseOverviewView { phase: Reference(phase) },
                };
                
                let task = self.core_service
                    .update( event);
                ProjectAction::Task(task)
            }
            ProjectMessage::Navigate(path) => {
                // TODO if the path starts with `/project/` then show/hide UI elements based on the path, 
                //      e.g. update a dynamic that controls a per-project-tab-bar dynamic selector
                info!("ProjectMessage::Navigate. path: {}", path);
                self.project_tree_path.set(path.clone());

                let phase_pattern = Regex::new(r"/project/phases/(?<phase>.*){1}").unwrap();
                if let Some(captures) = phase_pattern.captures(&path) {
                    let phase_reference: String = captures.name("phase").unwrap().as_str().to_string();
                    debug!("phase_reference: {}", phase_reference);
                    

                    // TODO activate the corresponding pile widget
                    
                    ProjectAction::Task(Task::done(ProjectMessage::RequestView(ProjectViewRequest::PhaseOverview { phase: 
                        phase_reference 
                    })))
                } else {
                    ProjectAction::None
                }
            }
            ProjectMessage::Error(error) => {
                ProjectAction::ShowError(error)
            }
            ProjectMessage::UpdateView(view) => {
                // update the GUI using the view
                match view {
                    ProjectView::Overview(project_overview) => {
                        debug!("project overview: {:?}", project_overview);
                        self.name.set(Some(project_overview.name.clone()));
                        
                        ProjectAction::NameChanged(project_overview.name)
                    }
                    ProjectView::ProjectTree(project_tree) => {
                        debug!("project tree: {:?}", project_tree);

                        self.update_tree(project_tree);
                        
                        ProjectAction::None
                    }
                    ProjectView::Placements(placements) => {
                        ProjectAction::None
                    }
                    ProjectView::PhaseOverview(phase_overview) => {
                        debug!("phase overview: {:?}", phase_overview);
                        ProjectAction::None
                    }
                    ProjectView::PhasePlacementOrderings(phase_placement_orderings) => {
                        ProjectAction::None
                    }
                }
            }
        };

        Action::new(action)
    }

    fn update_tree(&mut self, project_tree_view: ProjectTreeView) {

        // TODO maybe synchronize instead of rebuild, when we need to show a selected tree item this will be a problem
        //      as the selection will be lost and need to be re-determined.
        //      instead of syncronization, maybe just remember the 'path' and re-select a tree item that has the same path  
        let mut project_tree = self.project_tree.lock();
        project_tree.clear();

        //
        // Create the tree widget nodes from the project tree view
        //
        // Assumes the only relationships in the tree are parent->child, i.e. parent->grandchild is catered handled.

        use petgraph::graph::node_index as n;

        let start = n(0);
        
        let mut stack: Vec<(Option<TreeNodeKey>, Option<TreeNodeKey>)> = vec![];

        let mut current_parent_key: Option<TreeNodeKey> = None;
        let mut current_node_key: Option<TreeNodeKey> = None;
        
        // FIXME depth_first_search doesn't emit (Discover) nodes in the same order they were added to the tree.
        //       the order *is* important here.
        
        depth_first_search(&project_tree_view.tree, Some(start),{

            |event| {

                trace!("dfs. event: {:?}", event);
                match event {
                    DfsEvent::Discover(node, _) => {
                        let item = &project_tree_view.tree[node];
                        
                        let path = ProjectPath(format!("/project{}", item.path).to_string());

                        let message = self.message.clone();
                        let node_widget = item.name
                            .to_button()
                            .on_click(move |_event|{
                                message.force_set(ProjectMessage::Navigate(path.clone()));
                            })
                            .make_widget();

                        let child_key = project_tree.insert_child(node_widget, current_parent_key.as_ref()).unwrap();

                        current_node_key.replace(child_key);
                    }
                    DfsEvent::TreeEdge(_from, _to) => {
                        stack.push((current_node_key.clone(), current_parent_key.clone()));
                        current_parent_key.replace(current_node_key.take().unwrap());
                        current_node_key.take();
                    }
                    DfsEvent::Finish(_from, _time) => {
                        if let Some((node_key, parent_key)) = stack.pop() {
                            current_node_key.replace(node_key.unwrap_or_default());
                            current_parent_key.replace(parent_key.unwrap_or_default());
                        }
                    }
                    _ => {
                    }
                }
                Control::<()>::Continue
            }
        });
    }
}

