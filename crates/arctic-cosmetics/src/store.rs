//! SQLite storage: each player's look (skin, model, cape) and the texture
//! images, stored once per content hash.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{Connection, OptionalExtension, params};
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

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
    /// Worn 3D cosmetics (catalog ids, one per slot).
    #[serde(default)]
    pub cosmetics: Vec<String>,
    /// How some of them are worn, by id (only worn ones).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fit: BTreeMap<String, Fit>,
}

/// Half pixels a face cosmetic can move up or down.
pub const FIT_UP: i8 = 6;
/// Half pixels it can move forward or back.
pub const FIT_FORWARD: i8 = 4;

/// How a cosmetic sits: moved (in half pixels: up, toward the face), or worn on the other side.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fit {
    #[serde(default, skip_serializing_if = "is_zero")]
    pub up: i8,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub forward: i8,
    /// A one-sided cosmetic (a shoulder pet) worn on the other side, mirrored.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub mirror: bool,
}

fn is_zero(v: &i8) -> bool {
    *v == 0
}

impl Fit {
    /// Within its limits, or None when it changes nothing.
    pub fn clamped(&self) -> Option<Fit> {
        let fit = Fit {
            up: self.up.clamp(-FIT_UP, FIT_UP),
            forward: self.forward.clamp(-FIT_FORWARD, FIT_FORWARD),
            mirror: self.mirror,
        };
        (fit != Fit::default()).then_some(fit)
    }
}

