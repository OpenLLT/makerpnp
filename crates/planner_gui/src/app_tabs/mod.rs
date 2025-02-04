//! The tabs for the application.

use cushy::value::Dynamic;
use cushy::widget::WidgetInstance;
use planner_gui::action::Action;
use planner_gui::context::Context;
use planner_gui::widgets::tab_bar::{Tab, TabKey};

use crate::app_tabs::home::{HomeTab, HomeTabAction, HomeTabMessage};
use crate::app_tabs::new::{NewTab, NewTabAction, NewTabMessage};
use crate::app_tabs::project::{ProjectTab, ProjectTabAction, ProjectTabMessage};

pub mod home;
pub mod new;
pub mod project;

#[derive(Clone)]
pub enum TabKind {
    Home(HomeTab),
    New(NewTab),
    Project(ProjectTab),
}

#[derive(Clone, Debug)]
pub enum TabKindMessage {
    HomeTabMessage(HomeTabMessage),
    NewTabMessage(NewTabMessage),
    ProjectTabMessage(ProjectTabMessage),
}

#[derive(Debug)]
pub enum TabKindAction {
    HomeTabAction(TabKey, HomeTabAction),
    NewTabAction(TabKey, NewTabAction),
    ProjectTabAction(TabKey, ProjectTabAction),
}

impl Tab<TabKindMessage, TabKindAction> for TabKind {
    fn label(&self, context: &Dynamic<Context>) -> String {
        match self {
            TabKind::Home(tab) => tab.label(context),
            TabKind::New(tab) => tab.label(context),
            TabKind::Project(tab) => tab.label(context),
        }
    }

    fn modified(&self, context: &Dynamic<Context>) -> bool {
        match self {
            TabKind::Home(tab) => tab.modified(context),
            TabKind::New(tab) => tab.modified(context),
            TabKind::Project(tab) => tab.modified(context),
        }
    }

    fn make_content(&self, context: &Dynamic<Context>, tab_key: TabKey) -> WidgetInstance {
        match self {
            TabKind::Home(tab) => tab.make_content(context, tab_key),
            TabKind::New(tab) => tab.make_content(context, tab_key),
            TabKind::Project(tab) => tab.make_content(context, tab_key),
        }
    }

    fn update(
        &mut self,
        context: &Dynamic<Context>,
        tab_key: TabKey,
        message: TabKindMessage,
    ) -> Action<TabKindAction> {
        match (self, message) {
            (TabKind::Home(tab), TabKindMessage::HomeTabMessage(message)) => tab
                .update(context, tab_key, message)
                .map(|action| TabKindAction::HomeTabAction(tab_key, action)),
            (TabKind::New(tab), TabKindMessage::NewTabMessage(message)) => tab
                .update(context, tab_key, message)
                .map(|action| TabKindAction::NewTabAction(tab_key, action)),
            (TabKind::Project(tab), TabKindMessage::ProjectTabMessage(message)) => tab
                .update(context, tab_key, message)
                .map(|action| TabKindAction::ProjectTabAction(tab_key, action)),
            (_, _) => {
                unreachable!()
            }
        }
    }
}
