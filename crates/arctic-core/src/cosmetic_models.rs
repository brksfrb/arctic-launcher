//! 3D cosmetics from the Arctic server: the catalog, and each item's
//! Blockbench (Bedrock) geometry, parsed with the same limits the game
//! uses. The launcher draws them on the skin preview; the game wears them.

use serde::Deserialize;

pub use arctic_mesh as mesh;
pub use arctic_mesh::Mesh;

use crate::net::agent;
use crate::{Error, Result};

const MAX_ITEMS: usize = 500;
const MAX_BONES: usize = 128;
const MAX_CUBES: usize = 512;
const MAX_COORD: f32 = 256.0;
const MAX_TEXTURE: u32 = 1024;
const MAX_JSON_BYTES: u64 = 512 * 1024;
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
    /// A sculpted version (hash of a `.glb`); used instead of the cuboids
    /// when this client can draw it.
    #[serde(default)]
    pub mesh: Option<String>,
}

/// A cosmetic that exists only as a sculpted mesh.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MeshItem {
    pub id: String,
    pub name: String,
    pub slot: String,
    /// Content hash of the `.glb`.
    pub mesh: String,
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
    /// Mesh-only cosmetics (older launchers never see them).
    #[serde(default)]
    pub meshes: Vec<MeshItem>,
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
            && i.mesh.as_deref().is_none_or(is_hash)
    });
    catalog.cosmetics.truncate(MAX_ITEMS);
    catalog
        .meshes
        .retain(|m| is_id(&m.id) && is_id(&m.slot) && is_hash(&m.mesh));
    catalog.meshes.truncate(MAX_ITEMS);
    catalog
        .emotes
        .retain(|e| is_id(&e.id) && is_hash(&e.animation));
    catalog.emotes.truncate(MAX_ITEMS);
    Ok(catalog)
}

