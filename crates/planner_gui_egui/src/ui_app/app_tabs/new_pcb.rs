use std::path::PathBuf;

use egui::{Button, TextEdit, Ui, Widget, WidgetText};
use egui_i18n::tr;
use egui_mobius::types::Value;
use egui_taffy::taffy::prelude::{auto, length, percent};
use egui_taffy::taffy::{AlignContent, AlignItems, Display, FlexDirection, Size, Style};
use egui_taffy::{Tui, TuiBuilderLogic, tui};
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::file_picker::Picker;
use crate::forms::Form;
use crate::forms::transforms::no_transform;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Default, Deserialize, Serialize)]
pub struct NewPcbTab {
    fields: Value<NewPcbFields>,

    #[serde(skip)]
    pub component: ComponentState<NewPcbTabUiCommand>,

    #[serde(skip)]
    file_picker: Value<Picker>,
}

#[derive(Debug, Clone)]
pub enum NewPcbTabUiCommand {
    NameChanged(String),
    UnitsChanged(u16),
    Submit,
    PickDirectoryClicked,
    DirectoryPicked(PathBuf),
}

#[derive(Debug)]
pub enum NewPcbTabAction {
    Submit(NewPcbArgs),
}

pub struct NewPcbTabContext {
    pub tab_key: TabKey,
}

impl Tab for NewPcbTab {
    type Context = NewPcbTabContext;

    fn label(&self) -> WidgetText {
        egui::widget_text::WidgetText::from(tr!("tab-label-new-pcb"))
    }

    fn ui(&mut self, ui: &mut Ui, tab_key: &TabKey, _context: &mut Self::Context) {
        let mut new_pcb_tab_context = NewPcbTabContext {
            tab_key: tab_key.clone(),
        };
        UiComponent::ui(self, ui, &mut new_pcb_tab_context);
    }
}

impl UiComponent for NewPcbTab {
    type UiContext<'context> = NewPcbTabContext;
    type UiCommand = NewPcbTabUiCommand;
    type UiAction = NewPcbTabAction;

    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        if let Ok(picked_directory) = self
            .file_picker
            .lock()
            .unwrap()
            .picked()
        {
            self.component
                .send(NewPcbTabUiCommand::DirectoryPicked(picked_directory.clone()));
        }

        ui.ctx().style_mut(|style| {
            // if this is not done, text in labels/checkboxes/etc wraps
            style.wrap_mode = Some(egui::TextWrapMode::Extend);
        });

        let form = Form::new(&self.fields, &self.component.sender, ());

        self.show_form(ui, &form);
    }

    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            NewPcbTabUiCommand::NameChanged(name) => {
                self.fields.lock().unwrap().name = name;
                None
            }
            NewPcbTabUiCommand::UnitsChanged(units) => {
                let mut fields = self.fields.lock().unwrap();

                fields.units = units;
                None
            }
            NewPcbTabUiCommand::Submit => {
                let fields = self.fields.lock().unwrap();
                let args = NewPcbArgs {
                    name: fields.name.clone(),
                    directory: fields.directory.clone().unwrap(),
                    units: fields.units,
                };
                Some(NewPcbTabAction::Submit(args))
            }
            NewPcbTabUiCommand::PickDirectoryClicked => {
                self.file_picker
                    .lock()
                    .unwrap()
                    .pick_folder();
                None
            }
            NewPcbTabUiCommand::DirectoryPicked(picked_directory) => {
                self.fields.lock().unwrap().directory = Some(picked_directory);
                None
            }
        }
    }
}

