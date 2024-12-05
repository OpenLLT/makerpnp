use std::path::PathBuf;
use cushy::value::{Destination, Dynamic, Source};
use slotmap::new_key_type;
use tracing::debug;
use planner_app::{Event, ProjectView};
use crate::action::Action;
use crate::app_core::CoreService;
use crate::task::Task;

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


pub struct Project {
    pub(crate) name: Dynamic<Option<String>>,
    pub(crate) path: PathBuf,
    core_service: CoreService,
}

impl Project {
    pub fn new(name: String, path: PathBuf) -> (Self, ProjectMessage) {
        let core_service = CoreService::new();
        let instance = Self {
            name: Dynamic::new(Some(name)),
            path,
            core_service,
        };

        (instance, ProjectMessage::Create)
    }

    pub fn from_path(path: PathBuf) -> (Self, ProjectMessage) {
        let core_service = CoreService::new();
        let instance = Self {
            name: Dynamic::default(),
            path,
            core_service,
        };

        (instance, ProjectMessage::Load)
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
                    ProjectViewRequest::ProjectTree => Event::RequestOverviewView {},
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
}

