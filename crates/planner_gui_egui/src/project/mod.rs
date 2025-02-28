use std::path::PathBuf;
use egui::{Modal, Ui};
use egui_extras::{Column, TableBuilder};
use egui_i18n::tr;
use egui_mobius::slot::Slot;
use egui_mobius::types::{Enqueue, Value};
use slotmap::new_key_type;
use tracing::{debug, error};
use planner_app::{Event, ProjectView, ProjectViewRequest, Reference};
use crate::planner_app_core::PlannerCoreService;
use crate::task::Task;

new_key_type! {
    /// A key for a project
    pub struct ProjectKey;
}

pub struct Project {
    planner_core_service: PlannerCoreService,
    sender: Enqueue<(ProjectKey, ProjectUiCommand)>,
    path: PathBuf,
    project_ui_state: Value<ProjectUiState>,
    project_slot: Slot<(ProjectKey, ProjectUiCommand)>,
    modified: bool,

    // list of errors to show
    errors: Vec<String>,
}

impl Project {
    pub fn from_path(path: PathBuf, sender: Enqueue<(ProjectKey, ProjectUiCommand)>, project_slot: Slot<(ProjectKey, ProjectUiCommand)>) -> (Self, ProjectUiCommand) {

        debug!("Creating project instance from path. path: {}", &path.display());

        let project_ui_state = Value::new(ProjectUiState::default());

        let core_service = PlannerCoreService::new();
        let instance = Self {
            sender,
            path,
            planner_core_service: core_service,
            project_ui_state,
            project_slot,
            modified: false,
            errors: Default::default(),
        };

        (instance, ProjectUiCommand::Load)
    }

    pub fn ui(&self, ui: &mut Ui, key: ProjectKey) {
        ui.label(format!("Project.  path: {}", self.path.display()));

        let state = self.project_ui_state.lock().unwrap();
        if let Some(name) = &state.name {
            ui.label(format!("name: {}", name));
        } else {
            ui.spinner();
        }

        if !self.errors.is_empty() {
            self.show_errors_modal(ui, key);
        }
    }

    fn show_errors_modal(&self, ui: &mut Ui, key: ProjectKey) {
        let errors_modal_id = ui.id().with("errors");

        Modal::new(errors_modal_id)
            .show(ui.ctx(), |ui| {
                ui.set_width(ui.available_width() * 0.8);
                let file_name = self.path.file_name().unwrap().to_str().unwrap();
                ui.heading(tr!("modal-errors-title", {file: file_name}));

                let mut table = TableBuilder::new(ui)
                    .striped(true)
                    .resizable(true)
                    .column(Column::auto())
                    .column(Column::remainder());

                table.header(20.0, |mut header| {
                    header.col(|ui| {
                        ui.strong(tr!("modal-errors-column-errors"));
                    });
                }).body(|mut body| {
                    for (index, error) in self.errors.iter().enumerate() {
                        body.row(18.0, |mut row| {
                            row.col(|ui| {
                                ui.label(format!("{}", index));
                            });
                            row.col(|ui| {
                                ui.label(error);
                            });
                        })
                    }
                });


                egui::Sides::new().show(
                    ui,
                    |_ui| {},
                    |ui| {
                        if ui.button(tr!("form-button-ok")).clicked() {
                            self.sender.send((key, ProjectUiCommand::ClearErrors)).expect("sent")
                        }
                    },
                );
            });
    }

    pub fn update(&mut self, key: ProjectKey, command: ProjectUiCommand) -> Task<Result<(ProjectKey, ProjectUiCommand), ProjectError>>{
        match command {
            ProjectUiCommand::None => {
                Task::none()
            }
            ProjectUiCommand::Load => {
                debug!("Loading project from path. path: {}", self.path.display());

                self.planner_core_service.update(Event::Load {
                    path: self.path.clone(),
                }, key)
                    .map(|result| {
                        result.map(|(key, _)| (key, ProjectUiCommand::Loaded))
                    })
            }
            ProjectUiCommand::Loaded => {
                let mut state = self.project_ui_state.lock().unwrap();
                state.loaded = true;
                self
                    .planner_core_service
                    .update(Event::RequestOverviewView {}, key)
                    .chain(Task::done(Ok((key, ProjectUiCommand::RequestView(ProjectViewRequest::ProjectTree)))))
            }
            ProjectUiCommand::RequestView(view_request) => {
                let event = match view_request {
                    ProjectViewRequest::Overview => Event::RequestOverviewView {},
                    ProjectViewRequest::ProjectTree => Event::RequestProjectTreeView {},
                };

                self.planner_core_service.update(event, key)
            }
            ProjectUiCommand::UpdateView(view) => {
                match view {
                    ProjectView::Overview(project_overview) => {
                        debug!("project overview: {:?}", project_overview);
                        let mut state = self.project_ui_state.lock().unwrap();
                        state.name = Some(project_overview.name);
                    }
                    ProjectView::ProjectTree(_) => {}
                    ProjectView::Placements(_) => {}
                    ProjectView::PhaseOverview(_) => {}
                    ProjectView::PhasePlacements(_) => {}
                    ProjectView::PhasePlacementOrderings(_) => {}
                }
                Task::none()
            }
            ProjectUiCommand::Error(error) => {
                match error {
                    ProjectError::CoreError(message) => {
                        self.errors.push(message);
                    }
                }
                Task::none()
            }
            ProjectUiCommand::ClearErrors => {
                self.errors.clear();
                Task::none()
            }
            ProjectUiCommand::SetModifiedState(modified_state) => {
                self.modified = modified_state;
                Task::none()
            }
        }
    }
}

#[derive(Default, Debug)]
pub struct ProjectUiState {
    loaded: bool,
    name: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ProjectUiCommand {
    None,
    Load,
    Loaded,
    UpdateView(ProjectView),
    Error(ProjectError),
    SetModifiedState(bool),
    RequestView(ProjectViewRequest),
    ClearErrors,
}

#[derive(Debug, Clone)]
pub enum ProjectError {
    CoreError(String),
}