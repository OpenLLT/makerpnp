use std::sync::Arc;
use tracing::debug;
use planner_app::{Effect, Event, NavigationOperation, Planner, ProjectView};
use planner_app::view_renderer::ViewRendererOperation;
use planner_gui::task::Task;
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
            match view.error.take() {
                Some(error) => Task::done(ProjectMessage::Error(error)),
                None => Task::done(ProjectMessage::None),
            }
        }
        Effect::Navigator(request) => {
            match request.operation {
                NavigationOperation::Navigate { path } => {
                    Task::done(ProjectMessage::Navigate(path))
                }
            }
        }
        
        Effect::ViewRenderer(request) => {
            let ViewRendererOperation::View { view} = request.operation;
            Task::done(ProjectMessage::UpdateView(view))
        }
    }
}

#[derive(Debug)]
pub enum CoreEvent {
    RenderView { view: ProjectView },
    Navigate { path: String },
}