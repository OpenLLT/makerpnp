use std::sync::Arc;

use planner_app::effects::pcb_view_renderer::PcbViewRendererOperation;
use planner_app::effects::project_view_renderer::ProjectViewRendererOperation;
use planner_app::{Effect, Event, PcbView, Planner, ProjectView};
use tracing::{error, trace};

type Core = Arc<planner_app::Core<Planner>>;

pub struct PlannerCoreService {
    core: Core,
}

#[derive(Debug, Clone)]
pub enum PlannerAction {
    SetModifiedState {
        project_modified: bool,
        pcbs_modified: bool,
    },
    ProjectView(ProjectView),
    PcbView(PcbView),
}

#[derive(Debug, Clone)]
pub enum PlannerError {
    CoreError((chrono::DateTime<chrono::Utc>, String)),
}

impl PlannerCoreService {
    pub fn new() -> Self {
        Self {
            core: Arc::new(planner_app::Core::new()),
        }
    }

    #[must_use]
    pub fn update(&mut self, event: Event) -> Result<Vec<PlannerAction>, PlannerError> {
        trace!("event: {:?}", event);

        let mut actions: Vec<PlannerAction> = Vec::new();

        for effect in self.core.process_event(event) {
            let action = Self::process_effect(&self.core, effect);
            match action {
                Ok(action) => {
                    actions.push(action);
                }
                Err(error) => return Err(error),
            }
        }

        Ok(actions)
    }

    pub fn process_effect(core: &Core, effect: Effect) -> Result<PlannerAction, PlannerError> {
        trace!("effect: {:?}", effect);

        match effect {
            Effect::Render(_) => {
                let mut view = core.view();
                let task = match view.error.take() {
                    Some(error) => {
                        error!("core error: {:?}", error);
                        Err(PlannerError::CoreError(error))
                    }
                    None => Ok(PlannerAction::SetModifiedState {
                        project_modified: view.project_modified,
                        pcbs_modified: view.pcbs_modified,
                    }),
                };

                task
            }
            Effect::ProjectView(request) => {
                let ProjectViewRendererOperation::View {
                    view,
                } = request.operation;

                Ok(PlannerAction::ProjectView(view))
            }
            Effect::PcbView(request) => {
                let PcbViewRendererOperation::View {
                    view,
                } = request.operation;

                Ok(PlannerAction::PcbView(view))
            }
        }
    }
}