/// A mesh cosmetic's `.glb` by hash (still to be checked with `mesh::parse`).
pub fn mesh_asset(base: &str, hash: &str) -> Result<Vec<u8>> {
    if !is_hash(hash) {
        return Err(Error::Other("bad asset hash".into()));
    }
    let mut resp = agent().get(&format!("{base}/v1/assets/{hash}")).call()?;
    Ok(resp
        .body_mut()
        .with_config()
        .limit(arctic_mesh::MAX_BYTES as u64)
        .read_to_vec()?)
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

/// A cube (Bedrock coordinates: y up, feet at 0). Its texture is either
/// box UV (`uv: [u, v]`, the usual layout around the cube) or one
/// rectangle per face (`uv: {"north": {"uv": [u, v], "uv_size": [w, h]}, …}`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Cube {
    pub origin: [f32; 3],
    pub size: [f32; 3],
    pub uv: Uv,
    #[serde(default)]
    pub inflate: f32,
    #[serde(default)]
    pub mirror: bool,
    /// Turns the cube around `pivot` (degrees), like a bone's rotation.
    #[serde(default)]
    pub rotation: [f32; 3],
    #[serde(default)]
    pub pivot: Option<[f32; 3]>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum Uv {
    Box([f32; 2]),
    Faces(FaceUvs),
}

/// A face's rectangle in the texture: top-left corner and size in texels.
/// A negative size flips the face.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct FaceUv {
    pub uv: [f32; 2],
    pub uv_size: [f32; 2],
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FaceUvs {
    pub north: Option<FaceUv>,
    pub south: Option<FaceUv>,
    pub east: Option<FaceUv>,
    pub west: Option<FaceUv>,
    pub up: Option<FaceUv>,
    pub down: Option<FaceUv>,
}

/// Face order everywhere: north (-z, the front), south, east, west, up, down.
pub const FACES: usize = 6;

impl Cube {
    /// Each face's texture rectangle `(u, v, w, h)` (a negative `w` or `h`
    /// flips it), or `None` for a face that isn't drawn. Box UV gives the
    /// usual six rectangles around `(u, v)`; mirrored cubes swap east and
    /// west and flip every rectangle sideways.
    pub fn face_rects(&self) -> [Option<[f32; 4]>; FACES] {
        match &self.uv {
            Uv::Faces(f) => [&f.north, &f.south, &f.east, &f.west, &f.up, &f.down]
                .map(|o| o.map(|f| [f.uv[0], f.uv[1], f.uv_size[0], f.uv_size[1]])),
            Uv::Box([u, v]) => {
                let [w, h, d] = self.size;
                let mut r = [
                    [u + d, v + d, w, h],
                    [u + 2.0 * d + w, v + d, w, h],
                    [*u, v + d, d, h],
                    [u + d + w, v + d, d, h],
                    [u + d, *v, w, d],
                    [u + d + w, *v, w, d],
                ];
                if self.mirror {
                    r.swap(2, 3);
                    for f in &mut r {
                        *f = [f[0] + f[2], f[1], -f[2], f[3]];
                    }
                }
                r.map(Some)
            }
        }
    }
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
                        && c.face_rects()
                            .iter()
                            .flatten()
                            .all(|r| r.iter().all(|x| x.is_finite() && x.abs() <= 2.0 * MAX_TEXTURE as f32))
                        && c.rotation.iter().all(|r| r.is_finite() && r.abs() <= 360.0)
                        && c.pivot.is_none_or(|p| in_range(&p))
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

    #[test]
    fn the_bundled_cosmetics_parse() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../arctic-cosmetics/assets/cosmetics");
        let mut models = 0;
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.to_string_lossy().ends_with(".geo.json") {
                let geo = Geometry::parse(&std::fs::read(&path).unwrap())
                    .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
                assert!(!geo.bones.is_empty(), "{}", path.display());
                models += 1;
            }
        }
        assert!(models >= 17, "{models}");
    }

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
        let junk_face = HALO.replace(r#""uv":[0,0]"#, r#""uv":{"middle":{"uv":[0,0],"uv_size":[1,1]}}"#);
        assert!(Geometry::parse(junk_face.as_bytes()).is_err());
        let huge = HALO.replace("[8,1,1]", "[1e9,1,1]");
        assert!(Geometry::parse(huge.as_bytes()).is_err());
        assert!(Geometry::parse(HALO.replace("32", "20").as_bytes()).is_err());
    }

    #[test]
    fn per_face_uv_and_cube_rotation() {
        let g = HALO.replace(
            r#""uv":[0,0]"#,
            r#""uv":{"north":{"uv":[2,4],"uv_size":[8,-1]},"up":{"uv":[0,0],"uv_size":[8,1]}},"pivot":[0,34,-5],"rotation":[0,0,20]"#,
        );
        let g = Geometry::parse(g.as_bytes()).unwrap();
        let cube = &g.bones[0].cubes[0];
        let rects = cube.face_rects();
        assert_eq!(rects[0], Some([2.0, 4.0, 8.0, -1.0]));
        assert_eq!(rects[1], None);
        assert_eq!(rects[4], Some([0.0, 0.0, 8.0, 1.0]));
        assert_eq!(cube.rotation, [0.0, 0.0, 20.0]);
        assert_eq!(cube.pivot, Some([0.0, 34.0, -5.0]));
    }

    #[test]
    fn box_uv_is_the_six_usual_rectangles() {
        let g = Geometry::parse(HALO.as_bytes()).unwrap();
        let r = g.bones[0].cubes[0].face_rects();
        // size (8, 1, 1) at uv (0, 0): w=8, h=1, d=1
        assert_eq!(r[0], Some([1.0, 1.0, 8.0, 1.0]));
        assert_eq!(r[2], Some([0.0, 1.0, 1.0, 1.0]));
        assert_eq!(r[4], Some([1.0, 0.0, 8.0, 1.0]));
        let mirrored = HALO.replace(r#""uv":[0,0]"#, r#""uv":[0,0],"mirror":true"#);
        let m = Geometry::parse(mirrored.as_bytes()).unwrap().bones[0].cubes[0].face_rects();
        assert_eq!(m[2], Some([10.0, 1.0, -1.0, 1.0]));
    }

    #[test]
    fn ids_and_hashes() {
        assert!(is_id("frost_wings") && !is_id("Frost") && !is_id("../x"));
        assert!(is_hash(&"a".repeat(40)) && !is_hash(&"A".repeat(40)));
    }
}
