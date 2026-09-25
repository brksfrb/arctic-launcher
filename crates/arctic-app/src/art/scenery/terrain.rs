//! Mountains (three layers with mist between them) and a snowy pine tree
//! line along the bottom edge.

use eframe::egui::epaint::Mesh;
use eframe::egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, pos2};

use crate::art::{hash01, lerp_color};
use crate::theme::Scene;

/// Shape of one mountain layer; colors come from `Scene::mountains`.
struct MountainShape {
    /// Max peak height as a fraction of the window height.
    height: f32,
    /// Horizontal distance between ridge points.
    segment: f32,
    snow_caps: bool,
    salt: u32,
}

const MOUNTAINS: [MountainShape; 3] = [
    MountainShape {
        height: 0.34,
        segment: 90.0,
        snow_caps: true,
        salt: 11,
    },
    MountainShape {
        height: 0.25,
        segment: 70.0,
        snow_caps: true,
        salt: 23,
    },
    MountainShape {
        height: 0.15,
        segment: 55.0,
        snow_caps: false,
        salt: 37,
    },
];

/// Horizontal spacing between potential tree spots.
const TREE_STEP: f32 = 13.0;
const TREE_MIN: f32 = 14.0;
const TREE_MAX: f32 = 42.0;

pub fn paint(painter: &Painter, rect: Rect, scene: &Scene) {
    for (i, (shape, colors)) in MOUNTAINS.iter().zip(scene.mountains).enumerate() {
        mountains(painter, rect, shape, colors, scene);
        if i < 2 {
            mist(painter, rect, scene, 0.22 - i as f32 * 0.07);
        }
    }
    trees(painter, rect, scene);
}

/// Jagged ridge: alternating peaks and valleys with hashed heights.
fn ridge(rect: Rect, shape: &MountainShape) -> Vec<Pos2> {
    let max_h = rect.height() * shape.height;
    let count = (rect.width() / shape.segment).ceil() as u32 + 2;
    (0..count)
        .map(|j| {
            let jitter = (hash01(j, shape.salt) - 0.5) * shape.segment * 0.6;
            let x = rect.left() - shape.segment + j as f32 * shape.segment + jitter;
            let (lo, hi) = if j % 2 == 0 {
                (0.55, 1.0)
            } else {
                (0.15, 0.45)
            };
            let h = max_h * (lo + (hi - lo) * hash01(j, shape.salt + 1));
            pos2(x, rect.bottom() - h)
        })
        .collect()
}

fn mountains(
    painter: &Painter,
    rect: Rect,
    shape: &MountainShape,
    colors: (Color32, Color32),
    scene: &Scene,
) {
    let (top, bottom) = colors;
    let points = ridge(rect, shape);
    let top_y = rect.bottom() - rect.height() * shape.height;
    let mut mesh = Mesh::default();
    for p in &points {
        let depth = ((p.y - top_y) / (rect.bottom() - top_y)).clamp(0.0, 1.0);
        mesh.colored_vertex(*p, lerp_color(top, bottom, depth));
        mesh.colored_vertex(pos2(p.x, rect.bottom()), bottom);
    }
    for j in 1..points.len() as u32 {
        let (a, b) = ((j - 1) * 2, j * 2);
        mesh.add_triangle(a, b, a + 1);
        mesh.add_triangle(b, b + 1, a + 1);
    }
    painter.add(Shape::mesh(mesh));

    if shape.snow_caps {
        for w in points
            .windows(3)
            .filter(|w| w[1].y < w[0].y && w[1].y < w[2].y)
        {
            let (left, peak, right) = (w[0], w[1], w[2]);
            let l = peak + (left - peak) * 0.28;
            let r = peak + (right - peak) * 0.28;
            let notch = pos2((l.x + r.x) * 0.5, l.y.max(r.y) + 3.0);
            painter.add(Shape::convex_polygon(
                vec![peak, r, notch, l],
                scene.snow_cap,
                Stroke::NONE,
            ));
        }
    }
    painter.add(Shape::line(points, Stroke::new(1.2, scene.rim)));
}

/// Soft horizontal haze band whose densest line sits `height` above the bottom.
fn mist(painter: &Painter, rect: Rect, scene: &Scene, height: f32) {
    let center = rect.bottom() - rect.height() * height;
    let half = rect.height() * 0.06;
    let clear = Color32::from_rgba_premultiplied(0, 0, 0, 0);
    let mut mesh = Mesh::default();
    for (y, color) in [
        (center - half, clear),
        (center, scene.mist),
        (center + half, clear),
    ] {
        mesh.colored_vertex(pos2(rect.left(), y), color);
        mesh.colored_vertex(pos2(rect.right(), y), color);
    }
    for band in 0..2u32 {
        let i = band * 2;
        mesh.add_triangle(i, i + 1, i + 2);
        mesh.add_triangle(i + 1, i + 3, i + 2);
    }
    painter.add(Shape::mesh(mesh));
}

/// Clusters of layered pines with snow on each tier.
fn trees(painter: &Painter, rect: Rect, scene: &Scene) {
    let count = (rect.width() / TREE_STEP) as u32 + 1;
    for i in 0..count {
        // Low-frequency noise groups trees into clusters with gaps.
        let cluster = ((i as f32 * 0.21).sin() + (i as f32 * 0.07 + 1.3).sin()) * 0.5;
        if cluster < 0.1 || hash01(i, 40) < 0.35 {
            continue;
        }
        let height = TREE_MIN + (TREE_MAX - TREE_MIN) * hash01(i, 41) * (0.5 + cluster);
        let x = rect.left() + i as f32 * TREE_STEP + (hash01(i, 42) - 0.5) * 8.0;
        pine(
            painter,
            pos2(x, rect.bottom() + 2.0),
            height.min(TREE_MAX),
            scene,
        );
    }
}

fn pine(painter: &Painter, base: Pos2, height: f32, scene: &Scene) {
    const TIERS: usize = 3;
    let width = height * 0.5;
    for tier in 0..TIERS {
        let f = tier as f32 / TIERS as f32;
        let bottom = base.y - height * f * 0.8;
        let top = bottom - height * 0.45;
        let half = width * (1.0 - f * 0.3) / 2.0;
        let tri = vec![
            pos2(base.x, top),
            pos2(base.x + half, bottom),
            pos2(base.x - half, bottom),
        ];
        painter.add(Shape::convex_polygon(tri, scene.trees, Stroke::NONE));
        // Snow resting on the upper part of each tier.
        let cap = vec![
            pos2(base.x, top),
            pos2(base.x + half * 0.45, top + (bottom - top) * 0.45),
            pos2(base.x - half * 0.3, top + (bottom - top) * 0.38),
        ];
        painter.add(Shape::convex_polygon(
            cap,
            scene.snow_cap.gamma_multiply(0.55),
            Stroke::NONE,
        ));
    }
}
