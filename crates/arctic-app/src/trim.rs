//! Giving memory back while the launcher is out of sight (in the tray, minimized for a game, or
//! behind a running game): decoded copies of images already on the GPU are dropped, the heap is
//! compacted, and the system is told it may move the rest out of RAM until the window is used
//! again. Nothing the window needs is lost; it's read back when it's drawn.

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

/// The window was hidden or minimized: give memory back now.
pub fn now(ctx: &egui::Context) {
    LAST.store(now_secs(), Ordering::Relaxed);
    let started = Instant::now();
    forget_decoded(ctx);
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

/// Each frame: trim once when the window is minimized, and now and then behind a running game
/// (the window may stay open there).
pub fn tick(ctx: &egui::Context, game_running: bool, tab: usize) {
    // A new tab: the last one's decoded images aren't needed (their GPU copies draw them).
    if LAST_TAB.swap(tab, Ordering::Relaxed) != tab {
        forget_decoded(ctx);
    }
    let minimized = ctx.input(|i| i.viewport().minimized == Some(true));
    if minimized && !MINIMIZED.swap(true, Ordering::Relaxed) {
        now(ctx);
        return;
    }
    if !minimized {
        MINIMIZED.store(false, Ordering::Relaxed);
    }
    if !game_running || ctx.input(|i| i.focused) {
        return;
    }
    let last = LAST.load(Ordering::Relaxed);
    if now_secs().saturating_sub(last) >= BEHIND_GAME_EVERY.as_secs() {
        now(ctx);
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
