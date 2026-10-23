//! Who may moderate: named keys, so access can be shared with other people
//! and taken back one person at a time. Read-only keys (for a script or an AI
//! looking through crash reports) can look but never change anything.

use rusqlite::params;
use sha2::{Digest, Sha256};

use crate::gallery::LogEntry;
use crate::store::Store;

/// Shortest key accepted; anything shorter is ignored at startup.
pub const MIN_KEY_LEN: usize = 16;

/// What a key may do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Everything on the dashboard.
    Moderator,
    /// Only reading crash reports and suggestions.
    Reader,
}

#[derive(Default)]
pub struct Moderators {
    /// (name, SHA-256 of the key, role). Only hashes are kept in memory.
    keys: Vec<(String, [u8; 32], Role)>,
}

fn hash(key: &str) -> [u8; 32] {
    Sha256::digest(key.as_bytes()).into()
}

/// Equal without stopping at the first difference.
fn same(a: &[u8; 32], b: &[u8; 32]) -> bool {
    a.iter().zip(b).fold(0u8, |diff, (x, y)| diff | (x ^ y)) == 0
}

impl Moderators {
    /// `ARCTIC_COSMETICS_ADMIN_KEYS` is `name:key,name:key`; the older
    /// single `ARCTIC_COSMETICS_ADMIN_KEY` counts as the moderator "admin".
    /// Entries that are malformed, repeat a name or have a short key are
    /// skipped (and said so, without the key).
    pub fn parse(list: Option<&str>, single: Option<&str>) -> Self {
        let mut keys = Self::default();
        keys.add_list(list, Role::Moderator);
        if let Some(key) = single {
            keys.add("admin", key.trim(), Role::Moderator);
        }
        keys
    }

    /// Adds `ARCTIC_COSMETICS_READ_KEYS` (`name:key,name:key`): keys that read
    /// everything the dashboard shows and change nothing. Names are shared with moderators,
    /// so one name never means two keys.
    pub fn with_readers(mut self, list: Option<&str>) -> Self {
        self.add_list(list, Role::Reader);
        self
    }

    fn add_list(&mut self, list: Option<&str>, role: Role) {
        for entry in list
            .unwrap_or_default()
            .split(',')
            .filter(|e| !e.trim().is_empty())
        {
            match entry.split_once(':') {
                Some((name, key)) => self.add(name, key.trim(), role),
                None => log::warn!("a key entry has no name:key shape; ignored"),
            }
        }
    }

