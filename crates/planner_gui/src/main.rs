/// Run as follows:
/// `cargo run --package planner_gui --bin planner_gui`
///
/// To enable logging, set the environment variable appropriately, for example:
/// `RUST_LOG=debug,selectors::matching=info`
extern crate core;

use std::path;
use std::path::PathBuf;

use cushy::channel::{Receiver, Sender};
use cushy::dialog::{FilePicker, FileType};
use cushy::figures::units::Px;
use cushy::localization::Localization;
use cushy::styles::components::IntrinsicPadding;
use cushy::value::Dynamic;
use cushy::widget::{IntoWidgetList, MakeWidget};
use cushy::window::{PendingWindow, WindowHandle};
use cushy::{App, Application};
use planner_gui::action::Action;
use planner_gui::context::Context;
use planner_gui::runtime::{Executor, MessageDispatcher, RunTime};
use planner_gui::task;
use planner_gui::task::Task;
use planner_gui::widgets::tab_bar::{TabAction, TabBar, TabKey, TabMessage};
use slotmap::SlotMap;
use thiserror::Error;
use tracing::{debug, error, info, trace};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};
use unic_langid::LanguageIdentifier;

use crate::app_tabs::home::{HomeTab, HomeTabAction};
use crate::app_tabs::new::{NewTab, NewTabAction, NewTabMessage};
use crate::app_tabs::project::{ProjectTab, ProjectTabAction, ProjectTabMessage};
use crate::app_tabs::{TabKind, TabKindAction, TabKindMessage};
use crate::config::Config;
use crate::project::{Project, ProjectKey, ProjectMessage};
use crate::toolbar::ToolbarMessage;

extern crate planner_gui;

mod app_core;
mod app_tabs;
mod config;
mod project;
mod toolbar;

#[derive(Clone, Debug, Default)]
enum AppMessage {
    #[default]
    None,
    TabMessage(TabMessage<TabKindMessage>),
    ToolBarMessage(ToolbarMessage),
    ChooseFile(WindowHandle),
    FileOpened(PathBuf),
    SaveActive,
}

enum AppError {
    None,
}

struct AppState {
    tab_bar: Dynamic<TabBar<TabKind, TabKindMessage, TabKindAction>>,
    config: Dynamic<Config>,
    context: Dynamic<Context>,

    projects: Dynamic<SlotMap<ProjectKey, Project>>,
    app_message_sender: Sender<AppMessage>,
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

    let languages: Vec<LanguageIdentifier> = vec![en_us_locale.clone(), es_es_locale.clone()];

    let language_identifier: Dynamic<LanguageIdentifier> = Dynamic::new(languages.first().unwrap().clone());

    app.cushy().localizations().add_default(
        Localization::for_language(
            "en-US",
            include_str!("../assets/translations/en-US/translations.ftl").to_owned(),
        )
        .unwrap(),
    );

    app.cushy().localizations().add(
        Localization::for_language(
            "es-ES",
            include_str!("../assets/translations/es-ES/translations.ftl").to_owned(),
        )
        .unwrap(),
    );

    let (app_message_sender, app_message_receiver) = cushy::channel::build().finish();

    let (mut sender, receiver) = futures::channel::mpsc::unbounded();

    let executor = Executor::new().expect("should be able to create an executor");
    executor.spawn(MessageDispatcher::dispatch(receiver, app_message_sender.clone()));
    let mut runtime = RunTime::new(executor, sender.clone());

    let pending = PendingWindow::default();
    let window = pending.handle();

    let config = Dynamic::new(config::load());
    let projects = Dynamic::new(SlotMap::default());

    let tab_message_sender: Sender<TabMessage<TabKindMessage>> = cushy::channel::build()
        .on_receive({
            let app_message_sender = app_message_sender.clone();
            move |tab_message| {
                debug!("tab_message: {:?}", tab_message);
                app_message_sender
                    .send(AppMessage::TabMessage(tab_message))
                    .map_err(|app_message| {
                        error!("unable to forward, app_message: {:?}", app_message);
                    })
                    .ok();
            }
        })
        .finish();

    let tab_bar = Dynamic::new(TabBar::new(tab_message_sender));

    let mut context = Context::default();
    context.provide(config.clone());
    context.provide(projects.clone());
    context.provide(window);

    let context = Dynamic::new(context);

