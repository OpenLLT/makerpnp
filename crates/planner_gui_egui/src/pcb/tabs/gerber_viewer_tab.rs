use derivative::Derivative;
use egui::scroll_area::ScrollBarVisibility;
use egui::{Resize, Ui, WidgetText};
use egui_extras::{Column, TableBuilder};
use egui_mobius::Value;
use planner_app::PcbOverview;
use tracing::trace;

use crate::pcb::tabs::PcbTabContext;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};
use crate::ui_components::gerber_viewer_ui::{
    GerberViewerMode, GerberViewerUi, GerberViewerUiAction, GerberViewerUiCommand, GerberViewerUiContext,
    GerberViewerUiInstanceArgs,
};
use egui_vertical_stack::VerticalStack;

#[derive(Derivative)]
#[derivative(Debug)]
pub struct GerberViewerTabUi {
    #[derivative(Debug = "ignore")]
    gerber_viewer_ui: GerberViewerUi,

    pub component: ComponentState<GerberViewerTabUiCommand>,
    #[derivative(Debug = "ignore")]
    stack: Value<VerticalStack>,
}

impl GerberViewerTabUi {
    const TABLE_SCROLL_HEIGHT_MIN: f32 = 40.0;

    pub fn new(args: GerberViewerUiInstanceArgs) -> Self {
        let component: ComponentState<GerberViewerTabUiCommand> = Default::default();

        let mut gerber_viewer_ui = GerberViewerUi::new(args);
        gerber_viewer_ui
            .component
            .configure_mapper(component.sender.clone(), |gerber_viewer_command| {
                trace!("gerber_viewer mapper. command: {:?}", gerber_viewer_command);
                GerberViewerTabUiCommand::GerberViewerUiCommand(gerber_viewer_command)
            });

        Self {
            gerber_viewer_ui,
            stack: Value::new(VerticalStack::new()
                .min_panel_height(150.0)
                .default_panel_height(50.0)),
            component,
        }
    }

    pub fn update_pcb_overview(&mut self, pcb_overview: PcbOverview) {
        self.gerber_viewer_ui
            .update_layers_from_pcb_overview(pcb_overview);
    }
}

#[derive(Debug, Clone)]
pub enum GerberViewerTabUiCommand {
    None,
    GerberViewerUiCommand(GerberViewerUiCommand),
}

#[derive(Debug, Clone)]
pub enum GerberViewerTabUiAction {
    None,
}

#[derive(Debug, Clone, Default)]
pub struct GerberViewerTabUiContext {}

