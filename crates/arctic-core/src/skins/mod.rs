//! Skins: a local library of skin files per profile, plus the Mojang API
//! for changing the account's real skin and cape (see [`api`]).

pub mod api;

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::storage::{load_json, save_json};
use crate::{Error, Result};

/// Largest skin file accepted (real skins are a few KB).
pub const MAX_SKIN_BYTES: usize = 256 * 1024;
const LIBRARY_FILE: &str = "library.json";

/// Arm width: Classic (Steve, 4 px) or Slim (Alex, 3 px).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Variant {
    #[default]
    Classic,
    Slim,
}

impl Variant {
    pub fn label(self) -> &'static str {
        match self {
            Variant::Classic => "Classic",
            Variant::Slim => "Slim",
        }
    }

    /// Value the Mojang API expects.
    pub fn api_name(self) -> &'static str {
        match self {
            Variant::Classic => "classic",
            Variant::Slim => "slim",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkinEntry {
    pub id: String,
    pub name: String,
    pub variant: Variant,
    /// Unix seconds.
    pub added: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Library {
    pub skins: Vec<SkinEntry>,
}

/// A decoded skin: 64×64 RGBA (legacy 64×32 skins are expanded).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkinImage {
    pub rgba: Vec<u8>,
    /// Legacy 64×32 source (no second layer on body and limbs).
    pub legacy: bool,
    /// The outer arm columns were transparent in the file (slim arms).
    slim_arms: bool,
}

impl SkinImage {
    pub const SIZE: usize = 64;

    pub fn pixel(&self, x: usize, y: usize) -> [u8; 4] {
        let i = (y * Self::SIZE + x) * 4;
        [
            self.rgba[i],
            self.rgba[i + 1],
            self.rgba[i + 2],
            self.rgba[i + 3],
        ]
    }

    /// Slim skins leave the outer arm columns transparent.
    pub fn guess_variant(&self) -> Variant {
        if self.slim_arms {
            Variant::Slim
        } else {
            Variant::Classic
        }
    }
}

/// Decode and check a skin PNG (64×64 or legacy 64×32).
pub fn decode(png_bytes: &[u8]) -> Result<SkinImage> {
    let bad = |e: String| Error::Other(format!("Not a Minecraft skin: {e}"));
    if png_bytes.len() > MAX_SKIN_BYTES {
        return Err(bad("the file is too large".into()));
    }
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png_bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|e| bad(e.to_string()))?;
    let mut buf = vec![0; reader.output_buffer_size().unwrap_or(0)];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| bad(e.to_string()))?;
    let (w, h) = (info.width as usize, info.height as usize);
    if w != 64 || (h != 64 && h != 32) {
        return Err(bad(format!("it is {w}×{h}, skins are 64×64")));
    }
    let channels = info.color_type.samples();
    if !(3..=4).contains(&channels) {
        return Err(bad("unsupported color format".into()));
    }
    let mut rgba = vec![0u8; 64 * 64 * 4];
    for y in 0..h {
        for x in 0..w {
            let s = (y * w + x) * channels;
            let d = (y * 64 + x) * 4;
            rgba[d..d + 3].copy_from_slice(&buf[s..s + 3]);
            rgba[d + 3] = if channels == 4 { buf[s + 3] } else { 255 };
        }
    }
    let legacy = h == 32;
    let alpha = |x: usize, y: usize| rgba[(y * 64 + x) * 4 + 3];
    let slim_arms = !legacy && alpha(54, 20) == 0 && alpha(55, 31) == 0;
    if legacy {
        expand_legacy(&mut rgba);
        clear_opaque_hat(&mut rgba);
    }
    // Like the game: base layers are always opaque.
    for (x0, y0, x1, y1) in [(0, 0, 32, 16), (0, 16, 64, 32), (16, 48, 48, 64)] {
        for y in y0..y1 {
            for x in x0..x1 {
                rgba[(y * 64 + x) * 4 + 3] = 255;
            }
        }
    }
    Ok(SkinImage {
        rgba,
        legacy,
        slim_arms,
    })
}

