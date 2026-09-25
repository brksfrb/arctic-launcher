//! Falling snow with depth: far flakes are soft dots, near ones are tiny
//! six-armed crystals that slowly spin.

use std::f32::consts::TAU;

use eframe::egui::{Color32, Painter, Rect, pos2};

use crate::art::{hash01, snowflake};

const SNOW_COUNT: u32 = 90;
/// Flakes with depth above this are drawn as crystals.
const CRYSTAL_DEPTH: f32 = 0.86;

pub fn paint(painter: &Painter, rect: Rect, t: f32, snow: Color32) {
    let span = rect.height() + 20.0;
    for i in 0..SNOW_COUNT {
        let depth = hash01(i, 7);
        let speed = 12.0 + depth * 38.0;
        let y = (hash01(i, 8) * span + t * speed) % span - 10.0;
        let sway = (t * (0.4 + depth) + hash01(i, 9) * TAU).sin() * (4.0 + 12.0 * depth);
        let pos = pos2(
            rect.left() + hash01(i, 10) * rect.width() + sway,
            rect.top() + y,
        );
        let color = snow.gamma_multiply(0.25 + 0.6 * depth);
        if depth > CRYSTAL_DEPTH {
            let radius = 2.6 + 2.0 * hash01(i, 11);
            let spin = t * (0.6 + hash01(i, 12)) + hash01(i, 13) * TAU;
            snowflake(painter, pos, radius, spin, color);
        } else {
            painter.circle_filled(pos, 0.6 + depth * 1.5, color);
        }
    }
}
