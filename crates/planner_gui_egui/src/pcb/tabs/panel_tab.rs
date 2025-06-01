use std::collections::HashMap;
use std::fmt::Debug;
use std::io::BufWriter;
use std::mem;
use std::ops::Add;
use std::sync::mpsc::Sender;

use derivative::Derivative;
use eframe::emath::Vec2;
use egui::scroll_area::ScrollBarVisibility;
use egui::{Resize, Ui, WidgetText};
use egui_extras::{Column, TableBuilder};
use egui_i18n::tr;
use egui_mobius::Value;
use egui_mobius::types::ValueGuard;
use gerber_viewer::gerber_types::{
    Aperture, Circle, Command, CoordinateFormat, GerberCode, GerberError, InterpolationMode, Unit,
};
use gerber_viewer::position::{Position, Vector};
use num_rational::Ratio;
use planner_app::{PcbOverview, PcbSide, PcbUnitIndex};
use tracing::{debug, trace};

use crate::pcb::tabs::PcbTabContext;
use crate::pcb::tabs::panel_tab::gerber_builder::GerberBuilder;
use crate::pcb::tabs::panel_tab::gerber_util::{
    gerber_line_commands, gerber_point_commands, gerber_rectangle_commands,
};
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};
use crate::ui_components::gerber_viewer_ui::{
    GerberViewerMode, GerberViewerUi, GerberViewerUiAction, GerberViewerUiCommand, GerberViewerUiContext,
    GerberViewerUiInstanceArgs,
};
use crate::ui_util::ratio_of_f64;

#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DragSliderParameters {
    pub speed: f64,
    pub fixed_decimals: usize,
}

pub mod defaults {
    use std::sync::LazyLock;

    use egui::ahash::HashMap;
    use gerber_viewer::gerber_types::Unit;

    use super::DragSliderParameters;

    pub static DRAG_SLIDER: LazyLock<HashMap<Unit, DragSliderParameters>> = LazyLock::new(|| {
        HashMap::from_iter([
            (Unit::Millimeters, DragSliderParameters {
                speed: 0.1,
                fixed_decimals: 3,
            }),
            (Unit::Inches, DragSliderParameters {
                speed: 0.1,
                fixed_decimals: 4,
            }),
        ])
    });
}

#[derive(Default, Debug, Clone, PartialEq, PartialOrd)]
pub struct Dimensions<T: Default + Debug + Clone + PartialEq + PartialOrd> {
    left: T,
    right: T,
    top: T,
    bottom: T,
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct DesignSizing {
    origin: Vector,
    offset: Vector,
    size: GerberSize,
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct UnitConfiguration {
    offset: Vector,
    /// clockwise positive radians
    rotation: f64,
}

#[derive(Derivative, Debug, Clone, PartialEq)]
#[derivative(Default)]
pub struct PanelSizing {
    #[derivative(Default(value = "Unit::Millimeters"))]
    units: Unit,

    #[derivative(Default(value = "GerberSize::new(100.0, 100.0)"))]
    size: GerberSize,

    #[derivative(Default(value = "Dimensions { left: 5.0, right: 5.0, top: 5.0, bottom: 5.0 }"))]
    edge_rails: Dimensions<f64>,

    fiducials: Vec<FiducialParameters>,
    design_sizings: Vec<DesignSizing>,
    unit_configurations: Vec<UnitConfiguration>,
}

impl PanelSizing {
    pub fn ensure_design_sizings(&mut self, design_count: usize) {
        self.design_sizings
            .resize_with(design_count, Default::default);
    }

    pub fn ensure_unit_configurations(&mut self, unit_count: u16) {
        self.unit_configurations
            .resize_with(unit_count as usize, Default::default);
    }
}

// TODO move this to the gerber_viewer crate
#[derive(Default, Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct GerberSize {
    // not using terms like length/width/height because they are ambiguous
    x: f64,
    y: f64,
}

impl GerberSize {
    pub fn new(x: f64, y: f64) -> Self {
        Self {
            x,
            y,
        }
    }
}

#[derive(Derivative, Debug)]
#[derivative(Default)]
struct PanelTabUiState {
    #[derivative(Default(value = "PcbSide::Top"))]
    pcb_side: PcbSide,
    new_fiducial: FiducialParameters,
}

#[derive(Debug, Derivative, Copy, Clone, PartialEq, PartialOrd)]
#[derivative(Default)]
pub struct FiducialParameters {
    position: Position,
    #[derivative(Default(value = "2.0"))]
    mask_diameter: f64,
    #[derivative(Default(value = "1.0"))]
    copper_diameter: f64,
}

impl FiducialParameters {
    pub fn copper_to_mask_ratio(&self) -> Option<Ratio<i64>> {
        ratio_of_f64(self.copper_diameter, self.mask_diameter)
    }
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct PanelTabUi {
    pcb_overview: Option<PcbOverview>,
    panel_sizing: Option<PanelSizing>,

    // TODO don't use a value unless we need to
    panel_tab_ui_state: Value<PanelTabUiState>,

    #[derivative(Debug = "ignore")]
    gerber_viewer_ui: Value<GerberViewerUi>,

    pub component: ComponentState<PanelTabUiCommand>,
}

impl PanelTabUi {
    const TABLE_HEIGHT_MAX: f32 = 400.0;
    const TABLE_SCROLL_HEIGHT_MIN: f32 = 30.0;

