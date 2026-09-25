//! Player-head avatars (8×8 face + hat layer) for the account UI.
//!
//! Microsoft accounts: skin URL from Mojang's session server → PNG → face.
//! Offline accounts (or any failure): the classic Steve face. Faces are
//! cached as raw RGBA under `cache/faces/` and refreshed daily.

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::Deserialize;

use crate::net::{agent, get_json};
use crate::storage::DataDirs;
use crate::{Error, Result};

const SESSION_URL: &str = "https://sessionserver.mojang.com/session/minecraft/profile";
const FACE_PIXELS: usize = 8 * 8;
const CACHE_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);
const MAX_SKIN_BYTES: u64 = 1024 * 1024;

/// 8×8 RGBA pixels, row-major.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Face(pub [[u8; 4]; FACE_PIXELS]);

impl Face {
    pub fn pixel(&self, x: usize, y: usize) -> [u8; 4] {
        self.0[y * 8 + x]
    }

    fn to_bytes(&self) -> Vec<u8> {
        self.0.iter().flatten().copied().collect()
    }

    fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != FACE_PIXELS * 4 {
            return None;
        }
        let (chunks, _) = bytes.as_chunks::<4>();
        let mut px = [[0u8; 4]; FACE_PIXELS];
        px.copy_from_slice(chunks);
        Some(Self(px))
    }
}

/// The default Steve face (stylised from the vanilla skin).
pub fn default_face() -> Face {
    const H: [u8; 4] = [0x2f, 0x20, 0x0f, 255]; // hair
    const S: [u8; 4] = [0xb5, 0x86, 0x6c, 255]; // skin
    const D: [u8; 4] = [0xa0, 0x74, 0x5c, 255]; // skin shade
    const W: [u8; 4] = [0xff, 0xff, 0xff, 255]; // eye white
    const E: [u8; 4] = [0x52, 0x3d, 0x89, 255]; // iris
    const N: [u8; 4] = [0x8e, 0x5a, 0x40, 255]; // nose
    const M: [u8; 4] = [0x6a, 0x40, 0x30, 255]; // mouth/beard
    #[rustfmt::skip]
    let rows: [[[u8; 4]; 8]; 8] = [
        [H, H, H, H, H, H, H, H],
        [H, H, H, H, H, H, H, H],
        [H, S, S, S, S, S, S, H],
        [S, S, D, S, S, D, S, S],
        [S, W, E, S, S, E, W, S],
        [S, D, S, N, N, S, D, S],
        [S, S, M, S, S, M, S, S],
        [S, S, M, M, M, M, S, S],
    ];
    let mut px = [[0u8; 4]; FACE_PIXELS];
    for (i, p) in rows.iter().flatten().enumerate() {
        px[i] = *p;
    }
    Face(px)
}

fn cache_path(dirs: &DataDirs, uuid: &str) -> PathBuf {
    dirs.cache().join("faces").join(format!("{uuid}.rgba"))
}

/// Cached face if present (any age), for instant display at startup.
pub fn cached_face(dirs: &DataDirs, uuid: &str) -> Option<Face> {
    Face::from_bytes(&fs::read(cache_path(dirs, uuid)).ok()?)
}

/// True when the cached face is missing or older than a day.
pub fn needs_refresh(dirs: &DataDirs, uuid: &str) -> bool {
    fs::metadata(cache_path(dirs, uuid))
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .is_none_or(|age| age > CACHE_MAX_AGE)
}

#[derive(Deserialize)]
struct SessionProfile {
    properties: Vec<Property>,
}

#[derive(Deserialize)]
struct Property {
    name: String,
    value: String,
}

#[derive(Deserialize)]
struct TexturesPayload {
    textures: Textures,
}

#[derive(Deserialize)]
struct Textures {
    #[serde(rename = "SKIN")]
    skin: Option<SkinRef>,
}

#[derive(Deserialize)]
struct SkinRef {
    url: String,
}

