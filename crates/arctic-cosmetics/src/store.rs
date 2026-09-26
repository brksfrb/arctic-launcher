//! SQLite storage: each player's look (skin, model, cape) and the texture
//! images, stored once per content hash.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

pub struct Store {
    db: Mutex<Connection>,
}

/// What other players see. Texture fields are content hashes (SHA-1 hex).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Look {
    pub skin: Option<String>,
    /// `classic` or `slim` (only meaningful with a skin).
    pub model: String,
    pub cape: Option<String>,
}

impl Store {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        Self::init(Connection::open(path)?)
    }

    #[cfg(test)]
    pub fn memory() -> rusqlite::Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> rusqlite::Result<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS players (
                 uuid TEXT PRIMARY KEY,
                 name TEXT NOT NULL,
                 skin TEXT,
                 model TEXT NOT NULL DEFAULT 'classic',
                 cape TEXT,
                 key_hash TEXT,
                 updated INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS textures (
                 sha1 TEXT PRIMARY KEY,
                 png BLOB NOT NULL,
                 created INTEGER NOT NULL
             );",
        )?;
        Ok(Self {
            db: Mutex::new(conn),
        })
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.db.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Record a verified player (keeps the latest name and any look).
    pub fn touch(&self, uuid: &str, name: &str, now: u64) -> rusqlite::Result<()> {
        self.conn().execute(
            "INSERT INTO players (uuid, name, updated) VALUES (?1, ?2, ?3)
             ON CONFLICT(uuid) DO UPDATE SET name = excluded.name",
            params![uuid, name, now as i64],
        )?;
        Ok(())
    }

    /// Stored key hash for an offline player (`None` = unclaimed).
    pub fn key_hash(&self, uuid: &str) -> rusqlite::Result<Option<String>> {
        Ok(self
            .conn()
            .query_row(
                "SELECT key_hash FROM players WHERE uuid = ?1",
                [uuid],
                |r| r.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten())
    }

    pub fn set_key_hash(&self, uuid: &str, hash: &str) -> rusqlite::Result<()> {
        self.conn().execute(
            "UPDATE players SET key_hash = ?2 WHERE uuid = ?1 AND key_hash IS NULL",
            params![uuid, hash],
        )?;
        Ok(())
    }

    pub fn look(&self, uuid: &str) -> rusqlite::Result<Look> {
        Ok(self
            .conn()
            .query_row(
                "SELECT skin, model, cape FROM players WHERE uuid = ?1",
                [uuid],
                |r| {
                    Ok(Look {
                        skin: r.get(0)?,
                        model: r.get(1)?,
                        cape: r.get(2)?,
                    })
                },
            )
            .optional()?
            .unwrap_or_else(|| Look {
                model: "classic".into(),
                ..Look::default()
            }))
    }

    pub fn set_look(&self, uuid: &str, look: &Look, now: u64) -> rusqlite::Result<()> {
        self.conn().execute(
            "UPDATE players SET skin = ?2, model = ?3, cape = ?4, updated = ?5 WHERE uuid = ?1",
            params![uuid, look.skin, look.model, look.cape, now as i64],
        )?;
        Ok(())
    }

    /// Looks for many players; players without any are left out.
    pub fn looks(&self, uuids: &[String]) -> rusqlite::Result<Vec<(String, Look)>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(
            "SELECT skin, model, cape FROM players WHERE uuid = ?1
             AND (skin IS NOT NULL OR cape IS NOT NULL)",
        )?;
        let mut out = Vec::new();
        for uuid in uuids {
            let look = stmt
                .query_row([uuid], |r| {
                    Ok(Look {
                        skin: r.get(0)?,
                        model: r.get(1)?,
                        cape: r.get(2)?,
                    })
                })
                .optional()?;
            if let Some(look) = look {
                out.push((uuid.clone(), look));
            }
        }
        Ok(out)
    }

    /// Store a texture (no-op if it's already there).
    pub fn put_texture(&self, sha1: &str, png: &[u8], now: u64) -> rusqlite::Result<()> {
        self.conn().execute(
            "INSERT OR IGNORE INTO textures (sha1, png, created) VALUES (?1, ?2, ?3)",
            params![sha1, png, now as i64],
        )?;
        Ok(())
    }

    pub fn texture(&self, sha1: &str) -> rusqlite::Result<Option<Vec<u8>>> {
        self.conn()
            .query_row("SELECT png FROM textures WHERE sha1 = ?1", [sha1], |r| {
                r.get(0)
            })
            .optional()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_round_trip() {
        let s = Store::memory().unwrap();
        s.touch("a", "Alice", 1).unwrap();
        s.touch("b", "Bob", 1).unwrap();
        assert_eq!(s.look("a").unwrap().skin, None);
        let look = Look {
            skin: Some("s1".into()),
            model: "slim".into(),
            cape: None,
        };
        s.set_look("a", &look, 2).unwrap();
        assert_eq!(s.look("a").unwrap(), look);
        let many = s.looks(&["a".into(), "b".into(), "zzz".into()]).unwrap();
        assert_eq!(many.len(), 1);
        // Name updates keep the look.
        s.touch("a", "Alicia", 3).unwrap();
        assert_eq!(s.look("a").unwrap(), look);
    }

    #[test]
    fn offline_keys_are_claimed_once() {
        let s = Store::memory().unwrap();
        s.touch("o", "Steve", 1).unwrap();
        assert_eq!(s.key_hash("o").unwrap(), None);
        s.set_key_hash("o", "h1").unwrap();
        s.set_key_hash("o", "h2").unwrap();
        assert_eq!(s.key_hash("o").unwrap().as_deref(), Some("h1"));
    }

    #[test]
    fn textures_dedupe() {
        let s = Store::memory().unwrap();
        s.put_texture("x", b"one", 1).unwrap();
        s.put_texture("x", b"two", 2).unwrap();
        assert_eq!(s.texture("x").unwrap().unwrap(), b"one");
        assert_eq!(s.texture("y").unwrap(), None);
    }
}
