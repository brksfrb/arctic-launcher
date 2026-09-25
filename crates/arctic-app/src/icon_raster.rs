// Dependency-free app icon rasterizer. Shared by the app (window icon) and
// `build.rs` (the .exe icon) via `include!`, so it must not use crates.
//
// The Arctic mark: six bold kite-shaped arms with notched tips and small
// side barbs around a hexagon core, shaded white at the center to ice blue
// at the tips, on a transparent background. Bold enough to stay crisp at
// 16px.

use std::f32::consts::{PI, TAU};

const INNER: [f32; 3] = [196.0, 234.0, 254.0];
const OUTER: [f32; 3] = [14.0, 165.0, 233.0];
/// Supersampling grid per axis (SS × SS samples per pixel).
const SS: u32 = 4;

// Arm geometry in units of the icon size, arm pointing up from the center
// (y grows downward, so "up" is negative y).
const KITE: [(f32, f32); 4] = [(0.0, -0.46), (0.075, -0.22), (0.0, -0.10), (-0.075, -0.22)];
const NOTCH: [(f32, f32); 6] = [
    (0.0, -0.36),
    (0.10, -0.42),
    (0.10, -0.30),
    (0.0, -0.27),
    (-0.10, -0.30),
    (-0.10, -0.42),
];
/// Side barbs: (x0, y0, x1, y1), stroked with round caps.
const BARBS: [(f32, f32, f32, f32); 2] = [(0.04, -0.33, 0.13, -0.38), (-0.04, -0.33, -0.13, -0.38)];
const BARB_WIDTH: f32 = 0.05;
const CORE_RADIUS: f32 = 0.10;
const GRADIENT_RADIUS: f32 = 0.46;

/// RGBA (unpremultiplied) icon, `size`×`size`.
pub fn app_icon_rgba(size: u32) -> Vec<u8> {
    let n = size as f32;
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let mut hits = 0u32;
            for sy in 0..SS {
                for sx in 0..SS {
                    // Sample position relative to the center, in icon units.
                    let u = (x as f32 + (sx as f32 + 0.5) / SS as f32) / n - 0.5;
                    let v = (y as f32 + (sy as f32 + 0.5) / SS as f32) / n - 0.5;
                    if inside_mark(u, v) {
                        hits += 1;
                    }
                }
            }
            let coverage = hits as f32 / (SS * SS) as f32;
            let (cx, cy) = ((x as f32 + 0.5) / n - 0.5, (y as f32 + 0.5) / n - 0.5);
            let t = ((cx * cx + cy * cy).sqrt() / GRADIENT_RADIUS).clamp(0.0, 1.0);
            let c = [
                INNER[0] + (OUTER[0] - INNER[0]) * t,
                INNER[1] + (OUTER[1] - INNER[1]) * t,
                INNER[2] + (OUTER[2] - INNER[2]) * t,
            ];
            rgba.extend_from_slice(&[c[0] as u8, c[1] as u8, c[2] as u8, (coverage * 255.0) as u8]);
        }
    }
    rgba
}

/// Whether point (u, v) (center-relative, icon units) is inside the mark.
fn inside_mark(u: f32, v: f32) -> bool {
    if inside_hexagon(u, v, CORE_RADIUS) {
        return true;
    }
    (0..6).any(|k| {
        // Rotate the point into arm k's frame (arm k points at k * 60°).
        let a = -(k as f32) * TAU / 6.0;
        let (s, c) = a.sin_cos();
        let (lu, lv) = (u * c - v * s, u * s + v * c);
        let blade = point_in_polygon(lu, lv, &KITE) && !point_in_polygon(lu, lv, &NOTCH);
        let barb = BARBS
            .iter()
            .any(|&(x0, y0, x1, y1)| dist_to_segment(lu, lv, x0, y0, x1, y1) <= BARB_WIDTH / 2.0);
        blade || barb
    })
}

fn inside_hexagon(u: f32, v: f32, r: f32) -> bool {
    let corners: Vec<(f32, f32)> = (0..6)
        .map(|k| {
            let a = k as f32 * PI / 3.0;
            (r * a.cos(), r * a.sin())
        })
        .collect();
    point_in_polygon(u, v, &corners)
}

/// Even-odd ray casting; works for concave polygons like the notch.
fn point_in_polygon(x: f32, y: f32, poly: &[(f32, f32)]) -> bool {
    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let (xi, yi) = poly[i];
        let (xj, yj) = poly[j];
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn dist_to_segment(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let (abx, aby) = (bx - ax, by - ay);
    let len_sq = (abx * abx + aby * aby).max(f32::EPSILON);
    let t = (((px - ax) * abx + (py - ay) * aby) / len_sq).clamp(0.0, 1.0);
    let (qx, qy) = (ax + abx * t - px, ay + aby * t - py);
    (qx * qx + qy * qy).sqrt()
}

/// Windows .ico with 32-bit BMP images at the given sizes.
#[allow(dead_code)]
pub fn ico_bytes(sizes: &[u32]) -> Vec<u8> {
    let images: Vec<Vec<u8>> = sizes.iter().map(|&s| bmp_entry(s)).collect();
    let mut out = Vec::new();
    out.extend_from_slice(&[0, 0, 1, 0]);
    out.extend_from_slice(&(sizes.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * sizes.len() as u32;
    for (&size, image) in sizes.iter().zip(&images) {
        let dim = if size >= 256 { 0 } else { size as u8 };
        out.extend_from_slice(&[dim, dim, 0, 0, 1, 0, 32, 0]);
        out.extend_from_slice(&(image.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += image.len() as u32;
    }
    for image in images {
        out.extend_from_slice(&image);
    }
    out
}

/// BITMAPINFOHEADER + bottom-up BGRA pixels + (empty) AND mask.
fn bmp_entry(size: u32) -> Vec<u8> {
    let rgba = app_icon_rgba(size);
    let mask_row = size.div_ceil(32) * 4;
    let mut out = Vec::new();
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(size as i32).to_le_bytes());
    out.extend_from_slice(&(size as i32 * 2).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(&[0u8; 24]);
    for y in (0..size).rev() {
        for x in 0..size {
            let i = ((y * size + x) * 4) as usize;
            out.extend_from_slice(&[rgba[i + 2], rgba[i + 1], rgba[i], rgba[i + 3]]);
        }
    }
    out.extend(std::iter::repeat_n(0u8, (mask_row * size) as usize));
    out
}
