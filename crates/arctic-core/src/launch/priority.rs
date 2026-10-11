//! A starting game goes ahead of other programs on the CPU. Startup is tens
//! of seconds of pure CPU work (Fabric, Mixin, Minecraft's bootstrap), and
//! with a build, an update or a browser busy in the background a game at
//! normal priority got a fraction of the CPU: 30-80 s starts instead of
//! ~10 s. The game runs above normal until it is ready, then goes back to
//! normal so it doesn't push the rest of the PC aside while playing.
//!
//! Windows only: Linux and macOS don't let a program raise another's
//! priority without admin rights.

use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// After the window opens, how long until the game counts as ready when it
/// doesn't say so itself (only the Arctic Client prints when it is ready).
const READY_AFTER_WINDOW: Duration = Duration::from_secs(15);
/// The boost never outlasts this, whatever the game prints.
const MAX_BOOST: Duration = Duration::from_secs(180);
const POLL: Duration = Duration::from_millis(250);

/// The Arctic Client's line once the loading screen is gone.
pub const READY_MARKER: &str = "[Arctic startup]";

/// Start the process above normal priority.
pub fn boost(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(windows_sys::Win32::System::Threading::ABOVE_NORMAL_PRIORITY_CLASS);
    }
    #[cfg(not(windows))]
    let _ = cmd;
}

/// Put process `pid` back to normal priority once `ready` is set, or a while
/// after `window` is, or after [`MAX_BOOST`] at the latest.
pub fn settle_when_ready(pid: u32, ready: Arc<AtomicBool>, window: Arc<AtomicBool>) {
    if !cfg!(windows) {
        return;
    }
    std::thread::spawn(move || {
        let start = Instant::now();
        let mut window_at = None;
        while !should_settle(
            start.elapsed(),
            ready.load(Ordering::Relaxed),
            &mut window_at,
            window.load(Ordering::Relaxed),
        ) {
            std::thread::sleep(POLL);
        }
        set_normal(pid);
    });
}

/// Time to go back to normal? `window_at` remembers when the window was
/// first seen open (as time since the start).
fn should_settle(
    elapsed: Duration,
    ready: bool,
    window_at: &mut Option<Duration>,
    window: bool,
) -> bool {
    if window && window_at.is_none() {
        *window_at = Some(elapsed);
    }
    ready
        || elapsed >= MAX_BOOST
        || window_at.is_some_and(|at| elapsed.saturating_sub(at) >= READY_AFTER_WINDOW)
}

#[cfg(windows)]
fn set_normal(pid: u32) {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        NORMAL_PRIORITY_CLASS, OpenProcess, PROCESS_SET_INFORMATION, SetPriorityClass,
    };
    // SAFETY: plain Win32 calls; the handle is checked and closed here.
    unsafe {
        let handle = OpenProcess(PROCESS_SET_INFORMATION, 0, pid);
        if handle.is_null() {
            // Already gone.
            return;
        }
        if SetPriorityClass(handle, NORMAL_PRIORITY_CLASS) == 0 {
            log::info!("couldn't set the game ({pid}) back to normal priority");
        }
        CloseHandle(handle);
    }
}

#[cfg(not(windows))]
fn set_normal(_pid: u32) {}

#[cfg(test)]
mod tests {
    use super::*;

    const S: fn(u64) -> Duration = Duration::from_secs;

    #[test]
    fn stays_boosted_while_starting() {
        let mut window_at = None;
        assert!(!should_settle(S(20), false, &mut window_at, false));
    }

    #[test]
    fn settles_as_soon_as_the_game_is_ready() {
        let mut window_at = None;
        assert!(should_settle(S(9), true, &mut window_at, true));
    }

    #[test]
    fn settles_a_while_after_the_window_without_a_ready_line() {
        let mut window_at = None;
        assert!(!should_settle(S(30), false, &mut window_at, true));
        assert!(!should_settle(S(44), false, &mut window_at, true));
        assert!(should_settle(S(45), false, &mut window_at, true));
    }

    #[cfg(windows)]
    #[test]
    fn a_boosted_process_starts_above_normal_and_settles_to_normal() {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::{
            ABOVE_NORMAL_PRIORITY_CLASS, GetPriorityClass, NORMAL_PRIORITY_CLASS, OpenProcess,
            PROCESS_QUERY_LIMITED_INFORMATION,
        };
        let class = |pid: u32| unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            assert!(!handle.is_null());
            let class = GetPriorityClass(handle);
            CloseHandle(handle);
            class
        };
        let mut cmd = Command::new("ping");
        cmd.args(["-n", "5", "127.0.0.1"])
            .stdout(std::process::Stdio::null());
        boost(&mut cmd);
        let mut child = cmd.spawn().unwrap();
        assert_eq!(class(child.id()), ABOVE_NORMAL_PRIORITY_CLASS);
        set_normal(child.id());
        assert_eq!(class(child.id()), NORMAL_PRIORITY_CLASS);
        let _ = child.kill();
        let _ = child.wait();
    }

    #[test]
    fn never_stays_boosted_past_the_limit() {
        let mut window_at = None;
        assert!(should_settle(MAX_BOOST, false, &mut window_at, false));
    }
}
