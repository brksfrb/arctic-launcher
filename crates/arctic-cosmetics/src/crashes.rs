//! Crash reports players chose to send: kept so the same crash seen by
//! many people shows up as one group with a count. A group is open until a
//! moderator marks it resolved; it opens again if the crash comes back from
//! the launcher version with the fix (or a newer one).

use std::cmp::Ordering;

use rusqlite::types::Value;
use rusqlite::{OptionalExtension, params, params_from_iter};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::filters::{compare_versions, is_version, newest_version, sort_newest_first};
use crate::store::Store;

/// Longest log text kept per report (bytes).
pub const MAX_LOG_BYTES: usize = 48 * 1024;
/// Reports kept in all; the oldest go first.
const KEEP: i64 = 2000;
/// Longest field other than the log.
const MAX_FIELD: usize = 120;
/// Longest error line shown in a list.
const MAX_LINE: usize = 300;
/// Reports of one group listed with its detail.
const GROUP_REPORTS: i64 = 100;

/// What the launcher sends.
pub struct Report<'a> {
    pub launcher: &'a str,
    pub game: &'a str,
    pub loader: &'a str,
    pub os: &'a str,
    pub title: &'a str,
    pub log: &'a str,
    /// How the player said they can be reached (a Discord name, say), or empty.
    pub contact: &'a str,
}

/// One group of the same crash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Group {
    /// The newest report of the group: open it for the full text.
    pub id: i64,
    pub fingerprint: String,
    /// The launcher's explanation, or "Crash" when it had none.
    pub title: String,
    pub game: String,
    pub loader: String,
    /// Reports matching the filter.
    pub count: i64,
    pub first: i64,
    pub last: i64,
    /// The launcher versions it came from, newest first, and the systems.
    pub launchers: Vec<String>,
    pub oses: Vec<String>,
    #[serde(flatten)]
    pub status: Status,
    #[serde(flatten)]
    pub summary: Summary,
}

/// Open or resolved, as a moderator set it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Status {
    /// `open` or `resolved`.
    pub status: String,
    pub resolved_at: Option<i64>,
    pub resolved_by: Option<String>,
    /// The moderator's note ("fixed in 0.3.5").
    pub note: Option<String>,
    /// The launcher version with the fix: the crash coming back from this
    /// version or a newer one opens the group again. Without one, a launcher
    /// newer than any that had sent it when it was resolved does.
    pub fixed_in: Option<String>,
}

/// The lines that say what went wrong, read from a report's log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Summary {
    /// The launcher's diagnosis (its title), when it recognised the crash.
    pub diagnosis: Option<String>,
    /// The first line naming an exception or error.
    pub first_error: Option<String>,
    /// The last `Caused by:` line, the deepest cause.
    pub root_cause: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Detail {
    pub id: i64,
    pub at: i64,
    pub launcher: String,
    pub game: String,
    pub loader: String,
    pub os: String,
    pub title: String,
    pub fingerprint: String,
    #[serde(flatten)]
    pub status: Status,
    #[serde(flatten)]
    pub summary: Summary,
    /// Every report of the same crash, newest first (without their logs).
    pub reports: Vec<ReportRow>,
    pub log: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReportRow {
    pub id: i64,
    pub at: i64,
    pub launcher: String,
    pub game: String,
    pub loader: String,
    pub os: String,
    /// How the sender can be reached, if they said.
    pub contact: String,
}

/// What a crash list is narrowed to; `None` doesn't narrow.
#[derive(Debug, Clone, Default)]
pub struct Filter {
    /// `open` or `resolved`.
    pub status: Option<String>,
    pub os: Option<String>,
    pub launcher: Option<String>,
    pub game: Option<String>,
    /// Reports from this Unix time on.
    pub since: Option<i64>,
}

/// How a moderator marks a group resolved.
pub struct Resolution<'a> {
    pub by: &'a str,
    pub note: &'a str,
    /// Empty: not known, so only a launcher newer than any seen so far reopens it.
    pub fixed_in: &'a str,
}

