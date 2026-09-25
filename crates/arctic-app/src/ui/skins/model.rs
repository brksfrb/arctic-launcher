//! A textured 3D player model drawn with egui meshes: each body part is a
//! box whose faces map to the standard 64×64 skin layout. Faces are
//! back-face culled and painted far-to-near.

use std::f32::consts::PI;

use arctic_core::skins::Variant;
use eframe::egui::{self, Color32, Mesh, Pos2, Rect, TextureId, pos2};

/// Model height in skin pixels (feet at 0, top of head at 32).
const HEIGHT: f32 = 32.0;
/// Camera distance for the slight perspective.
const CAMERA: f32 = 90.0;

#[derive(Debug, Clone, Copy)]
pub struct Pose {
    /// Rotation around the vertical axis, radians (0 = facing the viewer).
    pub yaw: f32,
    pub pitch: f32,
    /// Arm and leg swing, radians.
    pub swing: f32,
}

type V3 = [f32; 3];
type Uv = (f32, f32);
/// Corners (tl, tr, br, bl), outward normal and texture corners.
type BoxFace = ([V3; 4], V3, [Uv; 4]);
/// (min, size, base uv, overlay uv, pivot, angle, overlay inflate)
type PartLayout = (V3, V3, Uv, Uv, V3, f32, f32);

/// One box of the model.
struct Part {
    /// Local box corners.
    min: V3,
    max: V3,
    /// Texture origin and box size in texture pixels (w, h, d).
    uv: (f32, f32),
    size: V3,
    /// Rotation about the X axis around `pivot` (limb swing).
    pivot: V3,
    angle: f32,
    /// Turn the box around (the cape's texture front faces backwards).
    flip: bool,
}

struct Face {
    corners: [V3; 4],
    uv: [Pos2; 4],
    depth: f32,
}

/// Draw the player into `rect`. `cape` is a 64×32 cape texture.
pub fn paint(
    painter: &egui::Painter,
    rect: Rect,
    skin: TextureId,
    variant: Variant,
    has_overlay: bool,
    cape: Option<TextureId>,
    pose: Pose,
) {
    let scale = rect.height() / (HEIGHT + 6.0);
    let center = rect.center();
    let view = |p: V3| -> V3 { rotate_view(p, pose) };
    let project = |p: V3| -> Pos2 {
        let k = CAMERA / (CAMERA - p[2]);
        pos2(center.x + p[0] * scale * k, center.y - p[1] * scale * k)
    };
    if let Some(cape) = cape {
        let faces = build_faces(&[cape_part(pose.swing)], 64.0, 32.0, &view);
        painter.add(mesh(cape, &faces, &project));
    }
    let faces = build_faces(&parts(variant, has_overlay, pose.swing), 64.0, 64.0, &view);
    painter.add(mesh(skin, &faces, &project));
}

fn mesh(texture: TextureId, faces: &[Face], project: &impl Fn(V3) -> Pos2) -> Mesh {
    let mut mesh = Mesh::with_texture(texture);
    for face in faces {
        let base = mesh.vertices.len() as u32;
        for (corner, uv) in face.corners.iter().zip(face.uv) {
            mesh.vertices.push(egui::epaint::Vertex {
                pos: project(*corner),
                uv,
                color: Color32::WHITE,
            });
        }
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base, base + 2, base + 3);
    }
    mesh
}

/// Visible faces of all parts in view space, sorted far to near.
fn build_faces(parts: &[Part], tex_w: f32, tex_h: f32, view: &impl Fn(V3) -> V3) -> Vec<Face> {
    let mut faces = Vec::with_capacity(parts.len() * 3);
    for part in parts {
        for (corners, normal, uv) in box_faces(part) {
            let place = |p: V3| view(part_transform(part, p));
            let n = sub(place(normal), place([0.0; 3]));
            let corners = corners.map(place);
            // Face points away from the viewer (who looks down -z).
            if n[2] - 0.0 <= 1e-4 {
                continue;
            }
            let depth = corners.iter().map(|c| c[2]).sum::<f32>() / 4.0;
            let uv = uv.map(|(u, v)| pos2(u / tex_w, v / tex_h));
            faces.push(Face { corners, uv, depth });
        }
    }
    faces.sort_by(|a, b| a.depth.total_cmp(&b.depth));
    faces
}

