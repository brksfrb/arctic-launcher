//! Game-window input for the Arctic overlay, without subclassing the window.
//!
//! The adapter used to route every window message through hudhook's
//! `GWLP_WNDPROC` subclass. That subclass is torn down and re-installed from
//! the render thread whenever the swap chain is rebuilt (each resize), and its
//! teardown writes back whatever procedure it saw at install time, dropping any
//! procedure installed after it; its message filter is also a render-thread
//! flag one frame behind the window thread. Instead:
//!
//! * a `WH_GETMESSAGE` hook on the game's window thread observes keyboard
//!   messages, and only while the Arctic menu is open replaces the ones it
//!   takes with `WM_NULL` (key-ups always reach the game, so nothing sticks);
//! * a `WH_MOUSE_LL` hook on its own thread reports real mouse buttons and the
//!   wheel (Bedrock can stop legacy mouse messages while it has the mouse for
//!   mouse-look, so clicks per second can't come from window messages), and
//!   only while the menu is open and the game is in front swallows them.
//!
//! With the menu closed neither hook changes or delays anything.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicIsize, AtomicU32, AtomicUsize, Ordering};

use hudhook::windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use hudhook::windows::Win32::Graphics::Gdi::ScreenToClient;
use hudhook::windows::Win32::UI::Input::KeyboardAndMouse::{MAPVK_VSC_TO_VK_EX, MapVirtualKeyW};
use hudhook::windows::Win32::UI::WindowsAndMessaging::*;

static GAME_WINDOW: AtomicIsize = AtomicIsize::new(0);
static MODULE: AtomicUsize = AtomicUsize::new(0);
static MESSAGE_HOOK: AtomicIsize = AtomicIsize::new(0);
static MESSAGE_THREAD: AtomicU32 = AtomicU32::new(0);
static MOUSE_THREAD: AtomicBool = AtomicBool::new(false);
static MENU_OPEN: AtomicBool = AtomicBool::new(false);
/// Win32 cursor fallback (no ClientInstance cursor binding): 1 release, 2 restore.
static FALLBACK_REQUEST: AtomicI32 = AtomicI32::new(0);
static FALLBACK_SHOWN: AtomicI32 = AtomicI32::new(0);
static FALLBACK_CLIP: Mutex<Option<RECT>> = Mutex::new(None);
static EVENTS: Mutex<Vec<i32>> = Mutex::new(Vec::new());
static NOTES: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Event kinds shared with `NativeBridge.input` (pairs of ints).
pub const KEY_DOWN: i32 = 0;
pub const CHAR: i32 = 1;
pub const MOUSE_DOWN: i32 = 2;
pub const MOUSE_UP: i32 = 3;
pub const WHEEL: i32 = 4;
pub const DRAG: i32 = 5;
/// A click that went to the game (menu closed): clicks per second.
pub const GAME_CLICK: i32 = 6;

fn push(kind: i32, value: i32) {
    if let Ok(mut q) = EVENTS.try_lock().or_else(|_| EVENTS.lock())
        && q.len() < 4096
    {
        q.extend_from_slice(&[kind, value]);
    }
}

fn note(message: String) {
    if let Ok(mut n) = NOTES.lock()
        && n.len() < 64
    {
        n.push(message);
    }
}

/// Queued input since the last frame, and any log lines.
pub fn drain(log: &dyn Fn(String)) -> Vec<i32> {
    if let Ok(mut n) = NOTES.lock() {
        for line in n.drain(..) {
            log(line);
        }
    }
    EVENTS
        .lock()
        .map(|mut q| std::mem::take(&mut *q))
        .unwrap_or_default()
}

pub fn push_drag() {
    push(DRAG, 0);
}

fn game_window() -> HWND {
    HWND(GAME_WINDOW.load(Ordering::Acquire) as _)
}

