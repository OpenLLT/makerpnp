use std::path::PathBuf;

use egui::{Ui, WidgetText};
use egui_dock::{DockArea, DockState, Node, NodeIndex, Split, Style, Tree};
use egui_mobius::types::{Enqueue, Value, ValueGuard};
use serde::{Deserialize, Serialize};
use slotmap::SlotMap;
use tracing::{error, trace};

use crate::config::Config;
use crate::pcb::{Pcb, PcbKey};
use crate::project::{Project, ProjectKey};
use crate::tabs::{AppTabViewer, Tab, TabKey, Tabs};
use crate::ui_app::app_tabs::home::{HomeTab, HomeTabAction, HomeTabContext, HomeTabUiCommand};
use crate::ui_app::app_tabs::new_pcb::{NewPcbTab, NewPcbTabAction, NewPcbTabContext, NewPcbTabUiCommand};
use crate::ui_app::app_tabs::new_project::{
    NewProjectTab, NewProjectTabAction, NewProjectTabContext, NewProjectTabUiCommand,
};
use crate::ui_app::app_tabs::pcb::{PcbTab, PcbTabAction, PcbTabUiCommand};
use crate::ui_app::app_tabs::project::{ProjectTab, ProjectTabAction, ProjectTabUiCommand};
use crate::ui_component::{ComponentState, UiComponent};

pub mod home;
pub mod new_pcb;
pub mod new_project;
pub mod pcb;
pub mod project;

pub struct TabKindContext {
    pub config: Value<Config>,
    pub projects: Value<SlotMap<ProjectKey, Project>>,
    pub pcbs: Value<SlotMap<PcbKey, Pcb>>,
}

