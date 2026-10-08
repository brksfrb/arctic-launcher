//! Updates that install themselves (Settings → "Install updates automatically", on by default).
//!
//! At start the app's own check finds an update and installs it right away (see
//! `ArcticApp::on_update_checked`). A launcher left open is checked again every few hours by a
//! thread of its own: the window may be hidden in the tray, where the app runs no frames, so this
//! thread downloads and swaps in the new version itself. Once installed, a launcher waiting hidden
//! with no game running restarts into it (staying hidden); otherwise it switches over the next
//! time it's opened, or with the banner's "Restart now".

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use arctic_core::storage::DataDirs;
use arctic_core::update::{self, UpdateChannel};

use crate::tasks::{Event, Tasks};

/// How often a launcher left open looks for a new version.
const RECHECK: Duration = Duration::from_secs(6 * 60 * 60);

/// Mirrors of the app's state for the update thread (the app refreshes them every frame).
static AUTO_INSTALL: AtomicBool = AtomicBool::new(true);
static BETA: AtomicBool = AtomicBool::new(false);
/// A game (or a launch getting ready) is running: no restarting under it.
static BUSY: AtomicBool = AtomicBool::new(false);
/// A newer version is already swapped in; only a restart is left.
static INSTALLED: AtomicBool = AtomicBool::new(false);

/// Every frame: what the update thread needs to know.
pub fn sync(auto_install: bool, channel: UpdateChannel, busy: bool) {
    AUTO_INSTALL.store(auto_install, Ordering::Relaxed);
    BETA.store(channel == UpdateChannel::Beta, Ordering::Relaxed);
    BUSY.store(busy, Ordering::Relaxed);
}

/// The new version is in place (by the app or the thread).
pub fn mark_installed() {
    INSTALLED.store(true, Ordering::Relaxed);
}

/// Restart into the new version now, if nobody would notice: hidden in the tray, nothing running.
/// Returns whether the restart was started (the old launcher then closes).
pub fn restart_if_unnoticed() -> bool {
    if !crate::window::hidden() || BUSY.load(Ordering::Relaxed) {
        return false;
    }
    match update::restart_with(&["--hidden"]) {
        Ok(()) => {
            log::info!("restarting into the installed update (hidden)");
            crate::tray::set_quitting();
            crate::window::close();
            true
        }
        Err(e) => {
            log::warn!("couldn't restart into the update: {e}");
            false
        }
    }
}

/// Start the periodic check.
pub fn spawn(dirs: DataDirs, tasks: Tasks) {
    let spawned = std::thread::Builder::new()
        .name("auto-update".into())
        .spawn(move || {
            loop {
                std::thread::sleep(RECHECK);
                if AUTO_INSTALL.load(Ordering::Relaxed) && !INSTALLED.load(Ordering::Relaxed) {
                    check_and_install(&dirs, &tasks);
                }
                if INSTALLED.load(Ordering::Relaxed) && restart_if_unnoticed() {
                    return;
                }
            }
        });
    if let Err(e) = spawned {
        log::warn!("auto-update thread: {e}");
    }
}

fn check_and_install(dirs: &DataDirs, tasks: &Tasks) {
    let channel = if BETA.load(Ordering::Relaxed) {
        UpdateChannel::Beta
    } else {
        UpdateChannel::Stable
    };
    let info = match update::check(channel) {
        Ok(Some(info)) => info,
        Ok(None) => return,
        Err(e) => {
            log::warn!("update check failed: {e}");
            return;
        }
    };
    log::info!("installing update {} in the background", info.version);
    match update::download_and_apply(dirs, &info, &|_| {}) {
        Ok(()) => {
            mark_installed();
            tasks.send(Event::UpdateApplied(info.required));
        }
        Err(e) => log::warn!("background update to {} failed: {e}", info.version),
    }
}
