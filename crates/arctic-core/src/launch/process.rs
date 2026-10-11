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
use super::priority;
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

/// Handle to a running game. Dropping it does not stop the game.
#[derive(Debug, Clone)]
pub struct GameHandle {
    pid: u32,
    kill: Arc<AtomicBool>,
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
}

/// Start the game. Output is written to `plan.log_file`; `on_event` is
/// called from background threads.
pub fn spawn(
    plan: &LaunchPlan,
    on_event: impl Fn(GameEvent) + Send + Sync + 'static,
) -> Result<GameHandle> {
    log::info!("launching: {}", plan.redacted_command());
    let log = Arc::new(Mutex::new(File::create(&plan.log_file).at(&plan.log_file)?));
    let mut cmd = Command::new(game_exe(plan));
    cmd.args(&plan.args)
        .envs(crate::gpu_preference::env(plan.high_performance_gpu))
        .current_dir(&plan.game_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    priority::boost(&mut cmd);
    let mut child = cmd.spawn().at(&plan.java)?;
    if plan.maximize_window {
        super::game_window::maximize_when_open(child.id());
    }

    let on_event = Arc::new(on_event);
    let window_seen = Arc::new(AtomicBool::new(false));
    let ready_seen = Arc::new(AtomicBool::new(false));
    priority::settle_when_ready(child.id(), ready_seen.clone(), window_seen.clone());
    let secrets: Arc<[String]> = plan.log_secrets().into();
    let tee = |stream: Box<dyn Read + Send>| {
        let (log, seen, on_event) = (log.clone(), window_seen.clone(), on_event.clone());
        let (ready, secrets) = (ready_seen.clone(), secrets.clone());
        std::thread::spawn(move || {
            let seen = Seen {
                window: &seen,
                ready: &ready,
            };
            tee_output(stream, &log, seen, &secrets, &*on_event)
        });
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
    };
    let game_dir = plan.game_dir.clone();
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
        if code.is_some_and(|c| c != 0) {
            super::startup_cache::drop_pack(&game_dir);
        }
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
        .envs(crate::gpu_preference::env(plan.high_performance_gpu))
        .current_dir(&plan.game_dir)
        .stdin(Stdio::null())
        .stdout(log)
        .stderr(log_err)
        .spawn()
        .at(&plan.java)?;
    if plan.maximize_window {
        super::game_window::maximize_when_open(child.id());
    }
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
/// from a copy of the runtime named after the client instead,
/// `runtimes/.named/<runtime>/Arctic Client`, made of hard links (no extra
/// space). Java works out its home from that path, so nothing else changes.
/// (A junction isn't enough: recorders see the path it leads to.)
#[cfg(windows)]
fn in_named_folder(exe: &Path) -> PathBuf {
    const NAME: &str = "Arctic Client";
    let named = (|| {
        let bin = exe.parent()?;
        let runtime = bin.parent()?;
        if bin.file_name()? != "bin" || runtime.file_name()? == NAME {
            return None;
        }
        let named = runtime
            .parent()?
            .join(".named")
            .join(runtime.file_name()?)
            .join(NAME);
        // Earlier versions made a junction here: remove the link itself, never what it leads to.
        if junction::exists(&named).unwrap_or(false) {
            std::fs::remove_dir(&named)
                .inspect_err(|e| log::info!("old junction for the game's name: {e}"))
                .ok()?;
        }
        mirror_tree(runtime, &named)
            .inspect_err(|e| log::info!("named copy of the Java runtime: {e}"))
            .ok()?;
        Some(named.join("bin").join(exe.file_name()?))
    })();
    named.unwrap_or_else(|| exe.to_path_buf())
}

/// Make `dst` hold every file of `src` as a hard link (a plain copy where a link can't be
/// made). Files already linked are left alone, so this is quick once done; a file the runtime
/// replaced is linked again.
#[cfg(any(windows, test))]
fn mirror_tree(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let (from, to) = (entry.path(), dst.join(entry.file_name()));
        let kind = entry.file_type()?;
        if kind.is_dir() {
            mirror_tree(&from, &to)?;
            continue;
        }
        if !kind.is_file() || same_file(&from, &to) {
            continue;
        }
        let _ = std::fs::remove_file(&to);
        if std::fs::hard_link(&from, &to).is_err() {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

/// Already the same file (a hard link of it), or a copy as large and as new.
#[cfg(any(windows, test))]
fn same_file(a: &Path, b: &Path) -> bool {
    match (std::fs::metadata(a), std::fs::metadata(b)) {
        (Ok(a), Ok(b)) => a.len() == b.len() && a.modified().ok() == b.modified().ok(),
        _ => false,
    }
}

#[cfg(not(windows))]
fn in_named_folder(exe: &Path) -> PathBuf {
    exe.to_path_buf()
}

/// What the output readers of both streams have seen so far.
#[derive(Clone, Copy)]
struct Seen<'a> {
    window: &'a AtomicBool,
    /// The Arctic Client said the game is ready.
    ready: &'a AtomicBool,
}

/// Copy a child stream into the log line by line, firing `WindowReady` the
/// first time a marker shows up (on either stream). `secrets` never reach
/// the log or the launcher (1.8 prints its session token, for one).
fn tee_output(
    stream: Box<dyn Read + Send>,
    log: &Mutex<File>,
    seen: Seen,
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
        if is_window_marker(&line) && !seen.window.swap(true, Ordering::Relaxed) {
            on_event(GameEvent::WindowReady);
        }
        if String::from_utf8_lossy(&line).contains(priority::READY_MARKER) {
            seen.ready.store(true, Ordering::Relaxed);
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
    #[test]
    fn named_runtime_copy_links_every_file_and_updates_changed_ones() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = dir.path().join("java-runtime-delta");
        std::fs::create_dir_all(runtime.join("bin")).unwrap();
        std::fs::create_dir_all(runtime.join("lib/security")).unwrap();
        std::fs::write(runtime.join("bin/javaw.exe"), b"java").unwrap();
        std::fs::write(runtime.join("lib/security/cacerts"), b"certs").unwrap();
        let named = dir.path().join(".named/java-runtime-delta/Arctic Client");
        mirror_tree(&runtime, &named).unwrap();
        assert_eq!(std::fs::read(named.join("bin/javaw.exe")).unwrap(), b"java");
        assert_eq!(
            std::fs::read(named.join("lib/security/cacerts")).unwrap(),
            b"certs"
        );
        // A runtime update replaces a file: the copy follows.
        std::fs::remove_file(runtime.join("lib/security/cacerts")).unwrap();
        std::fs::write(runtime.join("lib/security/cacerts"), b"newer certs").unwrap();
        mirror_tree(&runtime, &named).unwrap();
        assert_eq!(
            std::fs::read(named.join("lib/security/cacerts")).unwrap(),
            b"newer certs"
        );
    }

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
        let (window, ready_seen) = (AtomicBool::new(false), AtomicBool::new(false));
        let seen = Seen {
            window: &window,
            ready: &ready_seen,
        };
        let fired = Mutex::new(Vec::new());
        let input =
            b"a\nLWJGL Version: 2\nLWJGL Version: 2\n[Arctic startup] ready 9000 ms\nz".to_vec();
        tee_output(
            Box::new(std::io::Cursor::new(input.clone())),
            &log,
            seen,
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
        assert_eq!((ready, lines), (1, 5));
        assert!(ready_seen.load(Ordering::Relaxed));
    }

    #[test]
    fn session_tokens_stay_out_of_the_log() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("game.log");
        let log = Mutex::new(File::create(&path).unwrap());
        let (window, ready) = (AtomicBool::new(false), AtomicBool::new(false));
        let seen = Seen {
            window: &window,
            ready: &ready,
        };
        let input = b"[Client thread/INFO] (Session ID is token:eyJ.SECRET:15ca)
ok SECRET SECRET
"
        .to_vec();
        let shown = Mutex::new(Vec::new());
        tee_output(
            Box::new(std::io::Cursor::new(input)),
            &log,
            seen,
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
}
