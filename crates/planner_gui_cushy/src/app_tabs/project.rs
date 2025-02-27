use cushy::reactive::value::Dynamic;
use cushy::widget::WidgetInstance;
use planner_gui_cushy::action::Action;
use planner_gui_cushy::context::Context;
use planner_gui_cushy::task::Task;
use planner_gui_cushy::widgets::tab_bar::{Tab, TabKey};
use slotmap::SlotMap;
use tracing::{error, info};

use crate::project::{Project, ProjectAction, ProjectKey, ProjectMessage};

#[derive(Clone, Debug)]
pub enum ProjectTabMessage {
    ProjectMessage(ProjectMessage),
}

#[derive(Debug)]
pub enum ProjectTabAction {
    None,
    Task(Task<ProjectMessage>),
    RenameTab(String),
    SetModifiedState(bool),
}

#[derive(Clone)]
pub struct ProjectTab {
    pub project_key: ProjectKey,

    pub label: String,
    pub modified: bool,
}

impl ProjectTab {
    pub fn new(project_key: ProjectKey) -> Self {
        Self {
            project_key,
            label: "Loading...".to_string(),
            modified: false,
        }
    }
}

impl Tab<ProjectTabMessage, ProjectTabAction> for ProjectTab {
    fn label(&self, _context: &Dynamic<Context>) -> String {
        self.label.clone()
    }

    fn modified(&self, _context: &Dynamic<Context>) -> bool {
        self.modified
    }

    fn make_content(&self, context: &Dynamic<Context>, _tab_key: TabKey) -> WidgetInstance {
        context
            .lock()
            .with_context::<Dynamic<SlotMap<ProjectKey, Project>>, _, _>(|projects| {
                let mut projects_guard = projects.lock();
                let project = projects_guard
                    .get_mut(self.project_key)
                    .unwrap();

                project.make_widget()
            })
            .unwrap()
    }

    fn update(
        &mut self,
        context: &Dynamic<Context>,
        _tab_key: TabKey,
        message: ProjectTabMessage,
    ) -> Action<ProjectTabAction> {
        let action = context
            .lock()
            .with_context::<Dynamic<SlotMap<ProjectKey, Project>>, _, _>(|projects| {
                let mut projects_guard = projects.lock();
                let project = projects_guard
                    .get_mut(self.project_key)
                    .unwrap();

                match message {
                    ProjectTabMessage::ProjectMessage(message) => {
                        let action = project.update(message);
                        match action.into_inner() {
                            ProjectAction::None => ProjectTabAction::None,
                            ProjectAction::NameChanged(name) => {
                                info!("Name changed. name: {:?}", name);
                                self.label = name.clone();
                                ProjectTabAction::RenameTab(name)
                            }
                            ProjectAction::SetModifiedState(modified) => {
                                info!("Modified state changed. modified: {:?}", modified);
                                self.modified = modified;
                                ProjectTabAction::SetModifiedState(modified)
                            }

                            ProjectAction::Task(task) => {
                                info!("ProjectAction::Task.");
                                ProjectTabAction::Task(task)
                            }
                            ProjectAction::ShowError(error) => {
                                // TODO show error dialog
                                error!("ProjectAction::ShowError. error: {}", error);
                                ProjectTabAction::None
                            }
                        }
                    }
                }
            })
            .unwrap();

        Action::new(action)
    }
}
