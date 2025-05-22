use std::collections::HashMap;
use std::path::PathBuf;

use derivative::Derivative;
use egui::Ui;
use egui_ltreeview::{NodeBuilder, TreeView, TreeViewState};
use egui_mobius::Value;
use egui_mobius::types::Enqueue;
use planner_app::{Event, FileReference, PcbOverview, PcbSide, PcbUnitIndex, PcbView};
use slotmap::new_key_type;
use tracing::debug;

use crate::pcb::core_helper::PcbCoreHelper;
use crate::planner_app_core::{PlannerCoreService, PlannerError};
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
    modified: bool,

    #[derivative(Debug = "ignore")]
    tree_view_state: Value<TreeViewState<usize>>,

    pcb_overview: Option<PcbOverview>,

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
            pcb_overview: None,
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
    Error(PlannerError),
    PcbView(PcbView),
    // FIXME don't care about projects, don't care ablout all pcbs, care about *this* PCB.
    SetModifiedState {
        project_modified: bool,
        pcbs_modified: bool,
    },
    Loaded,
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
            if let Some(pcb_overview) = &self.pcb_overview {
                ui.label(&pcb_overview.name);
                // TODO expand the ui, show different content based on tree view selection
            } else {
                ui.spinner();
            }

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
        let (key, command) = command;

        match command {
            PcbUiCommand::None => None,
            PcbUiCommand::DebugMarkModified => {
                self.modified = true;
                Some(PcbAction::SetModifiedState(self.modified))
            }
            PcbUiCommand::Load => {
                debug!("Loading pcb. path: {:?}", self.path);

                if let Some(path) = &self.path {
                    let pcb_file = FileReference::Absolute(path.clone());

                    self.planner_core_service
                        .update(Event::LoadPcb {
                            pcb_file,
                            root: None,
                        })
                        .when_ok(key, |_| Some(PcbUiCommand::Loaded))
                } else {
                    None
                }
            }
            PcbUiCommand::Save => {
                debug!("Saving pcb. path: {:?}", self.path);
                // TODO
                None
            }
            PcbUiCommand::Error(_) => {
                // TODO
                None
            }
            PcbUiCommand::PcbView(view) => {
                match view {
                    PcbView::PcbOverview(pcb_overview) => {
                        debug!("Received pcb overview.");
                        self.pcb_overview = Some(pcb_overview);
                    }
                }
                // TODO
                None
            }
            PcbUiCommand::SetModifiedState {
                ..
            } => {
                //TODO
                None
            }
            PcbUiCommand::Loaded => {
                debug!("Loaded pcb. path: {:?}", self.path);
                if let Some(path) = &self.path {
                    self.planner_core_service
                        .update(Event::RequestPcbOverviewView {
                            path: path.clone(),
                        })
                        .when_ok(key, |_| None)
                } else {
                    None
                }
            }
        }
    }
}

mod core_helper {
    use tracing::warn;

    use crate::pcb::{PcbAction, PcbKey, PcbUiCommand};
    use crate::planner_app_core::{PlannerAction, PlannerError};
    use crate::task::Task;

    #[must_use]
    fn when_ok_inner<F>(
        result: Result<Vec<PlannerAction>, PlannerError>,
        project_key: PcbKey,
        f: F,
    ) -> Option<PcbAction>
    where
        F: FnOnce(&mut Vec<Task<PcbAction>>) -> Option<PcbUiCommand>,
    {
        match result {
            Ok(actions) => {
                let mut tasks = vec![];
                let effect_tasks: Vec<Task<PcbAction>> = actions
                    .into_iter()
                    .map(|planner_action| {
                        let project_action = into_project_action(planner_action);
                        Task::done(project_action)
                    })
                    .collect();

                tasks.extend(effect_tasks);

                if let Some(command) = f(&mut tasks) {
                    let final_task = Task::done(PcbAction::UiCommand(command));
                    tasks.push(final_task);
                }

                let action = PcbAction::Task(project_key, Task::batch(tasks));

                Some(action)
            }
            Err(error) => Some(PcbAction::UiCommand(PcbUiCommand::Error(error))),
        }
    }

    fn into_actions_inner(result: Result<Vec<PlannerAction>, PlannerError>) -> Result<Vec<PcbAction>, PcbAction> {
        match result {
            Ok(actions) => Ok(actions
                .into_iter()
                .map(into_project_action)
                .collect()),
            Err(error) => Err(PcbAction::UiCommand(PcbUiCommand::Error(error))),
        }
    }

    fn into_project_action(action: PlannerAction) -> PcbAction {
        match action {
            PlannerAction::SetModifiedState {
                project_modified,
                pcbs_modified,
            } => PcbAction::UiCommand(PcbUiCommand::SetModifiedState {
                project_modified,
                pcbs_modified,
            }),
            PlannerAction::ProjectView(_project_view) => {
                warn!("pcb received project view action. ignoring.");
                PcbAction::UiCommand(PcbUiCommand::None)
            }
            PlannerAction::PcbView(pcb_view) => PcbAction::UiCommand(PcbUiCommand::PcbView(pcb_view)),
        }
    }

    pub trait PcbCoreHelper {
        fn into_actions(self) -> Result<Vec<PcbAction>, PcbAction>;
        fn when_ok<F>(self, project_key: PcbKey, f: F) -> Option<PcbAction>
        where
            F: FnOnce(&mut Vec<Task<PcbAction>>) -> Option<PcbUiCommand>;
    }

    impl PcbCoreHelper for Result<Vec<PlannerAction>, PlannerError> {
        fn into_actions(self) -> Result<Vec<PcbAction>, PcbAction> {
            into_actions_inner(self)
        }

        fn when_ok<F>(self, project_key: PcbKey, f: F) -> Option<PcbAction>
        where
            F: FnOnce(&mut Vec<Task<PcbAction>>) -> Option<PcbUiCommand>,
        {
            when_ok_inner(self, project_key, f)
        }
    }
}
