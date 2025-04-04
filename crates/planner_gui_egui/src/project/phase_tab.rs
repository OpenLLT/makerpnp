use derivative::Derivative;
use egui::{Ui, WidgetText};
use egui_i18n::tr;
use planner_app::{ObjectPath, PhaseOverview, PhasePlacements, PlacementState, Reference};
use regex::Regex;
use tracing::debug;

use crate::project::placements_tab::PlacementsUiCommand;
use crate::project::tables::placements::{
    PlacementsTableUi, PlacementsTableUiAction, PlacementsTableUiCommand, PlacementsTableUiContext,
};
use crate::project::tabs::ProjectTabContext;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct PhaseUi {
    overview: Option<PhaseOverview>,
    #[derivative(Debug = "ignore")]
    placements_table_ui: PlacementsTableUi,

    pub component: ComponentState<PhaseUiCommand>,
}

impl PhaseUi {
    pub fn new() -> Self {
        let component: ComponentState<PlacementsUiCommand> = Default::default();

        let mut placements_table_ui = PlacementsTableUi::new();
        placements_table_ui
            .component
            .configure_mapper(component.sender.clone(), |placements_table_command| {
                debug!("phase placements table mapper. command: {:?}", placements_table_command);
                PlacementsUiCommand::PlacementsTableUiCommand(placements_table_command)
            });

        Self {
            overview: None,
            placements_table_ui,
            component: Default::default(),
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
pub enum PhaseUiCommand {
    None,
    PlacementsTableUiCommand(PlacementsTableUiCommand),
    AddPartsToLoadout {
        phase: Reference,
        manufacturer_pattern: Regex,
        mpn_pattern: Regex,
    },
}

#[derive(Debug, Clone)]
pub enum PhaseUiAction {
    None,
    RequestRepaint,
    UpdatePlacement {
        object_path: ObjectPath,
        new_placement: PlacementState,
        old_placement: PlacementState,
    },
    AddPartsToLoadout {
        phase: Reference,
        manufacturer_pattern: Regex,
        mpn_pattern: Regex,
    },
}

#[derive(Debug, Clone, Default)]
pub struct PhaseUiContext {}

impl UiComponent for PhaseUi {
    type UiContext<'context> = PhaseUiContext;
    type UiCommand = PhaseUiCommand;
    type UiAction = PhaseUiAction;

    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        ui.label(tr!("phase-placements-header"));

        ui.horizontal(|ui| {
            if ui
                .button(tr!("phase-toolbar-add-parts-to-loadout"))
                .clicked()
            {
                // FUTURE a nice feature here, would be to use the current manufacturer and mpn filters (if any)
                //        currently there is a single filter, so adding support for per-column filters would make
                //        implementing this feature easier.
                // FUTURE disable the button if there are no visible parts.
                if let Some(overview) = &self.overview {
                    self.component
                        .send(PhaseUiCommand::AddPartsToLoadout {
                            phase: overview.phase_reference.clone(),
                            manufacturer_pattern: Regex::new("^.*$").unwrap(),
                            mpn_pattern: Regex::new("^.*$").unwrap(),
                        })
                }
            }
        });

        self.placements_table_ui
            .ui(ui, &mut PlacementsTableUiContext::default());
    }

    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            PhaseUiCommand::None => Some(PhaseUiAction::None),
            PhaseUiCommand::PlacementsTableUiCommand(command) => {
                let action = self
                    .placements_table_ui
                    .update(command, &mut PlacementsTableUiContext::default());
                match action {
                    Some(PlacementsTableUiAction::None) => None,
                    Some(PlacementsTableUiAction::RequestRepaint) => Some(PhaseUiAction::RequestRepaint),
                    Some(PlacementsTableUiAction::UpdatePlacement {
                        object_path,
                        new_placement,
                        old_placement,
                    }) => Some(PhaseUiAction::UpdatePlacement {
                        object_path,
                        new_placement,
                        old_placement,
                    }),
                    None => None,
                }
            }
            PhaseUiCommand::AddPartsToLoadout {
                phase,
                manufacturer_pattern,
                mpn_pattern,
            } => Some(PhaseUiAction::AddPartsToLoadout {
                phase,
                manufacturer_pattern,
                mpn_pattern,
            }),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq)]
pub struct PhaseTab {
    phase: Reference,
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
        let phase_ui = state.phases.get(&self.phase).unwrap();
        UiComponent::ui(phase_ui, ui, &mut PhaseUiContext::default());
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, context: &mut Self::Context) -> bool {
        let mut state = context.state.lock().unwrap();
        if let Some(_phase_ui) = state.phases.remove(&self.phase) {
            debug!("removed orphaned phase: {:?}", &self.phase);
        }
        true
    }
}