/// Printable, one line, not too long.
pub fn clean_field(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control())
        .take(MAX_FIELD)
        .collect::<String>()
        .trim()
        .to_owned()
}

/// The text cut to `MAX_LOG_BYTES` (the end, where crashes are), on a character boundary.
pub fn clip_log(text: &str) -> &str {
    if text.len() <= MAX_LOG_BYTES {
        return text;
    }
    let mut start = text.len() - MAX_LOG_BYTES;
    while !text.is_char_boundary(start) {
        start += 1;
    }
    &text[start..]
}

/// A line naming a thrown exception or error class (`java.lang.FooException: ...`,
/// `Caused by: a.b.BarError`) or starting with `Error:`. Not a crash report's
/// `Suppressed Exceptions: ~~NONE~~` header, which would group unrelated crashes.
fn names_an_error(line: &str) -> bool {
    let thrown = line.split_whitespace().any(|word| {
        let name = word.trim_end_matches(':');
        name.contains('.') && (name.ends_with("Exception") || name.ends_with("Error"))
    });
    thrown || line.trim_start().starts_with("Error:")
}

/// Errors every session with a failed sign-in or no Realms access prints (an offline account, a token
/// that expired, a blocked Realms call): they say nothing about why a game crashed, and if they count as
/// "the first error", unrelated crashes land in one group.
fn background_noise(line: &str) -> bool {
    [
        "InvalidCredentialsException",
        "MinecraftClientHttpException",
        "Realms authentication",
        "Could not authorize you against Realms",
        "SignedJWT",
        "AuthenticationUnavailableException",
    ]
    .iter()
    .any(|noise| line.contains(noise))
}

/// A line that names an exception worth grouping by.
fn names_a_cause(line: &str) -> bool {
    names_an_error(line) && !background_noise(line)
}

