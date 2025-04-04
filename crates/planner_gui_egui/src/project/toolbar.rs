use egui::Ui;
use egui_i18n::tr;

use crate::ui_component::{ComponentState, UiComponent};

#[derive(Debug, Clone)]
pub enum ProjectToolbarUiCommand {
    ProjectExplorerClicked,
    AddPcbClicked,
    AddPhaseClicked,
    CreateUnitAssignmentClicked,
    RefreshFromDesignVariantsClicked,
    GenerateArtifactsClicked,
    RemoveUnknownPlacements,
}

pub enum ProjectToolbarAction {
    ShowProjectExplorer,
    ShowAddPcbDialog,
    ShowAddPhaseDialog,
    ShowCreateUnitAssignmentDialog,
    RefreshFromDesignVariants,
    GenerateArtifacts,
    RemoveUnknownPlacements,
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
                .button(tr!("project-toolbar-button-generate-artifacts"))
                .clicked()
            {
                self.component
                    .send(ProjectToolbarUiCommand::GenerateArtifactsClicked)
            }
            if ui
                .button(tr!("project-toolbar-button-refresh-from-variants"))
                .clicked()
            {
                self.component
                    .send(ProjectToolbarUiCommand::RefreshFromDesignVariantsClicked)
            }
            if ui
                .button(tr!("project-toolbar-button-remove-unknown-placements"))
                .clicked()
            {
                self.component
                    .send(ProjectToolbarUiCommand::RemoveUnknownPlacements)
            }
            if ui
                .button(tr!("project-toolbar-button-add-pcb"))
                .clicked()
            {
                self.component
                    .send(ProjectToolbarUiCommand::AddPcbClicked)
            }
            if ui
                .button(tr!("project-toolbar-button-add-phase"))
                .clicked()
            {
                self.component
                    .send(ProjectToolbarUiCommand::AddPhaseClicked)
            }
            if ui
                .button(tr!("project-toolbar-button-create-unit-assignment"))
                .clicked()
            {
                self.component
                    .send(ProjectToolbarUiCommand::CreateUnitAssignmentClicked)
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
            ProjectToolbarUiCommand::RefreshFromDesignVariantsClicked => {
                Some(ProjectToolbarAction::RefreshFromDesignVariants)
            }
            ProjectToolbarUiCommand::AddPcbClicked => Some(ProjectToolbarAction::ShowAddPcbDialog),
            ProjectToolbarUiCommand::AddPhaseClicked => Some(ProjectToolbarAction::ShowAddPhaseDialog),
            ProjectToolbarUiCommand::CreateUnitAssignmentClicked => {
                Some(ProjectToolbarAction::ShowCreateUnitAssignmentDialog)
            }
            ProjectToolbarUiCommand::GenerateArtifactsClicked => Some(ProjectToolbarAction::GenerateArtifacts),
            ProjectToolbarUiCommand::RemoveUnknownPlacements => Some(ProjectToolbarAction::RemoveUnknownPlacements),
        }
    }
}
