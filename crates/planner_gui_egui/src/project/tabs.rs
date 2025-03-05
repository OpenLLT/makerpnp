use egui::Ui;
use egui_dock::{DockArea, DockState, Style};
use egui_mobius::types::Value;
use tracing::debug;
use crate::project::{ProjectKey, ProjectTabKind, ProjectUiState};
use crate::tabs::{AppTabViewer, TabKey, Tabs};
use crate::tabs_impl;
use crate::ui_component::{ComponentState, UiComponent};

#[derive(serde::Deserialize, serde::Serialize)]
pub struct ProjectTabs {
    tabs: Value<Tabs<ProjectTabKind, ProjectTabContext>>,
    tree: Value<DockState<TabKey>>,

    #[serde(skip)]
    pub component: ComponentState<ProjectTabUiCommand>,
}

impl Default for ProjectTabs {
    fn default() -> Self {
        Self {
            tabs: Value::new(Tabs::new()),
            tree: Value::new(DockState::new(vec![])),
            component: ComponentState::default(),
        }
    }
}

// Not to be confused with the other one...
pub struct ProjectTabContext {
    pub key: ProjectKey,
    pub state: Value<ProjectUiState>,
}

impl ProjectTabs {
    tabs_impl!(ProjectTabKind, ProjectTabContext);
}

#[derive(Debug, Clone)]
pub enum ProjectTabUiCommand {
    None,
}

pub enum ProjectTabAction {
    None,
}

impl UiComponent for ProjectTabs {
    type UiContext<'context> = ProjectTabContext;
    type UiCommand = ProjectTabUiCommand;
    type UiAction = ProjectTabAction;

    fn ui<'context>(&self, ui: &mut Ui, context: &mut Self::UiContext<'context>) {
        let ctx = ui.ctx();

        let mut tab_viewer = AppTabViewer {
            tabs: self.tabs.clone(),
            context,
        };

        let mut tree = self.tree.lock().unwrap();

        DockArea::new(&mut tree)
            .id(ui.id().with("project-tabs"))
            .style(Style::from_egui(ctx.style().as_ref()))
            .show_inside(ui, &mut tab_viewer);
    }

    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        debug!("project tab. command: {:?}", command);
        match command {
            ProjectTabUiCommand::None => Some(ProjectTabAction::None),
        }
    }
}
