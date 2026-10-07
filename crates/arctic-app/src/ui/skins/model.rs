//! A textured 3D player model drawn with egui meshes: each body part is a
//! box whose faces map to the standard 64×64 skin layout. Faces are
//! back-face culled and painted far-to-near. Worn 3D cosmetics are drawn
//! with it, placed exactly as the game places them.

use std::f32::consts::PI;

use arctic_core::cosmetic_models::{Geometry, Mesh as MeshModel};
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
    texture: TextureId,
    /// Multiplies the texture (white for the player and cuboid cosmetics;
    /// the shading and vertex colors of a mesh).
    tint: Color32,
    /// Texels across and down (how finely it's cut up for depth sorting).
    cells: [u16; 2],
}

/// Faces are cut into pieces at most this many texels wide before sorting.
const MAX_CELLS: u16 = 16;

impl Face {
    /// Texels across and down, from texture-pixel corners (tl, tr, br, bl).
    fn cells_of(uv_px: &[(f32, f32); 4]) -> [u16; 2] {
        let across = (uv_px[1].0 - uv_px[0].0)
            .abs()
            .max((uv_px[1].1 - uv_px[0].1).abs());
        let down = (uv_px[3].1 - uv_px[0].1)
            .abs()
            .max((uv_px[3].0 - uv_px[0].0).abs());
        let cells = |n: f32| (n.round() as u16).clamp(1, MAX_CELLS);
        [cells(across), cells(down)]
    }

    /// The face cut into its texels. Sorting whole faces by their middle
    /// lets a big face (the head's side) cover a small cube in front of it
    /// (a helmet's horn); texel-sized pieces sort (nearly) right.
    fn cut(&self) -> impl Iterator<Item = Face> + '_ {
        let [nu, nv] = self.cells;
        let [tl, tr, br, bl] = self.corners;
        let [utl, utr, ubr, ubl] = self.uv;
        let at = move |s: f32, t: f32| -> (V3, Pos2) {
            let top = lerp3(tl, tr, s);
            let bottom = lerp3(bl, br, s);
            let uv_top = utl + (utr - utl) * s;
            let uv_bottom = ubl + (ubr - ubl) * s;
            (lerp3(top, bottom, t), uv_top + (uv_bottom - uv_top) * t)
        };
        (0..nv).flat_map(move |j| {
            (0..nu).map(move |i| {
                let (s0, s1) = (i as f32 / nu as f32, (i + 1) as f32 / nu as f32);
                let (t0, t1) = (j as f32 / nv as f32, (j + 1) as f32 / nv as f32);
                let points = [at(s0, t0), at(s1, t0), at(s1, t1), at(s0, t1)];
                let corners = points.map(|p| p.0);
                Face {
                    corners,
                    uv: points.map(|p| p.1),
                    depth: corners.iter().map(|c| c[2]).sum::<f32>() / 4.0,
                    texture: self.texture,
                    tint: self.tint,
                    cells: [1, 1],
                }
            })
        })
    }
}

fn lerp3(a: V3, b: V3, t: f32) -> V3 {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// A 3D cosmetic to draw: its geometry and texture.
#[derive(Clone, Copy)]
pub struct Worn<'a> {
    pub geometry: &'a Geometry,
    pub texture: TextureId,
}

/// A sculpted cosmetic to draw: its mesh and textures.
#[derive(Clone, Copy)]
pub struct WornMesh<'a> {
    pub mesh: &'a MeshModel,
    /// One texture per image embedded in the mesh.
    pub textures: &'a [TextureId],
    /// A plain white texel, for parts painted with vertex colors alone.
    pub white: TextureId,
    /// Seconds into the idle animation (`None`: the authored rest pose).
    pub time: Option<f64>,
    /// The wearer's arm and leg swing, radians: pieces on a limb follow it.
    pub swing: f32,
}

/// Where a piece that follows a limb turns (the shoulder or hip height in the preview) and which way:
/// arms and legs swing in opposite directions, as `parts` has them.
fn limb(root: &str, swing: f32) -> Option<(f32, f32)> {
    match root.to_ascii_lowercase().replace('_', "").as_str() {
        "rightarm" => Some((22.0, swing)),
        "leftarm" => Some((22.0, -swing)),
        "rightleg" => Some((12.0, -swing)),
        "leftleg" => Some((12.0, swing)),
        _ => None,
    }
}

