use std::path::PathBuf;
use cushy::value::Dynamic;
use slotmap::new_key_type;
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
    Load,
    View(ProjectView),
}

#[derive(Default)]
pub enum ProjectAction {
    #[default]
    None,
    Task(Task<ProjectMessage>)
}


pub struct Project {
    pub(crate) name: Dynamic<Option<String>>,
    pub(crate) path: PathBuf,
    core_service: CoreService,
}

impl Project {
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

                // TODO a file called `.<extension>' will cause a panic here
                let project_name = self.path.file_stem().unwrap().to_str().unwrap().to_string();
                let directory_path = self.path.parent().unwrap().to_path_buf();

                let task = self.core_service.update(Event::Load { project_name, directory_path });
                ProjectAction::Task(task)
            }
            ProjectMessage::View(view) => {
                println!("View: {:?}", view);
                ProjectAction::None
            }
        };

        Action::new(action)
    }
}