impl NewPcbTab {
    fn show_form(&self, ui: &mut Ui, form: &Form<NewPcbFields, NewPcbTabUiCommand>) {
        let default_style = || Style {
            padding: length(2.),
            gap: length(2.),
            ..Default::default()
        };

        tui(ui, ui.id().with("new_pcb_form"))
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
                form.show_fields_vertical(tui, |form, tui| {
                    form.add_field_ui("name", tr!("form-new-pcb-input-name"), tui, {
                        // NOTE text input does not resize with grid cell when using `.ui_add`, known issue - https://discord.com/channels/900275882684477440/904461220592119849/1338883750922293319
                        //      as a workaround we use `ui_add_manual` for now, with `no_transform`.
                        move |ui: &mut Ui, fields, sender| {
                            let mut name_clone = fields.name.clone();
                            let output = TextEdit::singleline(&mut name_clone)
                                .desired_width(ui.available_width())
                                .hint_text(tr!("form-new-pcb-input-name-placeholder"))
                                .show(ui);

                            if !fields.name.eq(&name_clone) {
                                sender
                                    .send(NewPcbTabUiCommand::NameChanged(name_clone))
                                    .expect("sent")
                            }

                            output.response
                        }
                    });

                    form.add_field_tui("directory", tr!("form-new-pcb-input-directory"), tui, {
                        move |tui: &mut Tui, fields, sender| {
                            tui.style(Style {
                                display: Display::Flex,
                                align_content: Some(AlignContent::Stretch),
                                flex_grow: 1.0,
                                ..default_style()
                            })
                            .add(|tui| {
                                tui.style(Style {
                                    flex_grow: 1.0,
                                    ..default_style()
                                })
                                .ui_add_manual(
                                    |ui| {
                                        // NOTE text input does not resize with grid cell when using `.ui_add`, known issue - https://discord.com/channels/900275882684477440/904461220592119849/1338883750922293319
                                        //      as a workaround we use `ui_add_manual` for now, with `no_transform`.
                                        let mut chosen_directory = fields
                                            .directory
                                            .as_ref()
                                            .map_or("".to_string(), |path| path.display().to_string());

                                        TextEdit::singleline(&mut chosen_directory)
                                            .desired_width(ui.available_width())
                                            .interactive(false)
                                            .ui(ui)
                                    },
                                    no_transform,
                                );

                                if tui
                                    .style(Style {
                                        flex_grow: 0.0,
                                        ..default_style()
                                    })
                                    .ui_add(Button::new("..."))
                                    .clicked()
                                {
                                    sender
                                        .send(NewPcbTabUiCommand::PickDirectoryClicked)
                                        .expect("sent");
                                }
                            })
                        }
                    });
                    form.add_field_ui("units", tr!("form-new-pcb-input-units"), tui, {
                        move |ui: &mut Ui, fields, sender| {
                            let mut units = fields.units;
                            ui.add(egui::DragValue::new(&mut units).range(1..=u16::MAX));

                            if units != fields.units {
                                sender
                                    .send(NewPcbTabUiCommand::UnitsChanged(units))
                                    .expect("sent");
                            }

                            ui.response()
                        }
                    });
                });
                if tui
                    .style(Style {
                        ..default_style()
                    })
                    .enabled_ui(form.is_valid())
                    .ui_add(Button::new(tr!("form-button-ok")))
                    .clicked()
                {
                    self.component
                        .send(NewPcbTabUiCommand::Submit);
                }
            });
    }
}

#[derive(Clone, Debug, Default, Validate, serde::Deserialize, serde::Serialize)]
struct NewPcbFields {
    #[validate(length(min = 1, code = "form-input-error-length"))]
    name: String,

    #[validate(required(code = "form-option-error-required"))]
    directory: Option<PathBuf>,

    #[validate(range(min = 1, max = u16::MAX, code = "form-input-error-range"))]
    units: u16,
}

/// Value object
#[derive(Debug, Clone)]
pub struct NewPcbArgs {
    pub name: String,
    pub directory: PathBuf,
    pub units: u16,
}

impl NewPcbArgs {
    pub fn build_path(&self) -> PathBuf {
        let Self {
            name,
            directory,
            ..
        } = self;

        let mut pcb_file_path: PathBuf = directory.clone();
        pcb_file_path.push(format!("{}.pcb.json", name));
        pcb_file_path
    }
}
