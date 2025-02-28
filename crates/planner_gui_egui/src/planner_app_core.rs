use std::sync::Arc;
use planner_app::{Effect, Event, Planner, ProjectView};
use planner_app::capabilities::view_renderer::ProjectViewRendererOperation;
use egui_mobius::types::{Enqueue, Value};
use tracing::debug;
use crate::project::ProjectUiState;
use crate::ui_commands::UiCommand;

type Core = Arc<planner_app::Core<Planner>>;

pub struct PlannerCoreService {
    core: Core,
    ui_state: Value<ProjectUiState>,
}

impl PlannerCoreService {
    pub fn new(ui_state: Value<ProjectUiState>) -> Self {
        Self {
            core: Arc::new(planner_app::Core::new()),
            ui_state,
        }
    }

    pub fn update(&mut self, event: Event, sender: Enqueue<UiCommand>) {
        debug!("event: {:?}", event);

        for effect in self.core.process_event(event) {
            Self::process_effect(&self.core, effect, sender.clone(), self.ui_state.clone());
        }
    }

    pub fn process_effect(core: &Core, effect: Effect, _sender: Enqueue<UiCommand>, ui_state: Value<ProjectUiState>) {
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

                let mut project_ui_state = ui_state.lock().unwrap();
                match view {
                    ProjectView::Overview(project_overview) => {
                        // todo - update ui state somehow...
                    }
                    _ => todo!()
                }

            }
        }
    }
}