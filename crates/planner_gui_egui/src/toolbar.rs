use egui_i18n::tr;
use egui_mobius::slot::Slot;
use egui_mobius::types::Enqueue;

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
    //
    // ui component fields
    //
    sender: Enqueue<ToolbarUiCommand>,
    #[allow(dead_code)]
    toolbar_slot: Slot<ToolbarUiCommand>,
}

impl Toolbar {
    pub fn new(sender: Enqueue<ToolbarUiCommand>, toolbar_slot: Slot<ToolbarUiCommand>) -> Self {
        Self {
            sender,
            toolbar_slot,
        }
    }

    pub fn ui(&self, ui: &mut egui::Ui) {
        egui::Frame::new().show(ui, |ui| {
            ui.horizontal(|ui| {
                let home_button = ui.button(tr!("toolbar-button-home"));
                let open_button = ui.button(tr!("toolbar-button-open"));
                let close_all_button = ui.button(tr!("toolbar-button-close-all"));

                if home_button.clicked() {
                    self.sender.send(ToolbarUiCommand::ShowHomeTabClicked).ok();
                }

                if open_button.clicked() {
                    self.sender.send(ToolbarUiCommand::OpenClicked).ok();
                }

                if close_all_button.clicked() {
                    self.sender.send(ToolbarUiCommand::CloseAllTabsClicked).ok();
                }
            });
        });
    }
    
    pub fn update(&mut self, command: ToolbarUiCommand) -> ToolbarAction {
        match command {
            ToolbarUiCommand::ShowHomeTabClicked => {
                ToolbarAction::ShowHomeTab
            }
            ToolbarUiCommand::CloseAllTabsClicked => {
                ToolbarAction::CloseAllTabs
            }
            ToolbarUiCommand::OpenClicked => {
                ToolbarAction::PickFile
            }
        }
    }
}
