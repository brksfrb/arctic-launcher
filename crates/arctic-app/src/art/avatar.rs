//! Player heads painted as 8×8 crisp pixels (no textures needed).

use arctic_core::auth::avatar::Face;
use eframe::egui::{Color32, CornerRadius, Painter, Rect, Stroke, StrokeKind, vec2};

/// Paint `face` filling `rect`, with a square frame like the head itself.
pub fn paint_face(painter: &Painter, rect: Rect, face: &Face, frame: Color32) {
    let px = rect.width() / 8.0;
    let clip = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
    for y in 0..8 {
        for x in 0..8 {
            let [r, g, b, _] = face.pixel(x, y);
            let min = rect.min + vec2(x as f32 * px, y as f32 * px);
            // Slight overlap hides seams between pixels at fractional scales.
            let cell = Rect::from_min_size(min, vec2(px + 0.6, px + 0.6));
            clip.rect_filled(cell, 0.0, Color32::from_rgb(r, g, b));
        }
    }
    // Heads are square: a tiny radius just softens the frame's corners.
    painter.rect_stroke(
        rect,
        CornerRadius::same(2),
        Stroke::new(2.0, frame),
        StrokeKind::Outside,
    );
}