/// Draw the player into `rect`. `cape` is a 64×32 cape texture.
#[allow(clippy::too_many_arguments)]
pub fn paint(
    painter: &egui::Painter,
    rect: Rect,
    skin: TextureId,
    variant: Variant,
    has_overlay: bool,
    cape: Option<TextureId>,
    cosmetics: &[Worn],
    meshes: &[WornMesh],
    pose: Pose,
) {
    let scale = rect.height() / (HEIGHT + 6.0);
    let center = rect.center();
    let view = |p: V3| -> V3 { rotate_view(p, pose) };
    let project = |p: V3| -> Pos2 {
        let k = CAMERA / (CAMERA - p[2]);
        pos2(center.x + p[0] * scale * k, center.y - p[1] * scale * k)
    };
    let mut faces = build_faces(
        &parts(variant, has_overlay, pose.swing),
        skin,
        64.0,
        64.0,
        &view,
    );
    if let Some(cape) = cape {
        faces.extend(build_faces(
            &[cape_part(pose.swing)],
            cape,
            64.0,
            32.0,
            &view,
        ));
    }
    for worn in cosmetics {
        faces.extend(cosmetic_faces(worn, &view));
    }
    for worn in meshes {
        faces.extend(mesh_faces(worn, &view));
    }
    // One depth order for body, cape and cosmetics, so each hides the others
    // correctly; consecutive faces with the same texture share a mesh.
    let mut faces: Vec<Face> = faces.iter().flat_map(Face::cut).collect();
    faces.sort_by(|a, b| a.depth.total_cmp(&b.depth));
    for run in faces.chunk_by(|a, b| a.texture == b.texture) {
        painter.add(mesh(run, &project));
    }
}

fn mesh(faces: &[Face], project: &impl Fn(V3) -> Pos2) -> Mesh {
    let mut mesh = Mesh::with_texture(faces.first().map_or(TextureId::default(), |f| f.texture));
    for face in faces {
        let base = mesh.vertices.len() as u32;
        for (corner, uv) in face.corners.iter().zip(face.uv) {
            mesh.vertices.push(egui::epaint::Vertex {
                pos: project(*corner),
                uv,
                color: face.tint,
            });
        }
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base, base + 2, base + 3);
    }
    mesh
}

