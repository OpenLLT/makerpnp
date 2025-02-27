use egui::{Ui, WidgetText};
use egui_mobius::types::{Enqueue, Value};
use serde::{Deserialize, Serialize};
use crate::config::Config;
use crate::tabs::{Tab, TabKey};
use crate::ui_app::app_tabs::home::HomeTab;
use crate::ui_app::app_tabs::project::ProjectTab;
use crate::ui_commands::UiCommand;

pub mod home;
pub mod project;

pub struct TabContext {
    pub config: Value<Config>,
    pub sender: Enqueue<UiCommand>,
}

#[derive(Deserialize, Serialize)]
pub enum TabKind {
    Home(HomeTab),
    Project(ProjectTab),
}

impl Tab for TabKind {
    type Context = TabContext;
    
    fn label(&self) -> WidgetText {
        match self {
            TabKind::Home(tab) => tab.label(),
            TabKind::Project(tab) => tab.label(),
        }
    }

    fn ui(&mut self, ui: &mut Ui, tab_key: &TabKey, context: &mut Self::Context) {
        match self {
            TabKind::Home(tab) => tab.ui(ui, tab_key, context),
            TabKind::Project(tab) => tab.ui(ui, tab_key, context),
        }
    }

    fn on_close(&mut self, tab_key: &TabKey, context: &mut Self::Context) -> bool {
        match self {
            TabKind::Home(tab) => tab.on_close(tab_key, context),
            TabKind::Project(tab) => tab.on_close(tab_key, context),
        }
    }
}
