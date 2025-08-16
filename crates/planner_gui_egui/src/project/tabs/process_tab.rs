use std::str::FromStr;

use derivative::Derivative;
use egui::{Frame, TextEdit, Ui, WidgetText};
use egui_dock::tab_viewer::OnCloseResponse;
use egui_extras::Column;
use egui_i18n::tr;
use egui_mobius::Value;
use egui_taffy::taffy::prelude::{auto, length, percent};
use egui_taffy::taffy::{AlignItems, FlexDirection, Size, Style};
use egui_taffy::tui;
use indexmap::IndexMap;
use planner_app::{
    OperationDefinition, OperationReference, ProcessDefinition, ProcessReference, ProcessRuleReference, Reference,
    TaskReference,
};
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
    available_tasks: Vec<TaskReference>,
    fields: Value<ProcessFields>,
    new_operation_fields: Value<NewOperationFields>,
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
        let available_tasks: Vec<TaskReference> = vec![
            TaskReference::from_raw_str("core::load_pcbs"),
            TaskReference::from_raw_str("core::place_components"),
            TaskReference::from_raw_str("core::automated_soldering"),
            TaskReference::from_raw_str("core::manual_soldering"),
        ];

        let initial_process_reference = process_definition.reference.clone();

        let fields = ProcessFields::from_process_definition(process_definition);
        let initial_args = fields.build_args(initial_process_reference.clone());

        self.state = Some(ProcessTabUiState {
            fields: Value::new(fields),
            new_operation_fields: Value::new(NewOperationFields::default()),
            available_tasks,
            initial_args,
            initial_process_reference,
        })
    }

    fn show_form(&self, ui: &mut Ui, form: &Form<ProcessFields, ProcessTabUiCommand>, state: &ProcessTabUiState) {
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
                            let mut reference_clone = fields.process_reference.clone();
                            let output = TextEdit::singleline(&mut reference_clone)
                                .desired_width(ui.available_width().min(200.0))
                                .show(ui);

                            if !fields
                                .process_reference
                                .eq(&reference_clone)
                            {
                                sender
                                    .send(ProcessTabUiCommand::ReferenceChanged(reference_clone))
                                    .expect("sent")
                            }

                            output.response
                        }
                    });

                    form.add_field_ui("operations", tr!("form-process-operations"), tui, {
                        // NOTE text input does not resize with grid cell when using `.ui_add`, known issue - https://discord.com/channels/900275882684477440/904461220592119849/1338883750922293319
                        //      as a workaround we use `ui_add_manual` for now, with `no_transform`.
                        move |ui: &mut Ui, fields, sender| {
                            let text_height = egui::TextStyle::Body
                                .resolve(ui.style())
                                .size
                                .max(ui.spacing().interact_size.y);

                            Frame::group(&egui::Style::default()).show(ui, |ui| {
                                let mut table_builder = egui_extras::TableBuilder::new(ui)
                                    .striped(true)
                                    .column(Column::remainder());

                                for _ in 0..state.available_tasks.len() {
                                    table_builder = table_builder.column(Column::auto());
                                }
                                table_builder = table_builder.column(Column::auto());

                                table_builder
                                    .header(text_height, |mut header| {
                                        header.col(|ui| {
                                            ui.strong(tr!("table-operations-column-operation"));
                                        });
                                        for task in &state.available_tasks {
                                            header.col(|ui| {
                                                // TODO generate an i18n key from the task instead
                                                ui.strong(
                                                    tr!("table-operations-column-task", { task: task.to_string() }),
                                                );
                                            });
                                        }
                                        header.col(|ui| {
                                            ui.strong(tr!("table-operations-column-actions"));
                                        });
                                    })
                                    .body(|mut body| {
                                        for (operation, tasks) in fields.operations.iter() {
                                            body.row(text_height, |mut row| {
                                                row.col(|ui| {
                                                    let mut operation_reference = operation.to_string();
                                                    let output =
                                                        TextEdit::singleline(&mut operation_reference).show(ui);
                                                    if output.response.changed() {
                                                        self.component.send(
                                                            ProcessTabUiCommand::OperationReferenceChanged {
                                                                operation: operation.clone(),
                                                                new_operation_reference: operation_reference,
                                                            },
                                                        );
                                                    }
                                                });

                                                for task in &state.available_tasks {
                                                    row.col(|ui| {
                                                        let mut checked = tasks.contains(task);
                                                        if ui
                                                            .add(egui::Checkbox::without_text(&mut checked))
                                                            .changed()
                                                        {
                                                            sender
                                                                .send(ProcessTabUiCommand::TaskChanged {
                                                                    operation: operation.clone(),
                                                                    task: task.clone(),
                                                                    checked,
                                                                })
                                                                .expect("sent");
                                                        }
                                                    });
                                                }

                                                row.col(|ui| {
                                                    if ui
                                                        .button(format!("🗑 {}", tr!("form-common-button-delete")))
                                                        .clicked()
                                                    {
                                                        self.component.send(
                                                            ProcessTabUiCommand::DeleteOperationClicked {
                                                                operation: operation.clone(),
                                                            },
                                                        );
                                                    }
                                                });
                                            })
                                        }
                                    });
                            });
                            ui.response()
                        }
                        // end of form.add_field_ui
                    });
                    // end of form.show_fields_vertical
                });

                // end of tui.show
            });
    }

    /// we need an entirely different form for adding a new operation.
    /// if we were to combine the 'operation_reference' with the other fields, it would prevent applying changes
    /// when the field is empty or invalid, which is not the intention
    /// FUTURE consider moving this form to a 'new operation' modal
    pub fn show_new_operaton_form(
        &self,
        ui: &mut Ui,
        form: &Form<NewOperationFields, ProcessTabUiCommand>,
        _state: &ProcessTabUiState,
    ) {
        let default_style = || Style {
            padding: length(2.),
            gap: length(2.),
            ..Default::default()
        };

        tui(ui, ui.id().with("new_operation_form"))
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
                    form.add_field_ui(
                        "operation_reference",
                        tr!("form-common-input-operation-reference"),
                        tui,
                        {
                            move |ui: &mut Ui, fields, sender| {
                                ui.horizontal(|ui| {
                                    let mut operation_reference = fields.operation_reference.to_string();
                                    let output = TextEdit::singleline(&mut operation_reference).show(ui);
                                    if output.response.changed() {
                                        sender
                                            .send(ProcessTabUiCommand::NewOperationReferenceChanged(
                                                operation_reference.clone(),
                                            ))
                                            .expect("sent");
                                    }

                                    let is_new_operation_valid = form
                                        .field_validation_errors("operation_reference")
                                        .is_none();

                                    ui.add_enabled_ui(is_new_operation_valid, |ui| {
                                        if ui
                                            .button(tr!("form-common-button-add"))
                                            .clicked()
                                        {
                                            sender
                                                .send(ProcessTabUiCommand::AddOperationClicked)
                                                .expect("sent");
                                        }
                                    });
                                });

                                ui.response()
                            }
                        },
                    );

                    // end of form.show_fields_vertical
                });

                // end of tui.show
            });
    }
}

