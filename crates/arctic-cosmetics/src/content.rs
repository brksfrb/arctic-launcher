//! 3D cosmetics and emotes offered to everyone, made in Blockbench:
//!
//! ```text
//! assets/cosmetics.json            [{ "id", "name", "slot" }]
//! assets/cosmetics/<id>.geo.json   Bedrock geometry (bones and cubes)
//! assets/cosmetics/<id>.png        its texture
//! assets/cosmetics/<id>.glow.png   optional: parts drawn fullbright (same size as the texture)
//! assets/cosmetics/<id>.animation.json   optional idle animation
//! assets/cosmetics/<id>.glb        optional sculpted mesh (see the arctic-mesh crate)
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
pub const SLOTS: [&str; 6] = ["head", "face", "back", "body", "shoulders", "arms"];
/// Largest geometry or animation file accepted.
pub const MAX_JSON_BYTES: usize = 512 * 1024;
/// Most bones and cubes in one cosmetic (clients enforce the same).
pub const MAX_BONES: usize = 128;
pub const MAX_CUBES: usize = 512;
/// Largest cosmetic texture side.
pub const MAX_TEXTURE: u32 = 1024;
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
    /// Optional second texture of the same size: its opaque pixels are drawn fullbright.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub glow: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub animation: Option<String>,
    /// A sculpted mesh version (`/v1/assets/<hash>`, a .glb): clients that
    /// can draw meshes use it; the rest draw this cosmetic's cuboid model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mesh: Option<String>,
}

/// A cosmetic that exists only as a mesh. It is listed apart from the
/// cuboid cosmetics (`meshes` in the catalog), so clients that cannot draw
/// meshes never see it and keep parsing the rest as before: it is simply not
/// shown to them, and nothing else takes its place.
#[derive(Debug, Clone, Serialize)]
pub struct MeshCosmetic {
    pub id: String,
    pub name: String,
    pub slot: String,
    /// Content hash of the `.glb`: `/v1/assets/<hash>`.
    pub mesh: String,
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
    /// The player rig it needs: 1, the six vanilla parts; 2, the expressive rig (a `root` or
    /// `torso` bone), which only clients that ask for it get (older ones would play it wrong).
    #[serde(skip)]
    pub rig: u8,
}

/// The newest player rig an emote can need.
pub const EXPRESSIVE_RIG: u8 = 2;

/// The rig an emote's animation needs (see [`Emote::rig`]).
fn rig_of(json: &serde_json::Value) -> u8 {
    let bones = json
        .get("animations")
        .and_then(|a| a.as_object())
        .and_then(|a| a.values().next())
        .and_then(|a| a.get("bones"))
        .and_then(|b| b.as_object());
    match bones {
        Some(b) if b.contains_key("root") || b.contains_key("torso") => EXPRESSIVE_RIG,
        _ => 1,
    }
}

