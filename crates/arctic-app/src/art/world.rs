//! The "your world" backdrop: the player's newest screenshot of the selected
//! instance (or its version's title panorama), softened and dimmed so text
//! stays readable. Loaded off the UI thread and redone when the instance or
//! version changes.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};

use arctic_core::storage::DataDirs;
use eframe::egui::{
    self, Color32, ColorImage, Painter, Pos2, Rect, TextureHandle, TextureOptions, epaint::Mesh,
};

/// Working width: plenty for a softened background, cheap to blur.
const WIDTH: u32 = 1100;
const BLUR: f32 = 2.5;

#[derive(Default)]
pub struct WorldBackdrop {
    /// What the current (or loading) picture is for.
    key: String,
    texture: Option<TextureHandle>,
    /// Nothing to show for this key (no screenshots, version not installed).
    missing: bool,
    pending: Option<Receiver<Option<ColorImage>>>,
}

impl WorldBackdrop {
    /// Make sure the picture for this instance and version is loaded (or loading).
    pub fn want(
        &mut self,
        ctx: &egui::Context,
        dirs: &DataDirs,
        game_dir: PathBuf,
        version: Option<String>,
    ) {
        let key = format!(
            "{}|{}",
            game_dir.display(),
            version.as_deref().unwrap_or("")
        );
        if key == self.key {
            self.poll(ctx);
            return;
        }
        self.key = key;
        self.missing = false;
        let (tx, rx) = mpsc::channel();
        self.pending = Some(rx);
        let dirs = dirs.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let image = arctic_core::screenshots::backdrop(&dirs, &game_dir, version.as_deref())
                .and_then(|(bytes, panorama)| prepare(&bytes, panorama));
            let _ = tx.send(image);
            ctx.request_repaint();
        });
    }

    fn poll(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.pending else {
            return;
        };
        if let Ok(result) = rx.try_recv() {
            self.pending = None;
            match result {
                Some(image) => {
                    self.texture =
                        Some(ctx.load_texture("world-backdrop", image, TextureOptions::LINEAR));
                }
                None => {
                    self.texture = None;
                    self.missing = true;
                }
            }
        }
    }

    /// True when there's nothing to show (the caller draws something else).
    pub fn missing(&self) -> bool {
        self.missing && self.texture.is_none()
    }

    /// Fill `rect` with the picture (cropped to fit), dimmed toward `base`:
    /// lighter at the top, where it's seen, darker below, where lists are.
    pub fn paint(&self, painter: &Painter, rect: Rect, base: Color32) {
        painter.rect_filled(rect, 0.0, base);
        let Some(tex) = &self.texture else {
            return;
        };
        let [w, h] = tex.size().map(|v| v as f32);
        let uv = cover_uv(w / h, rect.width() / rect.height());
        painter.image(tex.id(), rect, uv, Color32::WHITE);
        let top = with_alpha(base, 95);
        let bottom = with_alpha(base, 210);
        let mut mesh = Mesh::default();
        mesh.colored_vertex(rect.left_top(), top);
        mesh.colored_vertex(rect.right_top(), top);
        mesh.colored_vertex(rect.right_bottom(), bottom);
        mesh.colored_vertex(rect.left_bottom(), bottom);
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        painter.add(mesh);
    }
}

/// The part of an image with aspect `image` that covers an area with aspect `area`.
fn cover_uv(image: f32, area: f32) -> Rect {
    if image > area {
        let w = area / image;
        Rect::from_min_max(
            Pos2::new((1.0 - w) / 2.0, 0.0),
            Pos2::new((1.0 + w) / 2.0, 1.0),
        )
    } else {
        let h = image / area;
        Rect::from_min_max(
            Pos2::new(0.0, (1.0 - h) / 2.0),
            Pos2::new(1.0, (1.0 + h) / 2.0),
        )
    }
}

fn with_alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

/// Decode, shrink and soften the picture. The panorama is mostly sky: keep
/// the band around its horizon.
fn prepare(bytes: &[u8], panorama: bool) -> Option<ColorImage> {
    let mut img = image::load_from_memory(bytes).ok()?;
    if panorama {
        let (w, h) = (img.width(), img.height());
        img = img.crop_imm(0, h * 3 / 10, w, h / 2);
    }
    let img = if img.width() > WIDTH {
        img.resize(WIDTH, u32::MAX, image::imageops::FilterType::Triangle)
    } else {
        img
    };
    let rgba = image::imageops::blur(&img.to_rgba8(), BLUR);
    let size = [rgba.width() as usize, rgba.height() as usize];
    Some(ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_pictures_are_cropped_at_the_sides() {
        let uv = cover_uv(2.0, 1.0);
        assert!((uv.min.x - 0.25).abs() < 1e-6 && (uv.max.x - 0.75).abs() < 1e-6);
        assert_eq!((uv.min.y, uv.max.y), (0.0, 1.0));
    }

    #[test]
    fn tall_pictures_are_cropped_top_and_bottom() {
        let uv = cover_uv(1.0, 2.0);
        assert_eq!((uv.min.x, uv.max.x), (0.0, 1.0));
        assert!((uv.min.y - 0.25).abs() < 1e-6 && (uv.max.y - 0.75).abs() < 1e-6);
    }
}
