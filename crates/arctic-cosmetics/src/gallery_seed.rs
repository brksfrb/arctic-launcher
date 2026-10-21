//! Skins the gallery starts with: `assets/gallery.json` lists them, with
//! their PNGs in `assets/gallery/`. They show as Arctic's, alongside what
//! players share. Repainting a seed refreshes its existing gallery item.

use std::collections::HashSet;
use std::path::Path;

use rusqlite::params;
use serde::Deserialize;

use crate::images::{self, Kind};
use crate::store::Store;

/// The "author" of seeded skins.
pub const AUTHOR_UUID: &str = "arctic";
pub const AUTHOR_NAME: &str = "Arctic";

#[derive(Deserialize)]
struct Entry {
    /// File name in `assets/gallery/`.
    file: String,
    name: String,
    #[serde(default)]
    model: String,
    #[serde(default)]
    category: String,
}

pub struct SeedSkin {
    pub id: String,
    pub name: String,
    pub model: &'static str,
    pub category: &'static str,
    pub texture: String,
    pub png: Vec<u8>,
}

/// Reads `gallery.json` (none is fine: no seeds). A bad entry stops the
/// server with its name, like the other content.
pub fn load(assets: &Path) -> Result<Vec<SeedSkin>, String> {
    let path = assets.join("gallery.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(Vec::new());
    };
    let entries: Vec<Entry> =
        serde_json::from_str(&text).map_err(|e| format!("gallery.json: {e}"))?;
    let mut seeds = Vec::with_capacity(entries.len());
    let mut ids = HashSet::new();
    let mut names = HashSet::new();
    for entry in entries {
        let stem = entry.file.strip_suffix(".png").unwrap_or("");
        if stem.is_empty()
            || !stem
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        {
            return Err(format!(
                "gallery.json: {:?} must be a lowercase .png file name with letters, digits or _",
                entry.file
            ));
        }
        let id = format!("arctic-{stem}");
        if !ids.insert(id.clone()) {
            return Err(format!(
                "gallery.json: duplicate seed file {:?}",
                entry.file
            ));
        }
        let file = assets.join("gallery").join(&entry.file);
        let png = std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        let texture =
            images::check(&png, Kind::Skin).map_err(|e| format!("{}: {e}", file.display()))?;
        let name = crate::gallery::clean_name(&entry.name)
            .ok_or_else(|| format!("gallery.json: {} needs a name", entry.file))?;
        if !names.insert(name.to_lowercase()) {
            return Err(format!("gallery.json: duplicate seed name {name:?}"));
        }
        let model = match entry.model.as_str() {
            "slim" => "slim",
            "classic" | "" => "classic",
            other => {
                return Err(format!(
                    "gallery.json: {}: model {other:?} isn't classic or slim",
                    entry.file
                ));
            }
        };
        let category = match entry.category.as_str() {
            "" | "community" => "community",
            "adventure" => "adventure",
            "fantasy" => "fantasy",
            "gamer" => "gamer",
            "animals" => "animals",
            "pvp" => "pvp",
            "casual" => "casual",
            "minecraft" => "minecraft",
            other => {
                return Err(format!(
                    "gallery.json: {}: unknown category {other:?}",
                    entry.file
                ));
            }
        };
        seeds.push(SeedSkin {
            id,
            name,
            model,
            category,
            texture,
            png,
        });
    }
    Ok(seeds)
}

