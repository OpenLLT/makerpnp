use std::sync::Arc;
use planner_app::{Effect, Event, Planner};
use planner_app::capabilities::view_renderer::ProjectViewRendererOperation;
use egui_mobius::types::{Enqueue};
use tracing::debug;
use crate::project::{ProjectKey, ProjectUiCommand};
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

    pub fn update(&mut self, event: Event, project_key: ProjectKey) -> Task<(ProjectKey, ProjectUiCommand)> {
        debug!("event: {:?}", event);

        let mut tasks: Vec<Task<(ProjectKey, ProjectUiCommand)>> = Vec::new();

        for effect in self.core.process_event(event) {
            let effect_task = Self::process_effect(&self.core, effect, project_key);
            tasks.push(effect_task);
        }

        Task::batch(tasks)
    }

    pub fn process_effect(core: &Core, effect: Effect, project_key: ProjectKey) -> Task<(ProjectKey, ProjectUiCommand)> {
        debug!("effect: {:?}", effect);

        match effect {
            Effect::Render(_) => {
                let mut view = core.view();
                let task = match view.error.take() {
                    Some(error) => Task::done((project_key, ProjectUiCommand::Error(error))),
                    None => Task::done((project_key, ProjectUiCommand::SetModifiedState(view.modified))),
                };

                task
            }
            Effect::ProjectViewRenderer(request) => {
                let ProjectViewRendererOperation::View {
                    view,
                } = request.operation;
                
                Task::done((project_key, ProjectUiCommand::UpdateView(view)))
            }
        }
    }
}