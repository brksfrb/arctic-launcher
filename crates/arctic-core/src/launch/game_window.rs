//! "Start the game maximized": Minecraft has no option for it. The Arctic Client maximizes its
//! own window (every system, see its WindowMaximizeMixin); the launcher also asks the system to
//! maximize the game's window, which covers games without the Arctic Client and LWJGL 2
//! (1.8.9-1.12.2), which can't do it itself: on Windows directly, on Linux through the X11
//! window manager (Xorg, and XWayland under Wayland). On macOS the client does it alone.

use std::time::Duration;

/// How long after the start the game's window is looked for (a first start after an update is slow).
#[cfg_attr(not(any(windows, target_os = "linux")), allow(dead_code))]
const WATCH_FOR: Duration = Duration::from_secs(180);
#[cfg_attr(not(any(windows, target_os = "linux")), allow(dead_code))]
const POLL: Duration = Duration::from_millis(150);

/// The JVM flag that tells the Arctic Client to maximize its window.
pub const CLIENT_FLAG: &str = "-Darctic.maximized=true";

/// Maximize the window game `pid` opens, once (a player who restores it keeps it that way).
pub fn maximize_when_open(pid: u32) {
    #[cfg(any(windows, target_os = "linux"))]
    std::thread::spawn(move || imp::watch(pid));
    #[cfg(not(any(windows, target_os = "linux")))]
    let _ = pid;
}

#[cfg(windows)]
mod imp {
    use std::time::Instant;

    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetClassNameW, GetWindowThreadProcessId, IsWindowVisible, SW_MAXIMIZE,
        ShowWindow,
    };

    use super::{POLL, WATCH_FOR};

    /// Window classes of LWJGL 3 (GLFW) and LWJGL 2.
    const GAME_CLASSES: [&str; 2] = ["GLFW30", "LWJGL"];

    struct Search {
        pid: u32,
        found: Option<HWND>,
    }

    pub fn watch(pid: u32) {
        let start = Instant::now();
        while start.elapsed() < WATCH_FOR {
            if let Some(hwnd) = find(pid) {
                // SAFETY: a live top-level window of the game, found just now.
                unsafe {
                    ShowWindow(hwnd, SW_MAXIMIZE);
                }
                log::info!("maximized the game window");
                return;
            }
            std::thread::sleep(POLL);
        }
    }

    fn find(pid: u32) -> Option<HWND> {
        let mut search = Search { pid, found: None };
        // SAFETY: the callback only runs during this call, while `search` is alive.
        unsafe {
            EnumWindows(Some(visit), &mut search as *mut Search as LPARAM);
        }
        search.found
    }

    unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> i32 {
        // SAFETY: `lparam` is the `Search` that `find` passed in.
        let search = unsafe { &mut *(lparam as *mut Search) };
        let mut owner = 0u32;
        // SAFETY: `hwnd` comes from EnumWindows; the out pointers are valid locals.
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut owner);
            if owner != search.pid || IsWindowVisible(hwnd) == 0 {
                return 1;
            }
            let mut class = [0u16; 32];
            let len = GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32);
            let class = String::from_utf16_lossy(&class[..len.max(0) as usize]);
            if GAME_CLASSES.contains(&class.as_str()) {
                search.found = Some(hwnd);
                return 0;
            }
        }
        1
    }
}

#[cfg(target_os = "linux")]
mod imp {
    use std::time::Instant;

    use x11rb::connection::Connection;
    use x11rb::protocol::res::{ClientIdMask, ClientIdSpec, ConnectionExt as _};
    use x11rb::protocol::xproto::{
        Atom, ClientMessageEvent, ConnectionExt as _, EventMask, MapState, Window,
    };

    use super::{POLL, WATCH_FOR};

    /// `_NET_WM_STATE_ADD` in a `_NET_WM_STATE` request.
    const STATE_ADD: u32 = 1;
    /// The game's window sits at most this deep under the root (inside the window manager's frame).
    const MAX_DEPTH: u8 = 3;

    pub fn watch(pid: u32) {
        // Without an X server (a Wayland-only session) only the Arctic Client can do it.
        if std::env::var_os("DISPLAY").is_none() {
            return;
        }
        let Ok((conn, screen)) = x11rb::connect(None) else {
            return;
        };
        let root = conn.setup().roots[screen].root;
        let atom = |name: &str| -> Option<Atom> {
            Some(
                conn.intern_atom(false, name.as_bytes())
                    .ok()?
                    .reply()
                    .ok()?
                    .atom,
            )
        };
        let (Some(state), Some(vert), Some(horz)) = (
            atom("_NET_WM_STATE"),
            atom("_NET_WM_STATE_MAXIMIZED_VERT"),
            atom("_NET_WM_STATE_MAXIMIZED_HORZ"),
        ) else {
            return;
        };
        let start = Instant::now();
        while start.elapsed() < WATCH_FOR {
            if let Some(window) = find(&conn, root, pid) {
                let event =
                    ClientMessageEvent::new(32, window, state, [STATE_ADD, vert, horz, 1, 0]);
                let mask = EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY;
                if conn.send_event(false, root, mask, event).is_ok() && conn.flush().is_ok() {
                    log::info!("maximized the game window");
                }
                return;
            }
            std::thread::sleep(POLL);
        }
    }

    /// The game's visible window: one made by the X client that is process `pid`.
    fn find(conn: &impl Connection, root: Window, pid: u32) -> Option<Window> {
        let base = client_base(conn, pid)?;
        let mask = conn.setup().resource_id_mask;
        search(conn, root, MAX_DEPTH, &|w| w & !mask == base)
    }

    /// The id range X gave process `pid`'s connection (what its windows' ids start with).
    fn client_base(conn: &impl Connection, pid: u32) -> Option<u32> {
        let all = ClientIdSpec {
            client: 0,
            mask: ClientIdMask::LOCAL_CLIENT_PID,
        };
        let reply = conn.res_query_client_ids(&[all]).ok()?.reply().ok()?;
        reply
            .ids
            .iter()
            .find(|id| id.value.first() == Some(&pid))
            .map(|id| id.spec.client)
    }

    fn search(
        conn: &impl Connection,
        window: Window,
        depth: u8,
        is_game: &dyn Fn(Window) -> bool,
    ) -> Option<Window> {
        let children = conn.query_tree(window).ok()?.reply().ok()?.children;
        for child in children {
            if is_game(child) && viewable(conn, child) {
                return Some(child);
            }
            if depth > 1
                && let Some(found) = search(conn, child, depth - 1, is_game)
            {
                return Some(found);
            }
        }
        None
    }

    fn viewable(conn: &impl Connection, window: Window) -> bool {
        conn.get_window_attributes(window)
            .ok()
            .and_then(|c| c.reply().ok())
            .is_some_and(|a| a.map_state == MapState::VIEWABLE)
    }
}