/// The six faces of a box: corners (tl, tr, br, bl as seen from outside),
/// outward normal and texture corners.
fn box_faces(part: &Part) -> [BoxFace; 6] {
    let [x0, y0, z0] = part.min;
    let [x1, y1, z1] = part.max;
    let (u, v) = part.uv;
    let [w, h, d] = part.size;
    let rect = |u: f32, v: f32, w: f32, h: f32| [(u, v), (u + w, v), (u + w, v + h), (u, v + h)];
    [
        // Front (+z)
        (
            [[x0, y1, z1], [x1, y1, z1], [x1, y0, z1], [x0, y0, z1]],
            [0.0, 0.0, 1.0],
            rect(u + d, v + d, w, h),
        ),
        // Back (-z)
        (
            [[x1, y1, z0], [x0, y1, z0], [x0, y0, z0], [x1, y0, z0]],
            [0.0, 0.0, -1.0],
            rect(u + 2.0 * d + w, v + d, w, h),
        ),
        // Player's right (-x)
        (
            [[x0, y1, z0], [x0, y1, z1], [x0, y0, z1], [x0, y0, z0]],
            [-1.0, 0.0, 0.0],
            rect(u, v + d, d, h),
        ),
        // Player's left (+x)
        (
            [[x1, y1, z1], [x1, y1, z0], [x1, y0, z0], [x1, y0, z1]],
            [1.0, 0.0, 0.0],
            rect(u + d + w, v + d, d, h),
        ),
        // Top (+y)
        (
            [[x0, y1, z0], [x1, y1, z0], [x1, y1, z1], [x0, y1, z1]],
            [0.0, 1.0, 0.0],
            rect(u + d, v, w, d),
        ),
        // Bottom (-y)
        (
            [[x0, y0, z1], [x1, y0, z1], [x1, y0, z0], [x0, y0, z0]],
            [0.0, -1.0, 0.0],
            rect(u + d + w, v, w, d),
        ),
    ]
}

fn part_transform(part: &Part, p: V3) -> V3 {
    let mut p = p;
    if part.flip {
        // Half turn around the box's vertical center line.
        let cx = (part.min[0] + part.max[0]) / 2.0;
        let cz = (part.min[2] + part.max[2]) / 2.0;
        p = [2.0 * cx - p[0], p[1], 2.0 * cz - p[2]];
    }
    if part.angle != 0.0 {
        let (s, c) = part.angle.sin_cos();
        let [_, py, pz] = part.pivot;
        let (y, z) = (p[1] - py, p[2] - pz);
        p = [p[0], py + y * c - z * s, pz + y * s + z * c];
    }
    p
}

fn rotate_view(p: V3, pose: Pose) -> V3 {
    // Center the model vertically so it turns around its middle.
    let (x, y, z) = (p[0], p[1] - HEIGHT / 2.0, p[2]);
    let (sy, cy) = pose.yaw.sin_cos();
    let (x, z) = (x * cy + z * sy, -x * sy + z * cy);
    let (sp, cp) = pose.pitch.sin_cos();
    let (y, z) = (y * cp - z * sp, y * sp + z * cp);
    [x, y, z]
}

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// A box of size (w, h, d) at `min`, grown by `inflate` on every side.
fn cuboid(min: V3, size: V3, uv: (f32, f32), inflate: f32, pivot: V3, angle: f32) -> Part {
    Part {
        min: [min[0] - inflate, min[1] - inflate, min[2] - inflate],
        max: [
            min[0] + size[0] + inflate,
            min[1] + size[1] + inflate,
            min[2] + size[2] + inflate,
        ],
        uv,
        size,
        pivot,
        angle,
        flip: false,
    }
}

