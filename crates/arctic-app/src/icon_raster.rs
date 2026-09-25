// Dependency-free snowflake icon rasterizer. Shared by the app (window icon)
// and `build.rs` (the .exe icon) via `include!`, so it must not use crates.
//
// Transparent background; a detailed six-armed crystal with a white→ice
// gradient and a thin deep-blue outline so it reads on light and dark
// wallpapers. Small sizes use a simplified design to stay crisp.

use std::f32::consts::{FRAC_PI_3, PI, TAU};

/// Line segment with its stroke width: [x0, y0, x1, y1, width].
type Seg = [f32; 5];

const INNER: [f32; 3] = [240.0, 251.0, 255.0];
const ICE: [f32; 3] = [125.0, 211.0, 252.0];
const DEEP: [f32; 3] = [56.0, 189.0, 248.0];
const OUTLINE: [f32; 3] = [12.0, 74.0, 110.0];
/// Supersampling grid per axis (SS × SS samples per pixel).
const SS: u32 = 3;

/// RGBA (unpremultiplied) icon, `size`×`size`.
pub fn app_icon_rgba(size: u32) -> Vec<u8> {
    let n = size as f32;
    let segs = flake_segments(n);
    let radius = n * 0.46;
    let outline = (n * 0.012).max(0.8);
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let (mut flake, mut edge) = (0.0f32, 0.0f32);
            for sy in 0..SS {
                for sx in 0..SS {
                    let px = x as f32 + (sx as f32 + 0.5) / SS as f32;
                    let py = y as f32 + (sy as f32 + 0.5) / SS as f32;
                    let d = signed_distance(&segs, px, py);
                    flake += (0.5 - d).clamp(0.0, 1.0);
                    edge += (0.5 - (d - outline)).clamp(0.0, 1.0);
                }
            }
            let samples = (SS * SS) as f32;
            let (flake, edge) = (flake / samples, edge / samples);
            let dist =
                ((x as f32 + 0.5 - n / 2.0).powi(2) + (y as f32 + 0.5 - n / 2.0).powi(2)).sqrt();
            let t = (dist / radius).clamp(0.0, 1.0);
            let fill = if t < 0.5 {
                mix3(INNER, ICE, t / 0.5)
            } else {
                mix3(ICE, DEEP, (t - 0.5) / 0.5)
            };
            // Flake over outline, both over transparency.
            let alpha = flake + edge * (1.0 - flake);
            let color = if alpha > 0.0 {
                mix3(OUTLINE, fill, flake / alpha)
            } else {
                [0.0; 3]
            };
            rgba.extend_from_slice(&[
                color[0] as u8,
                color[1] as u8,
                color[2] as u8,
                (alpha * 255.0) as u8,
            ]);
        }
    }
    rgba
}

fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// Distance to the stroked shape (negative inside).
fn signed_distance(segs: &[Seg], px: f32, py: f32) -> f32 {
    segs.iter()
        .map(|s| dist_to_segment(px, py, s) - s[4] / 2.0)
        .fold(f32::MAX, f32::min)
}

fn flake_segments(n: f32) -> Vec<Seg> {
    let (cx, cy) = (n / 2.0, n / 2.0);
    let r = n * 0.44;
    let detailed = n >= 40.0;
    let arm_w = if detailed {
        n * 0.03
    } else {
        (n * 0.085).max(1.6)
    };
    let branch_w = arm_w * if detailed { 0.78 } else { 0.9 };
    // (position along the arm, branch length) as fractions of r.
    let branches: &[(f32, f32)] = if detailed {
        &[(0.38, 0.24), (0.60, 0.21), (0.80, 0.12)]
    } else {
        &[(0.55, 0.28)]
    };
    let mut segs = Vec::new();
    let mut seg = |x0: f32, y0: f32, x1: f32, y1: f32, w: f32| segs.push([x0, y0, x1, y1, w]);
    for k in 0..6 {
        let a = k as f32 * TAU / 6.0 - PI / 2.0;
        let (dx, dy) = (a.cos(), a.sin());
        seg(cx, cy, cx + dx * r, cy + dy * r, arm_w);
        for &(at, len) in branches {
            let (bx, by) = (cx + dx * r * at, cy + dy * r * at);
            for side in [-1.0f32, 1.0] {
                let b = a + side * FRAC_PI_3;
                seg(
                    bx,
                    by,
                    bx + b.cos() * r * len,
                    by + b.sin() * r * len,
                    branch_w,
                );
            }
        }
        if detailed {
            // Small fork at the tip and a short spike between arms.
            let (tx, ty) = (cx + dx * r * 0.93, cy + dy * r * 0.93);
            for side in [-1.0f32, 1.0] {
                let b = a + side * FRAC_PI_3;
                seg(
                    tx,
                    ty,
                    tx + b.cos() * r * 0.06,
                    ty + b.sin() * r * 0.06,
                    branch_w * 0.8,
                );
            }
            let between = a + PI / 6.0;
            let (sx, sy) = (cx + between.cos() * r * 0.18, cy + between.sin() * r * 0.18);
            seg(
                sx,
                sy,
                cx + between.cos() * r * 0.3,
                cy + between.sin() * r * 0.3,
                branch_w * 0.7,
            );
        }
    }
    if detailed {
        // Hexagonal core ring.
        let hr = r * 0.13;
        for k in 0..6 {
            let a0 = k as f32 * TAU / 6.0 - PI / 2.0 + PI / 6.0;
            let a1 = a0 + TAU / 6.0;
            seg(
                cx + a0.cos() * hr,
                cy + a0.sin() * hr,
                cx + a1.cos() * hr,
                cy + a1.sin() * hr,
                branch_w * 0.8,
            );
        }
    }
    segs
}

fn dist_to_segment(px: f32, py: f32, s: &Seg) -> f32 {
    let (ax, ay, bx, by) = (s[0], s[1], s[2], s[3]);
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
