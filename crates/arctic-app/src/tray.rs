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

/// If the window doesn't close on its own after quitting (hidden and not processing frames, or
/// stuck shutting something down), exit anyway after this long.
const QUIT_GRACE: std::time::Duration = std::time::Duration::from_secs(3);

/// Let the next close request really quit instead of hiding, and make sure the process ends
/// even if closing hangs (a restart into an update must not leave a frozen old window behind).
pub fn set_quitting() {
    if !QUITTING.swap(true, Ordering::Relaxed) {
        std::thread::spawn(|| {
            std::thread::sleep(QUIT_GRACE);
            std::process::exit(0);
        });
    }
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

/// Linux: the icon being made, on its own thread.
///
/// Creating it talks to D-Bus, and ksni's blocking `spawn` does not always come back: on a
/// session whose StatusNotifier host never answers it parks whoever called it, for good. That
/// caller used to be the UI thread, inside the first `logic` pass — before a single frame had
/// been painted. eframe keeps the window hidden until the first frame (to avoid a white flash)
/// and un-hides it in `post_rendering`, so the launcher sat there with no window at all, on
/// Wayland and on X11 alike, until the tray was switched off.
#[cfg(not(windows))]
static MAKING: std::sync::Mutex<Option<Making>> = std::sync::Mutex::new(None);

/// How long to wait before saying out loud that the tray is being slow.
#[cfg(not(windows))]
const TRAY_SLOW: std::time::Duration = std::time::Duration::from_secs(5);

#[cfg(not(windows))]
struct Making {
    /// The finished icon, or `None` when this session has no tray to put it in.
    icon: Receiver<Option<imp::Handle>>,
    /// Made up front, so menu clicks have somewhere to go the moment the icon is live.
    actions: Receiver<Action>,
    since: std::time::Instant,
    said_slow: bool,
}

impl Tray {
    /// Ask for the tray icon.
    ///
    /// Windows builds it here and hands it back at once. Linux cannot: see [`MAKING`]. There the
    /// first call starts a thread and returns `None`, and a later call returns the icon once it
    /// is live — so the window is never waiting on a tray.
    #[cfg(windows)]
    pub fn new(ctx: &egui::Context) -> Option<Self> {
        let (tx, rx) = mpsc::channel();
        imp::create(ctx, tx).map(|_icon| Tray { _icon, actions: rx })
    }

    #[cfg(not(windows))]
    pub fn new(ctx: &egui::Context) -> Option<Self> {
        use std::sync::mpsc::TryRecvError;

        let mut slot = MAKING.lock().ok()?;
        if slot.is_none() {
            if TRIED.swap(true, Ordering::Relaxed) {
                // Already been through this: there is no tray here, or there is one already.
                return None;
            }
            let (action_tx, actions) = mpsc::channel();
            let (icon_tx, icon) = mpsc::channel();
            let thread_ctx = ctx.clone();
            std::thread::Builder::new()
                .name("tray".to_owned())
                .spawn(move || {
                    // Whatever this does — answer, fail, or never return — the UI thread is
                    // already drawing.
                    let _ = icon_tx.send(imp::create(&thread_ctx, action_tx));
                })
                .map_err(|e| log::warn!("no thread for the tray icon: {e}"))
                .ok()?;
            *slot = Some(Making {
                icon,
                actions,
                since: std::time::Instant::now(),
                said_slow: false,
            });
            ctx.request_repaint_after(TRAY_POLL);
            return None;
        }

        let ready = slot.as_mut().and_then(|making| match making.icon.try_recv() {
            Ok(icon) => Some(icon),
            // The thread is gone without an answer; treat it as no tray.
            Err(TryRecvError::Disconnected) => Some(None),
            Err(TryRecvError::Empty) => {
                if !making.said_slow && making.since.elapsed() > TRAY_SLOW {
                    making.said_slow = true;
                    log::warn!(
                        "the system tray is taking its time ({:?} so far); the launcher carries on without it",
                        making.since.elapsed()
                    );
                }
                None
            }
        });

        match ready {
            // Still working: come back for it.
            None => {
                ctx.request_repaint_after(TRAY_POLL);
                None
            }
            // No tray in this session. `TRIED` keeps it from being asked again.
            Some(None) => {
                *slot = None;
                None
            }
            Some(Some(_icon)) => {
                let making = slot.take()?;
                Some(Tray {
                    _icon,
                    actions: making.actions,
                })
            }
        }
    }
}

/// How often to look for the finished icon while it is being made.
#[cfg(not(windows))]
const TRAY_POLL: std::time::Duration = std::time::Duration::from_millis(250);

#[cfg(windows)]
mod imp {
    use super::*;
    use crate::window;
    use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{Icon, MouseButton, TrayIconBuilder, TrayIconEvent};

    pub type Handle = tray_icon::TrayIcon;

    const ICON_SIZE: u32 = 32;

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
                set_quitting();
                window::close();
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
                        set_quitting();
                        window::close();
                        tray.ctx.request_repaint();
                    }),
                    ..Default::default()
                }
                .into(),
            ]
        }
    }

    /// The icon as the tray protocol wants it: ARGB, bytes in that order.
    fn argb(rgba: &[u8]) -> Vec<u8> {
        rgba.as_chunks::<4>()
            .0
            .iter()
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
        .disable_dbus_name(arctic_core::update::managed_by_flatpak())
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
