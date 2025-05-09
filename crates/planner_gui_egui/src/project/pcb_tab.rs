use std::path::PathBuf;

use derivative::Derivative;
use egui::{Ui, WidgetText};
use egui_extras::Column;
use egui_i18n::tr;
use planner_app::{DesignName, GerberPurpose, ObjectPath, PcbOverview, PcbSide};
use tracing::{debug, trace};

use crate::project::ProjectUiCommand;
use crate::project::dialogs::create_unit_assignment::{
    CreateUnitAssignmentArgs, CreateUnitAssignmentModal, CreateUnitAssignmentModalAction,
    CreateUnitAssignmentModalUiCommand,
};
use crate::project::dialogs::manage_gerbers::{
    ManageGerbersModal, ManagerGerberModalAction, ManagerGerbersModalUiCommand,
};
use crate::project::tabs::ProjectTabContext;
use crate::project::toolbar::{ProjectToolbarAction, ProjectToolbarUiCommand};
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct PcbUi {
    path: PathBuf,
    pcb_overview: Option<PcbOverview>,

    manage_gerbers_modal: Option<ManageGerbersModal>,
    create_unit_assignment_modal: Option<CreateUnitAssignmentModal>,

    pub component: ComponentState<PcbUiCommand>,
}

impl PcbUi {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            pcb_overview: None,
            manage_gerbers_modal: None,
            create_unit_assignment_modal: None,
            component: Default::default(),
        }
    }

    pub fn update_overview(&mut self, pcb_overview: PcbOverview) {
        if let Some(modal) = &mut self.manage_gerbers_modal {
            modal.update_gerbers(&pcb_overview.gerbers)
        }
        self.pcb_overview = Some(pcb_overview);
    }

    fn show_manage_gerbers_modal(&mut self, design_index: usize) {
        let Some((design_name, design_gerbers)) = self
            .pcb_overview
            .as_ref()
            .map(|pcb_overview| {
                let design_name = pcb_overview.designs[design_index].clone();
                let gerbers = pcb_overview.gerbers[design_index].clone();
                (design_name, gerbers)
            })
        else {
            return;
        };

        let mut modal = ManageGerbersModal::new(design_index, design_name, design_gerbers);
        modal
            .component
            .configure_mapper(self.component.sender.clone(), move |command| {
                trace!("manage gerbers modal mapper. command: {:?}", command);
                PcbUiCommand::ManageGerbersModalUiCommand(command)
            });

        self.manage_gerbers_modal = Some(modal);
    }

    fn show_create_unit_assignments_modal(&mut self) {
        let Some(pcb_overview) = &self.pcb_overview else {
            return;
        };

        let mut modal = CreateUnitAssignmentModal::new(self.path.clone(), pcb_overview.index, pcb_overview.units);
        modal
            .component
            .configure_mapper(self.component.sender.clone(), move |command| {
                trace!("create unit assignment modal mapper. command: {:?}", command);
                PcbUiCommand::CreateUnitAssignmentModalCommand(command)
            });

        self.create_unit_assignment_modal = Some(modal);
    }
}

#[derive(Debug, Clone)]
pub enum PcbUiCommand {
    None,
    ManageGerbersClicked { design_index: usize },
    ManageGerbersModalUiCommand(ManagerGerbersModalUiCommand),
    CreateUnitAssignmentClicked,
    CreateUnitAssignmentModalCommand(CreateUnitAssignmentModalUiCommand),
}

#[derive(Debug, Clone)]
pub enum PcbUiAction {
    None,
    AddGerberFiles {
        pcb_index: u16,
        design: DesignName,
        files: Vec<(PathBuf, Option<PcbSide>, GerberPurpose)>,
    },
    RemoveGerberFiles {
        pcb_index: u16,
        design: DesignName,
        files: Vec<PathBuf>,
    },
    CreateUnitAssignment(CreateUnitAssignmentArgs),
}

#[derive(Debug, Clone, Default)]
pub struct PcbUiContext {}

impl UiComponent for PcbUi {
    type UiContext<'context> = PcbUiContext;
    type UiCommand = PcbUiCommand;
    type UiAction = PcbUiAction;

    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        ui.label(tr!("project-pcb-header"));
        let Some(pcb_overview) = &self.pcb_overview else {
            ui.spinner();
            return;
        };

        //
        // toolbar
        //

        if ui
            .button(tr!("project-toolbar-button-create-unit-assignment"))
            .clicked()
        {
            self.component
                .send(PcbUiCommand::CreateUnitAssignmentClicked)
        }

        ui.separator();

        //
        // overview
        //
        ui.label(&pcb_overview.name);

        let text_height = egui::TextStyle::Body
            .resolve(ui.style())
            .size
            .max(ui.spacing().interact_size.y);

        ui.separator();

        //
        // designs table
        //
        ui.label(tr!("project-pcb-designs-header"));

