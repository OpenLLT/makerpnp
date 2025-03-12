use egui::Ui;
use egui_i18n::tr;

use crate::tabs::TabKey;
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Debug, Clone)]
pub enum ToolbarUiCommand {
    ShowHomeTabClicked,
    CloseAllTabsClicked,
    OpenClicked,
    SaveClicked(TabKey),
}

pub enum ToolbarAction {
    ShowHomeTab,
    CloseAllTabs,
    PickFile,
    SaveTab(TabKey),
}

pub struct ToolbarContext {
    pub active_tab: Option<TabKey>,
    pub can_save: bool,
}

pub struct Toolbar {
    pub component: ComponentState<ToolbarUiCommand>,
}

impl Toolbar {
    pub fn new() -> Self {
        Self {
            component: ComponentState::default(),
        }
    }
}

impl UiComponent for Toolbar {
    type UiContext<'context> = ToolbarContext;
    type UiCommand = ToolbarUiCommand;
    type UiAction = ToolbarAction;

    fn ui<'context>(&self, ui: &mut Ui, context: &mut Self::UiContext<'context>) {
        egui::Frame::new().show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .button(tr!("toolbar-button-home"))
                    .clicked()
                {
                    self.component
                        .send(ToolbarUiCommand::ShowHomeTabClicked);
                }

                if ui
                    .button(tr!("toolbar-button-open"))
                    .clicked()
                {
                    self.component
                        .send(ToolbarUiCommand::OpenClicked);
                }

                ui.add_enabled_ui(context.can_save, |ui| {
                    if ui
                        .button(tr!("toolbar-button-save"))
                        .clicked()
                    {
                        self.component
                            .send(ToolbarUiCommand::SaveClicked(context.active_tab.unwrap()));
                    }
                });

                if ui
                    .button(tr!("toolbar-button-close-all"))
                    .clicked()
                {
                    self.component
                        .send(ToolbarUiCommand::CloseAllTabsClicked);
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
            ToolbarUiCommand::ShowHomeTabClicked => Some(ToolbarAction::ShowHomeTab),
            ToolbarUiCommand::CloseAllTabsClicked => Some(ToolbarAction::CloseAllTabs),
            ToolbarUiCommand::OpenClicked => Some(ToolbarAction::PickFile),
            ToolbarUiCommand::SaveClicked(tab_key) => Some(ToolbarAction::SaveTab(tab_key)),
        }
    }
}