#[derive(Clone, Debug, Default, Validate, serde::Deserialize, serde::Serialize)]
pub struct ProcessFields {
    // FUTURE could also validate that the reference is not already used
    #[validate(length(min = 1, code = "form-input-error-length"))]
    #[validate(custom(function = "crate::forms::validation::CommonValidation::validate_reference"))]
    process_reference: String,

    operations: IndexMap<OperationReference, Vec<TaskReference>>,

    rules: Vec<ProcessRuleReference>,
}

#[derive(Clone, Debug, Validate, serde::Deserialize, serde::Serialize)]
pub struct NewOperationFields {
    // FUTURE could also validate that the reference is not already used
    #[validate(length(min = 1, code = "form-input-error-length"))]
    #[validate(custom(function = "crate::forms::validation::CommonValidation::validate_reference"))]
    pub operation_reference: String,
}

impl Default for NewOperationFields {
    fn default() -> Self {
        Self {
            operation_reference: "".to_string(),
        }
    }
}

impl ProcessFields {
    pub fn from_process_definition(process: ProcessDefinition) -> Self {
        Self {
            process_reference: process.reference.to_string(),
            operations: process
                .operations
                .into_iter()
                .map(|it| (it.reference, it.tasks))
                .collect(),
            rules: process.rules.clone(),
        }
    }

