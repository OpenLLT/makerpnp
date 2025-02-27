use std::sync::Arc;

use planner_app::capabilities::view_renderer::ProjectViewRendererOperation;
use planner_app::{Effect, Event, Planner};
use planner_gui_cushy::task::Task;
use tracing::debug;

use crate::project::ProjectMessage;

type Core = Arc<planner_app::Core<Effect, Planner>>;

pub struct CoreService {
    core: Core,
}

impl CoreService {
    pub fn new() -> Self {
        debug!("initializing core service");
        Self {
            core: Arc::new(planner_app::Core::new()),
        }
    }

    pub fn update(&self, event: Event) -> Task<ProjectMessage> {
        debug!("event: {:?}", event);

        let mut tasks: Vec<Task<ProjectMessage>> = Vec::new();

        for effect in self.core.process_event(event) {
            let effect_task = process_effect(&self.core, effect);
            tasks.push(effect_task);
        }

        Task::batch(tasks)
    }
}

fn process_effect(core: &Core, effect: Effect) -> Task<ProjectMessage> {
    debug!("core::process_effect. effect: {:?}", effect);
    match effect {
        ref _render @ Effect::Render(ref _request) => {
            let mut view = core.view();
            let task = match view.error.take() {
                Some(error) => Task::done(ProjectMessage::Error(error)),
                None => Task::done(ProjectMessage::SetModifiedState(view.modified)),
            };

            task
        }
        Effect::ProjectViewRenderer(request) => {
            let ProjectViewRendererOperation::View {
                view,
            } = request.operation;
            Task::done(ProjectMessage::UpdateView(view))
        }
    }
}
