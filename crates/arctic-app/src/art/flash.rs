//! Flash Hosting's mark (Arctic's supporter): an isometric block with an
//! amber lightning bolt, drawn from the same outline as their site's logo.

use eframe::egui::epaint::Mesh;
use eframe::egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, pos2};

/// The bolt's amber.
pub const AMBER: Color32 = Color32::from_rgb(245, 158, 11);

/// Their logo's drawing box (the SVG's viewBox: 3..29 on both axes).
const ORIGIN: f32 = 3.0;
const SPAN: f32 = 26.0;

/// The mark in `rect` (square). `faces` is the block's color: dark on light
/// backgrounds, light on dark ones; its three sides are shaded from it.
pub fn mark(painter: &Painter, rect: Rect, faces: Color32) {
    let at = |x: f32, y: f32| -> Pos2 {
        pos2(
            rect.left() + (x - ORIGIN) / SPAN * rect.width(),
            rect.top() + (y - ORIGIN) / SPAN * rect.height(),
        )
    };
    let shade = |alpha: f32| faces.gamma_multiply(alpha);
    let face = |points: [(f32, f32); 4], color: Color32| {
        Shape::convex_polygon(
            points.iter().map(|&(x, y)| at(x, y)).collect(),
            color,
            Stroke::NONE,
        )
    };
    painter.add(face(
        [(16.0, 4.0), (26.0, 10.0), (16.0, 16.0), (6.0, 10.0)],
        shade(0.62),
    ));
    painter.add(face(
        [(6.0, 10.0), (16.0, 16.0), (16.0, 28.0), (6.0, 22.0)],
        shade(0.85),
    ));
    painter.add(face(
        [(26.0, 10.0), (16.0, 16.0), (16.0, 28.0), (26.0, 22.0)],
        faces,
    ));
    // The bolt is scaled 1.3x about the block's center, as in the logo.
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
        mesh.colored_vertex(p, AMBER);
    }
    for [a, b, c] in [[0, 1, 2], [0, 2, 5], [2, 3, 4], [2, 4, 5]] {
        mesh.add_triangle(a, b, c);
    }
    painter.add(Shape::mesh(mesh));
}