/// Visible faces of all parts in view space, sorted far to near.
fn build_faces(
    parts: &[Part],
    texture: TextureId,
    tex_w: f32,
    tex_h: f32,
    view: &impl Fn(V3) -> V3,
) -> Vec<Face> {
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
            let cells = Face::cells_of(&uv);
            let uv = uv.map(|(u, v)| pos2(u / tex_w, v / tex_h));
            faces.push(Face {
                corners,
                uv,
                depth,
                texture,
                tint: Color32::WHITE,
                cells,
            });
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

/// Draw one cosmetic on its own, turning slowly, fitted to `rect`.
pub fn paint_cosmetic(painter: &egui::Painter, rect: Rect, worn: Worn, yaw: f32) {
    paint_turning(painter, rect, yaw, |view| cosmetic_faces(&worn, &|p| view(p)));
}

/// The same for a sculpted cosmetic.
pub fn paint_mesh_cosmetic(painter: &egui::Painter, rect: Rect, worn: WornMesh, yaw: f32) {
    paint_turning(painter, rect, yaw, |view| mesh_faces(&worn, &|p| view(p)));
}

/// Draws the faces `faces_of` makes for a view, turned around their own middle.
fn paint_turning(
    painter: &egui::Painter,
    rect: Rect,
    yaw: f32,
    faces_of: impl Fn(&dyn Fn(V3) -> V3) -> Vec<Face>,
) {
    let pose = Pose {
        yaw,
        pitch: 0.25,
        swing: 0.0,
    };
    // Turned around its own middle (not the wearer's), and sized from the
    // sphere around it, so it stays put and keeps its size while it turns.
    let corners: Vec<V3> = faces_of(&|p| p)
        .iter()
        .flat_map(|f| f.corners)
        .collect();
    if corners.is_empty() {
        return;
    }
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for c in &corners {
        for i in 0..3 {
            lo[i] = lo[i].min(c[i]);
            hi[i] = hi[i].max(c[i]);
        }
    }
    let mid: V3 = std::array::from_fn(|i| (lo[i] + hi[i]) / 2.0);
    let radius = corners
        .iter()
        .map(|c| (0..3).map(|i| (c[i] - mid[i]).powi(2)).sum::<f32>().sqrt())
        .fold(0.5, f32::max);
    // rotate_view turns around HEIGHT / 2; move the middle there first.
    let view = |p: V3| {
        rotate_view(
            [p[0] - mid[0], p[1] - mid[1] + HEIGHT / 2.0, p[2] - mid[2]],
            pose,
        )
    };
    let mut faces: Vec<Face> = faces_of(&view).iter().flat_map(Face::cut).collect();
    faces.sort_by(|a, b| a.depth.total_cmp(&b.depth));
    let scale = rect.width().min(rect.height()) * 0.9 / (2.0 * radius);
    let center = rect.center();
    let project = |p: V3| pos2(center.x + p[0] * scale, center.y - p[1] * scale);
    painter.add(mesh(&faces, &project));
}

/// The triangles of a sculpted cosmetic, placed like the game places them:
/// vertices are in model space (as for cuboid cosmetics), moved by their
/// node, and the preview's z is the model's z flipped. Lit from the front
/// and above, on both sides (so a mesh reads the same whichever way its
/// triangles wind).
fn mesh_faces(worn: &WornMesh, view: &impl Fn(V3) -> V3) -> Vec<Face> {
    use arctic_core::cosmetic_models::mesh::apply;
    let light = {
        let l: V3 = [0.35, 0.65, 0.68];
        let len = (l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt();
        l.map(|x| x / len)
    };
    let world = worn.mesh.world(worn.time.map(|t| t as f32));
    let mut faces = Vec::with_capacity(worn.mesh.triangles());
    for prim in &worn.mesh.primitives {
        let material = prim.material.map(|m| &worn.mesh.materials[m]);
        let texture = material
            .and_then(|m| m.texture)
            .and_then(|i| worn.textures.get(i))
            .copied()
            .unwrap_or(worn.white);
        let base = material.map_or([1.0; 4], |m| m.base_color);
        let matrix = &world[prim.node];
        // The root node says which part of the player the piece follows.
        let mut root = prim.node;
        while let Some(parent) = worn.mesh.nodes[root].parent {
            root = parent;
        }
        let turn = limb(&worn.mesh.nodes[root].name, worn.swing);
        let at = |i: u32| -> V3 {
            let p = apply(matrix, prim.positions[i as usize]);
            let mut p = [p[0], p[1], -p[2]];
            if let Some((pivot_y, angle)) = turn {
                // Around the shoulder or hip, as the limb itself turns (see `part_transform`).
                let (s, c) = angle.sin_cos();
                let y = p[1] - pivot_y;
                p = [p[0], pivot_y + y * c - p[2] * s, y * s + p[2] * c];
            }
            view(p)
        };
        for tri in prim.indices.chunks_exact(3) {
            // The travelling light (in the model's own space, as the game draws it).
            let glint = match (&worn.mesh.sheen, worn.time) {
                (Some(sheen), Some(t)) => {
                    let m = |i: u32| apply(matrix, prim.positions[i as usize]);
                    let (p0, p1, p2) = (m(tri[0]), m(tri[1]), m(tri[2]));
                    let centre = [
                        (p0[0] + p1[0] + p2[0]) / 3.0,
                        (p0[1] + p1[1] + p2[1]) / 3.0,
                        (p0[2] + p1[2] + p2[2]) / 3.0,
                    ];
                    let (u, v) = (sub(p1, p0), sub(p2, p0));
                    let nz = u[0] * v[1] - u[1] * v[0];
                    let len = (u[1] * v[2] - u[2] * v[1]).hypot(u[2] * v[0] - u[0] * v[2]).hypot(nz).max(1e-9);
                    sheen.at(centre, nz / len, t as f32)
                }
                _ => 0.0,
            };
            let (a, b, c) = (at(tri[0]), at(tri[1]), at(tri[2]));
            let (u, v) = (sub(b, a), sub(c, a));
            let mut n = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-9);
            n = n.map(|x| x / len);
            if n[2] < 0.0 {
                n = n.map(|x| -x);
            }
            let shade = 0.5 + 0.5 * (n[0] * light[0] + n[1] * light[1] + n[2] * light[2]).max(0.0);
            let mut color = [0.0f32; 4];
            for &i in tri {
                let vc = prim.colors[i as usize];
                for k in 0..4 {
                    color[k] += vc[k] / 3.0;
                }
            }
            let to_u8 = |x: f32| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
            let lit = |k: usize| -> f32 {
                let light = color[k] * base[k] * shade;
                match &worn.mesh.sheen {
                    Some(sheen) => light * (1.0 - glint) + sheen.tint[k] * glint,
                    None => light,
                }
            };
            let tint = Color32::from_rgba_unmultiplied(
                to_u8(lit(0)),
                to_u8(lit(1)),
                to_u8(lit(2)),
                to_u8(color[3] * base[3]),
            );
            let uv = |i: u32| {
                let t = prim.uvs[i as usize];
                pos2(t[0], t[1])
            };
            faces.push(Face {
                corners: [a, b, c, c],
                uv: [uv(tri[0]), uv(tri[1]), uv(tri[2]), uv(tri[2])],
                depth: (a[2] + b[2] + c[2]) / 3.0,
                texture,
                tint,
                cells: [1, 1],
            });
        }
    }
    faces
}

/// Model height in Java model space: Bedrock y (feet at 0, up) is 24 - y there.
const JAVA_HEIGHT: f32 = 24.0;
type Mat = [[f32; 3]; 3];

/// A bone's placement in Java model space: rotation, then translation.
#[derive(Clone, Copy)]
struct Placement {
    rot: Mat,
    at: V3,
}

impl Placement {
    const IDENTITY: Self = Self {
        rot: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        at: [0.0; 3],
    };

    fn apply(&self, p: V3) -> V3 {
        let r = &self.rot;
        [
            r[0][0] * p[0] + r[0][1] * p[1] + r[0][2] * p[2] + self.at[0],
            r[1][0] * p[0] + r[1][1] * p[1] + r[1][2] * p[2] + self.at[1],
            r[2][0] * p[0] + r[2][1] * p[1] + r[2][2] * p[2] + self.at[2],
        ]
    }

    /// Like Minecraft's ModelPart: move to `offset`, then rotate Z·Y·X.
    fn then(&self, offset: V3, degrees: V3) -> Self {
        let at = self.apply(offset);
        let [x, y, z] = degrees.map(f32::to_radians);
        let rot = mul(&self.rot, &mul(&rot_z(z), &mul(&rot_y(y), &rot_x(x))));
        Self { rot, at }
    }
}

fn mul(a: &Mat, b: &Mat) -> Mat {
    let mut out = [[0.0; 3]; 3];
    for (i, row) in out.iter_mut().enumerate() {
        for (j, cell) in row.iter_mut().enumerate() {
            *cell = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    out
}

fn rot_x(a: f32) -> Mat {
    let (s, c) = a.sin_cos();
    [[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]]
}

fn rot_y(a: f32) -> Mat {
    let (s, c) = a.sin_cos();
    [[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]]
}

fn rot_z(a: f32) -> Mat {
    let (s, c) = a.sin_cos();
    [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]]
}

/// A bone's pivot in Java model space.
fn java_pivot(pivot: [f32; 3]) -> V3 {
    [pivot[0], JAVA_HEIGHT - pivot[1], pivot[2]]
}

/// Java model space to the preview's (y up, front toward +z), and back:
/// a half turn around x, so the same map both ways.
fn flip(p: V3) -> V3 {
    [p[0], -p[1], -p[2]]
}

/// Visible faces of a cosmetic: each bone placed like the game's model
/// parts, each cube's faces laid out with the preview's box UVs.
fn cosmetic_faces(worn: &Worn, view: &impl Fn(V3) -> V3) -> Vec<Face> {
    let g = worn.geometry;
    let (tex_w, tex_h) = (g.texture_width as f32, g.texture_height as f32);
    let mut faces = Vec::new();
    // Roots first, then children once their parent is placed (depth <= bones).
    let mut placed: Vec<(&str, Placement, V3)> = Vec::new();
    for _ in 0..g.bones.len() {
        for bone in &g.bones {
            if placed.iter().any(|(n, _, _)| *n == bone.name) {
                continue;
            }
            let parent = match &bone.parent {
                None => Some((Placement::IDENTITY, [0.0; 3])),
                Some(p) => placed
                    .iter()
                    .find(|(n, _, _)| n == p)
                    .map(|(_, at, pivot)| (*at, *pivot)),
            };
            let Some((parent_at, parent_pivot)) = parent else {
                continue;
            };
            let pivot = java_pivot(bone.pivot);
            let offset = sub(pivot, parent_pivot);
            let at = parent_at.then(offset, bone.rotation);
            placed.push((&bone.name, at, pivot));
            for cube in &bone.cubes {
                faces.extend(cube_faces(
                    cube,
                    pivot,
                    &at,
                    worn.texture,
                    tex_w,
                    tex_h,
                    view,
                ));
            }
        }
    }
    faces
}

#[allow(clippy::too_many_arguments)]
fn cube_faces(
    cube: &arctic_core::cosmetic_models::Cube,
    pivot: V3,
    at: &Placement,
    texture: TextureId,
    tex_w: f32,
    tex_h: f32,
    view: &impl Fn(V3) -> V3,
) -> Vec<Face> {
    let [w, h, d] = cube.size;
    // The cube in the bone's Java space (y down), grown by `inflate`.
    let min = [
        cube.origin[0] - pivot[0] - cube.inflate,
        JAVA_HEIGHT - cube.origin[1] - h - pivot[1] - cube.inflate,
        cube.origin[2] - pivot[2] - cube.inflate,
    ];
    let max = [
        min[0] + w + 2.0 * cube.inflate,
        min[1] + h + 2.0 * cube.inflate,
        min[2] + d + 2.0 * cube.inflate,
    ];
    // The same box in the preview's orientation, so its faces get the
    // preview's (already correct) box-UV layout.
    let (a, b) = (flip(min), flip(max));
    let part = cuboid(
        [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])],
        [w, h, d],
        (0.0, 0.0),
        0.0,
        [0.0; 3],
        0.0,
    );
    let part = Part {
        max: [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])],
        ..part
    };
    // The faces come in the same order as the cube's rectangles (north,
    // south, east, west, up, down); a face without a rectangle isn't drawn.
    let rects = cube.face_rects();
    let boxes = box_faces(&part);
    // The cube's own turn, around its pivot (the middle unless given), in
    // the bone's Java space.
    let turn = if cube.rotation == [0.0; 3] {
        Placement::IDENTITY
    } else {
        let mid = [
            cube.origin[0] + w / 2.0,
            cube.origin[1] + h / 2.0,
            cube.origin[2] + d / 2.0,
        ];
        let at_pivot = sub(java_pivot(cube.pivot.unwrap_or(mid)), pivot);
        let turned = Placement::IDENTITY.then([0.0; 3], cube.rotation);
        Placement {
            at: sub(at_pivot, turned.apply(at_pivot)),
            ..turned
        }
    };
    let place = |p: V3| {
        let world = at.apply(turn.apply(flip(p)));
        view([world[0], JAVA_HEIGHT - world[1], -world[2]])
    };
    let origin = place([0.0; 3]);
    let mut out = Vec::new();
    for ((corners, normal, _), rect) in boxes.into_iter().zip(rects) {
        let Some([u, v, rw, rh]) = rect else {
            continue;
        };
        let n = sub(place(normal), origin);
        if n[2] <= 1e-4 {
            continue;
        }
        let uv = [(u, v), (u + rw, v), (u + rw, v + rh), (u, v + rh)];
        let corners = corners.map(place);
        let depth = corners.iter().map(|c| c[2]).sum::<f32>() / 4.0;
        out.push(Face {
            corners,
            uv: uv.map(|(u, v)| pos2(u / tex_w, v / tex_h)),
            depth,
            texture,
            tint: Color32::WHITE,
            cells: Face::cells_of(&uv),
        });
    }
    out
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
        let faces = build_faces(&head, TextureId::default(), 64.0, 64.0, &view);
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
        assert_eq!(
            build_faces(&head, TextureId::default(), 64.0, 64.0, &view).len(),
            3
        );
    }

    #[test]
    fn faces_are_sorted_far_to_near() {
        let view = |p: V3| rotate_view(p, pose(0.8));
        let faces = build_faces(
            &parts(Variant::Classic, true, 0.2),
            TextureId::default(),
            64.0,
            64.0,
            &view,
        );
        assert!(faces.windows(2).all(|w| w[0].depth <= w[1].depth));
        assert!(!faces.is_empty());
    }

    #[test]
    fn legacy_skins_skip_body_overlays() {
        assert_eq!(parts(Variant::Classic, true, 0.0).len(), 12);
        assert_eq!(parts(Variant::Slim, false, 0.0).len(), 7);
    }

    #[test]
    fn a_halo_sits_above_the_head_facing_the_viewer() {
        let geometry = Geometry::parse(
            br#"{"minecraft:geometry":[{"description":{"texture_width":32,"texture_height":32},
                "bones":[{"name":"head","pivot":[0,24,0],"cubes":[{"origin":[-4,34,-5],"size":[8,1,1],"uv":[0,0]}]}]}]}"#,
        )
        .unwrap();
        let worn = Worn {
            geometry: &geometry,
            texture: TextureId::default(),
        };
        let view = |p: V3| p;
        let faces = cosmetic_faces(&worn, &view);
        assert!(!faces.is_empty());
        // Above the head (top at 32), toward the front (+z in the preview).
        for face in &faces {
            for c in face.corners {
                assert!((34.0..=35.0).contains(&c[1]), "{c:?}");
                assert!((4.0..=5.0).contains(&c[2]), "{c:?}");
            }
        }
    }

    fn faces_of(geometry_json: &str) -> Vec<([V3; 4], [Pos2; 4])> {
        let geometry = arctic_core::cosmetic_models::Geometry::parse(geometry_json.as_bytes()).unwrap();
        let worn = Worn {
            geometry: &geometry,
            texture: TextureId::default(),
        };
        // Seen from the front and from behind, so every face shows once.
        let mut out = Vec::new();
        for yaw in [0.3_f32, 3.4] {
            let view = |p: V3| rotate_view(p, Pose { yaw, pitch: 0.4, swing: 0.0 });
            out.extend(
                cosmetic_faces(&worn, &view)
                    .into_iter()
                    .map(|f| (f.corners, f.uv)),
            );
        }
        out
    }

    fn one_cube(cube: &str) -> String {
        format!(
            r#"{{"minecraft:geometry":[{{"description":{{"texture_width":64,"texture_height":64}},
            "bones":[{{"name":"back","pivot":[0,24,0],"cubes":[{cube}]}}]}}]}}"#
        )
    }

    #[test]
    fn per_face_uv_matches_the_box_layout_it_replaces() {
        // Box UV at (4, 6) for a 6×8×2 cube, written out face by face:
        // w = 6, h = 8, d = 2.
        let boxed = one_cube(r#"{"origin":[-3,14,2],"size":[6,8,2],"uv":[4,6]}"#);
        let faces = one_cube(
            r#"{"origin":[-3,14,2],"size":[6,8,2],"uv":{
                "north":{"uv":[6,8],"uv_size":[6,8]},
                "south":{"uv":[14,8],"uv_size":[6,8]},
                "east":{"uv":[4,8],"uv_size":[2,8]},
                "west":{"uv":[12,8],"uv_size":[2,8]},
                "up":{"uv":[6,6],"uv_size":[6,2]},
                "down":{"uv":[12,6],"uv_size":[6,2]}}}"#,
        );
        assert_eq!(faces_of(&boxed), faces_of(&faces));
    }

    #[test]
    fn a_cube_turned_by_zero_degrees_stays_put_and_a_turn_moves_it() {
        let flat = r#"{"origin":[-4,14,2],"size":[8,8,1],"uv":{"north":{"uv":[0,0],"uv_size":[8,8]}}"#;
        let still = faces_of(&one_cube(&format!(r#"{flat},"rotation":[0,0,0]}}"#)));
        let plain = faces_of(&one_cube(&format!("{flat}}}")));
        assert_eq!(still, plain);
        let turned = faces_of(&one_cube(&format!(
            r#"{flat},"pivot":[0,18,2.5],"rotation":[0,0,90]}}"#
        )));
        assert_ne!(turned, plain);
        // Only the faces that were given are drawn.
        assert!(plain.len() <= 2 && !plain.is_empty());
    }
}
