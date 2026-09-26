//! 3D cosmetics and emotes offered to everyone, made in Blockbench:
//!
//! ```text
//! assets/cosmetics.json            [{ "id", "name", "slot" }]
//! assets/cosmetics/<id>.geo.json   Bedrock geometry (bones and cubes)
//! assets/cosmetics/<id>.png        its texture
//! assets/cosmetics/<id>.animation.json   optional idle animation
//! assets/emotes.json               [{ "id", "name" }]
//! assets/emotes/<id>.animation.json      Bedrock animation of the player's bones
//! ```
//!
//! Files are checked when the server starts and served by content hash, so
//! clients cache them forever and a broken file never reaches a player.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

use crate::store::Store;

/// Where a cosmetic sits; a player wears at most one per slot.
pub const SLOTS: [&str; 5] = ["head", "face", "back", "body", "shoulders"];
/// Largest geometry or animation file accepted.
pub const MAX_JSON_BYTES: usize = 256 * 1024;
/// Most bones and cubes in one cosmetic (clients enforce the same).
pub const MAX_BONES: usize = 64;
pub const MAX_CUBES: usize = 256;
/// Largest cosmetic texture side.
pub const MAX_TEXTURE: u32 = 512;
/// Longest emote, in seconds.
pub const MAX_EMOTE_SECS: f64 = 15.0;

