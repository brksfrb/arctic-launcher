//! Running the game: spawn, tee output to the log file, detect when the
//! window is up, and report the exit, all on background threads so the
//! launcher UI never waits on the game.

use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::LaunchPlan;
use super::logparse::{LogLine, LogParser};
use crate::Result;
use crate::error::IoContext;

/// How often the watcher checks for exit or a kill request.
const WATCH_INTERVAL: Duration = Duration::from_millis(250);

/// Log lines printed right around the moment the game window opens:
/// modern versions (1.13+), LWJGL 2 era, and a late fallback.
const WINDOW_MARKERS: [&str; 3] = ["Backend library:", "LWJGL Version:", "Sound engine started"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameEvent {
    /// The game window is (about to be) visible.
    WindowReady,
    /// The process ended. `code` is `None` if it was killed by a signal.
    Exited { code: Option<i32> },
    /// Parsed output lines (one raw line may expand to several).
    Output(Vec<LogLine>),
}

/// Keeps a game's windows out of sight while it loads in the background
/// ("ready to play"), and shows them when the player presses Play.
#[derive(Debug, Default)]
struct Hold {
    /// While true, any window the game opens is hidden at once.
    hiding: bool,
    /// The windows hidden so far (native handles).
    windows: Vec<isize>,
}

#[cfg(windows)]
mod windows_guard {
    use std::ffi::c_void;

    use windows_sys::Win32::UI::WindowsAndMessaging::{
        AllowSetForegroundWindow, EnumWindows, GetWindowThreadProcessId, IsIconic, IsWindowVisible,
        SW_HIDE, SW_RESTORE, SW_SHOW, SetForegroundWindow, ShowWindow,
    };

    struct Search {
        pid: u32,
        visible_only: bool,
        found: Vec<isize>,
    }

    unsafe extern "system" fn each(hwnd: *mut c_void, lparam: isize) -> i32 {
        // SAFETY: `lparam` is the address of the `Search` that `windows_of` keeps alive for the call.
        let search = unsafe { &mut *(lparam as *mut Search) };
        let mut pid = 0u32;
        // SAFETY: `hwnd` comes from EnumWindows; `pid` is a valid out pointer.
        unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        // SAFETY: as above.
        if pid == search.pid && (!search.visible_only || unsafe { IsWindowVisible(hwnd) } != 0) {
            search.found.push(hwnd as isize);
        }
        1
    }

    fn windows_of(pid: u32, visible_only: bool) -> Vec<isize> {
        let mut search = Search {
            pid,
            visible_only,
            found: Vec::new(),
        };
        // SAFETY: the callback only uses `search`, which outlives the call.
        unsafe { EnumWindows(Some(each), &mut search as *mut Search as isize) };
        search.found
    }

    /// Hide every visible top-level window of `pid`; the ones hidden.
    pub fn hide_visible(pid: u32) -> Vec<isize> {
        let found = windows_of(pid, true);
        for hwnd in &found {
            // SAFETY: a window handle of the game's process; hiding is harmless if it's gone.
            unsafe { ShowWindow(*hwnd as *mut c_void, SW_HIDE) };
        }
        found
    }

    /// Show `windows` again and bring the first to the front.
    pub fn show(pid: u32, windows: &[isize]) {
        // The launcher is in front when Play is pressed: let the game take over.
        // SAFETY: plain API calls on handles of the game's process.
        unsafe { AllowSetForegroundWindow(pid) };
        for hwnd in windows {
            let hwnd = *hwnd as *mut c_void;
            // SAFETY: as above.
            unsafe {
                ShowWindow(hwnd, SW_SHOW);
                if IsIconic(hwnd) != 0 {
                    ShowWindow(hwnd, SW_RESTORE);
                }
            }
        }
        if let Some(first) = windows.first() {
            // SAFETY: as above.
            unsafe { SetForegroundWindow(*first as *mut c_void) };
        }
    }
}

#[cfg(not(windows))]
mod windows_guard {
    pub fn hide_visible(_pid: u32) -> Vec<isize> {
        Vec::new()
    }

    pub fn show(_pid: u32, _windows: &[isize]) {}
}

/// How often hidden-game windows are looked for (a window is hidden a few
/// milliseconds after it appears).
const GUARD_INTERVAL: Duration = Duration::from_millis(4);
/// The guard stops by itself after this long.
const GUARD_LIMIT: Duration = Duration::from_secs(30 * 60);

/// Handle to a running game. Dropping it does not stop the game.
#[derive(Debug, Clone)]
pub struct GameHandle {
    pid: u32,
    kill: Arc<AtomicBool>,
    hold: Arc<Mutex<Hold>>,
}

impl GameHandle {
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// End the game now, without waiting for the watcher (the launcher is
    /// quitting and its threads go with it).
    pub fn terminate(&self) {
        terminate(self.pid);
    }

    /// Ask the watcher to terminate the game (Force close).
    pub fn kill(&self) {
        self.kill.store(true, Ordering::Relaxed);
    }

    /// Keep the game's windows hidden from now on (Windows): it loads in the
    /// background until [`GameHandle::reveal`]. Elsewhere this does nothing and
    /// the game's window just shows.
    pub fn hide_windows(&self) {
        if !cfg!(windows) {
            return;
        }
        match self.hold.lock() {
            Ok(mut hold) if !hold.hiding => hold.hiding = true,
            _ => return,
        }
        let (pid, hold) = (self.pid, self.hold.clone());
        std::thread::spawn(move || {
            let started = std::time::Instant::now();
            loop {
                {
                    let Ok(mut hold) = hold.lock() else {
                        return;
                    };
                    if !hold.hiding {
                        return;
                    }
                    for window in windows_guard::hide_visible(pid) {
                        if !hold.windows.contains(&window) {
                            hold.windows.push(window);
                        }
                    }
                }
                if started.elapsed() > GUARD_LIMIT {
                    return;
                }
                std::thread::sleep(GUARD_INTERVAL);
            }
        });
    }

    /// Show the windows [`GameHandle::hide_windows`] kept hidden, in front.
    pub fn reveal(&self) {
        let Ok(mut hold) = self.hold.lock() else {
            return;
        };
        hold.hiding = false;
        let windows = std::mem::take(&mut hold.windows);
        windows_guard::show(self.pid, &windows);
    }
}

/// Start the game. Output is written to `plan.log_file`; `on_event` is
/// called from background threads.
pub fn spawn(
    plan: &LaunchPlan,
    on_event: impl Fn(GameEvent) + Send + Sync + 'static,
) -> Result<GameHandle> {
    log::info!("launching: {}", plan.redacted_command());
    let log = Arc::new(Mutex::new(File::create(&plan.log_file).at(&plan.log_file)?));
    let mut child = Command::new(game_exe(plan))
        .args(&plan.args)
        .current_dir(&plan.game_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .at(&plan.java)?;

    let on_event = Arc::new(on_event);
    let window_seen = Arc::new(AtomicBool::new(false));
    let secrets: Arc<[String]> = plan.log_secrets().into();
    let tee = |stream: Box<dyn Read + Send>| {
        let (log, seen, on_event) = (log.clone(), window_seen.clone(), on_event.clone());
        let secrets = secrets.clone();
        std::thread::spawn(move || tee_output(stream, &log, &seen, &secrets, &*on_event));
    };
    if let Some(out) = child.stdout.take() {
        tee(Box::new(out));
    }
    if let Some(err) = child.stderr.take() {
        tee(Box::new(err));
    }

    let kill = Arc::new(AtomicBool::new(false));
    let handle = GameHandle {
        pid: child.id(),
        kill: kill.clone(),
        hold: Arc::default(),
    };
    std::thread::spawn(move || {
        let code = loop {
            if kill.swap(false, Ordering::Relaxed) {
                let _ = child.kill();
            }
            match child.try_wait() {
                Ok(Some(status)) => break status.code(),
                Ok(None) => std::thread::sleep(WATCH_INTERVAL),
                Err(e) => {
                    log::error!("lost track of the game process: {e}");
                    break None;
                }
            }
        };
        on_event(GameEvent::Exited { code });
    });
    Ok(handle)
}

/// Start the game fully detached from this process: output goes straight
/// to `plan.log_file` (no pipes), so the game keeps running and logging
/// after a short-lived caller (e.g. the CLI) exits. Returns the pid.
pub fn spawn_detached(plan: &LaunchPlan) -> Result<u32> {
    log::info!("launching (detached): {}", plan.redacted_command());
    let log = File::create(&plan.log_file).at(&plan.log_file)?;
    let log_err = log.try_clone().at(&plan.log_file)?;
    let child = Command::new(game_exe(plan))
        .args(&plan.args)
        .current_dir(&plan.game_dir)
        .stdin(Stdio::null())
        .stdout(log)
        .stderr(log_err)
        .spawn()
        .at(&plan.java)?;
    Ok(child.id())
}

/// End process `pid` (and, on Windows, what it started).
pub fn terminate(pid: u32) {
    let mut cmd = if cfg!(windows) {
        let mut c = Command::new("taskkill");
        c.args(["/PID", &pid.to_string(), "/T", "/F"]);
        c
    } else {
        let mut c = Command::new("kill");
        c.arg(pid.to_string());
        c
    };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: no console flashing up.
        cmd.creation_flags(0x0800_0000);
    }
    if let Err(e) = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        log::warn!("couldn't end the game ({pid}): {e}");
    }
}

