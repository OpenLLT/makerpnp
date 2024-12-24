use std::str::FromStr;
use crux_core::App;
use crux_core::macros::Effect;
use crux_core::render::Render;
use serde_with::serde_as;

pub use crux_core::Core;
use thiserror::Error;

extern crate serde_regex;

#[derive(Default)]
pub struct VariantBuilder;

#[derive(Default)]
pub struct Model {
    error: Option<String>
}

#[derive(serde::Serialize, serde::Deserialize, Default, PartialEq, Debug)]
pub struct OperationViewModel {
    pub error: Option<String>
}

#[derive(Effect)]
pub struct Capabilities {
    render: Render<Event>,
}

#[serde_as]
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub enum Event {
    None,

    //
    // Views
    //
}

impl App for VariantBuilder {
    type Event = Event;
    type Model = Model;
    type ViewModel = OperationViewModel;
    type Capabilities = Capabilities;

    fn update(&self, event: Self::Event, model: &mut Self::Model, caps: &Self::Capabilities) {
        let mut default_render = true;
        match event {
            Event::None => {}
        }

        if default_render {
            // This causes the shell to request the view, via `view()`
            caps.render.render();
        }
    }

    fn view(&self, model: &Self::Model) -> Self::ViewModel {
        OperationViewModel {
            error: model.error.clone(),
        }
    }
}

#[derive(Error, Debug)]
enum AppError {
    #[error("Operation error, cause: {0}")]
    OperationError(anyhow::Error),
}

#[cfg(test)]
mod app_tests {
    use super::*;
    use crux_core::{assert_effect, testing::AppTester};

    #[test]
    fn minimal() {
        let hello = AppTester::<VariantBuilder, _>::default();
        let mut model = Model::default();

        // Call 'update' and request effects
        let update = hello.update(Event::None, &mut model);

        // Check update asked us to `Render`
        assert_effect!(update, Effect::Render(_));

        // Make sure the view matches our expectations
        let actual_view = &hello.view(&model);
        let expected_view = OperationViewModel::default();
        assert_eq!(actual_view, &expected_view);
    }
}
