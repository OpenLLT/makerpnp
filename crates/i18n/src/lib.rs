use std::fs;
use std::path::PathBuf;

use tracing::{debug, error, info};

pub struct I18nConfig {
    pub default: String,
    pub fallback: String,
    pub languages: Vec<String>,
}

pub fn init(config: I18nConfig) {
    for identifier in config.languages {
        let mut path = PathBuf::from("assets/translations");
        path.push(identifier.clone());
        path.push("translations.ftl");
        debug!(
            "Loading translations. identifier: {}, absolute_path: {:?}",
            identifier,
            std::path::absolute(path.clone()).unwrap()
        );
        match fs::read_to_string(path.clone()) {
            Ok(content) => match egui_i18n::load_translations_from_text(identifier, content) {
                Err(e) => error!("Error parsing translation file: {}, cause: {}", path.display(), e),
                Ok(_) => info!("Loaded translations. file: {}", path.display()),
            },
            Err(e) => {
                error!("Error reading translation file: {}, cause: {}", path.display(), e);
            }
        }
    }

    egui_i18n::set_language(&config.default);
    egui_i18n::set_fallback(&config.fallback);

    debug!("languages: {:?}", egui_i18n::languages());
}

pub mod fluent_argument_helpers {

    #[cfg(feature = "json")]
    pub mod json {
        use std::borrow::Cow;
        use std::collections::HashMap;

        use egui_i18n::fluent_bundle::types::{FluentNumber, FluentNumberOptions};
        use egui_i18n::fluent_bundle::{FluentArgs, FluentValue};
        use serde_json::Value;

        pub fn build_fluent_args<'a>(params: &'a HashMap<Cow<'_, str>, Value>) -> FluentArgs<'a> {
            let mut args = FluentArgs::new();
            for (key, value) in params.iter() {
                match value {
                    Value::Null => {
                        // this occurs for an `Option` with a value of `None`
                    }
                    Value::Bool(_) => todo!(),
                    Value::Number(number) => {
                        // TODO make sure this is correct!  perhaps write some integration tests to prove the conversion is correct.
                        if number.is_f64() {
                            let value = FluentValue::Number(FluentNumber::new(
                                number.as_f64().unwrap(),
                                FluentNumberOptions::default(),
                            ));
                            args.set(key.to_string(), value);
                        } else if number.is_i64() {
                            let value = FluentValue::Number(FluentNumber::new(
                                number.as_i64().unwrap() as f64,
                                FluentNumberOptions::default(),
                            ));
                            args.set(key.to_string(), value);
                        } else if number.is_u64() {
                            let value = FluentValue::Number(FluentNumber::new(
                                number.as_u64().unwrap() as f64,
                                FluentNumberOptions::default(),
                            ));
                            args.set(key.to_string(), value);
                        } else {
                            unreachable!()
                        }
                    }
                    Value::String(string) => {
                        let value = FluentValue::String(string.into());
                        args.set(key.to_string(), value);
                    }
                    Value::Array(array) => {
                        // FIXME for now, just convert to a string, probably needs better handling but need the use-case first!
                        let string = format!("{:?}", array);

                        let value = FluentValue::String(string.into());
                        args.set(key.to_string(), value);
                    }
                    Value::Object(_) => todo!(),
                }
            }
            args
        }
    }

    #[cfg(feature = "planner_app")]
    pub mod planner_app {
        use std::collections::HashMap;

        use egui_i18n::fluent_bundle::{FluentArgs, FluentValue};
        use planner_app::Arg;

        pub fn build_fluent_args(args: &HashMap<String, Arg>) -> FluentArgs<'_> {
            let mut fluent_args = FluentArgs::new();
            for (key, value) in args.iter() {
                match value {
                    Arg::String(value) => {
                        fluent_args.set(key, FluentValue::String(value.into()));
                    }
                    Arg::Integer(value) => {
                        fluent_args.set(key, FluentValue::Number(value.into()));
                    }
                    Arg::Boolean(value) => {
                        let number: u8 = *value as u8;
                        fluent_args.set(key, FluentValue::Number(number.into()));
                    }
                }
            }
            fluent_args
        }
    }
}
