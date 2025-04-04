use std::sync::Arc;

use planner_app::capabilities::view_renderer::ProjectViewRendererOperation;
use planner_app::{Effect, Event, Planner};
use tracing::{debug, error};

use crate::project::{ProjectAction, ProjectError, ProjectKey, ProjectUiCommand};
use crate::task::Task;

type Core = Arc<planner_app::Core<Planner>>;

pub struct PlannerCoreService {
    core: Core,
}

impl PlannerCoreService {
    pub fn new() -> Self {
        Self {
            core: Arc::new(planner_app::Core::new()),
        }
    }

    #[must_use]
    pub fn update(&mut self, project_key: ProjectKey, event: Event) -> ResultHelper {
        debug!("event: {:?}", event);

        let mut actions: Vec<ProjectAction> = Vec::new();

        for effect in self.core.process_event(event) {
            let action = Self::process_effect(&self.core, effect);
            match action {
                Ok(action) => {
                    actions.push(action);
                }
                Err(error) => return ResultHelper::new(project_key, Err(error)),
            }
        }

        ResultHelper::new(project_key, Ok(actions))
    }

    pub fn process_effect(core: &Core, effect: Effect) -> Result<ProjectAction, ProjectError> {
        debug!("effect: {:?}", effect);

        match effect {
            Effect::Render(_) => {
                let mut view = core.view();
                let task = match view.error.take() {
                    Some(error) => {
                        error!("core error: {:?}", error);
                        Err(ProjectError::CoreError(error))
                    }
                    None => Ok(ProjectAction::UiCommand(ProjectUiCommand::SetModifiedState(
                        view.modified,
                    ))),
                };

                task
            }
            Effect::ProjectViewRenderer(request) => {
                let ProjectViewRendererOperation::View {
                    view,
                } = request.operation;

                Ok(ProjectAction::UiCommand(ProjectUiCommand::UpdateView(view)))
            }
        }
    }
}

pub struct ResultHelper {
    result: Result<Vec<ProjectAction>, ProjectError>,
    project_key: ProjectKey,
}

impl ResultHelper {
    pub fn new(project_key: ProjectKey, result: Result<Vec<ProjectAction>, ProjectError>) -> Self {
        Self {
            project_key,
            result,
        }
    }

    #[must_use]
    pub fn when_ok<F>(self, f: F) -> Option<ProjectAction>
    where
        F: FnOnce(&mut Vec<Task<ProjectAction>>) -> Option<ProjectUiCommand>,
    {
        match self.result {
            Ok(actions) => {
                let mut tasks = vec![];
                let effect_tasks: Vec<Task<ProjectAction>> = actions
                    .into_iter()
                    .map(Task::done)
                    .collect();

                tasks.extend(effect_tasks);

                if let Some(command) = f(&mut tasks) {
                    let final_task = Task::done(ProjectAction::UiCommand(command));
                    tasks.push(final_task);
                }

                let action = ProjectAction::Task(self.project_key, Task::batch(tasks));

                Some(action)
            }
            Err(error) => Some(ProjectAction::UiCommand(ProjectUiCommand::Error(error))),
        }
    }

    pub fn into_actions(self) -> Result<Vec<ProjectAction>, ProjectAction> {
        match self.result {
            Ok(actions) => Ok(actions),
            Err(error) => Err(ProjectAction::UiCommand(ProjectUiCommand::Error(error))),
        }
    }
}
