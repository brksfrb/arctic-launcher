//! Vector art: backdrop scenery, logo, icons, avatars and the intro splash.
//! No image files; everything is drawn with egui shapes, except the brand
//! mark, which is the app icon's own rasterizer rendered to a texture.

pub mod avatar;
pub mod flakes;
pub mod icons;
pub mod scenery;
pub mod splash;

use std::f32::consts::{PI, TAU};

use eframe::egui::epaint::Mesh;
use eframe::egui::{
    self, Align2, Color32, FontId, Painter, Pos2, Rect, Sense, Stroke, TextureHandle, Ui, Vec2,
    vec2,
};

use crate::theme::Palette;

/// Stable pseudo-random value in `[0, 1)` for index `i` (no RNG state needed,
/// so every frame draws the same stars/peaks).
pub fn hash01(i: u32, salt: u32) -> f32 {
    let mut x = i.wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB_352D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846C_A68B);
    x ^= x >> 16;
    (x >> 8) as f32 / (1u32 << 24) as f32
}

/// Premultiplied color with zero alpha, which egui blends additively (a glow).
pub fn additive(color: Color32, strength: f32) -> Color32 {
    let s = strength.clamp(0.0, 1.0);
    let scale = |c: u8| (c as f32 * s) as u8;
    Color32::from_rgba_premultiplied(scale(color.r()), scale(color.g()), scale(color.b()), 0)
}

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t.clamp(0.0, 1.0)) as u8;
    Color32::from_rgb(mix(a.r(), b.r()), mix(a.g(), b.g()), mix(a.b(), b.b()))
}

/// Six-armed snowflake with two pairs of branches per arm.
pub fn snowflake(painter: &Painter, center: Pos2, radius: f32, rotation: f32, color: Color32) {
    let stroke = Stroke::new((radius * 0.09).max(1.2), color);
    for k in 0..6 {
        let angle = rotation + k as f32 * TAU / 6.0;
        let dir = Vec2::angled(angle);
        painter.line_segment([center, center + dir * radius], stroke);
        for (at, len) in [(0.45, 0.32), (0.72, 0.22)] {
            let base = center + dir * radius * at;
            for side in [-1.0, 1.0] {
                let branch = Vec2::angled(angle + side * PI / 4.0);
                painter.line_segment([base, base + branch * radius * len], stroke);
            }
        }
    }
    painter.circle_stroke(center, radius * 0.16, stroke);
}

/// Soft additive halo: a radial-gradient mesh (no visible banding).
pub fn glow(painter: &Painter, center: Pos2, radius: f32, color: Color32, strength: f32) {
    const SEGMENTS: u32 = 48;
    /// (fraction of radius, fraction of strength): an approximately
    /// Gaussian falloff from three rings.
    const RINGS: [(f32, f32); 3] = [(0.25, 0.7), (0.6, 0.22), (1.0, 0.0)];
    let mut mesh = Mesh::default();
    mesh.colored_vertex(center, additive(color, strength * 0.45));
    for (dist, alpha) in RINGS {
        for s in 0..SEGMENTS {
            let dir = Vec2::angled(s as f32 * TAU / SEGMENTS as f32);
            mesh.colored_vertex(
                center + dir * radius * dist,
                additive(color, strength * 0.45 * alpha),
            );
        }
    }
    let ring = |r: u32, s: u32| 1 + r * SEGMENTS + s % SEGMENTS;
    for s in 0..SEGMENTS {
        mesh.add_triangle(0, ring(0, s), ring(0, s + 1));
        for r in 1..RINGS.len() as u32 {
            let (a, b) = (ring(r - 1, s), ring(r - 1, s + 1));
            let (c, d) = (ring(r, s), ring(r, s + 1));
            mesh.add_triangle(a, c, b);
            mesh.add_triangle(b, c, d);
        }
    }
    painter.add(egui::Shape::mesh(mesh));
}

