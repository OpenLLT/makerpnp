use std::path::PathBuf;

use derivative::Derivative;
use egui::Modal;
use egui_extras::Column;
use egui_i18n::tr;
use egui_mobius::Value;
use planner_app::{DesignName, GerberPurpose, PcbGerberItem, PcbSide};

use crate::file_picker::Picker;
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct ManageGerbersModal {
    design_index: usize,
    design_name: DesignName,
    gerbers: Vec<PcbGerberItem>,

    #[derivative(Debug = "ignore")]
    file_picker: Value<Picker>,

    pub component: ComponentState<ManagerGerbersModalUiCommand>,
}

impl ManageGerbersModal {
    pub fn new(design_index: usize, design_name: DesignName, gerbers: Vec<PcbGerberItem>) -> Self {
        Self {
            design_index,
            design_name,
            gerbers,
            component: Default::default(),
            file_picker: Default::default(),
        }
    }

    // the first vector is the designs, the second is the gerbers for the design
    pub fn update_gerbers(&mut self, gerbers: &[Vec<PcbGerberItem>]) {
        let design_gerbers = &gerbers[self.design_index];
        self.gerbers = design_gerbers.clone();
    }
}

#[derive(Debug, Clone)]
pub enum ManagerGerbersModalUiCommand {
    Close,
    Remove { index: usize },
    Add,
    GerberFilesPicked { picked_files: Vec<PathBuf> },
}

#[derive(Debug, Clone)]
pub enum ManagerGerberModalAction {
    CloseDialog,
    RemoveGerberFiles { design_index: usize, files: Vec<PathBuf> },
    AddGerberFiles { design_index: usize, files: Vec<PathBuf> },
}

impl UiComponent for ManageGerbersModal {
    type UiContext<'context> = ();
    type UiCommand = ManagerGerbersModalUiCommand;
    type UiAction = ManagerGerberModalAction;

    fn ui<'context>(&self, ui: &mut egui::Ui, _context: &mut Self::UiContext<'context>) {
        if let Ok(picked_files) = self
            .file_picker
            .lock()
            .unwrap()
            .picked_multi()
        {
            self.component
                .send(ManagerGerbersModalUiCommand::GerberFilesPicked {
                    picked_files,
                });
        }

        ui.ctx().style_mut(|style| {
            // if this is not done, text in labels/checkboxes/etc wraps when using taffy
            style.wrap_mode = Some(egui::TextWrapMode::Extend);
        });

        let modal_id = ui.id().with("manage_gerbers_modal");

        Modal::new(modal_id).show(ui.ctx(), |ui| {
            ui.set_width(ui.available_width() * 0.8);

            ui.heading(tr!("modal-manager-gerbers-title", { design: self.design_name.to_string() }));

            let text_height = egui::TextStyle::Body
                .resolve(ui.style())
                .size
                .max(ui.spacing().interact_size.y);

            // FUTURE perhaps a data-table would be better here?
            egui_extras::TableBuilder::new(ui)
                .striped(true)
                .column(Column::auto())
                .column(Column::remainder())
                .column(Column::auto())
                .column(Column::auto())
                .column(Column::auto())
                .header(text_height, |mut header| {
                    header.col(|ui| {
                        ui.strong(tr!("table-gerbers-column-index"));
                    });
                    header.col(|ui| {
                        ui.strong(tr!("table-gerbers-column-file"));
                    });
                    header.col(|ui| {
                        ui.strong(tr!("table-gerbers-column-pcb-side"));
                    });
                    header.col(|ui| {
                        ui.strong(tr!("table-gerbers-column-gerber-purpose"));
                    });
                    header.col(|ui| {
                        ui.strong(tr!("table-gerbers-column-actions"));
                    });
                })
                .body(|mut body| {
                    for (
                        index,
                        PcbGerberItem {
                            pcb_side,
                            path,
                            purpose,
                        },
                    ) in self.gerbers.iter().enumerate()
                    {
                        body.row(text_height, |mut row| {
                            row.col(|ui| {
                                ui.label(index.to_string());
                            });
                            row.col(|ui| {
                                ui.label(
                                    path.file_stem()
                                        .unwrap()
                                        .to_string_lossy()
                                        .to_string(),
                                );
                            });
                            row.col(|ui| {
                                // TODO replace label with a dropdown to allow the user to change the side
                                let pcb_side_choice = GerberPcbSideChoice::from(pcb_side);
                                let label = match pcb_side_choice {
                                    GerberPcbSideChoice::Both => tr!("pcb-side-both"),
                                    GerberPcbSideChoice::Top => tr!("pcb-side-top"),
                                    GerberPcbSideChoice::Bottom => tr!("pcb-side-bottom"),
                                };
                                ui.label(label);
                            });
                            row.col(|ui| {
                                // TODO replace label with a dropdown to allow the user to change the side
                                let label = match purpose {
                                    GerberPurpose::Assembly => tr!("gerber-purpose-assembly"),
                                    GerberPurpose::Copper => tr!("gerber-purpose-copper"),
                                    GerberPurpose::Legend => tr!("gerber-purpose-legend"),
                                    GerberPurpose::PasteMask => tr!("gerber-purpose-pastemask"),
                                    GerberPurpose::Other => tr!("gerber-purpose-other"),
                                    GerberPurpose::SolderMask => tr!("gerber-purpose-soldermask"),
                                };
                                ui.label(label);
                            });
                            row.col(|ui| {
                                if ui
                                    .button(tr!("form-button-remove"))
                                    .clicked()
                                {
                                    self.component
                                        .send(ManagerGerbersModalUiCommand::Remove {
                                            index,
                                        });
                                }
                            });
                        })
                    }
                });

            egui::Sides::new().show(
                ui,
                |ui| {
                    if ui
                        .button(tr!("form-button-add"))
                        .clicked()
                    {
                        self.component
                            .send(ManagerGerbersModalUiCommand::Add);
                    }
                },
                |ui| {
                    if ui
                        .button(tr!("form-button-close"))
                        .clicked()
                    {
                        self.component
                            .send(ManagerGerbersModalUiCommand::Close);
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
            ManagerGerbersModalUiCommand::Close => Some(ManagerGerberModalAction::CloseDialog),
            ManagerGerbersModalUiCommand::Remove {
                index,
            } => {
                let files = vec![self.gerbers[index].path.clone()];
                Some(ManagerGerberModalAction::RemoveGerberFiles {
                    design_index: self.design_index,
                    files,
                })
            }
            ManagerGerbersModalUiCommand::Add => {
                self.file_picker
                    .lock()
                    .unwrap()
                    .pick_files();
                None
            }
            ManagerGerbersModalUiCommand::GerberFilesPicked {
                picked_files,
            } => Some(ManagerGerberModalAction::AddGerberFiles {
                design_index: self.design_index,
                files: picked_files,
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum GerberPcbSideChoice {
    Both,
    Top,
    Bottom,
}

impl From<GerberPcbSideChoice> for Option<PcbSide> {
    fn from(value: GerberPcbSideChoice) -> Self {
        match value {
            GerberPcbSideChoice::Top => Some(PcbSide::Top),
            GerberPcbSideChoice::Bottom => Some(PcbSide::Bottom),
            GerberPcbSideChoice::Both => None,
        }
    }
}

impl From<&Option<PcbSide>> for GerberPcbSideChoice {
    fn from(value: &Option<PcbSide>) -> Self {
        match value {
            None => GerberPcbSideChoice::Both,
            Some(PcbSide::Top) => GerberPcbSideChoice::Top,
            Some(PcbSide::Bottom) => GerberPcbSideChoice::Bottom,
        }
    }
}
