use crux_core::capability::{CapabilityContext, Operation};
use crux_core::macros::Capability;
use thiserror::Error;

#[derive(Capability)]
pub struct Navigator<Ev> {
    context: CapabilityContext<NavigationOperation, Ev>,
}

impl<Ev> Navigator<Ev> {
    pub fn new(context: CapabilityContext<NavigationOperation, Ev>) -> Self {
        Self {
            context,
        }
    }
}
impl<Ev: 'static> Navigator<Ev> {
    pub fn navigate<F>(&self, path: String, make_event: F)
    where
        F: FnOnce(Result<Option<String>, NavigationError>) -> Ev + Send + Sync + 'static,
    {
        self.context.spawn({
            let context = self.context.clone();
            async move {
                let response = navigate(&context, path).await;
                context.update_app(make_event(response))
            }
        });
    }
}

async fn navigate<Ev: 'static>(
    context: &CapabilityContext<NavigationOperation, Ev>,
    path: String,
) -> Result<Option<String>, NavigationError> {
    context
        .request_from_shell(NavigationOperation::Navigate {
            path,
        })
        .await
        .unwrap_set()
}

#[derive(Clone, serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq)]
pub enum NavigationResult {
    Ok { response: NavigationResponse },
    Err { error: NavigationError },
}

#[derive(Clone, serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq)]
pub enum NavigationResponse {
    Navigate { previous: String },
}

impl NavigationResult {
    fn unwrap_set(self) -> Result<Option<String>, NavigationError> {
        match self {
            NavigationResult::Ok {
                response,
            } => match response {
                NavigationResponse::Navigate {
                    previous,
                } => Ok(previous.into()),
                // _ => {
                //     panic!("attempt to convert NavigationResponse other than Ok to Option<String>")
                // }
            },
            NavigationResult::Err {
                error,
            } => Err(error.clone()),
        }
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq)]
pub enum NavigationOperation {
    Navigate { path: String },
}

impl Operation for NavigationOperation {
    type Output = NavigationResult;
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Error)]
#[serde(rename_all = "camelCase")]
pub enum NavigationError {
    #[error("other error: {message}")]
    Other { message: String },
}
