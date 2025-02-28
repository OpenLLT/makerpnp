use std::mem::MaybeUninit;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use egui_mobius::types::{Enqueue, Value, ValueGuard};
use egui_dock::{DockArea, DockState, Style};
use egui_i18n::tr;
use egui_mobius::factory;
use egui_mobius::slot::Slot;
use slotmap::SlotMap;
use tracing::{debug, info};
use crate::config::Config;
use crate::{fonts, toolbar};
use crate::ui_commands::{handle_command, UiCommand};
use crate::file_picker::Picker;
use crate::project::{Project, ProjectKey};
use crate::tabs::{AppTabViewer, TabKey, Tabs};
use crate::ui_app::app_tabs::home::HomeTab;
use crate::ui_app::app_tabs::{TabContext, TabKind};
use crate::ui_app::app_tabs::project::ProjectTab;

pub mod app_tabs;
#[derive(serde::Deserialize, serde::Serialize)]
pub struct PersistentUiState {
    tabs: Value<Tabs<TabKind, TabContext>>,
    tree: DockState<TabKey>,
}

impl Default for PersistentUiState {
    fn default() -> Self {
        Self {
            tabs: Value::new(Tabs::new()),
            tree: DockState::new(vec![]),
        }
    }
}

impl PersistentUiState {
    pub fn show_home_tab(&mut self) {
        let home_tab = self.find_home_tab();

        if let Some(home_tab_key) = &home_tab {
            // although we have the tab, we don't know the tab_index, which is required for the call to `set_active_tab`,
            // so we have to call `find_tab`
            let find_result = self.tree.find_tab(home_tab_key).unwrap();
            self.tree.set_active_tab(find_result);
        } else {
            // create a new home tab
            let mut tabs = self.tabs.lock().unwrap();
            let tab_id = tabs.add(TabKind::Home(HomeTab::default()));
            self.tree.push_to_focused_leaf(tab_id);
        }
    }


    fn find_home_tab(&self) -> Option<&TabKey> {
        let home_tab = self
            .tree
            .iter_all_tabs()
            .find_map(|(_surface_and_node, tab_key)| {
                let tabs = self.tabs.lock().unwrap();
                let tab_kind = tabs.get(tab_key).unwrap();

                match tab_kind {
                    TabKind::Home(_) => Some(tab_key),
                    _ => None,
                }
            });
        home_tab
    }

    pub fn close_all_tabs(&mut self) {
        // FIXME there's a bug in `egui_dock` where the `on_close` handler is not called
        //       when programmatically closing all the tabs - reported via discord: https://discord.com/channels/900275882684477440/1075333382290026567/1340993744941617233
        self.tree.retain_tabs(|_tab_key| false);
    }

    fn add_tab(&mut self, tab_kind: TabKind) {
        let mut tabs = self.tabs.lock().unwrap();
        let tab_id = tabs.add(tab_kind);
        self.tree.push_to_focused_leaf(tab_id);
    }
}

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct UiApp {
    ui_state: Value<PersistentUiState>,

    config: Value<Config>,

    // state contains fields that cannot be initialized using 'Default'
    #[serde(skip)]
    state: MaybeUninit<Value<AppState>>,

    // the slot handler needs state, so slot can't be *in* state
    #[serde(skip)]
    slot: MaybeUninit<Slot<UiCommand>>,
}

pub struct AppState {
    // TODO find a better way of doing this that doesn't require this boolean
    startup_done: bool,
    file_picker: Picker,

    command_sender: Enqueue<UiCommand>,
    
    // TODO consider using `Value` here
    pub(crate) projects: Arc<Mutex<SlotMap<ProjectKey, Project>>>,
}

impl AppState {
    pub fn init(sender: Enqueue<UiCommand>) -> Self {

        Self {
            startup_done: false,
            file_picker: Picker::default(),

            command_sender: sender,
            projects: Arc::new(Mutex::new(SlotMap::default())),
        }
    }

    pub fn pick_file(&mut self) {
        if !self.file_picker.is_picking() {
            self.file_picker.pick_file();
        }
    }

    pub fn open_file(&mut self, path: PathBuf, ui_state: Value<PersistentUiState>) {
        info!("open file. path: {:?}", path);
        
        let label = path.file_name().unwrap().to_string_lossy().to_string();
        
        let sender = self.command_sender.clone();
        
        let project_key = self.projects.lock().unwrap().insert_with_key({
            let sender = sender.clone();
        
            |new_key| {
                Project::from_path(path.clone(), sender, new_key)
            }
        });
        let tab_kind = TabKind::Project(ProjectTab::new(label, path, project_key));
        
        ui_state.lock().unwrap().add_tab(tab_kind);
    }
    
    pub fn close_project(&mut self, project_key: ProjectKey) {
        debug!("closing project. key: {:?}", project_key);
        self.projects.lock().unwrap().remove(project_key);
    }
}



impl Default for UiApp {
    fn default() -> Self {
        Self {
            ui_state: Default::default(),
            config: Default::default(),
            state: MaybeUninit::uninit(),
            slot: MaybeUninit::uninit(),
        }
    }
}

