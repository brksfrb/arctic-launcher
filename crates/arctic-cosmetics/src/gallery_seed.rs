//! Skins the gallery starts with: `assets/gallery.json` lists them, with
//! their PNGs in `assets/gallery/`. They show as Arctic's, alongside what
//! players share, and are added once (a seed already there is left alone).

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
}

pub struct SeedSkin {
    pub name: String,
    pub model: &'static str,
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
    for entry in entries {
        if entry.file.contains(['/', '\\']) || entry.file.contains("..") {
            return Err(format!(
                "gallery.json: {:?} must be a plain file name",
                entry.file
            ));
        }
        let file = assets.join("gallery").join(&entry.file);
        let png = std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        let texture =
            images::check(&png, Kind::Skin).map_err(|e| format!("{}: {e}", file.display()))?;
        let name = crate::gallery::clean_name(&entry.name)
            .ok_or_else(|| format!("gallery.json: {} needs a name", entry.file))?;
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
        seeds.push(SeedSkin {
            name,
            model,
            texture,
            png,
        });
    }
    Ok(seeds)
}

impl Store {
    /// Add the seeds not in the gallery yet; returns how many were added.
    pub fn gallery_seed(&self, seeds: &[SeedSkin], now: u64) -> rusqlite::Result<usize> {
        let mut added = 0;
        for seed in seeds {
            self.put_texture(&seed.texture, &seed.png, now)?;
            let look = crate::images::pixel_key(&seed.png).unwrap_or_else(|| seed.texture.clone());
            let id = format!("arctic-{}", &seed.texture[..16]);
            added += self.conn().execute(
                "INSERT OR IGNORE INTO gallery (id, texture, model, name, author_uuid, author_name, created, look)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![id, seed.texture, seed.model, seed.name, AUTHOR_UUID, AUTHOR_NAME, now as i64, look],
            )?;
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

    #[test]
    fn seeds_once_and_checks_the_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("gallery")).unwrap();
        std::fs::write(dir.path().join("gallery/frosty.png"), skin_png()).unwrap();
        std::fs::write(
            dir.path().join("gallery.json"),
            r#"[{"file": "frosty.png", "name": "Frosty", "model": "slim"}]"#,
        )
        .unwrap();
        let seeds = load(dir.path()).unwrap();
        assert_eq!(seeds.len(), 1);
        let store = Store::memory().unwrap();
        assert_eq!(store.gallery_seed(&seeds, 1).unwrap(), 1);
        assert_eq!(store.gallery_seed(&seeds, 2).unwrap(), 0);

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
    fn no_seed_file_is_fine() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load(dir.path()).unwrap().is_empty());
    }

    #[test]
    fn the_bundled_gallery_loads() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        load(&dir).unwrap();
    }
}
