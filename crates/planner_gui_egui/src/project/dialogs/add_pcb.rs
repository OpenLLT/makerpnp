use i18n::fluent_argument_helpers::json::build_fluent_args;
use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use egui::{Modal, Response, RichText, TextEdit, Ui, Widget};
use egui_i18n::{tr, translate_fluent};
use egui_mobius::types::{Value, ValueGuard};
use egui_taffy::taffy::prelude::{auto, fit_content, fr, length, percent, span};
use egui_taffy::taffy::{AlignItems, AlignSelf, Display, FlexDirection, Style};
use egui_taffy::{taffy, tui, Tui, TuiBuilderLogic, TuiContainerResponse};
use taffy::Size;
use tracing::trace;
use validator::{Validate, ValidationErrors};
use crate::project::dialogs::PcbKindChoice;
use crate::project::ProjectKey;
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Debug)]
pub struct AddPcbModal {
    fields: Value<AddPcbFields>,
    
    path: PathBuf,
    key: ProjectKey,
    
    pub component: ComponentState<AddPcbModalUiCommand>
}

impl AddPcbModal {
    pub fn new(path: PathBuf, key: ProjectKey) -> Self {
        Self {
            fields: Default::default(),
            path,
            key,
            component: Default::default(),
        }
    }

    fn show_form(&self, ui: &mut Ui) {
        let validation_errors = {
            let fields = self.fields.lock().unwrap();

            fields.validate()
        };


        let default_style = || Style {
            padding: length(2.),
            gap: length(2.),
            ..Default::default()
        };

        let no_padding_style = || Style {
            padding: length(0.),
            gap: length(0.),
            ..Default::default()
        };

        tui(ui, ui.id().with("new"))
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
                //
                // form fields container
                //
                tui.style(Style {
                    flex_direction: FlexDirection::Row,
                    align_self: Some(AlignSelf::Stretch),
                    ..default_style()
                })
                    .add(|tui| {
                        //
                        // grid container
                        //
                        tui.style(Style {
                            flex_grow: 1.0,
                            display: Display::Grid,
                            grid_template_columns: vec![fit_content(percent(1.)), fr(1.)],
                            grid_template_rows: vec![fr(1.), fr(1.)],

                            // ensure items are centered vertically on rows
                            align_items: Some(AlignItems::Center),
                            ..default_style()
                        })
                            .add(|tui| {
                                form_field("name", tr!("form-add-pcb-input-name"), self.fields.clone(), self.component.sender.clone(), &validation_errors, tui, {
                                    // NOTE text input does not resize with grid cell when using `.ui_add`, known issue - https://discord.com/channels/900275882684477440/904461220592119849/1338883750922293319
                                    //      as a workaround we use `ui_add_manual` for now, with `no_transform`.
                                    move |ui: &mut Ui, fields, sender|{
                                        let mut name_clone = fields.name.clone();
                                        let output = TextEdit::singleline(&mut name_clone)
                                            .desired_width(ui.available_width())
                                            .show(ui);
                                        
                                        if !fields.name.eq(&name_clone) {
                                            sender.send(AddPcbModalUiCommand::NameChanged(name_clone)).expect("sent")
                                        }
                                        
                                        output.response
                                    }
                                });
                                
                                form_field("pcb_kind", tr!("form-common-choice-pcb-kind"), self.fields.clone(), self.component.sender.clone(), &validation_errors, tui, {
                                    move |ui: &mut Ui, fields, sender |{
                                        let kind = fields.kind.clone();
                                        
                                        let available_size = ui.available_size();

                                        ui.add_sized(available_size, |ui: &mut Ui| {
                                            let kind_id = ui.id();
                                            egui::ComboBox::from_id_salt(kind_id)
                                                .width(ui.available_width())
                                                .selected_text(match kind {
                                                    None => tr!("form-common-combo-default"),
                                                    Some(PcbKindChoice::Single) => tr!("form-common-choice-pcb-kind-single"),
                                                    Some(PcbKindChoice::Panel) => {
                                                        tr!("form-common-choice-pcb-kind-panel")
                                                    }
                                                })
                                                .show_ui(ui, |ui| {
                                                    if ui
                                                        .add(egui::SelectableLabel::new(
                                                            kind == Some(PcbKindChoice::Single),
                                                            tr!("form-common-choice-pcb-kind-single"),
                                                        ))
                                                        .clicked()
                                                    {
                                                        sender.send(AddPcbModalUiCommand::PcbKindChanged(PcbKindChoice::Single)).expect("sent");
                                                    }
                                                    if ui
                                                        .add(egui::SelectableLabel::new(
                                                            kind == Some(PcbKindChoice::Panel),
                                                            tr!("form-common-choice-pcb-kind-panel"),
                                                        ))
                                                        .clicked()
                                                    {
                                                        sender.send(AddPcbModalUiCommand::PcbKindChanged(PcbKindChoice::Panel)).expect("sent");
                                                    }
                                                })
                                                .response
                                        })
                                    }
                                });
                                // end of grid container content
                            });

                        // end of form fields container content
                    });
            });
    }
}

