//! Launcher logger: stderr + `logs/launcher.log` + an in-memory ring buffer
//! shown on the Logs tab.

use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use arctic_core::launch::logparse::{Level, LogLine};

/// Lines kept in memory for the Logs tab.
pub const MAX_LINES: usize = 5_000;

const RETENTION_AGE: Duration = Duration::from_secs(14 * 24 * 60 * 60);
const RETENTION_BYTES: u64 = 50_000_000;

struct Logbook {
    start: Instant,
    /// (lines ever logged, recent lines)
    lines: Mutex<(u64, VecDeque<LogLine>)>,
    file: Option<Mutex<LogFile>>,
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
            && let Ok(mut file) = file.lock()
        {
            file.write_line(&text, SystemTime::now());
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
            && let Ok(f) = file.lock()
        {
            let _ = f.current.sync_data();
        }
    }
}

/// Install the logger and start a fresh `launcher.log` after archiving the previous run.
pub fn init(file: PathBuf) {
    let logbook = LOGBOOK.get_or_init(|| Logbook {
        start: Instant::now(),
        lines: Mutex::new((0, VecDeque::new())),
        file: match LogFile::start(&file, SystemTime::now()) {
            Ok(file) => Some(Mutex::new(file)),
            Err(e) => {
                eprintln!("could not start launcher log {}: {e}", file.display()); // i18n-ignore: diagnostics
                None
            }
        },
    });
    if log::set_logger(logbook).is_ok() {
        log::set_max_level(log::LevelFilter::Info);
    }
}

struct ArchivedLog {
    path: PathBuf,
    modified: SystemTime,
    bytes: u64,
}

struct LogFile {
    current: File,
    current_bytes: u64,
    archived_bytes: u64,
    archives: Vec<ArchivedLog>,
}

impl LogFile {
    fn start(file: &Path, now: SystemTime) -> io::Result<Self> {
        let dir = file.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(dir)?;
        let entries = fs::read_dir(dir)?;
        // The primary launcher owns this directory. A failed archive must never truncate the old log.
        archive(file)?;
        let current = File::create_new(file)?;
        for entry in entries.flatten() {
            let name = entry.file_name();
            let is_numbered = name
                .to_str()
                .and_then(|name| name.strip_prefix("launcher.")?.strip_suffix(".log"))
                .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
            if is_numbered && let Err(e) = archive(&entry.path()) {
                eprintln!(
                    "could not migrate launcher log {}: {e}", // i18n-ignore: diagnostics
                    entry.path().display()
                );
            }
        }
        let archives = archived_logs(dir);
        let mut file = Self {
            current,
            current_bytes: 0,
            archived_bytes: archives.iter().map(|log| log.bytes).sum(),
            archives,
        };
        file.prune(now);
        Ok(file)
    }

    fn write_line(&mut self, text: &str, now: SystemTime) {
        if writeln!(self.current, "{text}").is_ok() {
            self.current_bytes += text.len() as u64 + 1;
        } else if let Ok(metadata) = self.current.metadata() {
            self.current_bytes = metadata.len();
        }
        if !self.archives.is_empty() {
            self.prune(now);
        }
    }

    fn prune(&mut self, now: SystemTime) {
        let expired = |log: &ArchivedLog| {
            now.duration_since(log.modified)
                .is_ok_and(|age| age > RETENTION_AGE)
        };
        let over_limit = |archives| self.current_bytes.saturating_add(archives) > RETENTION_BYTES;
        if !over_limit(self.archived_bytes) && !self.archives.first().is_some_and(expired) {
            return;
        }
        // Oldest first, so expired files go before any size-based eviction.
        self.archives.retain(|log| {
            if (expired(log) || over_limit(self.archived_bytes))
                && fs::remove_file(&log.path)
                    .map_or_else(|e| e.kind() == ErrorKind::NotFound, |()| true)
            {
                self.archived_bytes -= log.bytes;
                false
            } else {
                true
            }
        });
    }
}