    let app_state = AppState {
        tab_bar: tab_bar.clone(),
        context: context.clone(),
        config,
        projects,
        app_message_sender: app_message_sender.clone(),
    };

    let toolbar_message_sender: Sender<ToolbarMessage> = cushy::channel::build()
        .on_receive({
            let app_message_sender = app_message_sender.clone();
            move |toolbar_message| {
                debug!("forwarding toolbar message. message: {:?}", toolbar_message);
                app_message_sender
                    .send(AppMessage::ToolBarMessage(toolbar_message))
                    .map_err(|app_message| {
                        error!("unable to forward toolbar message, app_message: {:?}", app_message);
                    })
                    .ok();
            }
        })
        .finish();

    let toolbar = toolbar::make_toolbar(toolbar_message_sender, language_identifier.clone(), &languages);

    let ui_elements = [toolbar.make_widget(), app_state.tab_bar.lock().make_widget()];

    let dyn_app_state = Dynamic::new(app_state);

    app_message_receiver.on_receive({
        let dyn_app_state = dyn_app_state.clone();

        move |app_message| {
            trace!("message received. app_message: {:?}", app_message);
            let task = dyn_app_state.lock().update(app_message);

            if let Some(stream) = task::into_stream(task) {
                runtime.run(stream);
            }
        }
    });

