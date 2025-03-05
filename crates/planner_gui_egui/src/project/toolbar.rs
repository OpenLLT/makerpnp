use egui::Ui;
use egui_i18n::tr;

use crate::ui_component::{ComponentState, UiComponent};

#[derive(Debug, Clone)]
pub enum ProjectToolbarUiCommand {
    ProjectExplorerClicked,
    AddPcbClicked,
}

pub enum ProjectToolbarAction {
    ShowProjectExplorer,
    ShowAddPcbDialog,
}

#[derive(Default)]
pub struct ProjectToolbar {
    pub component: ComponentState<ProjectToolbarUiCommand>,
}

impl ProjectToolbar {}

impl UiComponent for ProjectToolbar {
    type UiContext<'context> = ();
    type UiCommand = ProjectToolbarUiCommand;
    type UiAction = ProjectToolbarAction;

    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        ui.horizontal(|ui| {
            if ui
                .button(tr!("project-toolbar-button-show-explorer"))
                .clicked()
            {
                self.component
                    .send(ProjectToolbarUiCommand::ProjectExplorerClicked)
            }

            if ui
                .button(tr!("project-toolbar-button-add-pcb"))
                .clicked()
            {
                self.component
                    .send(ProjectToolbarUiCommand::AddPcbClicked)
            }
        });
    }

    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            ProjectToolbarUiCommand::ProjectExplorerClicked => Some(ProjectToolbarAction::ShowProjectExplorer),
            ProjectToolbarUiCommand::AddPcbClicked => Some(ProjectToolbarAction::ShowAddPcbDialog),
        }
    }
}
