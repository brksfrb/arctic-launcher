//! Procedural arctic backdrop: sky, stars, moon/sun, shooting stars,
//! aurora, ice mountains with mist, a pine tree line and snowfall.
//! Everything is vector-painted each frame (no image assets). Colors come
//! from the active theme's `Scene`; with `animated = false` it renders one
//! still frame.

mod snow;
mod terrain;

use std::f32::consts::TAU;

use eframe::egui::epaint::Mesh;
use eframe::egui::{Color32, Painter, Rect, Shape, Stroke, pos2, vec2};

use super::{additive, glow, hash01, lerp_color};
use crate::theme::{Celestial, Scene};

const STAR_COUNT: u32 = 90;
/// Stars live in the upper part of the sky only (fraction of height).
const STAR_BAND: f32 = 0.42;
const AURORA_STEP: f32 = 8.0;
const SHOOTING_PERIOD: f32 = 9.0;
const SHOOTING_DURATION: f32 = 0.9;
const MOON_RADIUS: f32 = 20.0;

struct Aurora {
    color: Color32,
    /// Vertical position of the bright edge, as a fraction of the height.
    y: f32,
    /// Curtain height in px.
    height: f32,
    speed: f32,
    brightness: f32,
}

const AURORAS: [Aurora; 3] = [
    Aurora {
        color: Color32::from_rgb(0x34, 0xd3, 0x99),
        y: 0.20,
        height: 130.0,
        speed: 1.0,
        brightness: 0.42,
    },
    Aurora {
        color: Color32::from_rgb(0x22, 0xd3, 0xee),
        y: 0.27,
        height: 100.0,
        speed: 0.7,
        brightness: 0.32,
    },
    Aurora {
        color: Color32::from_rgb(0xa7, 0x8b, 0xfa),
        y: 0.16,
        height: 80.0,
        speed: 1.3,
        brightness: 0.22,
    },
];

pub fn paint(painter: &Painter, rect: Rect, scene: &Scene, time: f32, animated: bool) {
    let t = if animated { time } else { 0.0 };
    sky(painter, rect, scene);
    if scene.star_strength > 0.0 {
        stars(painter, rect, t, scene);
        if animated {
            shooting_star(painter, rect, t, scene);
        }
    }
    celestial(painter, rect, scene);
    if scene.aurora_strength > 0.0 {
        for (i, aurora) in AURORAS.iter().enumerate() {
            aurora_ribbon(painter, rect, t, i as f32, aurora, scene.aurora_strength);
        }
    }
    terrain::paint(painter, rect, scene);
    snow::paint(painter, rect, t, scene.snow);
}

/// Sky color at height fraction `y` (0 = top), matching `sky()`.
fn sky_at(scene: &Scene, y: f32) -> Color32 {
    let [top, mid, horizon] = scene.sky;
    if y < 0.55 {
        lerp_color(top, mid, y / 0.55)
    } else {
        lerp_color(mid, horizon, (y - 0.55) / 0.45)
    }
}