/// Logo: snowflake + wordmark, for the nav header.
pub fn logo(ui: &mut Ui, p: &Palette) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 56.0), Sense::hover());
    let painter = ui.painter_at(rect);
    let center = rect.left_center() + vec2(22.0, 0.0);
    let t = ui.input(|i| i.time) as f32;
    glow(&painter, center, 26.0, p.accent, 0.8);
    brand_mark(ui.ctx(), &painter, center, 19.0, t * 0.15);
    painter.text(
        center + vec2(30.0, -9.0),
        Align2::LEFT_CENTER,
        "Arctic",
        FontId::proportional(26.0),
        p.text,
    );
    painter.text(
        center + vec2(31.0, 15.0),
        Align2::LEFT_CENTER,
        "LAUNCHER",
        FontId::proportional(11.0),
        p.muted,
    );
}

/// Texture resolution of the brand mark (sharp up to ~2x DPI at logo size).
const MARK_TEXTURE: u32 = 160;
/// Tip of the mark's arms, as a fraction of the icon's size.
const MARK_TIP: f32 = 0.46;

/// The Arctic mark (same art as the window and .exe icon), with its arms
/// reaching `radius` from `center`, turned by `rotation` radians.
pub fn brand_mark(
    ctx: &egui::Context,
    painter: &Painter,
    center: Pos2,
    radius: f32,
    rotation: f32,
) {
    let id = egui::Id::new("arctic-brand-mark");
    let texture = ctx
        .data_mut(|d| d.get_temp::<TextureHandle>(id))
        .unwrap_or_else(|| {
            let rgba = crate::icon_raster::app_icon_rgba(MARK_TEXTURE);
            let size = [MARK_TEXTURE as usize; 2];
            let image = egui::ColorImage::from_rgba_unmultiplied(size, &rgba);
            let handle = ctx.load_texture("arctic-brand-mark", image, egui::TextureOptions::LINEAR);
            ctx.data_mut(|d| d.insert_temp(id, handle.clone()));
            handle
        });
    let rect = Rect::from_center_size(center, Vec2::splat(radius / MARK_TIP));
    let uv = Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0));
    let mut mesh = Mesh::with_texture(texture.id());
    mesh.add_rect_with_uv(rect, uv, Color32::WHITE);
    mesh.rotate(egui::emath::Rot2::from_angle(rotation), center);
    painter.add(egui::Shape::mesh(mesh));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_stable_and_in_range() {
        for i in 0..1000 {
            let v = hash01(i, 3);
            assert!((0.0..1.0).contains(&v));
            assert_eq!(v, hash01(i, 3));
        }
        assert_ne!(hash01(1, 1), hash01(1, 2));
    }

    #[test]
    fn additive_has_zero_alpha() {
        let c = additive(Color32::from_rgb(200, 100, 50), 0.5);
        assert_eq!((c.r(), c.g(), c.b(), c.a()), (100, 50, 25, 0));
    }

    #[test]
    fn lerp_endpoints() {
        let (a, b) = (Color32::BLACK, Color32::WHITE);
        assert_eq!(lerp_color(a, b, 0.0), a);
        assert_eq!(lerp_color(a, b, 1.0), b);
    }

    #[test]
    fn icon_has_opaque_center_and_transparent_corner() {
        let size = 32;
        let rgba = crate::icon_raster::app_icon_rgba(size);
        assert_eq!(rgba.len(), (size * size * 4) as usize);
        let at = |x: u32, y: u32| rgba[((y * size + x) * 4 + 3) as usize];
        assert_eq!(at(16, 16), 255);
        assert_eq!(at(0, 0), 0);
    }

    /// `ARCTIC_ICON_DUMP=<dir> cargo test -p arctic-app dump_icon` writes raw
    /// RGBA previews for eyeballing the icon without launching the app.
    #[test]
    fn dump_icon() {
        let Some(dir) = std::env::var_os("ARCTIC_ICON_DUMP") else {
            return;
        };
        for size in [256u32, 48, 32, 16] {
            let path = std::path::Path::new(&dir).join(format!("icon-{size}.rgba"));
            std::fs::write(path, crate::icon_raster::app_icon_rgba(size)).unwrap();
        }
    }

    #[test]
    fn ico_header_lists_every_size() {
        let ico = crate::icon_raster::ico_bytes(&[16, 32]);
        assert_eq!(&ico[..6], &[0, 0, 1, 0, 2, 0]);
        assert_eq!(ico[6], 16);
        assert_eq!(ico[22], 32);
    }
}