    pub fn new() -> Self {
        let component: ComponentState<PanelTabUiCommand> = Default::default();

        // FIXME probably the gerber viewer UI needs different args now
        let args = GerberViewerUiInstanceArgs {
            mode: GerberViewerMode::Panel,
        };

        let mut gerber_viewer_ui = GerberViewerUi::new(args);
        gerber_viewer_ui
            .component
            .configure_mapper(component.sender.clone(), |gerber_viewer_command| {
                trace!("gerber_viewer mapper. command: {:?}", gerber_viewer_command);
                PanelTabUiCommand::GerberViewerUiCommand(gerber_viewer_command)
            });

        let mut instance = Self {
            panel_sizing: None,
            pcb_overview: None,
            panel_tab_ui_state: Value::default(),
            gerber_viewer_ui: Value::new(gerber_viewer_ui),
            component,
        };

        // TODO remove this
        let panel_sizing = Default::default();
        instance.update_panel(panel_sizing);

        instance
    }

    pub fn update_pcb_overview(&mut self, pcb_overview: PcbOverview) {
        if let Some(panel_sizing) = &mut self.panel_sizing {
            panel_sizing.ensure_design_sizings(pcb_overview.designs.len());
            panel_sizing.ensure_unit_configurations(pcb_overview.units);
        }

        self.pcb_overview.replace(pcb_overview);
    }

    pub fn update_panel(&mut self, panel_sizing: PanelSizing) {
        self.panel_sizing.replace(panel_sizing);
        self.update_panel_preview();
    }

    fn left_panel_content(
        ui: &mut Ui,
        panel_sizing: &PanelSizing,
        state: ValueGuard<PanelTabUiState>,
        sender: Sender<PanelTabUiCommand>,
        pcb_overview: &PcbOverview,
    ) {
        let text_height = egui::TextStyle::Body
            .resolve(ui.style())
            .size
            .max(ui.spacing().interact_size.y);

        egui::ScrollArea::both().show(ui, |ui| {
            // TODO let the user choose units

            Self::top_bottom_controls(&state, &sender, ui);
            ui.separator();
            Self::panel_size_controls(&panel_sizing, &sender, ui);
            ui.separator();
            Self::edge_rails_controls(&panel_sizing, &sender, ui);
            ui.separator();
            Self::fiducials_controls(panel_sizing, state, &sender, text_height, ui);
            ui.separator();
            Self::design_configuration_controls(&panel_sizing, pcb_overview, &sender, text_height, ui);
            ui.separator();
            Self::unit_positions_controls(&panel_sizing, &pcb_overview, &sender, text_height, ui);
        });
    }

    /// show top/bottom selector
    fn top_bottom_controls(state: &ValueGuard<PanelTabUiState>, sender: &Sender<PanelTabUiCommand>, ui: &mut Ui) {
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

            // TODO add mirroring vertical/horizontal dropdown for top/bottom
            // TODO add rotation for panel?
        });
    }

    fn panel_size_controls(panel_sizing: &PanelSizing, sender: &Sender<PanelTabUiCommand>, ui: &mut Ui) {
        ui.label(tr!("pcb-panel-tab-panel-size-header"));

        let mut size = panel_sizing.size;

        ui.horizontal(|ui| {
            ui.label(tr!("form-common-input-x"));
            ui.add(
                egui::DragValue::new(&mut size.x)
                    .range(0.0..=f64::MAX)
                    .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                    .fixed_decimals(defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals),
            );
        });
        ui.horizontal(|ui| {
            ui.label(tr!("form-common-input-y"));
            ui.add(
                egui::DragValue::new(&mut size.y)
                    .range(0.0..=f64::MAX)
                    .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                    .fixed_decimals(defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals),
            );
        });

        if !size.eq(&panel_sizing.size) {
            sender
                .send(PanelTabUiCommand::SizeChanged(size))
                .expect("sent");
        }
    }

