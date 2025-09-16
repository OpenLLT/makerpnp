use egui::{Resize, Ui, WidgetText};
use egui_dock::tab_viewer::OnCloseResponse;
use egui_extras::{Column, TableBuilder};
use egui_i18n::tr;
use planner_app::{PhaseOverview, PhaseReference, ProjectOverview};

use crate::project::tabs::ProjectTabContext;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Debug)]
pub struct OverviewTabUi {
    overview: Option<ProjectOverview>,
    phases: Option<Vec<PhaseOverview>>,

    pub component: ComponentState<OverviewTabUiCommand>,
}

impl OverviewTabUi {
    const TABLE_HEIGHT_MAX: f32 = 200.0;

    pub fn new() -> Self {
        Self {
            overview: None,
            phases: None,
            component: Default::default(),
        }
    }

    pub fn update_overview(&mut self, project_overview: ProjectOverview) {
        self.overview.replace(project_overview);
    }

    pub fn update_phases(&mut self, phases: Vec<PhaseOverview>) {
        self.phases.replace(phases);
    }

    fn show_phases(&self, ui: &mut Ui, text_height: f32) {
        if let Some(phases) = &self.phases {
            ui.label(tr!("project-overview-phases-header"));

            let available_size = ui.available_size();

            Resize::default()
                .resizable([true, true])
                .default_size(available_size / 2.0)
                .max_width(available_size.x)
                .max_height(Self::TABLE_HEIGHT_MAX)
                .show(ui, |ui| {
                    // HACK: search codebase for 'HACK: table-resize-hack' for details
                    egui::Frame::new()
                        .outer_margin(4.0)
                        .show(ui, |ui| {
                            TableBuilder::new(ui)
                                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                                .min_scrolled_height(80.0)
                                .column(Column::remainder())
                                .column(Column::auto().resizable(false))
                                .striped(true)
                                .resizable(true)
                                .auto_shrink([false, false])
                                .header(20.0, |mut header| {
                                    header.col(|ui| {
                                        ui.strong(tr!("table-phases-column-name"));
                                    });
                                    header.col(|ui| {
                                        ui.strong(tr!("table-phases-column-actions"));
                                    });
                                })
                                .body(|body| {
                                    body.rows(text_height, phases.len(), |mut row| {
                                        if let Some(phase_overview) = phases.get(row.index()) {
                                            row.col(|ui| {
                                                ui.label(
                                                    phase_overview
                                                        .phase_reference
                                                        .to_string(),
                                                );
                                            });

                                            row.col(|ui| {
                                                let can_delete = phase_overview.state.is_pending();

                                                ui.add_enabled_ui(can_delete, |ui| {
                                                    if ui
                                                        .button(tr!("form-common-button-delete"))
                                                        .clicked()
                                                    {
                                                        self.component
                                                            .send(OverviewTabUiCommand::PhaseDeleteClicked(
                                                                phase_overview.phase_reference.clone(),
                                                            ));
                                                    }
                                                });
                                            });
                                        }
                                    });
                                });
                        });
                });
        }
    }
}

#[derive(Debug, Clone)]
pub enum OverviewTabUiCommand {
    None,
    PhaseDeleteClicked(PhaseReference),
}

#[derive(Debug, Clone)]
pub enum OverviewTabUiAction {
    None,
    DeletePhase(PhaseReference),
}

#[derive(Debug, Clone, Default)]
pub struct OverviewTabUiContext {}

impl UiComponent for OverviewTabUi {
    type UiContext<'context> = OverviewTabUiContext;
    type UiCommand = OverviewTabUiCommand;
    type UiAction = OverviewTabUiAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        let text_height = egui::TextStyle::Body
            .resolve(ui.style())
            .size
            .max(ui.spacing().interact_size.y);

        ui.heading(tr!("project-overview-header"));

        if let Some(overview) = &self.overview {
            ui.label(tr!("project-overview-detail-name", { name: &overview.name }));
        }

        ui.separator();

        self.show_phases(ui, text_height);
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            OverviewTabUiCommand::None => Some(OverviewTabUiAction::None),
            OverviewTabUiCommand::PhaseDeleteClicked(reference) => Some(OverviewTabUiAction::DeletePhase(reference)),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Default, PartialEq)]
pub struct OverviewTab {}

impl Tab for OverviewTab {
    type Context = ProjectTabContext;

    fn label(&self) -> WidgetText {
        egui::widget_text::WidgetText::from(tr!("project-overview-tab-label"))
    }

    fn ui<'a>(&mut self, ui: &mut Ui, _tab_key: &TabKey, context: &mut Self::Context) {
        let state = context.state.lock().unwrap();
        UiComponent::ui(&state.overview_ui, ui, &mut OverviewTabUiContext::default());
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, _context: &mut Self::Context) -> OnCloseResponse {
        OnCloseResponse::Close
    }
}
