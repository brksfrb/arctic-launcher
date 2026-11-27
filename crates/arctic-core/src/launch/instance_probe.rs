//! Which launch is which: a launch id, and whether another game was already running on the
//! same game directory (two launches seconds apart share files and are easy to confuse).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

const MARKER: &str = "arctic-game.pid";

/// Unique per launch within and across launcher runs: time, launcher pid, counter.
pub fn new_launch_id() -> String {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    format!(
        "{ms:x}-{:x}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtherInstance {
    pub pid: u32,
    pub launch: String,
}

fn marker_path(game_dir: &Path) -> PathBuf {
    game_dir.join(MARKER)
}

fn parse_marker(text: &str) -> Option<OtherInstance> {
    let (pid, launch) = text.trim().split_once(' ')?;
    let launch = launch.trim();
    if launch.is_empty()
        || !launch
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
    {
        return None;
    }
    Some(OtherInstance {
        pid: pid.parse().ok()?,
        launch: launch.to_owned(),
    })
}

/// A previous game on this directory that is still a running Java/Arctic process.
pub fn running_on(game_dir: &Path) -> Option<OtherInstance> {
    let other = parse_marker(&std::fs::read_to_string(marker_path(game_dir)).ok()?)?;
    (other.pid != std::process::id() && is_game_process(other.pid)).then_some(other)
}

/// Remember this launch so the next one on the directory can tell it is still running.
pub fn record(game_dir: &Path, pid: u32, launch: &str) {
    let _ = std::fs::write(marker_path(game_dir), format!("{pid} {launch}\n"));
}

/// Forget this launch when it ends, unless a newer launch has taken the marker over.
pub fn clear(game_dir: &Path, launch: &str) {
    let path = marker_path(game_dir);
    if std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| parse_marker(&t))
        .is_some_and(|m| m.launch == launch)
    {
        let _ = std::fs::remove_file(path);
    }
}

/// Process name looks like Java or the branded Arctic client (guards against pid reuse).
fn looks_like_game(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.contains("java") || name.contains("arctic")
}

#[cfg(unix)]
fn is_game_process(pid: u32) -> bool {
    std::process::Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .is_some_and(|o| looks_like_game(&String::from_utf8_lossy(&o.stdout)))
}

#[cfg(windows)]
fn is_game_process(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        QueryFullProcessImageNameW,
    };
    const STILL_ACTIVE: u32 = 259;
    // SAFETY: the handle is closed on every path; buffers are sized as passed.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return false;
        }
        let mut code = 0;
        let alive = GetExitCodeProcess(handle, &mut code) != 0 && code == STILL_ACTIVE;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let named = QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut len) != 0;
        CloseHandle(handle);
        alive && named && looks_like_game(&String::from_utf16_lossy(&buf[..len as usize]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_ids_differ_between_close_launches() {
        assert_ne!(new_launch_id(), new_launch_id());
    }

    #[test]
    fn marker_roundtrip_and_stale_or_foreign_markers_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(running_on(dir.path()), None);
        // The launcher's own pid is never a "game" even if recorded.
        record(dir.path(), std::process::id(), "a-1");
        assert_eq!(running_on(dir.path()), None);
        // A pid that cannot exist is stale.
        record(dir.path(), u32::MAX - 1, "a-2");
        assert_eq!(running_on(dir.path()), None);
        std::fs::write(marker_path(dir.path()), "garbage").unwrap();
        assert_eq!(running_on(dir.path()), None);
        assert_eq!(
            parse_marker("42 abc-1.x\n"),
            Some(OtherInstance {
                pid: 42,
                launch: "abc-1.x".into()
            })
        );
        assert_eq!(parse_marker("42 bad id"), None);
    }

    #[test]
    fn clear_only_removes_its_own_launch() {
        let dir = tempfile::tempdir().unwrap();
        record(dir.path(), 5, "second");
        clear(dir.path(), "first");
        assert!(marker_path(dir.path()).exists());
        clear(dir.path(), "second");
        assert!(!marker_path(dir.path()).exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_live_java_process_on_the_directory_is_detected() {
        let dir = tempfile::tempdir().unwrap();
        // `ps` reports a renamed copy of sleep as its file name, so use a java-named link.
        let link = dir.path().join("java-fake");
        let Ok(sleep) = which_sleep() else { return };
        std::os::unix::fs::symlink(sleep, &link).unwrap();
        let mut child = std::process::Command::new(&link).arg("30").spawn().unwrap();
        record(dir.path(), child.id(), "live-1");
        let found = running_on(dir.path());
        child.kill().ok();
        child.wait().ok();
        assert_eq!(
            found,
            Some(OtherInstance {
                pid: child.id(),
                launch: "live-1".into()
            })
        );
    }

    #[cfg(unix)]
    fn which_sleep() -> Result<PathBuf, ()> {
        ["/bin/sleep", "/usr/bin/sleep"]
            .iter()
            .map(PathBuf::from)
            .find(|p| p.exists())
            .ok_or(())
    }
}
