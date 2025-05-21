use std::collections::HashMap;
use std::path::PathBuf;

use derivative::Derivative;
use egui::Ui;
use egui_ltreeview::{NodeBuilder, TreeView, TreeViewState};
use egui_mobius::Value;
use egui_mobius::types::Enqueue;
use planner_app::{PcbSide, PcbUnitIndex};
use slotmap::new_key_type;
use tracing::debug;

use crate::planner_app_core::PlannerCoreService;
use crate::task::Task;
use crate::ui_component::{ComponentState, UiComponent};

new_key_type! {
    /// A key for a pcb
    pub struct PcbKey;
}

#[derive(Debug)]
pub enum PcbAction {
    Task(PcbKey, Task<PcbAction>),
    SetModifiedState(bool),
    UiCommand(PcbUiCommand),
    RequestRepaint,
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Pcb {
    #[derivative(Debug = "ignore")]
    planner_core_service: PlannerCoreService,

    path: Option<PathBuf>,

    #[derivative(Debug = "ignore")]
    tree_view_state: Value<TreeViewState<usize>>,

    modified: bool,

    pub component: ComponentState<(PcbKey, PcbUiCommand)>,
}

impl Pcb {
    fn show_pcb_tree(&self, ui: &mut Ui, key: &mut PcbKey) {
        let mut tree_view_state = self.tree_view_state.lock().unwrap();

        let (_response, actions) = TreeView::new(ui.make_persistent_id("project_explorer_tree")).show_state(
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

                //
                // Configuration
                //

                node_id += 1;
                tree_builder.leaf(node_id, "Configuration"); // TODO translate

                //
                // Views
                //

                node_id += 1;
                tree_builder.node(
                    NodeBuilder::dir(node_id)
                        .activatable(true)
                        .label_ui(|ui| {
                            ui.add(egui::Label::new("PCB").selectable(false)); // TODO translate
                        }),
                );

                node_id += 1;
                tree_builder.leaf(node_id, "Top"); // TODO translate
                node_id += 1;
                tree_builder.leaf(node_id, "Bottom"); // TODO translate

                tree_builder.close_dir();

                //
                // designs
                //

                node_id += 1;
                tree_builder.node(
                    NodeBuilder::dir(node_id)
                        .activatable(true)
                        .label_ui(|ui| {
                            ui.add(egui::Label::new("Designs").selectable(false)); // TODO translate
                        }),
                );

                for design in designs {
                    node_id += 1;
                    tree_builder.node(
                        NodeBuilder::dir(node_id)
                            .activatable(true)
                            .label_ui(|ui| {
                                ui.add(egui::Label::new(design.to_string()).selectable(false)); // TODO translate
                            }),
                    );

                    tree_builder.leaf(node_id, "Top");
                    tree_builder.leaf(node_id, "Bottom");

                    tree_builder.close_dir();
                }

                tree_builder.close_dir();

                //
                // units
                //
                node_id += 1;
                tree_builder.node(
                    NodeBuilder::dir(node_id)
                        .activatable(true)
                        .label_ui(|ui| {
                            ui.add(egui::Label::new("Units").selectable(false)); // TODO translate
                        }),
                );

                for index in 0_u16..units {
                    node_id += 1;

                    let assignment = match unit_map.get(&(index as PcbUnitIndex)) {
                        Some(name) => name,
                        None => "Unassigned", // TODO translate
                    };

                    let label = format!("{}: {}", index + 1, assignment);

                    tree_builder.leaf(node_id, label.to_string());
                }
                tree_builder.close_dir();
            },
        );
    }
}

impl Pcb {
    pub fn from_path(path: PathBuf, key: PcbKey) -> (Self, PcbUiCommand) {
        let instance = Self::new_inner(Some(path), key, None);
        (instance, PcbUiCommand::Load)
    }

    pub fn new(key: PcbKey) -> (Self, PcbUiCommand) {
        let instance = Self::new_inner(None, key, None);
        (instance, PcbUiCommand::Load)
    }

    fn new_inner(path: Option<PathBuf>, key: PcbKey, name: Option<String>) -> Self {
        debug!("Creating pcb instance from path. path: {:?}", path);

        let component: ComponentState<(PcbKey, PcbUiCommand)> = ComponentState::default();

        let core_service = PlannerCoreService::new();
        Self {
            planner_core_service: core_service,
            tree_view_state: Default::default(),
            path,
            modified: false,
            component,
        }
    }
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct PcbUiState {
    name: Option<String>,

    key: PcbKey,
    sender: Enqueue<(PcbKey, PcbUiCommand)>,
}

impl PcbUiState {
    pub fn new(key: PcbKey, name: Option<String>, sender: Enqueue<(PcbKey, PcbUiCommand)>) -> Self {
        Self {
            name,
            key,
            sender,
        }
    }
}

#[derive(Debug, Clone)]
pub enum PcbUiCommand {
    None,
    DebugMarkModified,
    Load,
    Save,
}

pub struct PcbContext {
    pub key: PcbKey,
}

impl UiComponent for Pcb {
    type UiContext<'context> = PcbContext;
    type UiCommand = (PcbKey, PcbUiCommand);
    type UiAction = PcbAction;

    fn ui<'context>(&self, ui: &mut Ui, context: &mut Self::UiContext<'context>) {
        let PcbContext {
            key,
        } = context;

        egui::SidePanel::left(ui.id().with("left_panel"))
            .resizable(true)
            .min_width(40.0)
            .show_inside(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.show_pcb_tree(ui, key);
                })
            });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.label("TODO: PCB UI"); // TODO

            if ui.button("mark modified").clicked() {
                self.component
                    .send((*key, PcbUiCommand::DebugMarkModified))
            }
        });
    }

    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        let (_key, command) = command;

        match command {
            PcbUiCommand::None => None,
            PcbUiCommand::DebugMarkModified => {
                self.modified = true;
                Some(PcbAction::SetModifiedState(self.modified))
            }
            PcbUiCommand::Load => {
                debug!("Loading pcb. path: {:?}", self.path);
                None
            }
            PcbUiCommand::Save => {
                debug!("Saving pcb. path: {:?}", self.path);
                None
            }
        }
    }
}
