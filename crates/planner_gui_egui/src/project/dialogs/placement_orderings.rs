use egui::{Modal, Ui};
use egui_i18n::tr;
use egui_mobius::Value;
use egui_taffy::taffy::prelude::{auto, length, percent};
use egui_taffy::taffy::{AlignContent, AlignItems, Display, FlexDirection, Size, Style};
use egui_taffy::{Tui, TuiBuilderLogic, tui};
use planner_app::{PlacementSortingItem, Reference};
use validator::Validate;

use crate::forms::Form;
use crate::forms::transforms::no_transform;
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Debug)]
pub struct PlacementOrderingsModal {
    phase_reference: Reference,

    fields: Value<PlacementOrderingFields>,

    pub component: ComponentState<PlacementOrderingsModalUiCommand>,
}

impl PlacementOrderingsModal {
    pub fn new(phase_reference: Reference) -> Self {
        Self {
            phase_reference,
            fields: Value::default(),
            component: ComponentState::default(),
        }
    }

    fn show_form(&self, ui: &mut Ui, form: &Form<PlacementOrderingFields, PlacementOrderingsModalUiCommand>) {
        let default_style = || Style {
            padding: length(2.),
            gap: length(2.),
            ..Default::default()
        };

        tui(ui, ui.id().with("placement_orderings_form"))
            .reserve_available_width()
            .style(Style {
                align_items: Some(AlignItems::Center),
                flex_direction: FlexDirection::Column,
                size: Size {
                    width: percent(1.),
                    height: auto(),
                },
                padding: length(8.),
                gap: length(8.),
                ..default_style()
            })
            .show(|tui| {
                form.show_fields(tui, |form, tui| {
                    form.add_field_tui(
                        "orderings",
                        tr!("form-phase-placement-orderings-input-orderings"),
                        tui,
                        {
                            move |tui: &mut Tui, fields, sender| {
                                tui.style(Style {
                                    display: Display::Flex,
                                    align_content: Some(AlignContent::Stretch),
                                    flex_grow: 1.0,
                                    ..default_style()
                                })
                                .add(|tui| {
                                    for column_index in 0..3 {
                                        tui.style(Style {
                                            flex_grow: 1.0,
                                            ..default_style()
                                        })
                                        .with_border_style_from_egui_style()
                                        .add_with_border(|tui: &mut Tui| {
                                            tui.ui_add_manual(|ui| ui.label(column_index.to_string()), no_transform);
                                            // end of cell
                                        })
                                        // end of column
                                    }
                                    // end of row
                                });

                                // end of field
                            }
                        },
                    );
                    // end of fields
                });
                // end of form
            });
    }
}

#[derive(Clone, Debug, Default, Validate, serde::Deserialize, serde::Serialize)]
pub struct PlacementOrderingFields {}

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

            let form = Form::new(&self.fields, &self.component.sender, ());

            self.show_form(ui, &form);

            let is_form_valid = form.is_valid();

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
