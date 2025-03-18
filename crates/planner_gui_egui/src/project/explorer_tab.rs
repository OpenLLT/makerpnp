use derivative::Derivative;
use egui::{Ui, WidgetText};
use egui_i18n::{tr, translate_fluent};
use egui_ltreeview::{TreeView, TreeViewBuilder, TreeViewState};
use egui_mobius::types::{Enqueue, Value};
use i18n::fluent_argument_helpers::planner_app::build_fluent_args;
use petgraph::Graph;
use petgraph::graph::NodeIndex;
use planner_app::{ProjectTreeItem, ProjectTreeView};
use tracing::trace;

use crate::project::tabs::ProjectTabContext;
use crate::project::{
    ProjectKey, ProjectPath, ProjectUiCommand, project_path_from_view_path, view_path_from_project_path,
};
use crate::tabs::{Tab, TabKey};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct ExplorerUi {
    project_tree_view: Option<ProjectTreeView>,

    // tuple struct containing old state and new state, state changes trigger events.
    #[derivative(Debug = "ignore")]
    tree_view_state: Value<(TreeViewState<usize>, TreeViewState<usize>)>,
    sender: Enqueue<(ProjectKey, ProjectUiCommand)>,
}

impl ExplorerUi {
    pub fn new(sender: Enqueue<(ProjectKey, ProjectUiCommand)>) -> Self {
        Self {
            project_tree_view: None,
            tree_view_state: Default::default(),
            sender,
        }
    }

    fn show_project_tree(
        &self,
        ui: &mut Ui,
        graph: &Graph<ProjectTreeItem, ()>,
        node: NodeIndex,
        project_key: &ProjectKey,
    ) {
        let mut tree_view_state = self.tree_view_state.lock().unwrap();

        TreeView::new(ui.make_persistent_id("project_explorer_tree")).show_state(
            ui,
            &mut tree_view_state.0,
            |builder: &mut egui_ltreeview::TreeViewBuilder<'_, usize>| {
                self.show_project_tree_inner(builder, graph, node, project_key);
            },
        );

        let selected = tree_view_state.0.selected();
        let previously_selected = tree_view_state.1.selected();
        if selected != previously_selected {
            if selected.len() == 1 {
                let node = *selected.first().unwrap();
                let item = &graph[NodeIndex::new(node)];
                let path = project_path_from_view_path(&item.path);

                self.sender
                    .send((*project_key, ProjectUiCommand::Navigate(path)))
                    .expect("sent");
            } else {
                trace!("multi-select");
            }
        }

        tree_view_state.1 = tree_view_state.0.clone();
    }

    fn show_project_tree_inner(
        &self,
        tree_builder: &mut TreeViewBuilder<usize>,
        graph: &Graph<ProjectTreeItem, ()>,
        node: NodeIndex,
        project_key: &ProjectKey,
    ) {
        let item = &graph[node];

        let key = format!("project-explorer-node-{}", item.key);
        let args = build_fluent_args(&item.args);

        let label = translate_fluent(&key, &args);

        let node_id = node.index();

        let mut node_created: bool = false;
        for neighbour in graph.neighbors(node) {
            if !node_created {
                tree_builder.dir(node_id, &label);
                node_created = true;
            }
            self.show_project_tree_inner(tree_builder, graph, neighbour, project_key);
        }
        if node_created {
            tree_builder.close_dir();
        } else {
            tree_builder.leaf(node_id, &label);
        }
    }

    pub fn select_path(&mut self, project_path: &ProjectPath) {
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
            tree_view_state
                .0
                .set_one_selected(node_index);
            tree_view_state
                .0
                .expand_node(node_index);
        } else {
            unreachable!()
        }
    }

    pub fn update_tree(&mut self, project_tree_view: ProjectTreeView) {
        self.project_tree_view
            .replace(project_tree_view);
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
        if let Some(tree) = &state.project_tree.project_tree_view {
            state
                .project_tree
                .show_project_tree(ui, &tree.tree, NodeIndex::new(0), &context.key);
        } else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
        }
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, _context: &mut Self::Context) -> bool {
        true
    }
}
