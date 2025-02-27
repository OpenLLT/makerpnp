use std::path::PathBuf;
use egui::{Ui, WidgetText};
use serde::{Deserialize, Serialize};
use tracing::debug;
use crate::project::ProjectKey;
use crate::tabs::{Tab, TabKey};
use crate::ui_app::app_tabs::TabContext;
use crate::ui_commands::UiCommand;

/// This is persisted between application restarts
#[derive(Clone, Default, Debug, Deserialize, Serialize)]
pub struct ProjectTab {
    pub project_key: ProjectKey,
    
    // path is required here so the project can be loaded when the application restarts
    pub path: PathBuf,
    pub label: String,
    pub modified: bool,
}

impl ProjectTab {
    pub fn new(label: String, path: PathBuf, project_key: ProjectKey) -> Self {
        debug!("Creating project tab. key: {:?}, path: {}", &project_key, &path.display());
        Self {
            project_key,
            path,
            label,
            modified: false,
        }
    }
}

impl Tab for ProjectTab {
    type Context = TabContext;

    fn label(&self) -> WidgetText {
        egui::widget_text::WidgetText::from(self.label.clone())
    }

    fn ui(&mut self, ui: &mut Ui, _tab_key: &TabKey, _tab_context: &mut Self::Context) {
        ui.label("project tab");
    }

    fn on_close(&mut self, _tab_key: &TabKey, tab_context: &mut Self::Context) -> bool {
        tab_context.sender.send(UiCommand::ProjectClosed(self.project_key)).ok();

        true
    }
}
