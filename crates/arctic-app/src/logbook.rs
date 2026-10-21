//! Launcher logger: stderr + `logs/launcher.log` + an in-memory ring buffer
//! shown on the Logs tab.

use std::collections::VecDeque;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
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

/// Log files kept: this run's and the runs before it (`launcher.1.log` is the last run's).
const KEPT_LOGS: usize = 5;

/// Install the logger. The log of the run before is kept (see [`KEPT_LOGS`]); `file` starts empty.
pub fn init(file: PathBuf) {
    let file = file
        .parent()
        .map(std::fs::create_dir_all)
        .inspect(|_| rotate(&file))
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

/// `launcher.log` becomes `launcher.1.log`, that one `launcher.2.log`, and so on; the oldest goes.
fn rotate(file: &Path) {
    let numbered = |n: usize| file.with_file_name(format!("launcher.{n}.log"));
    let _ = std::fs::remove_file(numbered(KEPT_LOGS - 1));
    for n in (1..KEPT_LOGS - 1).rev() {
        let _ = std::fs::rename(numbered(n), numbered(n + 1));
    }
    let _ = std::fs::rename(file, numbered(1));
}

/// "2026-10-21 17:59:53 (UTC+03:00)" for now on this computer's clock; UTC when the zone isn't known.
pub fn started_at() -> String {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64);
    let utc = now_ms.div_euclid(1000);
    let local_of_day = arctic_core::system::local_clock(now_ms).and_then(|clock| {
        let mut parts = clock.split(':').filter_map(|p| p.parse::<i64>().ok());
        Some(parts.next()? * 3600 + parts.next()? * 60 + parts.next()?)
    });
    // The zone's offset is what the local time of day differs from UTC's by, within -12 h to +14 h.
    let offset = local_of_day.map_or(0, |local| {
        let mut diff = (local - utc.rem_euclid(86_400)).rem_euclid(86_400);
        if diff > 14 * 3600 {
            diff -= 86_400;
        }
        diff
    });
    let (year, month, day) = civil_date((utc + offset).div_euclid(86_400));
    let secs = (utc + offset).rem_euclid(86_400);
    let sign = if offset < 0 { '-' } else { '+' };
    let off = offset.abs();
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02} (UTC{sign}{:02}:{:02})",
        secs / 3600,
        secs % 3600 / 60,
        secs % 60,
        off / 3600,
        off % 3600 / 60
    )
}

/// (year, month, day) of a day count since 1970-01-01.
fn civil_date(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// Copy of the buffered launcher lines if anything was logged since
/// `seen` (a count previously returned here); `None` when unchanged.
pub fn snapshot_if_changed(seen: u64) -> Option<(u64, Vec<LogLine>)> {
    let guard = LOGBOOK.get()?.lines.lock().ok()?;
    let (total, lines) = &*guard;
    (*total != seen).then(|| (*total, lines.iter().cloned().collect()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_become_dates() {
        assert_eq!(civil_date(0), (1970, 1, 1));
        assert_eq!(civil_date(-1), (1969, 12, 31));
        assert_eq!(civil_date(11_016), (2000, 2, 29));
        assert_eq!(civil_date(20_747), (2026, 10, 21));
    }

    #[test]
    fn the_start_line_has_a_date_and_a_zone() {
        let text = started_at();
        assert_eq!(text.len(), "2026-10-21 17:59:53 (UTC+03:00)".len());
        assert!(text.contains("(UTC"));
    }

    #[test]
    fn the_last_few_logs_are_kept() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("launcher.log");
        for run in 1..=7 {
            rotate(&file);
            std::fs::write(&file, format!("run {run}")).unwrap();
        }
        let read = |name: &str| std::fs::read_to_string(dir.path().join(name)).ok();
        assert_eq!(read("launcher.log").as_deref(), Some("run 7"));
        assert_eq!(read("launcher.1.log").as_deref(), Some("run 6"));
        assert_eq!(read("launcher.3.log").as_deref(), Some("run 4"));
        assert_eq!(read("launcher.4.log").as_deref(), Some("run 3"));
        assert_eq!(read("launcher.5.log"), None);
    }
}