/// What the game's process is called on Windows and macOS.
const CLIENT_PROCESS: &str = if cfg!(windows) {
    "Arctic Client.exe"
} else {
    "arctic-client"
};

/// Discord marks any `javaw.exe` (macOS: `java`) running Minecraft as
/// "Playing Minecraft", which buries the launcher's own presence. A copy of
/// the Java launcher under the client's name, next to the original (it finds
/// the rest of Java from its own folder), keeps that from happening and
/// shows "Arctic Client" in Task Manager. Falls back to `java` on any error
/// (a read-only Java folder, say).
/// The executable the game runs as, set to the graphics card the player wants.
fn game_exe(plan: &LaunchPlan) -> PathBuf {
    let exe = branded_java(&plan.java);
    crate::gpu_preference::apply(&exe, plan.high_performance_gpu);
    exe
}

fn branded_java(java: &Path) -> PathBuf {
    let is_java = java
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| matches!(n, "javaw.exe" | "java"));
    if !is_java || !(cfg!(windows) || cfg!(target_os = "macos")) {
        return java.to_path_buf();
    }
    let copy = java.with_file_name(CLIENT_PROCESS);
    let same = |a: &Path, b: &Path| match (std::fs::metadata(a), std::fs::metadata(b)) {
        (Ok(a), Ok(b)) => a.len() == b.len() && a.modified().ok() <= b.modified().ok(),
        _ => false,
    };
    if !same(java, &copy)
        && let Err(e) = std::fs::copy(java, &copy)
    {
        log::info!("running the game as {}: {e}", java.display());
        return java.to_path_buf();
    }
    in_named_folder(&copy)
}

