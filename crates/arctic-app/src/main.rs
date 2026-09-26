// Release builds are GUI-only on Windows (no console window).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod art;
mod devshot;
mod discord;
mod fonts;
mod gallery_tasks;
mod icon_raster;
mod logbook;
mod motion;
mod session;
mod single_instance;
mod skin_tasks;
mod startup;
mod tasks;
mod theme;
mod theme_fade;
mod titlebar;
mod toasts;
mod tray;
mod ui;
mod widgets;
mod window;
mod world_tasks;

use std::sync::Arc;

use arctic_core::profiles::ProfileStore;
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
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Already running (maybe hidden in the tray): hand over and exit.
    let forwarded = match single_instance::claim(dirs.root(), &args) {
        single_instance::Claim::Forwarded => return Ok(()),
        single_instance::Claim::Primary(rx) => rx,
    };
    logbook::init(dirs.launcher_logs().join("launcher.log"));
    let startup = startup::StartupOptions::parse(args);
    if let Err(e) = dirs.ensure() {
        log::error!("could not create data folders: {e}");
    }
    let mut profiles = match ProfileStore::load_or_init(&dirs) {
        Ok(p) => p,
        Err(e) => {
            log::error!("profiles unavailable: {e}");
            std::process::exit(1);
        }
    };
    if let Some(query) = &startup.profile {
        match profiles.find(query).map(|p| p.id.clone()) {
            Some(id) => {
                profiles.set_active(&id);
            }
            None => log::warn!("no profile named '{query}'"),
        }
    }

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
        Box::new(move |cc| {
            Ok(Box::new(app::ArcticApp::new(
                cc, dirs, profiles, startup, forwarded,
            )))
        }),
    )
}
