//! Snowflake styles used as instance icons.

use std::f32::consts::{FRAC_PI_3, PI, TAU};

use arctic_core::instances::{FlakeStyle, InstanceIcon};
use eframe::egui::{Color32, Painter, Pos2, Shape, Stroke, Vec2};

use super::snowflake;

/// Color of an instance icon: custom or the theme accent.
pub fn icon_color(icon: &InstanceIcon, accent: Color32) -> Color32 {
    icon.color
        .map_or(accent, |[r, g, b]| Color32::from_rgb(r, g, b))
}

/// Draw `style` centred at `center`.
pub fn draw(
    painter: &Painter,
    style: FlakeStyle,
    center: Pos2,
    radius: f32,
    rotation: f32,
    color: Color32,
) {
    let stroke = Stroke::new((radius * 0.08).max(1.1), color);
    let thin = Stroke::new(stroke.width * 0.75, color);
    let arm = |k: usize| Vec2::angled(rotation + k as f32 * TAU / 6.0 - PI / 2.0);
    let line = |a: Pos2, b: Pos2, s: Stroke| painter.line_segment([a, b], s);
    let branch = |base: Pos2, dir: Vec2, len: f32, s: Stroke| {
        for side in [-1.0, 1.0] {
            let d = dir.rot90() * side * 0.866 + dir * 0.5; // ±60°
            line(base, base + d * len, s);
        }
    };
    match style {
        FlakeStyle::Classic => snowflake(painter, center, radius, rotation, color),
        FlakeStyle::Stellar => {
            for k in 0..6 {
                let d = arm(k);
                line(center, center + d * radius, stroke);
                for i in 1..=4 {
                    let at = 0.25 + i as f32 * 0.15;
                    branch(
                        center + d * radius * at,
                        d,
                        radius * (0.2 - i as f32 * 0.025),
                        thin,
                    );
                }
            }
            painter.circle_filled(center, radius * 0.12, color);
        }
        FlakeStyle::Dendrite => {
            for k in 0..6 {
                let d = arm(k);
                line(center, center + d * radius, stroke);
                for (at, len) in [(0.4, 0.42), (0.72, 0.26)] {
                    let base = center + d * radius * at;
                    for side in [-1.0, 1.0] {
                        let b = d.rot90() * side * 0.866 + d * 0.5;
                        let tip = base + b * radius * len;
                        line(base, tip, thin);
                        branch(base + b * radius * len * 0.55, b, radius * len * 0.35, thin);
                    }
                }
            }
        }
        FlakeStyle::Plate => {
            let hex = |r: f32, s: Stroke| {
                let pts: Vec<Pos2> = (0..6).map(|k| center + arm(k) * r).collect();
                painter.add(Shape::closed_line(pts, s));
            };
            hex(radius * 0.95, stroke);
            hex(radius * 0.55, thin);
            for k in 0..6 {
                let d = arm(k);
                line(center + d * radius * 0.55, center + d * radius * 0.95, thin);
                line(center, center + d * radius * 0.3, thin);
            }
        }
        FlakeStyle::Star => {
            let points: Vec<Pos2> = (0..12)
                .map(|i| {
                    let r = if i % 2 == 0 { radius } else { radius * 0.45 };
                    center + Vec2::angled(rotation + i as f32 * PI / 6.0 - PI / 2.0) * r
                })
                .collect();
            painter.add(Shape::closed_line(points, stroke));
            for k in 0..6 {
                line(center, center + arm(k) * radius * 0.45, thin);
            }
            painter.circle_filled(center, radius * 0.1, color);
        }
        FlakeStyle::Crystal => {
            for k in 0..6 {
                let d = arm(k);
                line(center, center + d * radius * 0.7, stroke);
                // Diamond tip.
                let tip = center + d * radius;
                let mid = center + d * radius * 0.75;
                let side = d.rot90() * radius * 0.12;
                painter.add(Shape::closed_line(
                    vec![
                        mid,
                        mid + d * radius * 0.12 + side,
                        tip,
                        mid + d * radius * 0.12 - side,
                    ],
                    thin,
                ));
                branch(center + d * radius * 0.45, d, radius * 0.18, thin);
            }
            let core: Vec<Pos2> = (0..6)
                .map(|k| center + Vec2::angled(rotation + k as f32 * FRAC_PI_3) * radius * 0.2)
                .collect();
            painter.add(Shape::closed_line(core, thin));
        }
    }
}

/// Preset colors offered in the instance icon picker.
pub const SWATCHES: [[u8; 3]; 8] = [
    [0x7d, 0xd3, 0xfc], // ice
    [0x34, 0xd3, 0x99], // aurora green
    [0xa7, 0x8b, 0xfa], // violet
    [0xf4, 0x72, 0xb6], // rose
    [0xfb, 0xbf, 0x24], // amber
    [0x5e, 0xea, 0xd4], // mint
    [0xf8, 0x71, 0x71], // coral
    [0xe6, 0xf1, 0xff], // frost white
];