/// Old skins often fill the hat layer with solid color; the game treats a
/// fully opaque hat layer as absent.
fn clear_opaque_hat(rgba: &mut [u8]) {
    let hat = |f: &mut dyn FnMut(usize)| {
        for y in 0..16 {
            for x in 32..64 {
                f((y * 64 + x) * 4 + 3);
            }
        }
    };
    let mut opaque = true;
    hat(&mut |i| opaque &= rgba[i] == 255);
    if opaque {
        hat(&mut |i| rgba[i] = 0);
    }
}

/// Legacy skins have one arm and one leg; the left limbs mirror the right.
fn expand_legacy(rgba: &mut [u8]) {
    // (source x, source y, dest x, dest y) for a 16×16 limb block.
    for (sx, sy, dx, dy) in [(0, 16, 16, 48), (40, 16, 32, 48)] {
        for y in 0..16 {
            for x in 0..16 {
                let s = ((sy + y) * 64 + sx + x) * 4;
                let d = ((dy + y) * 64 + dx + x) * 4;
                for c in 0..4 {
                    rgba[d + c] = rgba[s + c];
                }
            }
        }
    }
}

impl Library {
    pub fn dir(profile_root: &Path) -> PathBuf {
        profile_root.join("skins")
    }

    pub fn load(dir: &Path) -> Result<Self> {
        Ok(load_json::<Self>(&dir.join(LIBRARY_FILE))?.unwrap_or_default())
    }

    pub fn save(&self, dir: &Path) -> Result<()> {
        save_json(&dir.join(LIBRARY_FILE), self)
    }

    pub fn png_path(dir: &Path, id: &str) -> PathBuf {
        dir.join(format!("{id}.png"))
    }

    /// The library skin with the same pixels as `png_bytes`, if any.
    pub fn find_same(&self, dir: &Path, png_bytes: &[u8]) -> Option<&SkinEntry> {
        let wanted = decode(png_bytes).ok()?;
        self.find_image(dir, &wanted)
    }

    fn find_image(&self, dir: &Path, wanted: &SkinImage) -> Option<&SkinEntry> {
        self.skins.iter().find(|entry| {
            Self::read_png(dir, &entry.id)
                .ok()
                .and_then(|bytes| decode(&bytes).ok())
                .is_some_and(|image| image.rgba == wanted.rgba)
        })
    }

    /// Validate and store a skin file; returns the new entry, or the
    /// existing one when the library already has this skin.
    pub fn add(
        &mut self,
        dir: &Path,
        name: &str,
        png_bytes: &[u8],
        variant: Option<Variant>,
    ) -> Result<SkinEntry> {
        let image = decode(png_bytes)?;
        if let Some(existing) = self.find_image(dir, &image) {
            return Ok(existing.clone());
        }
        fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
        let id = uuid::Uuid::new_v4().simple().to_string();
        let path = Self::png_path(dir, &id);
        fs::write(&path, png_bytes).map_err(|e| Error::io(&path, e))?;
        let name = name.trim();
        let entry = SkinEntry {
            id,
            name: if name.is_empty() {
                "Skin".into()
            } else {
                name.chars().take(48).collect()
            },
            variant: variant.unwrap_or_else(|| image.guess_variant()),
            added: crate::auth::now_secs(),
        };
        self.skins.insert(0, entry.clone());
        self.save(dir)?;
        Ok(entry)
    }

    pub fn remove(&mut self, dir: &Path, id: &str) -> Result<()> {
        self.skins.retain(|s| s.id != id);
        let path = Self::png_path(dir, id);
        if path.exists() {
            fs::remove_file(&path).map_err(|e| Error::io(&path, e))?;
        }
        self.save(dir)
    }

    pub fn update(
        &mut self,
        dir: &Path,
        id: &str,
        change: impl FnOnce(&mut SkinEntry),
    ) -> Result<()> {
        if let Some(entry) = self.skins.iter_mut().find(|s| s.id == id) {
            change(entry);
        }
        self.save(dir)
    }

    pub fn read_png(dir: &Path, id: &str) -> Result<Vec<u8>> {
        let path = Self::png_path(dir, id);
        fs::read(&path).map_err(|e| Error::io(&path, e))
    }
}

#[cfg(test)]
mod tests;