#[derive(Clone, Debug, Default, Validate, serde::Deserialize, serde::Serialize)]
pub struct AddPcbFields {
    #[validate(length(min = 1, code = "form-input-error-length"))]
    name: String,
    #[validate(required(code = "form-option-error-required"))]
    kind: Option<PcbKindChoice>,
}

#[derive(Debug, Clone)]
pub enum AddPcbModalUiCommand {
    Submit,
    NameChanged(String),
    PcbKindChanged(PcbKindChoice),
}

#[derive(Debug, Clone)]
pub enum AddPcbModalAction {
    CloseDialog,
}

impl UiComponent for AddPcbModal {
    type UiContext<'context> = ();
    type UiCommand = AddPcbModalUiCommand;
    type UiAction = AddPcbModalAction;

    fn ui<'context>(&self, ui: &mut egui::Ui, _context: &mut Self::UiContext<'context>) {
        let modal_id = ui.id().with("add_pcb_modal");

        Modal::new(modal_id).show(ui.ctx(), |ui| {
            ui.set_width(ui.available_width() * 0.8);

            let file_name = self
                .path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap();
            ui.heading(tr!("modal-add-pcb-title", {file: file_name}));

            self.show_form(ui);

            egui::Sides::new().show(
                ui,
                |_ui| {},
                |ui| {
                    if ui
                        .button(tr!("form-button-ok"))
                        .clicked()
                    {
                        self.component
                            .send(AddPcbModalUiCommand::Submit);
                    }
                },
            );

        });
    }

    fn update<'context>(&mut self, command: Self::UiCommand, _context: &mut Self::UiContext<'context>) -> Option<Self::UiAction> {
        match command {
            AddPcbModalUiCommand::Submit => {
                // todo validation, etc...
                Some(AddPcbModalAction::CloseDialog)
            }
            AddPcbModalUiCommand::NameChanged(name) => {
                self.fields.lock().unwrap().name = name;
                None
            }
            AddPcbModalUiCommand::PcbKindChanged(kind) => {
                self.fields.lock().unwrap().kind = Some(kind);
                None
            }
        }
    }
}

// TODO move these functions somewhere more appropriate on second use.

pub fn form_default_style() -> fn() -> Style {
    let default_style = || Style {
        padding: length(2.),
        gap: length(2.),
        ..Default::default()
    };

    default_style
}

pub fn form_field<T, C>(
    field_name: &str, 
    label: String, 
    fields: Value<T>,
    sender: Sender<C>,
    validation_errors: &Result<(), ValidationErrors>, 
    tui: &mut Tui, 
    mut ui_builder: impl FnMut(&mut Ui, ValueGuard<'_, T>, Sender<C>) -> Response) 
{
    let default_style = form_default_style();

    tui.style(Style { ..default_style() }).add(|tui| {
        tui.label(label);
    });

    tui.style(Style {
        flex_grow: 1.0,
        ..default_style()
    })
        .add(|tui| {
            tui.style(Style {
                flex_grow: 1.0,
                ..default_style()
            })
                .ui_add_manual(
                    | ui | {
                        ui_builder(ui, fields.lock().unwrap(), sender)
                    },
                    no_transform,
                );
        });

    field_error(validation_errors, default_style, tui, field_name);
}

fn no_transform(value: TuiContainerResponse<Response>, _ui: &Ui) -> TuiContainerResponse<Response> {
    value
}

fn field_error(
    validation_errors: &Result<(), ValidationErrors>,
    default_style: fn() -> Style,
    tui: &mut Tui,
    field_name: &str,
) {
    if let Err(errors) = validation_errors {
        let errs = errors.field_errors();
        if let Some(field_errors) = errs.get(field_name) {
            tui.style(Style {
                grid_column: span(2),
                ..default_style()
            })
                .add(|tui| {
                    for field_error in field_errors.iter() {
                        let code = &field_error.code;
                        let params = &field_error.params;

                        let args = build_fluent_args(params);

                        let message = translate_fluent(code, &args);

                        trace!("field_error: {}", field_error);

                        tui.label(RichText::new(message).color(colors::ERROR));
                    }
                });
        }
    }
}

mod colors {
    use egui::Color32;

    pub const ERROR: Color32 = Color32::from_rgb(0xcb, 0x63, 0x5d);
}
