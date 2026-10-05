//! Crash reports players chose to send: kept so the same crash seen by
//! many people shows up as one group with a count.

use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::store::Store;

/// Longest log text kept per report (bytes).
pub const MAX_LOG_BYTES: usize = 48 * 1024;
/// Reports kept in all; the oldest go first.
const KEEP: i64 = 2000;
/// Longest field other than the log.
const MAX_FIELD: usize = 120;

/// What the launcher sends.
pub struct Report<'a> {
    pub launcher: &'a str,
    pub game: &'a str,
    pub loader: &'a str,
    pub os: &'a str,
    pub title: &'a str,
    pub log: &'a str,
}

/// One group of the same crash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Group {
    /// The newest report of the group: open it for the full text.
    pub id: i64,
    pub title: String,
    pub game: String,
    pub count: i64,
    pub last: i64,
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
    pub log: String,
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

/// Groups reports of one crash: the game version plus the first line that names an exception.
fn fingerprint(game: &str, title: &str, log: &str) -> String {
    let cause = log
        .lines()
        .find(|l| l.contains("Exception") || l.contains("Error:"))
        .unwrap_or(title);
    let digits_gone: String = cause.chars().filter(|c| !c.is_ascii_digit()).collect();
    let digest = Sha256::digest(format!("{game}|{digits_gone}").as_bytes());
    digest.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

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
                 fingerprint TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS crashes_fingerprint ON crashes (fingerprint, at);",
        )
    }

    /// Keep a report; returns its number.
    pub fn crash_add(&self, report: &Report, now: u64) -> rusqlite::Result<i64> {
        let log = clip_log(report.log);
        let print = fingerprint(report.game, report.title, log);
        let conn = self.conn();
        conn.execute(
            "INSERT INTO crashes (at, launcher, game, loader, os, title, log, fingerprint)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                now as i64,
                report.launcher,
                report.game,
                report.loader,
                report.os,
                report.title,
                log,
                print
            ],
        )?;
        let id = conn.last_insert_rowid();
        conn.execute(
            "DELETE FROM crashes WHERE id NOT IN (SELECT id FROM crashes ORDER BY id DESC LIMIT ?1)",
            [KEEP],
        )?;
        Ok(id)
    }

    /// The crashes seen, grouped, the most recent first.
    pub fn crash_groups(&self, limit: usize) -> rusqlite::Result<Vec<Group>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT MAX(id), title, game, COUNT(*), MAX(at) FROM crashes
             GROUP BY fingerprint ORDER BY MAX(at) DESC, MAX(id) DESC LIMIT ?1",
        )?;
        stmt.query_map([limit as i64], |r| {
            Ok(Group {
                id: r.get(0)?,
                title: r.get(1)?,
                game: r.get(2)?,
                count: r.get(3)?,
                last: r.get(4)?,
            })
        })?
        .collect()
    }

    pub fn crash_detail(&self, id: i64) -> rusqlite::Result<Option<Detail>> {
        self.conn()
            .query_row(
                "SELECT id, at, launcher, game, loader, os, title, log FROM crashes WHERE id = ?1",
                [id],
                |r| {
                    Ok(Detail {
                        id: r.get(0)?,
                        at: r.get(1)?,
                        launcher: r.get(2)?,
                        game: r.get(3)?,
                        loader: r.get(4)?,
                        os: r.get(5)?,
                        title: r.get(6)?,
                        log: r.get(7)?,
                    })
                },
            )
            .optional()
    }

    /// Forget every report of the same crash as this one.
    pub fn crash_delete_group(&self, id: i64) -> rusqlite::Result<bool> {
        let conn = self.conn();
        let print: Option<String> = conn
            .query_row("SELECT fingerprint FROM crashes WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .optional()?;
        let Some(print) = print else {
            return Ok(false);
        };
        conn.execute("DELETE FROM crashes WHERE fingerprint = ?1", [print])?;
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
        }
    }

    #[test]
    fn the_same_crash_groups_and_counts() {
        let s = Store::memory().unwrap();
        let a = "java.lang.NullPointerException at x.y(Z.java:12)";
        let b = "java.lang.NullPointerException at x.y(Z.java:99)";
        s.crash_add(&report("1.21.4", a), 10).unwrap();
        s.crash_add(&report("1.21.4", b), 20).unwrap();
        s.crash_add(&report("1.20.1", a), 30).unwrap();
        let groups = s.crash_groups(10).unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!((groups[0].game.as_str(), groups[0].count), ("1.20.1", 1));
        assert_eq!((groups[1].game.as_str(), groups[1].count), ("1.21.4", 2));
        let detail = s.crash_detail(groups[1].id).unwrap().unwrap();
        assert_eq!(detail.log, b);
        assert!(s.crash_delete_group(groups[1].id).unwrap());
        assert_eq!(s.crash_groups(10).unwrap().len(), 1);
    }

    #[test]
    fn long_logs_keep_their_end() {
        let long = format!("{}END", "é".repeat(MAX_LOG_BYTES));
        let clipped = clip_log(&long);
        assert!(clipped.len() <= MAX_LOG_BYTES);
        assert!(clipped.ends_with("END"));
        assert_eq!(clean_field("a\nb\u{7}c "), "abc");
    }
}