fn archive(path: &Path) -> io::Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => metadata,
        Ok(_) => {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                "expected a log file",
            ));
        }
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    let modified = metadata.modified()?;
    let seconds = match modified.duration_since(UNIX_EPOCH) {
        Ok(since) => since.as_secs() as i64,
        Err(before) => {
            let before = before.duration();
            -(before.as_secs() as i64) - i64::from(before.subsec_nanos() != 0)
        }
    };
    let (year, month, day) = civil_date(seconds.div_euclid(86_400));
    let seconds = seconds.rem_euclid(86_400);
    let stamp = format!(
        "launcher-{year:04}{month:02}{day:02}-{:02}{:02}{:02}",
        seconds / 3600,
        seconds % 3600 / 60,
        seconds % 60
    );
    let mut n = 0;
    loop {
        let name = if n == 0 {
            format!("{stamp}.log")
        } else {
            format!("{stamp}-{n}.log")
        };
        let destination = path.with_file_name(name);
        match fs::symlink_metadata(&destination) {
            Err(e) if e.kind() == ErrorKind::NotFound => return fs::rename(path, destination),
            Err(e) => return Err(e),
            Ok(_) => n += 1,
        }
    }
}

/// Show the UTC archive timestamp and, when present, the collision suffix.
pub fn archive_label(path: &Path) -> Option<String> {
    let timestamp = path
        .file_name()?
        .to_str()?
        .strip_prefix("launcher-")?
        .strip_suffix(".log")?;
    let mut parts = timestamp.splitn(3, '-');
    let date = parts.next()?;
    let time = parts.next()?;
    let suffix = parts.next();
    let digits = |part: &str| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit());
    if date.len() != 8
        || time.len() != 6
        || !digits(date)
        || !digits(time)
        || suffix.is_some_and(|part| !digits(part))
    {
        return None;
    }
    let suffix = suffix.map_or_else(String::new, |n| format!(" ({n})"));
    Some(format!(
        "{}-{}-{} {}:{}:{} UTC{suffix}", // i18n-ignore: archive timestamp
        &date[..4],
        &date[4..6],
        &date[6..],
        &time[..2],
        &time[2..4],
        &time[4..]
    ))
}

fn archived_logs(dir: &Path) -> Vec<ArchivedLog> {
    let mut logs: Vec<_> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            archive_label(&path)?;
            if !entry.file_type().ok()?.is_file() {
                return None;
            }
            let metadata = entry.metadata().ok()?;
            Some(ArchivedLog {
                path,
                modified: metadata.modified().ok()?,
                bytes: metadata.len(),
            })
        })
        .collect();
    logs.sort_unstable_by(|a, b| {
        a.modified
            .cmp(&b.modified)
            .then_with(|| a.path.cmp(&b.path))
    });
    logs
}

