use std::sync::Arc;
use planner_app::{Effect, Event, Planner};
use planner_app::capabilities::view_renderer::ProjectViewRendererOperation;
use egui_mobius::types::{Enqueue};
use tracing::debug;
use crate::project::{ProjectKey, ProjectUiCommand};

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

    pub fn update(&mut self, event: Event, project_key: ProjectKey, sender: Enqueue<(ProjectKey, ProjectUiCommand)>) {
        debug!("event: {:?}", event);

        for effect in self.core.process_event(event) {
            Self::process_effect(&self.core, effect, project_key, sender.clone());
        }
    }

    pub fn process_effect(core: &Core, effect: Effect, project_key: ProjectKey, sender: Enqueue<(ProjectKey, ProjectUiCommand)>) {
        debug!("effect: {:?}", effect);

        match effect {
            Effect::Render(_) => {
                let view = core.view();
                if view.modified {
                    debug!("modified");
                }
            }
            Effect::ProjectViewRenderer(request) => {
                let ProjectViewRendererOperation::View {
                    view,
                } = request.operation;
                
                sender.send((project_key, ProjectUiCommand::UpdateView(view))).expect("sent");
            }
        }
    }
}