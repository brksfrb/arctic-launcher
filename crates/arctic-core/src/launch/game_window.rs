//! "Start the game maximized": Minecraft has no option for it, so on Windows the launcher finds
//! the game's window once it opens and maximizes it. Every version and loader opens its window
//! through LWJGL (GLFW from 1.13, LWJGL 2 before), so that's the window looked for.

use std::time::Duration;

/// How long after the start the game's window is looked for (a first start after an update is slow).
#[cfg_attr(not(windows), allow(dead_code))]
const WATCH_FOR: Duration = Duration::from_secs(180);
#[cfg_attr(not(windows), allow(dead_code))]
const POLL: Duration = Duration::from_millis(150);

/// Maximize the window game `pid` opens, once (a player who restores it keeps it that way).
pub fn maximize_when_open(pid: u32) {
    #[cfg(windows)]
    std::thread::spawn(move || imp::watch(pid));
    #[cfg(not(windows))]
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
