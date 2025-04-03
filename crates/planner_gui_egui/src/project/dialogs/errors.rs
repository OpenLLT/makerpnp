use std::path::PathBuf;

use egui::{Modal, RichText, Ui};
use egui_extras::{Column, TableBuilder};
use egui_i18n::tr;

use crate::project::{ProjectKey, ProjectUiCommand};
use crate::ui_component::ComponentState;

pub fn show_errors_modal(
    ui: &mut Ui,
    key: ProjectKey,
    path: &PathBuf,
    errors: &Vec<String>,
    component: &ComponentState<(ProjectKey, ProjectUiCommand)>,
) {
    let modal_id = ui.id().with("errors");

    let width = ui.ctx().screen_rect().width() * 0.8;

    Modal::new(modal_id).show(ui.ctx(), |ui| {
        ui.set_width(width);
        let file_name = path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();

        ui.add(
            egui::Label::new(RichText::from(tr!("modal-errors-title", {file: file_name})).heading()).selectable(false),
        );

        let table = TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .column(Column::auto())
            .column(Column::remainder());

        table
            .header(20.0, |mut header| {
                header.col(|ui| {
                    ui.strong(tr!("modal-errors-column-errors"));
                });
            })
            .body(|mut body| {
                for (index, error) in errors.iter().enumerate() {
                    body.row(18.0, |mut row| {
                        row.col(|ui| {
                            ui.label(format!("{}", index));
                        });
                        row.col(|ui| {
                            ui.label(error);
                        });
                    })
                }
            });

        egui::Sides::new().show(
            ui,
            |_ui| {},
            |ui| {
                if ui
                    .button(tr!("form-button-ok"))
                    .clicked()
                {
                    component.send((key, ProjectUiCommand::ClearErrors))
                }
            },
        );
    });
}
