use std::fmt::Debug;

use derivative::Derivative;
use egui::scroll_area::ScrollBarVisibility;
use egui::{Resize, Ui, WidgetText};
use egui_extras::{Column, TableBuilder};
use egui_i18n::tr;
use egui_mobius::Value;
use gerber_viewer::position::Position;
use planner_app::{PcbOverview, PcbSide};

use crate::pcb::tabs::PcbTabContext;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Default, Debug, Clone, PartialEq, PartialOrd)]
pub struct Dimensions<T: Default + Debug + Clone + PartialEq + PartialOrd> {
    left: T,
    right: T,
    top: T,
    bottom: T,
}

#[derive(Default, Debug, Clone, PartialEq, PartialOrd)]
pub struct PanelSizing {
    size: GerberSize,
    edge_rails: Dimensions<f64>,

    fiducials: Vec<Position>,
}

#[derive(Default, Debug, Clone, PartialEq, PartialOrd)]
pub struct GerberSize {
    // not using terms like length/width/height because they are ambiguous
    x: f64,
    y: f64,
}

#[derive(Derivative, Debug)]
#[derivative(Default)]
struct PanelTabUiState {
    #[derivative(Default(value = "PcbSide::Top"))]
    pcb_side: PcbSide,
    new_fiducial_x: f64,
    new_fiducial_y: f64,
}

#[derive(Debug)]
pub struct PanelTabUi {
    pcb_overview: Option<PcbOverview>,
    panel_sizing: Option<PanelSizing>,

    // TODO don't use a value unless we need to
    panel_tab_ui_state: Value<PanelTabUiState>,

    pub component: ComponentState<PanelTabUiCommand>,
}

impl PanelTabUi {
    const TABLE_HEIGHT_MAX: f32 = 400.0;
    const TABLE_SCROLL_HEIGHT_MIN: f32 = 30.0;

    pub fn new() -> Self {
        Self {
            // TODO default this to none, wait for it to be given
            panel_sizing: Some(Default::default()),
            pcb_overview: None,
            panel_tab_ui_state: Value::default(),

            component: Default::default(),
        }
    }

    pub fn update_pcb_overview(&mut self, pcb_overview: PcbOverview) {
        self.pcb_overview.replace(pcb_overview);
    }

    pub fn update_panel(&mut self, panel_sizing: PanelSizing) {
        self.panel_sizing.replace(panel_sizing);
    }
}

#[derive(Debug, Clone)]
pub enum PanelTabUiCommand {
    None,
    PcbSideChanged(PcbSide),
    NewFiducialChanged(f64, f64),
    AddFiducial(f64, f64),
    DeleteFiducial(usize),
    UpdateFiducial { index: usize, position: Position },
    SizeChanged(GerberSize),
    EdgeRailsChanged(Dimensions<f64>),
}

#[derive(Debug, Clone)]
pub enum PanelTabUiAction {
    None,
}

#[derive(Debug, Clone, Default)]
pub struct PanelTabUiContext {}

impl UiComponent for PanelTabUi {
    type UiContext<'context> = PanelTabUiContext;
    type UiCommand = PanelTabUiCommand;
    type UiAction = PanelTabUiAction;

    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        let (Some(pcb_overview), Some(panel_sizing)) = (&self.pcb_overview, &self.panel_sizing) else {
            ui.spinner();
            return;
        };

        // specifically NON-mutable state here
        let state = self.panel_tab_ui_state.lock().unwrap();

        let sender = self.component.sender.clone();

        let text_height = egui::TextStyle::Body
            .resolve(ui.style())
            .size
            .max(ui.spacing().interact_size.y);

