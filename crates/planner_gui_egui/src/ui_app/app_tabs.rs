use egui::{Ui, WidgetText};
use serde::{Deserialize, Serialize};
use crate::config::Config;
use crate::tabs::{Tab, TabKey};
use crate::ui_app::app_tabs::home::HomeTab;
use crate::ui_app::app_tabs::project::ProjectTab;

pub mod home;
pub mod project;

pub struct TabContext<'a> {
    pub config: &'a mut Config,
}

#[derive(Deserialize, Serialize)]
pub enum TabKind {
    Home(HomeTab),
    Project(ProjectTab),
}

impl Tab for TabKind {
    type Context<'a> = TabContext<'a>;

    fn label(&self) -> WidgetText {
        match self {
            TabKind::Home(tab) => tab.label(),
            TabKind::Project(tab) => tab.label(),
        }
    }

    fn ui(&mut self, ui: &mut Ui, tab_key: &mut TabKey, context: &mut Self::Context<'_>) {
        match self {
            TabKind::Home(tab) => tab.ui(ui, tab_key, context),
            TabKind::Project(tab) => tab.ui(ui, tab_key, context),
        }
    }

    fn on_close(&mut self, tab_key: &mut TabKey, context: &mut Self::Context<'_>) -> bool {
        match self {
            TabKind::Home(tab) => tab.on_close(tab_key, context),
            TabKind::Project(tab) => tab.on_close(tab_key, context),
        }
    }
}
