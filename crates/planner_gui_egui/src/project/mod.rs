use std::path::PathBuf;
use egui::Ui;
use egui_mobius::slot::Slot;
use egui_mobius::types::{Enqueue, Value};
use slotmap::new_key_type;
use tracing::debug;
use planner_app::{Event, ProjectView};
use crate::planner_app_core::PlannerCoreService;

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
        };

        (instance, ProjectUiCommand::Load)
    }
    
    pub fn ui(&self, ui: &mut Ui) {
        ui.label(format!("Project.  path: {}", self.path.display()));

        let state = self.project_ui_state.lock().unwrap();
        if let Some(name) = &state.name {
            ui.label(format!("name: {}", name));
        } else {
            ui.spinner();
        }
        
    }
    
    pub fn update(&mut self, key: ProjectKey, command: ProjectUiCommand) {
        match command {
            ProjectUiCommand::None => {}
            ProjectUiCommand::Load => {
                debug!("Loading project from path. path: {}", self.path.display());
                
                self.planner_core_service.update(Event::Load {
                    path: self.path.clone(),
                }, key, self.sender.clone());
            }
            ProjectUiCommand::Loaded => {
                let mut state = self.project_ui_state.lock().unwrap();
                state.loaded = true;
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
}