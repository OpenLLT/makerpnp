use std::collections::HashMap;
use egui::{Ui, WidgetText};
use egui_i18n::{tr, translate_fluent};
use egui_mobius::types::Enqueue;
use petgraph::Graph;
use petgraph::graph::NodeIndex;
use i18n::fluent_argument_helpers::planner_app::build_fluent_args;
use planner_app::{ProjectTreeItem, ProjectTreeView};
use crate::project::{project_path_from_view_path, view_path_from_project_path, ProjectKey, ProjectPath, ProjectTabContext, ProjectUiCommand};
use crate::tabs::{Tab, TabKey};

#[derive(Debug)]
pub struct ProjectExplorerUi {
    project_tree_view: Option<ProjectTreeView>,
    project_tree_state: HashMap<NodeIndex, bool>,
    sender: Enqueue<(ProjectKey, ProjectUiCommand)>,
}

impl ProjectExplorerUi {
    pub fn new(sender: Enqueue<(ProjectKey, ProjectUiCommand)>) -> Self {
        Self {
            project_tree_view: None,
            project_tree_state: HashMap::new(),
            sender,
        }
    }

    fn show_project_tree(&self, ui: &mut egui::Ui, graph: &Graph<ProjectTreeItem, ()>, node: NodeIndex, selection_state: &HashMap<NodeIndex, bool>, project_key: &ProjectKey) {
        let item = &graph[node];

        let path = project_path_from_view_path(&item.path);

        let key = format!("project-explorer-node-{}", item.key);
        let args = build_fluent_args(&item.args);

        let label = translate_fluent(&key, &args);

        let mut is_selected = if let Some(value) = selection_state.get(&node) {
            *value
        } else {
            false
        };

        let id = ui.make_persistent_id(node);

        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true)
            .show_header(ui, |ui| {
                if ui.toggle_value(&mut is_selected, label).clicked() {
                    self.sender.send((*project_key, ProjectUiCommand::Navigate(path))).expect("sent");
                }
            })
            .body(|ui| {
                for neighbor in graph.neighbors(node) {
                    self.show_project_tree(ui, graph, neighbor, selection_state, project_key);
                }
            });
    }

    pub fn select_path(&mut self, project_path: &ProjectPath) {
        // it's an error to be given a path without a corresponding tree view node that has the same path
        let path = view_path_from_project_path(project_path).unwrap();

        let graph = &self.project_tree_view.as_mut().unwrap().tree;

        if let Some(node) = graph.node_indices().find(|&index| {
            graph[index].path.eq(&path)
        }) {
            self.project_tree_state.clear();
            self.project_tree_state.insert(node, true);
        } else {
            unreachable!()
        }
    }

    pub fn update_tree(&mut self, project_tree_view: ProjectTreeView) {
        self.project_tree_view.replace(project_tree_view);
    }
}

#[derive(Default, Debug)]
pub struct ProjectExplorerTab {
}

impl Tab for ProjectExplorerTab {
    type Context = ProjectTabContext;

    fn label(&self) -> WidgetText {
        let title = tr!("project-explorer-tab-label");

        egui::widget_text::WidgetText::from(title)
    }

    fn ui<'a>(&mut self, ui: &mut Ui, tab_key: &TabKey, context: &mut Self::Context) {
        let state = context.state.lock().unwrap();
        if let Some(tree) = &state.project_tree.project_tree_view {
            state.project_tree.show_project_tree(ui, &tree.tree, NodeIndex::new(0), &state.project_tree.project_tree_state, &context.key);
        } else {
            ui.centered_and_justified(|ui|{
                ui.spinner();
            });
        }
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, _context: &mut Self::Context) -> bool {
        true
    }
}
