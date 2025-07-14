use std::collections::HashSet;

use derivative::Derivative;
use egui::{Ui, WidgetText};
use egui_dock::tab_viewer::OnCloseResponse;
use egui_extras::{Column, TableBuilder};
use egui_i18n::tr;
use planner_app::PcbOverview;
use tracing::trace;

use crate::i18n::conversions::{gerber_file_function_to_i18n_key, pcb_side_to_i18n_key};
use crate::pcb::tabs::PcbTabContext;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};
use crate::ui_components::gerber_viewer_ui::{
    GerberViewerMode, GerberViewerUi, GerberViewerUiAction, GerberViewerUiCommand, GerberViewerUiContext,
    GerberViewerUiInstanceArgs, LayersMap,
};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct GerberViewerTabUi {
    #[derivative(Debug = "ignore")]
    gerber_viewer_ui: GerberViewerUi,

    pub component: ComponentState<GerberViewerTabUiCommand>,
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
            component,
        }
    }

    pub fn update_pcb_overview(&mut self, pcb_overview: PcbOverview) {
        self.gerber_viewer_ui
            .update_layers_from_pcb_overview(pcb_overview);
    }

    fn show_layers_table(ui: &mut Ui, layers: &LayersMap) {
        ui.style_mut()
            .interaction
            .selectable_labels = false;

        let show_function_column = layers
            .iter()
            .all(|((_, function), _)| function.is_some());

        let show_file_column = layers
            .iter()
            .all(|((path, _), _)| path.is_some());

        let pcb_sides = layers
            .iter()
            .filter_map(|((_, function), _)| {
                function
                    .map(|function| function.pcb_side())
                    .flatten()
            })
            .collect::<HashSet<_>>();

        let show_pcb_side_column = pcb_sides.len() > 1;

        let column_count = [true, show_function_column, show_pcb_side_column, show_file_column]
            .iter()
            .filter(|&&show| show)
            .count();

        let text_height = egui::TextStyle::Body
            .resolve(ui.style())
            .size
            .max(ui.spacing().interact_size.y);

        let mut table_builder = TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            //.auto_shrink([false, true])
            .min_scrolled_height(Self::TABLE_SCROLL_HEIGHT_MIN)
            //.scroll_bar_visibility(ScrollBarVisibility::AlwaysVisible)
            .sense(egui::Sense::click());

        for index in 0..column_count {
            let is_last = index == column_count - 1;

            let column = if is_last { Column::remainder() } else { Column::auto() };
            table_builder = table_builder.column(column);
        }

        table_builder
            .header(20.0, |mut header| {
                header.col(|ui| {
                    ui.strong(tr!("table-gerber-viewer-layers-column-index"));
                });
                if show_function_column {
                    header.col(|ui| {
                        ui.strong(tr!("table-gerber-viewer-layers-column-gerber-file-function"));
                    });
                }
                if show_pcb_side_column {
                    header.col(|ui| {
                        ui.strong(tr!("table-gerber-viewer-layers-column-pcb-side"));
                    });
                }
                if show_file_column {
                    header.col(|ui| {
                        ui.strong(tr!("table-gerber-viewer-layers-column-file"));
                    });
                }
            })
            .body(|mut body| {
                for (row_index, ((path, function), _)) in layers.iter().enumerate() {
                    body.row(text_height, |mut row| {
                        row.col(|ui| {
                            ui.label(row_index.to_string());
                        });

                        if show_function_column {
                            row.col(|ui| {
                                if let Some(function) = function {
                                    ui.label(tr!(gerber_file_function_to_i18n_key(function)));
                                }
                            });
                        }

                        if show_pcb_side_column {
                            row.col(|ui| {
                                if let Some(pcb_side) = function
                                    .map(|function| function.pcb_side())
                                    .flatten()
                                {
                                    ui.label(tr!(pcb_side_to_i18n_key(&pcb_side)));
                                }
                            });
                        }

                        if show_file_column {
                            row.col(|ui| {
                                if let Some(path) = path {
                                    ui.label(format!(
                                        "{}",
                                        path.file_name()
                                            .unwrap()
                                            .to_string_lossy()
                                    ));
                                }
                            });
                        }
                    });
                }
            });
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
        egui::CentralPanel::default().show_inside(ui, |ui| {
            self.gerber_viewer_ui
                .ui(ui, &mut GerberViewerUiContext {});

            //
            // tool windows
            //
            // these must be rendered AFTER the gerber viewer ui otherwise the windows appear behind the gerber layers

            let tool_windows_id = ui.id();
            egui_tool_windows::ToolWindows::new().windows(ui, {
                move |builder| {
                    builder
                        .add_window(tool_windows_id.with("layers"))
                        .default_pos([20.0, 20.0])
                        .default_size([200.0, 200.0])
                        .show(tr!("pcb-gerber-viewer-layers-window-title"), {
                            let layers_binding = self.gerber_viewer_ui.layers();
                            move |ui| {
                                let layers_map = layers_binding.lock().unwrap();

                                Self::show_layers_table(ui, &layers_map);
                            }
                        })
                }
            });
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
            GerberViewerMode::Panel => tr!("pcb-gerber-viewer-tab-label-panel"),
            // TODO improve the tab title by using the design name, not the index
            GerberViewerMode::Design(design_index) => {
                tr!("pcb-gerber-viewer-tab-label-design", { index:  design_index })
            }
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

    fn on_close<'a>(&mut self, _tab_key: &TabKey, _context: &mut Self::Context) -> OnCloseResponse {
        OnCloseResponse::Close
    }
}