/// Download the current skin of a Microsoft account and cache its face.
pub fn fetch_face(dirs: &DataDirs, uuid: &str) -> Result<Face> {
    let profile: SessionProfile = get_json(&format!("{SESSION_URL}/{uuid}"))?;
    let textures = profile
        .properties
        .iter()
        .find(|p| p.name == "textures")
        .ok_or_else(|| Error::Other("profile has no textures".into()))?;
    let decoded = STANDARD
        .decode(&textures.value)
        .map_err(|e| Error::Other(format!("bad textures payload: {e}")))?;
    let payload: TexturesPayload = serde_json::from_slice(&decoded)?;
    let face = match payload.textures.skin {
        // Texture URLs are published as http://; the host supports https.
        Some(skin) => {
            let url = skin.url.replacen("http://", "https://", 1);
            let mut resp = agent().get(&url).call()?;
            let png = resp
                .body_mut()
                .with_config()
                .limit(MAX_SKIN_BYTES)
                .read_to_vec()?;
            face_from_skin(&png)?
        }
        None => default_face(),
    };
    let path = cache_path(dirs, uuid);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(&path, face.to_bytes());
    Ok(face)
}

/// Face (8,8)-(16,16) with the hat overlay (40,8)-(48,16) alpha-blended on top.
pub fn face_from_skin(png_bytes: &[u8]) -> Result<Face> {
    let bad = |e: String| Error::Other(format!("unreadable skin: {e}"));
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png_bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|e| bad(e.to_string()))?;
    let mut buf = vec![0; reader.output_buffer_size().unwrap_or(0)];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| bad(e.to_string()))?;
    let channels = info.color_type.samples();
    let width = info.width as usize;
    if info.width < 48 || info.height < 16 || !(3..=4).contains(&channels) {
        return Err(bad(format!("{}x{} skin", info.width, info.height)));
    }
    let at = |x: usize, y: usize| -> [u8; 4] {
        let i = (y * width + x) * channels;
        let a = if channels == 4 { buf[i + 3] } else { 255 };
        [buf[i], buf[i + 1], buf[i + 2], a]
    };
    let mut px = [[0u8; 4]; FACE_PIXELS];
    for y in 0..8 {
        for x in 0..8 {
            px[y * 8 + x] = blend(at(8 + x, 8 + y), at(40 + x, 8 + y));
        }
    }
    Ok(Face(px))
}

fn blend(base: [u8; 4], over: [u8; 4]) -> [u8; 4] {
    let a = over[3] as u32;
    let mix = |b: u8, o: u8| ((o as u32 * a + b as u32 * (255 - a)) / 255) as u8;
    [
        mix(base[0], over[0]),
        mix(base[1], over[1]),
        mix(base[2], over[2]),
        255,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skin_png(face: [u8; 4], hat: [u8; 4]) -> Vec<u8> {
        let (w, h) = (64u32, 64u32);
        let mut data = vec![0u8; (w * h * 4) as usize];
        for y in 8..16 {
            for x in 8..16 {
                let i = ((y * w + x) * 4) as usize;
                data[i..i + 4].copy_from_slice(&face);
            }
            for x in 40..48 {
                let i = ((y * w + x) * 4) as usize;
                data[i..i + 4].copy_from_slice(&hat);
            }
        }
        let mut out = Vec::new();
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(&data).unwrap();
        out
    }

    #[test]
    fn extracts_face_with_transparent_hat() {
        let face = face_from_skin(&skin_png([10, 20, 30, 255], [0, 0, 0, 0])).unwrap();
        assert_eq!(face.pixel(0, 0), [10, 20, 30, 255]);
        assert_eq!(face.pixel(7, 7), [10, 20, 30, 255]);
    }

    #[test]
    fn opaque_hat_covers_face() {
        let face = face_from_skin(&skin_png([10, 20, 30, 255], [200, 100, 0, 255])).unwrap();
        assert_eq!(face.pixel(3, 3), [200, 100, 0, 255]);
    }

    #[test]
    fn rejects_tiny_images() {
        let mut out = Vec::new();
        let mut enc = png::Encoder::new(&mut out, 4, 4);
        enc.set_color(png::ColorType::Rgba);
        enc.write_header()
            .unwrap()
            .write_image_data(&[0; 64])
            .unwrap();
        assert!(face_from_skin(&out).is_err());
    }

    #[test]
    fn cache_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let dirs = DataDirs::new(dir.path());
        assert!(cached_face(&dirs, "u").is_none());
        assert!(needs_refresh(&dirs, "u"));
        let path = cache_path(&dirs, "u");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, default_face().to_bytes()).unwrap();
        assert_eq!(cached_face(&dirs, "u"), Some(default_face()));
        assert!(!needs_refresh(&dirs, "u"));
    }
}