impl UiApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.
        fonts::initialize(&cc.egui_ctx);

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        let mut instance = if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Self::default()
        };

        let (signal, slot) = factory::create_signal_slot::<UiCommand>();

        let sender = signal.sender.clone();
        
        let state = Value::new(AppState::init(sender));

        instance.state.write(state.clone());
        // Safety: `Self::state()` is now safe to call.

        let ui_state = instance.ui_state.clone();

        // Define a handler function for the slot
        let handler = {
            let command_sender = signal.sender.clone();
            let ui_state = ui_state.clone();

            move |command: UiCommand| {
                handle_command(state.clone(), ui_state.clone(), command, command_sender.clone());
            }
        };

        // Start the slot with the handler
        slot.start(handler);
        
        instance.slot.write(slot);
        
        

        instance
    }

    /// provide mutable access to the state.
    ///
    /// Safety: it's always safe, because `new` calls `state.write()`
    fn app_state(&mut self) -> ValueGuard<AppState> {
        unsafe { self.state.assume_init_mut().lock().unwrap() }
    }

    /// Safety: call only once on startup, before the tabs are shown.
    fn show_home_tab_on_startup(&mut self) {
        // TODO consider moving this method into `UiState`
        let mut ui_state = self.ui_state.lock().unwrap();

        if self.config.lock().unwrap().show_home_tab_on_startup {
            ui_state.show_home_tab();
        } else {
            if let Some(home_tab_key) = ui_state.find_home_tab() {
                let find_result = ui_state.tree.find_tab(home_tab_key).unwrap();
                ui_state.tree.remove_tab(find_result);
            }
        }
    }

    /// Due to bugs in egui_dock where it doesn't call `on_close` when closing tabs, it's possible that the tabs
    /// and the dock tree are out of sync.  `on_close` should be removing elements from `self.tabs` corresponding to the
    /// tab being closed, but because it is not called there can be orphaned elements, we need to find and remove them.
    pub fn cleanup_tabs(&mut self, tab_context: &mut TabContext) {
        // TODO consider moving this method into `UiState`
        let ui_state = self.ui_state.lock().unwrap();

        let known_tab_keys = ui_state
            .tree
            .iter_all_tabs()
            .map(|(_surface_and_node, tab_key)| tab_key.clone())
            .collect::<Vec<_>>();

        let mut tabs = ui_state.tabs.lock().unwrap();
        
        
        tabs.retain_all(&known_tab_keys, tab_context);
    }


    /// when the app starts up, the documents will be empty, and the document tabs will have keys that don't exist
    /// in the documents list (because it's empty now).
    /// we have to find these tabs, create documents, store them in the map and replace the tab's document key
    /// with the new key generated when adding the key to the map
    ///
    /// Safety: call only once on startup, before the tabs are shown.
    fn restore_documents_on_startup(&mut self) {
        // we have to do this as a two-step process to above borrow-checker issues
        // we also have to limit the scope of the access to ui_state and app_state

        // step 1 - find the document tabs, return the tab keys and paths.
        let tab_keys_and_paths = {
            let ui_state = self.ui_state.lock().unwrap();
            let mut tabs = ui_state.tabs.lock().unwrap();
            
            tabs
                .iter_mut()
                .filter_map(|(tab_key, tab_kind)| match tab_kind {
                    TabKind::Project(project_tab) => {
                        Some((tab_key.clone(), project_tab.path.clone()))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
        };

        // step 2 - store the documents and update the document key for the tab.
        for (tab_key, path) in tab_keys_and_paths {

            let new_key = {
                let app_state = self.app_state();
                let sender = app_state.command_sender.clone();

                app_state.projects.lock().unwrap().insert_with_key({
                    let sender = sender.clone();
                    |new_key| {
                        Project::from_path(path.clone(), sender, new_key)
                    }
                })
            };
            
            {
                let ui_state = self.ui_state.lock().unwrap();
                let mut tabs = ui_state.tabs.lock().unwrap();
                if let TabKind::Project(project_tab) = tabs.get_mut(&tab_key).unwrap() {
                    project_tab.project_key = new_key;
                } else {
                    unreachable!()
                }
            }
        }
    }

}

impl eframe::App for UiApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:

            egui::menu::bar(ui, |ui| {
                // NOTE: no File->Quit on web pages!
                let is_web = cfg!(target_arch = "wasm32");
                if !is_web {
                    ui.menu_button(tr!("menu-top-level-file"), |ui| {
                        if ui.button(tr!("menu-item-quit")).clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                    ui.add_space(16.0);
                }

                egui::widgets::global_theme_preference_buttons(ui);
            });

            toolbar::show(ui, self.app_state().command_sender.clone());
        });

        if !self.app_state().startup_done {
            self.app_state().startup_done = true;

            self.show_home_tab_on_startup();
            self.restore_documents_on_startup();
        }

        
        // in a block to limit the scope of the `ui_state` borrow/guard
        {
            let sender = self.app_state().command_sender.clone();
            let mut tab_context = TabContext {
                config: self.config.clone(),
                sender,
            };

            // FIXME remove this when `on_close` bugs in egui_dock are fixed.
            self.cleanup_tabs(&mut tab_context);

            let mut ui_state = self.ui_state.lock().unwrap();

            let mut my_tab_viewer = AppTabViewer {
                tabs: ui_state.tabs.clone(),
                context: &mut tab_context,
            };

            DockArea::new(&mut ui_state.tree)
                .style(Style::from_egui(ctx.style().as_ref()))
                .show(ctx, &mut my_tab_viewer);
        }

        let mut app_state = self.app_state();
        
        if let Ok(picked_file) = app_state.file_picker.picked() {
            // FIXME this `update` method does not get called immediately after picking a file, instead update gets
            //       called when the user moves the mouse or interacts with the window again.
            app_state.command_sender.send(UiCommand::OpenFile(picked_file)).ok();
        }

    }
}
