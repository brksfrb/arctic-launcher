//! SQLite storage: players and what they have equipped.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{Connection, OptionalExtension, params};

pub struct Store {
    db: Mutex<Connection>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Equipped {
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
                 cape TEXT,
                 updated INTEGER NOT NULL
             );",
        )?;
        Ok(Self {
            db: Mutex::new(conn),
        })
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.db.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Record a verified sign-in (keeps the latest name).
    pub fn touch(&self, uuid: &str, name: &str, now: u64) -> rusqlite::Result<()> {
        self.conn().execute(
            "INSERT INTO players (uuid, name, cape, updated) VALUES (?1, ?2, NULL, ?3)
             ON CONFLICT(uuid) DO UPDATE SET name = excluded.name",
            params![uuid, name, now as i64],
        )?;
        Ok(())
    }

    pub fn equipped(&self, uuid: &str) -> rusqlite::Result<Equipped> {
        let cape = self
            .conn()
            .query_row("SELECT cape FROM players WHERE uuid = ?1", [uuid], |r| {
                r.get::<_, Option<String>>(0)
            })
            .optional()?
            .flatten();
        Ok(Equipped { cape })
    }

    pub fn set_cape(&self, uuid: &str, cape: Option<&str>, now: u64) -> rusqlite::Result<()> {
        self.conn().execute(
            "UPDATE players SET cape = ?2, updated = ?3 WHERE uuid = ?1",
            params![uuid, cape, now as i64],
        )?;
        Ok(())
    }

    /// Equipped cosmetics for many players; players without any are left out.
    pub fn equipped_many(&self, uuids: &[String]) -> rusqlite::Result<Vec<(String, Equipped)>> {
        let conn = self.conn();
        let mut stmt =
            conn.prepare_cached("SELECT cape FROM players WHERE uuid = ?1 AND cape IS NOT NULL")?;
        let mut out = Vec::new();
        for uuid in uuids {
            let cape: Option<String> = stmt.query_row([uuid], |r| r.get(0)).optional()?;
            if let Some(cape) = cape {
                out.push((uuid.clone(), Equipped { cape: Some(cape) }));
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equip_and_query() {
        let s = Store::memory().unwrap();
        s.touch("a", "Alice", 1).unwrap();
        s.touch("b", "Bob", 1).unwrap();
        assert_eq!(s.equipped("a").unwrap().cape, None);
        s.set_cape("a", Some("aurora"), 2).unwrap();
        assert_eq!(s.equipped("a").unwrap().cape.as_deref(), Some("aurora"));
        let many = s
            .equipped_many(&["a".into(), "b".into(), "zzz".into()])
            .unwrap();
        assert_eq!(many.len(), 1);
        assert_eq!(many[0].0, "a");
        s.set_cape("a", None, 3).unwrap();
        assert!(s.equipped_many(&["a".into()]).unwrap().is_empty());
        // Name updates keep the equipped cape.
        s.set_cape("b", Some("glacier"), 4).unwrap();
        s.touch("b", "Bobby", 5).unwrap();
        assert_eq!(s.equipped("b").unwrap().cape.as_deref(), Some("glacier"));
    }
}
