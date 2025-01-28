use crux_core::capability::{CapabilityContext, Operation};
use crux_core::macros::Capability;
use thiserror::Error;

use crate::ProjectView;

#[derive(Capability)]
pub struct ViewRenderer<Ev> {
    context: CapabilityContext<ViewRendererOperation, Ev>,
}

impl<Ev> ViewRenderer<Ev> {
    pub fn new(context: CapabilityContext<ViewRendererOperation, Ev>) -> Self {
        Self {
            context,
        }
    }
}
impl<Ev: 'static> ViewRenderer<Ev> {
    pub fn view(&self, view: ProjectView) {
        self.context.spawn({
            let context = self.context.clone();
            async move {
                run_view(&context, view).await;
            }
        });
    }
}

async fn run_view<Ev: 'static>(context: &CapabilityContext<ViewRendererOperation, Ev>, view: ProjectView) {
    context
        .notify_shell(ViewRendererOperation::View {
            view,
        })
        .await
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub enum ViewRendererOperation {
    View { view: ProjectView },
}

impl Operation for ViewRendererOperation {
    type Output = ();
}