fn parts(variant: Variant, overlay: bool, swing: f32) -> Vec<Part> {
    let arm = if variant == Variant::Slim { 3.0 } else { 4.0 };
    let shoulder_r = [-4.0 - arm / 2.0, 22.0, 0.0];
    let shoulder_l = [4.0 + arm / 2.0, 22.0, 0.0];
    let hip = [0.0, 12.0, 0.0];
    let none = [0.0; 3];
    let layout: [PartLayout; 6] = [
        (
            [-4.0, 24.0, -4.0],
            [8.0, 8.0, 8.0],
            (0.0, 0.0),
            (32.0, 0.0),
            none,
            0.0,
            0.5,
        ),
        (
            [-4.0, 12.0, -2.0],
            [8.0, 12.0, 4.0],
            (16.0, 16.0),
            (16.0, 32.0),
            none,
            0.0,
            0.25,
        ),
        (
            [-4.0 - arm, 12.0, -2.0],
            [arm, 12.0, 4.0],
            (40.0, 16.0),
            (40.0, 32.0),
            shoulder_r,
            swing,
            0.25,
        ),
        (
            [4.0, 12.0, -2.0],
            [arm, 12.0, 4.0],
            (32.0, 48.0),
            (48.0, 48.0),
            shoulder_l,
            -swing,
            0.25,
        ),
        (
            [-4.0, 0.0, -2.0],
            [4.0, 12.0, 4.0],
            (0.0, 16.0),
            (0.0, 32.0),
            hip,
            -swing,
            0.25,
        ),
        (
            [0.0, 0.0, -2.0],
            [4.0, 12.0, 4.0],
            (16.0, 48.0),
            (0.0, 48.0),
            hip,
            swing,
            0.25,
        ),
    ];
    let mut parts = Vec::with_capacity(12);
    for (i, (min, size, uv, over, pivot, angle, inflate)) in layout.into_iter().enumerate() {
        parts.push(cuboid(min, size, uv, 0.0, pivot, angle));
        // Legacy skins only have the hat layer.
        if overlay || i == 0 {
            parts.push(cuboid(min, size, over, inflate, pivot, angle));
        }
    }
    parts
}

/// Cape hanging from the shoulders, swaying a little with the walk.
fn cape_part(swing: f32) -> Part {
    let mut part = cuboid(
        [-5.0, 8.0, -3.2],
        [10.0, 16.0, 1.0],
        (0.0, 0.0),
        0.0,
        [0.0, 24.0, -2.0],
        0.0,
    );
    part.flip = true;
    part.angle = 0.18 + swing.abs() * 0.25;
    part
}

/// Default idle pose: a gentle walk cycle.
pub fn idle_swing(time: f64) -> f32 {
    ((time * 1.6).sin() as f32) * 0.35
}

/// Clamp a dragged pitch to something that still reads as a person.
pub fn clamp_pitch(pitch: f32) -> f32 {
    pitch.clamp(-PI / 4.0, PI / 4.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pose(yaw: f32) -> Pose {
        Pose {
            yaw,
            pitch: 0.0,
            swing: 0.0,
        }
    }

    #[test]
    fn front_view_shows_front_faces_only() {
        let view = |p: V3| rotate_view(p, pose(0.0));
        let head = [cuboid(
            [-4.0, 24.0, -4.0],
            [8.0, 8.0, 8.0],
            (0.0, 0.0),
            0.0,
            [0.0; 3],
            0.0,
        )];
        let faces = build_faces(&head, 64.0, 64.0, &view);
        assert_eq!(faces.len(), 1);
        // Head front is at (8,8)-(16,16) in the texture.
        assert_eq!(faces[0].uv[0], pos2(8.0 / 64.0, 8.0 / 64.0));
        assert_eq!(faces[0].uv[2], pos2(16.0 / 64.0, 16.0 / 64.0));
    }

    #[test]
    fn angled_view_shows_three_faces() {
        let view = |p: V3| {
            rotate_view(
                p,
                Pose {
                    yaw: 0.6,
                    pitch: 0.3,
                    swing: 0.0,
                },
            )
        };
        let head = [cuboid(
            [-4.0, 24.0, -4.0],
            [8.0, 8.0, 8.0],
            (0.0, 0.0),
            0.0,
            [0.0; 3],
            0.0,
        )];
        assert_eq!(build_faces(&head, 64.0, 64.0, &view).len(), 3);
    }

    #[test]
    fn faces_are_sorted_far_to_near() {
        let view = |p: V3| rotate_view(p, pose(0.8));
        let faces = build_faces(&parts(Variant::Classic, true, 0.2), 64.0, 64.0, &view);
        assert!(faces.windows(2).all(|w| w[0].depth <= w[1].depth));
        assert!(!faces.is_empty());
    }

    #[test]
    fn legacy_skins_skip_body_overlays() {
        assert_eq!(parts(Variant::Classic, true, 0.0).len(), 12);
        assert_eq!(parts(Variant::Slim, false, 0.0).len(), 7);
    }
}
