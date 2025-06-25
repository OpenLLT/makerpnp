use std::collections::HashSet;

use derivative::Derivative;
use egui::scroll_area::ScrollBarVisibility;
use egui::{Ui, WidgetText};
use egui_extras::{Column, TableBuilder};
use egui_i18n::tr;
use egui_mobius::Value;
use egui_vertical_stack::VerticalStack;
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
            stack: Value::new(
                VerticalStack::new()
                    .min_panel_height(150.0)
                    .default_panel_height(50.0),
            ),
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

        let text_height = egui::TextStyle::Body
            .resolve(ui.style())
            .size
            .max(ui.spacing().interact_size.y);

        let mut table_builder = TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .auto_shrink([false, true])
            .min_scrolled_height(Self::TABLE_SCROLL_HEIGHT_MIN)
            .scroll_bar_visibility(ScrollBarVisibility::AlwaysVisible)
            .sense(egui::Sense::click());

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

        if show_function_column {
            // add another column
            table_builder = table_builder.column(Column::auto());
        }

        if show_pcb_side_column {
            // add another column
            table_builder = table_builder.column(Column::auto());
        }

        if show_file_column {
            // add another column
            table_builder = table_builder.column(Column::auto());
        }

        // add the last column, which is always 'remainder'
        table_builder = table_builder.column(Column::remainder());

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
        egui::SidePanel::left(
            ui.id()
                .with("gerber_viewer_tab_left_panel"),
        )
        .resizable(true)
        .show_inside(ui, |ui| {
            let layers_binding = self.gerber_viewer_ui.layers();
            let mut stack = self.stack.lock().unwrap();
            stack
                .id_salt(ui.id().with("vertical_stack"))
                .body(ui, move |body| {
                    body.add_panel("top", {
                        let layers_binding = layers_binding.clone();

                        move |ui| {
                            let layers_map = layers_binding.lock().unwrap();

                            Self::show_layers_table(ui, &layers_map);
                        }
                    });
                });
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

    fn on_close<'a>(&mut self, _tab_key: &TabKey, _context: &mut Self::Context) -> bool {
        true
    }
}
