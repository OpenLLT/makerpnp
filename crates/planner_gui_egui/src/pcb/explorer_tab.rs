use std::collections::HashMap;

use derivative::Derivative;
use egui::{Ui, WidgetText};
use egui_i18n::tr;
use egui_ltreeview::{NodeBuilder, TreeView, TreeViewState};
use egui_mobius::types::Value;
use planner_app::{PcbOverview, PcbSide, PcbUnitIndex};

use crate::pcb::tabs::PcbTabContext;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};
use crate::ui_util::NavigationPath;

#[derive(Derivative)]
#[derivative(Debug)]
pub struct ExplorerUi {
    pcb_overview: Option<PcbOverview>,

    #[derivative(Debug = "ignore")]
    tree_view_state: Value<TreeViewState<usize>>,

    pub component: ComponentState<ExplorerUiCommand>,
}

impl ExplorerUi {
    pub fn new() -> Self {
        Self {
            pcb_overview: None,
            tree_view_state: Default::default(),
            component: Default::default(),
        }
    }

    pub fn update_pcb_overview(&mut self, pcb_overview: PcbOverview) {
        self.pcb_overview.replace(pcb_overview);
    }

    fn show_pcb_tree(&self, ui: &mut Ui) {
        let Some(pcb_overview) = self.pcb_overview.as_ref() else {
            return;
        };

        let mut tree_view_state = self.tree_view_state.lock().unwrap();

        let (_response, actions) = TreeView::new(ui.make_persistent_id("pcb_explorer_tree")).show_state(
            ui,
            &mut tree_view_state,
            |tree_builder: &mut egui_ltreeview::TreeViewBuilder<'_, usize>| {
                let designs = vec!["Design 1", "Design 2"];
                let gerbers: HashMap<&str, Vec<(&str, Vec<PcbSide>)>> = HashMap::from_iter([
                    ("Design 1", vec![
                        ("top silk", vec![PcbSide::Top]),
                        ("pcb outline", vec![PcbSide::Top, PcbSide::Bottom]),
                        ("bottom silk", vec![PcbSide::Bottom]),
                    ]),
                    ("Design 2", vec![
                        ("top silk", vec![PcbSide::Top]),
                        ("pcb outline", vec![PcbSide::Top, PcbSide::Bottom]),
                        ("bottom silk", vec![PcbSide::Bottom]),
                    ]),
                ]);

                let units = 100;

                let unit_map: HashMap<PcbUnitIndex, &str> =
                    HashMap::from_iter([(0, "Design 1"), (1, "Design 1"), (4, "Design 2"), (5, "Design 2")]);

                let mut node_id = 0;
                tree_builder.node(
                    NodeBuilder::dir(node_id)
                        .activatable(true)
                        .label_ui(|ui| {
                            ui.add(egui::Label::new(tr!("pcb-explorer-node-root", {name: &pcb_overview.name})).selectable(false));
                        }),
                );

                //
                // Configuration
                //

                node_id += 1;
                tree_builder.leaf(node_id, tr!("pcb-explorer-node-configuration"));

                //
                // Views
                //

                node_id += 1;
                tree_builder.node(
                    NodeBuilder::dir(node_id)
                        .activatable(true)
                        .label_ui(|ui| {
                            ui.add(egui::Label::new(tr!("pcb-explorer-node-pcb-view")).selectable(false));
                        }),
                );

                node_id += 1;
                tree_builder.leaf(node_id, tr!("pcb-side-top"));
                node_id += 1;
                tree_builder.leaf(node_id, tr!("pcb-side-bottom"));

                // views
                tree_builder.close_dir();

                //
                // designs
                //

                node_id += 1;
                tree_builder.node(
                    NodeBuilder::dir(node_id)
                        .activatable(true)
                        .label_ui(|ui| {
                            ui.add(egui::Label::new(tr!("pcb-explorer-node-designs")).selectable(false));
                        }),
                );

                for design in designs {
                    node_id += 1;
                    tree_builder.node(
                        NodeBuilder::dir(node_id)
                            .activatable(true)
                            .label_ui(|ui| {
                                ui.add(egui::Label::new(design.to_string()).selectable(false));
                            }),
                    );

                    node_id += 1;
                    tree_builder.leaf(node_id, tr!("pcb-side-top"));
                    node_id += 1;
                    tree_builder.leaf(node_id, tr!("pcb-side-bottom"));

                    tree_builder.close_dir();
                }

                // designs
                tree_builder.close_dir();

                //
                // units
                //
                node_id += 1;
                tree_builder.node(
                    NodeBuilder::dir(node_id)
                        .activatable(true)
                        .label_ui(|ui| {
                            ui.add(egui::Label::new(tr!("pcb-explorer-node-units")).selectable(false));
                        }),
                );

                for index in 0_u16..units {
                    node_id += 1;

                    let pcb_number = index + 1;
                    let label = unit_map
                        .get(&(index as PcbUnitIndex))
                        .map(|name| tr!("pcb-explorer-node-units-assignment-assigned", { pcb_number: pcb_number, design_name: name.to_string()}))
                        .unwrap_or_else(|| tr!("pcb-explorer-node-units-assignment-unassigned", {pcb_number: pcb_number}).to_string());

                    tree_builder.leaf(node_id, label.to_string());
                }
                // units
                tree_builder.close_dir();

                // root
                tree_builder.close_dir();

            },
        );
    }

    pub fn select_path(&mut self, navigation_path: &NavigationPath) {
        // TODO
    }
}

#[derive(Debug, Clone)]
pub enum ExplorerUiCommand {
    Navigate(NavigationPath),
}

#[derive(Debug, Clone)]
pub enum ExplorerUiAction {
    Navigate(NavigationPath),
}

#[derive(Debug, Clone, Default)]
pub struct ExplorerUiContext {}

impl UiComponent for ExplorerUi {
    type UiContext<'context> = ExplorerUiContext;
    type UiCommand = ExplorerUiCommand;
    type UiAction = ExplorerUiAction;

    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        if self.pcb_overview.is_some() {
            egui::ScrollArea::vertical().show(ui, |ui| {
                self.show_pcb_tree(ui);
            });
        } else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
        }
    }

    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            ExplorerUiCommand::Navigate(path) => {
                self.select_path(&path);

                Some(ExplorerUiAction::Navigate(path))
            }
        }
    }
}

#[derive(Default, Debug, serde::Deserialize, serde::Serialize)]
pub struct ExplorerTab {}

impl Tab for ExplorerTab {
    type Context = PcbTabContext;

    fn label(&self) -> WidgetText {
        let title = tr!("pcb-explorer-tab-label");

        egui::widget_text::WidgetText::from(title)
    }

    fn ui<'a>(&mut self, ui: &mut Ui, _tab_key: &TabKey, context: &mut Self::Context) {
        let state = context.state.lock().unwrap();
        UiComponent::ui(&state.explorer_ui, ui, &mut ExplorerUiContext::default());
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, _context: &mut Self::Context) -> bool {
        true
    }
}