/// NVIDIA's recorder files clips under the name of the folder that holds
/// `bin`, which is the runtime's (`java-runtime-delta`). The game is started
/// through a junction named after the client instead:
/// `runtimes/.named/<runtime>/Arctic Client` → `runtimes/<runtime>`. Java
/// works out its home from that path, so nothing else changes.
#[cfg(windows)]
fn in_named_folder(exe: &Path) -> PathBuf {
    const NAME: &str = "Arctic Client";
    let named = (|| {
        let bin = exe.parent()?;
        let runtime = bin.parent()?;
        if bin.file_name()? != "bin" || runtime.file_name()? == NAME {
            return None;
        }
        let link = runtime
            .parent()?
            .join(".named")
            .join(runtime.file_name()?)
            .join(NAME);
        if junction::get_target(&link).ok().as_deref() != Some(runtime) {
            let _ = std::fs::remove_dir(&link);
            std::fs::create_dir_all(link.parent()?).ok()?;
            junction::create(runtime, &link)
                .inspect_err(|e| log::info!("junction for the game's name: {e}"))
                .ok()?;
        }
        Some(link.join("bin").join(exe.file_name()?))
    })();
    named.unwrap_or_else(|| exe.to_path_buf())
}

#[cfg(not(windows))]
fn in_named_folder(exe: &Path) -> PathBuf {
    exe.to_path_buf()
}

