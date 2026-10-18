//! The tray without the launcher window (Windows).
//!
//! A hidden launcher still holds the window, the GPU context, fonts and every cache, which is far
//! more than a tray icon needs. So after a while in the tray with nothing running, the launcher
//! starts a small second process of itself (`--tray-host`) that only keeps the icon and the
//! single-instance listener, and ends. Open, Play, or a second start brings the real launcher back
//! (`--replace`, which takes over from this process) and ends this one too.

use std::sync::mpsc::Receiver;
#[cfg(windows)]
use std::time::Duration;

/// Starts the process that only keeps the tray icon.
pub const FLAG: &str = "--tray-host";

/// Hidden in the tray this long with nothing running: hand the tray over.
#[cfg(windows)]
pub const HAND_OVER_AFTER: Duration = Duration::from_secs(60);

/// Time for the new process to show its icon before the launcher's own goes.
#[cfg(windows)]
const ICON_SETTLE: Duration = Duration::from_millis(900);

/// Start the tray-only process; true if it started (the caller then closes the launcher).
#[cfg(windows)]
pub fn start() -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    // `--replace`: it takes over from the launcher that is still running for a moment.
    let started = std::process::Command::new(exe)
        .args([FLAG, crate::single_instance::REPLACE_FLAG])
        .spawn()
        .map_err(|e| log::warn!("tray host: {e}"))
        .is_ok();
    if started {
        std::thread::sleep(ICON_SETTLE);
    }
    started
}

/// Should this start be the tray-only process (asked for, or a hidden start with the tray on)?
pub fn wanted(args: &[String], tray_setting: bool) -> bool {
    cfg!(windows)
        && (args.iter().any(|a| a == FLAG)
            || (tray_setting && args.iter().any(|a| a == "--hidden")))
}

/// Keep the tray icon until something asks for the launcher. Does not return.
#[cfg(windows)]
pub fn run(dirs: &arctic_core::storage::DataDirs, forwarded: Receiver<Vec<String>>) -> ! {
    use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{Icon, MouseButton, TrayIconBuilder, TrayIconEvent};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, GetMessageW, MSG, TranslateMessage,
    };

    const ICON_SIZE: u32 = 32;
    log::info!("tray host started");
    // Another start was handed to us: the launcher it asked for opens, and we end.
    std::thread::spawn(move || {
        if let Ok(args) = forwarded.recv() {
            open_launcher(&args);
        }
    });
    let open = MenuItem::new("Open Arctic Launcher", true, None);
    let play = MenuItem::new("Play", true, None);
    let quit = MenuItem::new("Quit", true, None);
    let menu = Menu::new();
    let built = menu.append_items(&[&open, &play, &PredefinedMenuItem::separator(), &quit]);
    let icon = Icon::from_rgba(
        crate::icon_raster::app_icon_rgba(ICON_SIZE),
        ICON_SIZE,
        ICON_SIZE,
    );
    let tray = match (built, icon) {
        (Ok(()), Ok(icon)) => TrayIconBuilder::new()
            .with_tooltip(arctic_core::APP_NAME)
            .with_icon(icon)
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .build(),
        _ => {
            log::warn!("tray host: couldn't build the menu or icon");
            open_launcher(&[]);
        }
    };
    let Ok(_tray) = tray else {
        log::warn!("tray host: no tray icon; opening the launcher");
        open_launcher(&[]);
    };
    let (open_id, play_id, quit_id) = (open.id().clone(), play.id().clone(), quit.id().clone());
    let last_version = arctic_core::settings::Settings::load(dirs)
        .ok()
        .and_then(|s| s.last_version);
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        if event.id == open_id {
            open_launcher(&[]);
        } else if event.id == play_id {
            match &last_version {
                Some(version) => open_launcher(&["--launch".to_owned(), version.clone()]),
                None => open_launcher(&["--tab".to_owned(), "play".to_owned()]),
            }
        } else if event.id == quit_id {
            std::process::exit(0);
        }
    }));
    TrayIconEvent::set_event_handler(Some(|event: TrayIconEvent| {
        let open = matches!(
            event,
            TrayIconEvent::DoubleClick { .. }
                | TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: tray_icon::MouseButtonState::Up,
                    ..
                }
        );
        if open {
            open_launcher(&[]);
        }
    }));
    // The icon lives on this thread's message loop.
    // SAFETY: a zeroed MSG is the documented way to start; the calls only read and write it.
    unsafe {
        let mut message: MSG = core::mem::zeroed();
        while GetMessageW(&mut message, core::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    std::process::exit(0);
}

/// Start the launcher (taking over from this process) with these arguments, and end this one.
#[cfg(windows)]
fn open_launcher(args: &[String]) -> ! {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe)
            .arg(crate::single_instance::REPLACE_FLAG)
            .args(args)
            .spawn()
            .map_err(|e| log::warn!("tray host: couldn't open the launcher: {e}"));
    }
    std::process::exit(0);
}

#[cfg(not(windows))]
pub fn run(_dirs: &arctic_core::storage::DataDirs, _forwarded: Receiver<Vec<String>>) -> ! {
    unreachable!("the tray host is Windows-only; `wanted` is false elsewhere")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asked_for_by_flag_or_hidden_start_with_tray() {
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(wanted(&args(&[FLAG]), false), cfg!(windows));
        assert_eq!(wanted(&args(&["--hidden"]), true), cfg!(windows));
        assert!(!wanted(&args(&["--hidden"]), false));
        assert!(!wanted(&args(&[]), true));
    }
}
