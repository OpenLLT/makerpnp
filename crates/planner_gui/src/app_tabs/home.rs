use cushy::localization::Localize;
use cushy::reactive::value::Source;
use cushy::widget::{IntoWidgetList, MakeWidget, WidgetInstance};
use planner_gui::action::Action;
use planner_gui::context::Context;
use planner_gui::widgets::tab_bar::{Tab, TabKey};

use crate::config::Config;
use crate::Dynamic;

#[derive(Clone, Debug)]
pub enum HomeTabMessage {
    None,
}

impl Default for HomeTabMessage {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Debug)]
pub enum HomeTabAction {
    None,
}

#[derive(Clone, Default)]
pub struct HomeTab {}

impl Tab<HomeTabMessage, HomeTabAction> for HomeTab {
    fn label(&self, _context: &Dynamic<Context>) -> String {
        "Home".to_string()
    }

    fn modified(&self, _context: &Dynamic<Context>) -> bool {
        false
    }

    fn make_content(&self, context: &Dynamic<Context>, _tab_key: TabKey) -> WidgetInstance {
        context
            .lock()
            .with_context::<Dynamic<Config>, _, _>(|config| {
                let config_guard = config.lock();
                let show_on_startup_value = Dynamic::new(config_guard.show_home_on_startup);
                // FIXME why is an explicit drop required since cushy commit 4501558b0f
                //       without this, the callback panics when `config_binding.lock()`is called in the closure
                drop(config_guard);

                let callback = show_on_startup_value.for_each_cloned({
                    let config_binding = config.clone();

                    move |value| {
                        println!("updating config, show_home_on_startup: {}", value);
                        let mut config_guard = config_binding.lock();
                        config_guard.show_home_on_startup = value;
                    }
                });

                callback.persist();

                let home_label = Localize::new("home-banner")
                    .xxxx_large()
                    .centered()
                    .make_widget();

                let show_on_startup_button = Localize::new("home-checkbox-label-show-on-startup")
                    .into_checkbox(show_on_startup_value)
                    .centered()
                    .make_widget();

                [home_label, show_on_startup_button]
                    .into_rows()
                    // center all the children, not individually
                    .centered()
                    .make_widget()
            })
            .unwrap()
    }

    fn update(
        &mut self,
        _context: &Dynamic<Context>,
        _tab_key: TabKey,
        message: HomeTabMessage,
    ) -> Action<HomeTabAction> {
        match message {
            HomeTabMessage::None => {}
        }
        Action::new(HomeTabAction::None)
    }
}