    fn edge_rails_controls(panel_sizing: &PanelSizing, sender: &Sender<PanelTabUiCommand>, ui: &mut Ui) {
        let mut edge_rails = panel_sizing.edge_rails.clone();

        ui.label(tr!("pcb-panel-tab-panel-edge-rails-header"));

        egui::Grid::new("edge_rails_grid")
            .num_columns(3)
            .show(ui, |ui| {
                ui.label(""); // placeholder
                ui.horizontal(|ui| {
                    ui.label(tr!("form-common-input-top"));
                    ui.add(
                        egui::DragValue::new(&mut edge_rails.top)
                            .range(0.0..=f64::MAX)
                            .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                            .fixed_decimals(defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals),
                    );
                });
                ui.label(""); // placeholder
                ui.end_row();

                ui.horizontal(|ui| {
                    ui.label(tr!("form-common-input-left"));
                    ui.add(
                        egui::DragValue::new(&mut edge_rails.left)
                            .range(0.0..=f64::MAX)
                            .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                            .fixed_decimals(defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals),
                    );
                });
                ui.label(""); // placeholder
                ui.horizontal(|ui| {
                    ui.label(tr!("form-common-input-right"));
                    ui.add(
                        egui::DragValue::new(&mut edge_rails.right)
                            .range(0.0..=f64::MAX)
                            .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                            .fixed_decimals(defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals),
                    );
                });
                ui.end_row();

                ui.label(""); // placeholder
                ui.horizontal(|ui| {
                    ui.label(tr!("form-common-input-bottom"));
                    ui.add(
                        egui::DragValue::new(&mut edge_rails.bottom)
                            .range(0.0..=f64::MAX)
                            .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                            .fixed_decimals(defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals),
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
    }

    fn fiducials_controls(
        panel_sizing: &PanelSizing,
        state: ValueGuard<PanelTabUiState>,
        sender: &Sender<PanelTabUiCommand>,
        text_height: f32,
        ui: &mut Ui,
    ) {
        ui.label(tr!("pcb-panel-tab-panel-fiducials-header"));

        ui.horizontal(|ui| {
            let mut new_fiducial = state.new_fiducial;

            ui.label(tr!("form-common-input-x"));
            ui.add(
                egui::DragValue::new(&mut new_fiducial.position.x)
                    .range(0.0..=f64::MAX)
                    .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                    .fixed_decimals(defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals),
            );
            ui.label(tr!("form-common-input-y"));
            ui.add(
                egui::DragValue::new(&mut new_fiducial.position.y)
                    .range(0.0..=f64::MAX)
                    .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                    .fixed_decimals(defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals),
            );

            ui.label(tr!("form-common-input-copper-diameter"));
            ui.add(
                egui::DragValue::new(&mut new_fiducial.copper_diameter)
                    .range(0.0..=f64::MAX)
                    .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                    .fixed_decimals(defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals),
            );

            ui.label(tr!("form-common-input-mask-diameter"));
            ui.add(
                egui::DragValue::new(&mut new_fiducial.mask_diameter)
                    .range(0.0..=f64::MAX)
                    .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                    .fixed_decimals(defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals),
            );

            if let Some(ratio) = new_fiducial.copper_to_mask_ratio() {
                ui.label(tr!("ratio", {numerator: ratio.numer(), denominator: ratio.denom() }));
            } else {
                ui.label(tr!("ratio-error"));
            }

            if !new_fiducial.eq(&state.new_fiducial) {
                sender
                    .send(PanelTabUiCommand::NewFiducialChanged(new_fiducial))
                    .expect("sent");
            }

            if ui
                .button(tr!("form-common-button-add"))
                .clicked()
            {
                sender
                    .send(PanelTabUiCommand::AddFiducial(new_fiducial))
                    .expect("sent");
            }
        });

        ui.push_id("fiducials", |ui| {
            let initial_size = calculate_initial_table_height(panel_sizing.fiducials.len(), text_height, ui);

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
                                        ui.strong(tr!("table-fiducials-column-index"));
                                    });
                                    header.col(|ui| {
                                        ui.strong(tr!("table-fiducials-column-x"));
                                    });
                                    header.col(|ui| {
                                        ui.strong(tr!("table-fiducials-column-y"));
                                    });
                                    header.col(|ui| {
                                        ui.strong(tr!("table-fiducials-column-mask-diameter"));
                                    });
                                    header.col(|ui| {
                                        ui.strong(tr!("table-fiducials-column-copper-diameter"));
                                    });
                                    header.col(|ui| {
                                        ui.strong(tr!("table-fiducials-column-actions"));
                                    });
                                })
                                .body(|mut body| {
                                    for (fiducial_index, parameters) in panel_sizing
                                        .fiducials
                                        .iter()
                                        .enumerate()
                                    {
                                        let mut new_parameters = parameters.clone();

                                        body.row(text_height, |mut row| {
                                            row.col(|ui| {
                                                ui.label((fiducial_index + 1).to_string());
                                            });

                                            row.col(|ui| {
                                                ui.add(
                                                    egui::DragValue::new(&mut new_parameters.position.x)
                                                        .range(0.0..=f64::MAX)
                                                        .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                                                        .fixed_decimals(
                                                            defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals,
                                                        ),
                                                );
                                            });
                                            row.col(|ui| {
                                                ui.add(
                                                    egui::DragValue::new(&mut new_parameters.position.y)
                                                        .range(0.0..=f64::MAX)
                                                        .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                                                        .fixed_decimals(
                                                            defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals,
                                                        ),
                                                );
                                            });
                                            row.col(|ui| {
                                                ui.add(
                                                    egui::DragValue::new(&mut new_parameters.mask_diameter)
                                                        .range(0.0..=f64::MAX)
                                                        .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                                                        .fixed_decimals(
                                                            defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals,
                                                        ),
                                                );
                                            });
                                            row.col(|ui| {
                                                ui.add(
                                                    egui::DragValue::new(&mut new_parameters.copper_diameter)
                                                        .range(0.0..=f64::MAX)
                                                        .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                                                        .fixed_decimals(
                                                            defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals,
                                                        ),
                                                );
                                            });

                                            row.col(|ui| {
                                                if ui
                                                    .button(tr!("form-common-button-delete"))
                                                    .clicked()
                                                {
                                                    sender
                                                        .send(PanelTabUiCommand::DeleteFiducial(fiducial_index))
                                                        .expect("sent");
                                                }
                                            });
                                        });

                                        if !new_parameters.eq(parameters) {
                                            sender
                                                .send(PanelTabUiCommand::UpdateFiducial {
                                                    index: fiducial_index,
                                                    parameters: new_parameters,
                                                })
                                                .expect("sent");
                                        }
                                    }
                                });
                        });
                });
        });
    }

    fn design_configuration_controls(
        panel_sizing: &PanelSizing,
        pcb_overview: &PcbOverview,
        sender: &Sender<PanelTabUiCommand>,
        text_height: f32,
        ui: &mut Ui,
    ) {
        ui.label(tr!("pcb-panel-tab-panel-design-configuration-header"));

        ui.push_id("design_configuration", |ui| {
            let initial_size = calculate_initial_table_height(pcb_overview.designs.len(), text_height, ui);

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
                                        ui.strong(tr!("table-design-layout-column-x-size"));
                                    });
                                    header.col(|ui| {
                                        ui.strong(tr!("table-design-layout-column-y-size"));
                                    });
                                    header.col(|ui| {
                                        ui.strong(tr!("table-design-layout-column-design-name"));
                                    });

                                    // TODO maybe add per-design mirroring?
                                })
                                .body(|mut body| {
                                    for (design_index, design_name) in pcb_overview.designs.iter().enumerate() {
                                        let design_number = design_index + 1;

                                        let Some(mut design_sizing) = panel_sizing
                                            .design_sizings
                                            .get(design_index)
                                            .cloned()
                                        else {
                                            continue;
                                        };

                                        body.row(text_height, |mut row| {
                                            row.col(|ui| {
                                                ui.label(design_number.to_string());
                                            });

                                            row.col(|ui| {
                                                ui.add(
                                                    egui::DragValue::new(&mut design_sizing.offset.x)
                                                        .range(0.0..=f64::MAX)
                                                        .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                                                        .fixed_decimals(
                                                            defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals,
                                                        ),
                                                );
                                            });
                                            row.col(|ui| {
                                                ui.add(
                                                    egui::DragValue::new(&mut design_sizing.offset.y)
                                                        .range(0.0..=f64::MAX)
                                                        .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                                                        .fixed_decimals(
                                                            defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals,
                                                        ),
                                                );
                                            });
                                            row.col(|ui| {
                                                ui.add(
                                                    egui::DragValue::new(&mut design_sizing.origin.x)
                                                        .range(0.0..=f64::MAX)
                                                        .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                                                        .fixed_decimals(
                                                            defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals,
                                                        ),
                                                );
                                            });
                                            row.col(|ui| {
                                                ui.add(
                                                    egui::DragValue::new(&mut design_sizing.origin.y)
                                                        .range(0.0..=f64::MAX)
                                                        .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                                                        .fixed_decimals(
                                                            defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals,
                                                        ),
                                                );
                                            });
                                            row.col(|ui| {
                                                ui.add(
                                                    egui::DragValue::new(&mut design_sizing.size.x)
                                                        .range(0.0..=f64::MAX)
                                                        .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                                                        .fixed_decimals(
                                                            defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals,
                                                        ),
                                                );
                                            });
                                            row.col(|ui| {
                                                ui.add(
                                                    egui::DragValue::new(&mut design_sizing.size.y)
                                                        .range(0.0..=f64::MAX)
                                                        .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                                                        .fixed_decimals(
                                                            defaults::DRAG_SLIDER[&panel_sizing.units].fixed_decimals,
                                                        ),
                                                );
                                            });

                                            row.col(|ui| {
                                                ui.label(design_name.to_string());
                                            });
                                        });

                                        if design_sizing != panel_sizing.design_sizings[design_index] {
                                            sender
                                                .send(PanelTabUiCommand::DesignSizingChanged {
                                                    design_index,
                                                    design_sizing,
                                                })
                                                .expect("sent");
                                        }
                                    }
                                });
                        });
                });
        });
    }

    fn unit_positions_controls(
        panel_sizing: &PanelSizing,
        pcb_overview: &PcbOverview,
        sender: &Sender<PanelTabUiCommand>,
        text_height: f32,
        ui: &mut Ui,
    ) {
        ui.label(tr!("pcb-panel-tab-panel-unit-positions-header"));

        ui.push_id("unit_positions", |ui| {
            let initial_size = calculate_initial_table_height(pcb_overview.units as usize, text_height, ui);

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
                                            let Some(mut unit_configuration) = panel_sizing
                                                .unit_configurations
                                                .get(pcb_unit_index as usize)
                                                .cloned()
                                            else {
                                                continue;
                                            };

                                            let design_name = &pcb_overview.designs[*assigned_design_index];

                                            body.row(text_height, |mut row| {
                                                row.col(|ui| {
                                                    ui.label((pcb_unit_index + 1).to_string());
                                                });

                                                row.col(|ui| {
                                                    ui.add(
                                                        egui::DragValue::new(&mut unit_configuration.offset.x)
                                                            .range(0.0..=f64::MAX)
                                                            .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                                                            .fixed_decimals(
                                                                defaults::DRAG_SLIDER[&panel_sizing.units]
                                                                    .fixed_decimals,
                                                            ),
                                                    );
                                                });
                                                row.col(|ui| {
                                                    ui.add(
                                                        egui::DragValue::new(&mut unit_configuration.offset.y)
                                                            .range(0.0..=f64::MAX)
                                                            .speed(defaults::DRAG_SLIDER[&panel_sizing.units].speed)
                                                            .fixed_decimals(
                                                                defaults::DRAG_SLIDER[&panel_sizing.units]
                                                                    .fixed_decimals,
                                                            ),
                                                    );
                                                });
                                                row.col(|ui| {
                                                    // TODO Rotation
                                                });

                                                row.col(|ui| {
                                                    ui.label(design_name.to_string());
                                                });
                                            });

                                            if !unit_configuration
                                                .eq(&panel_sizing.unit_configurations[pcb_unit_index as usize])
                                            {
                                                sender
                                                    .send(PanelTabUiCommand::UnitConfigurationChanged {
                                                        pcb_unit_index,
                                                        unit_configuration,
                                                    })
                                                    .expect("sent");
                                            }
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
    }

    fn central_panel_content(ui: &mut Ui, gerber_viewer_ui: &mut GerberViewerUi) {
        gerber_viewer_ui.ui(ui, &mut GerberViewerUiContext::default());
    }

    fn update_panel_preview(&mut self) {
        let (Some(panel_sizing), Some(pcb_overview)) = (&self.panel_sizing, &self.pcb_overview) else {
            return;
        };

        let mut gerber_viewer_ui = self.gerber_viewer_ui.lock().unwrap();

        if let Ok(commands) = build_panel_preview_commands(panel_sizing, pcb_overview) {
            dump_gerber_source(&commands);
            gerber_viewer_ui.use_single_layer(commands);
        } else {
            // TODO show an error message if the gerber preview could not be generated
        }
    }
}

#[derive(Debug, Clone)]
pub enum PanelTabUiCommand {
    None,
    PcbSideChanged(PcbSide),
    NewFiducialChanged(FiducialParameters),
    AddFiducial(FiducialParameters),
    DeleteFiducial(usize),
    UpdateFiducial {
        index: usize,
        parameters: FiducialParameters,
    },
    SizeChanged(GerberSize),
    EdgeRailsChanged(Dimensions<f64>),
    GerberViewerUiCommand(GerberViewerUiCommand),
    DesignSizingChanged {
        design_index: usize,
        design_sizing: DesignSizing,
    },
    UnitConfigurationChanged {
        pcb_unit_index: PcbUnitIndex,
        unit_configuration: UnitConfiguration,
    },
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

        egui::SidePanel::left(ui.id().with("left_panel"))
            .resizable(true)
            .show_inside(ui, |ui| {
                // specifically NON-mutable state here
                let state = self.panel_tab_ui_state.lock().unwrap();
                let sender = self.component.sender.clone();

                Self::left_panel_content(ui, &panel_sizing, state, sender, &pcb_overview);
            });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            // specifically NON-mutable state here
            let mut gerber_viewer_ui = self.gerber_viewer_ui.lock().unwrap();

            Self::central_panel_content(ui, &mut gerber_viewer_ui);
        });
    }

    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        debug!("PanelTabUi::update, command: {:?}", command);
        let mut update_panel_preview = false;
        let action = match command {
            PanelTabUiCommand::None => Some(PanelTabUiAction::None),
            PanelTabUiCommand::PcbSideChanged(pcb_side) => {
                let mut state = self.panel_tab_ui_state.lock().unwrap();
                state.pcb_side = pcb_side;
                update_panel_preview = true;
                None
            }
            PanelTabUiCommand::SizeChanged(size) => {
                if let Some(panel_sizing) = &mut self.panel_sizing {
                    panel_sizing.size = size;
                    update_panel_preview = true;
                }
                None
            }
            PanelTabUiCommand::EdgeRailsChanged(edge_rails) => {
                if let Some(panel_sizing) = &mut self.panel_sizing {
                    panel_sizing.edge_rails = edge_rails;
                    update_panel_preview = true;
                }
                None
            }
            PanelTabUiCommand::NewFiducialChanged(parameters) => {
                let mut state = self.panel_tab_ui_state.lock().unwrap();
                state.new_fiducial = parameters;
                update_panel_preview = true;
                None
            }
            PanelTabUiCommand::AddFiducial(parameters) => {
                if let Some(panel_sizing) = &mut self.panel_sizing {
                    panel_sizing.fiducials.push(parameters);
                    update_panel_preview = true;
                }
                None
            }
            PanelTabUiCommand::DeleteFiducial(index) => {
                if let Some(panel_sizing) = &mut self.panel_sizing {
                    panel_sizing.fiducials.remove(index);
                    update_panel_preview = true;
                }
                None
            }
            PanelTabUiCommand::UpdateFiducial {
                index,
                parameters,
            } => {
                if let Some(panel_sizing) = &mut self.panel_sizing {
                    panel_sizing.fiducials[index] = parameters;
                    update_panel_preview = true;
                }
                None
            }
            PanelTabUiCommand::DesignSizingChanged {
                design_index,
                design_sizing,
            } => {
                if let Some(panel_sizing) = &mut self.panel_sizing {
                    panel_sizing.design_sizings[design_index] = design_sizing;
                    update_panel_preview = true;
                }
                None
            }
            PanelTabUiCommand::UnitConfigurationChanged {
                pcb_unit_index,
                unit_configuration,
            } => {
                if let Some(panel_sizing) = &mut self.panel_sizing {
                    panel_sizing.unit_configurations[pcb_unit_index as usize] = unit_configuration;
                    update_panel_preview = true;
                }
                None
            }
            PanelTabUiCommand::GerberViewerUiCommand(command) => {
                let mut gerber_viewer_ui = self.gerber_viewer_ui.lock().unwrap();
                let action = gerber_viewer_ui.update(command, &mut GerberViewerUiContext::default());
                match action {
                    None => None,
                    Some(GerberViewerUiAction::None) => None,
                }
            }
        };

        if update_panel_preview {
            self.update_panel_preview();
        }

        action
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