impl Store {
    /// Bind a curated row to its manifest file across future name and art changes.
    pub(crate) fn migrate_gallery_seed_key(&self) -> rusqlite::Result<()> {
        let conn = self.conn();
        let has_column = {
            let mut stmt = conn.prepare("SELECT name FROM pragma_table_info('gallery')")?;
            stmt.query_map([], |row| row.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?
                .iter()
                .any(|name| name == "seed_key")
        };
        if !has_column {
            conn.execute_batch("ALTER TABLE gallery ADD COLUMN seed_key TEXT")?;
        }
        conn.execute_batch(
            "CREATE UNIQUE INDEX IF NOT EXISTS gallery_seed_key ON gallery (seed_key)
             WHERE seed_key IS NOT NULL",
        )
    }

    /// Add or refresh curated skins. File-based IDs are stable across art
    /// revisions; older texture-derived IDs keep their downloads and reports.
    pub fn gallery_seed(&self, seeds: &[SeedSkin], now: u64) -> rusqlite::Result<usize> {
        let mut added = 0;
        for seed in seeds {
            self.put_texture(&seed.texture, &seed.png, now)?;
            let look = crate::images::pixel_key(&seed.png).unwrap_or_else(|| seed.texture.clone());
            let mut conn = self.conn();
            let tx = conn.transaction()?;
            // Legacy seeds used a texture hash as their ID. Match their
            // existing name once, then keep that ID as the art changes.
            let matches: Vec<(String, i64)> = {
                let mut stmt = tx.prepare(
                    "SELECT id, downloads FROM gallery
                     WHERE author_uuid = ?1 AND
                           (seed_key = ?2 OR id = ?2 OR name = ?3 OR texture = ?4)
                     ORDER BY created, id",
                )?;
                stmt.query_map(
                    params![AUTHOR_UUID, seed.id, seed.name, seed.texture],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )?
                .collect::<rusqlite::Result<_>>()?
            };
            if let Some((winner, _)) = matches.first() {
                // If an earlier art update already produced duplicate Arctic
                // seeds, retain the oldest ID and its gallery history.
                for (loser, downloads) in matches.iter().skip(1) {
                    tx.execute(
                        "INSERT OR IGNORE INTO gallery_reports (item, reporter)
                         SELECT ?1, reporter FROM gallery_reports WHERE item = ?2",
                        params![winner, loser],
                    )?;
                    tx.execute("DELETE FROM gallery_reports WHERE item = ?1", [loser])?;
                    tx.execute("DELETE FROM gallery WHERE id = ?1", [loser])?;
                    tx.execute(
                        "UPDATE gallery SET downloads = downloads + ?1 WHERE id = ?2",
                        params![downloads, winner],
                    )?;
                }
                if matches.len() > 1 {
                    tx.execute(
                        "UPDATE gallery SET reports =
                         (SELECT COUNT(*) FROM gallery_reports WHERE item = ?1) WHERE id = ?1",
                        [winner],
                    )?;
                }
                tx.execute(
                    "UPDATE gallery SET texture = ?1, look = ?2, model = ?3,
                     name = ?4, category = ?5, seed_key = ?6 WHERE id = ?7",
                    params![
                        seed.texture,
                        look,
                        seed.model,
                        seed.name,
                        seed.category,
                        seed.id,
                        winner
                    ],
                )?;
            } else {
                added += tx.execute(
                    "INSERT INTO gallery (id, texture, model, name, author_uuid, author_name, created, look, category, seed_key)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    params![seed.id, seed.texture, seed.model, seed.name, AUTHOR_UUID, AUTHOR_NAME, now as i64, look, seed.category, seed.id],
                )?;
            }
            tx.commit()?;
        }
        Ok(added)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skin_png() -> Vec<u8> {
        crate::images::test_png(64, 64)
    }

    fn skin_png_color(color: u8) -> Vec<u8> {
        let mut out = Vec::new();
        let mut enc = png::Encoder::new(&mut out, 64, 64);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().unwrap();
        writer.write_image_data(&vec![color; 64 * 64 * 4]).unwrap();
        drop(writer);
        out
    }

    #[test]
    fn seeds_once_and_checks_the_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("gallery")).unwrap();
        std::fs::write(dir.path().join("gallery/frosty.png"), skin_png()).unwrap();
        std::fs::write(
            dir.path().join("gallery.json"),
            r#"[{"file": "frosty.png", "name": "Frosty", "model": "slim", "category": "animals"}]"#,
        )
        .unwrap();
        let seeds = load(dir.path()).unwrap();
        assert_eq!(seeds.len(), 1);
        let store = Store::memory().unwrap();
        assert_eq!(store.gallery_seed(&seeds, 1).unwrap(), 1);
        assert_eq!(store.gallery_seed(&seeds, 2).unwrap(), 0);
        assert_eq!(
            store
                .gallery_page_filtered(crate::gallery::Sort::New, "", "animals", 0, 10)
                .unwrap()
                .total,
            1
        );
        assert_eq!(
            store
                .gallery_page_filtered(crate::gallery::Sort::New, "", "fantasy", 0, 10)
                .unwrap()
                .total,
            0
        );

        // Existing installs receive new collection assignments without
        // inserting a duplicate skin or resetting its gallery state.
        std::fs::write(
            dir.path().join("gallery.json"),
            r#"[{"file": "frosty.png", "name": "Frosty", "model": "slim", "category": "fantasy"}]"#,
        )
        .unwrap();
        assert_eq!(
            store.gallery_seed(&load(dir.path()).unwrap(), 3).unwrap(),
            0
        );
        assert_eq!(
            store
                .gallery_page_filtered(crate::gallery::Sort::New, "", "animals", 0, 10)
                .unwrap()
                .total,
            0
        );
        assert_eq!(
            store
                .gallery_page_filtered(crate::gallery::Sort::New, "", "fantasy", 0, 10)
                .unwrap()
                .total,
            1
        );

        // Repainting a seed updates its existing gallery item and keeps its
        // download history, instead of adding a texture-hash-named duplicate.
        store
            .conn()
            .execute(
                "UPDATE gallery SET downloads = 7 WHERE id = 'arctic-frosty'",
                [],
            )
            .unwrap();
        let old_texture = seeds[0].texture.clone();
        std::fs::write(dir.path().join("gallery/frosty.png"), skin_png_color(90)).unwrap();
        let repainted = load(dir.path()).unwrap();
        assert_ne!(repainted[0].texture, old_texture);
        assert_eq!(store.gallery_seed(&repainted, 4).unwrap(), 0);
        let (texture, downloads, model): (String, i64, String) = store
            .conn()
            .query_row(
                "SELECT texture, downloads, model FROM gallery WHERE id = 'arctic-frosty'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(texture, repainted[0].texture);
        assert_eq!(downloads, 7);
        assert_eq!(model, "slim");
        assert_eq!(
            store
                .gallery_page_filtered(crate::gallery::Sort::New, "", "", 0, 10)
                .unwrap()
                .total,
            1
        );
        std::fs::write(
            dir.path().join("gallery.json"),
            r#"[{"file": "frosty.png", "name": "Frosty", "category": "unknown"}]"#,
        )
        .unwrap();
        assert!(load(dir.path()).is_err());

        std::fs::write(
            dir.path().join("gallery.json"),
            r#"[{"file": "../x.png", "name": "X"}]"#,
        )
        .unwrap();
        assert!(load(dir.path()).is_err());
        std::fs::write(
            dir.path().join("gallery.json"),
            r#"[{"file": "missing.png", "name": "X"}]"#,
        )
        .unwrap();
        assert!(load(dir.path()).is_err());
    }

    #[test]
    fn repainted_legacy_seeds_merge_without_losing_downloads() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("gallery")).unwrap();
        std::fs::write(dir.path().join("gallery/frosty.png"), skin_png_color(90)).unwrap();
        std::fs::write(
            dir.path().join("gallery.json"),
            r#"[{"file": "frosty.png", "name": "Frosty", "model": "classic", "category": "animals"}]"#,
        ).unwrap();
        let store = Store::memory().unwrap();
        for (id, color, created, downloads) in [
            ("arctic-old-first", 50, 1, 3),
            ("arctic-old-second", 70, 2, 4),
        ] {
            let png = skin_png_color(color);
            let texture = crate::images::check(&png, Kind::Skin).unwrap();
            let look = crate::images::pixel_key(&png).unwrap();
            store.put_texture(&texture, &png, created).unwrap();
            store
                .conn()
                .execute(
                    "INSERT INTO gallery (id, texture, model, name, author_uuid, author_name,
                 created, downloads, look, category) VALUES (?1, ?2, 'slim', 'Frosty',
                 'arctic', 'Arctic', ?3, ?4, ?5, 'adventure')",
                    params![id, texture, created as i64, downloads, look],
                )
                .unwrap();
            store
                .conn()
                .execute(
                    "INSERT INTO gallery_reports (item, reporter) VALUES (?1, ?2)",
                    params![id, format!("reporter-{id}")],
                )
                .unwrap();
        }
        let seed = load(dir.path()).unwrap();
        assert_eq!(store.gallery_seed(&seed, 3).unwrap(), 0);
        let (id, texture, downloads, reports, model, category): (
            String,
            String,
            i64,
            i64,
            String,
            String,
        ) = store
            .conn()
            .query_row(
                "SELECT id, texture, downloads, reports, model, category FROM gallery",
                [],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(id, "arctic-old-first");
        assert_eq!(texture, seed[0].texture);
        assert_eq!(downloads, 7);
        assert_eq!(reports, 2);
        assert_eq!(model, "classic");
        assert_eq!(category, "animals");
        assert_eq!(
            store
                .gallery_page_filtered(crate::gallery::Sort::New, "", "", 0, 10)
                .unwrap()
                .total,
            1
        );
        std::fs::write(dir.path().join("gallery/frosty.png"), skin_png_color(120)).unwrap();
        std::fs::write(
            dir.path().join("gallery.json"),
            r#"[{"file": "frosty.png", "name": "Frosty Repaint", "model": "classic", "category": "fantasy"}]"#,
        ).unwrap();
        let renamed = load(dir.path()).unwrap();
        assert_eq!(store.gallery_seed(&renamed, 4).unwrap(), 0);
        let (id, name, downloads): (String, String, i64) = store
            .conn()
            .query_row("SELECT id, name, downloads FROM gallery", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .unwrap();
        assert_eq!(id, "arctic-old-first");
        assert_eq!(name, "Frosty Repaint");
        assert_eq!(downloads, 7);
    }

    #[test]
    fn no_seed_file_is_fine() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load(dir.path()).unwrap().is_empty());
    }

    #[test]
    fn the_bundled_gallery_loads() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        let skins = load(&dir).unwrap();
        assert_eq!(skins.len(), 121);
        for collection in [
            "adventure",
            "fantasy",
            "gamer",
            "animals",
            "pvp",
            "casual",
            "minecraft",
        ] {
            assert!(
                skins.iter().any(|skin| skin.category == collection),
                "missing {collection}"
            );
        }
        let store = Store::memory().unwrap();
        assert_eq!(store.gallery_seed(&skins, 1).unwrap(), skins.len());
        assert_eq!(store.gallery_seed(&skins, 2).unwrap(), 0);
        assert_eq!(
            store
                .gallery_page_filtered(crate::gallery::Sort::New, "", "", 0, 1)
                .unwrap()
                .total,
            skins.len() as i64,
        );
    }
}
