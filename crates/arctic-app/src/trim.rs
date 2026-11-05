//! Giving memory back while the launcher is out of sight (in the tray, minimized for a game, or
//! behind a running game): decoded copies of images already on the GPU are dropped, the heap is
//! compacted, and the system is told it may move the rest out of RAM until the window is used
//! again. Nothing the window needs is lost; it's read back when it's drawn.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use eframe::egui;

/// While a game runs and the launcher isn't in front: trim at most this often.
const BEHIND_GAME_EVERY: Duration = Duration::from_secs(60);
/// The last trim, in seconds since the Unix epoch (0: never).
static LAST: AtomicU64 = AtomicU64::new(0);

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Keys last used more than `limit` before `now`.
pub fn stale_keys(used: &HashMap<String, Instant>, now: Instant, limit: Duration) -> Vec<String> {
    used.iter()
        .filter(|(_, at)| now.saturating_duration_since(**at) > limit)
        .map(|(key, _)| key.clone())
        .collect()
}

/// Was something last used (or never) more than `limit` before `now`?
pub fn is_idle(seen: Option<Instant>, now: Instant, limit: Duration) -> bool {
    seen.is_none_or(|at| now.saturating_duration_since(at) > limit)
}

/// The window was hidden or minimized: give memory back now. The app has already let go of its own
/// textures (`ArcticApp::trim_now`); this empties egui's image caches, the GPU copies included
/// (they are uploaded again from the file bytes when drawn), and the system's side.
pub fn now(ctx: &egui::Context) {
    LAST.store(now_secs(), Ordering::Relaxed);
    let started = Instant::now();
    forget_decoded(ctx);
    for loader in ctx.loaders().texture.lock().iter() {
        loader.forget_all();
    }
    system_trim();
    log::debug!("memory trimmed in {:?}", started.elapsed());
}

/// The decoded pixels of every image shown: the GPU keeps its own copy (drawn from that) and the
/// file's bytes stay, so a decode only happens again if the image's size changes.
pub fn forget_decoded(ctx: &egui::Context) {
    for loader in ctx.loaders().image.lock().iter() {
        loader.forget_all();
    }
}

/// The tab drawn last frame.
static LAST_TAB: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// The window was minimized last frame (a minimize by hand trims once).
static MINIMIZED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Each frame: is it time to trim (the window was just minimized, or it has been a minute behind a
/// running game, where the window may stay open)? The caller trims.
pub fn tick(ctx: &egui::Context, game_running: bool, tab: usize) -> bool {
    // A new tab: the last one's decoded images aren't needed (their GPU copies draw them).
    if LAST_TAB.swap(tab, Ordering::Relaxed) != tab {
        forget_decoded(ctx);
    }
    let minimized = ctx.input(|i| i.viewport().minimized == Some(true));
    if minimized && !MINIMIZED.swap(true, Ordering::Relaxed) {
        return true;
    }
    if !minimized {
        MINIMIZED.store(false, Ordering::Relaxed);
    }
    if !game_running || ctx.input(|i| i.focused) {
        return false;
    }
    let last = LAST.load(Ordering::Relaxed);
    now_secs().saturating_sub(last) >= BEHIND_GAME_EVERY.as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_keys_are_those_unused_past_the_limit() {
        let start = Instant::now();
        let later = start + Duration::from_secs(30);
        let used = HashMap::from([
            ("old".to_owned(), start),
            ("fresh".to_owned(), later - Duration::from_secs(5)),
        ]);
        assert_eq!(
            stale_keys(&used, later, Duration::from_secs(20)),
            vec!["old".to_owned()]
        );
        assert!(stale_keys(&used, start, Duration::from_secs(20)).is_empty());
    }

    #[test]
    fn idle_when_never_seen_or_past_the_limit() {
        let start = Instant::now();
        let limit = Duration::from_secs(10);
        assert!(is_idle(None, start, limit));
        assert!(!is_idle(Some(start), start + Duration::from_secs(5), limit));
        assert!(is_idle(Some(start), start + Duration::from_secs(11), limit));
    }
}

#[cfg(windows)]
fn system_trim() {
    use windows_sys::Win32::System::Memory::{GetProcessHeap, HeapCompact};
    use windows_sys::Win32::System::ProcessStatus::K32EmptyWorkingSet;
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    // SAFETY: the process's own heap and pseudo-handle; both calls only ask Windows to release
    // free heap blocks and to page out what isn't in use (pages come back when touched).
    unsafe {
        HeapCompact(GetProcessHeap(), 0);
        K32EmptyWorkingSet(GetCurrentProcess());
    }
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn system_trim() {
    // SAFETY: glibc's own call to hand free heap memory back to the system.
    unsafe {
        libc::malloc_trim(0);
    }
}

#[cfg(not(any(windows, all(target_os = "linux", target_env = "gnu"))))]
fn system_trim() {}