/// Groups reports of one crash: the game version plus the first line that names an exception.
fn fingerprint(game: &str, title: &str, log: &str) -> String {
    let cause = log.lines().find(|l| names_a_cause(l)).unwrap_or(title);
    let digits_gone: String = cause.chars().filter(|c| !c.is_ascii_digit()).collect();
    let digest = Sha256::digest(format!("{game}|{digits_gone}").as_bytes());
    digest.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

/// A JVM's own fatal error header (`# Native memory allocation (mmap) failed...`).
fn jvm_fatal(line: &str) -> bool {
    let line = line.trim_start();
    line.starts_with("# ")
        && [
            "failed",
            "insufficient",
            "SIGSEGV",
            "EXCEPTION_ACCESS_VIOLATION",
        ]
        .iter()
        .any(|w| line.contains(w))
}

fn short_line(line: &str) -> String {
    line.trim().chars().take(MAX_LINE).collect()
}

/// What went wrong, in a few lines a person or an AI can read without the log.
pub fn summarize(title: &str, log: &str) -> Summary {
    let diagnosis = (!title.is_empty() && title != "Crash").then(|| title.to_owned());
    let first_error = log
        .lines()
        .find(|l| names_a_cause(l) || jvm_fatal(l))
        .map(short_line);
    let root_cause = log
        .lines()
        .rev()
        .find(|l| l.trim_start().starts_with("Caused by:"))
        .map(short_line);
    Summary {
        diagnosis,
        first_error,
        root_cause,
    }
}

/// The status columns (`status, resolved_at, resolved_by, note, fixed_in`)
/// from `start`; no row means open.
fn status_at(r: &rusqlite::Row, start: usize) -> rusqlite::Result<Status> {
    let status: Option<String> = r.get(start)?;
    if status.as_deref() != Some("resolved") {
        return Ok(Status {
            status: "open".into(),
            resolved_at: None,
            resolved_by: None,
            note: None,
            fixed_in: None,
        });
    }
    Ok(Status {
        status: "resolved".into(),
        resolved_at: r.get(start + 1)?,
        resolved_by: r.get(start + 2)?,
        note: r.get(start + 3)?,
        fixed_in: r.get(start + 4)?,
    })
}

/// A `GROUP_CONCAT` as a list; versions sorted newest first.
fn split_list(joined: Option<String>, versions: bool) -> Vec<String> {
    let mut items: Vec<String> = joined
        .unwrap_or_default()
        .split('\n')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    if versions {
        sort_newest_first(&mut items);
    }
    items
}

/// `WHERE ...` for a filter, with its values numbered from `?1`.
fn filter_sql(filter: &Filter) -> (String, Vec<Value>) {
    let mut clauses = Vec::new();
    let mut values = Vec::new();
    let mut add = |clause: &str, value: Value| {
        values.push(value);
        clauses.push(clause.replace('?', &format!("?{}", values.len())));
    };
    if let Some(status) = &filter.status {
        add(
            "COALESCE(s.status, 'open') = ?",
            Value::Text(status.clone()),
        );
    }
    if let Some(os) = &filter.os {
        add("c.os = ? COLLATE NOCASE", Value::Text(os.clone()));
    }
    if let Some(launcher) = &filter.launcher {
        add("c.launcher = ?", Value::Text(launcher.clone()));
    }
    if let Some(game) = &filter.game {
        add("c.game = ?", Value::Text(game.clone()));
    }
    if let Some(since) = filter.since {
        add("c.at >= ?", Value::Integer(since));
    }
    if clauses.is_empty() {
        return (String::new(), values);
    }
    (format!("WHERE {}", clauses.join(" AND ")), values)
}

type GroupRow = (i64, String, i64, i64, i64, Vec<String>, Vec<String>, Status);

impl Store {
    pub(crate) fn migrate_crashes(&self) -> rusqlite::Result<()> {
        self.conn().execute_batch(
            "CREATE TABLE IF NOT EXISTS crashes (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 at INTEGER NOT NULL,
                 launcher TEXT NOT NULL,
                 game TEXT NOT NULL,
                 loader TEXT NOT NULL,
                 os TEXT NOT NULL,
                 title TEXT NOT NULL,
                 log TEXT NOT NULL,
                 fingerprint TEXT NOT NULL,
                 contact TEXT NOT NULL DEFAULT ''
             );
             CREATE INDEX IF NOT EXISTS crashes_fingerprint ON crashes (fingerprint, at);
             CREATE TABLE IF NOT EXISTS crash_status (
                 fingerprint TEXT PRIMARY KEY,
                 status TEXT NOT NULL,
                 resolved_at INTEGER,
                 resolved_by TEXT,
                 note TEXT,
                 fixed_in TEXT,
                 newest_seen TEXT
             );",
        )?;
        // Tables made before reports had a contact get the column (the second time it already exists).
        let _ = self.conn().execute(
            "ALTER TABLE crashes ADD COLUMN contact TEXT NOT NULL DEFAULT ''",
            [],
        );
        Ok(())
    }
    /// Is report `id` worth a notification? A new crash (first report of its kind) or a
    /// spike of one still open (the 5th, 25th and 100th report). Repeats of a known crash,
    /// and crashes already closed, stay quiet: one player retrying must not buzz every time.
    pub fn crash_notify_worthy(&self, id: i64) -> rusqlite::Result<bool> {
        let conn = self.conn();
        let print: String =
            conn.query_row("SELECT fingerprint FROM crashes WHERE id = ?1", [id], |r| {
                r.get(0)
            })?;
        let closed: bool = conn
            .query_row(
                "SELECT 1 FROM crash_status WHERE fingerprint = ?1 AND status = 'resolved'",
                [&print],
                |_| Ok(true),
            )
            .optional()?
            .unwrap_or(false);
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM crashes WHERE fingerprint = ?1",
            [&print],
            |r| r.get(0),
        )?;
        Ok(!closed && matches!(count, 1 | 5 | 25 | 100))
    }

    /// Keep a report; returns its number.
    pub fn crash_add(&self, report: &Report, now: u64) -> rusqlite::Result<i64> {
        let log = clip_log(report.log);
        let print = fingerprint(report.game, report.title, log);
        let conn = self.conn();
        conn.execute(
            "INSERT INTO crashes (at, launcher, game, loader, os, title, log, fingerprint, contact)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                now as i64,
                report.launcher,
                report.game,
                report.loader,
                report.os,
                report.title,
                log,
                print,
                report.contact
            ],
        )?;
        let id = conn.last_insert_rowid();
        let resolved: Option<(Option<String>, Option<String>)> = conn
            .query_row(
                "SELECT fixed_in, newest_seen FROM crash_status
                 WHERE fingerprint = ?1 AND status = 'resolved'",
                [&print],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        // Back on a launcher that has the fix (or, with no fix version given,
        // one newer than any seen when it was resolved, or any real version
        // when only development builds had sent it): open again. An older
        // launcher crashing the same way doesn't say the fix failed.
        let back = resolved.is_some_and(|(fixed_in, newest_seen)| {
            let launcher = report.launcher;
            match (fixed_in, newest_seen) {
                (Some(fix), _) => compare_versions(launcher, &fix).is_some_and(Ordering::is_ge),
                (None, Some(seen)) => {
                    compare_versions(launcher, &seen).is_some_and(Ordering::is_gt)
                }
                (None, None) => is_version(launcher),
            }
        });
        if back {
            conn.execute("DELETE FROM crash_status WHERE fingerprint = ?1", [&print])?;
        }
        conn.execute(
            "DELETE FROM crashes WHERE id NOT IN (SELECT id FROM crashes ORDER BY id DESC LIMIT ?1)",
            [KEEP],
        )?;
        conn.execute(
            "DELETE FROM crash_status WHERE fingerprint NOT IN (SELECT fingerprint FROM crashes)",
            [],
        )?;
        Ok(id)
    }

    /// The crashes seen, grouped, the most recent first. Only reports that
    /// match the filter are counted.
    pub fn crash_groups(&self, filter: &Filter, limit: usize) -> rusqlite::Result<Vec<Group>> {
        let (narrow, mut values) = filter_sql(filter);
        values.push(Value::Integer(limit as i64));
        // GROUP_CONCAT(DISTINCT x) only joins with commas: those become line
        // breaks, which the fields can't hold (`clean_field` drops them).
        let sql = format!(
            "SELECT MAX(c.id), c.fingerprint, COUNT(*), MIN(c.at), MAX(c.at),
                    REPLACE(GROUP_CONCAT(DISTINCT REPLACE(c.launcher, ',', ' ')), ',', char(10)),
                    REPLACE(GROUP_CONCAT(DISTINCT REPLACE(c.os, ',', ' ')), ',', char(10)),
                    s.status, s.resolved_at, s.resolved_by, s.note, s.fixed_in
             FROM crashes c LEFT JOIN crash_status s ON s.fingerprint = c.fingerprint
             {narrow}
             GROUP BY c.fingerprint ORDER BY MAX(c.at) DESC, MAX(c.id) DESC LIMIT ?{}",
            values.len()
        );
        let conn = self.conn();
        let rows: Vec<GroupRow> = conn
            .prepare(&sql)?
            .query_map(params_from_iter(values), |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    split_list(r.get(5)?, true),
                    split_list(r.get(6)?, false),
                    status_at(r, 7)?,
                ))
            })?
            .collect::<rusqlite::Result<_>>()?;
        let mut newest =
            conn.prepare("SELECT title, game, loader, log FROM crashes WHERE id = ?1")?;
        rows.into_iter()
            .map(
                |(id, fingerprint, count, first, last, launchers, oses, status)| {
                    let (title, game, loader, log): (String, String, String, String) = newest
                        .query_row([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
                    Ok(Group {
                        id,
                        fingerprint,
                        summary: summarize(&title, &log),
                        title,
                        game,
                        loader,
                        count,
                        first,
                        last,
                        launchers,
                        oses,
                        status,
                    })
                },
            )
            .collect()
    }

    pub fn crash_detail(&self, id: i64) -> rusqlite::Result<Option<Detail>> {
        let conn = self.conn();
        let found = conn
            .query_row(
                "SELECT c.id, c.at, c.launcher, c.game, c.loader, c.os, c.title, c.log, c.fingerprint,
                        s.status, s.resolved_at, s.resolved_by, s.note, s.fixed_in
                 FROM crashes c LEFT JOIN crash_status s ON s.fingerprint = c.fingerprint
                 WHERE c.id = ?1",
                [id],
                |r| {
                    let (title, log): (String, String) = (r.get(6)?, r.get(7)?);
                    Ok(Detail {
                        id: r.get(0)?,
                        at: r.get(1)?,
                        launcher: r.get(2)?,
                        game: r.get(3)?,
                        loader: r.get(4)?,
                        os: r.get(5)?,
                        summary: summarize(&title, &log),
                        title,
                        log,
                        fingerprint: r.get(8)?,
                        status: status_at(r, 9)?,
                        reports: Vec::new(),
                    })
                },
            )
            .optional()?;
        let Some(mut detail) = found else {
            return Ok(None);
        };
        detail.reports = conn
            .prepare(
                "SELECT id, at, launcher, game, loader, os, contact FROM crashes
                 WHERE fingerprint = ?1 ORDER BY id DESC LIMIT ?2",
            )?
            .query_map(params![detail.fingerprint, GROUP_REPORTS], |r| {
                Ok(ReportRow {
                    id: r.get(0)?,
                    at: r.get(1)?,
                    launcher: r.get(2)?,
                    game: r.get(3)?,
                    loader: r.get(4)?,
                    os: r.get(5)?,
                    contact: r.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(Some(detail))
    }

    fn crash_fingerprint(&self, id: i64) -> rusqlite::Result<Option<String>> {
        self.conn()
            .query_row("SELECT fingerprint FROM crashes WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .optional()
    }

    /// Mark the group of this report resolved, or open again (`None`).
    pub fn crash_set_status(
        &self,
        id: i64,
        resolution: Option<&Resolution>,
        now: u64,
    ) -> rusqlite::Result<bool> {
        let Some(print) = self.crash_fingerprint(id)? else {
            return Ok(false);
        };
        let conn = self.conn();
        let Some(resolution) = resolution else {
            conn.execute("DELETE FROM crash_status WHERE fingerprint = ?1", [&print])?;
            return Ok(true);
        };
        let seen = conn
            .prepare("SELECT DISTINCT launcher FROM crashes WHERE fingerprint = ?1")?
            .query_map([&print], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let newest_seen = newest_version(seen.iter().map(String::as_str));
        let fixed_in = Some(clean_field(resolution.fixed_in)).filter(|v| !v.is_empty());
        conn.execute(
            "INSERT INTO crash_status
                 (fingerprint, status, resolved_at, resolved_by, note, fixed_in, newest_seen)
             VALUES (?1, 'resolved', ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (fingerprint) DO UPDATE SET status = 'resolved', resolved_at = ?2,
                 resolved_by = ?3, note = ?4, fixed_in = ?5, newest_seen = ?6",
            params![
                print,
                now as i64,
                resolution.by,
                clean_field(resolution.note),
                fixed_in,
                newest_seen
            ],
        )?;
        Ok(true)
    }

    /// Forget every report of the same crash as this one.
    pub fn crash_delete_group(&self, id: i64) -> rusqlite::Result<bool> {
        let Some(print) = self.crash_fingerprint(id)? else {
            return Ok(false);
        };
        let conn = self.conn();
        conn.execute("DELETE FROM crashes WHERE fingerprint = ?1", [&print])?;
        conn.execute("DELETE FROM crash_status WHERE fingerprint = ?1", [&print])?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report<'a>(game: &'a str, log: &'a str) -> Report<'a> {
        Report {
            launcher: "0.2.1",
            game,
            loader: "fabric",
            os: "windows",
            title: "Crash",
            log,
            contact: "",
        }
    }

    fn all(s: &Store) -> Vec<Group> {
        s.crash_groups(&Filter::default(), 10).unwrap()
    }

    #[test]
    fn the_same_crash_groups_and_counts() {
        let s = Store::memory().unwrap();
        let a = "java.lang.NullPointerException at x.y(Z.java:12)";
        let b = "java.lang.NullPointerException at x.y(Z.java:99)";
        s.crash_add(&report("1.21.4", a), 10).unwrap();
        s.crash_add(&report("1.21.4", b), 20).unwrap();
        s.crash_add(&report("1.20.1", a), 30).unwrap();
        let groups = all(&s);
        assert_eq!(groups.len(), 2);
        assert_eq!((groups[0].game.as_str(), groups[0].count), ("1.20.1", 1));
        assert_eq!((groups[1].game.as_str(), groups[1].count), ("1.21.4", 2));
        assert_eq!((groups[1].first, groups[1].last), (10, 20));
        let detail = s.crash_detail(groups[1].id).unwrap().unwrap();
        assert_eq!(detail.log, b);
        assert_eq!(detail.reports.len(), 2);
        assert!(s.crash_delete_group(groups[1].id).unwrap());
        assert_eq!(all(&s).len(), 1);
    }

    #[test]
    fn lists_say_what_went_wrong() {
        let log = "[main/INFO] hi\njava.lang.RuntimeException: boom\n\tat a.b\nCaused by: x.Y: mid\nCaused by: x.Z: deep";
        let summary = summarize("A mod is for a different version", log);
        assert_eq!(
            summary.diagnosis.as_deref(),
            Some("A mod is for a different version")
        );
        assert_eq!(
            summary.first_error.as_deref(),
            Some("java.lang.RuntimeException: boom")
        );
        assert_eq!(summary.root_cause.as_deref(), Some("Caused by: x.Z: deep"));
        let header = summarize(
            "Crash",
            "\tSuppressed Exceptions: ~~NONE~~\njava.lang.IllegalStateException: No GLCapabilities",
        );
        assert_eq!(
            header.first_error.as_deref(),
            Some("java.lang.IllegalStateException: No GLCapabilities")
        );
        let jvm = summarize(
            "Crash",
            "#\n# Native memory allocation (mmap) failed to map 1 bytes",
        );
        assert_eq!(jvm.diagnosis, None);
        assert_eq!(
            jvm.first_error.as_deref(),
            Some("# Native memory allocation (mmap) failed to map 1 bytes")
        );
    }

    #[test]
    fn filters_narrow_the_reports_counted() {
        let s = Store::memory().unwrap();
        let log = "java.lang.IllegalStateException: x";
        let mut linux = report("1.21.11", log);
        linux.os = "linux";
        linux.launcher = "0.3.4";
        s.crash_add(&report("1.21.11", log), 10).unwrap();
        s.crash_add(&linux, 20).unwrap();
        s.crash_add(&report("1.8.9", log), 30).unwrap();
        let only = |f: Filter| s.crash_groups(&f, 10).unwrap();
        let groups = only(Filter {
            os: Some("Linux".into()),
            ..Filter::default()
        });
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].count, 1);
        let both = &all(&s)[1];
        assert_eq!(both.launchers, ["0.3.4", "0.2.1"]);
        assert_eq!(both.oses.len(), 2);
        assert_eq!(
            only(Filter {
                game: Some("1.21.11".into()),
                launcher: Some("0.2.1".into()),
                ..Filter::default()
            })[0]
                .count,
            1
        );
        assert_eq!(
            only(Filter {
                since: Some(25),
                ..Filter::default()
            })
            .len(),
            1
        );
    }

    #[test]
    fn resolved_crashes_reopen_only_from_a_launcher_with_the_fix() {
        let s = Store::memory().unwrap();
        let log = "java.lang.OutOfMemoryError: Out of stack space.";
        let on = |launcher: &'static str, at: u64| {
            let mut r = report("1.21.11", log);
            r.launcher = launcher;
            s.crash_add(&r, at).unwrap()
        };
        let id = on("0.3.4", 10);
        let fix = Resolution {
            by: "alice",
            note: "fixed in 0.3.5",
            fixed_in: "0.3.5",
        };
        assert!(s.crash_set_status(id, Some(&fix), 50).unwrap());
        let open = Filter {
            status: Some("open".into()),
            ..Filter::default()
        };
        assert!(s.crash_groups(&open, 10).unwrap().is_empty());
        let group = &all(&s)[0];
        assert_eq!(
            (group.status.status.as_str(), group.status.note.as_deref()),
            ("resolved", Some("fixed in 0.3.5"))
        );
        on("0.3.4", 60);
        assert_eq!(all(&s)[0].status.status, "resolved");
        on("0.3.5", 70);
        assert_eq!(all(&s)[0].status.status, "open");

        // No fix version given: the newest launcher seen doesn't reopen it, a newer one does.
        let id = on("0.3.5", 80);
        let unsure = Resolution {
            by: "alice",
            note: "",
            fixed_in: "",
        };
        s.crash_set_status(id, Some(&unsure), 90).unwrap();
        assert_eq!(all(&s)[0].status.fixed_in, None);
        on("0.3.5", 100);
        assert_eq!(all(&s)[0].status.status, "resolved");
        on("0.3.6", 110);
        assert_eq!(all(&s)[0].status.status, "open");

        s.crash_set_status(id, Some(&unsure), 120).unwrap();
        assert!(s.crash_set_status(id, None, 130).unwrap());
        assert_eq!(all(&s)[0].status.status, "open");
        assert!(!s.crash_set_status(9999, None, 130).unwrap());

        // Only development builds (no version) had sent it: any release reopens it.
        let mut from_dev = report("1.8.9", "java.lang.IllegalStateException: dev only");
        from_dev.launcher = "";
        let id = s.crash_add(&from_dev, 140).unwrap();
        s.crash_set_status(id, Some(&unsure), 150).unwrap();
        s.crash_add(&from_dev, 160).unwrap();
        assert_eq!(all(&s)[0].status.status, "resolved");
        from_dev.launcher = "0.3.6";
        s.crash_add(&from_dev, 170).unwrap();
        assert_eq!(all(&s)[0].status.status, "open");
    }

    #[test]
    fn long_logs_keep_their_end() {
        let long = format!("{}END", "é".repeat(MAX_LOG_BYTES));
        let clipped = clip_log(&long);
        assert!(clipped.len() <= MAX_LOG_BYTES);
        assert!(clipped.ends_with("END"));
        assert_eq!(clean_field("a\nb\u{7}c "), "abc");
    }
    #[test]
    fn failed_sign_in_noise_is_not_the_first_error() {
        let log = "[17:57:02] [Download-2/ERROR] Failed to fetch user properties
    com.mojang.authlib.exceptions.InvalidCredentialsException: Status: 401
    net.minecraft.class_4355: Realms authentication error with message 'java.lang.RuntimeException: Failed to parse into SignedJWT: 0'
java.lang.OutOfMemoryError: Java heap space";
        let summary = summarize("Crash", log);
        assert_eq!(
            summary.first_error.as_deref(),
            Some("java.lang.OutOfMemoryError: Java heap space")
        );
        // Two different crashes after the same noise are two groups.
        let other = "com.mojang.authlib.exceptions.InvalidCredentialsException: Status: 401
java.lang.NullPointerException: x";
        assert_ne!(
            fingerprint("1.21.11", "Crash", log),
            fingerprint("1.21.11", "Crash", other)
        );
    }
}