/// The game window is in front and the cursor is inside its client area.
fn over_game(point: POINT) -> bool {
    let hwnd = game_window();
    if hwnd.0.is_null() {
        return false;
    }
    // SAFETY: plain window queries on a window handle owned by this process.
    unsafe {
        if GetForegroundWindow() != hwnd {
            return false;
        }
        let mut local = point;
        let mut rect = RECT::default();
        ScreenToClient(hwnd, &mut local).as_bool()
            && GetClientRect(hwnd, &mut rect).is_ok()
            && local.x >= rect.left
            && local.x < rect.right
            && local.y >= rect.top
            && local.y < rect.bottom
    }
}

/// Cursor position in the game's client area, scaled to `size`.
pub fn cursor(size: [i32; 2]) -> Option<[i32; 2]> {
    let hwnd = game_window();
    if hwnd.0.is_null() {
        return None;
    }
    let mut point = POINT::default();
    let mut rect = RECT::default();
    // SAFETY: plain cursor/window queries.
    unsafe {
        GetCursorPos(&mut point).ok()?;
        if !ScreenToClient(hwnd, &mut point).as_bool() {
            return None;
        }
        GetClientRect(hwnd, &mut rect).ok()?;
    }
    let (w, h) = (
        (rect.right - rect.left).max(1),
        (rect.bottom - rect.top).max(1),
    );
    Some([
        (i64::from(point.x) * i64::from(size[0]) / i64::from(w)) as i32,
        (i64::from(point.y) * i64::from(size[1]) / i64::from(h)) as i32,
    ])
}