        egui::SidePanel::left(ui.id().with("left_panel"))
            .resizable(true)
            .show_inside(ui, |ui| {
                egui::ScrollArea::both().show(ui, |ui| {
                    //
                    // show top/bottom
                    //

                    ui.horizontal(|ui| {
                        ui.label(tr!("pcb-panel-tab-input-show-top-bottom"));

                        egui::ComboBox::from_id_salt(ui.id().with("pcb_side"))
                            .width(ui.available_width())
                            .selected_text(match state.pcb_side {
                                PcbSide::Top => tr!("form-common-choice-pcb-side-top"),
                                PcbSide::Bottom => tr!("form-common-choice-pcb-side-bottom"),
                            })
                            .show_ui(ui, |ui| {
                                if ui
                                    .add(egui::SelectableLabel::new(
                                        state.pcb_side == PcbSide::Top,
                                        tr!("form-common-choice-pcb-side-top"),
                                    ))
                                    .clicked()
                                {
                                    sender
                                        .send(PanelTabUiCommand::PcbSideChanged(PcbSide::Top))
                                        .expect("sent");
                                }
                                if ui
                                    .add(egui::SelectableLabel::new(
                                        state.pcb_side == PcbSide::Bottom,
                                        tr!("form-common-choice-pcb-side-bottom"),
                                    ))
                                    .clicked()
                                {
                                    sender
                                        .send(PanelTabUiCommand::PcbSideChanged(PcbSide::Bottom))
                                        .expect("sent");
                                }
                            });
                    });

                    ui.separator();

                    // TODO add mirroring vertical/horizontal dropdown for top/bottom
                    // TODO add rotation for panel?

                    //
                    // panel size
                    //
                    ui.label(tr!("pcb-panel-tab-panel-size-header"));

                    let mut size = panel_sizing.size.clone();

                    ui.horizontal(|ui| {
                        ui.label(tr!("form-common-input-x"));
                        ui.add(
                            egui::DragValue::new(&mut size.x)
                                .range(0.0..=f64::MAX)
                                .max_decimals(4),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label(tr!("form-common-input-y"));
                        ui.add(
                            egui::DragValue::new(&mut size.y)
                                .range(0.0..=f64::MAX)
                                .max_decimals(4),
                        );
                    });

                    if !size.eq(&panel_sizing.size) {
                        sender
                            .send(PanelTabUiCommand::SizeChanged(size))
                            .expect("sent");
                    }

                    ui.separator();

                    //
                    // edge rails
                    //
                    let mut edge_rails = panel_sizing.edge_rails.clone();

                    ui.label(tr!("pcb-panel-tab-panel-edge-rails-header"));

                    egui::Grid::new("edge_rails_grid").show(ui, |ui| {
                        ui.label(""); // placeholder
                        ui.horizontal(|ui| {
                            ui.label(tr!("form-common-input-top"));
                            ui.add(
                                egui::DragValue::new(&mut edge_rails.top)
                                    .range(0.0..=f64::MAX)
                                    .max_decimals(4),
                            );
                        });
                        ui.label(""); // placeholder
                        ui.end_row();

                        ui.horizontal(|ui| {
                            ui.label(tr!("form-common-input-left"));
                            ui.add(
                                egui::DragValue::new(&mut edge_rails.left)
                                    .range(0.0..=f64::MAX)
                                    .max_decimals(4),
                            );
                        });
                        ui.label(""); // placeholder
                        ui.horizontal(|ui| {
                            ui.label(tr!("form-common-input-right"));
                            ui.add(
                                egui::DragValue::new(&mut edge_rails.right)
                                    .range(0.0..=f64::MAX)
                                    .max_decimals(4),
                            );
                        });
                        ui.end_row();

                        ui.label(""); // placeholder
                        ui.horizontal(|ui| {
                            ui.label(tr!("form-common-input-bottom"));
                            ui.add(
                                egui::DragValue::new(&mut edge_rails.bottom)
                                    .range(0.0..=f64::MAX)
                                    .max_decimals(4),
                            );
                        });
                        ui.label(""); // placeholder
                        ui.end_row();
                    });

                    if !edge_rails.eq(&panel_sizing.edge_rails) {
                        sender
                            .send(PanelTabUiCommand::EdgeRailsChanged(edge_rails))
                            .expect("sent");
                    }

                    ui.separator();

                    //
                    // Fiducials
                    //
                    ui.label(tr!("pcb-panel-tab-panel-fiducials-header"));

                    ui.horizontal(|ui| {
                        let mut new_fiducial_x = state.new_fiducial_x;
                        let mut new_fiducial_y = state.new_fiducial_y;

                        ui.label(tr!("form-common-input-x"));
                        ui.add(
                            egui::DragValue::new(&mut new_fiducial_x)
                                .range(0.0..=f64::MAX)
                                .max_decimals(4),
                        );
                        ui.label(tr!("form-common-input-y"));
                        ui.add(
                            egui::DragValue::new(&mut new_fiducial_y)
                                .range(0.0..=f64::MAX)
                                .max_decimals(4),
                        );

                        if new_fiducial_x != state.new_fiducial_x || new_fiducial_y != state.new_fiducial_y {
                            sender
                                .send(PanelTabUiCommand::NewFiducialChanged(new_fiducial_x, new_fiducial_y))
                                .expect("sent");
                        }

                        if ui
                            .button(tr!("form-common-button-add"))
                            .clicked()
                        {
                            sender
                                .send(PanelTabUiCommand::AddFiducial(new_fiducial_x, new_fiducial_y))
                                .expect("sent");
                        }
                    });

                    ui.push_id("fiducials", |ui| {
                        // FIXME this calculation isn't exact, but it's close enough for now
                        //       the size the resize widget so it contains the table precisely.
                        let initial_height =
                            (text_height * 8.0) + ((text_height + 6.0) * panel_sizing.fiducials.len() as f32);
                        let initial_width = ui.available_width();
                        let initial_size = egui::Vec2::new(initial_width, initial_height);

                        Resize::default()
                            .resizable([false, true])
                            .default_size(initial_size)
                            .min_width(ui.available_width())
                            .max_width(ui.available_width())
                            .max_height(Self::TABLE_HEIGHT_MAX)
                            .show(ui, |ui| {
                                // HACK: search codebase for 'HACK: table-resize-hack' for details
                                egui::Frame::new()
                                    .outer_margin(4.0)
                                    .show(ui, |ui| {
                                        ui.style_mut()
                                            .interaction
                                            .selectable_labels = false;

                                        TableBuilder::new(ui)
                                            .striped(true)
                                            .resizable(true)
                                            .auto_shrink([false, false])
                                            .min_scrolled_height(Self::TABLE_SCROLL_HEIGHT_MIN)
                                            .scroll_bar_visibility(ScrollBarVisibility::AlwaysVisible)
                                            .sense(egui::Sense::click())
                                            .column(Column::auto())
                                            .column(Column::auto())
                                            .column(Column::auto())
                                            .column(Column::remainder())
                                            .header(20.0, |mut header| {
                                                header.col(|ui| {
                                                    ui.strong(tr!("table-fiducials-column-index"));
                                                });
                                                header.col(|ui| {
                                                    ui.strong(tr!("table-fiducials-column-x"));
                                                });
                                                header.col(|ui| {
                                                    ui.strong(tr!("table-fiducials-column-y"));
                                                });
                                                header.col(|ui| {
                                                    ui.strong(tr!("table-fiducials-column-actions"));
                                                });
                                            })
                                            .body(|mut body| {
                                                for (fiducial_index, position) in panel_sizing
                                                    .fiducials
                                                    .iter()
                                                    .enumerate()
                                                {
                                                    let mut new_position = position.clone();

                                                    body.row(text_height, |mut row| {
                                                        row.col(|ui| {
                                                            ui.label((fiducial_index + 1).to_string());
                                                        });

                                                        row.col(|ui| {
                                                            ui.add(
                                                                egui::DragValue::new(&mut new_position.x)
                                                                    .range(0.0..=f64::MAX)
                                                                    .max_decimals(4),
                                                            );
                                                        });
                                                        row.col(|ui| {
                                                            ui.add(
                                                                egui::DragValue::new(&mut new_position.y)
                                                                    .range(0.0..=f64::MAX)
                                                                    .max_decimals(4),
                                                            );
                                                        });

                                                        row.col(|ui| {
                                                            if ui
                                                                .button(tr!("form-common-button-delete"))
                                                                .clicked()
                                                            {
                                                                sender
                                                                    .send(PanelTabUiCommand::DeleteFiducial(
                                                                        fiducial_index,
                                                                    ))
                                                                    .expect("sent");
                                                            }
                                                        });
                                                    });

                                                    if !new_position.eq(position) {
                                                        sender
                                                            .send(PanelTabUiCommand::UpdateFiducial {
                                                                index: fiducial_index,
                                                                position: new_position,
                                                            })
                                                            .expect("sent");
                                                    }
                                                }
                                            });
                                    });
                            });
                    });

                    ui.separator();

                    //
                    // design configuration
                    //
                    ui.label(tr!("pcb-panel-tab-panel-design-configuration-header"));

                    ui.push_id("design_configuration", |ui| {
                        // FIXME this calculation isn't exact, but it's close enough for now
                        //       the size the resize widget so it contains the table precisely.
                        let initial_height =
                            (text_height + 10.0) + ((text_height + 5.0) * pcb_overview.designs.len() as f32);
                        let initial_width = ui.available_width();
                        let initial_size = egui::Vec2::new(initial_width, initial_height);

                        Resize::default()
                            .resizable([false, true])
                            .default_size(initial_size)
                            .min_width(ui.available_width())
                            .max_width(ui.available_width())
                            .max_height(Self::TABLE_HEIGHT_MAX)
                            .show(ui, |ui| {
                                // HACK: search codebase for 'HACK: table-resize-hack' for details
                                egui::Frame::new()
                                    .outer_margin(4.0)
                                    .show(ui, |ui| {
                                        ui.style_mut()
                                            .interaction
                                            .selectable_labels = false;

                                        TableBuilder::new(ui)
                                            .striped(true)
                                            .resizable(true)
                                            .auto_shrink([false, false])
                                            .min_scrolled_height(Self::TABLE_SCROLL_HEIGHT_MIN)
                                            .scroll_bar_visibility(ScrollBarVisibility::AlwaysVisible)
                                            .sense(egui::Sense::click())
                                            .column(Column::auto())
                                            .column(Column::auto())
                                            .column(Column::auto())
                                            .column(Column::auto())
                                            .column(Column::auto())
                                            .column(Column::remainder())
                                            .header(20.0, |mut header| {
                                                header.col(|ui| {
                                                    ui.strong(tr!("table-design-layout-column-index"));
                                                });
                                                header.col(|ui| {
                                                    ui.strong(tr!("table-design-layout-column-x-offset"));
                                                });
                                                header.col(|ui| {
                                                    ui.strong(tr!("table-design-layout-column-y-offset"));
                                                });
                                                header.col(|ui| {
                                                    ui.strong(tr!("table-design-layout-column-x-origin"));
                                                });
                                                header.col(|ui| {
                                                    ui.strong(tr!("table-design-layout-column-y-origin"));
                                                });
                                                header.col(|ui| {
                                                    ui.strong(tr!("table-design-layout-column-design-name"));
                                                });

                                                // TODO maybe add per-design mirroring?
                                            })
                                            .body(|mut body| {
                                                for (design_index, design_name) in
                                                    pcb_overview.designs.iter().enumerate()
                                                {
                                                    let design_number = design_index + 1;

                                                    body.row(text_height, |mut row| {
                                                        row.col(|ui| {
                                                            ui.label(design_number.to_string());
                                                        });

                                                        row.col(|ui| {
                                                            // TODO X offset
                                                        });
                                                        row.col(|ui| {
                                                            // TODO Y offset
                                                        });
                                                        row.col(|ui| {
                                                            // TODO X origin
                                                        });
                                                        row.col(|ui| {
                                                            // TODO Y origin
                                                        });

                                                        row.col(|ui| {
                                                            ui.label(design_name.to_string());
                                                        });
                                                    });
                                                }
                                            });
                                    });
                            });
                    });

                    ui.separator();

                    //
                    // unit positions
                    //
                    ui.label(tr!("pcb-panel-tab-panel-unit-positions-header"));

                    ui.push_id("unit_positions", |ui| {
                        // FIXME this calculation isn't exact, but it's close enough for now
                        //       the size the resize widget so it contains the table precisely.
                        let initial_height = (text_height * 8.0) + ((text_height + 6.0) * pcb_overview.units as f32);
                        let initial_width = ui.available_width();
                        let initial_size = egui::Vec2::new(initial_width, initial_height);

                        Resize::default()
                            .resizable([false, true])
                            .default_size(initial_size)
                            .min_width(ui.available_width())
                            .max_width(ui.available_width())
                            .max_height(Self::TABLE_HEIGHT_MAX)
                            .show(ui, |ui| {
                                // HACK: search codebase for 'HACK: table-resize-hack' for details
                                egui::Frame::new()
                                    .outer_margin(4.0)
                                    .show(ui, |ui| {
                                        ui.style_mut()
                                            .interaction
                                            .selectable_labels = false;

                                        TableBuilder::new(ui)
                                            .striped(true)
                                            .resizable(true)
                                            .auto_shrink([false, false])
                                            .min_scrolled_height(Self::TABLE_SCROLL_HEIGHT_MIN)
                                            .scroll_bar_visibility(ScrollBarVisibility::AlwaysVisible)
                                            .sense(egui::Sense::click())
                                            .column(Column::auto())
                                            .column(Column::auto())
                                            .column(Column::auto())
                                            .column(Column::auto())
                                            .column(Column::remainder())
                                            .header(20.0, |mut header| {
                                                header.col(|ui| {
                                                    ui.strong(tr!("table-unit-positions-column-index"));
                                                });
                                                header.col(|ui| {
                                                    ui.strong(tr!("table-unit-positions-column-x"));
                                                });
                                                header.col(|ui| {
                                                    ui.strong(tr!("table-unit-positions-column-y"));
                                                });
                                                header.col(|ui| {
                                                    ui.strong(tr!("table-unit-positions-column-rotation"));
                                                });
                                                header.col(|ui| {
                                                    ui.strong(tr!("table-unit-positions-column-design-name"));
                                                });
                                            })
                                            .body(|mut body| {
                                                for pcb_unit_index in 0..pcb_overview.units {
                                                    if let Some(assigned_design_index) = pcb_overview
                                                        .unit_map
                                                        .get(&pcb_unit_index)
                                                    {
                                                        let design_name = &pcb_overview.designs[*assigned_design_index];

                                                        body.row(text_height, |mut row| {
                                                            row.col(|ui| {
                                                                ui.label((pcb_unit_index + 1).to_string());
                                                            });

                                                            row.col(|ui| {
                                                                // TODO X
                                                            });
                                                            row.col(|ui| {
                                                                // TODO Y
                                                            });
                                                            row.col(|ui| {
                                                                // TODO Rotation
                                                            });

                                                            row.col(|ui| {
                                                                ui.label(design_name.to_string());
                                                            });
                                                        });
                                                    } else {
                                                        body.row(text_height, |mut row| {
                                                            row.col(|ui| {
                                                                ui.label((pcb_unit_index + 1).to_string());
                                                            });
                                                            row.col(|ui| {
                                                                ui.label(tr!("common-value-not-available"));
                                                            });
                                                            row.col(|ui| {
                                                                ui.label(tr!("common-value-not-available"));
                                                            });
                                                            row.col(|ui| {
                                                                ui.label(tr!("common-value-not-available"));
                                                            });
                                                        })
                                                    }
                                                }
                                            });
                                    });
                            });
                    });
                });
            });
    }

    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            PanelTabUiCommand::None => Some(PanelTabUiAction::None),
            PanelTabUiCommand::PcbSideChanged(pcb_side) => {
                let mut state = self.panel_tab_ui_state.lock().unwrap();
                state.pcb_side = pcb_side;
                None
            }
            PanelTabUiCommand::SizeChanged(size) => {
                if let Some(panel_sizing) = &mut self.panel_sizing {
                    panel_sizing.size = size;
                }
                None
            }
            PanelTabUiCommand::EdgeRailsChanged(edge_rails) => {
                if let Some(panel_sizing) = &mut self.panel_sizing {
                    panel_sizing.edge_rails = edge_rails;
                }
                None
            }
            PanelTabUiCommand::NewFiducialChanged(x, y) => {
                let mut state = self.panel_tab_ui_state.lock().unwrap();
                state.new_fiducial_x = x;
                state.new_fiducial_y = y;
                None
            }
            PanelTabUiCommand::AddFiducial(x, y) => {
                if let Some(panel_sizing) = &mut self.panel_sizing {
                    panel_sizing
                        .fiducials
                        .push(Position::new(x, y));
                }
                None
            }
            PanelTabUiCommand::DeleteFiducial(index) => {
                if let Some(panel_sizing) = &mut self.panel_sizing {
                    panel_sizing.fiducials.remove(index);
                }
                None
            }
            PanelTabUiCommand::UpdateFiducial {
                index,
                position,
            } => {
                if let Some(panel_sizing) = &mut self.panel_sizing {
                    panel_sizing.fiducials[index] = position;
                }
                None
            }
        }
    }
}

#[derive(Default, Debug, serde::Deserialize, serde::Serialize)]
pub struct PanelTab {}

impl Tab for PanelTab {
    type Context = PcbTabContext;

    fn label(&self) -> WidgetText {
        let title = tr!("pcb-panel-tab-label");

        egui::widget_text::WidgetText::from(title)
    }

    fn ui<'a>(&mut self, ui: &mut Ui, _tab_key: &TabKey, context: &mut Self::Context) {
        let state = context.state.lock().unwrap();
        UiComponent::ui(&state.panel_tab_ui, ui, &mut PanelTabUiContext::default());
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, _context: &mut Self::Context) -> bool {
        true
    }
}
