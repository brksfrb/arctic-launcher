// Release builds are GUI-only on Windows (no console window).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod art;
mod icon_raster;
mod logbook;
mod motion;
mod startup;
mod tasks;
mod theme;
mod titlebar;
mod toasts;
mod ui;
mod widgets;

use std::sync::Arc;

use arctic_core::storage::DataDirs;
use eframe::egui;

const MIN_WINDOW: [f32; 2] = [860.0, 560.0];
const DEFAULT_WINDOW: [f32; 2] = [1060.0, 680.0];
const ICON_SIZE: u32 = 64;

fn main() -> eframe::Result {
    let dirs = match DataDirs::resolve() {
        Ok(dirs) => dirs,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    logbook::init(dirs.logs().join("launcher.log"));
    let startup = startup::StartupOptions::parse(std::env::args().skip(1));
    log::info!(
        "{} {} — data dir {}",
        arctic_core::APP_NAME,
        arctic_core::APP_VERSION,
        dirs.root().display()
    );

    let icon = egui::IconData {
        rgba: icon_raster::app_icon_rgba(ICON_SIZE),
        width: ICON_SIZE,
        height: ICON_SIZE,
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(arctic_core::APP_NAME)
            .with_icon(Arc::new(icon))
            .with_inner_size(DEFAULT_WINDOW)
            .with_min_inner_size(MIN_WINDOW),
        ..Default::default()
    };
    eframe::run_native(
        arctic_core::APP_NAME,
        options,
        Box::new(move |cc| Ok(Box::new(app::ArcticApp::new(cc, dirs, startup)))),
    )
}