    fn add(&mut self, name: &str, key: &str, role: Role) {
        let name = name.trim();
        let valid_name = !name.is_empty()
            && name.len() <= 32
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if !valid_name || key.len() < MIN_KEY_LEN || self.keys.iter().any(|(n, ..)| n == name) {
            log::warn!("key for {name:?} ignored (bad name, short key or repeated)");
            return;
        }
        self.keys.push((name.to_owned(), hash(key), role));
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Whether a moderator by this name still has a key (a removed key's phones stop hearing from us).
    pub fn is_moderator(&self, name: &str) -> bool {
        self.keys
            .iter()
            .any(|(n, _, role)| n == name && *role == Role::Moderator)
    }

    /// Whose this key is and what it may do. Every key is compared, so the
    /// time taken doesn't say how close a guess was or whose it nearly was.
    pub fn identify(&self, key: &str) -> Option<(&str, Role)> {
        let given = hash(key);
        let mut found = None;
        for (name, stored, role) in &self.keys {
            if same(stored, &given) {
                found = Some((name.as_str(), *role));
            }
        }
        found
    }
}

/// Longest request noted for a read-only key.
const MAX_READ_NOTE: usize = 200;
/// Read notes are forgotten after this many seconds (decisions are kept).
const KEEP_READS: i64 = 30 * 86_400;

impl Store {
    /// Note a read-only key's request in History, forgetting old read notes.
    pub fn read_note(&self, name: &str, what: &str, now: u64) -> rusqlite::Result<()> {
        let what: String = what.chars().take(MAX_READ_NOTE).collect();
        let conn = self.conn();
        conn.execute(
            "INSERT INTO moderation_log (at, moderator, action, target, detail)
             VALUES (?1, ?2, 'read', '', ?3)",
            params![now as i64, name, what],
        )?;
        conn.execute(
            "DELETE FROM moderation_log WHERE action = 'read' AND at < ?1",
            [now as i64 - KEEP_READS],
        )?;
        Ok(())
    }

    /// History, newest first: moderators' decisions, or read-only keys' reads.
    pub fn moderation_history(&self, reads: bool, limit: usize) -> rusqlite::Result<Vec<LogEntry>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT at, moderator, action, target, detail FROM moderation_log
             WHERE (action = 'read') = ?1 ORDER BY id DESC LIMIT ?2",
        )?;
        stmt.query_map(params![reads, limit as i64], |r| {
            Ok(LogEntry {
                at: r.get(0)?,
                moderator: r.get(1)?,
                action: r.get(2)?,
                target: r.get(3)?,
                detail: r.get(4)?,
            })
        })?
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_keys_identify_their_owner() {
        let m = Moderators::parse(
            Some("alice:aaaaaaaaaaaaaaaaaaaa, bob:bbbbbbbbbbbbbbbbbbbb"),
            None,
        );
        assert_eq!(
            m.identify("aaaaaaaaaaaaaaaaaaaa"),
            Some(("alice", Role::Moderator))
        );
        assert_eq!(
            m.identify("bbbbbbbbbbbbbbbbbbbb"),
            Some(("bob", Role::Moderator))
        );
        assert_eq!(m.identify("cccccccccccccccccccc"), None);
        assert_eq!(m.identify(""), None);
    }

    #[test]
    fn single_key_is_admin_and_bad_entries_are_skipped() {
        let m = Moderators::parse(
            Some(
                "short:abc,no-colon,alice:aaaaaaaaaaaaaaaaaaaa,alice:dddddddddddddddddddd,bad name:eeeeeeeeeeeeeeeeeeee",
            ),
            Some("admin-key-admin-key"),
        );
        assert_eq!(
            m.identify("admin-key-admin-key"),
            Some(("admin", Role::Moderator))
        );
        assert_eq!(
            m.identify("aaaaaaaaaaaaaaaaaaaa"),
            Some(("alice", Role::Moderator))
        );
        assert_eq!(m.identify("dddddddddddddddddddd"), None);
        assert_eq!(m.identify("eeeeeeeeeeeeeeeeeeee"), None);
        assert!(Moderators::parse(None, None).is_empty());
    }

    #[test]
    fn read_keys_only_read_and_never_take_a_moderators_name() {
        let m = Moderators::parse(Some("alice:aaaaaaaaaaaaaaaaaaaa"), None).with_readers(Some(
            "ai:rrrrrrrrrrrrrrrrrrrr, alice:ssssssssssssssssssss, tiny:abc",
        ));
        assert_eq!(
            m.identify("rrrrrrrrrrrrrrrrrrrr"),
            Some(("ai", Role::Reader))
        );
        assert_eq!(m.identify("ssssssssssssssssssss"), None);
        assert_eq!(m.identify("abc"), None);
        assert!(
            !Moderators::parse(None, None)
                .with_readers(Some("ai:rrrrrrrrrrrrrrrrrrrr"))
                .is_empty()
        );
    }

    #[test]
    fn reads_are_kept_apart_from_decisions_and_forgotten() {
        let s = Store::memory().unwrap();
        s.moderation_note("alice", "resolve", "1", "crash group", 10)
            .unwrap();
        s.read_note("ai", &"x".repeat(500), 20).unwrap();
        let decisions = s.moderation_history(false, 10).unwrap();
        assert_eq!(decisions.len(), 1);
        assert_eq!(decisions[0].action, "resolve");
        let reads = s.moderation_history(true, 10).unwrap();
        assert_eq!(reads[0].detail.len(), MAX_READ_NOTE);
        s.read_note("ai", "/later", 20 + KEEP_READS as u64 + 1)
            .unwrap();
        assert_eq!(s.moderation_history(true, 10).unwrap().len(), 1);
        assert_eq!(s.moderation_history(false, 10).unwrap().len(), 1);
    }
}
