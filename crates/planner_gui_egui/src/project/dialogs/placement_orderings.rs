use egui::{Modal, Ui};
use egui_i18n::tr;
use planner_app::{PlacementSortingItem, Reference};

use crate::ui_component::{ComponentState, UiComponent};

#[derive(Debug)]
pub struct PlacementOrderingsModal {
    phase_reference: Reference,

    pub component: ComponentState<PlacementOrderingsModalUiCommand>,
}

impl PlacementOrderingsModal {
    pub fn new(phase_reference: Reference) -> Self {
        Self {
            phase_reference,
            component: ComponentState::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum PlacementOrderingsModalUiCommand {
    Submit,
    Cancel,
}

#[derive(Debug, Clone)]
pub enum PlacementOrderingsModalAction {
    Submit(PlacementOrderingsArgs),
    CloseDialog,
}

/// Value object
#[derive(Debug, Clone)]
pub struct PlacementOrderingsArgs {
    pub orderings: Vec<PlacementSortingItem>,
}

impl UiComponent for PlacementOrderingsModal {
    type UiContext<'context> = ();
    type UiCommand = PlacementOrderingsModalUiCommand;
    type UiAction = PlacementOrderingsModalAction;

    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        ui.ctx().style_mut(|style| {
            // if this is not done, text in labels/checkboxes/etc wraps
            style.wrap_mode = Some(egui::TextWrapMode::Extend);
        });

        let modal_id = ui
            .id()
            .with("phase_placement_orderings_modal");

        Modal::new(modal_id).show(ui.ctx(), |ui| {
            ui.set_width(ui.available_width() * 0.8);

            ui.heading(tr!("modal-phase-placement-orderings-title", { phase: self.phase_reference.to_string() }));

            ui.label("<TODO: FORM HERE>");

            // TODO use a form here
            let is_form_valid = true;

            egui::Sides::new().show(
                ui,
                |_ui| {},
                |ui| {
                    if ui
                        .button(tr!("form-button-cancel"))
                        .clicked()
                    {
                        self.component
                            .send(PlacementOrderingsModalUiCommand::Cancel);
                    }
                    if ui
                        .button(tr!("form-button-ok"))
                        .clicked()
                        && is_form_valid
                    {
                        self.component
                            .send(PlacementOrderingsModalUiCommand::Cancel);
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
            PlacementOrderingsModalUiCommand::Submit => {
                let args = PlacementOrderingsArgs {
                    // TODO build the orderings from the UI
                    orderings: vec![],
                };
                Some(PlacementOrderingsModalAction::Submit(args))
            }
            PlacementOrderingsModalUiCommand::Cancel => Some(PlacementOrderingsModalAction::CloseDialog),
        }
    }
}
