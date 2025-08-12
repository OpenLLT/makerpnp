use derivative::Derivative;
use egui::{TextEdit, Ui, WidgetText};
use egui_dock::tab_viewer::OnCloseResponse;
use egui_i18n::tr;
use egui_mobius::Value;
use egui_taffy::taffy::prelude::{auto, length, percent};
use egui_taffy::taffy::{AlignItems, FlexDirection, Size, Style};
use egui_taffy::tui;
use planner_app::{ProcessDefinition, ProcessReference, Reference};
use tracing::debug;
use validator::Validate;

use crate::forms::Form;
use crate::project::tabs::ProjectTabContext;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct ProcessTabUi {
    state: Option<ProcessTabUiState>,

    pub component: ComponentState<ProcessTabUiCommand>,
}

#[derive(Debug)]
pub struct ProcessTabUiState {
    available_tasks: Vec<String>,
    fields: Value<ProcessFields>,
    initial_args: ProcessDefinitionArgs,
    initial_process_reference: ProcessReference,
}

impl ProcessTabUi {
    pub fn new() -> Self {
        let component: ComponentState<ProcessTabUiCommand> = Default::default();

        Self {
            state: None,
            component,
        }
    }

    pub fn reset(&mut self) {
        self.state = None;
    }

    pub fn update_definition(&mut self, process_definition: ProcessDefinition) {
        let available_tasks: Vec<String> = vec![
            "core::load_pcbs".into(),
            "core::place_components".into(),
            "core::automated_soldering".into(),
            "core::manual_soldering".into(),
        ];

        let initial_process_reference = process_definition.reference.clone();

        let fields = ProcessFields::from_process_definition(process_definition);
        let initial_args = fields.build_args(initial_process_reference.clone());

        self.state = Some(ProcessTabUiState {
            fields: Value::new(fields),
            available_tasks,
            initial_args,
            initial_process_reference,
        })
    }

    fn show_form(&self, ui: &mut Ui, form: &Form<ProcessFields, ProcessTabUiCommand>) {
        let default_style = || Style {
            padding: length(2.),
            gap: length(2.),
            ..Default::default()
        };

        tui(ui, ui.id().with("process_form"))
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
                    form.add_field_ui("reference", tr!("form-common-input-process-reference"), tui, {
                        // NOTE text input does not resize with grid cell when using `.ui_add`, known issue - https://discord.com/channels/900275882684477440/904461220592119849/1338883750922293319
                        //      as a workaround we use `ui_add_manual` for now, with `no_transform`.
                        move |ui: &mut Ui, fields, sender| {
                            let mut reference_clone = fields.reference.clone();
                            let output = TextEdit::singleline(&mut reference_clone)
                                .desired_width(ui.available_width())
                                .show(ui);

                            if !fields.reference.eq(&reference_clone) {
                                sender
                                    .send(ProcessTabUiCommand::ReferenceChanged(reference_clone))
                                    .expect("sent")
                            }

                            output.response
                        }
                    });
                });
            });
    }
}

#[derive(Clone, Debug, Default, Validate, serde::Deserialize, serde::Serialize)]
pub struct ProcessFields {
    // FUTURE could also validate that the reference is not already used
    #[validate(length(min = 1, code = "form-input-error-length"))]
    #[validate(custom(function = "crate::forms::validation::CommonValidation::validate_reference"))]
    reference: String,
}

impl ProcessFields {
    pub fn from_process_definition(process: ProcessDefinition) -> Self {
        Self {
            reference: process.reference.to_string(),
        }
    }

