//! Running the game: spawn, tee output to the log file, detect when the
//! window is up, and report the exit, all on background threads so the
//! launcher UI never waits on the game.

use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
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
    let mut child = Command::new(&plan.java)
        .args(&plan.args)
        .current_dir(&plan.game_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .at(&plan.java)?;

    let on_event = Arc::new(on_event);
    let window_seen = Arc::new(AtomicBool::new(false));
    let tee = |stream: Box<dyn Read + Send>| {
        let (log, seen, on_event) = (log.clone(), window_seen.clone(), on_event.clone());
        std::thread::spawn(move || tee_output(stream, &log, &seen, &*on_event));
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
    let child = Command::new(&plan.java)
        .args(&plan.args)
        .current_dir(&plan.game_dir)
        .stdin(Stdio::null())
        .stdout(log)
        .stderr(log_err)
        .spawn()
        .at(&plan.java)?;
    Ok(child.id())
}

/// Copy a child stream into the log line by line, firing `WindowReady` the
/// first time a marker shows up (on either stream).
fn tee_output(
    stream: Box<dyn Read + Send>,
    log: &Mutex<File>,
    window_seen: &AtomicBool,
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

fn is_window_marker(line: &[u8]) -> bool {
    let text = String::from_utf8_lossy(line);
    WINDOW_MARKERS.iter().any(|m| text.contains(m))
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
