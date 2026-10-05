// Release builds are GUI-only on Windows (no console window).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod art;
mod bridge_host;
mod devshot;
mod discord;
mod fonts;
mod friend_tasks;
mod gallery_tasks;
mod icon_raster;
mod logbook;
mod migrate_tasks;
mod motion;
mod pack_tasks;
mod runs;
mod server_tasks;
mod session;
mod share_tasks;
mod shot_tasks;
mod single_instance;
mod skin_tasks;
mod standby;
mod startup;
mod tasks;
mod theme;
mod theme_fade;
mod titlebar;
mod toasts;
mod tray;
mod ui;
mod video_encoder;
mod voice;
mod voice_svc;
mod widgets;
mod window;
mod world_tasks;

use std::sync::Arc;

use arctic_core::profiles::ProfileStore;
use arctic_core::storage::DataDirs;
use eframe::egui;

const MIN_WINDOW: [f32; 2] = [960.0, 620.0];
const DEFAULT_WINDOW: [f32; 2] = [1180.0, 720.0];
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
    // The game exporting a replay video (see video_encoder): no window.
    if args.first().map(String::as_str) == Some(video_encoder::SWITCH) {
        std::process::exit(video_encoder::run(&args[1..]));
    }
    // Packaging (the macOS app icon): `--write-icon <out.png> <size>`.
    if args.first().map(String::as_str) == Some("--write-icon") {
        std::process::exit(write_icon(&args[1..]));
    }
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

/// Save the app icon as a PNG of `size`×`size` pixels (for packaging).
fn write_icon(args: &[String]) -> i32 {
    let (Some(out), Some(size)) = (
        args.first(),
        args.get(1).and_then(|s| s.parse::<u32>().ok()),
    ) else {
        eprintln!("usage: --write-icon <out.png> <size>");
        return 2;
    };
    let size = size.clamp(16, 2048);
    let Some(image) = image::RgbaImage::from_raw(size, size, icon_raster::app_icon_rgba(size))
    else {
        return 1;
    };
    match image.save(out) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("{out}: {e}");
            1
        }
    }
}