    pub fn build_args(&self, initial_process_reference: ProcessReference) -> ProcessDefinitionArgs {
        ProcessDefinitionArgs {
            process_reference: initial_process_reference,
            process_definition: ProcessDefinition {
                // safety, validation ensures that the reference is valid
                reference: ProcessReference::from_raw(self.reference.clone()),
                operations: vec![],
                rules: vec![],
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProcessDefinitionArgs {
    pub process_reference: ProcessReference,
    pub process_definition: ProcessDefinition,
}

#[derive(Debug, Clone)]
pub enum ProcessTabUiCommand {
    None,
    ReferenceChanged(String),
    Apply,
    Reset,
}

#[derive(Debug, Clone)]
pub enum ProcessTabUiAction {
    None,
    Reset { process_reference: ProcessReference },
    Apply(ProcessDefinitionArgs),
}

#[derive(Debug, Clone, Default)]
pub struct ProcessTabUiContext {}

impl UiComponent for ProcessTabUi {
    type UiContext<'context> = ProcessTabUiContext;
    type UiCommand = ProcessTabUiCommand;
    type UiAction = ProcessTabUiAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        ui.ctx().style_mut(|style| {
            // if this is not done, text in labels/checkboxes/etc wraps
            style.wrap_mode = Some(egui::TextWrapMode::Extend);
        });

        ui.label(tr!("project-process-header"));

        let Some(state) = &self.state else {
            ui.spinner();
            return;
        };

        let form = Form::new(&state.fields, &self.component.sender, ());

        self.show_form(ui, &form);

        let is_changed = state
            .fields
            .lock()
            .unwrap()
            .build_args(state.initial_process_reference.clone())
            != state.initial_args;

        egui::Sides::new().show(
            ui,
            |ui| {
                if ui
                    .add_enabled(is_changed, egui::Button::new(tr!("form-common-button-reset")))
                    .clicked()
                {
                    self.component
                        .send(ProcessTabUiCommand::Reset);
                }

                if ui
                    .add_enabled(
                        is_changed && form.is_valid(),
                        egui::Button::new(tr!("form-common-button-apply")),
                    )
                    .clicked()
                {
                    self.component
                        .send(ProcessTabUiCommand::Apply);
                }
            },
            |_ui| {},
        );
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        let Some(state) = &mut self.state else {
            let result = match command {
                ProcessTabUiCommand::None => Some(ProcessTabUiAction::None),
                _ => {
                    // there are no commands that can be processed without the state
                    None
                }
            };
            return result;
        };

        let mut fields = state.fields.lock().unwrap();
        match command {
            ProcessTabUiCommand::None => Some(ProcessTabUiAction::None),
            ProcessTabUiCommand::ReferenceChanged(reference) => {
                fields.reference = reference;
                None
            }

            //
            // form submission
            //
            ProcessTabUiCommand::Apply => {
                let args = fields.build_args(state.initial_process_reference.clone());

                Some(ProcessTabUiAction::Apply(args))
            }
            ProcessTabUiCommand::Reset => Some(ProcessTabUiAction::Reset {
                process_reference: state
                    .initial_args
                    .process_reference
                    .clone(),
            }),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq)]
pub struct ProcessTab {
    pub process: ProcessReference,
}

impl ProcessTab {
    pub fn new(process: Reference) -> Self {
        Self {
            process,
        }
    }
}

impl Tab for ProcessTab {
    type Context = ProjectTabContext;

    fn label(&self) -> WidgetText {
        let title = tr!("project-process-tab-label", {process: self.process.to_string()});
        egui::widget_text::WidgetText::from(title)
    }

    fn ui<'a>(&mut self, ui: &mut Ui, _tab_key: &TabKey, context: &mut Self::Context) {
        let state = context.state.lock().unwrap();
        let Some(process_ui) = state.process_tab_uis.get(&self.process) else {
            ui.spinner();
            return;
        };
        UiComponent::ui(process_ui, ui, &mut ProcessTabUiContext::default());
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, context: &mut Self::Context) -> OnCloseResponse {
        let mut state = context.state.lock().unwrap();
        if let Some(_process_ui) = state
            .process_tab_uis
            .remove(&self.process)
        {
            debug!("removed orphaned process ui. process: {:?}", &self.process);
        }
        OnCloseResponse::Close
    }
}
