//! Cosmetics on offer, loaded from `catalog.json` plus one PNG per item.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub name: String,
    /// Everyone owns free items.
    #[serde(default)]
    pub free: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PublicItem {
    pub id: String,
    pub name: String,
    pub kind: &'static str,
    pub free: bool,
    pub texture: String,
}

pub struct Catalog {
    items: Vec<Item>,
    textures: HashMap<String, Vec<u8>>,
}

impl Catalog {
    /// Reads `dir/catalog.json` and `dir/capes/<id>.png`.
    pub fn load(dir: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(dir.join("catalog.json"))
            .map_err(|e| format!("catalog.json: {e}"))?;
        let items: Vec<Item> =
            serde_json::from_str(&text).map_err(|e| format!("catalog.json: {e}"))?;
        let mut textures = HashMap::new();
        for item in &items {
            if !item
                .id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
            {
                return Err(format!("bad cosmetic id {:?}", item.id));
            }
            let path = dir.join("capes").join(format!("{}.png", item.id));
            let png = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            textures.insert(item.id.clone(), png);
        }
        Ok(Self { items, textures })
    }

    pub fn public(&self) -> Vec<PublicItem> {
        self.items
            .iter()
            .map(|i| PublicItem {
                id: i.id.clone(),
                name: i.name.clone(),
                kind: "cape",
                free: i.free,
                texture: format!("/v1/textures/{}.png", i.id),
            })
            .collect()
    }

    pub fn texture(&self, id: &str) -> Option<Vec<u8>> {
        self.textures.get(id).cloned()
    }

    /// Items this player may equip. Paid items would be granted per player.
    pub fn owned_by(&self, _uuid: &str) -> Vec<String> {
        self.items
            .iter()
            .filter(|i| i.free)
            .map(|i| i.id.clone())
            .collect()
    }
}
