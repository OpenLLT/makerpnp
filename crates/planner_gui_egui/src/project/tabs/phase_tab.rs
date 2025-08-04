use derivative::Derivative;
use egui::{Ui, Vec2, WidgetText};
use egui_dock::tab_viewer::OnCloseResponse;
use egui_i18n::tr;
use planner_app::{
    ObjectPath, OperationReference, OperationStatus, PcbSide, PhaseOverview, PhasePlacements, PhaseReference,
    PlacementPositionUnit, PlacementState, Reference, TaskAction, TaskReference, TaskStatus,
};
use regex::Regex;
use tracing::{debug, trace};

use crate::i18n::conversions::{process_operation_status_to_i18n_key, process_task_status_to_i18n_key};
use crate::project::dialogs::placement_orderings::{
    PlacementOrderingsArgs, PlacementOrderingsModal, PlacementOrderingsModalAction, PlacementOrderingsModalUiCommand,
};
use crate::project::process::build_task_actions;
use crate::project::tables::placements::{
    PlacementsTableUi, PlacementsTableUiAction, PlacementsTableUiCommand, PlacementsTableUiContext,
};
use crate::project::tabs::ProjectTabContext;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};
use crate::ui_util::green_orange_red_grey_from_style;

#[derive(Derivative)]
#[derivative(Debug)]
pub struct PhaseTabUi {
    overview: Option<PhaseOverview>,
    #[derivative(Debug = "ignore")]
    placements_table_ui: PlacementsTableUi,

    placement_orderings_modal: Option<PlacementOrderingsModal>,

    pub component: ComponentState<PhaseTabUiCommand>,
}

impl PhaseTabUi {
    pub fn new() -> Self {
        let component: ComponentState<PhaseTabUiCommand> = Default::default();

        let mut placements_table_ui = PlacementsTableUi::new();
        placements_table_ui
            .component
            .configure_mapper(component.sender.clone(), |placements_table_command| {
                trace!("phase placements table mapper. command: {:?}", placements_table_command);
                PhaseTabUiCommand::PlacementsTableUiCommand(placements_table_command)
            });

        Self {
            overview: None,
            placements_table_ui,
            placement_orderings_modal: None,
            component,
        }
    }

    pub fn update_overview(&mut self, phase_overview: PhaseOverview) {
        self.overview.replace(phase_overview);
    }

    pub fn update_placements(&mut self, phase_placements: PhasePlacements, phases: Vec<PhaseOverview>) {
        self.placements_table_ui
            .update_placements(phase_placements.placements, phases);
    }
}

#[derive(Debug, Clone)]
pub enum PhaseTabUiCommand {
    None,
    PlacementsTableUiCommand(PlacementsTableUiCommand),
    AddPartsToLoadout {
        phase: PhaseReference,
        manufacturer_pattern: Regex,
        mpn_pattern: Regex,
    },
    PhasePlacementsOrderingsClicked,
    PlacementOrderingsModalUiCommand(PlacementOrderingsModalUiCommand),
    TaskAction {
        operation: OperationReference,
        task: TaskReference,
        action: TaskAction,
    },
}

#[derive(Debug, Clone)]
pub enum PhaseTabUiAction {
    None,
    RequestRepaint,
    UpdatePlacement {
        object_path: ObjectPath,
        new_placement: PlacementState,
        old_placement: PlacementState,
    },
    AddPartsToLoadout {
        phase: PhaseReference,
        manufacturer_pattern: Regex,
        mpn_pattern: Regex,
    },
    SetPlacementOrderings(PlacementOrderingsArgs),
    TaskAction {
        phase: PhaseReference,
        operation: OperationReference,
        task: TaskReference,
        action: TaskAction,
    },
    LocatePlacement {
        /// Full object path of the component
        object_path: ObjectPath,

        pcb_side: PcbSide,
        design_position: PlacementPositionUnit,
        unit_position: PlacementPositionUnit,
    },
}

#[derive(Debug, Clone, Default)]
pub struct PhaseTabUiContext {}

impl UiComponent for PhaseTabUi {
    type UiContext<'context> = PhaseTabUiContext;
    type UiCommand = PhaseTabUiCommand;
    type UiAction = PhaseTabUiAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        ui.label(tr!("phase-placements-header"));

