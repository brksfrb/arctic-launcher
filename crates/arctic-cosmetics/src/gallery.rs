//! Community skin gallery: players share skins, everyone can browse and
//! use them. Reported items are hidden after a few reports.

use rusqlite::{OptionalExtension, params};
use serde::Serialize;

use crate::store::Store;

/// Items hidden after this many different players report them.
pub const HIDE_AFTER_REPORTS: i64 = 3;
/// Most items one player can have in the gallery.
pub const MAX_PER_AUTHOR: i64 = 50;
pub const MAX_PAGE: usize = 48;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Item {
    pub id: String,
    pub name: String,
    /// Texture hash.
    pub texture: String,
    pub model: String,
    pub author: String,
    pub downloads: i64,
    pub created: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    Popular,
    New,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Page {
    pub items: Vec<Item>,
    pub total: i64,
}

/// Why a publish was refused.
#[derive(Debug, PartialEq, Eq)]
pub enum PublishError {
    TooMany,
    Duplicate,
    Db(String),
}

impl Store {
    pub fn gallery_page(
        &self,
        sort: Sort,
        query: &str,
        offset: usize,
        limit: usize,
    ) -> rusqlite::Result<Page> {
        let conn = self.conn();
        let pattern = format!("%{}%", query.trim().replace(['%', '_'], ""));
        let order = match sort {
            Sort::Popular => "downloads DESC, created DESC",
            Sort::New => "created DESC",
        };
        let sql = format!(
            "SELECT id, name, texture, model, author_name, downloads, created FROM gallery
             WHERE hidden = 0 AND (name LIKE ?1 OR author_name LIKE ?1)
             ORDER BY {order} LIMIT ?2 OFFSET ?3"
        );
        let mut stmt = conn.prepare(&sql)?;
        let items = stmt
            .query_map(
                params![pattern, limit.min(MAX_PAGE) as i64, offset as i64],
                |r| {
                    Ok(Item {
                        id: r.get(0)?,
                        name: r.get(1)?,
                        texture: r.get(2)?,
                        model: r.get(3)?,
                        author: r.get(4)?,
                        downloads: r.get(5)?,
                        created: r.get(6)?,
                    })
                },
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let total = conn.query_row(
            "SELECT COUNT(*) FROM gallery WHERE hidden = 0 AND (name LIKE ?1 OR author_name LIKE ?1)",
            [&pattern],
            |r| r.get(0),
        )?;
        Ok(Page { items, total })
    }

    /// Gallery items are unique by look (their pixels), whoever shares them.
    /// Older databases get the column, and their duplicates are merged into
    /// the earliest share.
    pub(crate) fn migrate_gallery_looks(&self) -> rusqlite::Result<()> {
        let has_column = {
            let conn = self.conn();
            let mut stmt = conn.prepare("SELECT name FROM pragma_table_info('gallery')")?;
            let names = stmt
                .query_map([], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            names.iter().any(|n| n == "look")
        };
        if !has_column {
            self.conn()
                .execute_batch("ALTER TABLE gallery ADD COLUMN look TEXT")?;
        }
        let missing: Vec<(String, String)> = {
            let conn = self.conn();
            let mut stmt = conn.prepare("SELECT id, texture FROM gallery WHERE look IS NULL")?;
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        for (id, texture) in missing {
            let look = self.look_of(&texture)?;
            self.conn().execute(
                "UPDATE gallery SET look = ?1 WHERE id = ?2",
                params![look, id],
            )?;
        }
        let conn = self.conn();
        let copies: Vec<String> = {
            let mut stmt = conn.prepare(
                "SELECT id FROM gallery g WHERE EXISTS (
                     SELECT 1 FROM gallery o WHERE o.look = g.look
                     AND (o.created < g.created OR (o.created = g.created AND o.id < g.id)))",
            )?;
            stmt.query_map([], |r| r.get(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        for id in &copies {
            conn.execute("DELETE FROM gallery WHERE id = ?1", [id])?;
            conn.execute("DELETE FROM gallery_reports WHERE item = ?1", [id])?;
        }
        if !copies.is_empty() {
            log::info!("gallery: merged {} duplicate skins", copies.len());
        }
        conn.execute_batch("CREATE UNIQUE INDEX IF NOT EXISTS gallery_look ON gallery (look)")
    }

    /// The look key of a stored texture (its hash if it can't be decoded).
    fn look_of(&self, texture: &str) -> rusqlite::Result<String> {
        Ok(self
            .texture(texture)?
            .and_then(|png| crate::images::pixel_key(&png))
            .unwrap_or_else(|| texture.to_owned()))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn gallery_publish(
        &self,
        id: &str,
        texture: &str,
        model: &str,
        name: &str,
        author_uuid: &str,
        author_name: &str,
        now: u64,
    ) -> Result<(), PublishError> {
        let db = |e: rusqlite::Error| PublishError::Db(e.to_string());
        let look = self.look_of(texture).map_err(db)?;
        let conn = self.conn();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM gallery WHERE author_uuid = ?1",
                [author_uuid],
                |r| r.get(0),
            )
            .map_err(db)?;
        if count >= MAX_PER_AUTHOR {
            return Err(PublishError::TooMany);
        }
        let inserted = conn
            .execute(
                "INSERT OR IGNORE INTO gallery (id, texture, model, name, author_uuid, author_name, created, look)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![id, texture, model, name, author_uuid, author_name, now as i64, look],
            )
            .map_err(db)?;
        if inserted == 0 {
            return Err(PublishError::Duplicate);
        }
        Ok(())
    }

    /// Texture hash of a visible item, counting the download.
    pub fn gallery_take(&self, id: &str) -> rusqlite::Result<Option<String>> {
        let conn = self.conn();
        let texture: Option<String> = conn
            .query_row(
                "SELECT texture FROM gallery WHERE id = ?1 AND hidden = 0",
                [id],
                |r| r.get(0),
            )
            .optional()?;
        if texture.is_some() {
            conn.execute(
                "UPDATE gallery SET downloads = downloads + 1 WHERE id = ?1",
                [id],
            )?;
        }
        Ok(texture)
    }

    /// One report per player; hides the item once enough players report it.
    pub fn gallery_report(&self, id: &str, reporter: &str) -> rusqlite::Result<bool> {
        let conn = self.conn();
        let exists: Option<i64> = conn
            .query_row("SELECT 1 FROM gallery WHERE id = ?1", [id], |r| r.get(0))
            .optional()?;
        if exists.is_none() {
            return Ok(false);
        }
        let new = conn.execute(
            "INSERT OR IGNORE INTO gallery_reports (item, reporter) VALUES (?1, ?2)",
            params![id, reporter],
        )?;
        if new > 0 {
            conn.execute(
                "UPDATE gallery SET reports = reports + 1,
                 hidden = CASE WHEN reports + 1 >= ?2 THEN 1 ELSE hidden END WHERE id = ?1",
                params![id, HIDE_AFTER_REPORTS],
            )?;
        }
        Ok(true)
    }

    /// Delete an item: its author may, and so may an admin (`author` = None).
    pub fn gallery_delete(&self, id: &str, author: Option<&str>) -> rusqlite::Result<bool> {
        let conn = self.conn();
        let n = match author {
            Some(a) => conn.execute(
                "DELETE FROM gallery WHERE id = ?1 AND author_uuid = ?2",
                params![id, a],
            )?,
            None => conn.execute("DELETE FROM gallery WHERE id = ?1", [id])?,
        };
        if n > 0 {
            conn.execute("DELETE FROM gallery_reports WHERE item = ?1", [id])?;
        }
        Ok(n > 0)
    }
}

/// A display name: trimmed, printable, 1–32 characters.
pub fn clean_name(name: &str) -> Option<String> {
    let cleaned: String = name.chars().filter(|c| !c.is_control()).take(32).collect();
    let cleaned = cleaned.trim().to_owned();
    (!cleaned.is_empty()).then_some(cleaned)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Store {
        let s = Store::memory().unwrap();
        s.touch("a", "Alice", 1).unwrap();
        s
    }

    #[test]
    fn publish_browse_download() {
        let s = store();
        s.gallery_publish("1", "t1", "slim", "Ice Knight", "a", "Alice", 10)
            .unwrap();
        s.gallery_publish("2", "t2", "classic", "Frost", "a", "Alice", 20)
            .unwrap();
        assert_eq!(
            s.gallery_publish("3", "t1", "slim", "Again", "a", "Alice", 30),
            Err(PublishError::Duplicate)
        );
        assert_eq!(s.gallery_take("1").unwrap().as_deref(), Some("t1"));
        let popular = s.gallery_page(Sort::Popular, "", 0, 10).unwrap();
        assert_eq!(popular.items[0].id, "1");
        let newest = s.gallery_page(Sort::New, "", 0, 10).unwrap();
        assert_eq!(newest.items[0].id, "2");
        let found = s.gallery_page(Sort::New, "knight", 0, 10).unwrap();
        assert_eq!(found.total, 1);
        // LIKE wildcards in queries are ignored, not interpreted.
        assert_eq!(s.gallery_page(Sort::New, "%", 0, 10).unwrap().total, 2);
    }

    #[test]
    fn one_entry_per_look_whoever_shares_it() {
        let s = store();
        s.touch("b", "Bob", 1).unwrap();
        s.gallery_publish("1", "t1", "classic", "Mine", "a", "Alice", 10)
            .unwrap();
        assert_eq!(
            s.gallery_publish("2", "t1", "classic", "Also mine", "b", "Bob", 20),
            Err(PublishError::Duplicate)
        );
    }

    #[test]
    fn migration_merges_existing_duplicates() {
        let s = store();
        let png = crate::images::test_png(64, 64);
        s.put_texture("h1", &png, 1).unwrap();
        s.put_texture("h2", &png, 1).unwrap();
        {
            let conn = s.conn();
            conn.execute_batch("DROP INDEX gallery_look").unwrap();
            for (id, tex, t) in [("old", "h1", 10), ("new", "h2", 20)] {
                conn.execute(
                    "INSERT INTO gallery (id, texture, model, name, author_uuid, author_name, created)
                     VALUES (?1, ?2, 'classic', 'x', 'a', 'Alice', ?3)",
                    params![id, tex, t],
                )
                .unwrap();
            }
        }
        s.migrate_gallery_looks().unwrap();
        let page = s.gallery_page(Sort::New, "", 0, 10).unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].id, "old");
    }

    #[test]
    fn reports_hide_after_threshold_once_per_player() {
        let s = store();
        s.gallery_publish("1", "t1", "classic", "Bad", "a", "Alice", 10)
            .unwrap();
        for _ in 0..5 {
            s.gallery_report("1", "r1").unwrap();
        }
        assert_eq!(s.gallery_page(Sort::New, "", 0, 10).unwrap().total, 1);
        s.gallery_report("1", "r2").unwrap();
        s.gallery_report("1", "r3").unwrap();
        assert_eq!(s.gallery_page(Sort::New, "", 0, 10).unwrap().total, 0);
        assert_eq!(s.gallery_take("1").unwrap(), None);
    }

    #[test]
    fn authors_and_admins_can_delete() {
        let s = store();
        s.gallery_publish("1", "t1", "classic", "Mine", "a", "Alice", 10)
            .unwrap();
        assert!(!s.gallery_delete("1", Some("someone-else")).unwrap());
        assert!(s.gallery_delete("1", Some("a")).unwrap());
        s.gallery_publish("2", "t2", "classic", "Mine", "a", "Alice", 10)
            .unwrap();
        assert!(s.gallery_delete("2", None).unwrap());
    }

    #[test]
    fn names_are_cleaned() {
        assert_eq!(
            clean_name("  Ice\u{0}Knight  ").as_deref(),
            Some("IceKnight")
        );
        assert_eq!(clean_name("   "), None);
        assert_eq!(clean_name(&"x".repeat(40)).unwrap().len(), 32);
    }
}
