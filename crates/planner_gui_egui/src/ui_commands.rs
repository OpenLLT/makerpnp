use std::path::PathBuf;

use egui::{Context, ThemePreference};
use egui_mobius::types::Value;
use tracing::{debug, trace};

use crate::config::Config;
use crate::project::{ProjectAction, ProjectUiCommand};
use crate::tabs::TabKey;
use crate::task::Task;
use crate::toolbar::{ToolbarAction, ToolbarUiCommand};
use crate::ui_app::app_tabs::home::HomeTabAction;
use crate::ui_app::app_tabs::new_project::NewProjectTabAction;
use crate::ui_app::app_tabs::project::{ProjectTabAction, ProjectTabUiCommand};
use crate::ui_app::app_tabs::{
    AppTabs, TabAction, TabKind, TabKindAction, TabKindContext, TabKindUiCommand, TabUiCommand,
};
use crate::ui_app::{AppState, build_toolbar_context};
use crate::ui_component::UiComponent;

#[derive(Debug, Clone)]
pub enum UiCommand {
    #[allow(dead_code)]
    None,
    ToolbarCommand(ToolbarUiCommand),
    OpenFile(PathBuf),
    TabCommand {
        tab_key: TabKey,
        command: TabUiCommand,
    },
    LangageChanged(String),
    ThemeChanged(ThemePreference),
}

// TODO perhaps the return type of this method be `Task<Result<UiCommand, UiAppError>>`
pub fn handle_command(
    command: UiCommand,
    app_state: Value<AppState>,
    app_tabs: Value<AppTabs>,
    config: Value<Config>,
    ui_context: Context,
) -> Task<UiCommand> {
    trace!("Handling command: {:?}", command);

    match command {
        UiCommand::None => Task::none(),
        UiCommand::LangageChanged(language) => {
            egui_i18n::set_language(&language);
            config
                .lock()
                .unwrap()
                .language_identifier = language;
            Task::none()
        }
        UiCommand::ThemeChanged(theme) => {
            ui_context.set_theme(theme);
            Task::none()
        }
        UiCommand::ToolbarCommand(command) => {
            let mut context = build_toolbar_context(&app_tabs);

            let toolbar_action = app_state
                .lock()
                .unwrap()
                .toolbar
                .update(command, &mut context);

            let task = handle_toolbar_action(toolbar_action, &app_state, &app_tabs);
            task
        }
        UiCommand::OpenFile(picked_file) => {
            let mut app_state = app_state.lock().unwrap();
            app_state.open_file(picked_file, app_tabs);
            Task::none()
        }
        UiCommand::TabCommand {
            tab_key,
            command,
        } => {
            let mut context = TabKindContext {
                config: config.clone(),
                projects: app_state
                    .lock()
                    .unwrap()
                    .projects
                    .clone(),
            };

            let action = {
                let mut app_tabs = app_tabs.lock().unwrap();
                app_tabs.update((tab_key, command), &mut context)
            };
            debug!("handling tab command action: {:?}", action);
            match action {
                None => Task::none(),
                Some(TabAction::None) => Task::none(),
                Some(TabAction::TabKindAction {
                    action,
                }) => match action {
                    TabKindAction::None => Task::none(),
                    TabKindAction::HomeTabAction {
                        action,
                    } => match action {
                        HomeTabAction::None => Task::none(),
                    },
                    TabKindAction::NewProjectTabAction {
                        action,
                    } => match action {
                        NewProjectTabAction::Submit(args) => {
                            let mut app_state = app_state.lock().unwrap();
                            app_state.create_project(tab_key, args, app_tabs);
                            Task::none()
                        }
                    },
                    TabKindAction::ProjectTabAction {
                        action,
                    } => match action {
                        ProjectTabAction::ProjectTask(key, task) => task.map(move |action| {
                            debug!("handling project action: {:?}", action);
                            match action {
                                ProjectAction::UiCommand(command) => UiCommand::TabCommand {
                                    tab_key,
                                    command: TabUiCommand::TabKindCommand(TabKindUiCommand::ProjectTabCommand {
                                        command: ProjectTabUiCommand::ProjectCommand {
                                            key,
                                            command,
                                        },
                                    }),
                                },
                                ProjectAction::Task(_, _) => {
                                    // unsupported here, no corresponding TabCommands
                                    // should have already been handled by the project
                                    // HINT: when batching tasks, make sure the batch doesn't include ProjectAction::Task
                                    panic!("unsupported")
                                }

                                ProjectAction::SetModifiedState(_) => {
                                    // unsupported here, no corresponding TabCommands
                                    // should have already been handled by the project
                                    // HINT: when batching tasks, make sure the batch doesn't include ProjectAction::SetModifiedState
                                    panic!("unsupported")
                                }
                                ProjectAction::RequestRepaint => {
                                    panic!("unsupported")
                                }
                            }
                        }),
                        ProjectTabAction::SetModifiedState(modified_state) => {
                            let app_tabs = app_tabs.lock().unwrap();
                            app_tabs.with_tab_mut(&tab_key, |tab| match tab {
                                TabKind::Project(project_tab, _) => {
                                    project_tab.modified = modified_state;
                                }
                                _ => unreachable!(),
                            });
                            Task::none()
                        }
                        ProjectTabAction::RequestRepaint => {
                            ui_context.request_repaint();
                            Task::none()
                        }
                    },
                },
            }
        }
    }
}

fn handle_toolbar_action(
    toolbar_action: Option<ToolbarAction>,
    app_state: &Value<AppState>,
    ui_state: &Value<AppTabs>,
) -> Task<UiCommand> {
    let Some(toolbar_action) = toolbar_action else {
        return Task::none();
    };

    match toolbar_action {
        ToolbarAction::ShowHomeTab => {
            let mut ui_state = ui_state.lock().unwrap();
            ui_state.show_home_tab();
            Task::none()
        }
        ToolbarAction::AddNewProjectTab => {
            let mut ui_state = ui_state.lock().unwrap();
            ui_state.add_new_project_tab();
            Task::none()
        }
        ToolbarAction::CloseAllTabs => {
            let mut ui_state = ui_state.lock().unwrap();
            ui_state.close_all_tabs();
            Task::none()
        }
        ToolbarAction::PickFile => {
            let mut app_state = app_state.lock().unwrap();
            app_state.pick_file();
            Task::none()
        }
        ToolbarAction::SaveTab(tab_key) => {
            let ui_state = ui_state.lock().unwrap();
            ui_state.with_tab_mut(&tab_key, |tab_kind| match tab_kind {
                TabKind::Project(project_tab, _) => {
                    let command = UiCommand::TabCommand {
                        tab_key,
                        command: TabUiCommand::TabKindCommand(TabKindUiCommand::ProjectTabCommand {
                            command: ProjectTabUiCommand::ProjectCommand {
                                key: project_tab.project_key,
                                command: ProjectUiCommand::Save,
                            },
                        }),
                    };
                    debug!("save command: {:?}", command);
                    Task::done(command)
                }
                _ => Task::none(),
            })
        }
    }
}
