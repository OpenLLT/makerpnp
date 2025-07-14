use std::borrow::Cow;
use std::path::PathBuf;

use derivative::Derivative;
use egui::{Ui, WidgetText};
use egui_dock::tab_viewer::OnCloseResponse;
use egui_i18n::{tr, translate_fluent};
use egui_ltreeview::{Action, Activate, NodeBuilder, TreeView, TreeViewBuilder, TreeViewState};
use egui_mobius::types::Value;
use i18n::fluent_argument_helpers::planner_app::build_fluent_args;
use petgraph::Graph;
use petgraph::graph::NodeIndex;
use planner_app::{Arg, ProjectTreeItem, ProjectTreeView};
use util::path::clip_path;

use crate::project::tabs::ProjectTabContext;
use crate::project::{project_path_from_view_path, view_path_from_project_path};
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};
use crate::ui_util::NavigationPath;

#[derive(Derivative)]
#[derivative(Debug)]
pub struct ExplorerTabUi {
    project_directory: PathBuf,
    project_tree_view: Option<ProjectTreeView>,

    #[derivative(Debug = "ignore")]
    tree_view_state: Value<TreeViewState<usize>>,

    pub component: ComponentState<ExplorerTabUiCommand>,
}

impl ExplorerTabUi {
    pub fn new(project_directory: PathBuf) -> Self {
        Self {
            project_directory,
            project_tree_view: None,
            tree_view_state: Default::default(),
            component: Default::default(),
        }
    }

    fn show_project_tree(&self, ui: &mut Ui, graph: &Graph<ProjectTreeItem, ()>, node: NodeIndex) {
        let mut tree_view_state = self.tree_view_state.lock().unwrap();

        let (_response, actions) = TreeView::new(ui.make_persistent_id("project_explorer_tree")).show_state(
            ui,
            &mut tree_view_state,
            |builder: &mut egui_ltreeview::TreeViewBuilder<'_, usize>| {
                self.show_project_tree_inner(builder, graph, node);
            },
        );

