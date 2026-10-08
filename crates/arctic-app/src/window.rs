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
