//! System tray (Windows and Linux): closing the window hides it, the tray
//! icon brings it back (Open / Play) or quits. Elsewhere closing quits.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};

use eframe::egui;

/// What the tray asked the app to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(any(windows, target_os = "linux")), allow(dead_code))] // no tray on macOS yet
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
    /// Keeps the icon alive (it goes when this is dropped).
    _icon: imp::Handle,
    pub actions: Receiver<Action>,
}

/// Linux: tried once. With no tray host running (GNOME without its
/// extension, say) trying every frame would only waste time; the window
/// then just closes as it always did.
#[cfg(not(windows))]
static TRIED: AtomicBool = AtomicBool::new(false);

impl Tray {
    /// Create the tray icon (on the UI thread, after the window exists).
    pub fn new(ctx: &egui::Context) -> Option<Self> {
        #[cfg(not(windows))]
        if TRIED.swap(true, Ordering::Relaxed) {
            return None;
        }
        let (tx, rx) = mpsc::channel();
        imp::create(ctx, tx).map(|_icon| Tray { _icon, actions: rx })
    }
}

#[cfg(windows)]
mod imp {
    use super::*;
    use crate::window;
    use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{Icon, MouseButton, TrayIconBuilder, TrayIconEvent};

    pub type Handle = tray_icon::TrayIcon;

    const ICON_SIZE: u32 = 32;
    /// If the window doesn't close on its own after Quit (it may be hidden
    /// and not processing frames), exit anyway after this long.
    const QUIT_GRACE: std::time::Duration = std::time::Duration::from_secs(3);

    pub fn create(ctx: &egui::Context, tx: Sender<Action>) -> Option<Handle> {
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

#[cfg(target_os = "linux")]
mod imp {
    use super::*;
    use crate::window;
    use ksni::blocking::TrayMethods;

    pub type Handle = ksni::blocking::Handle<ArcticTray>;

    const ICON_SIZE: u32 = 32;
    /// If the window doesn't close on its own after Quit, exit anyway after this long.
    const QUIT_GRACE: std::time::Duration = std::time::Duration::from_secs(3);

    pub struct ArcticTray {
        tx: Sender<Action>,
        ctx: egui::Context,
        icon: ksni::Icon,
    }

    impl ArcticTray {
        fn open(&self) {
            window::show();
            self.ctx.request_repaint();
        }
    }

    impl ksni::Tray for ArcticTray {
        fn id(&self) -> String {
            "arctic-launcher".into()
        }

        fn title(&self) -> String {
            arctic_core::APP_NAME.into()
        }

        fn icon_pixmap(&self) -> Vec<ksni::Icon> {
            vec![self.icon.clone()]
        }

        /// A click on the icon.
        fn activate(&mut self, _x: i32, _y: i32) {
            self.open();
        }

        fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
            use ksni::menu::StandardItem;
            vec![
                StandardItem {
                    label: "Open Arctic Launcher".into(),
                    activate: Box::new(|tray: &mut Self| tray.open()),
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: "Play".into(),
                    activate: Box::new(|tray: &mut Self| {
                        tray.open();
                        let _ = tray.tx.send(Action::Play);
                    }),
                    ..Default::default()
                }
                .into(),
                ksni::MenuItem::Separator,
                StandardItem {
                    label: "Quit".into(),
                    activate: Box::new(|tray: &mut Self| {
                        QUITTING.store(true, Ordering::Relaxed);
                        window::close();
                        tray.ctx.request_repaint();
                        std::thread::spawn(|| {
                            std::thread::sleep(QUIT_GRACE);
                            std::process::exit(0);
                        });
                    }),
                    ..Default::default()
                }
                .into(),
            ]
        }
    }

    /// The icon as the tray protocol wants it: ARGB, bytes in that order.
    fn argb(rgba: &[u8]) -> Vec<u8> {
        rgba.chunks_exact(4)
            .flat_map(|p| [p[3], p[0], p[1], p[2]])
            .collect()
    }

    pub fn create(ctx: &egui::Context, tx: Sender<Action>) -> Option<Handle> {
        window::set_context(ctx);
        let icon = ksni::Icon {
            width: ICON_SIZE as i32,
            height: ICON_SIZE as i32,
            data: argb(&crate::icon_raster::app_icon_rgba(ICON_SIZE)),
        };
        ArcticTray {
            tx,
            ctx: ctx.clone(),
            icon,
        }
        .spawn()
        .map_err(|e| log::info!("no system tray here: {e}"))
        .ok()
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn argb_puts_alpha_first() {
            assert_eq!(
                super::argb(&[1, 2, 3, 4, 5, 6, 7, 8]),
                [4, 1, 2, 3, 8, 5, 6, 7]
            );
        }
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
mod imp {
    use super::*;

    pub type Handle = ();

    pub fn create(_ctx: &egui::Context, _tx: Sender<Action>) -> Option<Handle> {
        None
    }
}