fn sky(painter: &Painter, rect: Rect, scene: &Scene) {
    let [top, mid, horizon] = scene.sky;
    let mid_y = rect.top() + rect.height() * 0.55;
    let mut mesh = Mesh::default();
    for (y, color) in [(rect.top(), top), (mid_y, mid), (rect.bottom(), horizon)] {
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

/// Tiny twinkling points; the brightest few get a four-point sparkle so
/// they read as stars, not snow.
fn stars(painter: &Painter, rect: Rect, t: f32, scene: &Scene) {
    for i in 0..STAR_COUNT {
        let pos = pos2(
            rect.left() + hash01(i, 1) * rect.width(),
            rect.top() + hash01(i, 2).powf(1.4) * rect.height() * STAR_BAND,
        );
        let twinkle = 0.5 + 0.5 * (t * (0.8 + hash01(i, 3) * 2.2) + hash01(i, 4) * TAU).sin();
        let brightness = hash01(i, 6);
        let strength = (0.2 + 0.6 * brightness * (0.4 + 0.6 * twinkle)) * scene.star_strength;
        painter.circle_filled(
            pos,
            0.45 + brightness * 0.55,
            additive(scene.snow, strength),
        );
        if brightness > 0.86 {
            let len = 2.5 + 3.5 * twinkle;
            let stroke = Stroke::new(0.8, additive(scene.snow, strength * 0.7));
            painter.line_segment([pos - vec2(len, 0.0), pos + vec2(len, 0.0)], stroke);
            painter.line_segment([pos - vec2(0.0, len), pos + vec2(0.0, len)], stroke);
        }
    }
}

/// Every few seconds a short streak crosses the upper sky.
fn shooting_star(painter: &Painter, rect: Rect, t: f32, scene: &Scene) {
    let cycle = (t / SHOOTING_PERIOD).floor();
    let seed = cycle as u32;
    let start = cycle * SHOOTING_PERIOD + hash01(seed, 20) * (SHOOTING_PERIOD - SHOOTING_DURATION);
    let progress = (t - start) / SHOOTING_DURATION;
    if !(0.0..1.0).contains(&progress) {
        return;
    }
    let origin = pos2(
        rect.left() + rect.width() * (0.3 + 0.65 * hash01(seed, 21)),
        rect.top() + rect.height() * (0.04 + 0.2 * hash01(seed, 22)),
    );
    let dir = vec2(-0.87, 0.5);
    let travel = rect.width() * 0.22;
    let head = origin + dir * travel * progress;
    let fade = (1.0 - progress) * progress.min(0.15) / 0.15;
    let tail = head - dir * (60.0 + 40.0 * progress);
    let mut mesh = Mesh::default();
    let normal = vec2(dir.y, -dir.x) * 1.2;
    mesh.colored_vertex(head + normal, additive(scene.snow, fade));
    mesh.colored_vertex(head - normal, additive(scene.snow, fade));
    mesh.colored_vertex(tail, Color32::TRANSPARENT);
    mesh.add_triangle(0, 1, 2);
    painter.add(Shape::mesh(mesh));
    painter.circle_filled(head, 1.3, additive(scene.snow, fade));
}

fn celestial(painter: &Painter, rect: Rect, scene: &Scene) {
    let center = pos2(
        rect.left() + rect.width() * 0.84,
        rect.top() + rect.height() * 0.13,
    );
    match scene.celestial {
        Celestial::Moon => {
            glow(
                painter,
                center,
                MOON_RADIUS * 4.0,
                scene.snow,
                0.35 * scene.star_strength.max(0.4),
            );
            painter.circle_filled(
                center,
                MOON_RADIUS,
                lerp_color(scene.snow, Color32::WHITE, 0.3),
            );
            // Crescent: cover part of the disc with the sky color behind it.
            let y = (center.y - rect.top()) / rect.height();
            let shadow = center + vec2(MOON_RADIUS * 0.55, -MOON_RADIUS * 0.25);
            painter.circle_filled(shadow, MOON_RADIUS * 0.92, sky_at(scene, y));
        }
        Celestial::Sun => {
            for (r, a) in [(170.0, 0.10), (90.0, 0.16), (40.0, 0.35)] {
                painter.circle_filled(center, r, Color32::from_white_alpha((255.0 * a) as u8));
            }
        }
    }
}

/// A curtain of light: bright lower edge fading upward, drifting sideways.
fn aurora_ribbon(painter: &Painter, rect: Rect, t: f32, k: f32, a: &Aurora, strength: f32) {
    let base_y = rect.top() + rect.height() * a.y;
    let mut mesh = Mesh::default();
    let columns = (rect.width() / AURORA_STEP).ceil() as u32 + 1;
    for c in 0..columns {
        let x = rect.left() + c as f32 * AURORA_STEP;
        let phase = x * 0.005 * a.speed + t * 0.12 * a.speed + k * 2.1;
        let y = base_y + 22.0 * phase.sin() + 11.0 * (phase * 2.3 + k).sin();
        let pulse = 0.5 + 0.5 * (x * 0.004 + t * 0.25 + k * 1.7).sin();
        let alpha = a.brightness * strength * (0.35 + 0.65 * pulse);
        let top = a.height * (0.75 + 0.25 * (phase * 1.7).sin());
        mesh.colored_vertex(pos2(x, y - top), Color32::TRANSPARENT);
        mesh.colored_vertex(pos2(x, y), additive(a.color, alpha));
        mesh.colored_vertex(pos2(x, y + 10.0), Color32::TRANSPARENT);
        if c > 0 {
            let (p, n) = ((c - 1) * 3, c * 3);
            for row in 0..2 {
                mesh.add_triangle(p + row, n + row, p + row + 1);
                mesh.add_triangle(n + row, n + row + 1, p + row + 1);
            }
        }
    }
    painter.add(Shape::mesh(mesh));
}