    let ui = pending
        .with_root(
            ui_elements
                .into_rows()
                .width(Px::new(640)..)
                .height(Px::new(480)..)
                .fit_vertically()
                .fit_horizontally()
                .with(&IntrinsicPadding, Px::new(4))
                .localized_in(language_identifier)
                .make_widget(),
        )
        .on_close({
            let dyn_app_state = dyn_app_state.clone();
            let config = dyn_app_state.lock().config.clone();
            move || {
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

        if app_state
            .config
            .lock()
            .show_home_on_startup
        {
            add_home_tab(&context, &app_state.tab_bar);
        }
    }

    {
        // TODO here we can re-open previously-opened project files from the last session

        let messages: Vec<_> = vec![];

        for message in messages {
            let _result = sender.start_send(message);
        }
    }

    ui.open_centered(app)?;

    // FIXME control never returns here (at least on windows)

    Ok(())
}

fn add_home_tab(context: &Dynamic<Context>, tab_bar: &Dynamic<TabBar<TabKind, TabKindMessage, TabKindAction>>) {
    let mut tab_bar_guard = tab_bar.lock();

    let home_tab_result = tab_bar_guard.with_tabs(|mut iter| {
        iter.find_map(move |(_key, tab)| match tab {
            TabKind::Home(tab) => Some((_key, tab.clone())),
            _ => None,
        })
    });

    if let Some((key, _tab)) = home_tab_result {
        tab_bar_guard.activate(key);
    } else {
        tab_bar_guard.add_tab(context, TabKind::Home(HomeTab::default()));
    }
}

impl AppState {
    fn update(&mut self, message: AppMessage) -> Task<AppMessage> {
        //println!("AppState::update, message: {:?}", message);
        match message {
            AppMessage::None => Task::none(),
            AppMessage::TabMessage(message) => {
                let action = self
                    .tab_bar
                    .lock()
                    .update(&self.context, message);

                self.on_tab_action(action)
            }
            AppMessage::ToolBarMessage(message) => self.on_toolbar_message(message),
            AppMessage::ChooseFile(window) => {
                // TODO translate strings using the window's locale
                FilePicker::new()
                    .with_title("Open file")
                    .with_types([
                        // FIXME 'mpnp.json' doesn't work on OSX (no files are selectable), works fine on Windows.
                        //       consider using different file extensions for each type of json file, however this isn't ideal
                        //       since all the other tools woud need to be told about all the different types of file extensions
                        FileType::from(("Project files", ["json"])),
                    ])
                    .pick_file(&window, {
                        let app_message_sender = self.app_message_sender.clone();

                        move |path| {
                            if let Some(path) = path {
                                println!("path: {:?}", path);
                                app_message_sender
                                    .send(AppMessage::FileOpened(path))
                                    .expect("sent");
                            }
                        }
                    });

                Task::none()
            }
            AppMessage::FileOpened(path) => {
                match self.open_project(path) {
                    Ok(message) => Task::done(message),
                    Err(_error) => {
                        // TODO improve error handling by using '_error'
                        Task::none()
                    }
                }
            }
            AppMessage::SaveActive => {
                match self.save_active() {
                    Some(Ok(message)) => Task::done(message),
                    Some(Err(_error)) => {
                        // TODO improve error handling by using '_error'
                        Task::none()
                    }
                    None => {
                        // No active tab
                        Task::none()
                    }
                }
            }
        }
    }

    fn on_toolbar_message(&mut self, message: ToolbarMessage) -> Task<AppMessage> {
        match message {
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
                let window = self
                    .context
                    .lock()
                    .with_context::<WindowHandle, _, _>(|window_handle| window_handle.clone())
                    .unwrap();

                println!("open clicked");

                Task::done(AppMessage::ChooseFile(window))
            }
            ToolbarMessage::SaveClicked => {
                println!("Save clicked");

                Task::done(AppMessage::SaveActive)
            }
            ToolbarMessage::CloseAllClicked => {
                println!("close all clicked");
                let closed_tabs = self.tab_bar.lock().close_all();
                let tasks: Vec<_> = closed_tabs
                    .into_iter()
                    .map(|(key, kind)| self.on_tab_closed(key, kind))
                    .collect();

                Task::batch(tasks)
            }
        }
    }

    fn add_new_tab(&self) {
        let (new_tab_message_sender, new_tab_message_receiver) = cushy::channel::build().finish();

        let tab_key = self
            .tab_bar
            .lock()
            .add_tab(&self.context, TabKind::New(NewTab::new(new_tab_message_sender)));

        new_tab_message_receiver.on_receive({
            let app_message_sender = self.app_message_sender.clone();
            move |new_tab_message| {
                debug!("new_tab_message: {:?}", new_tab_message);
                app_message_sender
                    .send(AppMessage::TabMessage(TabMessage::TabKindMessage(
                        tab_key,
                        TabKindMessage::NewTabMessage(new_tab_message),
                    )))
                    .map_err(|message| error!("unable to forward new tab message. message: {:?}", message))
                    .ok();
            }
        });
    }

    fn on_tab_action(&mut self, action: Action<TabAction<TabKindAction, TabKind>>) -> Task<AppMessage> {
        let action = action.into_inner();

        match action {
            TabAction::TabSelected(tab_key) => {
                println!("tab selected, key: {:?}", tab_key);
                Task::none()
            }
            TabAction::TabClosed(tab_key, tab) => {
                self.on_tab_closed(tab_key, tab);

                Task::none()
            }
            TabAction::TabAction(tab_key, tab_action) => {
                println!("tab action. key: {:?}, action: {:?}", tab_key, tab_action);
                match tab_action {
                    TabKindAction::HomeTabAction(_tab_key, action) => match action {
                        HomeTabAction::None => Task::none(),
                    },
                    TabKindAction::NewTabAction(tab_key, action) => match action {
                        NewTabAction::None => Task::none(),
                        NewTabAction::CreateProject(name, directory) => self.create_project(tab_key, name, directory),
                        NewTabAction::Task(task) => task.map(move |message| {
                            AppMessage::TabMessage(TabMessage::TabKindMessage(
                                tab_key,
                                TabKindMessage::NewTabMessage(message),
                            ))
                        }),
                    },
                    TabKindAction::ProjectTabAction(tab_key, action) => match action {
                        ProjectTabAction::None => Task::none(),
                        ProjectTabAction::Task(task) => task.map(move |message| {
                            AppMessage::TabMessage(TabMessage::TabKindMessage(
                                tab_key,
                                TabKindMessage::ProjectTabMessage(ProjectTabMessage::ProjectMessage(message)),
                            ))
                        }),
                        ProjectTabAction::RenameTab(label) => {
                            Task::done(AppMessage::TabMessage(TabMessage::RenameTab(tab_key, label)))
                        }
                    },
                }
            }
            TabAction::None => Task::none(),
        }
    }

    fn on_tab_closed(&mut self, tab_key: TabKey, tab: TabKind) -> Task<AppMessage> {
        println!("tab closed, key: {:?}", tab_key);
        match tab {
            TabKind::Home(_tab) => (),
            TabKind::New(_tab) => (),
            TabKind::Project(tab) => {
                self.projects
                    .lock()
                    .remove(tab.project_key);
            }
        }
        Task::none()
    }

    fn create_project(&self, tab_key: TabKey, name: String, directory: PathBuf) -> Task<AppMessage> {
        let (project_tab_message_sender, project_tab_message_receiver) = cushy::channel::build().finish();

        self.create_project_tab_mapping(project_tab_message_receiver, tab_key);

        let path = build_project_file_path(&name, directory);
        let (project_message_sender, project_message_receiver) = cushy::channel::build().finish();

        self.create_project_mapping(project_message_receiver, project_tab_message_sender);

        let (project, message) = Project::new(name, path, project_message_sender);

        let project_key = self.projects.lock().insert(project);
        let project_tab = ProjectTab::new(project_key);

        self.tab_bar
            .lock()
            .replace(tab_key, &self.context, TabKind::Project(project_tab));

        let message_to_emit = AppMessage::TabMessage(TabMessage::TabKindMessage(
            tab_key,
            TabKindMessage::ProjectTabMessage(ProjectTabMessage::ProjectMessage(message)),
        ));

        Task::done(message_to_emit)
    }

    fn open_project(&self, path: PathBuf) -> Result<AppMessage, OpenProjectError> {
        println!("open_project. path: {:?}", path);

        let path = path::absolute(path).or_else(|cause| {
            Err(OpenProjectError::IoError {
                cause,
            })
        })?;

        let (project_tab_message_sender, project_tab_message_receiver) = cushy::channel::build().finish();

        let (project_message_sender, project_message_receiver) = cushy::channel::build().finish();

        self.create_project_mapping(project_message_receiver, project_tab_message_sender);

        let (project, message) = Project::from_path(path, project_message_sender);

        let project_key = self.projects.lock().insert(project);

        let project_tab = ProjectTab::new(project_key);

        let mut tab_bar_guard = self.tab_bar.lock();
        let tab_key = tab_bar_guard.add_tab(&self.context, TabKind::Project(project_tab));

        self.create_project_tab_mapping(project_tab_message_receiver, tab_key);

        let message_to_emit = AppMessage::TabMessage(TabMessage::TabKindMessage(
            tab_key,
            TabKindMessage::ProjectTabMessage(ProjectTabMessage::ProjectMessage(message)),
        ));

        Ok(message_to_emit)
    }

    fn create_project_tab_mapping(&self, project_tab_message_receiver: Receiver<ProjectTabMessage>, tab_key: TabKey) {
        project_tab_message_receiver.on_receive({
            let app_message_sender = self.app_message_sender.clone();
            move |project_tab_message| {
                debug!("project_tab_message: {:?}", project_tab_message);
                app_message_sender
                    .send(AppMessage::TabMessage(TabMessage::TabKindMessage(
                        tab_key,
                        TabKindMessage::ProjectTabMessage(project_tab_message),
                    )))
                    .map_err(|message| error!("unable to forward project tab message. message: {:?}", message))
                    .ok();
            }
        });
    }

    fn create_project_mapping(
        &self,
        project_message_receiver: Receiver<ProjectMessage>,
        project_tab_message_sender: Sender<ProjectTabMessage>,
    ) {
        project_message_receiver.on_receive({
            move |project_message| {
                debug!("project_message: {:?}", project_message);
                project_tab_message_sender
                    .send(ProjectTabMessage::ProjectMessage(project_message))
                    .map_err(|message| error!("unable to forward project message, message: {:?}", message))
                    .ok();
            }
        });
    }

    fn save_active(&self) -> Option<Result<AppMessage, AppError>> {
        let tab_bar = self.tab_bar.lock();
        let message = tab_bar.with_active(|tab_key, tab_kind| {
            match tab_kind {
                TabKind::Project(_project_tab) => Ok(AppMessage::TabMessage(TabMessage::TabKindMessage(
                    tab_key,
                    TabKindMessage::ProjectTabMessage(ProjectTabMessage::ProjectMessage(ProjectMessage::Save)),
                ))),
                // nothing to do for other tabs
                _ => Err(AppError::None),
            }
        });

        message
    }
}

#[derive(Error, Debug)]
enum OpenProjectError {
    #[error("IO error, cause: {cause}")]
    IoError { cause: std::io::Error },
}

pub fn build_project_file_path(name: &str, directory: PathBuf) -> PathBuf {
    let mut project_file_path: PathBuf = directory;
    project_file_path.push(format!("project-{}.mpnp.json", name));
    project_file_path
}
