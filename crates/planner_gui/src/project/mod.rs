use std::path::PathBuf;
use cushy::value::{Destination, Dynamic, Source};
use cushy::widget::{MakeWidget, WidgetInstance};
use cushy::widgets::label::Displayable;
use slotmap::new_key_type;
use tracing::debug;
use planner_app::{Event, ProjectTree, ProjectView};
use planner_gui::action::Action;
use crate::app_core::CoreService;
use planner_gui::task::Task;
use cushy::widgets::tree::Tree;

new_key_type! {
    /// A key for a project
    pub struct ProjectKey;
}

#[derive(Debug, Clone)]
pub enum ProjectMessage {
    None,
    
    //
    // User interactions
    //
    
    Load,
    Navigate(String),
    
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
}


#[derive(Default)]
pub enum ProjectAction {
    #[default]
    None,
    Task(Task<ProjectMessage>),
    Navigate(String),
    ShowError(String),
    NameChanged(String),
}

#[derive(Default)]
struct ProjectTreeViewItem {
    name: String,
}

pub struct Project {
    pub(crate) name: Dynamic<Option<String>>,
    pub(crate) path: PathBuf,
    core_service: CoreService,
    project_tree: Dynamic<Tree>,
}

impl Project {
    pub fn new(name: String, path: PathBuf) -> (Self, ProjectMessage) {
        let project_tree = Dynamic::new(Tree::default());
        
        let core_service = CoreService::new();
        let instance = Self {
            name: Dynamic::new(Some(name)),
            path,
            core_service,
            project_tree,
        };

        (instance, ProjectMessage::Create)
    }

    pub fn from_path(path: PathBuf) -> (Self, ProjectMessage) {
        let project_tree = Dynamic::new(Tree::default());
        let core_service = CoreService::new();
        let instance = Self {
            name: Dynamic::default(),
            path,
            core_service,
            project_tree,
        };

        (instance, ProjectMessage::Load)
    }

    pub fn make_widget(&self) -> WidgetInstance {

        let project_tree_widget = self.project_tree.lock().make_widget();
        let project_explorer = "Project Explorer".contain()
            .and(project_tree_widget.contain())
            .into_rows()
            .contain()
            .make_widget();

        project_explorer
            .and("content-pane".to_label().centered().expand().contain())
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
                };
                
                let task = self.core_service
                    .update( event);
                ProjectAction::Task(task)
            }
            ProjectMessage::Navigate(path) => {
                // TODO if the path starts with `/project/` then show/hide UI elements based on the path, e.g. update a dynamic that controls a per-project-tab-bar dynamic selector
                //      otherwise delegate to the app.
                ProjectAction::Navigate(path)
            }
            ProjectMessage::Error(error) => {
                ProjectAction::ShowError(error)
            }
            ProjectMessage::UpdateView(view) => {
                // TODO update the GUI using the view
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

    fn update_tree(&mut self, project_tree_view: ProjectTree) {

        // TODO synchronize instead of rebuild, when we need to show a selected tree item this will be a problem
        //      as the selection will be lost and need to be re-determined.
        // FIXME currently the `project_tree` is not a tree, it's a list with no depth and no way to uniquely identify items
        //       so the only way to synchronize would be by name, which is a bad idea.
        let mut project_tree = self.project_tree.lock();
        project_tree.clear();
        
        let root = project_tree.insert_child("Root".to_label().make_widget(), None).unwrap();
        
        for item in project_tree_view.items {
            let child_key = project_tree.insert_child(item.name.to_label().make_widget(), Some(&root));
        }
    }
}

