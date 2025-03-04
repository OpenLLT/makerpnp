use egui::Ui;
use egui_i18n::tr;
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Debug, Clone)]
pub enum ToolbarUiCommand {
    ShowHomeTabClicked,
    CloseAllTabsClicked,
    OpenClicked,
}

pub enum ToolbarAction {
    ShowHomeTab,
    CloseAllTabs,
    PickFile,
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
    type UiContext<'context> = ();
    type UiCommand = ToolbarUiCommand;
    type UiAction = ToolbarAction;

    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        egui::Frame::new().show(ui, |ui| {
            ui.horizontal(|ui| {
                let home_button = ui.button(tr!("toolbar-button-home"));
                let open_button = ui.button(tr!("toolbar-button-open"));
                let close_all_button = ui.button(tr!("toolbar-button-close-all"));

                if home_button.clicked() {
                    self.component.send(ToolbarUiCommand::ShowHomeTabClicked);
                }

                if open_button.clicked() {
                    self.component.send(ToolbarUiCommand::OpenClicked);
                }

                if close_all_button.clicked() {
                    self.component.send(ToolbarUiCommand::CloseAllTabsClicked);
                }
            });
        });
    }

    fn update<'context>(&mut self, command: Self::UiCommand, _context: &mut Self::UiContext<'context>) -> Option<Self::UiAction> {
        match command {
            ToolbarUiCommand::ShowHomeTabClicked => {
                Some(ToolbarAction::ShowHomeTab)
            }
            ToolbarUiCommand::CloseAllTabsClicked => {
                Some(ToolbarAction::CloseAllTabs)
            }
            ToolbarUiCommand::OpenClicked => {
                Some(ToolbarAction::PickFile)
            }
        }
    }
}