#[derive(Deserialize, Serialize)]
pub enum TabKind {
    Home(HomeTab, #[serde(skip)] ComponentState<TabKindUiCommand>),
    NewProject(NewProjectTab, #[serde(skip)] ComponentState<TabKindUiCommand>),
    Project(ProjectTab, #[serde(skip)] ComponentState<TabKindUiCommand>),
    NewPcb(NewPcbTab, #[serde(skip)] ComponentState<TabKindUiCommand>),
    Pcb(PcbTab, #[serde(skip)] ComponentState<TabKindUiCommand>),
}

#[derive(Debug, Clone)]
pub enum TabKindUiCommand {
    HomeTabCommand { command: HomeTabUiCommand },
    NewProjectTabCommand { command: NewProjectTabUiCommand },
    ProjectTabCommand { command: ProjectTabUiCommand },
    NewPcbTabCommand { command: NewPcbTabUiCommand },
    PcbTabCommand { command: PcbTabUiCommand },
}

#[derive(Debug)]
pub enum TabKindAction {
    None,
    HomeTabAction { action: HomeTabAction },
    NewProjectTabAction { action: NewProjectTabAction },
    ProjectTabAction { action: ProjectTabAction },
    NewPcbTabAction { action: NewPcbTabAction },
    PcbTabAction { action: PcbTabAction },
}

impl Tab for TabKind {
    type Context = TabKindContext;

    fn label(&self) -> WidgetText {
        match self {
            TabKind::Home(tab, _) => tab.label(),
            TabKind::NewProject(tab, _) => tab.label(),
            TabKind::Project(tab, _) => tab.label(),
            TabKind::NewPcb(tab, _) => tab.label(),
            TabKind::Pcb(tab, _) => tab.label(),
        }
    }

    fn ui(&mut self, ui: &mut Ui, tab_key: &TabKey, context: &mut Self::Context) {
        UiComponent::ui(self, ui, &mut (tab_key.clone(), context));
    }

    fn on_close(&mut self, tab_key: &TabKey, context: &mut Self::Context) -> bool {
        match self {
            TabKind::Home(tab, _) => {
                let mut home_tab_context = HomeTabContext {
                    tab_key: tab_key.clone(),
                    config: context.config.clone(),
                };
                tab.on_close(tab_key, &mut home_tab_context)
            }
            TabKind::NewProject(tab, _) => {
                let mut new_project_tab_context = NewProjectTabContext {
                    tab_key: tab_key.clone(),
                };
                tab.on_close(tab_key, &mut new_project_tab_context)
            }
            TabKind::Project(tab, _) => {
                let mut project_tab_context = project::ProjectTabContext {
                    tab_key: tab_key.clone(),
                    projects: context.projects.clone(),
                };
                tab.on_close(tab_key, &mut project_tab_context)
            }
            TabKind::NewPcb(tab, _) => {
                let mut new_pcb_tab_context = NewPcbTabContext {
                    tab_key: tab_key.clone(),
                };
                tab.on_close(tab_key, &mut new_pcb_tab_context)
            }
            TabKind::Pcb(tab, _) => {
                let mut pcb_tab_context = pcb::PcbTabContext {
                    tab_key: tab_key.clone(),
                    pcbs: context.pcbs.clone(),
                };
                tab.on_close(tab_key, &mut pcb_tab_context)
            }
        }
    }
}

impl UiComponent for TabKind {
    type UiContext<'context> = (TabKey, &'context mut TabKindContext);
    type UiCommand = (TabKey, TabKindUiCommand);
    type UiAction = TabKindAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, context: &mut Self::UiContext<'context>) {
        let (tab_key, context) = context;
        let tab_key = *tab_key;

        match self {
            TabKind::Home(tab, _) => {
                let mut home_tab_context = HomeTabContext {
                    tab_key,
                    config: context.config.clone(),
                };
                tab.ui(ui, &mut home_tab_context)
            }
            TabKind::NewProject(tab, _) => {
                let mut new_project_tab_context = NewProjectTabContext {
                    tab_key,
                };
                tab.ui(ui, &mut new_project_tab_context)
            }
            TabKind::Project(tab, _) => {
                let mut project_tab_context = project::ProjectTabContext {
                    tab_key,
                    projects: context.projects.clone(),
                };
                tab.ui(ui, &mut project_tab_context)
            }
            TabKind::NewPcb(tab, _) => {
                let mut new_pcb_tab_context = NewPcbTabContext {
                    tab_key,
                };
                tab.ui(ui, &mut new_pcb_tab_context)
            }
            TabKind::Pcb(tab, _) => {
                let mut pcb_tab_context = pcb::PcbTabContext {
                    tab_key,
                    pcbs: context.pcbs.clone(),
                };
                tab.ui(ui, &mut pcb_tab_context)
            }
        }
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        let (tab_key, command) = command;
        let (_tab_key, context) = context;

        match (self, command) {
            (
                TabKind::Home(tab, _),
                TabKindUiCommand::HomeTabCommand {
                    command,
                },
            ) => {
                let mut home_tab_context = HomeTabContext {
                    tab_key,
                    config: context.config.clone(),
                };
                tab.update(command, &mut home_tab_context)
                    .map(|action| TabKindAction::HomeTabAction {
                        action,
                    })
            }
            (
                TabKind::NewProject(tab, _),
                TabKindUiCommand::NewProjectTabCommand {
                    command,
                },
            ) => {
                let mut new_project_tab_content = NewProjectTabContext {
                    tab_key,
                };
                tab.update(command, &mut new_project_tab_content)
                    .map(|action| TabKindAction::NewProjectTabAction {
                        action,
                    })
            }
            (
                TabKind::Project(tab, _),
                TabKindUiCommand::ProjectTabCommand {
                    command,
                },
            ) => {
                let mut project_tab_context = project::ProjectTabContext {
                    tab_key,
                    projects: context.projects.clone(),
                };
                tab.update(command, &mut project_tab_context)
                    .map(|action| TabKindAction::ProjectTabAction {
                        action,
                    })
            }
            (
                TabKind::NewPcb(tab, _),
                TabKindUiCommand::NewPcbTabCommand {
                    command,
                },
            ) => {
                let mut new_pcb_tab_content = NewPcbTabContext {
                    tab_key,
                };
                tab.update(command, &mut new_pcb_tab_content)
                    .map(|action| TabKindAction::NewPcbTabAction {
                        action,
                    })
            }
            (
                TabKind::Pcb(tab, _),
                TabKindUiCommand::PcbTabCommand {
                    command,
                },
            ) => {
                let mut project_tab_context = pcb::PcbTabContext {
                    tab_key,
                    pcbs: context.pcbs.clone(),
                };
                tab.update(command, &mut project_tab_context)
                    .map(|action| TabKindAction::PcbTabAction {
                        action,
                    })
            }
            _ => {
                // this can occur when adding new tab kinds or when the types are mismatched
                unreachable!()
            }
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct AppTabs {
    tabs: Value<Tabs<TabKind, TabKindContext>>,

    // Note: `tree` is wrapped in a value because `ui()` only gives us `&self`
    //       but dockstate needs a mutable tree.
    tree: Value<DockState<TabKey>>,

    #[serde(skip)]
    pub component: ComponentState<(TabKey, TabUiCommand)>,
}

impl Default for AppTabs {
    fn default() -> Self {
        // TODO dockstate needs translations, see https://docs.rs/egui_dock/latest/egui_dock/#translations

        Self {
            tabs: Value::new(Tabs::new()),
            tree: Value::new(DockState::new(vec![])),
            component: ComponentState::default(),
        }
    }
}

#[macro_export]
macro_rules! tabs_impl {
    ( $tab_kind:ident, $tab_context:ident ) => {
        //
        // methods common to all tab kinds
        //

        #[allow(dead_code)]
        pub fn close_all_tabs(&mut self) {
            let mut tree = self.tree.lock().unwrap();

            // FIXME there's a bug in `egui_dock` where the `on_close` handler is not called
            //       when programmatically closing all the tabs - reported via discord: https://discord.com/channels/900275882684477440/1075333382290026567/1340993744941617233
            tree.retain_tabs(|_tab_key| false);
        }

        /// Due to bugs in egui_dock where it doesn't call `on_close` when closing tabs, it's possible that the tabs
        /// and the dock tree are out of sync.  `on_close` should be removing elements from `self.tabs` corresponding to the
        /// tab being closed, but because it is not called there can be orphaned elements, we need to find and remove them.
        pub fn cleanup_tabs(&mut self, tab_context: &mut $tab_context) {
            let tree = self.tree.lock().unwrap();

            let known_tab_keys = tree
                .iter_all_tabs()
                .map(|(_surface_and_node, tab_key)| tab_key.clone())
                .collect::<Vec<_>>();

            let mut tabs = self.tabs.lock().unwrap();

            tabs.retain_all(&known_tab_keys, tab_context);
        }

        pub fn add_tab(&mut self, tab_kind: $tab_kind) -> TabKey {
            let mut tabs = self.tabs.lock().unwrap();
            let tab_key = tabs.add(tab_kind);

            let mut tree = self.tree.lock().unwrap();
            tree.push_to_focused_leaf(tab_key);

            tab_key
        }

        #[allow(dead_code)]
        pub fn find_tab<F>(&self, f: F) -> Option<TabKey>
        where
            F: Fn(&$tab_kind) -> bool,
        {
            let tree = self.tree.lock().unwrap();

            let tab = tree
                .iter_all_tabs()
                .find_map(|(_surface_and_node, tab_key)| {
                    let tabs = self.tabs.lock().unwrap();
                    let tab_kind = tabs.get(tab_key).unwrap();

                    match f(tab_kind) {
                        true => Some(*tab_key),
                        false => None,
                    }
                });

            tab
        }

        #[allow(dead_code)]
        pub fn filter_map<B, F>(&self, f: F) -> Vec<B>
        where
            F: FnMut((&TabKey, &$tab_kind)) -> Option<B>,
        {
            let tabs = self.tabs.lock().unwrap();
            tabs.iter()
                .filter_map(f)
                .collect::<Vec<_>>()
        }

        #[allow(dead_code)]
        pub fn with_tab_mut<F, O>(&self, tab_key: &TabKey, f: F) -> O
        where
            F: Fn(&mut $tab_kind) -> O,
        {
            let mut tabs = self.tabs.lock().unwrap();
            let mut tab = tabs.get_mut(tab_key).unwrap();
            f(&mut tab)
        }

        #[allow(dead_code)]
        pub fn show_tab<F>(&mut self, f: F) -> Result<TabKey, ()>
        where
            F: Fn(&$tab_kind) -> bool,
        {
            let tab = self.find_tab(f);

            let mut tree = self.tree.lock().unwrap();

            if let Some(tab_key) = tab {
                let find_result = tree.find_tab(&tab_key).unwrap();
                tree.set_active_tab(find_result);
                Ok(tab_key)
            } else {
                Err(())
            }
        }

        #[allow(dead_code)]
        pub fn active_tab(&self) -> Option<TabKey> {
            let mut tree = self.tree.lock().unwrap();
            tree.find_active_focused()
                .map(|(_, tab_key)| tab_key.clone())
        }

        #[allow(dead_code)]
        pub fn retain<F>(&mut self, f: F)
        where
            F: Fn(&TabKey, &$tab_kind) -> bool,
        {
            let tabs = self.tabs.lock().unwrap();
            let tab_keys_to_retain = tabs
                .iter()
                .filter_map(|(tab_key, tab_kind)| match f(tab_key, tab_kind) {
                    true => Some(tab_key.clone()),
                    false => None,
                })
                .collect::<Vec<_>>();

            let mut tree = self.tree.lock().unwrap();
            tree.retain_tabs(|tab_key| tab_keys_to_retain.contains(&tab_key));
        }

        #[allow(dead_code)]
        pub fn add_tab_to_second_leaf_or_split(&mut self, tab_kind: $tab_kind) -> TabKey {
            let mut tabs = self.tabs.lock().unwrap();
            let tab_key = tabs.add(tab_kind);

            let mut tree = self.tree.lock().unwrap();

            fn get_leaf_mut<T>(tree: &mut Tree<T>, target_index: usize) -> Option<&mut Node<T>> {
                tree.iter_mut()
                    .filter(|node| node.is_leaf())
                    .nth(target_index)
            }

            if let Some(leaf) = get_leaf_mut(tree.main_surface_mut(), 1) {
                leaf.append_tab(tab_key);
            } else {
                let [_old_node_index, _new_node_index] =
                    tree.main_surface_mut()
                        .split_tabs(NodeIndex::root(), Split::Right, 0.25, vec![tab_key]);
            }

            tab_key
        }
    };
}

impl AppTabs {
    tabs_impl!(TabKind, TabKindContext);

    pub fn replace(&mut self, tab_key: &TabKey, replacement_tab_kind: TabKind) -> Result<(), ()> {
        let mut tabs = self.tabs.lock().unwrap();

        if let Some(tab_kind) = tabs.get_mut(tab_key) {
            *tab_kind = replacement_tab_kind;
            Ok(())
        } else {
            Err(())
        }
    }
    //
    // methods specific to this instance
    //

    pub fn add_new_project_tab(&mut self) {
        // create a new project tab
        let tab_kind_component = ComponentState::default();

        let mut tabs = self.tabs.lock().unwrap();
        let new_project_tab = NewProjectTab::default();
        let tab_kind = TabKind::NewProject(new_project_tab, tab_kind_component);
        let tab_key = tabs.add(tab_kind);

        let tab_kind_sender = self.component.sender.clone();
        Self::configure_new_project_tab_mappers(tab_kind_sender, tabs, tab_key);

        let mut tree = self.tree.lock().unwrap();
        tree.push_to_focused_leaf(tab_key);
    }

    pub fn add_new_pcb_tab(&mut self) {
        // create a new pcb tab
        let tab_kind_component = ComponentState::default();

        let mut tabs = self.tabs.lock().unwrap();
        let new_pcb_tab = NewPcbTab::default();
        let tab_kind = TabKind::NewPcb(new_pcb_tab, tab_kind_component);
        let tab_key = tabs.add(tab_kind);

        let tab_kind_sender = self.component.sender.clone();
        Self::configure_new_pcb_tab_mappers(tab_kind_sender, tabs, tab_key);

        let mut tree = self.tree.lock().unwrap();
        tree.push_to_focused_leaf(tab_key);
    }

    fn configure_new_project_tab_mappers(
        tab_kind_sender: Enqueue<(TabKey, TabUiCommand)>,
        mut tabs: ValueGuard<Tabs<TabKind, TabKindContext>>,
        tab_key: TabKey,
    ) {
        match tabs.tabs.get_mut(&tab_key).unwrap() {
            TabKind::NewProject(new_project_tab, tab_kind_component) => {
                tab_kind_component.configure_mapper(tab_kind_sender, move |command| {
                    trace!("tab kind mapper. command: {:?}", command);
                    (tab_key, TabUiCommand::TabKindCommand(command))
                });

                let tab_kind_component_sender = tab_kind_component.sender.clone();

                new_project_tab
                    .component
                    .configure_mapper(tab_kind_component_sender, move |command| {
                        trace!("new project tab mapper. command: {:?}", command);
                        TabKindUiCommand::NewProjectTabCommand {
                            command,
                        }
                    });
            }
            _ => unreachable!(),
        }
    }

    fn configure_new_pcb_tab_mappers(
        tab_kind_sender: Enqueue<(TabKey, TabUiCommand)>,
        mut tabs: ValueGuard<Tabs<TabKind, TabKindContext>>,
        tab_key: TabKey,
    ) {
        match tabs.tabs.get_mut(&tab_key).unwrap() {
            TabKind::NewPcb(new_pcb_tab, tab_kind_component) => {
                tab_kind_component.configure_mapper(tab_kind_sender, move |command| {
                    trace!("tab kind mapper. command: {:?}", command);
                    (tab_key, TabUiCommand::TabKindCommand(command))
                });

                let tab_kind_component_sender = tab_kind_component.sender.clone();

                new_pcb_tab
                    .component
                    .configure_mapper(tab_kind_component_sender, move |command| {
                        trace!("new pcb tab mapper. command: {:?}", command);
                        TabKindUiCommand::NewPcbTabCommand {
                            command,
                        }
                    });
            }
            _ => unreachable!(),
        }
    }

    pub fn show_home_tab(&mut self) {
        self.show_tab(|candidate_tab| matches!(candidate_tab, TabKind::Home(..)))
            .inspect_err(|_| {
                // create a new home tab
                let tab_kind_component = ComponentState::default();

                let mut tabs = self.tabs.lock().unwrap();
                let home_tab = HomeTab::default();
                let tab_kind = TabKind::Home(home_tab, tab_kind_component);
                let tab_key = tabs.add(tab_kind);

                let tab_kind_sender = self.component.sender.clone();
                Self::configure_home_tab_mappers(tab_kind_sender, tabs, tab_key);

                let mut tree = self.tree.lock().unwrap();
                tree.push_to_focused_leaf(tab_key);
            })
            .ok();
    }

    pub fn show_pcb_tab(&mut self, path: &PathBuf) -> Result<TabKey, ()> {
        self.show_tab(|candidate_tab| matches!(candidate_tab, TabKind::Pcb(tab, _) if tab.path.eq(path)))
    }

    fn configure_home_tab_mappers(
        tab_kind_sender: Enqueue<(TabKey, TabUiCommand)>,
        mut tabs: ValueGuard<Tabs<TabKind, TabKindContext>>,
        tab_key: TabKey,
    ) {
        match tabs.tabs.get_mut(&tab_key).unwrap() {
            TabKind::Home(home_tab, tab_kind_component) => {
                tab_kind_component.configure_mapper(tab_kind_sender, move |command| {
                    trace!("tab kind mapper. command: {:?}", command);
                    (tab_key, TabUiCommand::TabKindCommand(command))
                });

                let tab_kind_component_sender = tab_kind_component.sender.clone();

                home_tab
                    .component
                    .configure_mapper(tab_kind_component_sender, move |command| {
                        trace!("home tab mapper. command: {:?}", command);
                        TabKindUiCommand::HomeTabCommand {
                            command,
                        }
                    });
            }
            _ => unreachable!(),
        }
    }

    pub fn find_home_tab(&self) -> Option<TabKey> {
        self.find_tab(|candidate_tab| matches!(candidate_tab, TabKind::Home(..)))
    }

    /// Safety: call only once on startup, before the tabs are shown.
    pub fn show_home_tab_on_startup(&mut self, show_home_tab_on_startup: bool) {
        if show_home_tab_on_startup {
            // the home tab's components won't be wired up, because they got restored with `Default`, so we have to fix it
            if let Some(home_tab_key) = self.find_home_tab() {
                let tabs = self.tabs.lock().unwrap();
                let tab_kind_sender = self.component.sender.clone();
                Self::configure_home_tab_mappers(tab_kind_sender, tabs, home_tab_key);
            }

            self.show_home_tab();
        } else if let Some(home_tab_key) = self.find_home_tab() {
            // TODO refactor into 'retain_tabs(...)` or `remove_tab(...)`
            let mut tree = self.tree.lock().unwrap();
            let find_result = tree.find_tab(&home_tab_key).unwrap();

            tree.remove_tab(find_result);
        }
    }
}

#[derive(Debug, Clone)]
pub enum TabUiCommand {
    TabKindCommand(TabKindUiCommand),
}

#[derive(Debug)]
pub enum TabAction {
    None,
    TabKindAction { action: TabKindAction },
}

impl UiComponent for AppTabs {
    type UiContext<'context> = TabKindContext;
    type UiCommand = (TabKey, TabUiCommand);
    type UiAction = TabAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, context: &mut Self::UiContext<'context>) {
        let ctx = ui.ctx();

        let mut app_tab_viewer = AppTabViewer {
            tabs: self.tabs.clone(),
            context,
        };

        let mut tree = self.tree.lock().unwrap();

        DockArea::new(&mut tree)
            .id(ui.id().with("app-tabs"))
            .style(Style::from_egui(ctx.style().as_ref()))
            .show(ctx, &mut app_tab_viewer);
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        let (tab_key, command) = command;
        match command {
            TabUiCommand::TabKindCommand(tab_kind_command) => {
                let mut tabs = self.tabs.lock().unwrap();
                if let Some(tab_kind) = tabs.get_mut(&tab_key) {
                    let tab_action = tab_kind.update((tab_key, tab_kind_command), &mut (tab_key, context));

                    tab_action.map(|action| TabAction::TabKindAction {
                        action,
                    })
                } else {
                    error!("tab not found: {:?}", tab_key);
                    None
                }
            }
        }
    }
}
