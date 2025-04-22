use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io;
use std::io::BufReader;
use std::path::PathBuf;

use eframe::{CreationContext, Frame, NativeOptions, egui, run_native};
use egui::Context;
use egui::scroll_area::ScrollBarVisibility;
use egui::style::ScrollStyle;
use gerber_parser::error::GerberParserErrorWithContext;
use gerber_parser::gerber_doc::GerberDoc;
use gerber_parser::parser::parse_gerber;
use gerber_types::Command;
use log::{error, info};
use rfd::FileDialog;
use thiserror::Error;

fn main() -> eframe::Result<()> {
    env_logger::init(); // Log to stderr (optional).
    let native_options = NativeOptions::default();
    run_native(
        "Gerber Viewer",
        native_options,
        Box::new(|cc| Ok(Box::new(GerberViewer::new(cc)))),
    )
}
struct GerberViewer {
    gerber_doc: Option<GerberDoc>,

    log: Vec<AppLogItem>,
}

struct GerberState {
    path: PathBuf,
    gerber_commands: Vec<Command>,
}

#[derive(Error, Debug)]
enum AppError {
    #[error("No file selected")]
    NoFileSelected,
    #[error("IO Error. cause: {0:?}")]
    IoError(io::Error),
}

enum AppLogItem {
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

impl GerberViewer {
    /// FIXME: Blocks main thread when file selector is open
    fn open_gerber_file(&mut self) {
        self.open_gerber_file_inner()
            .inspect_err(|e| {
                let message = format!("Error opening file: {:?}", e);
                error!("{}", message);
                self.log
                    .push(AppLogItem::Error(message.to_string()));
            })
            .ok();
    }

    fn open_gerber_file_inner(&mut self) -> Result<(), AppError> {
        let path = FileDialog::new()
            .add_filter("Gerber Files", &["gbr", "gbl", "gbo", "gbs", "gko", "gko", "gto"])
            .pick_file()
            .ok_or(AppError::NoFileSelected)?;

        self.parse_gerber_file(path)?;

        Ok(())
    }
    pub fn parse_gerber_file(&mut self, path: PathBuf) -> Result<(), AppError> {
        let file = File::open(path.clone()).map_err(AppError::IoError)?;
        let reader = BufReader::new(file);

        let gerber_doc: GerberDoc = parse_gerber(reader);

        let log = gerber_doc
            .commands
            .iter()
            .map(|c| match c {
                Ok(command) => AppLogItem::Info(format!("{:?}", command)),
                Err(error) => AppLogItem::Error(format!("{:?}", error)),
            })
            .collect::<Vec<_>>();
        self.log.extend(log);

        self.gerber_doc = Some(gerber_doc);

        let message = "Gerber file parsed successfully";
        info!("{}", message);
        self.log
            .push(AppLogItem::Info(message.to_string()));

        Ok(())
    }

    pub fn clear_log(&mut self) {
        self.log.clear();
    }
}

impl GerberViewer {
    pub fn new(_cc: &CreationContext) -> Self {
        _cc.egui_ctx
            .style_mut(|style| style.spacing.scroll = ScrollStyle::solid());
        Self {
            gerber_doc: None,
            log: Vec::new(),
        }
    }
}

impl eframe::App for GerberViewer {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        egui::TopBottomPanel::bottom("bottom_panel")
            .resizable(true)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Clear").clicked() {
                        self.clear_log();
                    }
                });

                egui::ScrollArea::vertical()
                    .scroll_bar_visibility(ScrollBarVisibility::AlwaysVisible)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for error in self.log.iter() {
                            ui.label(format!("{}", error));
                        }
                    })
            });

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            if ui.button("Open Gerber File").clicked() {
                self.open_gerber_file();
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {});
    }
}
