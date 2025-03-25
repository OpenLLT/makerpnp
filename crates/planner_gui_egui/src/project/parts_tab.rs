use egui::scroll_area::ScrollBarVisibility;
use egui::{Sense, Ui, WidgetText};
use egui_extras::{Column, TableBuilder};
use egui_i18n::tr;
use egui_mobius::types::Value;
use planner_app::PartStates;
use tracing::debug;

use crate::project::tables;
use crate::project::tabs::ProjectTabContext;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Debug)]
pub struct PartsUi {
    part_states: Option<PartStates>,
    editing: Value<Option<(usize, usize)>>,

    pub component: ComponentState<PartsUiCommand>,
}

impl PartsUi {
    pub fn new() -> Self {
        Self {
            part_states: None,
            editing: Value::default(),

            component: Default::default(),
        }
    }

    pub fn update_part_states(&mut self, part_states: PartStates) {
        self.part_states.replace(part_states);
    }
}

#[derive(Debug, Clone)]
pub enum PartsUiCommand {
    None,
}

#[derive(Debug, Clone)]
pub enum PartsUiAction {
    None,
}

#[derive(Debug, Clone, Default)]
pub struct PartsUiContext {}

impl UiComponent for PartsUi {
    type UiContext<'context> = PartsUiContext;
    type UiCommand = PartsUiCommand;
    type UiAction = PartsUiAction;

    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        ui.label(tr!("project-parts-header"));
        if let Some(part_states) = &self.part_states {
            let table = TableBuilder::new(ui)
                .striped(true)
                .resizable(true)
                .sense(Sense::CLICK)
                .scroll_bar_visibility(ScrollBarVisibility::AlwaysVisible)
                .column(Column::auto()) // index
                .column(Column::remainder()) // mfr
                .column(Column::remainder()) // mpn
                .column(Column::auto()); // processes

            table
                .header(20.0, |mut header| {
                    header.col(|ui| {
                        ui.strong(tr!("table-parts-column-index"));
                    });
                    header.col(|ui| {
                        ui.strong(tr!("table-parts-column-manufacturer"));
                    });
                    header.col(|ui| {
                        ui.strong(tr!("table-parts-column-mpn"));
                    });
                    header.col(|ui| {
                        ui.strong(tr!("table-parts-column-processes"));
                    });
                })
                .body(|body| {
                    let row_count = part_states.parts.len();
                    body.rows(18.0, row_count, move |mut row| {
                        let index = row.index();
                        let part_state = &part_states.parts[index];
                        let mut editing = self.editing.lock().unwrap();

                        row.col(|ui| {
                            ui.label(format!("{}", tables::index_to_human_readable(index)));
                        });

                        row.col(|ui| {
                            if ui
                                .label(&part_state.part.manufacturer)
                                .clicked()
                                || ui.response().clicked()
                            {
                                debug!("clicked");
                            };
                        });

                        let this_cell = (index, 2);
                        let is_editing = editing.is_some() && editing.unwrap() == this_cell;
                        let (_rect, response) = row.col(|ui| {
                            if is_editing {
                                let mut value = part_state.part.mpn.clone();
                                let response =
                                    ui.add_sized(ui.available_size(), egui::TextEdit::singleline(&mut value));
                                if response.clicked_elsewhere() || response.lost_focus() {
                                    editing.take();
                                }

                                if part_state.part.mpn.eq(&value) {
                                    // TODO signal state update with new value so that the part_state.part.mpn is correct on the next frame
                                    debug!("changed");
                                }
                            } else {
                                ui.add(egui::Label::new(&part_state.part.mpn).selectable(false));
                            }
                        });
                        if !is_editing && response.clicked() {
                            debug!("clicked");
                            editing.replace(this_cell);
                        }

                        row.col(|ui| {
                            let processes: String = part_state
                                .processes
                                .iter()
                                .map(|process| process.to_string())
                                .collect::<Vec<_>>()
                                .join(",");
                            ui.label(processes);
                        });
                    })
                });
        }
    }

    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            PartsUiCommand::None => Some(PartsUiAction::None),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Default, PartialEq)]
pub struct PartsTab {}

impl Tab for PartsTab {
    type Context = ProjectTabContext;

    fn label(&self) -> WidgetText {
        egui::widget_text::WidgetText::from(tr!("project-parts-tab-label"))
    }

    fn ui<'a>(&mut self, ui: &mut Ui, _tab_key: &TabKey, context: &mut Self::Context) {
        let state = context.state.lock().unwrap();
        UiComponent::ui(&state.parts_ui, ui, &mut PartsUiContext::default());
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, _context: &mut Self::Context) -> bool {
        true
    }
}
