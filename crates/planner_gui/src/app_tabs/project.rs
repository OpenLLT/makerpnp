use cushy::value::Dynamic;
use cushy::widget::{MakeWidget, WidgetInstance};
use cushy::widgets::label::Displayable;
use slotmap::SlotMap;
use crate::action::Action;
use crate::context::Context;
use crate::project::{Project, ProjectKey, ProjectMessage};
use crate::task::Task;
use crate::widgets::tab_bar::{Tab, TabKey};

#[derive(Clone, Debug, Default)]
pub enum ProjectTabMessage {
    #[default]
    None,
    ProjectMessage(ProjectMessage)
}

#[derive(Debug)]
pub enum ProjectTabAction {
    None,
    Task(Task<ProjectMessage>),
}

#[derive(Clone)]
pub struct ProjectTab {
    pub project_key: ProjectKey,
    message: Dynamic<ProjectTabMessage>,
}

impl ProjectTab {
    pub fn new(project_key: ProjectKey, message: Dynamic<ProjectTabMessage>) -> Self {
        Self {
            project_key,
            message,
        }
    }
}

impl Tab<ProjectTabMessage, ProjectTabAction> for ProjectTab {
    fn label(&self, context: &Dynamic<Context>) -> String {
        
        "Loading...".to_string()
    }

    fn make_content(&self, context: &Dynamic<Context>, _tab_key: TabKey) -> WidgetInstance {
        let path = context.lock().with_context::<Dynamic<SlotMap<ProjectKey, Project>>, _, _>(|projects| {
            let projects_guard = projects.lock();
            let project = projects_guard.get(self.project_key).unwrap();

            project.path.clone()
        }).unwrap();

        format!("Path: '{:?}'", path).into_label().make_widget()
    }

    fn update(&mut self, context: &Dynamic<Context>, _tab_key: TabKey, message: ProjectTabMessage) -> Action<ProjectTabAction> {
        Action::new(ProjectTabAction::None)
    }
}
