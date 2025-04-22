#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
use i18n::I18nConfig;
use planner_gui_egui::ui_app::UiApp;
/// Run as follows:
/// `run --package planner_gui_egui --bin planner_gui_egui`
///
/// To enable logging, set the environment variable appropriately, for example:
/// `RUST_LOG=debug,eframe=warn,egui_glow=warn,egui=warn`
use tracing::info;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt};

fn main() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    info!("Started");

    i18n::init(I18nConfig {
        languages: vec![String::from("es-ES"), String::from("en-US")],
        default: "en-US".to_string(),
        fallback: "en-US".to_string(),
    });

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_icon(std::sync::Arc::new(egui::IconData {
                rgba: image::load_from_memory(include_bytes!("../../../assets/logos/makerpnp_icon_1_384x384.png"))
                    .unwrap()
                    .to_rgba8()
                    .to_vec(),
                width: 384,
                height: 384,
            }))
            .with_inner_size([600.0, 440.0])
            .with_min_inner_size([300.0, 220.0]),
        ..Default::default()
    };

    if let Err(e) = eframe::run_native(
        "MakerPnP - Planner",
        options,
        Box::new(|cc| Ok(Box::new(UiApp::new(cc)))),
    ) {
        eprintln!("Failed to run eframe: {:?}", e);
    }
}