        //
        // State operation progress
        //
        ui.horizontal(|ui| {
            let (green, orange, red, grey) = green_orange_red_grey_from_style(ui.style());

            if let Some(overview) = &self.overview {
                let mut previous_operation_status = None;
                for (index, operation_state) in overview
                    .state
                    .operation_states
                    .iter()
                    .enumerate()
                {
                    trace!("operation state: {:?}, index: {}", operation_state, index);
                    if index > 0 {
                        ui.label(">");
                    }

                    let operation_status = operation_state.status();

                    ui.label(operation_state.reference.to_string());

                    let mut previous_task_status = None;
                    for (task_index, (task_reference, task_state)) in operation_state
                        .task_states
                        .iter()
                        .enumerate()
                    {
                        trace!("task state: {:?}, index: {}", task_state, task_index);
                        let task_status = task_state.status();

                        // scope the task ui, to prevent id clash if two comboboxes are displayed
                        // FIXME this which should never happen since it's caused by invalid state.
                        //       fix the planner core to prevent placements from being placed unless the task state is started.
                        ui.push_id(task_index, |ui| {
                            let color = match task_status {
                                TaskStatus::Pending => grey,
                                TaskStatus::Started => orange,
                                TaskStatus::Complete => green,
                                TaskStatus::Abandoned => red,
                            };
                            if task_index > 0 {
                                ui.label("+");
                            }
                            ui.colored_label(color, task_reference.to_string());

                            let status = tr!(process_task_status_to_i18n_key(&task_status));

                            if let Some(actions) = build_task_actions(
                                &previous_operation_status,
                                &operation_status,
                                &previous_task_status,
                                &task_status,
                                task_state.can_complete(),
                            ) {
                                egui::ComboBox::from_id_salt(ui.id().with("kind"))
                                    .selected_text(status)
                                    .show_ui(ui, |ui| {
                                        for action in actions {
                                            if ui
                                                .add(egui::Button::selectable(
                                                    false,
                                                    format!("{:?}", action).to_string(),
                                                ))
                                                .clicked()
                                            {
                                                debug!("clicked: {:?}", action);
                                                self.component
                                                    .send(PhaseTabUiCommand::TaskAction {
                                                        operation: operation_state.reference.clone(),
                                                        task: task_reference.clone(),
                                                        action,
                                                    });
                                            }
                                        }
                                    });
                            } else {
                                ui.colored_label(color, status);
                            }
                        });
                        previous_task_status = Some(task_status);
                    }

                    ui.label("=");

                    let color = match operation_status {
                        OperationStatus::Pending => grey,
                        OperationStatus::Started => orange,
                        OperationStatus::Complete => green,
                        OperationStatus::Abandoned => red,
                    };
                    let status = tr!(process_operation_status_to_i18n_key(&operation_status));
                    ui.colored_label(color, status);

                    previous_operation_status = Some(operation_status);
                }
            }
        });

        ui.separator();

        //
        // Toolbar
        //

        ui.horizontal(|ui| {
            let toolbar_height_id = ui.id().with("toolbar_height");
            let toolbar_height = ui.ctx().memory_mut(|mem| {
                mem.data
                    .get_temp::<f32>(toolbar_height_id)
            });

            if let Some(toolbar_height) = toolbar_height {
                ui.set_min_height(toolbar_height);
            }

            // FIXME due to some vertical padding issue, we wrap the buttons and a separator in a horizontal layout...
            //       remove the ui.horizontal() and fix the issue properly, no idea where the spacing above the filter
            //       is coming from. various attempts at adjusting the style all failed.
            //       an alternative workaround is to display the filter first, then the buttons... but we WANT the buttons first.

            ui.horizontal(|ui| {
                let toolbar_element_min_size =
                    Vec2::new(0.0, ui.spacing().interact_size.y + crate::filter::MARGIN.sum().y);

                if ui
                    .add(
                        egui::Button::new(tr!("phase-toolbar-add-parts-to-loadout")).min_size(toolbar_element_min_size),
                    )
                    .clicked()
                {
                    // FUTURE a nice feature here, would be to use the current manufacturer and mpn filters (if any)
                    //        currently there is a single filter, so adding support for per-column filters would make
                    //        implementing this feature easier.
                    // FUTURE disable the button if there are no visible parts.
                    if let Some(overview) = &self.overview {
                        self.component
                            .send(PhaseTabUiCommand::AddPartsToLoadout {
                                phase: overview.phase_reference.clone(),
                                manufacturer_pattern: Regex::new("^.*$").unwrap(),
                                mpn_pattern: Regex::new("^.*$").unwrap(),
                            })
                    }
                }

                if ui
                    .add(egui::Button::new(tr!("phase-toolbar-placement-orderings")).min_size(toolbar_element_min_size))
                    .clicked()
                {
                    self.component
                        .send(PhaseTabUiCommand::PhasePlacementsOrderingsClicked)
                }
                ui.separator();
            });

            self.placements_table_ui.filter_ui(ui);

            // the filter_ui is taller than previously added elements, need a sizing pass
            // and then to set the height of the toolbar row on the next frame

            let current_height = ui.max_rect().height();
            let height = toolbar_height.map_or(current_height, |height| current_height.max(height));

            if toolbar_height.is_none() {
                println!("setting toolbar height");
                let ctx = ui.ctx();
                ctx.memory_mut(|mem| {
                    mem.data
                        .insert_temp(toolbar_height_id, height)
                });
            }
        });

