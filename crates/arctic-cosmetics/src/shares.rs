//! Share codes: short codes for small JSON bundles players pass around
//! (instances as mod lists, HUD layouts, crosshairs, whole profiles). The
//! server only stores them; clients check every bundle before using it.

use rusqlite::{OptionalExtension, params};
use sha2::{Digest, Sha256};

use crate::store::Store;

/// Largest bundle accepted (a profile with a few big modpacks fits easily).
pub const MAX_BYTES: usize = 256 * 1024;
/// Codes one player can create per day.
pub const MAX_PER_DAY: i64 = 100;
/// Kinds of bundle clients know how to use.
pub const KINDS: &[&str] = &["instance", "hud", "crosshair", "client", "profile"];
/// No look-alike characters (0/o, 1/l/i).
const ALPHABET: &[u8] = b"abcdefghjkmnpqrstuvwxyz23456789";
pub const CODE_LEN: usize = 8;
const DAY_SECS: i64 = 24 * 60 * 60;

#[derive(Debug, PartialEq, Eq)]
pub enum ShareError {
    TooMany,
    Db(String),
}

impl Store {
    pub(crate) fn migrate_shares(&self) -> rusqlite::Result<()> {
        self.conn().execute_batch(
            "CREATE TABLE IF NOT EXISTS shares (
                 code TEXT PRIMARY KEY,
                 kind TEXT NOT NULL,
                 body TEXT NOT NULL,
                 hash TEXT NOT NULL UNIQUE,
                 owner TEXT NOT NULL,
                 created INTEGER NOT NULL,
                 uses INTEGER NOT NULL DEFAULT 0
             );
             CREATE INDEX IF NOT EXISTS shares_owner ON shares (owner, created);",
        )
    }

    /// The code for `body`: the existing one if anyone shared the same
    /// bundle before, otherwise a new one.
    pub fn share_create(
        &self,
        kind: &str,
        body: &str,
        owner: &str,
        now: u64,
    ) -> Result<String, ShareError> {
        let db = |e: rusqlite::Error| ShareError::Db(e.to_string());
        let hash = hex(&Sha256::digest(body.as_bytes()));
        let conn = self.conn();
        let existing: Option<String> = conn
            .query_row("SELECT code FROM shares WHERE hash = ?1", [&hash], |r| {
                r.get(0)
            })
            .optional()
            .map_err(db)?;
        if let Some(code) = existing {
            return Ok(code);
        }
        let today: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM shares WHERE owner = ?1 AND created > ?2",
                params![owner, now as i64 - DAY_SECS],
                |r| r.get(0),
            )
            .map_err(db)?;
        if today >= MAX_PER_DAY {
            return Err(ShareError::TooMany);
        }
        loop {
            let code = new_code();
            let inserted = conn
                .execute(
                    "INSERT OR IGNORE INTO shares (code, kind, body, hash, owner, created)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![code, kind, body, hash, owner, now as i64],
                )
                .map_err(db)?;
            if inserted == 1 {
                return Ok(code);
            }
        }
    }

    /// The bundle behind a code, counting the use.
    pub fn share_get(&self, code: &str) -> rusqlite::Result<Option<String>> {
        let conn = self.conn();
        let body: Option<String> = conn
            .query_row("SELECT body FROM shares WHERE code = ?1", [code], |r| {
                r.get(0)
            })
            .optional()?;
        if body.is_some() {
            conn.execute("UPDATE shares SET uses = uses + 1 WHERE code = ?1", [code])?;
        }
        Ok(body)
    }
}

/// `code` as stored (lowercase, no dashes), if it could be one.
pub fn clean_code(code: &str) -> Option<String> {
    let code: String = code
        .chars()
        .filter(|c| *c != '-')
        .map(|c| c.to_ascii_lowercase())
        .collect();
    (code.len() == CODE_LEN && code.bytes().all(|b| ALPHABET.contains(&b))).then_some(code)
}

fn new_code() -> String {
    let random = uuid::Uuid::new_v4();
    random
        .as_bytes()
        .iter()
        .take(CODE_LEN)
        .map(|b| char::from(ALPHABET[usize::from(*b) % ALPHABET.len()]))
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_bundle_same_code() {
        let s = Store::memory().unwrap();
        let a = s.share_create("hud", "{\"x\":1}", "p1", 10).unwrap();
        let b = s.share_create("hud", "{\"x\":1}", "p2", 11).unwrap();
        let c = s.share_create("hud", "{\"x\":2}", "p1", 12).unwrap();
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(s.share_get(&a).unwrap().as_deref(), Some("{\"x\":1}"));
        assert_eq!(s.share_get("zzzzzzzz").unwrap(), None);
    }

    #[test]
    fn daily_cap_per_player() {
        let s = Store::memory().unwrap();
        for i in 0..MAX_PER_DAY {
            s.share_create("hud", &format!("{{\"n\":{i}}}"), "p1", 100)
                .unwrap();
        }
        assert_eq!(
            s.share_create("hud", "{\"n\":-1}", "p1", 100),
            Err(ShareError::TooMany)
        );
        assert!(s.share_create("hud", "{\"n\":-1}", "p2", 100).is_ok());
        assert!(
            s.share_create("hud", "{\"n\":-2}", "p1", 100 + DAY_SECS as u64)
                .is_ok()
        );
    }

    #[test]
    fn codes_are_cleaned() {
        let code = new_code();
        assert_eq!(code.len(), CODE_LEN);
        assert_eq!(clean_code(&code).as_deref(), Some(code.as_str()));
        assert_eq!(clean_code("ABCD-EFGH").as_deref(), Some("abcdefgh"));
        assert_eq!(clean_code("abcd-efg0"), None);
        assert_eq!(clean_code("abc"), None);
        assert_eq!(clean_code("../../etc"), None);
    }
}
