use egui::Modal;
use egui_i18n::tr;
use planner_app::DesignName;

use crate::ui_component::{ComponentState, UiComponent};

#[derive(Debug)]
pub struct ManageGerbersModal {
    design_index: usize,
    design_name: DesignName,
    pub component: ComponentState<ManagerGerbersModalUiCommand>,
}

impl ManageGerbersModal {
    pub fn new(design_index: usize, design_name: DesignName) -> Self {
        Self {
            design_index,
            design_name,
            component: Default::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ManagerGerbersModalUiCommand {
    Close,
}

#[derive(Debug, Clone)]
pub enum ManagerGerberModalAction {
    CloseDialog,
}

impl UiComponent for ManageGerbersModal {
    type UiContext<'context> = ();
    type UiCommand = ManagerGerbersModalUiCommand;
    type UiAction = ManagerGerberModalAction;

    fn ui<'context>(&self, ui: &mut egui::Ui, _context: &mut Self::UiContext<'context>) {
        ui.ctx().style_mut(|style| {
            // if this is not done, text in labels/checkboxes/etc wraps
            style.wrap_mode = Some(egui::TextWrapMode::Extend);
        });

        let modal_id = ui.id().with("manage_gerbers_modal");

        Modal::new(modal_id).show(ui.ctx(), |ui| {
            ui.set_width(ui.available_width() * 0.8);

            ui.heading(tr!("modal-manager-gerbers-title", { design: self.design_name.to_string() }));

            egui::Sides::new().show(
                ui,
                |_ui| {},
                |ui| {
                    if ui
                        .button(tr!("form-button-close"))
                        .clicked()
                    {
                        self.component
                            .send(ManagerGerbersModalUiCommand::Close);
                    }
                },
            );
        });
    }

    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            ManagerGerbersModalUiCommand::Close => Some(ManagerGerberModalAction::CloseDialog),
        }
    }
}