/// Retained launcher logs, current first when present, then newest archived run first.
pub fn retained_log_paths(dir: &Path) -> Vec<PathBuf> {
    let current = dir.join("launcher.log");
    current
        .is_file()
        .then_some(current)
        .into_iter()
        .chain(archived_logs(dir).into_iter().rev().map(|log| log.path))
        .collect()
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

    fn test_time() -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(30 * 24 * 60 * 60)
    }

    fn fixture(path: &Path, contents: &str, bytes: u64, modified: SystemTime) {
        let mut file = File::create(path).unwrap();
        file.write_all(contents.as_bytes()).unwrap();
        file.set_len(bytes).unwrap();
        file.set_modified(modified).unwrap();
    }

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
    fn each_restart_archives_the_previous_log_without_overwriting_collisions() {
        let dir = tempfile::tempdir().unwrap();
        let current = dir.path().join("launcher.log");
        let now = test_time();
        let previous = now - Duration::from_secs(1);
        for run in 0..7 {
            let mut file = LogFile::start(&current, now).unwrap();
            assert_eq!(fs::metadata(&current).unwrap().len(), 0);
            file.write_line(&format!("run {run}"), now);
            file.current.set_modified(previous).unwrap();
        }
        let paths = retained_log_paths(dir.path());
        assert_eq!(paths.len(), 7);
        assert_eq!(paths[0], current);
        assert_eq!(fs::read_to_string(&current).unwrap(), "run 6\n");
        let first = dir.path().join("launcher-19700130-235959.log");
        let second = dir.path().join("launcher-19700130-235959-1.log");
        assert_eq!(fs::read_to_string(&first).unwrap(), "run 0\n");
        assert_eq!(fs::read_to_string(&second).unwrap(), "run 1\n");
        assert_eq!(fs::metadata(first).unwrap().modified().unwrap(), previous);
    }

    #[test]
    fn numbered_logs_are_migrated_and_expired_by_their_original_mtimes() {
        let dir = tempfile::tempdir().unwrap();
        let current = dir.path().join("launcher.log");
        let now = test_time();
        let cutoff = now - RETENTION_AGE;
        let second = Duration::from_secs(1);
        for (n, modified) in [
            (1, cutoff + second),
            (2, cutoff),
            (3, cutoff - second),
            (4, cutoff - second * 2),
        ] {
            fixture(
                &dir.path().join(format!("launcher.{n}.log")),
                &n.to_string(),
                1,
                modified,
            );
        }
        let unrelated = dir.path().join("launcher-backup.log");
        fixture(&unrelated, "backup", 6, cutoff - second);
        let _file = LogFile::start(&current, now).unwrap();
        for n in 1..=4 {
            assert!(!dir.path().join(format!("launcher.{n}.log")).exists());
        }
        let paths = retained_log_paths(dir.path());
        assert_eq!(paths.len(), 3);
        assert_eq!(paths[0], current);
        assert_eq!(fs::read_to_string(&paths[1]).unwrap(), "1");
        assert_eq!(fs::read_to_string(&paths[2]).unwrap(), "2");
        assert_eq!(
            fs::metadata(&paths[1]).unwrap().modified().unwrap(),
            cutoff + second
        );
        assert!(unrelated.is_file());
    }

    #[test]
    fn startup_applies_age_and_size_pruning_in_one_pass() {
        let dir = tempfile::tempdir().unwrap();
        let current = dir.path().join("launcher.log");
        let now = test_time();
        let second = Duration::from_secs(1);
        let expired = dir.path().join("launcher-19700101-000000.log");
        let oldest = dir.path().join("launcher-19700130-235957.log");
        let newer = dir.path().join("launcher-19700130-235958.log");
        let newest = dir.path().join("launcher-19700130-235959.log");
        fixture(&expired, "", RETENTION_BYTES, UNIX_EPOCH);
        fixture(&oldest, "", 2, now - second * 3);
        fixture(&newer, "", RETENTION_BYTES - 1, now - second * 2);
        fixture(&newest, "", 1, now - second);
        let _file = LogFile::start(&current, now).unwrap();
        assert!(!expired.exists());
        assert!(!oldest.exists());
        assert_eq!(retained_log_paths(dir.path()), [current, newest, newer]);
    }

    #[test]
    fn growth_prunes_oldest_archives_and_keeps_an_oversized_current_log() {
        let dir = tempfile::tempdir().unwrap();
        let current = dir.path().join("launcher.log");
        let now = test_time();
        let oldest = dir.path().join("launcher-19700130-235958.log");
        let newest = dir.path().join("launcher-19700130-235959.log");
        fixture(&oldest, "", 2, now - Duration::from_secs(2));
        fixture(
            &newest,
            "",
            RETENTION_BYTES - 2,
            now - Duration::from_secs(1),
        );
        let mut file = LogFile::start(&current, now).unwrap();
        file.write_line("x", now);
        assert!(!oldest.exists());
        assert!(newest.exists());

        file.current.set_len(RETENTION_BYTES + 1).unwrap();
        file.current_bytes = file.current.metadata().unwrap().len();
        file.prune(now);
        assert!(!newest.exists());
        assert_eq!(fs::metadata(&current).unwrap().len(), RETENTION_BYTES + 1);
        assert_eq!(retained_log_paths(dir.path()), [current]);
    }
}
