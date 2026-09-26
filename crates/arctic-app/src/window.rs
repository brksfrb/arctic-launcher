//! Show, hide and close the main window directly through the OS, which
//! keeps working while the window is hidden (egui doesn't run frames then).

use std::sync::atomic::{AtomicIsize, Ordering};

use raw_window_handle::HasWindowHandle;

/// The main window's native handle (Windows `HWND`), 0 until known.
static HANDLE: AtomicIsize = AtomicIsize::new(0);

/// Remember the window handle (call once the window exists).
pub fn remember(window: &impl HasWindowHandle) {
    if let Ok(handle) = window.window_handle()
        && let raw_window_handle::RawWindowHandle::Win32(win32) = handle.as_raw()
    {
        HANDLE.store(win32.hwnd.get(), Ordering::Relaxed);
    }
}

pub fn is_known() -> bool {
    HANDLE.load(Ordering::Relaxed) != 0
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
    pub fn show() {}
    pub fn hide() {}
}

/// Only the Windows tray closes the window from outside.
#[cfg(windows)]
pub use imp::close;
pub use imp::{hide, show};
