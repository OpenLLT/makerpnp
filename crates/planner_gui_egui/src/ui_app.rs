use std::mem::MaybeUninit;
use egui_mobius::types::{Enqueue, Value};
use egui_dock::{DockArea, DockState, Style};
use egui_i18n::tr;
use egui_mobius::factory;
use egui_mobius::slot::Slot;
use crate::config::Config;
use crate::{fonts, toolbar};
use crate::ui_commands::{handle_command, UiCommand};
use crate::app_core;
use crate::tabs::{AppTabViewer, TabKey, Tabs};
use crate::ui_app::app_tabs::home::HomeTab;
use crate::ui_app::app_tabs::{TabContext, TabKind};

pub mod app_tabs;
#[derive(serde::Deserialize, serde::Serialize)]
pub struct UiState {
    tabs: Value<Tabs<TabKind>>,
    tree: DockState<TabKey>,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            tabs: Value::new(Tabs::new()),
            tree: DockState::new(vec![]),
        }
    }
}

impl UiState {
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
                let mut tabs = self.tabs.lock().unwrap();
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

}

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct UiApp {
    ui_state: Value<UiState>,

    config: Config,

    // state contains fields that cannot be initialized using 'Default'
    #[serde(skip)]
    state: MaybeUninit<AppState>,
}

pub struct AppState {
    // TODO find a better way of doing this that doesn't require this boolean
    startup_done: bool,

    command_sender: Enqueue<UiCommand>,
    slot: Slot<UiCommand>,
}

impl AppState {
    pub fn init() -> Self {
        let (signal, slot) = factory::create_signal_slot::<UiCommand>();

        let ui_state = Value::new(UiState::default());

        let core_service = Value::new(app_core::CoreService::new(ui_state.clone()));

        // Define a handler function for the slot
        let handler = {
            let core_service = core_service.clone();
            let command_sender = signal.sender.clone();
            let ui_state = ui_state.clone();

            move |command: UiCommand| {
                handle_command(ui_state.clone(), command, core_service.clone(), command_sender.clone());
            }
        };

        // Start the slot with the handler
        slot.start(handler);

        Self {
            startup_done: false,

            command_sender: signal.sender.clone(),
            slot
        }
    }
}



impl Default for UiApp {
    fn default() -> Self {
        let config = Config::default();

        Self {
            ui_state: Default::default(),
            config,
            state: MaybeUninit::uninit(),
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

        instance.state.write(AppState::init());
        // Safety: `Self::state()` is now safe to call.

        instance
    }

    /// provide mutable access to the state.
    ///
    /// Safety: it's always safe, because `new` calls `state.write()`
    ///
    /// Note: it's either `self.state()` everywhere or `self.state.unwrap()` if `AppSate` was wrapped in an `Option`
    /// instead if `MaybeUninit`, this is less verbose.
    fn state(&mut self) -> &mut AppState {
        unsafe { self.state.assume_init_mut() }
    }

    /// Safety: call only once on startup, before the tabs are shown.
    fn show_home_tab_on_startup(&mut self) {
        // TODO consider moving this method into `UiState`
        let mut ui_state = self.ui_state.lock().unwrap();

        if self.config.show_home_tab_on_startup {
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
    pub fn cleanup_tabs(&mut self) {
        // TODO consider moving this method into `UiState`
        let mut ui_state = self.ui_state.lock().unwrap();

        let known_tab_keys = ui_state
            .tree
            .iter_all_tabs()
            .map(|(_surface_and_node, tab_key)| tab_key.clone())
            .collect::<Vec<_>>();

        let mut tabs = ui_state.tabs.lock().unwrap();
        tabs.retain_all(&known_tab_keys);
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

            toolbar::show(ui, self.state().command_sender.clone());
        });

        if !self.state().startup_done {
            self.state().startup_done = true;

            self.show_home_tab_on_startup();
        }

        // FIXME remove this when `on_close` bugs in egui_dock are fixed.
        self.cleanup_tabs();

        let mut tab_context = TabContext {
            config: &mut self.config,
        };

        let mut ui_state = self.ui_state.lock().unwrap();

        let mut my_tab_viewer = AppTabViewer {
            tabs: ui_state.tabs.clone(),
            context: &mut tab_context,
        };

        DockArea::new(&mut ui_state.tree)
            .style(Style::from_egui(ctx.style().as_ref()))
            .show(ctx, &mut my_tab_viewer);

    }
}