fn calculate_initial_table_height(item_count: usize, text_height: f32, ui: &mut Ui) -> Vec2 {
    // FIXME this calculation isn't exact, but it's close enough for now
    //       the size the resize widget so it contains the table precisely.
    let initial_height = (text_height + 11.0) + ((text_height + 3.0) * item_count as f32);
    let initial_width = ui.available_width();
    let initial_size = egui::Vec2::new(initial_width, initial_height);
    initial_size
}

fn dump_gerber_source(commands: &Vec<Command>) {
    let gerber_source = gerber_commands_to_source(commands);

    debug!("Gerber source:\n{}", gerber_source);
}

fn gerber_commands_to_source(commands: &Vec<Command>) -> String {
    let mut buf = BufWriter::new(Vec::new());
    commands
        .serialize(&mut buf)
        .expect("Could not generate Gerber code");
    let bytes = buf.into_inner().unwrap();
    let gerber_source = String::from_utf8(bytes).unwrap();
    gerber_source
}

fn build_panel_preview_commands(
    panel_sizing: &PanelSizing,
    pcb_overview: &PcbOverview,
) -> Result<Vec<Command>, GerberError> {
    // FUTURE generate multiple, real, gerber layers instead of a 'preview' layer
    //        i.e. 'board outline, top mask, top copper layers, bottom mask, bottom copper layer, v-score/cut (rails)'

    let coordinate_format = CoordinateFormat::new(4, 6);

    let mut gerber_builder = GerberBuilder::new()
        .with_units(panel_sizing.units)
        .with_coordinate_format(coordinate_format);

    gerber_builder.set_interpolation_mode(InterpolationMode::Linear);
    let drawing_aperture_code = gerber_builder.define_aperture(Aperture::Circle(Circle {
        diameter: 0.1,
        hole_diameter: None,
    }));

    gerber_builder.select_aperture(drawing_aperture_code);
    let origin = Position::new(0.0, 0.0);

    gerber_builder.push_commands(gerber_rectangle_commands(coordinate_format, origin, panel_sizing.size)?);

    //
    // rails
    //
    if panel_sizing.edge_rails.left > 0.0 {
        let start = origin.add_x(panel_sizing.edge_rails.left);
        let end = start.add_y(panel_sizing.size.y);
        let rail_commands = gerber_line_commands(coordinate_format, start, end)?;
        gerber_builder.push_commands(rail_commands);
    }
    if panel_sizing.edge_rails.right > 0.0 {
        let start = origin.add_x(panel_sizing.size.x - panel_sizing.edge_rails.right);
        let end = start.add_y(panel_sizing.size.y);
        let rail_commands = gerber_line_commands(coordinate_format, start, end)?;
        gerber_builder.push_commands(rail_commands);
    }

    if panel_sizing.edge_rails.bottom > 0.0 {
        let start = origin.add_y(panel_sizing.edge_rails.bottom);
        let end = start.add_x(panel_sizing.size.x);
        let rail_commands = gerber_line_commands(coordinate_format, start, end)?;
        gerber_builder.push_commands(rail_commands);
    }

    if panel_sizing.edge_rails.top > 0.0 {
        let start = origin.add_y(panel_sizing.size.y - panel_sizing.edge_rails.top);
        let end = start.add_x(panel_sizing.size.x);
        let rail_commands = gerber_line_commands(coordinate_format, start, end)?;
        gerber_builder.push_commands(rail_commands);
    }

    //
    // units
    //
    for (pcb_unit_index, unit_configuration) in panel_sizing
        .unit_configurations
        .iter()
        .enumerate()
    {
        let Some(design_index) = pcb_overview
            .unit_map
            .get(&(pcb_unit_index as PcbUnitIndex))
        else {
            continue;
        };

        let design_sizing = &panel_sizing.design_sizings[*design_index];

        let unit_origin = origin
            .add_x(unit_configuration.offset.x)
            .add_y(unit_configuration.offset.y);
        let unit_size = design_sizing.size;

        // TODO support rotation
        let unit_rotation = unit_configuration.rotation;

        gerber_builder.push_commands(gerber_rectangle_commands(coordinate_format, unit_origin, unit_size)?);
    }

    //
    // fiducials
    //

    for fiducial in &panel_sizing.fiducials {
        let aperture_code = gerber_builder.define_circle_with_hole(fiducial.mask_diameter, fiducial.copper_diameter);
        gerber_builder.select_aperture(aperture_code);
        gerber_builder.push_commands(gerber_point_commands(coordinate_format, fiducial.position)?);
    }

    Ok(gerber_builder.as_commands())
}

