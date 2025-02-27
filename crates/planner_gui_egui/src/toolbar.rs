use egui_i18n::tr;
use egui_mobius::types::Enqueue;
use crate::ui_commands::UiCommand;

pub fn show(ui: &mut egui::Ui, sender: Enqueue<UiCommand>) {
    egui::Frame::new().show(ui, |ui| {
        ui.horizontal(|ui| {
            let home_button = ui.button(tr!("toolbar-button-home"));
            let close_all_button = ui.button(tr!("toolbar-button-close-all"));

            if home_button.clicked() {
                sender.send(UiCommand::ShowHomeTab).ok();
            }

            if close_all_button.clicked() {
                sender.send(UiCommand::CloseAllTabs).ok();
            }
        });
    });

}