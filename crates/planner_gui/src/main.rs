extern crate core;

use std::path::PathBuf;
/// Run as follows:
/// `cargo run --package planner_gui --bin planner_gui`
///
/// To enable logging, set the environment variable appropriately, for example:
/// `RUST_LOG=debug,selectors::matching=info`

use cushy::{App, Application, Run};
use cushy::figures::units::Px;
use cushy::styles::components::IntrinsicPadding;
use cushy::value::{Destination, Dynamic, Source};
use cushy::widget::{IntoWidgetList, MakeWidget};
use cushy::widgets::label::Displayable;
use cushy::window::{PendingWindow, WindowHandle};
use slotmap::SlotMap;
use tracing::info;
use tracing_subscriber::{fmt, EnvFilter};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use unic_langid::LanguageIdentifier;
use crate::action::Action;
use crate::app_tabs::{TabKind, TabKindAction, TabKindMessage};
use crate::app_tabs::home::{HomeTab, HomeTabAction};
use crate::app_tabs::new::{NewTab, NewTabAction, NewTabMessage};
use crate::config::Config;
use crate::context::Context;
use crate::runtime::{Executor, MessageDispatcher, RunTime};
use crate::task::Task;
use crate::toolbar::ToolbarMessage;
use crate::widgets::tab_bar::{TabAction, TabBar, TabKey, TabMessage};

mod widgets;
mod action;
mod context;
mod app_core;
mod app_tabs;
mod config;
mod toolbar;
mod runtime;
mod task;

#[derive(Clone, Debug, Default)]
enum AppMessage {
    #[default]
    None,
    TabMessage(TabMessage<TabKindMessage>),
    ToolBarMessage(ToolbarMessage),
    ChooseFile(WindowHandle),
}

struct AppState {
    tab_bar: Dynamic<TabBar<TabKind, TabKindMessage, TabKindAction>>,
    config: Dynamic<Config>,
    context: Dynamic<Context>,

    message: Dynamic<AppMessage>,
}

#[cushy::main]
fn main(app: &mut App) -> cushy::Result {

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    info!("Started");


    let en_us_locale: LanguageIdentifier = "en-US".parse().unwrap();
    let es_es_locale: LanguageIdentifier = "es-ES".parse().unwrap();

    let languages: Vec<LanguageIdentifier> = vec![
        en_us_locale.clone(),
        es_es_locale.clone()
    ];

    let language_identifier: Dynamic<LanguageIdentifier> = Dynamic::new(languages.first().unwrap().clone());

    let translations = app.cushy().translations();
    translations
        .add(en_us_locale, include_str!("../assets/translations/en-US/translations.ftl").to_owned());
    translations
        .add(es_es_locale, include_str!("../assets/translations/es-ES/translations.ftl").to_owned());

    let message: Dynamic<AppMessage> = Dynamic::default();

    let (mut sender, receiver) = futures::channel::mpsc::unbounded();

    let executor = Executor::new().expect("should be able to create an executor");
    executor.spawn(MessageDispatcher::dispatch(receiver, message.clone()));
    let mut runtime = RunTime::new(executor, sender.clone());

    let pending = PendingWindow::default();
    let window = pending.handle();

    let config = Dynamic::new(config::load());

    let tab_message = Dynamic::default();
    tab_message.for_each_cloned({
        let message = message.clone();
        move |tab_message|{
            message.force_set(AppMessage::TabMessage(tab_message));
        }
    })
        .persist();

    let tab_bar = Dynamic::new(TabBar::new(&tab_message));

    let mut context = Context::default();
    context.provide(config.clone());
    context.provide(window);

    let context = Dynamic::new(context);

    let app_state = AppState {
        tab_bar: tab_bar.clone(),
        context: context.clone(),
        config,
        message: message.clone(),
    };

    let toolbar_message: Dynamic<ToolbarMessage> = Dynamic::default();
    toolbar_message.for_each_cloned({
        let message = message.clone();
        move |toolbar_message|{
            message.force_set(AppMessage::ToolBarMessage(toolbar_message));
        }
    })
        .persist();

    let toolbar = toolbar::make_toolbar(toolbar_message, language_identifier.clone(), &languages);

    let ui_elements = [
        toolbar.make_widget(),
        app_state.tab_bar.lock().make_widget(),
    ];

    let dyn_app_state = Dynamic::new(app_state);

    message
        .for_each_cloned({
            let dyn_app_state = dyn_app_state.clone();
            move |message|{
                let task = dyn_app_state.lock().update(message);

                if let Some(stream) = task::into_stream(task) {
                    runtime.run(stream);
                }
            }
        })
        .persist();

    let ui = pending.with_root(
        ui_elements
            .into_rows()
            .width(Px::new(640)..)
            .height(Px::new(480)..)
            .fit_vertically()
            .fit_horizontally()
            .with(&IntrinsicPadding, Px::new(4))
            .localized(language_identifier)
            .make_widget()
    )
        .on_close({
            let dyn_app_state = dyn_app_state.clone();
            let config = dyn_app_state.lock().config.clone();
            move ||{
                let config = config.lock();
                println!("Saving config");
                config::save(&*config);
            }
        })
        // TODO add translation support for the window title.
        .titled("MakerPnP - Planner");

    {
        let app_state_guard = dyn_app_state.lock();
        let app_state = &*app_state_guard;


        if app_state.config.lock().show_home_on_startup
        {
            add_home_tab(&context, &app_state.tab_bar);
        }
    }

    ui.open_centered(app)?;

    // FIXME control never returns here (at least on windows)

    Ok(())
}