mod gerber_util {
    use gerber_viewer::gerber_types::{
        Command, CoordinateFormat, CoordinateNumber, Coordinates, DCode, FunctionCode, GerberError, Operation,
    };
    use gerber_viewer::position::Position;

    use crate::pcb::tabs::panel_tab::GerberSize;

    #[allow(dead_code)]
    pub fn x_y_to_gerber(x: f64, y: f64, format: CoordinateFormat) -> Result<Coordinates, GerberError> {
        let x = CoordinateNumber::try_from(x)?;
        let y = CoordinateNumber::try_from(y)?;

        x.gerber(&format)?;
        y.gerber(&format)?;

        Ok(Coordinates {
            x: Some(x),
            y: Some(y),
            format,
        })
    }

    #[allow(dead_code)]
    pub fn is_valid(value: f64, format: &CoordinateFormat) -> bool {
        let Ok(value) = CoordinateNumber::try_from(value) else {
            return false;
        };

        value.gerber(format).is_ok()
    }

    pub fn gerber_rectangle_commands(
        coordinate_format: CoordinateFormat,
        origin: Position,
        size: GerberSize,
    ) -> Result<Vec<Command>, GerberError> {
        Ok(vec![
            Command::FunctionCode(FunctionCode::DCode(DCode::Operation(Operation::Move(x_y_to_gerber(
                origin.x,
                origin.y,
                coordinate_format,
            )?)))),
            Command::FunctionCode(FunctionCode::DCode(DCode::Operation(Operation::Interpolate(
                x_y_to_gerber(origin.x + size.x, origin.y, coordinate_format)?,
                None,
            )))),
            Command::FunctionCode(FunctionCode::DCode(DCode::Operation(Operation::Interpolate(
                x_y_to_gerber(origin.x + size.x, origin.y + size.y, coordinate_format)?,
                None,
            )))),
            Command::FunctionCode(FunctionCode::DCode(DCode::Operation(Operation::Interpolate(
                x_y_to_gerber(origin.x, origin.y + size.y, coordinate_format)?,
                None,
            )))),
            Command::FunctionCode(FunctionCode::DCode(DCode::Operation(Operation::Interpolate(
                x_y_to_gerber(origin.x, origin.y, coordinate_format)?,
                None,
            )))),
        ])
    }

