//! Preset capes offered to everyone, loaded from `catalog.json` plus one
//! PNG per preset. Presets are ordinary textures once registered; players
//! can just as well upload their own.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::images::{self, Kind};
use crate::store::Store;

/// Animated capes play at this many frames a second unless the catalog says otherwise.
pub const DEFAULT_FPS: u32 = 8;
/// Fastest playback a cape may ask for.
pub const MAX_FPS: u32 = 30;

#[derive(Debug, Clone, Deserialize)]
struct Item {
    id: String,
    name: String,
    /// Playback speed of an animated cape (default 8).
    #[serde(default)]
    fps: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Preset {
    pub id: String,
    pub name: String,
    /// Content hash, fetch it from `/v1/textures/<hash>.png`.
    pub texture: String,
    /// Animation frames stacked in the image (1: a still cape).
    pub frames: u32,
    /// Frames per second when animated.
    pub fps: u32,
    /// An animated cape's designated still image (hash), shown when its
    /// wearer turns "Animate" off; absent for capes that don't move.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub still: Option<String>,
    #[serde(skip)]
    png: Vec<u8>,
    #[serde(skip)]
    still_png: Option<Vec<u8>>,
}

/// Frames in a cape image: each is half as tall as the image is wide.
fn frames(png: &[u8]) -> u32 {
    let Ok(reader) = png::Decoder::new(std::io::Cursor::new(png)).read_info() else {
        return 1;
    };
    let (w, h) = (reader.info().width, reader.info().height);
    if w < 2 {
        return 1;
    }
    (h / (w / 2)).max(1)
}

/// The first frame of an animated cape as a PNG of its own: the still shown
/// when its wearer turns "Animate" off, for capes that have no painted one.
fn first_frame(png_bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png_bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or("image too large")?];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let (w, frame_h) = (info.width, info.width / 2);
    let channels = match info.color_type {
        png::ColorType::Rgba => 4,
        png::ColorType::Rgb => 3,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Grayscale => 1,
        png::ColorType::Indexed => return Err("indexed image".into()),
    };
    let texels = (w * frame_h) as usize;
    let mut rgba = Vec::with_capacity(texels * 4);
    for px in buf[..texels * channels].chunks_exact(channels) {
        rgba.extend_from_slice(&match info.color_type {
            png::ColorType::Rgba => [px[0], px[1], px[2], px[3]],
            png::ColorType::Rgb => [px[0], px[1], px[2], 255],
            png::ColorType::GrayscaleAlpha => [px[0], px[0], px[0], px[1]],
            _ => [px[0], px[0], px[0], 255],
        });
    }
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, w, frame_h);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(&rgba).map_err(|e| e.to_string())?;
    drop(writer);
    Ok(out)
}

pub struct Catalog {
    presets: Vec<Preset>,
}

impl Catalog {
    /// Reads `dir/catalog.json`, `dir/capes/<id>.png` and, for animated
    /// capes, an optional single-frame `dir/capes/<id>-still.png`.
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
            let frames = frames(&png);
            let fps = item.fps.unwrap_or(DEFAULT_FPS);
            if !(1..=MAX_FPS).contains(&fps) {
                return Err(format!("cape {}: fps must be 1 to {MAX_FPS}", item.id));
            }
            let still_path = dir.join("capes").join(format!("{}-still.png", item.id));
            let (still, still_png) = match std::fs::read(&still_path) {
                Ok(still_png) => {
                    let hash = images::check(&still_png, Kind::Cape)
                        .map_err(|e| format!("{}: {e}", still_path.display()))?;
                    if frames < 2 || self::frames(&still_png) != 1 {
                        return Err(format!(
                            "{}: a still belongs to an animated cape and has one frame",
                            still_path.display()
                        ));
                    }
                    (Some(hash), Some(still_png))
                }
                // No painted still: the cape's first frame stands in for it.
                Err(_) if frames > 1 => {
                    let still_png = first_frame(&png)
                        .map_err(|e| format!("cape {}: first frame: {e}", item.id))?;
                    let hash = images::check(&still_png, Kind::Cape)
                        .map_err(|e| format!("cape {}: first frame: {e}", item.id))?;
                    (Some(hash), Some(still_png))
                }
                Err(_) => (None, None),
            };
            presets.push(Preset {
                id: item.id,
                name: item.name,
                texture,
                frames,
                fps,
                still,
                png,
                still_png,
            });
        }
        Ok(Self { presets })
    }

    /// Make the preset textures downloadable.
    pub fn register(&self, store: &Store, now: u64) -> rusqlite::Result<()> {
        for p in &self.presets {
            store.put_texture(&p.texture, &p.png, now)?;
            if let (Some(hash), Some(png)) = (&p.still, &p.still_png) {
                store.put_texture(hash, png, now)?;
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundled_capes_load() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        let catalog = Catalog::load(&dir).unwrap();
        assert!(!catalog.presets().is_empty());
        // Borealis is the animated one: 6 frames of 128x64.
        let borealis = catalog
            .presets()
            .iter()
            .find(|p| p.id == "borealis")
            .unwrap();
        assert_eq!(borealis.frames, 6);
        assert!(catalog.presets().iter().any(|p| p.frames == 1));
        assert!(catalog.presets().iter().all(|p| p.fps >= 1));
        // Every animated cape has a still (painted, or its first frame); the others have none.
        for p in catalog.presets() {
            assert_eq!(p.still.is_some(), p.frames > 1, "{}", p.id);
        }
        assert!(borealis.still.is_some());
    }
}