        for action in actions {
            if let Action::Activate(Activate {
                selected,
                modifiers,
            }) = action
            {
                let _ = modifiers;
                for &node_id in &selected {
                    let item = &graph[NodeIndex::new(node_id)];
                    let path = project_path_from_view_path(&item.path);

                    self.component
                        .send(ExplorerTabUiCommand::Navigate(path));

                    // HACK: tree-view-dir-activate-expand-hack
                    tree_view_state.expand_node(&node_id);
                }
            }
        }
    }

    fn show_project_tree_inner(
        &self,
        tree_builder: &mut TreeViewBuilder<usize>,
        graph: &Graph<ProjectTreeItem, ()>,
        node: NodeIndex,
    ) {
        let item = &graph[node];

        fn handle_phase_loadout<'p>(
            default_key: String,
            item: &'p ProjectTreeItem,
            project_directory: &'_ PathBuf,
        ) -> Result<(String, Cow<'p, ProjectTreeItem>), ()> {
            if !item.key.eq("phase-loadout") {
                return Err(());
            }

            //
            // create a clipped load-out-source-path if possible
            //
            let Arg::String(load_out_source) = &item.args["source"] else {
                return Err(());
            };

            let load_out_source_path = PathBuf::from(load_out_source);

            let clipped_load_out_source = clip_path(project_directory.clone(), load_out_source_path, None);

            let mut item = item.clone();
            item.args
                .insert("source".to_string(), Arg::String(clipped_load_out_source));

            let item: Cow<'p, ProjectTreeItem> = Cow::Owned(item);

            Ok((default_key, item))
        }

        fn handle_unit_assignment<'p>(
            _default_key: String,
            item: &'p ProjectTreeItem,
            _project_directory: &'_ PathBuf,
        ) -> Result<(String, Cow<'p, ProjectTreeItem>), ()> {
            if !item.key.eq("unit-assignment") {
                return Err(());
            }

            fn contains_all(values: &[&String], required: &[&str]) -> bool {
                required
                    .iter()
                    .all(|item| values.iter().any(|s| s == item))
            }

            let required_keys = ["variant_name"];

            let keys: Vec<&String> = item.args.keys().collect();
            let key = if contains_all(&keys, &required_keys) {
                "project-explorer-node-unit-assignment-assigned".to_string()
            } else {
                "project-explorer-node-unit-assignment-unassigned".to_string()
            };

            Ok((key, Cow::Borrowed(item)))
        }

        fn default_handler<'p>(
            default_key: String,
            item: &'p ProjectTreeItem,
            _project_directory: &'_ PathBuf,
        ) -> Result<(String, Cow<'p, ProjectTreeItem>), ()> {
            Ok((default_key, Cow::Borrowed(item)))
        }

        // some items need additional processing
        let handlers = [handle_phase_loadout, handle_unit_assignment, default_handler];

        let default_key = format!("project-explorer-node-{}", item.key);

        let (key, item) = handlers
            .iter()
            .find_map(|handler| handler(default_key.clone(), item, &self.project_directory).ok())
            .unwrap();

        let args = build_fluent_args(&item.args);

        let label = translate_fluent(&key, &args);

        let node_id = node.index();

        let mut node_created: bool = false;
        for &neighbour in graph
            .neighbors(node)
            .collect::<Vec<_>>()
            .iter()
            .rev()
        {
            if !node_created {
                tree_builder.node(
                    NodeBuilder::dir(node_id)
                        .activatable(true)
                        .label_ui(|ui| {
                            ui.add(egui::Label::new(label.clone()).selectable(false));
                        }),
                );
                node_created = true;
            }
            self.show_project_tree_inner(tree_builder, graph, neighbour);
        }
        if node_created {
            tree_builder.close_dir();
        } else {
            tree_builder.leaf(node_id, &label);
        }
    }

    pub fn select_path(&mut self, project_path: &NavigationPath) {
        // it's an error to be given a path without a corresponding tree view node that has the same path
        let path = view_path_from_project_path(project_path).unwrap();

        let graph = &self
            .project_tree_view
            .as_mut()
            .unwrap()
            .tree;

        if let Some(node) = graph
            .node_indices()
            .find(|&index| graph[index].path.eq(&path))
        {
            let mut tree_view_state = self.tree_view_state.lock().unwrap();

            let node_index = node.index();

            let mut selection = tree_view_state.selected().clone();
            if !selection.contains(&node_index) {
                selection.push(node_index)
            }

            tree_view_state.set_selected(selection);
        } else {
            unreachable!()
        }
    }

    pub fn update_tree(&mut self, project_tree_view: ProjectTreeView) {
        self.project_tree_view
            .replace(project_tree_view);
    }
}

#[derive(Debug, Clone)]
pub enum ExplorerTabUiCommand {
    Navigate(NavigationPath),
}

#[derive(Debug, Clone)]
pub enum ExplorerTabUiAction {
    Navigate(NavigationPath),
}

#[derive(Debug, Clone, Default)]
pub struct ExplorerTabUiContext {}

impl UiComponent for ExplorerTabUi {
    type UiContext<'context> = ExplorerTabUiContext;
    type UiCommand = ExplorerTabUiCommand;
    type UiAction = ExplorerTabUiAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        if let Some(tree) = &self.project_tree_view {
            self.show_project_tree(ui, &tree.tree, NodeIndex::new(0));
        } else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
        }
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            ExplorerTabUiCommand::Navigate(path) => {
                self.select_path(&path);

                Some(ExplorerTabUiAction::Navigate(path))
            }
        }
    }
}

#[derive(Default, Debug, serde::Deserialize, serde::Serialize)]
pub struct ExplorerTab {}

impl Tab for ExplorerTab {
    type Context = ProjectTabContext;

    fn label(&self) -> WidgetText {
        let title = tr!("project-explorer-tab-label");

        egui::widget_text::WidgetText::from(title)
    }

    fn ui<'a>(&mut self, ui: &mut Ui, _tab_key: &TabKey, context: &mut Self::Context) {
        let state = context.state.lock().unwrap();
        UiComponent::ui(&state.explorer_tab_ui, ui, &mut ExplorerTabUiContext::default());
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, _context: &mut Self::Context) -> OnCloseResponse {
        OnCloseResponse::Close
    }
}
