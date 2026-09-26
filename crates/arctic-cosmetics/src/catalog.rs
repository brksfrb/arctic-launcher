//! Preset capes offered to everyone, loaded from `catalog.json` plus one
//! PNG per preset. Presets are ordinary textures once registered; players
//! can just as well upload their own.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::images::{self, Kind};
use crate::store::Store;

#[derive(Debug, Clone, Deserialize)]
struct Item {
    id: String,
    name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Preset {
    pub id: String,
    pub name: String,
    /// Content hash, fetch it from `/v1/textures/<hash>.png`.
    pub texture: String,
    #[serde(skip)]
    png: Vec<u8>,
}

pub struct Catalog {
    presets: Vec<Preset>,
}

impl Catalog {
    /// Reads `dir/catalog.json` and `dir/capes/<id>.png`.
    pub fn load(dir: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(dir.join("catalog.json"))
            .map_err(|e| format!("catalog.json: {e}"))?;
        let items: Vec<Item> =
            serde_json::from_str(&text).map_err(|e| format!("catalog.json: {e}"))?;
        let mut presets = Vec::with_capacity(items.len());
        for item in items {
            let path = dir.join("capes").join(format!("{}.png", item.id));
            let png = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let texture =
                images::check(&png, Kind::Cape).map_err(|e| format!("{}: {e}", path.display()))?;
            presets.push(Preset {
                id: item.id,
                name: item.name,
                texture,
                png,
            });
        }
        Ok(Self { presets })
    }

    /// Make the preset textures downloadable.
    pub fn register(&self, store: &Store, now: u64) -> rusqlite::Result<()> {
        for p in &self.presets {
            store.put_texture(&p.texture, &p.png, now)?;
        }
        Ok(())
    }

    pub fn presets(&self) -> &[Preset] {
        &self.presets
    }

    /// Texture hash of a preset by id.
    pub fn preset(&self, id: &str) -> Option<&str> {
        self.presets
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.texture.as_str())
    }
}