    pub fn build_args(&self, initial_process_reference: ProcessReference) -> ProcessDefinitionArgs {
        let operations = self
            .operations
            .iter()
            .map(|(operation, tasks)| OperationDefinition {
                reference: operation.clone(),
                tasks: tasks.clone(),
            })
            .collect::<Vec<_>>();

        let rules = self.rules.clone();

        ProcessDefinitionArgs {
            process_reference: initial_process_reference,
            process_definition: ProcessDefinition {
                // safety, validation ensures that the reference is valid
                reference: ProcessReference::from_raw(self.process_reference.clone()),
                operations,
                rules,
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
    ApplyClicked,
    ResetClicked,
    DeleteClicked,
    TaskChanged {
        operation: OperationReference,
        task: TaskReference,
        checked: bool,
    },
    DeleteOperationClicked {
        operation: OperationReference,
    },
    AddOperationClicked,
    OperationReferenceChanged {
        operation: OperationReference,
        new_operation_reference: String,
    },
    NewOperationReferenceChanged(String),
}

#[derive(Debug, Clone)]
pub enum ProcessTabUiAction {
    None,
    Reset { process_reference: ProcessReference },
    Apply(ProcessDefinitionArgs),
    Delete(ProcessReference),
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

        //
        // toolbar
        //
        ui.horizontal(|ui| {
            if ui
                .button(format!("🗑 {}", tr!("form-common-button-delete")))
                .clicked()
            {
                self.component
                    .send(ProcessTabUiCommand::DeleteClicked);
            }
        });

        //
        // form
        //
        let form = Form::new(&state.fields, &self.component.sender, ());

        self.show_form(ui, &form, state);

        let new_operation_form = Form::new(&state.new_operation_fields, &self.component.sender, ());

        self.show_new_operaton_form(ui, &new_operation_form, state);

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
                        .send(ProcessTabUiCommand::ResetClicked);
                }

                if ui
                    .add_enabled(
                        is_changed && form.is_valid(),
                        egui::Button::new(tr!("form-common-button-apply")),
                    )
                    .clicked()
                {
                    self.component
                        .send(ProcessTabUiCommand::ApplyClicked);
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
                fields.process_reference = reference;
                None
            }
            ProcessTabUiCommand::NewOperationReferenceChanged(operation_reference) => {
                let mut new_operation_fields = state
                    .new_operation_fields
                    .lock()
                    .unwrap();

                new_operation_fields.operation_reference = operation_reference;
                None
            }
            ProcessTabUiCommand::DeleteClicked => {
                Some(ProcessTabUiAction::Delete(state.initial_process_reference.clone()))
            }
            ProcessTabUiCommand::TaskChanged {
                operation,
                task,
                checked,
            } => {
                debug!("operation: {:?}, task: {:?}, checked: {:?}", operation, task, checked);

                // each task can only belong to one operation
                for (operation_reference, tasks) in fields.operations.iter_mut() {
                    enum Action {
                        Add,
                        Remove,
                    }
                    let action = match (operation_reference == &operation, checked) {
                        (true, true) => Action::Add,
                        (true, false) => Action::Remove,
                        (false, _) => Action::Remove,
                    };

                    match action {
                        Action::Add => {
                            tasks.push(task.clone());
                        }
                        Action::Remove => {
                            tasks.retain(|candidate_task| !task.eq(candidate_task));
                        }
                    }
                }
                None
            }
            ProcessTabUiCommand::AddOperationClicked => {
                let new_operation_fields = state
                    .new_operation_fields
                    .lock()
                    .unwrap();

                if let Ok(new_operation) = OperationReference::from_str(&new_operation_fields.operation_reference) {
                    fields
                        .operations
                        .insert(new_operation, vec![]);
                }
                None
            }
            ProcessTabUiCommand::OperationReferenceChanged {
                operation,
                new_operation_reference,
            } => {
                if let Ok(new_operation_reference) = OperationReference::from_str(&new_operation_reference) {
                    if let Some(position) = fields
                        .operations
                        .iter()
                        .position(|(candidate_operation, _tasks)| operation.eq(candidate_operation))
                    {
                        let tasks = fields
                            .operations
                            .shift_remove(&operation)
                            .unwrap();
                        fields
                            .operations
                            .insert_before(position, new_operation_reference, tasks);
                    }
                }
                None
            }
            ProcessTabUiCommand::DeleteOperationClicked {
                operation,
            } => fields
                .operations
                .shift_remove(&operation)
                .map(|_operation| {
                    let args = fields.build_args(state.initial_process_reference.clone());

                    ProcessTabUiAction::Apply(args)
                }),

            //
            // form submission
            //
            ProcessTabUiCommand::ApplyClicked => {
                let args = fields.build_args(state.initial_process_reference.clone());

                Some(ProcessTabUiAction::Apply(args))
            }
            ProcessTabUiCommand::ResetClicked => Some(ProcessTabUiAction::Reset {
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