        egui_extras::TableBuilder::new(ui)
            .striped(true)
            .column(Column::auto())
            .column(Column::auto())
            .column(Column::remainder())
            .header(text_height, |mut header| {
                header.col(|ui| {
                    ui.strong(tr!("table-designs-column-index"));
                });
                header.col(|ui| {
                    ui.strong(tr!("table-designs-column-actions"));
                });
                header.col(|ui| {
                    ui.strong(tr!("table-designs-column-name"));
                });
            })
            .body(|mut body| {
                for (index, design) in pcb_overview.designs.iter().enumerate() {
                    body.row(text_height, |mut row| {
                        row.col(|ui| {
                            ui.label(index.to_string());
                        });

                        row.col(|ui| {
                            if ui
                                .button(tr!("project-pcb-designs-button-gerbers"))
                                .clicked()
                            {
                                self.component
                                    .send(PcbUiCommand::ManageGerbersClicked {
                                        design_index: index,
                                    });
                            }
                        });

                        row.col(|ui| {
                            ui.label(design.to_string());
                        });
                    })
                }
            });

        //
        // Modals
        //
        if let Some(dialog) = &self.manage_gerbers_modal {
            dialog.ui(ui, &mut ());
        }
        if let Some(dialog) = &self.create_unit_assignment_modal {
            dialog.ui(ui, &mut ());
        }
    }

    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            PcbUiCommand::None => Some(PcbUiAction::None),
            PcbUiCommand::ManageGerbersClicked {
                design_index,
            } => {
                self.show_manage_gerbers_modal(design_index);
                None
            }
            PcbUiCommand::ManageGerbersModalUiCommand(command) => {
                if let Some(modal) = &mut self.manage_gerbers_modal {
                    match modal.update(command, &mut ()) {
                        None => None,
                        Some(ManagerGerberModalAction::CloseDialog) => {
                            self.manage_gerbers_modal = None;
                            None
                        }
                        Some(ManagerGerberModalAction::RemoveGerberFiles {
                            design_index,
                            files,
                        }) => {
                            debug!(
                                "removing gerber file. design_index: {}, files: {:?}",
                                design_index, files
                            );
                            if let Some(pcb_overview) = &mut self.pcb_overview {
                                let design = pcb_overview.designs[design_index].clone();
                                Some(PcbUiAction::RemoveGerberFiles {
                                    pcb_index: pcb_overview.index,
                                    design,
                                    files,
                                })
                            } else {
                                None
                            }
                        }
                        Some(ManagerGerberModalAction::AddGerberFiles {
                            design_index,
                            files,
                        }) => {
                            debug!(
                                "gerber files picked. design_index: {}, picked: {:?}",
                                design_index, files
                            );
                            if let Some(pcb_overview) = &mut self.pcb_overview {
                                let design = pcb_overview.designs[design_index].clone();
                                let files = files
                                    .into_iter()
                                    .map(|file| (file, None, GerberPurpose::Other))
                                    .collect();
                                Some(PcbUiAction::AddGerberFiles {
                                    pcb_index: pcb_overview.index,
                                    design,
                                    files,
                                })
                            } else {
                                None
                            }
                        }
                    }
                } else {
                    None
                }
            }
            PcbUiCommand::CreateUnitAssignmentClicked => {
                self.show_create_unit_assignments_modal();
                None
            }
            PcbUiCommand::CreateUnitAssignmentModalCommand(command) => {
                if let (Some(pcb_overview), Some(modal)) = (&self.pcb_overview, &mut self.create_unit_assignment_modal)
                {
                    let action = modal.update(command, &mut ());
                    match action {
                        None => None,
                        Some(CreateUnitAssignmentModalAction::Submit(args)) => {
                            self.create_unit_assignment_modal.take();
                            Some(PcbUiAction::CreateUnitAssignment(args))
                        }
                        Some(CreateUnitAssignmentModalAction::CloseDialog) => {
                            self.create_unit_assignment_modal.take();
                            None
                        }
                    }
                } else {
                    None
                }
            }
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq)]
pub struct PcbTab {
    pcb_index: u16,
}

impl PcbTab {
    pub fn new(pcb_index: u16) -> Self {
        Self {
            pcb_index,
        }
    }
}

impl Tab for PcbTab {
    type Context = ProjectTabContext;

    fn label(&self) -> WidgetText {
        let pcb = format!("{}", self.pcb_index).to_string();
        egui::widget_text::WidgetText::from(tr!("project-pcb-tab-label", {pcb: pcb}))
    }

    fn ui<'a>(&mut self, ui: &mut Ui, _tab_key: &TabKey, context: &mut Self::Context) {
        let state = context.state.lock().unwrap();
        let pcb_ui = state
            .pcbs
            .get(&(self.pcb_index as usize))
            .unwrap();
        UiComponent::ui(pcb_ui, ui, &mut PcbUiContext::default());
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, context: &mut Self::Context) -> bool {
        let mut state = context.state.lock().unwrap();
        if let Some(_pcb_ui) = state
            .pcbs
            .remove(&(self.pcb_index as usize))
        {
            debug!("removed orphaned pcb ui. pcb_index: {}", self.pcb_index);
        }
        true
    }
}