#[derive(Debug, Default)]
pub struct Content {
    pub cosmetics: Vec<Cosmetic>,
    pub meshes: Vec<MeshCosmetic>,
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
            // A sculpted version, if there is one (checked completely: what a client
            // would otherwise allocate for is refused here).
            let mesh = match std::fs::read(at("glb")) {
                Ok(bytes) => {
                    check_mesh(&bytes).map_err(|e| format!("cosmetic {} (mesh): {e}", entry.id))?;
                    Some(content.add_file(bytes))
                }
                Err(_) => None,
            };
            if mesh.is_some() && !at("geo.json").exists() {
                content.meshes.push(MeshCosmetic {
                    id: entry.id,
                    name: entry.name,
                    slot: entry.slot,
                    mesh: mesh.unwrap_or_default(),
                });
                continue;
            }
            let geo = read_json(&at("geo.json"))?;
            let size = check_geometry(&geo).map_err(|e| format!("cosmetic {}: {e}", entry.id))?;
            let png =
                std::fs::read(at("png")).map_err(|e| format!("{}: {e}", at("png").display()))?;
            let texture = check_texture(&png, Some(size))
                .map_err(|e| format!("cosmetic {}: {e}", entry.id))?;
            let glow = match std::fs::read(at("glow.png")) {
                Ok(bytes) => {
                    let hash = check_texture(&bytes, Some(size))
                        .map_err(|e| format!("cosmetic {} (glow): {e}", entry.id))?;
                    content.textures.push((hash.clone(), bytes));
                    Some(hash)
                }
                Err(_) => None,
            };
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
                glow,
                animation,
                mesh,
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
            let rig = rig_of(&json);
            let animation = content.add_file(bytes);
            content.emotes.push(Emote {
                id: entry.id,
                name: entry.name,
                animation,
                length,
                looping,
                rig,
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

    pub fn emote(&self, id: &str) -> Option<&Emote> {
        self.emotes.iter().find(|e| e.id == id)
    }

    /// A valid set to wear: known ids, one per slot.
    pub fn check_worn(&self, ids: &[String]) -> Result<Vec<String>, String> {
        let mut slots: Vec<&str> = Vec::new();
        let mut out = Vec::new();
        for id in ids {
            let (slot, id) = self
                .cosmetics
                .iter()
                .map(|c| (c.slot.as_str(), c.id.as_str()))
                .chain(self.meshes.iter().map(|m| (m.slot.as_str(), m.id.as_str())))
                .find(|(_, i)| i == id)
                .ok_or_else(|| format!("unknown cosmetic {id}"))?;
            if slots.contains(&slot) {
                return Err(format!("two cosmetics for the {slot} slot"));
            }
            slots.push(slot);
            out.push(id.to_owned());
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

/// Bedrock geometry within the limits clients accept; returns the texture
/// size it declares, which the PNG must match.
fn check_geometry(file: &JsonFile) -> Result<(u32, u32), String> {
    let geometry = file
        .1
        .get("minecraft:geometry")
        .and_then(|g| g.as_array())
        .and_then(|g| g.first())
        .ok_or("no minecraft:geometry")?;
    let side = |key: &str| -> Result<u32, String> {
        let n = geometry
            .pointer(&format!("/description/{key}"))
            .and_then(serde_json::Value::as_u64)
            .ok_or(format!("description.{key} is missing"))?;
        u32::try_from(n)
            .ok()
            .filter(|s| (1..=MAX_TEXTURE).contains(s) && s.is_power_of_two())
            .ok_or(format!(
                "{key} is {n}; sides must be powers of two up to {MAX_TEXTURE}"
            ))
    };
    let (tex_w, tex_h) = (side("texture_width")?, side("texture_height")?);
    let bones = geometry
        .get("bones")
        .and_then(|b| b.as_array())
        .ok_or("no bones")?;
    if bones.is_empty() || bones.len() > MAX_BONES {
        return Err(format!("needs 1 to {MAX_BONES} bones"));
    }
    let mut cubes = 0;
    for bone in bones {
        let name = bone.get("name").and_then(|n| n.as_str()).unwrap_or("?");
        for cube in bone
            .get("cubes")
            .and_then(|c| c.as_array())
            .map_or(&[][..], Vec::as_slice)
        {
            cubes += 1;
            check_cube(cube, tex_w, tex_h).map_err(|e| format!("bone {name}: {e}"))?;
        }
    }
    if cubes == 0 || cubes > MAX_CUBES {
        return Err(format!("needs 1 to {MAX_CUBES} cubes"));
    }
    Ok((tex_w, tex_h))
}

/// One cube: its numbers are finite and every texture rectangle it uses
/// lies inside the texture (a cube that reads past the edge shows garbage).
fn check_cube(cube: &serde_json::Value, tex_w: u32, tex_h: u32) -> Result<(), String> {
    let nums = |key: &str| -> Result<Option<Vec<f64>>, String> {
        match cube.get(key) {
            None => Ok(None),
            Some(v) => {
                let a = v
                    .as_array()
                    .filter(|a| a.len() == 3)
                    .ok_or(format!("{key} must be [x, y, z]"))?;
                a.iter()
                    .map(|n| n.as_f64().filter(|n| n.abs() <= 256.0))
                    .collect::<Option<Vec<_>>>()
                    .map(Some)
                    .ok_or(format!("{key} needs numbers within ±256"))
            }
        }
    };
    let size = nums("size")?.ok_or("a cube needs a size")?;
    nums("origin")?.ok_or("a cube needs an origin")?;
    nums("pivot")?;
    nums("rotation")?;
    let uv = cube.get("uv").ok_or("a cube needs uv")?;
    let in_texture = |u: f64, v: f64, w: f64, h: f64, what: &str| -> Result<(), String> {
        let (x0, x1) = (u.min(u + w), u.max(u + w));
        let (y0, y1) = (v.min(v + h), v.max(v + h));
        if x0 < 0.0 || y0 < 0.0 || x1 > f64::from(tex_w) || y1 > f64::from(tex_h) {
            return Err(format!(
                "{what} reaches outside the {tex_w}×{tex_h} texture (u {x0}..{x1}, v {y0}..{y1})"
            ));
        }
        Ok(())
    };
    if let Some(box_uv) = uv.as_array() {
        let (u, v) = match box_uv.as_slice() {
            [u, v] => u.as_f64().zip(v.as_f64()),
            _ => None,
        }
        .ok_or("uv must be [u, v]")?;
        let (w, h, d) = (size[0], size[1], size[2]);
        in_texture(u, v, 2.0 * (w + d), d + h, "box UV")?;
    } else if let Some(faces) = uv.as_object() {
        if faces.is_empty() {
            return Err("uv has no faces".into());
        }
        for (name, face) in faces {
            if !["north", "south", "east", "west", "up", "down"].contains(&name.as_str()) {
                return Err(format!("unknown face {name:?}"));
            }
            let pair = |key: &str| -> Result<(f64, f64), String> {
                match face.get(key).and_then(|a| a.as_array()).map(Vec::as_slice) {
                    Some([a, b]) => a.as_f64().zip(b.as_f64()),
                    _ => None,
                }
                .ok_or(format!("{name}: {key} must be [x, y]"))
            };
            let ((u, v), (w, h)) = (pair("uv")?, pair("uv_size")?);
            in_texture(u, v, w, h, &format!("face {name}"))?;
        }
    } else {
        return Err("uv must be [u, v] or a list of faces".into());
    }
    Ok(())
}

/// A mesh cosmetic's `.glb`: the whole subset check, and every embedded
/// texture decoded (so a damaged one is refused here, not on players' PCs).
fn check_mesh(bytes: &[u8]) -> Result<(), String> {
    let mesh = arctic_mesh::parse(bytes)?;
    for (i, png_bytes) in mesh.images.iter().enumerate() {
        let mut reader = png::Decoder::new(std::io::Cursor::new(png_bytes))
            .read_info()
            .map_err(|_| format!("texture {i} is not a PNG"))?;
        let size = reader
            .output_buffer_size()
            .ok_or_else(|| format!("texture {i} is too large"))?;
        let mut buf = vec![0; size];
        reader
            .next_frame(&mut buf)
            .map_err(|_| format!("texture {i} is damaged"))?;
    }
    Ok(())
}

/// PNG with power-of-two sides up to `MAX_TEXTURE` (and, when given, the
/// size the geometry declares); returns its hash.
fn check_texture(png: &[u8], expect: Option<(u32, u32)>) -> Result<String, String> {
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
    if let Some((ew, eh)) = expect
        && (w, h) != (ew, eh)
    {
        return Err(format!(
            "texture is {w}×{h} but the model declares {ew}×{eh}"
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
        "bones":[{"name":"head","pivot":[0,24,0],"cubes":[{"origin":[-4,33,-4],"size":[4,1,4],"uv":[0,0]}]}]}]}"#;
    const ANIM: &str = r#"{"format_version":"1.8.0","animations":{"animation.wave":{"loop":false,"animation_length":2.0,
        "bones":{"rightArm":{"rotation":{"0.0":[0,0,0],"1.0":[0,0,-150]}}}}}}"#;

    #[test]
    fn the_bundled_cosmetics_and_emotes_load() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        let content = Content::load(&dir).unwrap();
        assert!(content.cosmetics.len() >= 17, "{}", content.cosmetics.len());
        assert!(content.emotes.len() >= 6, "{}", content.emotes.len());
    }

    #[test]
    fn expressive_emotes_are_told_apart() {
        let six: serde_json::Value = serde_json::from_str(ANIM).unwrap();
        assert_eq!(rig_of(&six), 1);
        let rig: serde_json::Value = serde_json::from_str(
            r#"{"animations":{"a":{"animation_length":1,"bones":{"root":{"position":{"0":[0,0,0]}}}}}}"#,
        )
        .unwrap();
        assert_eq!(rig_of(&rig), EXPRESSIVE_RIG);
    }

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
            br#"{"minecraft:geometry":[{"description":{"texture_width":16,"texture_height":16},"bones":[]}]}"#,
        );
        write(d, "cosmetics/halo.png", &crate::images::test_png(16, 16));
        assert!(Content::load(d).unwrap_err().contains("bones"));
        write(d, "cosmetics/halo.geo.json", GEO.as_bytes());
        write(d, "cosmetics/halo.png", &crate::images::test_png(20, 16));
        assert!(Content::load(d).unwrap_err().contains("powers of two"));
    }

    fn geo_with_cube(cube: &str) -> JsonFile {
        let text = format!(
            r#"{{"minecraft:geometry":[{{"description":{{"texture_width":64,"texture_height":32}},
            "bones":[{{"name":"back","pivot":[0,24,0],"cubes":[{cube}]}}]}}]}}"#
        );
        JsonFile(
            text.clone().into_bytes(),
            serde_json::from_str(&text).unwrap(),
        )
    }

    #[test]
    fn per_face_uv_and_cube_rotation_are_checked() {
        let ok = r#"{"origin":[0,0,0],"size":[16,16,1],"pivot":[0,8,0],"rotation":[0,20,0],
            "uv":{"north":{"uv":[0,0],"uv_size":[32,32]},"south":{"uv":[32,0],"uv_size":[-32,32]}}}"#;
        assert_eq!(check_geometry(&geo_with_cube(ok)), Ok((64, 32)));
        let past_edge = ok.replace("[32,32]},\"south", "[80,32]},\"south");
        assert!(
            check_geometry(&geo_with_cube(&past_edge))
                .unwrap_err()
                .contains("outside")
        );
        let bad_face = ok.replace("north", "front");
        assert!(
            check_geometry(&geo_with_cube(&bad_face))
                .unwrap_err()
                .contains("unknown face")
        );
        let bad_box = r#"{"origin":[0,0,0],"size":[20,16,16],"uv":[0,0]}"#;
        assert!(
            check_geometry(&geo_with_cube(bad_box))
                .unwrap_err()
                .contains("box UV")
        );
    }

    /// Whatever the kit last wrote (tools/out, not committed) must load: run
    /// the example scripts, then this test, to check the kit and the server agree.
    #[test]
    fn the_kit_output_loads() {
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/tools/out");
        let Ok(dirs) = std::fs::read_dir(&out) else {
            return;
        };
        for dir in dirs.flatten().filter(|d| d.path().is_dir()) {
            let id = dir.file_name().to_string_lossy().into_owned();
            let tmp = tempfile::tempdir().unwrap();
            let target = tmp.path().join("cosmetics");
            std::fs::create_dir_all(&target).unwrap();
            for file in std::fs::read_dir(dir.path()).unwrap().flatten() {
                if !file.file_name().to_string_lossy().ends_with(".preview.png") {
                    std::fs::copy(file.path(), target.join(file.file_name())).unwrap();
                }
            }
            write(
                tmp.path(),
                "cosmetics.json",
                format!(r#"[{{"id":"{id}","name":"Kit","slot":"back"}}]"#).as_bytes(),
            );
            let c = Content::load(tmp.path()).unwrap_or_else(|e| panic!("{id}: {e}"));
            assert_eq!(c.cosmetics.len(), 1, "{id}");
        }
    }

    #[test]
    fn mesh_cosmetics_are_listed_apart_and_can_fall_back_to_cuboids() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        write(
            d,
            "cosmetics.json",
            br#"[{"id":"wings","name":"Wings","slot":"back"},{"id":"halo","name":"Halo","slot":"head"}]"#,
        );
        // Wings exist only as a mesh; the halo has both a mesh and a cuboid version.
        write(d, "cosmetics/wings.glb", &arctic_mesh::sample_glb());
        write(d, "cosmetics/halo.glb", &arctic_mesh::sample_glb());
        write(d, "cosmetics/halo.geo.json", GEO.as_bytes());
        write(d, "cosmetics/halo.png", &crate::images::test_png(16, 16));
        let c = Content::load(d).unwrap();
        assert_eq!(c.meshes.len(), 1);
        assert_eq!(c.meshes[0].id, "wings");
        assert_eq!(c.cosmetics.len(), 1);
        assert!(
            c.cosmetics[0].mesh.is_some(),
            "the halo's mesh is offered next to its cuboids"
        );
        assert!(c.file(&c.meshes[0].mesh).unwrap().starts_with(b"glTF"));
        // Both kinds can be worn, one per slot.
        assert_eq!(
            c.check_worn(&["wings".into(), "halo".into()]).unwrap(),
            ["wings", "halo"]
        );
        assert!(c.check_worn(&["wings".into(), "wings".into()]).is_err());
        // A broken mesh stops the server with the file's name in the message.
        write(d, "cosmetics/wings.glb", b"glTF-but-not-really");
        assert!(Content::load(d).unwrap_err().contains("wings (mesh)"));
    }

    #[test]
    fn glow_texture_must_match_the_model() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        write(
            d,
            "cosmetics.json",
            br#"[{"id":"halo","name":"Halo","slot":"head"}]"#,
        );
        write(d, "cosmetics/halo.geo.json", GEO.as_bytes());
        write(d, "cosmetics/halo.png", &crate::images::test_png(16, 16));
        write(
            d,
            "cosmetics/halo.glow.png",
            &crate::images::test_png(16, 16),
        );
        let c = Content::load(d).unwrap();
        assert!(c.cosmetics[0].glow.is_some());
        write(
            d,
            "cosmetics/halo.glow.png",
            &crate::images::test_png(32, 32),
        );
        assert!(Content::load(d).unwrap_err().contains("glow"));
    }

    #[test]
    fn no_lists_means_no_content() {
        let dir = tempfile::tempdir().unwrap();
        let c = Content::load(dir.path()).unwrap();
        assert!(c.cosmetics.is_empty() && c.emotes.is_empty());
    }
}
