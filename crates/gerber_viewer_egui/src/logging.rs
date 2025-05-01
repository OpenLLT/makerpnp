use std::fmt::{Display, Formatter};

pub enum AppLogItem {
    Info(String),
    Warning(String),
    Error(String),
}

impl AppLogItem {
    pub fn message(&self) -> &str {
        match self {
            AppLogItem::Info(message) => message,
            AppLogItem::Warning(message) => message,
            AppLogItem::Error(message) => message,
        }
    }

    pub fn level(&self) -> &'static str {
        match self {
            AppLogItem::Info(_) => "info",
            AppLogItem::Warning(_) => "warning",
            AppLogItem::Error(_) => "error",
        }
    }
}

impl Display for AppLogItem {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AppLogItem::Info(message) => f.write_fmt(format_args!("Info: {}", message)),
            AppLogItem::Warning(message) => f.write_fmt(format_args!("Warning: {}", message)),
            AppLogItem::Error(message) => f.write_fmt(format_args!("Error: {}", message)),
        }
    }
}