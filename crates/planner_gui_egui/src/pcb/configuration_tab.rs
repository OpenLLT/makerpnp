use std::path::PathBuf;

use egui::{Ui, WidgetText};
use egui_extras::Column;
use egui_i18n::tr;
use planner_app::{DesignName, GerberPurpose, PcbOverview, PcbSide};
use tracing::{debug, trace};

use crate::dialogs::manage_gerbers::{ManageGerbersModal, ManagerGerberModalAction, ManagerGerbersModalUiCommand};
use crate::pcb::tabs::PcbTabContext;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Debug)]
pub struct ConfigurationUi {
    pcb_overview: Option<PcbOverview>,

    manage_gerbers_modal: Option<ManageGerbersModal>,

    pub component: ComponentState<ConfigurationUiCommand>,
}

impl ConfigurationUi {
    fn show_designs(&self, ui: &mut Ui) {
        let Some(pcb_overview) = &self.pcb_overview else {
            return;
        };

        // TODO move translations from the project
        ui.label(tr!("project-pcb-designs-header"));

        let text_height = egui::TextStyle::Body
            .resolve(ui.style())
            .size
            .max(ui.spacing().interact_size.y);

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
                                    .send(ConfigurationUiCommand::ManageGerbersClicked {
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

        let mut modal = ManageGerbersModal::new(design_index, design_name.to_string(), design_gerbers);
        modal
            .component
            .configure_mapper(self.component.sender.clone(), move |command| {
                trace!("manage gerbers modal mapper. command: {:?}", command);
                ConfigurationUiCommand::ManageGerbersModalUiCommand(command)
            });

        self.manage_gerbers_modal = Some(modal);
    }
}

impl ConfigurationUi {
    pub fn new() -> Self {
        Self {
            pcb_overview: None,
            manage_gerbers_modal: None,
            component: Default::default(),
        }
    }

    pub fn update_pcb_overview(&mut self, pcb_overview: PcbOverview) {
        if let Some(modal) = &mut self.manage_gerbers_modal {
            modal.update_gerbers(&pcb_overview.gerbers)
        }

        self.pcb_overview.replace(pcb_overview);
    }
}

#[derive(Debug, Clone)]
pub enum ConfigurationUiCommand {
    None,
    ManageGerbersClicked { design_index: usize },
    ManageGerbersModalUiCommand(ManagerGerbersModalUiCommand),
}

#[derive(Debug, Clone)]
pub enum ConfigurationUiAction {
    None,
    AddGerberFiles {
        path: PathBuf,
        design: DesignName,
        files: Vec<(PathBuf, Option<PcbSide>, GerberPurpose)>,
    },
    RemoveGerberFiles {
        path: PathBuf,
        design: DesignName,
        files: Vec<PathBuf>,
    },
}

#[derive(Debug, Clone, Default)]
pub struct ConfigurationUiContext {}

impl UiComponent for ConfigurationUi {
    type UiContext<'context> = ConfigurationUiContext;
    type UiCommand = ConfigurationUiCommand;
    type UiAction = ConfigurationUiAction;

    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        ui.label(tr!("pcb-configuration-header"));

        if let Some(pcb_overview) = &self.pcb_overview {
            ui.label(tr!("pcb-configuration-detail-name", { name: &pcb_overview.name }));
        } else {
            ui.spinner();
            return;
        };

        ui.separator();

        //
        // designs table
        //

        self.show_designs(ui);

        //
        // Modals
        //
        if let Some(dialog) = &self.manage_gerbers_modal {
            dialog.ui(ui, &mut ());
        }
    }

    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            ConfigurationUiCommand::None => Some(ConfigurationUiAction::None),
            ConfigurationUiCommand::ManageGerbersClicked {
                design_index,
            } => {
                self.show_manage_gerbers_modal(design_index);
                None
            }
            ConfigurationUiCommand::ManageGerbersModalUiCommand(command) => {
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
                                Some(ConfigurationUiAction::RemoveGerberFiles {
                                    path: pcb_overview.path.clone(),
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
                                Some(ConfigurationUiAction::AddGerberFiles {
                                    path: pcb_overview.path.clone(),
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
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Default, PartialEq)]
pub struct ConfigurationTab {}

impl Tab for ConfigurationTab {
    type Context = PcbTabContext;

    fn label(&self) -> WidgetText {
        egui::widget_text::WidgetText::from(tr!("pcb-configuration-tab-label"))
    }

    fn ui<'a>(&mut self, ui: &mut Ui, _tab_key: &TabKey, context: &mut Self::Context) {
        let state = context.state.lock().unwrap();
        UiComponent::ui(&state.configuration_ui, ui, &mut ConfigurationUiContext::default());
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, _context: &mut Self::Context) -> bool {
        true
    }
}
