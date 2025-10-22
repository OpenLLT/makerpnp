use std::future::Future;

use crux_core::capability::{CapabilityContext, Operation};
use crux_core::command::NotificationBuilder;
use crux_core::macros::Capability;
use crux_core::{Command, Request};

use crate::PcbView;

#[derive(Capability)]
pub struct PcbViewRenderer<Ev> {
    context: CapabilityContext<PcbViewRendererOperation, Ev>,
}

impl<Ev> PcbViewRenderer<Ev> {
    pub fn new(context: CapabilityContext<PcbViewRendererOperation, Ev>) -> Self {
        Self {
            context,
        }
    }
}
impl<Ev: 'static> PcbViewRenderer<Ev> {
    pub fn view(&self, view: PcbView) {
        self.context.spawn({
            let context = self.context.clone();
            async move {
                run_view(&context, view).await;
            }
        });
    }
}

async fn run_view<Ev: 'static>(context: &CapabilityContext<PcbViewRendererOperation, Ev>, view: PcbView) {
    context
        .notify_shell(PcbViewRendererOperation::View {
            view,
        })
        .await
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub enum PcbViewRendererOperation {
    View { view: PcbView },
}

impl Operation for PcbViewRendererOperation {
    type Output = ();
}

pub fn view_builder<Effect, Event>(view: PcbView) -> NotificationBuilder<Effect, Event, impl Future<Output = ()>>
where
    Effect: From<Request<PcbViewRendererOperation>> + Send + 'static,
    Event: Send + 'static,
{
    Command::notify_shell(PcbViewRendererOperation::View {
        view,
    })
}

pub fn view<Effect, Event>(view: PcbView) -> Command<Effect, Event>
where
    Effect: From<Request<PcbViewRendererOperation>> + Send + 'static,
    Event: Send + 'static,
{
    view_builder(view).into()
}