    pub fn gerber_line_commands(
        coordinate_format: CoordinateFormat,
        origin: Position,
        end: Position,
    ) -> Result<Vec<Command>, GerberError> {
        Ok(vec![
            Command::FunctionCode(FunctionCode::DCode(DCode::Operation(Operation::Move(x_y_to_gerber(
                origin.x,
                origin.y,
                coordinate_format,
            )?)))),
            Command::FunctionCode(FunctionCode::DCode(DCode::Operation(Operation::Interpolate(
                x_y_to_gerber(end.x, end.y, coordinate_format)?,
                None,
            )))),
        ])
    }

    pub fn gerber_point_commands(
        coordinate_format: CoordinateFormat,
        origin: Position,
    ) -> Result<Vec<Command>, GerberError> {
        Ok(vec![Command::FunctionCode(FunctionCode::DCode(DCode::Operation(
            Operation::Flash(x_y_to_gerber(origin.x, origin.y, coordinate_format)?),
        )))])
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use gerber_viewer::gerber_types::Unit;
    use gerber_viewer::position::{Position, Vector};
    use indoc::indoc;
    use planner_app::PcbOverview;

    use crate::pcb::tabs::panel_tab::{
        DesignSizing, Dimensions, FiducialParameters, GerberSize, PanelSizing, UnitConfiguration,
        build_panel_preview_commands, gerber_commands_to_source,
    };

    #[test]
    pub fn test_build_panel_preview_layer() {
        // given
        let panel_sizing = PanelSizing {
            size: GerberSize {
                x: 100.0,
                y: 80.0,
            },
            edge_rails: Dimensions {
                left: 5.0,
                right: 10.0,
                top: 6.0,
                bottom: 12.0,
            },
            unit_configurations: vec![
                UnitConfiguration {
                    offset: Vector::new(5.0, 12.0),
                    rotation: 0.0,
                },
                UnitConfiguration {
                    offset: Vector::new(55.0, 12.0),
                    rotation: 0.0,
                },
                UnitConfiguration {
                    offset: Vector::new(5.0, 40.0),
                    rotation: 0.0,
                },
                UnitConfiguration {
                    offset: Vector::new(55.0, 40.0),
                    rotation: 0.0,
                },
            ],
            design_sizings: vec![
                DesignSizing {
                    origin: Default::default(),
                    offset: Default::default(),
                    size: GerberSize::new(30.0, 25.0),
                },
                DesignSizing {
                    origin: Default::default(),
                    offset: Default::default(),
                    size: GerberSize::new(40.0, 20.0),
                },
            ],
            fiducials: vec![
                // bottom row
                FiducialParameters {
                    position: Position::new(10.0, 12.0 / 2.0),
                    mask_diameter: 2.0,
                    copper_diameter: 1.0,
                },
                FiducialParameters {
                    position: Position::new(90.0, 12.0 / 2.0),
                    mask_diameter: 2.0,
                    copper_diameter: 1.0,
                },
                // top row
                FiducialParameters {
                    position: Position::new(20.0, 80.0 - 3.0),
                    mask_diameter: 2.0,
                    copper_diameter: 1.0,
                },
                FiducialParameters {
                    position: Position::new(90.0, 80.0 - 3.0),
                    mask_diameter: 2.0,
                    copper_diameter: 1.0,
                },
            ],
            units: Unit::Millimeters,
        };

        // and
        let pcb_overview = PcbOverview {
            path: Default::default(),
            name: "".to_string(),
            units: 4,
            designs: vec!["DESIGN_A".to_string(), "DESIGN_B".to_string()],
            unit_map: HashMap::from_iter([(0, 0), (1, 1), (2, 0), (2, 1)]),
            pcb_gerbers: vec![],
            design_gerbers: vec![],
        };

        // and
        let expected_source = indoc!(
            r#"
            %FSLAX24Y24*%
            %MOMM*%
            %ADD10C,0.1*%
            G01*
            D10*
            X0Y0D02*
            X100000Y0D01*
            X100000Y100000D01*
            X0Y100000D01*
            X0Y0D01*
        "#
        );

        // when
        let commands = build_panel_preview_commands(&panel_sizing, &pcb_overview).unwrap();

        // then
        let source = gerber_commands_to_source(&commands);
        println!("{}", source);

        assert_eq!(source, expected_source);
    }
}

#[allow(dead_code)]
mod gerber_builder {
    use std::collections::HashMap;

