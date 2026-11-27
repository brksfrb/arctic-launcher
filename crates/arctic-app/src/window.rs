//! Show, hide and close the main window directly through the OS, which
//! keeps working while the window is hidden (egui doesn't run frames then).

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};

use raw_window_handle::HasWindowHandle;

/// The main window's native handle (Windows `HWND`), 0 until known.
static HANDLE: AtomicIsize = AtomicIsize::new(0);

/// Remember the window handle (call once the window exists).
pub fn remember(window: &impl HasWindowHandle) {
    if let Ok(handle) = window.window_handle() {
        if let raw_window_handle::RawWindowHandle::Win32(win32) = handle.as_raw() {
            HANDLE.store(win32.hwnd.get(), Ordering::Relaxed);
        }
        imp::remembered();
    }
}

pub fn is_known() -> bool {
    HANDLE.load(Ordering::Relaxed) != 0 || imp::known()
}

/// Hidden in the tray (or started hidden), as far as this app has hidden or shown it.
static HIDDEN: AtomicBool = AtomicBool::new(false);

pub fn hidden() -> bool {
    HIDDEN.load(Ordering::Relaxed)
}

/// The window starts hidden (see `--hidden`).
pub fn started_hidden() {
    HIDDEN.store(true, Ordering::Relaxed);
}

/// Frames left in which a hidden start keeps telling eframe to stay hidden (it shows the window
/// once its first frame is drawn).
static HOLD_FRAMES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(30);
const HOLD_STEP: std::time::Duration = std::time::Duration::from_millis(50);

/// Every frame: a window that should be hidden stays hidden.
pub fn keep_hidden(ctx: &eframe::egui::Context) {
    if !HIDDEN.load(Ordering::Relaxed) {
        return;
    }
    ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Visible(false));
    #[cfg(windows)]
    imp::hide_now();
    if HOLD_FRAMES.load(Ordering::Relaxed) > 0 {
        HOLD_FRAMES.fetch_sub(1, Ordering::Relaxed);
        ctx.request_repaint_after(HOLD_STEP);
    }
}

/// Let show, hide and close reach the window. Call once, as soon as there is a context: the Linux
/// tray does it too, but a PC with no system tray still needs the window to be reachable (the
/// watchdog below, and a second start handed over by [`crate::single_instance`]).
#[cfg(not(windows))]
pub fn remember_context(ctx: &eframe::egui::Context) {
    imp::set_context(ctx);
}

/// Windows reaches its window through the handle [`remember`] keeps, so there is nothing to note.
#[cfg(windows)]
pub fn remember_context(_ctx: &eframe::egui::Context) {}

/// Frames painted so far (eframe shows the window after the first one).
static PAINTS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Called from every painted frame.
pub fn painted() {
    PAINTS.fetch_add(1, Ordering::Relaxed);
}

/// The watchdog's first look: by now a window that is coming up has come up.
const SHOW_GRACE: std::time::Duration = std::time::Duration::from_millis(600);
/// How often it looks after that.
const SHOW_EVERY: std::time::Duration = std::time::Duration::from_millis(250);
/// It keeps frames coming this long after the start, even once one has been painted.
const SHOW_SETTLE: std::time::Duration = std::time::Duration::from_secs(3);
/// With nothing painted by now, something else is wrong; the log says so and it stops.
const SHOW_FOR: std::time::Duration = std::time::Duration::from_secs(20);

/// Make sure the window really comes up.
///
/// eframe creates it hidden and shows it only once the first frame has been painted (so no white
/// flash). Where frames stop coming the window never appears and the launcher looks like it never
/// started, which is what happens on a Wayland session: a compositor gives no frame callbacks to a
/// surface it is not showing, so winit asks for no redraw, and nothing else asks either — no input
/// reaches a window that isn't on screen, and the animated backdrop only paces frames when it is
/// switched on. egui is told to paint on demand, so one frame is where it stops. (On X11 the same
/// knot ties itself differently: egui paints nothing at all while it believes the window is
/// minimized or occluded, which an unmapped window reads as.)
///
/// So this watches from its own thread — the only place a repaint can be asked for while no egui
/// pass is running, and the way eframe documents for waking it — and for the first seconds keeps
/// frames coming and, while nothing has been painted, asks for the window outright.
///
/// A start hidden in the tray (`--hidden`) is left alone.
pub fn watch_until_painted(ctx: &eframe::egui::Context) {
    use eframe::egui::ViewportCommand;
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        std::thread::sleep(SHOW_GRACE);
        let started = std::time::Instant::now();
        let mut asked = 0u32;
        loop {
            if HIDDEN.load(Ordering::Relaxed) {
                return;
            }
            let painted = PAINTS.load(Ordering::Relaxed) > 0;
            let waited = started.elapsed();
            if painted {
                if waited > SHOW_SETTLE {
                    if asked > 0 {
                        log::info!("the window came up after {asked} nudge(s)");
                    }
                    return;
                }
                // A painted frame is not the same as a window on screen: keep asking for frames
                // for a moment, so the compositor has something to show.
                ctx.request_repaint();
            } else {
                if waited > SHOW_FOR {
                    log::error!(
                        "no frame has been painted in {waited:?}: the window may not be on screen"
                    );
                    return;
                }
                asked += 1;
                if asked == 1 {
                    log::warn!("nothing painted yet: asking for the window again");
                }
                ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(ViewportCommand::Visible(true));
                ctx.request_repaint();
            }
            std::thread::sleep(SHOW_EVERY);
        }
    });
}

