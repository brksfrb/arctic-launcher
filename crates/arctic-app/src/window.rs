//! Show, hide and close the main window directly through the OS, which
//! keeps working while the window is hidden (egui doesn't run frames then).

use std::sync::atomic::{AtomicIsize, Ordering};

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

#[cfg(windows)]
mod imp {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        IsIconic, PostMessageW, SW_HIDE, SW_RESTORE, SW_SHOW, SetForegroundWindow, ShowWindow,
        WM_CLOSE,
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
        if let Some(hwnd) = hwnd() {
            // SAFETY: as above.
            unsafe {
                ShowWindow(hwnd, SW_HIDE);
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
        if let Some(ctx) = CTX.get() {
            ctx.send_viewport_cmd(ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(ViewportCommand::Focus);
            ctx.request_repaint();
        }
    }

    pub fn hide() {
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