    use gerber_viewer::gerber_types::{
        Aperture, ApertureDefinition, Circle, Command, CoordinateFormat, DCode, ExtendedCode, FunctionCode, GCode,
        InterpolationMode, Unit,
    };
    use num_traits::FromPrimitive;
    use rust_decimal::Decimal;

    pub struct GerberBuilder {
        commands: Vec<Command>,

        next_aperture_code: i32,
        current_aperture_code: Option<i32>,

        circle_apertures: HashMap<(Decimal, Decimal), i32>,
    }

    impl GerberBuilder {
        pub fn new() -> Self {
            Self {
                commands: Vec::new(),
                current_aperture_code: None,
                next_aperture_code: 10,
                circle_apertures: HashMap::new(),
            }
        }

        pub fn next_aperture_code(&mut self) -> i32 {
            let result = self.next_aperture_code;
            self.next_aperture_code += 1;
            result
        }

        pub fn push_commands(&mut self, commands: Vec<Command>) {
            self.commands.extend(commands);
        }

        pub fn push_command(&mut self, command: Command) {
            self.commands.push(command);
        }

        pub fn with_units(mut self, units: Unit) -> Self {
            self.commands
                .push(Command::ExtendedCode(ExtendedCode::Unit(units)));
            self
        }

        pub fn with_coordinate_format(mut self, coordinate_format: CoordinateFormat) -> Self {
            self.commands
                .push(Command::ExtendedCode(ExtendedCode::CoordinateFormat(coordinate_format)));
            self
        }