#[cfg(windows)]
mod imp {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        IsIconic, IsWindowVisible, PostMessageW, SW_HIDE, SW_RESTORE, SW_SHOW, SetForegroundWindow,
        ShowWindow, WM_CLOSE,
    };

    use super::*;

    fn hwnd() -> Option<*mut core::ffi::c_void> {
        let raw = HANDLE.load(Ordering::Relaxed);
        (raw != 0).then_some(raw as *mut core::ffi::c_void)
    }

    /// The handle is the whole answer here.
    pub fn known() -> bool {
        false
    }

    pub fn remembered() {}

    pub fn show() {
        HIDDEN.store(false, Ordering::Relaxed);
        if let Some(hwnd) = hwnd() {
            // SAFETY: `hwnd` is our own live top-level window.
            unsafe {
                ShowWindow(hwnd, SW_SHOW);
                if IsIconic(hwnd) != 0 {
                    ShowWindow(hwnd, SW_RESTORE);
                }
                SetForegroundWindow(hwnd);
            }
        }
    }

    pub fn hide() {
        HIDDEN.store(true, Ordering::Relaxed);
        if let Some(hwnd) = hwnd() {
            // SAFETY: as above.
            unsafe {
                ShowWindow(hwnd, SW_HIDE);
            }
        }
    }

    /// Hide the window if it's showing (it should be hidden).
    pub fn hide_now() {
        if let Some(hwnd) = hwnd() {
            // SAFETY: as above.
            unsafe {
                if IsWindowVisible(hwnd) != 0 {
                    ShowWindow(hwnd, SW_HIDE);
                }
            }
        }
    }

    pub fn close() {
        if let Some(hwnd) = hwnd() {
            // SAFETY: as above; posting WM_CLOSE asks the app to close normally.
            unsafe {
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
            }
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicBool, Ordering};

    use eframe::egui::{self, ViewportCommand};

    use super::HIDDEN;

    /// There's no native handle to keep here: the window is driven through
    /// egui (which keeps working while hidden, as the tray's events wake it).
    static KNOWN: AtomicBool = AtomicBool::new(false);
    static CTX: OnceLock<egui::Context> = OnceLock::new();

    pub fn known() -> bool {
        KNOWN.load(Ordering::Relaxed)
    }

    pub fn remembered() {
        KNOWN.store(true, Ordering::Relaxed);
    }

    /// Let show, hide and close reach the window (the tray calls this).
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    pub fn set_context(ctx: &egui::Context) {
        let _ = CTX.set(ctx.clone());
    }

    pub fn show() {
        HIDDEN.store(false, Ordering::Relaxed);
        if let Some(ctx) = CTX.get() {
            ctx.send_viewport_cmd(ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(ViewportCommand::Focus);
            ctx.request_repaint();
        }
    }

    pub fn hide() {
        HIDDEN.store(true, Ordering::Relaxed);
        if let Some(ctx) = CTX.get() {
            // Wayland can't hide a window from its own app: minimize instead.
            let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
            ctx.send_viewport_cmd(if wayland {
                ViewportCommand::Minimized(true)
            } else {
                ViewportCommand::Visible(false)
            });
            ctx.request_repaint();
        }
    }

    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    pub fn close() {
        if let Some(ctx) = CTX.get() {
            ctx.send_viewport_cmd(ViewportCommand::Close);
            ctx.request_repaint();
        }
    }
}

/// The tray closes the window from outside.
#[cfg(windows)]
pub use imp::close;
#[cfg(not(windows))]
#[cfg_attr(not(target_os = "linux"), allow(unused_imports))]
pub use imp::{close, set_context};
pub use imp::{hide, show};