/// Copy a child stream into the log line by line, firing `WindowReady` the
/// first time a marker shows up (on either stream). `secrets` never reach
/// the log or the launcher (1.8 prints its session token, for one).
fn tee_output(
    stream: Box<dyn Read + Send>,
    log: &Mutex<File>,
    window_seen: &AtomicBool,
    secrets: &[String],
    on_event: &(dyn Fn(GameEvent) + Send + Sync),
) {
    let mut reader = BufReader::new(stream);
    let mut parser = LogParser::default();
    let mut line = Vec::new();
    loop {
        line.clear();
        match reader.read_until(b'\n', &mut line) {
            Ok(0) | Err(_) => {
                let rest = parser.finish();
                if !rest.is_empty() {
                    on_event(GameEvent::Output(rest));
                }
                return;
            }
            Ok(_) => {}
        }
        redact(&mut line, secrets);
        if let Ok(mut file) = log.lock() {
            let _ = file.write_all(&line);
        }
        // `swap` makes the check-and-set atomic: stdout and stderr are read
        // by separate threads and must not both fire.
        if is_window_marker(&line) && !window_seen.swap(true, Ordering::Relaxed) {
            on_event(GameEvent::WindowReady);
        }
        let parsed = parser.feed(&String::from_utf8_lossy(&line));
        if !parsed.is_empty() {
            on_event(GameEvent::Output(parsed));
        }
    }
}

/// Replace every secret in `line` with `<redacted>`.
fn redact(line: &mut Vec<u8>, secrets: &[String]) {
    for secret in secrets.iter().map(String::as_bytes) {
        while let Some(at) = line.windows(secret.len()).position(|w| w == secret) {
            line.splice(at..at + secret.len(), b"<redacted>".iter().copied());
        }
    }
}