/// Fits for worn cosmetics only, within their limits.
pub fn fits_for(worn: &[String], fit: &BTreeMap<String, Fit>) -> BTreeMap<String, Fit> {
    fit.iter()
        .filter(|(id, _)| worn.contains(id))
        .filter_map(|(id, f)| f.clamped().map(|f| (id.clone(), f)))
        .collect()
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
             );
             CREATE TABLE IF NOT EXISTS gallery (
                 id TEXT PRIMARY KEY,
                 texture TEXT NOT NULL,
                 model TEXT NOT NULL,
                 name TEXT NOT NULL,
                 author_uuid TEXT NOT NULL,
                 author_name TEXT NOT NULL,
                 created INTEGER NOT NULL,
                 downloads INTEGER NOT NULL DEFAULT 0,
                 reports INTEGER NOT NULL DEFAULT 0,
                 hidden INTEGER NOT NULL DEFAULT 0,
                 seed_key TEXT
             );
             CREATE UNIQUE INDEX IF NOT EXISTS gallery_unique ON gallery (texture, author_uuid);
             CREATE TABLE IF NOT EXISTS gallery_reports (
                 item TEXT NOT NULL,
                 reporter TEXT NOT NULL,
                 PRIMARY KEY (item, reporter)
             );",
        )?;
        let store = Self {
            db: Mutex::new(conn),
        };
        store.migrate_gallery_looks()?;
        store.migrate_gallery_status()?;
        store.migrate_gallery_category()?;
        store.migrate_gallery_seed_key()?;
        store.migrate_crashes()?;
        store.migrate_suggestions()?;
        store.migrate_presence()?;
        store.migrate_shares()?;
        store.migrate_listing()?;
        store.migrate_friends()?;
        store.migrate_chat()?;
        store.migrate_voice()?;
        Ok(store)
    }

    /// Counts for the admin overview: accounts the server has seen (by the last time their looks
    /// were saved), friends profiles and crash reports.
    pub fn overview(&self, now: u64) -> rusqlite::Result<serde_json::Value> {
        let conn = self.conn();
        let count = |sql: &str, p: &[&dyn rusqlite::ToSql]| -> rusqlite::Result<i64> {
            conn.query_row(sql, p, |r| r.get(0))
        };
        let week = now.saturating_sub(7 * 86_400) as i64;
        let month = now.saturating_sub(30 * 86_400) as i64;
        Ok(serde_json::json!({
            "accounts": count("SELECT COUNT(*) FROM players", &[])?,
            "accounts_7d": count("SELECT COUNT(*) FROM players WHERE updated >= ?1", &[&week])?,
            "accounts_30d": count("SELECT COUNT(*) FROM players WHERE updated >= ?1", &[&month])?,
            "profiles": count("SELECT COUNT(*) FROM arctic_profiles", &[])?,
            "crash_reports": count("SELECT COUNT(*) FROM crashes", &[])?,
        }))
    }

    pub(crate) fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
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

    /// A signed-in player's current name.
    pub fn author_name(&self, uuid: &str) -> rusqlite::Result<Option<String>> {
        self.conn()
            .query_row("SELECT name FROM players WHERE uuid = ?1", [uuid], |r| {
                r.get(0)
            })
            .optional()
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

    /// Move an offline name to a new key (recovery on a new PC).
    /// A moderator frees an offline name so its owner's launcher can claim it again (a PC that lost
    /// its key). The profile and friends stay: they hang on the name's UUID. False when it has no claim.
    pub fn release_key(&self, uuid: &str) -> rusqlite::Result<bool> {
        let changed = self.conn().execute(
            "UPDATE players SET key_hash = NULL WHERE uuid = ?1 AND key_hash IS NOT NULL",
            params![uuid],
        )?;
        Ok(changed > 0)
    }

    pub fn replace_key_hash(&self, uuid: &str, hash: &str) -> rusqlite::Result<()> {
        self.conn().execute(
            "UPDATE players SET key_hash = ?2 WHERE uuid = ?1",
            params![uuid, hash],
        )?;
        Ok(())
    }

    pub fn look(&self, uuid: &str) -> rusqlite::Result<Look> {
        Ok(self
            .conn()
            .query_row(
                "SELECT skin, model, cape, cosmetics, fit FROM players WHERE uuid = ?1",
                [uuid],
                |r| {
                    Ok(Look {
                        skin: r.get(0)?,
                        model: r.get(1)?,
                        cape: r.get(2)?,
                        cosmetics: cosmetics_column(r.get(3)?),
                        fit: fit_column(r.get(4)?),
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
            "UPDATE players SET skin = ?2, model = ?3, cape = ?4, cosmetics = ?5, updated = ?6, fit = ?7
             WHERE uuid = ?1",
            params![
                uuid,
                look.skin,
                look.model,
                look.cape,
                serde_json::to_string(&look.cosmetics).unwrap_or_default(),
                now as i64,
                (!look.fit.is_empty()).then(|| serde_json::to_string(&look.fit).unwrap_or_default()),
            ],
        )?;
        Ok(())
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

/// The `fit` column: a JSON object by cosmetic id (NULL or broken = none).
pub(crate) fn fit_column(value: Option<String>) -> BTreeMap<String, Fit> {
    value
        .and_then(|v| serde_json::from_str(&v).ok())
        .unwrap_or_default()
}

/// The `cosmetics` column: a JSON list of ids (NULL or broken = none).
pub(crate) fn cosmetics_column(value: Option<String>) -> Vec<String> {
    value
        .and_then(|v| serde_json::from_str(&v).ok())
        .unwrap_or_default()
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
            cosmetics: vec!["halo".into()],
            fit: [(
                "halo".to_owned(),
                Fit {
                    up: -2,
                    forward: 1,
                    mirror: false,
                },
            )]
            .into(),
        };
        s.set_look("a", &look, 2).unwrap();
        assert_eq!(s.look("a").unwrap(), look);
        let many = s
            .entries(&["a".into(), "b".into(), "zzz".into()], 2)
            .unwrap();
        assert_eq!(many.len(), 1);
        // Name updates keep the look.
        s.touch("a", "Alicia", 3).unwrap();
        assert_eq!(s.look("a").unwrap(), look);
    }

    #[test]
    fn a_moderator_can_release_a_claimed_offline_name() {
        let s = Store::memory().unwrap();
        s.touch("o", "Steve", 1).unwrap();
        // Nothing to release before it is claimed.
        assert!(!s.release_key("o").unwrap());
        s.set_key_hash("o", "h1").unwrap();
        assert!(s.release_key("o").unwrap());
        assert_eq!(s.key_hash("o").unwrap(), None);
        // The next launcher claims it with its own key; the profile (looks) stays.
        s.set_key_hash("o", "h2").unwrap();
        assert_eq!(s.key_hash("o").unwrap().as_deref(), Some("h2"));
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
