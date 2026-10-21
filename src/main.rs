#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod http_client;
mod models;

use app::RustmanApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1150.0, 760.0])
            .with_min_inner_size([800.0, 550.0])
            .with_title("Rustman - Cliente HTTP estilo Postman"),
        ..Default::default()
    };

    eframe::run_native(
        "Rustman",
        native_options,
        Box::new(|cc| Ok(Box::new(RustmanApp::new(cc)))),
    )
}
