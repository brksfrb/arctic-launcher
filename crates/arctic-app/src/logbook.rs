//! Launcher logger: stderr + `logs/launcher.log` + an in-memory ring buffer
//! shown on the Logs tab.

use std::collections::VecDeque;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use arctic_core::launch::logparse::{Level, LogLine};

/// Lines kept in memory for the Logs tab.
pub const MAX_LINES: usize = 5_000;

struct Logbook {
    start: Instant,
    /// (lines ever logged, recent lines)
    lines: Mutex<(u64, VecDeque<LogLine>)>,
    file: Option<Mutex<File>>,
}

static LOGBOOK: OnceLock<Logbook> = OnceLock::new();

impl log::Log for Logbook {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        // Our own crates at info+, dependencies only when they warn.
        let ours = metadata.target().starts_with("arctic");
        metadata.level()
            <= if ours {
                log::Level::Info
            } else {
                log::Level::Warn
            }
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let secs = self.start.elapsed().as_secs_f64();
        let text = format!("[{secs:>8.2}s] [{}] {}", record.level(), record.args());
        eprintln!("{text}");
        if let Some(file) = &self.file
            && let Ok(mut f) = file.lock()
        {
            let _ = writeln!(f, "{text}");
        }
        if let Ok(mut guard) = self.lines.lock() {
            let (total, lines) = &mut *guard;
            *total += 1;
            if lines.len() >= MAX_LINES {
                lines.pop_front();
            }
            lines.push_back(LogLine {
                level: match record.level() {
                    log::Level::Error => Level::Error,
                    log::Level::Warn => Level::Warn,
                    log::Level::Info => Level::Info,
                    _ => Level::Debug,
                },
                text,
            });
        }
    }

    fn flush(&self) {
        if let Some(file) = &self.file
            && let Ok(mut f) = file.lock()
        {
            let _ = f.flush();
        }
    }
}

/// Install the logger. `file` is truncated on each start.
pub fn init(file: PathBuf) {
    let file = file
        .parent()
        .map(std::fs::create_dir_all)
        .and_then(|_| File::create(&file).ok())
        .map(Mutex::new);
    let logbook = LOGBOOK.get_or_init(|| Logbook {
        start: Instant::now(),
        lines: Mutex::new((0, VecDeque::new())),
        file,
    });
    if log::set_logger(logbook).is_ok() {
        log::set_max_level(log::LevelFilter::Info);
    }
}

/// Copy of the buffered launcher lines if anything was logged since
/// `seen` (a count previously returned here); `None` when unchanged.
pub fn snapshot_if_changed(seen: u64) -> Option<(u64, Vec<LogLine>)> {
    let guard = LOGBOOK.get()?.lines.lock().ok()?;
    let (total, lines) = &*guard;
    (*total != seen).then(|| (*total, lines.iter().cloned().collect()))
}
