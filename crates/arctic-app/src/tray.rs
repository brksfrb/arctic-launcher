//! System tray (Windows): closing the window hides it, the tray icon
//! brings it back (Open / Play) or quits. Elsewhere closing quits.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};

use eframe::egui;

/// What the tray asked the app to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))] // no tray outside Windows yet
pub enum Action {
    Play,
}

static QUITTING: AtomicBool = AtomicBool::new(false);

/// Quit was chosen (tray or restart): let the window really close.
pub fn quitting() -> bool {
    QUITTING.load(Ordering::Relaxed)
}

/// Let the next close request really quit instead of hiding.
pub fn set_quitting() {
    QUITTING.store(true, Ordering::Relaxed);
}

pub struct Tray {
    #[cfg(windows)]
    _icon: tray_icon::TrayIcon,
    pub actions: Receiver<Action>,
}

impl Tray {
    /// Create the tray icon (on the UI thread, after the window exists).
    pub fn new(ctx: &egui::Context) -> Option<Self> {
        let (tx, rx) = mpsc::channel();
        imp::create(ctx, tx).map(|_icon| Tray {
            #[cfg(windows)]
            _icon,
            actions: rx,
        })
    }
}

#[cfg(windows)]
mod imp {
    use super::*;
    use crate::window;
    use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{Icon, MouseButton, TrayIconBuilder, TrayIconEvent};

    const ICON_SIZE: u32 = 32;
    /// If the window doesn't close on its own after Quit (it may be hidden
    /// and not processing frames), exit anyway after this long.
    const QUIT_GRACE: std::time::Duration = std::time::Duration::from_secs(3);

    pub fn create(ctx: &egui::Context, tx: Sender<Action>) -> Option<tray_icon::TrayIcon> {
        let open = MenuItem::new("Open Arctic Launcher", true, None);
        let play = MenuItem::new("Play", true, None);
        let quit = MenuItem::new("Quit", true, None);
        let menu = Menu::new();
        menu.append_items(&[&open, &play, &PredefinedMenuItem::separator(), &quit])
            .ok()?;
        let rgba = crate::icon_raster::app_icon_rgba(ICON_SIZE);
        let icon = Icon::from_rgba(rgba, ICON_SIZE, ICON_SIZE).ok()?;
        let tray = TrayIconBuilder::new()
            .with_tooltip(arctic_core::APP_NAME)
            .with_icon(icon)
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .build()
            .map_err(|e| log::warn!("tray icon: {e}"))
            .ok()?;

        let (open_id, play_id, quit_id) = (open.id().clone(), play.id().clone(), quit.id().clone());
        let menu_ctx = ctx.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            if event.id == open_id {
                window::show();
            } else if event.id == play_id {
                window::show();
                let _ = tx.send(Action::Play);
            } else if event.id == quit_id {
                QUITTING.store(true, Ordering::Relaxed);
                window::close();
                std::thread::spawn(|| {
                    std::thread::sleep(QUIT_GRACE);
                    std::process::exit(0);
                });
            }
            menu_ctx.request_repaint();
        }));
        let click_ctx = ctx.clone();
        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
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
                window::show();
                click_ctx.request_repaint();
            }
        }));
        Some(tray)
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;

    pub fn create(_ctx: &egui::Context, _tx: Sender<Action>) -> Option<()> {
        None
    }
}