impl UiComponent for GerberViewerTabUi {
    type UiContext<'context> = GerberViewerTabUiContext;
    type UiCommand = GerberViewerTabUiCommand;
    type UiAction = GerberViewerTabUiAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        egui::SidePanel::left(
            ui.id()
                .with("gerber_viewer_tab_left_panel"),
        )
        .resizable(true)
        .show_inside(ui, |ui| {
            egui::ScrollArea::both()
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.label("Gerber Viewer Tab"); // TODO delete or translate
                    ui.separator();

                    let layers_binding = self.gerber_viewer_ui.layers();
                    let layers = layers_binding.lock().unwrap();

                    Resize::default()
                        .resizable([false, true])
                        .default_size(ui.available_size())
                        .min_width(ui.available_width())
                        .max_width(ui.available_width())
                        //.max_height(Self::TABLE_HEIGHT_MAX)
                        .show(ui, |ui| {
                            // HACK: search codebase for 'HACK: table-resize-hack' for details
                            egui::Frame::new()
                                .outer_margin(4.0)
                                .show(ui, |ui| {
                                    ui.style_mut()
                                        .interaction
                                        .selectable_labels = false;

                                    let text_height = egui::TextStyle::Body
                                        .resolve(ui.style())
                                        .size
                                        .max(ui.spacing().interact_size.y);

                                    let _table_response = TableBuilder::new(ui)
                                        .striped(true)
                                        .resizable(true)
                                        .auto_shrink([false, false])
                                        .min_scrolled_height(Self::TABLE_SCROLL_HEIGHT_MIN)
                                        .scroll_bar_visibility(ScrollBarVisibility::AlwaysVisible)
                                        .sense(egui::Sense::click())
                                        .column(Column::auto())
                                        .column(Column::auto())
                                        .column(Column::remainder())
                                        .header(20.0, |mut header| {
                                            header.col(|ui| {
                                                ui.strong("#"); // TODO translate
                                            });
                                            header.col(|ui| {
                                                ui.strong("NAME"); // TODO translate
                                            });
                                            header.col(|ui| {
                                                ui.strong("FILE"); // TODO translate
                                            });
                                        })
                                        .body(|mut body| {
                                            for (row_index, ((path, name), _)) in layers.iter().enumerate() {
                                                body.row(text_height, |mut row| {
                                                    row.col(|ui| {
                                                        ui.label(row_index.to_string());
                                                    });
                                                    row.col(|ui| {
                                                        ui.label(format!("{}", name));
                                                    });
                                                    row.col(|ui| {
                                                        ui.label(format!("{:?}", path));
                                                    });
                                                });
                                            }
                                        });
                                });
                        });
                });
        });
        egui::SidePanel::right(
            ui.id()
                .with("gerber_viewer_tab_right_panel"),

        ).show_inside(ui, |ui| {

            println!("right panel. available_size: {:?}", ui.available_size());

            let mut stack = self.stack.lock().unwrap();
            stack
                .id_salt(ui.id().with("vertical_stack"))
                .body(ui, |body|{
                body.add_panel(|ui|{
                    ui.label("top");
                    ui.label("top");
                    ui.label("top");
                    ui.label("top");
                    ui.label("top");
                    ui.label("top");
                    ui.label("top");
                    ui.label("top");
                    ui.label("top");
                    ui.label("top");
                    ui.label("top");
                    ui.label("top");
                    ui.label("top");
                });
                body.add_panel(|ui|{
                    ui.label("middle");
                    ui.label("middle with some very very very very very long text");
                    ui.label("middle");
                });
                body.add_panel(|ui|{
                    ui.label("bottom");
                    ui.label("bottom");
                    ui.label("bottom");
                    ui.label("bottom");
                    ui.label("bottom");
                    ui.label("bottom");
                    ui.label("bottom");
                    ui.label("bottom");
                    ui.label("bottom");
                    ui.label("bottom");
                    ui.label("bottom");
                    ui.label("bottom");
                    ui.label("bottom");
                    ui.label("bottom");
                    ui.label("bottom");
                    ui.label("bottom");
                    ui.label("bottom");
                    ui.label("bottom");
                    ui.label("bottom");
                    ui.label("bottom");
                    ui.label("bottom");
                });
            })
        });
        egui::CentralPanel::default().show_inside(ui, |ui| {
            self.gerber_viewer_ui
                .ui(ui, &mut GerberViewerUiContext {})
        });
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            GerberViewerTabUiCommand::None => Some(GerberViewerTabUiAction::None),
            GerberViewerTabUiCommand::GerberViewerUiCommand(command) => {
                let action = self
                    .gerber_viewer_ui
                    .update(command, &mut GerberViewerUiContext {});
                match action {
                    None => None,
                    Some(GerberViewerUiAction::None) => Some(GerberViewerTabUiAction::None),
                }
            }
        }
    }
}

#[derive(Default, Debug, serde::Deserialize, serde::Serialize)]
pub struct GerberViewerTab {
    pub(crate) args: GerberViewerUiInstanceArgs,
}

impl GerberViewerTab {
    pub fn new(args: GerberViewerUiInstanceArgs) -> Self {
        Self {
            args,
        }
    }
}

impl Tab for GerberViewerTab {
    type Context = PcbTabContext;

    fn label(&self) -> WidgetText {
        let title = match self.args.mode {
            GerberViewerMode::Panel => "Panel".to_string(),
            // TODO improve the tab title
            GerberViewerMode::Design(design_index) => format!("Design ({})", design_index),
        };

        egui::widget_text::WidgetText::from(title)
    }

    fn ui<'a>(&mut self, ui: &mut Ui, _tab_key: &TabKey, context: &mut Self::Context) {
        let state = context.state.lock().unwrap();
        let Some(instance) = state
            .gerber_viewer_tab_uis
            .get(&self.args)
        else {
            ui.spinner();
            return;
        };

        UiComponent::ui(instance, ui, &mut GerberViewerTabUiContext::default());
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, _context: &mut Self::Context) -> bool {
        true
    }
}