fn is_window_marker(line: &[u8]) -> bool {
    let text = String::from_utf8_lossy(line);
    WINDOW_MARKERS.iter().any(|m| text.contains(m))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(any(windows, target_os = "macos"))]
    fn the_game_runs_as_arctic_client() {
        let dir = tempfile::tempdir().unwrap();
        let java = dir
            .path()
            .join(if cfg!(windows) { "javaw.exe" } else { "java" });
        std::fs::write(&java, b"java v1").unwrap();
        let first = branded_java(&java);
        assert!(first.ends_with(CLIENT_PROCESS), "{}", first.display());
        assert!(dir.path().join(CLIENT_PROCESS).is_file());
        assert_eq!(std::fs::read(&first).unwrap(), b"java v1");
        // A Java update is copied again.
        std::fs::write(&java, b"java v2 (longer)").unwrap();
        assert_eq!(
            std::fs::read(branded_java(&java)).unwrap(),
            b"java v2 (longer)"
        );
        // Started from a folder named after the client (for recorders).
        let runtime = dir.path().join("java-runtime-delta");
        let bin = runtime.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join(java.file_name().unwrap()), b"java").unwrap();
        let named = branded_java(&bin.join(java.file_name().unwrap()));
        if cfg!(windows) {
            assert!(
                named.ends_with(format!("Arctic Client/bin/{CLIENT_PROCESS}")),
                "{}",
                named.display()
            );
            assert!(named.starts_with(dir.path().join(".named").join("java-runtime-delta")));
            assert_eq!(std::fs::read(&named).unwrap(), b"java");
            // A second launch reuses the same junction.
            assert_eq!(branded_java(&bin.join(java.file_name().unwrap())), named);
        }
        // Anything that isn't the Java launcher is left alone.
        let other = dir.path().join("custom-java.exe");
        assert_eq!(branded_java(&other), other);
    }

    #[test]
    fn detects_window_markers_in_plain_and_xml_logs() {
        assert!(is_window_marker(
            b"[12:00:01] [Render thread/INFO]: Backend library: LWJGL version 3.3.3"
        ));
        assert!(is_window_marker(
            b"<log4j:Message><![CDATA[Backend library: LWJGL version 3.4.3+4]]></log4j:Message>"
        ));
        assert!(is_window_marker(
            b"2013-04-01 [CLIENT] [INFO] LWJGL Version: 2.9.0"
        ));
        assert!(!is_window_marker(b"Setting user: Steve"));
    }

    #[test]
    fn tee_writes_log_and_fires_once() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("game.log");
        let log = Mutex::new(File::create(&path).unwrap());
        let seen = AtomicBool::new(false);
        let fired = Mutex::new(Vec::new());
        let input = b"a\nLWJGL Version: 2\nLWJGL Version: 2\nz".to_vec();
        tee_output(
            Box::new(std::io::Cursor::new(input.clone())),
            &log,
            &seen,
            &[],
            &|e| fired.lock().unwrap().push(e),
        );
        drop(log);
        assert_eq!(std::fs::read(&path).unwrap(), input);
        let fired = fired.into_inner().unwrap();
        let ready = fired
            .iter()
            .filter(|e| **e == GameEvent::WindowReady)
            .count();
        let lines: usize = fired
            .iter()
            .map(|e| match e {
                GameEvent::Output(l) => l.len(),
                _ => 0,
            })
            .sum();
        assert_eq!((ready, lines), (1, 4));
    }

    #[test]
    fn session_tokens_stay_out_of_the_log() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("game.log");
        let log = Mutex::new(File::create(&path).unwrap());
        let seen = AtomicBool::new(false);
        let input = b"[Client thread/INFO] (Session ID is token:eyJ.SECRET:15ca)
ok SECRET SECRET
"
        .to_vec();
        let shown = Mutex::new(Vec::new());
        tee_output(
            Box::new(std::io::Cursor::new(input)),
            &log,
            &seen,
            &["eyJ.SECRET".to_owned(), "SECRET".to_owned()],
            &|e| {
                if let GameEvent::Output(lines) = e {
                    shown
                        .lock()
                        .unwrap()
                        .extend(lines.into_iter().map(|l| l.text));
                }
            },
        );
        drop(log);
        let written = std::fs::read_to_string(&path).unwrap();
        assert!(!written.contains("SECRET"), "{written}");
        assert!(written.contains("token:<redacted>:15ca"));
        assert!(
            shown
                .into_inner()
                .unwrap()
                .iter()
                .all(|l| !l.contains("SECRET"))
        );
    }

    /// By hand (it opens a window for a moment): `cargo test -p arctic-core -- --ignored hidden_windows`.
    #[cfg(windows)]
    #[test]
    #[ignore = "opens a window"]
    fn hidden_windows_stay_hidden_until_revealed() {
        use windows_sys::Win32::UI::WindowsAndMessaging::IsWindowVisible;

        let mut child = Command::new("charmap.exe").spawn().unwrap();
        let game = GameHandle {
            pid: child.id(),
            kill: Arc::default(),
            hold: Arc::default(),
        };
        game.hide_windows();
        std::thread::sleep(Duration::from_secs(3));
        let hidden = game.hold.lock().unwrap().windows.len();
        assert!(hidden > 0, "no window of Character Map was hidden");
        let visible = |hwnd: isize| unsafe { IsWindowVisible(hwnd as *mut std::ffi::c_void) != 0 };
        let first = game.hold.lock().unwrap().windows[0];
        assert!(!visible(first), "still visible while hidden");
        game.reveal();
        assert!(visible(first), "not shown by reveal");
        let _ = child.kill();
    }
}