#[derive(Debug, Clone, Deserialize)]
struct Entry {
    id: String,
    name: String,
    #[serde(default)]
    slot: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Cosmetic {
    pub id: String,
    pub name: String,
    pub slot: String,
    /// Content hashes: `/v1/assets/<hash>` (JSON) and `/v1/textures/<hash>.png`.
    pub model: String,
    pub texture: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub animation: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Emote {
    pub id: String,
    pub name: String,
    pub animation: String,
    /// Seconds (for looping emotes, how long one plays).
    pub length: f64,
    #[serde(rename = "loop")]
    pub looping: bool,
}

#[derive(Debug, Default)]
pub struct Content {
    pub cosmetics: Vec<Cosmetic>,
    pub emotes: Vec<Emote>,
    /// JSON files by content hash.
    files: HashMap<String, Vec<u8>>,
    /// Cosmetic textures by hash (registered as ordinary textures).
    textures: Vec<(String, Vec<u8>)>,
}

impl Content {
    /// Load everything under `dir`; missing lists just mean none yet.
    pub fn load(dir: &Path) -> Result<Self, String> {
        let mut content = Self::default();
        for entry in read_list(&dir.join("cosmetics.json"))? {
            let at = |ext: &str| dir.join("cosmetics").join(format!("{}.{ext}", entry.id));
            if !SLOTS.contains(&entry.slot.as_str()) {
                return Err(format!(
                    "cosmetic {}: slot must be one of {}",
                    entry.id,
                    SLOTS.join(", ")
                ));
            }
            check_id(&entry.id)?;
            let geo = read_json(&at("geo.json"))?;
            check_geometry(&geo).map_err(|e| format!("cosmetic {}: {e}", entry.id))?;
            let png =
                std::fs::read(at("png")).map_err(|e| format!("{}: {e}", at("png").display()))?;
            let texture = check_texture(&png).map_err(|e| format!("cosmetic {}: {e}", entry.id))?;
            let animation = match std::fs::read(at("animation.json")) {
                Ok(bytes) => {
                    let json = parse_json(&bytes, &at("animation.json"))?;
                    animation_info(&json).map_err(|e| format!("cosmetic {}: {e}", entry.id))?;
                    Some(content.add_file(bytes))
                }
                Err(_) => None,
            };
            let model = content.add_file(geo.0);
            content.textures.push((texture.clone(), png));
            content.cosmetics.push(Cosmetic {
                id: entry.id,
                name: entry.name,
                slot: entry.slot,
                model,
                texture,
                animation,
            });
        }
        for entry in read_list(&dir.join("emotes.json"))? {
            check_id(&entry.id)?;
            let path = dir
                .join("emotes")
                .join(format!("{}.animation.json", entry.id));
            let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let json = parse_json(&bytes, &path)?;
            let (length, looping) =
                animation_info(&json).map_err(|e| format!("emote {}: {e}", entry.id))?;
            let animation = content.add_file(bytes);
            content.emotes.push(Emote {
                id: entry.id,
                name: entry.name,
                animation,
                length,
                looping,
            });
        }
        Ok(content)
    }

    fn add_file(&mut self, bytes: Vec<u8>) -> String {
        let hash = hex::encode(Sha1::digest(&bytes));
        self.files.insert(hash.clone(), bytes);
        hash
    }

    /// Make cosmetic textures downloadable like any other texture.
    pub fn register(&self, store: &Store, now: u64) -> rusqlite::Result<()> {
        for (hash, png) in &self.textures {
            store.put_texture(hash, png, now)?;
        }
        Ok(())
    }

    pub fn file(&self, hash: &str) -> Option<&[u8]> {
        self.files.get(hash).map(Vec::as_slice)
    }

    pub fn cosmetic(&self, id: &str) -> Option<&Cosmetic> {
        self.cosmetics.iter().find(|c| c.id == id)
    }

    pub fn emote(&self, id: &str) -> Option<&Emote> {
        self.emotes.iter().find(|e| e.id == id)
    }

    /// A valid set to wear: known ids, one per slot.
    pub fn check_worn(&self, ids: &[String]) -> Result<Vec<String>, String> {
        let mut slots = Vec::new();
        let mut out = Vec::new();
        for id in ids {
            let c = self
                .cosmetic(id)
                .ok_or_else(|| format!("unknown cosmetic {id}"))?;
            if slots.contains(&c.slot) {
                return Err(format!("two cosmetics for the {} slot", c.slot));
            }
            slots.push(c.slot.clone());
            out.push(c.id.clone());
        }
        Ok(out)
    }
}

fn read_list(path: &Path) -> Result<Vec<Entry>, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

fn check_id(id: &str) -> Result<(), String> {
    let ok = !id.is_empty()
        && id.len() <= 32
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-');
    if ok {
        Ok(())
    } else {
        Err(format!("id {id:?}: use 1-32 of a-z, 0-9, _ and -"))
    }
}

/// Parsed JSON plus its bytes (the bytes are what's served).
struct JsonFile(Vec<u8>, serde_json::Value);

fn read_json(path: &Path) -> Result<JsonFile, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let value = parse_json(&bytes, path)?;
    Ok(JsonFile(bytes, value))
}

fn parse_json(bytes: &[u8], path: &Path) -> Result<serde_json::Value, String> {
    if bytes.len() > MAX_JSON_BYTES {
        return Err(format!(
            "{}: larger than {MAX_JSON_BYTES} bytes",
            path.display()
        ));
    }
    serde_json::from_slice(bytes).map_err(|e| format!("{}: {e}", path.display()))
}

/// Bedrock geometry within the limits clients accept.
fn check_geometry(file: &JsonFile) -> Result<(), String> {
    let geometry = file
        .1
        .get("minecraft:geometry")
        .and_then(|g| g.as_array())
        .and_then(|g| g.first())
        .ok_or("no minecraft:geometry")?;
    let bones = geometry
        .get("bones")
        .and_then(|b| b.as_array())
        .ok_or("no bones")?;
    if bones.is_empty() || bones.len() > MAX_BONES {
        return Err(format!("needs 1 to {MAX_BONES} bones"));
    }
    let cubes: usize = bones
        .iter()
        .map(|b| {
            b.get("cubes")
                .and_then(|c| c.as_array())
                .map_or(0, Vec::len)
        })
        .sum();
    if cubes == 0 || cubes > MAX_CUBES {
        return Err(format!("needs 1 to {MAX_CUBES} cubes"));
    }
    Ok(())
}

/// PNG with power-of-two sides up to `MAX_TEXTURE`; returns its hash.
fn check_texture(png: &[u8]) -> Result<String, String> {
    let reader = png::Decoder::new(std::io::Cursor::new(png))
        .read_info()
        .map_err(|_| "texture isn't a PNG")?;
    let (w, h) = (reader.info().width, reader.info().height);
    let side_ok = |s: u32| (1..=MAX_TEXTURE).contains(&s) && s.is_power_of_two();
    if !side_ok(w) || !side_ok(h) {
        return Err(format!(
            "texture is {w}×{h}; sides must be powers of two up to {MAX_TEXTURE}"
        ));
    }
    Ok(hex::encode(Sha1::digest(png)))
}

/// (length in seconds, loops) of the first animation in a Bedrock file.
fn animation_info(json: &serde_json::Value) -> Result<(f64, bool), String> {
    let animation = json
        .get("animations")
        .and_then(|a| a.as_object())
        .and_then(|a| a.values().next())
        .ok_or("no animations")?;
    let length = animation
        .get("animation_length")
        .and_then(serde_json::Value::as_f64)
        .ok_or("animation_length is missing")?;
    if !(length > 0.0 && length <= MAX_EMOTE_SECS) {
        return Err(format!(
            "animation_length must be above 0 and at most {MAX_EMOTE_SECS}"
        ));
    }
    let looping = animation
        .get("loop")
        .is_some_and(|l| l.as_bool() == Some(true));
    Ok((length, looping))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, rel: &str, bytes: &[u8]) {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    const GEO: &str = r#"{"format_version":"1.12.0","minecraft:geometry":[{"description":{"identifier":"geometry.halo","texture_width":16,"texture_height":16},
        "bones":[{"name":"head","pivot":[0,24,0],"cubes":[{"origin":[-4,33,-4],"size":[8,1,8],"uv":[0,0]}]}]}]}"#;
    const ANIM: &str = r#"{"format_version":"1.8.0","animations":{"animation.wave":{"loop":false,"animation_length":2.0,
        "bones":{"rightArm":{"rotation":{"0.0":[0,0,0],"1.0":[0,0,-150]}}}}}}"#;

    #[test]
    fn loads_cosmetics_and_emotes() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        write(
            d,
            "cosmetics.json",
            br#"[{"id":"halo","name":"Halo","slot":"head"}]"#,
        );
        write(d, "cosmetics/halo.geo.json", GEO.as_bytes());
        write(d, "cosmetics/halo.png", &crate::images::test_png(16, 16));
        write(d, "emotes.json", br#"[{"id":"wave","name":"Wave"}]"#);
        write(d, "emotes/wave.animation.json", ANIM.as_bytes());
        let c = Content::load(d).unwrap();
        assert_eq!(c.cosmetics.len(), 1);
        assert!(c.file(&c.cosmetics[0].model).is_some());
        assert_eq!(c.emotes[0].length, 2.0);
        assert!(!c.emotes[0].looping);
        assert_eq!(c.check_worn(&["halo".into()]).unwrap(), ["halo"]);
        assert!(
            c.check_worn(&["halo".into(), "halo".into()]).is_err(),
            "one per slot"
        );
        assert!(c.check_worn(&["nope".into()]).is_err());
    }

    #[test]
    fn broken_content_stops_the_server_with_a_reason() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        write(
            d,
            "cosmetics.json",
            br#"[{"id":"halo","name":"Halo","slot":"tail"}]"#,
        );
        assert!(Content::load(d).unwrap_err().contains("slot"));
        write(
            d,
            "cosmetics.json",
            br#"[{"id":"halo","name":"Halo","slot":"head"}]"#,
        );
        write(
            d,
            "cosmetics/halo.geo.json",
            br#"{"minecraft:geometry":[{"bones":[]}]}"#,
        );
        write(d, "cosmetics/halo.png", &crate::images::test_png(16, 16));
        assert!(Content::load(d).unwrap_err().contains("bones"));
        write(d, "cosmetics/halo.geo.json", GEO.as_bytes());
        write(d, "cosmetics/halo.png", &crate::images::test_png(20, 16));
        assert!(Content::load(d).unwrap_err().contains("powers of two"));
    }

    #[test]
    fn no_lists_means_no_content() {
        let dir = tempfile::tempdir().unwrap();
        let c = Content::load(dir.path()).unwrap();
        assert!(c.cosmetics.is_empty() && c.emotes.is_empty());
    }
}
