use egui::{Button, FontFamily, RichText, Ui, WidgetText};
use egui_i18n::tr;
use egui_material_icons::icons::ICON_ADD;
use egui_taffy::taffy::Style;
use egui_taffy::taffy::prelude::{length, percent};
use egui_taffy::{TuiBuilderLogic, taffy, tui};
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Default, Debug, Deserialize, Serialize)]
pub struct NewProjectTab {

    #[serde(skip)]
    pub component: ComponentState<NewProjectTabUiCommand>,
}

#[derive(Debug, Clone)]
pub enum NewProjectTabUiCommand {
    None,
    Test,
}

pub enum NewProjectTabAction {
    None,
}

pub struct NewProjectTabContext {
    pub tab_key: TabKey,
}

impl Tab for NewProjectTab {
    type Context = NewProjectTabContext;

    fn label(&self) -> WidgetText {
        egui::widget_text::WidgetText::from(tr!("tab-label-new-project"))
    }

    fn ui(&mut self, ui: &mut Ui, tab_key: &TabKey, _context: &mut Self::Context) {
        let mut new_project_tab_context = NewProjectTabContext {
            tab_key: tab_key.clone(),
        };
        UiComponent::ui(self, ui, &mut new_project_tab_context);
    }
}

impl UiComponent for NewProjectTab {
    type UiContext<'context> = NewProjectTabContext;
    type UiCommand = NewProjectTabUiCommand;
    type UiAction = NewProjectTabAction;

    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        ui.ctx().style_mut(|style| {
            // if this is not done, text in labels/checkboxes/etc wraps
            style.wrap_mode = Some(egui::TextWrapMode::Extend);
        });

        let default_style = || Style {
            padding: length(8.),
            gap: length(8.),
            ..Default::default()
        };

        tui(ui, ui.id().with("new_project"))
            .reserve_available_space()
            .style(Style {
                justify_content: Some(taffy::JustifyContent::Center),
                align_items: Some(taffy::AlignItems::Center),
                flex_direction: taffy::FlexDirection::Column,
                size: taffy::Size {
                    width: percent(1.),
                    height: percent(1.),
                },
                ..default_style()
            })
            .show(|tui| {
                tui.style(Style {
                    flex_direction: taffy::FlexDirection::Row,
                    //align_self: Some(taffy::AlignItems::Center),
                    ..default_style()
                })
                .add_with_border(|tui| {
                    tui.label(
                        RichText::new(tr!("new-project-banner"))
                            .size(24.0)
                            .family(FontFamily::Proportional),
                    );
                });

                tui.ui(|ui| {
                    if ui
                        .add(Button::new(
                            "TEST",
                        ))
                        .clicked()
                    {
                        self.component
                            .send(NewProjectTabUiCommand::Test);
                    }
                });
            });
    }

    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            NewProjectTabUiCommand::None => Some(NewProjectTabAction::None),
            NewProjectTabUiCommand::Test => {
                debug!("test");
                None
            }
        }
    }
}
