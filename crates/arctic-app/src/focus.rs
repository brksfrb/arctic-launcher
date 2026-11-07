//! Bring a running game's window to the front (for "Join" while the game runs).
//!
//! Best effort: where the OS or the desktop doesn't allow it (Wayland, a missing helper program), the
//! game just isn't focused and `false` is returned.

/// Raise the top-level window of the process `pid`. Returns whether a window was found and asked to
/// come forward.
pub fn game_window(pid: u32) -> bool {
    imp::focus(pid)
}

#[cfg(windows)]
mod imp {
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GW_OWNER, GetWindow, GetWindowThreadProcessId, IsIconic, IsWindowVisible,
        SW_RESTORE, SetForegroundWindow, ShowWindow,
    };

    struct Search {
        pid: u32,
        found: HWND,
    }

    unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> i32 {
        // SAFETY: `lparam` is the `Search` on the caller's stack, alive for the whole enumeration.
        let search = unsafe { &mut *(lparam as *mut Search) };
        let mut owner_pid = 0u32;
        // SAFETY: plain queries on a window handle the system just gave us.
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut owner_pid);
            // A top-level, visible window that nothing owns (not a tool popup).
            if owner_pid == search.pid
                && IsWindowVisible(hwnd) != 0
                && GetWindow(hwnd, GW_OWNER).is_null()
            {
                search.found = hwnd;
                return 0;
            }
        }
        1
    }

    pub fn focus(pid: u32) -> bool {
        let mut search = Search {
            pid,
            found: std::ptr::null_mut(),
        };
        // SAFETY: `visit` only touches `search`, which outlives the call.
        unsafe {
            EnumWindows(Some(visit), &mut search as *mut Search as LPARAM);
        }
        if search.found.is_null() {
            return false;
        }
        // SAFETY: a window handle found a moment ago; the calls are harmless if it has gone.
        unsafe {
            if IsIconic(search.found) != 0 {
                ShowWindow(search.found, SW_RESTORE);
            }
            // The launcher is the foreground app (the player just clicked in it), which is what lets
            // this succeed.
            SetForegroundWindow(search.found) != 0
        }
    }
}

#[cfg(target_os = "macos")]
mod imp {
    pub fn focus(pid: u32) -> bool {
        let script = format!(
            "tell application \"System Events\" to set frontmost of (first process whose unix id is {pid}) to true"
        );
        std::process::Command::new("osascript")
            .args(["-e", &script])
            .status()
            .is_ok_and(|s| s.success())
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
mod imp {
    use std::process::{Command, Stdio};

    /// X11 (and XWayland, where Minecraft runs): `xdotool` finds the window by process and raises
    /// it. Wayland itself lets no program raise another's window, so there it fails quietly.
    pub fn focus(pid: u32) -> bool {
        let quiet = |mut c: Command| {
            c.stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            c
        };
        let mut search = quiet(Command::new("xdotool"));
        search.args(["search", "--onlyvisible", "--pid", &pid.to_string()]);
        let Ok(found) = search.stdout(Stdio::piped()).output() else {
            return false;
        };
        let text = String::from_utf8_lossy(&found.stdout);
        let Some(window) = text.lines().next_back().filter(|w| !w.is_empty()) else {
            return false;
        };
        quiet(Command::new("xdotool"))
            .args(["windowactivate", window])
            .status()
            .is_ok_and(|s| s.success())
    }
}
