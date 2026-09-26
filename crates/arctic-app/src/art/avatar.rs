//! Player heads: the 8×8 face as a crisp texture with rounded corners.

use std::hash::{Hash, Hasher};

use arctic_core::auth::avatar::Face;
use eframe::egui::{
    Color32, ColorImage, CornerRadius, Id, Painter, Rect, Stroke, StrokeKind, TextureHandle,
    TextureOptions, epaint::RectShape, pos2,
};

/// Paint `face` filling `rect`, clipped to rounded corners, with a frame.
pub fn paint_face(painter: &Painter, rect: Rect, face: &Face, frame: Color32) {
    let texture = face_texture(painter, face);
    let radius = CornerRadius::same((rect.width() * 0.18) as u8);
    let full = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    painter.add(RectShape::filled(rect, radius, Color32::WHITE).with_texture(texture.id(), full));
    painter.rect_stroke(rect, radius, Stroke::new(2.0, frame), StrokeKind::Outside);
}

/// One texture per distinct face, kept in egui's memory.
fn face_texture(painter: &Painter, face: &Face) -> TextureHandle {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    face.0.hash(&mut hasher);
    let id = Id::new(("face-texture", hasher.finish()));
    let ctx = painter.ctx();
    if let Some(handle) = ctx.data(|d| d.get_temp::<TextureHandle>(id)) {
        return handle;
    }
    let rgba: Vec<u8> = face.0.iter().flatten().copied().collect();
    let image = ColorImage::from_rgba_unmultiplied([8, 8], &rgba);
    let handle = ctx.load_texture("player-face", image, TextureOptions::NEAREST);
    ctx.data_mut(|d| d.insert_temp(id, handle.clone()));
    handle
}
