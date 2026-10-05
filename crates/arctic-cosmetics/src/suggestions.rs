//! Ideas and bug reports players typed into the launcher, with an optional
//! log they chose to attach. Shown in the admin dashboard.

use rusqlite::{OptionalExtension, params};
use serde::Serialize;

use crate::crashes::{clean_field, clip_log};
use crate::store::Store;

/// Longest message kept (characters).
pub const MAX_TEXT: usize = 2000;
/// Suggestions kept in all; the oldest go first.
const KEEP: i64 = 2000;

pub struct Suggestion<'a> {
    pub kind: &'a str,
    pub text: &'a str,
    pub contact: &'a str,
    pub launcher: &'a str,
    pub os: &'a str,
    pub log: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Entry {
    pub id: i64,
    pub at: i64,
    pub kind: String,
    pub text: String,
    pub contact: String,
    pub launcher: String,
    pub os: String,
    pub done: bool,
    /// Whether a log came with it (the log itself is in the detail).
    pub has_log: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Detail {
    #[serde(flatten)]
    pub entry: Entry,
    pub log: String,
}

/// `idea` or `bug`; anything else is an idea.
pub fn clean_kind(kind: &str) -> &'static str {
    if kind.trim().eq_ignore_ascii_case("bug") {
        "bug"
    } else {
        "idea"
    }
}

/// The message with control characters (other than line breaks) taken out and cut to `MAX_TEXT`.
pub fn clean_text(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control() || *c == '\n')
        .take(MAX_TEXT)
        .collect::<String>()
        .trim()
        .to_owned()
}

impl Store {
    pub(crate) fn migrate_suggestions(&self) -> rusqlite::Result<()> {
        self.conn().execute_batch(
            "CREATE TABLE IF NOT EXISTS suggestions (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 at INTEGER NOT NULL,
                 kind TEXT NOT NULL,
                 text TEXT NOT NULL,
                 contact TEXT NOT NULL,
                 launcher TEXT NOT NULL,
                 os TEXT NOT NULL,
                 log TEXT NOT NULL,
                 done INTEGER NOT NULL DEFAULT 0
             );",
        )
    }

    /// Keep a suggestion; returns its number.
    pub fn suggestion_add(&self, s: &Suggestion, now: u64) -> rusqlite::Result<i64> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO suggestions (at, kind, text, contact, launcher, os, log)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                now as i64,
                clean_kind(s.kind),
                clean_text(s.text),
                clean_field(s.contact),
                clean_field(s.launcher),
                clean_field(s.os),
                clip_log(s.log)
            ],
        )?;
        let id = conn.last_insert_rowid();
        conn.execute(
            "DELETE FROM suggestions WHERE id NOT IN (SELECT id FROM suggestions ORDER BY id DESC LIMIT ?1)",
            [KEEP],
        )?;
        Ok(id)
    }

    fn entry(r: &rusqlite::Row) -> rusqlite::Result<Entry> {
        Ok(Entry {
            id: r.get(0)?,
            at: r.get(1)?,
            kind: r.get(2)?,
            text: r.get(3)?,
            contact: r.get(4)?,
            launcher: r.get(5)?,
            os: r.get(6)?,
            done: r.get::<_, i64>(7)? != 0,
            has_log: r.get::<_, i64>(8)? > 0,
        })
    }

    /// Ones not yet dealt with first, newest first.
    pub fn suggestions(&self, limit: usize) -> rusqlite::Result<Vec<Entry>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, at, kind, text, contact, launcher, os, done, LENGTH(log) FROM suggestions
             ORDER BY done, id DESC LIMIT ?1",
        )?;
        stmt.query_map([limit as i64], Self::entry)?.collect()
    }

    pub fn suggestion_detail(&self, id: i64) -> rusqlite::Result<Option<Detail>> {
        self.conn()
            .query_row(
                "SELECT id, at, kind, text, contact, launcher, os, done, LENGTH(log), log FROM suggestions WHERE id = ?1",
                [id],
                |r| {
                    Ok(Detail {
                        entry: Self::entry(r)?,
                        log: r.get(9)?,
                    })
                },
            )
            .optional()
    }

    pub fn suggestion_set_done(&self, id: i64, done: bool) -> rusqlite::Result<bool> {
        Ok(self.conn().execute(
            "UPDATE suggestions SET done = ?1 WHERE id = ?2",
            params![done as i64, id],
        )? > 0)
    }

    pub fn suggestion_delete(&self, id: i64) -> rusqlite::Result<bool> {
        Ok(self
            .conn()
            .execute("DELETE FROM suggestions WHERE id = ?1", [id])?
            > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggestions_are_kept_cleaned_and_marked_done() {
        let s = Store::memory().unwrap();
        let id = s
            .suggestion_add(
                &Suggestion {
                    kind: "BUG",
                    text: "  a map\u{7} wall\nlags ",
                    contact: "me@example.com",
                    launcher: "0.3.0",
                    os: "linux",
                    log: "log text",
                },
                10,
            )
            .unwrap();
        s.suggestion_add(
            &Suggestion {
                kind: "whatever",
                text: "more themes",
                contact: "",
                launcher: "0.3.0",
                os: "windows",
                log: "",
            },
            20,
        )
        .unwrap();
        let all = s.suggestions(10).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!((all[0].kind.as_str(), all[0].has_log), ("idea", false));
        assert_eq!(all[1].text, "a map wall\nlags");
        assert_eq!(all[1].kind, "bug");
        assert_eq!(s.suggestion_detail(id).unwrap().unwrap().log, "log text");
        assert!(s.suggestion_set_done(id, true).unwrap());
        assert!(s.suggestions(10).unwrap()[1].done);
        assert!(s.suggestion_delete(id).unwrap());
        assert!(!s.suggestion_delete(id).unwrap());
    }
}