        ui.separator();

        //
        // Table
        //
        self.placements_table_ui
            .ui(ui, &mut PlacementsTableUiContext::default());

        //
        // Modals
        //
        if let Some(dialog) = &self.placement_orderings_modal {
            dialog.ui(ui, &mut ());
        }
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            PhaseTabUiCommand::None => Some(PhaseTabUiAction::None),
            PhaseTabUiCommand::PlacementsTableUiCommand(command) => {
                let action = self
                    .placements_table_ui
                    .update(command, &mut PlacementsTableUiContext::default());
                match action {
                    Some(PlacementsTableUiAction::None) => None,
                    Some(PlacementsTableUiAction::RequestRepaint) => Some(PhaseTabUiAction::RequestRepaint),
                    Some(PlacementsTableUiAction::UpdatePlacement {
                        object_path,
                        new_placement,
                        old_placement,
                    }) => Some(PhaseTabUiAction::UpdatePlacement {
                        object_path,
                        new_placement,
                        old_placement,
                    }),
                    Some(PlacementsTableUiAction::LocatePlacement {
                        object_path,
                        pcb_side,
                        design_position,
                        unit_position,
                    }) => Some(PhaseTabUiAction::LocatePlacement {
                        object_path,
                        pcb_side,
                        design_position,
                        unit_position,
                    }),
                    None => None,
                }
            }
            PhaseTabUiCommand::AddPartsToLoadout {
                phase,
                manufacturer_pattern,
                mpn_pattern,
            } => Some(PhaseTabUiAction::AddPartsToLoadout {
                phase,
                manufacturer_pattern,
                mpn_pattern,
            }),
            PhaseTabUiCommand::PhasePlacementsOrderingsClicked => {
                if let Some(overview) = &self.overview {
                    let mut modal = PlacementOrderingsModal::new(
                        overview.phase_reference.clone(),
                        &overview.phase_placement_orderings,
                    );
                    modal
                        .component
                        .configure_mapper(self.component.sender.clone(), move |command| {
                            trace!("placement orderings modal mapper. command: {:?}", command);
                            PhaseTabUiCommand::PlacementOrderingsModalUiCommand(command)
                        });

                    self.placement_orderings_modal = Some(modal);
                    None
                } else {
                    None
                }
            }
            PhaseTabUiCommand::PlacementOrderingsModalUiCommand(command) => {
                if let Some(modal) = self.placement_orderings_modal.as_mut() {
                    let action = modal
                        .update(command, &mut ())
                        .inspect(|action| {
                            trace!("placement ordering model action: {:?}", action);
                        });
                    match action {
                        None => None,
                        Some(PlacementOrderingsModalAction::Submit(args)) => {
                            self.placement_orderings_modal.take();
                            Some(PhaseTabUiAction::SetPlacementOrderings(args))
                        }
                        Some(PlacementOrderingsModalAction::CloseDialog) => {
                            self.placement_orderings_modal.take();
                            None
                        }
                    }
                } else {
                    None
                }
            }
            PhaseTabUiCommand::TaskAction {
                operation,
                task,
                action,
            } => Some(PhaseTabUiAction::TaskAction {
                phase: self
                    .overview
                    .as_ref()
                    .unwrap()
                    .phase_reference
                    .clone(),
                operation,
                task,
                action,
            }),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq)]
pub struct PhaseTab {
    pub phase: Reference,
}

impl PhaseTab {
    pub fn new(phase: Reference) -> Self {
        Self {
            phase,
        }
    }
}

impl Tab for PhaseTab {
    type Context = ProjectTabContext;

    fn label(&self) -> WidgetText {
        let title = format!("{}", self.phase).to_string();
        egui::widget_text::WidgetText::from(title)
    }

    fn ui<'a>(&mut self, ui: &mut Ui, _tab_key: &TabKey, context: &mut Self::Context) {
        let state = context.state.lock().unwrap();
        let Some(phase_ui) = state.phases_tab_uis.get(&self.phase) else {
            ui.spinner();
            return;
        };
        UiComponent::ui(phase_ui, ui, &mut PhaseTabUiContext::default());
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, context: &mut Self::Context) -> OnCloseResponse {
        let mut state = context.state.lock().unwrap();
        if let Some(_phase_ui) = state.phases_tab_uis.remove(&self.phase) {
            debug!("removed orphaned phase ui. phase: {:?}", &self.phase);
        }
        OnCloseResponse::Close
    }
}
