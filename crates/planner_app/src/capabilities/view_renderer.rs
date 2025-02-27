use crux_core::capability::{CapabilityContext, Operation};
use crux_core::{Command, Request};
use crux_core::macros::Capability;

use crate::ProjectView;

#[derive(Capability)]
pub struct ProjectViewRenderer<Ev> {
    context: CapabilityContext<ProjectViewRendererOperation, Ev>,
}

impl<Ev> ProjectViewRenderer<Ev> {
    pub fn new(context: CapabilityContext<ProjectViewRendererOperation, Ev>) -> Self {
        Self {
            context,
        }
    }
}
impl<Ev: 'static> ProjectViewRenderer<Ev> {
    pub fn view(&self, view: ProjectView) {
        self.context.spawn({
            let context = self.context.clone();
            async move {
                run_view(&context, view).await;
            }
        });
    }
}

async fn run_view<Ev: 'static>(context: &CapabilityContext<ProjectViewRendererOperation, Ev>, view: ProjectView) {
    context
        .notify_shell(ProjectViewRendererOperation::View {
            view,
        })
        .await
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub enum ProjectViewRendererOperation {
    View { view: ProjectView },
}

impl Operation for ProjectViewRendererOperation {
    type Output = ();
}

pub fn view<Effect, Event>(view: ProjectView) -> Command<Effect, Event>
where
    Effect: From<Request<ProjectViewRendererOperation>> + Send + 'static,
    Event: Send + 'static,
{
    Command::notify_shell(ProjectViewRendererOperation::View { view })
}