/// Fallback "playing" signal when no engine screen binding is available: the
/// game is in front with its cursor hidden (Bedrock hides it only while the
/// player controls the camera, i.e. in a world with no screen open).
pub fn game_has_mouse() -> bool {
    let hwnd = game_window();
    if hwnd.0.is_null() || MENU_OPEN.load(Ordering::Acquire) {
        return false;
    }
    let mut info = CURSORINFO {
        cbSize: std::mem::size_of::<CURSORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: plain cursor/window queries.
    unsafe {
        GetForegroundWindow() == hwnd
            && GetCursorInfo(&mut info).is_ok()
            && info.flags.0 & CURSOR_SHOWING.0 == 0
    }
}

/// Map WM_KEYDOWN's generic Shift/Ctrl/Alt to the left/right virtual key.
fn virtual_key(w: WPARAM, l: LPARAM) -> i32 {
    let vk = w.0 as i32;
    match vk {
        // SAFETY: a pure keyboard-layout lookup.
        0x10 => unsafe { MapVirtualKeyW(((l.0 >> 16) & 0xFF) as u32, MAPVK_VSC_TO_VK_EX) as i32 },
        0x11 => {
            if l.0 & (1 << 24) != 0 {
                0xA3
            } else {
                0xA2
            }
        }
        0x12 => {
            if l.0 & (1 << 24) != 0 {
                0xA5
            } else {
                0xA4
            }
        }
        _ => vk,
    }
}

unsafe extern "system" fn get_message(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 && wparam.0 == PM_REMOVE.0 as usize && lparam.0 != 0 {
        let _ = std::panic::catch_unwind(|| {
            // SAFETY: for HC_ACTION, lparam is the MSG being returned by
            // GetMessage/PeekMessage on this thread; it may be modified.
            let msg = unsafe { &mut *(lparam.0 as *mut MSG) };
            message(msg);
        });
    }
    // SAFETY: forwarding the hook chain unchanged.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

fn ours(hwnd: HWND) -> bool {
    let game = game_window();
    // SAFETY: plain window query.
    !game.0.is_null() && (hwnd == game || unsafe { IsChild(game, hwnd).as_bool() })
}

fn message(msg: &mut MSG) {
    fallback_cursor();
    if !ours(msg.hwnd) {
        return;
    }
    let open = MENU_OPEN.load(Ordering::Acquire);
    match msg.message {
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            let vk = virtual_key(msg.wParam, msg.lParam);
            let repeat = msg.lParam.0 & (1 << 30) != 0;
            // A held Right Shift must not repeatedly close/reopen the menu.
            if !(vk == 0xA1 && repeat) {
                push(KEY_DOWN, vk);
            }
            let alt = msg.lParam.0 & (1 << 29) != 0;
            let system = (alt && matches!(vk, 0x73 /* F4 */ | 0x09 /* Tab */ | 0x1B))
                || matches!(vk, 0x5B | 0x5C /* Windows keys */);
            if open && !system {
                // Produce WM_CHAR for our text fields, then hide the key from the game.
                // SAFETY: translating the message this thread is retrieving.
                unsafe {
                    let _ = TranslateMessage(msg);
                }
                msg.message = WM_NULL;
            }
        }
        WM_CHAR => {
            push(CHAR, msg.wParam.0 as i32);
            if open {
                msg.message = WM_NULL;
            }
        }
        WM_MOUSEMOVE | WM_LBUTTONDOWN | WM_LBUTTONUP | WM_LBUTTONDBLCLK | WM_RBUTTONDOWN
        | WM_RBUTTONUP | WM_RBUTTONDBLCLK | WM_MBUTTONDOWN | WM_MBUTTONUP | WM_MBUTTONDBLCLK
        | WM_MOUSEWHEEL | WM_MOUSEHWHEEL | WM_XBUTTONDOWN | WM_XBUTTONUP | WM_XBUTTONDBLCLK
            if open
                && !matches!(
                    msg.message,
                    WM_LBUTTONUP | WM_RBUTTONUP | WM_MBUTTONUP | WM_XBUTTONUP
                ) =>
        {
            // Buttons come from the low-level hook; the game must not see
            // hover/clicks under the menu. Button-ups still reach it.
            msg.message = WM_NULL;
        }
        _ => {}
    }
}

unsafe extern "system" fn low_level_mouse(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && lparam.0 != 0 {
        // SAFETY: for HC_ACTION, lparam points to the event's MSLLHOOKSTRUCT.
        let info = unsafe { *(lparam.0 as *const MSLLHOOKSTRUCT) };
        let message = wparam.0 as u32;
        let xbutton = || {
            if (info.mouseData >> 16) & 0xFFFF == 1 {
                3
            } else {
                4
            }
        };
        let event = match message {
            WM_LBUTTONDOWN => Some((MOUSE_DOWN, 0)),
            WM_RBUTTONDOWN => Some((MOUSE_DOWN, 1)),
            WM_MBUTTONDOWN => Some((MOUSE_DOWN, 2)),
            WM_XBUTTONDOWN => Some((MOUSE_DOWN, xbutton())),
            WM_LBUTTONUP => Some((MOUSE_UP, 0)),
            WM_RBUTTONUP => Some((MOUSE_UP, 1)),
            WM_MBUTTONUP => Some((MOUSE_UP, 2)),
            WM_XBUTTONUP => Some((MOUSE_UP, xbutton())),
            WM_MOUSEWHEEL => Some((WHEEL, ((info.mouseData >> 16) as u16 as i16) as i32)),
            _ => None,
        };
        if let Some((kind, value)) = event
            && over_game(info.pt)
        {
            if MENU_OPEN.load(Ordering::Acquire) {
                push(kind, value);
                // The menu owns the mouse: the game must not click or scroll under it.
                return LRESULT(1);
            } else if kind == MOUSE_DOWN {
                push(GAME_CLICK, value);
            }
        }
    }
    // SAFETY: forwarding the hook chain unchanged.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// Install both hooks for the swap chain's window (idempotent per window).
pub fn install(hwnd: HWND, module: usize, log: &dyn Fn(String)) {
    if hwnd.0.is_null() {
        return;
    }
    MODULE.store(module, Ordering::Release);
    let previous = GAME_WINDOW.swap(hwnd.0 as isize, Ordering::AcqRel);
    // SAFETY: plain window query.
    let thread = unsafe { GetWindowThreadProcessId(hwnd, None) };
    if previous != hwnd.0 as isize || MESSAGE_THREAD.load(Ordering::Acquire) != thread {
        let old = MESSAGE_HOOK.swap(0, Ordering::AcqRel);
        if old != 0 {
            // SAFETY: our own hook handle from a previous window.
            unsafe {
                let _ = UnhookWindowsHookEx(HHOOK(old as _));
            }
        }
        // SAFETY: a thread hook on this process's game window thread with a
        // procedure in this module.
        match unsafe {
            SetWindowsHookExW(
                WH_GETMESSAGE,
                Some(get_message),
                Some(HINSTANCE(module as _)),
                thread,
            )
        } {
            Ok(hook) => {
                MESSAGE_HOOK.store(hook.0 as isize, Ordering::Release);
                MESSAGE_THREAD.store(thread, Ordering::Release);
                log(format!(
                    "input: keyboard message hook installed on window thread {thread} (no window subclass)"
                ));
            }
            Err(e) => log(format!("input: keyboard message hook FAILED: {e}")),
        }
    }
    if !MOUSE_THREAD.swap(true, Ordering::AcqRel) {
        std::thread::Builder::new()
            .name("arctic-mouse-hook".into())
            .spawn(move || {
                // SAFETY: a low-level mouse hook serviced by this thread's message loop.
                let hook = unsafe {
                    SetWindowsHookExW(
                        WH_MOUSE_LL,
                        Some(low_level_mouse),
                        Some(HINSTANCE(MODULE.load(Ordering::Acquire) as _)),
                        0,
                    )
                };
                match hook {
                    Ok(_) => note(
                        "input: low-level mouse hook running (real clicks, menu-only blocking)"
                            .into(),
                    ),
                    Err(e) => {
                        note(format!("input: low-level mouse hook FAILED: {e}"));
                        return;
                    }
                }
                let mut msg = MSG::default();
                // SAFETY: this thread's own message loop.
                while unsafe { GetMessageW(&mut msg, None, 0, 0) }.0 > 0 {
                    unsafe {
                        let _ = TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
            })
            .ok();
    }
}

/// The Arctic menu opened or closed.
pub fn menu_changed(open: bool, engine_cursor: bool) {
    MENU_OPEN.store(open, Ordering::Release);
    if !engine_cursor {
        FALLBACK_REQUEST.store(if open { 1 } else { 2 }, Ordering::Release);
    }
}

/// Without the engine cursor binding: unclip and show the cursor on the game's
/// window thread (cursor visibility is per thread), and restore it on close.
fn fallback_cursor() {
    match FALLBACK_REQUEST.swap(0, Ordering::AcqRel) {
        1 => {
            let mut clip = RECT::default();
            // SAFETY: plain cursor calls on the game's window thread.
            unsafe {
                if GetClipCursor(&mut clip).is_ok()
                    && let Ok(mut saved) = FALLBACK_CLIP.lock()
                {
                    *saved = Some(clip);
                }
                let _ = ClipCursor(None);
                let mut shown = 0;
                while ShowCursor(true) < 0 && shown < 64 {
                    shown += 1;
                }
                FALLBACK_SHOWN.store(shown + 1, Ordering::Release);
                SetCursor(LoadCursorW(None, IDC_ARROW).ok());
            }
        }
        2 => {
            // SAFETY: undoing exactly what the release did, on the same thread.
            unsafe {
                for _ in 0..FALLBACK_SHOWN.swap(0, Ordering::AcqRel) {
                    ShowCursor(false);
                }
                if let Ok(mut saved) = FALLBACK_CLIP.lock()
                    && let Some(clip) = saved.take()
                {
                    let _ = ClipCursor(Some(&clip));
                }
            }
        }
        _ => {
            if MENU_OPEN.load(Ordering::Acquire) && FALLBACK_SHOWN.load(Ordering::Acquire) > 0 {
                // SAFETY: keep the game from re-clipping under the open menu.
                unsafe {
                    let _ = ClipCursor(None);
                }
            }
        }
    }
}