fn add_home_tab(context: &Dynamic<Context>, tab_bar: &Dynamic<TabBar<TabKind, TabKindMessage, TabKindAction>>) {
    let mut tab_bar_guard = tab_bar
        .lock();

    let home_tab_result = tab_bar_guard.with_tabs(|mut iter|{
        iter.find_map(move |(_key, tab)|
            match tab {
                TabKind::Home(tab) => Some((_key, tab.clone())),
                _ => None
            }
        )
    });

    if let Some((key, _tab)) = home_tab_result {
        tab_bar_guard.activate(key);
    } else {
        tab_bar_guard
            .add_tab(context, TabKind::Home(HomeTab::default()));
    }
}

fn into_array<T, const N: usize>(v: Vec<T>) -> [T; N] {
    v.try_into()
        .unwrap_or_else(|v: Vec<T>| panic!("Incorrect element count. required: {}, actual: {}", N, v.len()))
}

impl AppState {
    fn update(&mut self, message: AppMessage) -> Task<AppMessage> {
        //println!("AppState::update, message: {:?}", message);
        match message {
            AppMessage::None => Task::none(),
            AppMessage::TabMessage(message) => {
                let action = self.tab_bar.lock()
                    .update(&self.context, message);

                self.on_tab_action(action)
            }
            AppMessage::ToolBarMessage(message) => {
                self
                    .on_toolbar_message(message)
            }
            AppMessage::ChooseFile(window) => {
                // TODO
                Task::none()
            }
        }
    }

    fn on_toolbar_message(&mut self, message: ToolbarMessage) -> Task<AppMessage> {
        match message {
            ToolbarMessage::None => {
                Task::none()
            }
            ToolbarMessage::HomeClicked => {
                println!("home clicked");

                add_home_tab(&self.context, &self.tab_bar);

                Task::none()
            }
            ToolbarMessage::NewClicked => {
                println!("New clicked");

                self.add_new_tab();

                Task::none()
            }
            ToolbarMessage::OpenClicked => {
                let window = self.context.lock().with_context::<WindowHandle, _, _>(|window_handle| {
                    window_handle.clone()
                }).unwrap();

                println!("open clicked");

                Task::done(AppMessage::ChooseFile(window))
            }
            ToolbarMessage::CloseAllClicked => {
                println!("close all clicked");
                let closed_tabs = self.tab_bar.lock().close_all();
                let tasks: Vec<_> = closed_tabs.into_iter().map(|(key, kind)| self.on_tab_closed(key, kind)).collect();

                Task::batch(tasks)
            }
        }
    }

    fn add_new_tab(&self) {
        let new_tab_message: Dynamic<NewTabMessage> = Dynamic::default();

        let tab_key = self.tab_bar.lock()
            .add_tab(&self.context, TabKind::New(NewTab::new(new_tab_message.clone())));

        new_tab_message.for_each_cloned({
            let message = self.message.clone();
            move |new_tab_message| {
                message.force_set(
                    AppMessage::TabMessage(
                        TabMessage::TabKindMessage(
                            tab_key,
                            TabKindMessage::NewTabMessage(new_tab_message)
                        )
                    )
                );
            }
        })
            .persist();
    }

    fn on_tab_action(&mut self, action: Action<TabAction<TabKindAction, TabKind>>) -> Task<AppMessage> {
        let action = action.into_inner();

        match action {
            TabAction::TabSelected(tab_key) => {
                println!("tab selected, key: {:?}", tab_key);
                Task::none()
            },
            TabAction::TabClosed(tab_key, tab) => {
                self.on_tab_closed(tab_key, tab);

                Task::none()
            },
            TabAction::TabAction(tab_key, tab_action) => {
                println!("tab action. key: {:?}, action: {:?}", tab_key, tab_action);
                match tab_action {
                    TabKindAction::HomeTabAction(_tab_key, action) => {
                        match action {
                            HomeTabAction::None => Task::none(),
                        }
                    },
                    TabKindAction::NewTabAction(tab_key, action) => {
                        match action {
                            NewTabAction::None => Task::none(),
                            NewTabAction::CreateProject(name, path) => {
                                self.create_project(tab_key, name, path)
                            }
                            NewTabAction::Task(task) => {
                                task.map(move |message| {
                                    AppMessage::TabMessage(TabMessage::TabKindMessage(tab_key, TabKindMessage::NewTabMessage(message)))
                                })
                            }
                        }
                    }
                }
            }
            TabAction::None => Task::none(),
        }
    }

    fn on_tab_closed(&mut self, tab_key: TabKey, tab: TabKind) -> Task<AppMessage> {
        println!("tab closed, key: {:?}", tab_key);
        match tab {
            TabKind::Home(_tab) => (),
            TabKind::New(_tab) => ()
        }
        Task::none()
    }

    fn create_project(&self, tab_key: TabKey, name: String, path: PathBuf) -> Task<AppMessage> {
        // TODO
        Task::none()
    }
}