        pub fn set_interpolation_mode(&mut self, interpolation_mode: InterpolationMode) {
            self.commands
                .push(Command::FunctionCode(FunctionCode::GCode(GCode::InterpolationMode(
                    interpolation_mode,
                ))))
        }

        pub fn define_aperture(&mut self, aperture: Aperture) -> i32 {
            let code = self.next_aperture_code();
            let definition = ApertureDefinition {
                code,
                aperture,
            };
            self.commands
                .push(Command::ExtendedCode(ExtendedCode::ApertureDefinition(definition)));
            code
        }

        pub fn select_aperture(&mut self, code: i32) {
            match self.current_aperture_code {
                Some(current_code) if code == current_code => {}
                _ => {
                    self.current_aperture_code.replace(code);
                    self.commands
                        .push(Command::FunctionCode(FunctionCode::DCode(DCode::SelectAperture(code))))
                }
            }
        }

        /// defines a circle with a hole
        /// if the circle is already defined, the previously allocated aperture code is returned
        pub fn define_circle_with_hole(&mut self, outer_diameter: f64, inner_diameter: f64) -> i32 {
            let key = (
                Decimal::from_f64(outer_diameter).unwrap(),
                Decimal::from_f64(inner_diameter).unwrap(),
            );

            if let Some(&code) = self.circle_apertures.get(&key) {
                return code;
            }

            let code = self.define_aperture(Aperture::Circle(Circle {
                diameter: outer_diameter,
                hole_diameter: Some(inner_diameter),
            }));

            self.circle_apertures.insert(key, code);
            code
        }

        pub fn as_commands(self) -> Vec<Command> {
            self.commands
        }
    }
}
