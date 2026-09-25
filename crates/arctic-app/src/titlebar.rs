//! Tint the native Windows title bar to match the theme (Windows 11 DWM
//! caption colors; Windows 10 falls back to just the dark-mode flag).
//! Keeping the native bar preserves snapping, resizing and shadows.

use raw_window_handle::HasWindowHandle;

use crate::theme::Palette;

#[cfg(windows)]
pub fn apply(window: &impl HasWindowHandle, p: &Palette) {
    use raw_window_handle::RawWindowHandle;
    use windows_sys::Win32::Graphics::Dwm::{
        DWMWA_BORDER_COLOR, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE,
        DwmSetWindowAttribute,
    };

    let Ok(handle) = window.window_handle() else {
        return;
    };
    let RawWindowHandle::Win32(win32) = handle.as_raw() else {
        return;
    };
    let hwnd = win32.hwnd.get() as windows_sys::Win32::Foundation::HWND;
    let colorref = |c: eframe::egui::Color32| {
        u32::from(c.r()) | u32::from(c.g()) << 8 | u32::from(c.b()) << 16
    };
    let dark = i32::from(p.dark);
    let set = |attribute: i32, value: *const core::ffi::c_void, size: usize| {
        // SAFETY: `hwnd` is the live top-level window owned by eframe, and
        // `value` points at a correctly sized, initialized attribute value.
        let hr = unsafe { DwmSetWindowAttribute(hwnd, attribute as u32, value, size as u32) };
        if hr != 0 {
            log::debug!("DwmSetWindowAttribute({attribute}) failed: {hr:#x}");
        }
    };
    set(
        DWMWA_USE_IMMERSIVE_DARK_MODE,
        (&dark as *const i32).cast(),
        size_of::<i32>(),
    );
    for (attribute, color) in [
        (DWMWA_CAPTION_COLOR, p.titlebar),
        (DWMWA_BORDER_COLOR, p.titlebar),
        (DWMWA_TEXT_COLOR, p.text),
    ] {
        let value = colorref(color);
        set(attribute, (&value as *const u32).cast(), size_of::<u32>());
    }
}

#[cfg(not(windows))]
pub fn apply(_window: &impl HasWindowHandle, _p: &Palette) {}
