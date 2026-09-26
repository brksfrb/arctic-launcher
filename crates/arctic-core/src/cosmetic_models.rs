//! 3D cosmetics from the Arctic server: the catalog, and each item's
//! Blockbench (Bedrock) geometry, parsed with the same limits the game
//! uses. The launcher draws them on the skin preview; the game wears them.

use serde::Deserialize;

use crate::net::agent;
use crate::{Error, Result};

const MAX_ITEMS: usize = 500;
const MAX_BONES: usize = 64;
const MAX_CUBES: usize = 256;
const MAX_COORD: f32 = 256.0;
const MAX_TEXTURE: u32 = 512;
const MAX_JSON_BYTES: u64 = 256 * 1024;
const MAX_CATALOG_BYTES: u64 = 1024 * 1024;

/// A cosmetic anyone can wear (one per slot).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub slot: String,
    /// Content hashes of the geometry (JSON) and texture (PNG).
    pub model: String,
    pub texture: String,
    #[serde(default)]
    pub animation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Emote {
    pub id: String,
    pub name: String,
    pub animation: String,
    pub length: f64,
    #[serde(rename = "loop", default)]
    pub looping: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Catalog {
    #[serde(default)]
    pub cosmetics: Vec<Item>,
    #[serde(default)]
    pub emotes: Vec<Emote>,
}

/// The catalog; items with bad ids or hashes are dropped.
pub fn catalog(base: &str) -> Result<Catalog> {
    let mut resp = agent().get(&format!("{base}/v1/cosmetics")).call()?;
    let mut catalog: Catalog = resp
        .body_mut()
        .with_config()
        .limit(MAX_CATALOG_BYTES)
        .read_json()?;
    catalog.cosmetics.retain(|i| {
        is_id(&i.id)
            && is_id(&i.slot)
            && is_hash(&i.model)
            && is_hash(&i.texture)
            && i.animation.as_deref().is_none_or(is_hash)
    });
    catalog.cosmetics.truncate(MAX_ITEMS);
    catalog
        .emotes
        .retain(|e| is_id(&e.id) && is_hash(&e.animation));
    catalog.emotes.truncate(MAX_ITEMS);
    Ok(catalog)
}

/// A geometry or animation file by hash.
pub fn asset(base: &str, hash: &str) -> Result<Vec<u8>> {
    if !is_hash(hash) {
        return Err(Error::Other("bad asset hash".into()));
    }
    let mut resp = agent().get(&format!("{base}/v1/assets/{hash}")).call()?;
    Ok(resp
        .body_mut()
        .with_config()
        .limit(MAX_JSON_BYTES)
        .read_to_vec()?)
}

fn is_id(s: &str) -> bool {
    (1..=32).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

fn is_hash(s: &str) -> bool {
    s.len() == 40
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// A box-UV cube (Bedrock coordinates: y up, feet at 0).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Cube {
    pub origin: [f32; 3],
    pub size: [f32; 3],
    pub uv: [f32; 2],
    #[serde(default)]
    pub inflate: f32,
    #[serde(default)]
    pub mirror: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Bone {
    pub name: String,
    #[serde(default)]
    pub parent: Option<String>,
    pub pivot: [f32; 3],
    /// Degrees.
    #[serde(default)]
    pub rotation: [f32; 3],
    #[serde(default)]
    pub cubes: Vec<Cube>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Geometry {
    pub texture_width: u32,
    pub texture_height: u32,
    pub bones: Vec<Bone>,
}

impl Geometry {
    /// Parse and check a Bedrock geometry file (the same rules as the game).
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        #[derive(Deserialize)]
        struct File {
            #[serde(rename = "minecraft:geometry")]
            geometry: Vec<Entry>,
        }
        #[derive(Deserialize)]
        struct Entry {
            description: Description,
            bones: Vec<Bone>,
        }
        #[derive(Deserialize)]
        struct Description {
            texture_width: u32,
            texture_height: u32,
        }
        let bad = |why: &str| Error::Other(format!("cosmetic model: {why}"));
        let file: File = serde_json::from_slice(bytes).map_err(|e| bad(&e.to_string()))?;
        let entry = file
            .geometry
            .into_iter()
            .next()
            .ok_or_else(|| bad("no geometry"))?;
        let side = |s: u32| (1..=MAX_TEXTURE).contains(&s) && s.is_power_of_two();
        if !side(entry.description.texture_width) || !side(entry.description.texture_height) {
            return Err(bad("texture size"));
        }
        let bones = entry.bones;
        let cubes: usize = bones.iter().map(|b| b.cubes.len()).sum();
        if bones.is_empty() || bones.len() > MAX_BONES || cubes == 0 || cubes > MAX_CUBES {
            return Err(bad("too many or too few bones or cubes"));
        }
        let in_range = |v: &[f32]| v.iter().all(|x| x.is_finite() && x.abs() <= MAX_COORD);
        for b in &bones {
            let ok = in_range(&b.pivot)
                && b.rotation.iter().all(|r| r.is_finite() && r.abs() <= 360.0)
                && b.cubes.iter().all(|c| {
                    in_range(&c.origin)
                        && in_range(&c.size)
                        && c.size.iter().all(|s| *s >= 0.0)
                        && in_range(&c.uv)
                        && c.inflate.is_finite()
                        && c.inflate.abs() <= 16.0
                });
            if !ok {
                return Err(bad("numbers out of range"));
            }
            if let Some(parent) = &b.parent
                && !bones.iter().any(|o| &o.name == parent)
            {
                return Err(bad("missing parent bone"));
            }
        }
        // Parents must lead back to a root.
        for b in &bones {
            let mut at = b;
            for _ in 0..=bones.len() {
                match &at.parent {
                    None => break,
                    Some(p) => at = bones.iter().find(|o| &o.name == p).expect("checked above"),
                }
            }
            if at.parent.is_some() {
                return Err(bad("bone parents loop"));
            }
        }
        Ok(Self {
            texture_width: entry.description.texture_width,
            texture_height: entry.description.texture_height,
            bones,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HALO: &str = r#"{"minecraft:geometry":[{"description":{"texture_width":32,"texture_height":32},
        "bones":[{"name":"head","pivot":[0,24,0],"cubes":[{"origin":[-4,34,-5],"size":[8,1,1],"uv":[0,0]}]}]}]}"#;

    #[test]
    fn parses_blockbench_geometry() {
        let g = Geometry::parse(HALO.as_bytes()).unwrap();
        assert_eq!(g.bones[0].cubes[0].size, [8.0, 1.0, 1.0]);
        assert_eq!(g.texture_width, 32);
    }

    #[test]
    fn refuses_what_the_game_refuses() {
        let loops = r#"{"minecraft:geometry":[{"description":{"texture_width":16,"texture_height":16},
            "bones":[{"name":"a","parent":"b","pivot":[0,0,0],"cubes":[{"origin":[0,0,0],"size":[1,1,1],"uv":[0,0]}]},
                     {"name":"b","parent":"a","pivot":[0,0,0]}]}]}"#;
        assert!(Geometry::parse(loops.as_bytes()).is_err());
        let per_face = HALO.replace(
            r#""uv":[0,0]"#,
            r#""uv":{"north":{"uv":[0,0],"uv_size":[1,1]}}"#,
        );
        assert!(Geometry::parse(per_face.as_bytes()).is_err());
        let huge = HALO.replace("[8,1,1]", "[1e9,1,1]");
        assert!(Geometry::parse(huge.as_bytes()).is_err());
        assert!(Geometry::parse(HALO.replace("32", "20").as_bytes()).is_err());
    }

    #[test]
    fn ids_and_hashes() {
        assert!(is_id("frost_wings") && !is_id("Frost") && !is_id("../x"));
        assert!(is_hash(&"a".repeat(40)) && !is_hash(&"A".repeat(40)));
    }
}
