//! Flash Hosting's mark (Arctic's supporter): an amber tile with a dark
//! isometric block and an amber lightning bolt struck through it, drawn
//! from the same outline as their site's icon (icon.svg, viewBox 0..32).

use eframe::egui::epaint::Mesh;
use eframe::egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, pos2};

/// Flash's amber, for text beside the mark.
pub const AMBER: Color32 = Color32::from_rgb(245, 158, 11);
/// The tile and the bolt.
const TILE: Color32 = Color32::from_rgb(255, 176, 32);
/// The block's sides (shaded by opacity, as in the icon).
const BLOCK: [u8; 3] = [23, 17, 4];

/// Their icon's drawing box.
const SPAN: f32 = 32.0;
/// The tile's corner radius in that box.
const CORNER: f32 = 7.0;

/// The mark in `rect` (square). The same on light and dark backgrounds: the tile carries it.
pub fn mark(painter: &Painter, rect: Rect) {
    let at = |x: f32, y: f32| -> Pos2 {
        pos2(
            rect.left() + x / SPAN * rect.width(),
            rect.top() + y / SPAN * rect.height(),
        )
    };
    painter.rect_filled(rect, CORNER / SPAN * rect.width(), TILE);
    let block = |opacity: f32| {
        Color32::from_rgba_unmultiplied(
            BLOCK[0],
            BLOCK[1],
            BLOCK[2],
            (opacity * 255.0).round() as u8,
        )
    };
    let face = |points: [(f32, f32); 4], color: Color32| {
        Shape::convex_polygon(
            points.iter().map(|&(x, y)| at(x, y)).collect(),
            color,
            Stroke::NONE,
        )
    };
    painter.add(face(
        [(16.0, 4.0), (26.0, 10.0), (16.0, 16.0), (6.0, 10.0)],
        block(0.62),
    ));
    painter.add(face(
        [(6.0, 10.0), (16.0, 16.0), (16.0, 28.0), (6.0, 22.0)],
        block(0.85),
    ));
    painter.add(face(
        [(26.0, 10.0), (16.0, 16.0), (16.0, 28.0), (26.0, 22.0)],
        block(1.0),
    ));
    // The bolt is scaled 1.3x about the block's center, as in the icon.
    let bolt = [
        (17.4, 8.6),
        (11.9, 16.3),
        (14.8, 16.3),
        (14.0, 23.3),
        (20.0, 15.2),
        (16.8, 15.2),
    ]
    .map(|(x, y)| at(16.0 + (x - 16.0) * 1.3, 16.0 + (y - 16.0) * 1.3));
    // Not convex: two halves, as triangles.
    let mut mesh = Mesh::default();
    for p in bolt {
        mesh.colored_vertex(p, TILE);
    }
    for [a, b, c] in [[0, 1, 2], [0, 2, 5], [2, 3, 4], [2, 4, 5]] {
        mesh.add_triangle(a, b, c);
    }
    painter.add(Shape::mesh(mesh));
}
