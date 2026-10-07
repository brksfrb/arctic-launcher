//! Mesh cosmetics: a restricted glTF 2.0 (GLB) subset for sculpted items
//! (wings, shells, horns) that boxes and flat cards cannot make.
//!
//! One file, `<id>.glb`, holds everything: nodes, triangle meshes, materials,
//! embedded PNG textures and one optional looping animation. Nothing outside
//! the file is ever read, and everything is checked here before any client
//! allocates memory for it. The server, the launcher and the game (which has
//! its own Java copy of these rules) accept exactly the same files.
//!
//! # Space
//!
//! Positions are in **model pixels** (16 = one block), the same space as the
//! cuboid `.geo.json` cosmetics: the player stands on y = 0, y is up, the
//! player faces −z and +x is the player's left. A root node named `head`,
//! `body`, `rightArm`, `leftArm`, `rightLeg` or `leftLeg` follows that part
//! of the player (any other name follows the body); vertices are placed for
//! the player at rest, and the part's own movement is applied around its
//! rest pivot, as for bones. Node translation/rotation/scale and the
//! animation work as in glTF (relative to each node's own origin).
//!
//! # Supported
//!
//! Scene nodes (translation, rotation, scale; no matrices), triangle
//! primitives with `POSITION` (and `NORMAL`, `TEXCOORD_0`, `COLOR_0` when
//! present), materials with `baseColorFactor`, `baseColorTexture`,
//! `emissiveTexture`, `emissiveFactor`, `alphaMode` and `doubleSided`
//! (everything else, such as metalness, is ignored: lighting is
//! Minecraft's), embedded PNG images, and one animation of node
//! translation, rotation and scale (`LINEAR` or `STEP`). Without normals
//! the surface is flat-shaded. Skins, morph targets, cameras, extensions,
//! external files and sparse accessors are refused.

use std::collections::HashMap;

use serde_json::Value;

/// Largest file.
pub const MAX_BYTES: usize = 1024 * 1024;
pub const MAX_NODES: usize = 32;
pub const MAX_PRIMITIVES: usize = 16;
/// Triangles in the whole file (drawn for every player on screen: keep it
/// far lower, see the content guide).
pub const MAX_TRIANGLES: usize = 24_000;
pub const MAX_MATERIALS: usize = 6;
pub const MAX_IMAGES: usize = 6;
/// Largest texture side.
pub const MAX_TEXTURE_SIDE: u32 = 1024;
pub const MAX_ANIMATION_SECS: f32 = 10.0;
pub const MAX_KEYFRAMES: usize = 256;
pub const MAX_CHANNELS: usize = 16;
/// Coordinates stay within this many model pixels of the origin.
pub const MAX_COORD: f32 = 256.0;

const GLB_MAGIC: u32 = 0x4654_6C67; // "glTF"
const CHUNK_JSON: u32 = 0x4E4F_534A;
const CHUNK_BIN: u32 = 0x004E_4942;
const MAX_ACCESSOR: usize = 3 * MAX_TRIANGLES;

pub type Mat4 = [[f32; 4]; 4];

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub name: String,
    /// Index of the parent in [`Mesh::nodes`] (parents come first).
    pub parent: Option<usize>,
    pub translation: [f32; 3],
    /// Unit quaternion `[x, y, z, w]`.
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Alpha {
    Opaque,
    /// Pixels below this alpha are not drawn.
    Mask(f32),
    /// Smooth transparency (still cut below a tiny alpha).
    Blend,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Material {
    pub base_color: [f32; 4],
    /// Index into [`Mesh::images`].
    pub texture: Option<usize>,
    pub emissive_texture: Option<usize>,
    pub emissive_factor: [f32; 3],
    pub alpha: Alpha,
    pub double_sided: bool,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            base_color: [1.0; 4],
            texture: None,
            emissive_texture: None,
            emissive_factor: [0.0; 3],
            alpha: Alpha::Opaque,
            double_sided: false,
        }
    }
}

/// Triangles of one mesh primitive, ready to draw: every vertex has a
/// normal, a texture coordinate and a color.
#[derive(Debug, Clone, PartialEq)]
pub struct Primitive {
    pub node: usize,
    pub material: Option<usize>,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub colors: Vec<[f32; 4]>,
    /// Three per triangle.
    pub indices: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Path {
    Translation,
    Rotation,
    Scale,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Channel {
    pub node: usize,
    pub path: Path,
    pub step: bool,
    pub times: Vec<f32>,
    /// Vec3 (translation, scale) or quaternion `[x, y, z, w]` (rotation);
    /// vec3 values keep a 0 in the last place.
    pub values: Vec<[f32; 4]>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Animation {
    pub length: f32,
    pub channels: Vec<Channel>,
}

/// A light that travels across the surface (a "sheen"): a soft band that sweeps along an axis once
/// per `period`, tinted and added on top of the model. Written in the scene's `extras.arctic.sheen`.
#[derive(Debug, Clone, PartialEq)]
pub struct Sheen {
    /// Seconds per sweep.
    pub period: f32,
    pub tint: [f32; 3],
    /// How bright the band gets (0 to 1).
    pub strength: f32,
    /// Half-width of the band, as a fraction of the sweep.
    pub width: f32,
    /// A point's place in the sweep is `(axis · p - origin) / length`, plus the skew term.
    pub axis: [f32; 3],
    pub origin: f32,
    pub length: f32,
    /// Tilts the band: `skew * (skew_axis · p - skew_origin)` (its absolute value when `skew_abs`).
    pub skew_axis: [f32; 3],
    pub skew: f32,
    pub skew_origin: f32,
    pub skew_abs: bool,
}

impl Sheen {
    /// Brightness (0 to 1) of the band at model-space point `p` whose surface faces `facing_z` (the
    /// normal's z), `t` seconds into the loop.
    pub fn at(&self, p: [f32; 3], facing_z: f32, t: f32) -> f32 {
        let sweep = (t / self.period).rem_euclid(1.0);
        let mut place = (self.axis[0] * p[0] + self.axis[1] * p[1] + self.axis[2] * p[2] - self.origin) / self.length;
        let mut tilt = self.skew_axis[0] * p[0] + self.skew_axis[1] * p[1] + self.skew_axis[2] * p[2] - self.skew_origin;
        if self.skew_abs {
            tilt = tilt.abs();
        }
        place += self.skew * tilt;
        let d = (place - sweep) / self.width;
        let band = (-(d * d)).exp();
        let fade = smooth(0.0, 0.12, sweep) * (1.0 - smooth(0.88, 1.0, sweep));
        let facing = 0.62 + 0.38 * facing_z.abs();
        (band * fade * facing * self.strength).clamp(0.0, 1.0)
    }
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Mesh {
    pub nodes: Vec<Node>,
    pub primitives: Vec<Primitive>,
    pub materials: Vec<Material>,
    /// Embedded PNG files.
    pub images: Vec<Vec<u8>>,
    pub animation: Option<Animation>,
    pub sheen: Option<Sheen>,
}

impl Mesh {
    pub fn triangles(&self) -> usize {
        self.primitives.iter().map(|p| p.indices.len() / 3).sum()
    }

    /// Every node's matrix from the model's origin, at `time` seconds into the
    /// animation (`None`: the authored rest pose, with the animation off).
    pub fn world(&self, time: Option<f32>) -> Vec<Mat4> {
        let mut out: Vec<Mat4> = Vec::with_capacity(self.nodes.len());
        for (i, node) in self.nodes.iter().enumerate() {
            let (mut t, mut r, mut s) = (node.translation, node.rotation, node.scale);
            if let (Some(time), Some(anim)) = (time, &self.animation) {
                let time = if anim.length > 0.0 { time % anim.length } else { 0.0 };
                for c in anim.channels.iter().filter(|c| c.node == i) {
                    let v = c.sample(time);
                    match c.path {
                        Path::Translation => t = [v[0], v[1], v[2]],
                        Path::Scale => s = [v[0], v[1], v[2]],
                        Path::Rotation => r = v,
                    }
                }
            }
            let local = trs(t, r, s);
            out.push(match node.parent {
                Some(p) => mul(&out[p], &local),
                None => local,
            });
        }
        out
    }

    /// The smallest box around every vertex at rest: (min, max).
    pub fn bounds(&self) -> ([f32; 3], [f32; 3]) {
        let world = self.world(None);
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for p in &self.primitives {
            for v in &p.positions {
                let w = apply(&world[p.node], *v);
                for i in 0..3 {
                    lo[i] = lo[i].min(w[i]);
                    hi[i] = hi[i].max(w[i]);
                }
            }
        }
        (lo, hi)
    }
}

impl Channel {
    /// The value at `time` (held before the first and after the last key).
    pub fn sample(&self, time: f32) -> [f32; 4] {
        let n = self.times.len();
        if n == 1 || time <= self.times[0] {
            return self.values[0];
        }
        if time >= self.times[n - 1] {
            return self.values[n - 1];
        }
        let i = self.times.partition_point(|t| *t <= time).max(1);
        let (t0, t1) = (self.times[i - 1], self.times[i]);
        let (a, b) = (self.values[i - 1], self.values[i]);
        if self.step || t1 <= t0 {
            return a;
        }
        let k = (time - t0) / (t1 - t0);
        if self.path == Path::Rotation {
            return slerp(a, b, k);
        }
        std::array::from_fn(|c| a[c] + (b[c] - a[c]) * k)
    }
}

fn slerp(a: [f32; 4], mut b: [f32; 4], k: f32) -> [f32; 4] {
    let mut dot: f32 = (0..4).map(|i| a[i] * b[i]).sum();
    if dot < 0.0 {
        b = b.map(|x| -x);
        dot = -dot;
    }
    let out: [f32; 4] = if dot > 0.9995 {
        std::array::from_fn(|i| a[i] + (b[i] - a[i]) * k)
    } else {
        let theta = dot.clamp(-1.0, 1.0).acos();
        let (wa, wb) = (((1.0 - k) * theta).sin(), (k * theta).sin());
        let s = theta.sin();
        std::array::from_fn(|i| (a[i] * wa + b[i] * wb) / s)
    };
    let len = out.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-9);
    out.map(|x| x / len)
}

/// Matrix of translation · rotation · scale (row-major, column vectors).
pub fn trs(t: [f32; 3], q: [f32; 4], s: [f32; 3]) -> Mat4 {
    let [x, y, z, w] = q;
    let r = [
        [1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y - z * w), 2.0 * (x * z + y * w)],
        [2.0 * (x * y + z * w), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z - x * w)],
        [2.0 * (x * z - y * w), 2.0 * (y * z + x * w), 1.0 - 2.0 * (x * x + y * y)],
    ];
    [
        [r[0][0] * s[0], r[0][1] * s[1], r[0][2] * s[2], t[0]],
        [r[1][0] * s[0], r[1][1] * s[1], r[1][2] * s[2], t[1]],
        [r[2][0] * s[0], r[2][1] * s[1], r[2][2] * s[2], t[2]],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

pub fn mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut out = [[0.0; 4]; 4];
    for (i, row) in out.iter_mut().enumerate() {
        for (j, cell) in row.iter_mut().enumerate() {
            *cell = (0..4).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    out
}

/// A point through a matrix.
pub fn apply(m: &Mat4, p: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| m[i][0] * p[0] + m[i][1] * p[1] + m[i][2] * p[2] + m[i][3])
}

/// A direction through a matrix's rotation and scale (no translation, not
/// renormalized).
pub fn apply_dir(m: &Mat4, d: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| m[i][0] * d[0] + m[i][1] * d[1] + m[i][2] * d[2])
}

// ---- Reading --------------------------------------------------------------------------------

/// Parse and check a GLB file; the error says what is wrong.
pub fn parse(bytes: &[u8]) -> Result<Mesh, String> {
    if bytes.len() > MAX_BYTES {
        return Err(format!("larger than {MAX_BYTES} bytes"));
    }
    let (json, bin) = container(bytes)?;
    let doc: Value = serde_json::from_slice(json).map_err(|e| format!("glTF JSON: {e}"))?;
    Reader { doc: &doc, bin }.read()
}

fn u32_at(b: &[u8], at: usize) -> Result<u32, String> {
    b.get(at..at + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or_else(|| "file ends early".to_owned())
}

fn container(b: &[u8]) -> Result<(&[u8], &[u8]), String> {
    if b.len() < 20 || u32_at(b, 0)? != GLB_MAGIC {
        return Err("not a .glb file".into());
    }
    if u32_at(b, 4)? != 2 {
        return Err("only glTF 2.0 is supported".into());
    }
    if u32_at(b, 8)? as usize != b.len() {
        return Err("the header's length is wrong".into());
    }
    let (mut json, mut bin) = (None, &b[0..0]);
    let mut at = 12;
    while at < b.len() {
        let len = u32_at(b, at)? as usize;
        let kind = u32_at(b, at + 4)?;
        let body = b
            .get(at + 8..at + 8 + len)
            .ok_or_else(|| "a chunk runs past the end".to_owned())?;
        match kind {
            CHUNK_JSON if json.is_none() => json = Some(body),
            CHUNK_BIN if at > 12 => bin = body,
            _ => return Err("unexpected chunk".into()),
        }
        at += 8 + len.div_ceil(4) * 4;
    }
    Ok((json.ok_or("no JSON chunk")?, bin))
}

struct Reader<'a> {
    doc: &'a Value,
    bin: &'a [u8],
}

fn arr<'v>(v: &'v Value, key: &str) -> &'v [Value] {
    v.get(key).and_then(Value::as_array).map_or(&[], Vec::as_slice)
}

fn num(v: &Value) -> Result<f32, String> {
    let n = v.as_f64().ok_or("expected a number")?;
    if !n.is_finite() || n.abs() > 1e9 {
        return Err("number out of range".into());
    }
    Ok(n as f32)
}

fn floats<const N: usize>(v: Option<&Value>, default: [f32; N]) -> Result<[f32; N], String> {
    let Some(v) = v else {
        return Ok(default);
    };
    let a = v.as_array().filter(|a| a.len() == N).ok_or("wrong number of values")?;
    let mut out = [0.0; N];
    for (o, x) in out.iter_mut().zip(a) {
        *o = num(x)?;
    }
    Ok(out)
}

fn index(v: &Value, what: &str, len: usize) -> Result<usize, String> {
    let i = v.as_u64().ok_or_else(|| format!("{what} must be an index"))? as usize;
    if i >= len {
        return Err(format!("{what} {i} does not exist"));
    }
    Ok(i)
}

impl Reader<'_> {
    fn read(&self) -> Result<Mesh, String> {
        let d = self.doc;
        if d.pointer("/asset/version").and_then(Value::as_str) != Some("2.0") {
            return Err("asset.version must be 2.0".into());
        }
        if !arr(d, "extensionsUsed").is_empty() || !arr(d, "extensionsRequired").is_empty() {
            return Err("glTF extensions are not supported".into());
        }
        for key in ["cameras", "skins"] {
            if !arr(d, key).is_empty() {
                return Err(format!("{key} are not supported"));
            }
        }
        let buffers = arr(d, "buffers");
        if buffers.len() > 1 || buffers.iter().any(|b| b.get("uri").is_some()) {
            return Err("only the file's own binary chunk may hold data (no external or data: URIs)".into());
        }
        if arr(d, "materials").len() > MAX_MATERIALS {
            return Err(format!("more than {MAX_MATERIALS} materials"));
        }
        if arr(d, "images").len() > MAX_IMAGES {
            return Err(format!("more than {MAX_IMAGES} textures"));
        }
        let images = self.images()?;
        let materials = self.materials(images.len())?;
        let (nodes, order) = self.nodes()?;
        let primitives = self.primitives(&order, materials.len())?;
        let animation = self.animation(&order, nodes.len())?;
        let sheen = self.sheen()?;
        let mesh = Mesh {
            nodes,
            primitives,
            materials,
            images,
            animation,
            sheen,
        };
        if mesh.triangles() == 0 {
            return Err("no triangles".into());
        }
        if mesh.triangles() > MAX_TRIANGLES {
            return Err(format!("{} triangles (at most {MAX_TRIANGLES})", mesh.triangles()));
        }
        Ok(mesh)
    }

    fn images(&self) -> Result<Vec<Vec<u8>>, String> {
        let views = arr(self.doc, "bufferViews");
        let mut out = Vec::new();
        for (n, img) in arr(self.doc, "images").iter().enumerate() {
            if img.get("uri").is_some() {
                return Err(format!("image {n} is an external file; embed it in the .glb"));
            }
            if img.get("mimeType").and_then(Value::as_str) != Some("image/png") {
                return Err(format!("image {n} must be a PNG"));
            }
            let view = &views[index(img.get("bufferView").ok_or("an image has no bufferView")?, "bufferView", views.len())?];
            let bytes = self.view_bytes(view)?.to_vec();
            let info = png::Decoder::new(std::io::Cursor::new(&bytes))
                .read_info()
                .map_err(|_| format!("image {n} is not a PNG"))?;
            let (w, h) = (info.info().width, info.info().height);
            if w == 0 || h == 0 || w > MAX_TEXTURE_SIDE || h > MAX_TEXTURE_SIDE {
                return Err(format!("image {n} is {w}×{h} (at most {MAX_TEXTURE_SIDE}×{MAX_TEXTURE_SIDE})"));
            }
            out.push(bytes);
        }
        Ok(out)
    }

    fn view_bytes(&self, view: &Value) -> Result<&[u8], String> {
        let off = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
        let len = view.get("byteLength").and_then(Value::as_u64).ok_or("a bufferView has no length")? as usize;
        self.bin
            .get(off..off.checked_add(len).ok_or("bufferView overflow")?)
            .ok_or_else(|| "a bufferView runs past the binary chunk".to_owned())
    }

    fn materials(&self, images: usize) -> Result<Vec<Material>, String> {
        let textures = arr(self.doc, "textures");
        let tex = |v: Option<&Value>| -> Result<Option<usize>, String> {
            let Some(v) = v else {
                return Ok(None);
            };
            if v.get("texCoord").and_then(Value::as_u64).unwrap_or(0) != 0 {
                return Err("only TEXCOORD_0 is supported".into());
            }
            let t = &textures[index(v.get("index").ok_or("texture needs an index")?, "texture", textures.len())?];
            Ok(Some(index(t.get("source").ok_or("a texture has no source")?, "image", images)?))
        };
        let mut out = Vec::new();
        for m in arr(self.doc, "materials") {
            let pbr = m.get("pbrMetallicRoughness");
            let alpha = match m.get("alphaMode").and_then(Value::as_str).unwrap_or("OPAQUE") {
                "OPAQUE" => Alpha::Opaque,
                "MASK" => Alpha::Mask(m.get("alphaCutoff").map_or(Ok(0.5), num)?.clamp(0.0, 1.0)),
                "BLEND" => Alpha::Blend,
                other => return Err(format!("alphaMode {other}")),
            };
            out.push(Material {
                base_color: floats(pbr.and_then(|p| p.get("baseColorFactor")), [1.0; 4])?.map(|x| x.clamp(0.0, 1.0)),
                texture: tex(pbr.and_then(|p| p.get("baseColorTexture")))?,
                emissive_texture: tex(m.get("emissiveTexture"))?,
                emissive_factor: floats(m.get("emissiveFactor"), [0.0; 3])?.map(|x| x.clamp(0.0, 1.0)),
                alpha,
                double_sided: m.get("doubleSided").and_then(Value::as_bool).unwrap_or(false),
            });
        }
        Ok(out)
    }

    /// The travelling light of the scene's `extras.arctic.sheen`, if any.
    fn sheen(&self) -> Result<Option<Sheen>, String> {
        let scene = self.doc.get("scene").and_then(Value::as_u64).unwrap_or(0) as usize;
        let Some(s) = arr(self.doc, "scenes")
            .get(scene)
            .and_then(|s| s.get("extras"))
            .and_then(|e| e.get("arctic"))
            .and_then(|a| a.get("sheen"))
        else {
            return Ok(None);
        };
        let one = |key: &str, default: f32, lo: f32, hi: f32| -> Result<f32, String> {
            let v = s.get(key).map_or(Ok(default), num).map_err(|e| format!("sheen {key}: {e}"))?;
            if v < lo || v > hi {
                return Err(format!("sheen {key} must be between {lo} and {hi}"));
            }
            Ok(v)
        };
        let skew = s.get("skew");
        let skew_one = |key: &str| -> Result<f32, String> { skew.and_then(|k| k.get(key)).map_or(Ok(0.0), num) };
        let tint = floats(s.get("tint"), [1.0, 1.0, 1.0]).map_err(|e| format!("sheen tint: {e}"))?;
        if tint.iter().any(|c| !(0.0..=1.0).contains(c)) {
            return Err("sheen tint must be between 0 and 1".into());
        }
        let length = one("length", 1.0, 0.01, 1000.0)?;
        Ok(Some(Sheen {
            period: one("period", 3.0, 0.5, MAX_ANIMATION_SECS)?,
            tint,
            strength: one("strength", 1.0, 0.0, 1.0)?,
            width: one("width", 0.065, 0.01, 1.0)?,
            axis: floats(s.get("axis"), [0.0, 1.0, 0.0]).map_err(|e| format!("sheen axis: {e}"))?,
            origin: one("origin", 0.0, -1000.0, 1000.0)?,
            length,
            skew_axis: floats(skew.and_then(|k| k.get("axis")), [1.0, 0.0, 0.0]).map_err(|e| format!("sheen skew axis: {e}"))?,
            skew: skew_one("amount")?,
            skew_origin: skew_one("origin")?,
            skew_abs: skew.and_then(|k| k.get("abs")).and_then(Value::as_bool).unwrap_or(false),
        }))
    }

    /// The scene's nodes, parents first, and where each glTF node ended up.
    fn nodes(&self) -> Result<(Vec<Node>, HashMap<usize, usize>), String> {
        let raw = arr(self.doc, "nodes");
        let scenes = arr(self.doc, "scenes");
        let scene = self.doc.get("scene").and_then(Value::as_u64).unwrap_or(0) as usize;
        let roots = scenes
            .get(scene)
            .map(|s| arr(s, "nodes"))
            .ok_or("the file has no scene")?;
        let mut order = HashMap::new();
        let mut nodes: Vec<Node> = Vec::new();
        let mut stack: Vec<(usize, Option<usize>)> = roots
            .iter()
            .rev()
            .map(|r| index(r, "node", raw.len()).map(|i| (i, None)))
            .collect::<Result<_, _>>()?;
        while let Some((i, parent)) = stack.pop() {
            if order.contains_key(&i) {
                return Err(format!("node {i} is reached twice (nodes must form a tree)"));
            }
            if nodes.len() >= MAX_NODES {
                return Err(format!("more than {MAX_NODES} nodes"));
            }
            let n = &raw[i];
            for key in ["matrix", "skin", "camera", "weights"] {
                if n.get(key).is_some() {
                    return Err(format!("node {i}: {key} is not supported (use translation, rotation, scale)"));
                }
            }
            let mut rotation = floats(n.get("rotation"), [0.0, 0.0, 0.0, 1.0])?;
            let len = rotation.iter().map(|x| x * x).sum::<f32>().sqrt();
            if len < 1e-6 {
                return Err(format!("node {i}: rotation is zero"));
            }
            rotation = rotation.map(|x| x / len);
            let translation = floats(n.get("translation"), [0.0; 3])?;
            let scale = floats(n.get("scale"), [1.0; 3])?;
            if translation.iter().any(|x| x.abs() > MAX_COORD) || scale.iter().any(|x| x.abs() > 64.0) {
                return Err(format!("node {i}: translation or scale is too large"));
            }
            order.insert(i, nodes.len());
            nodes.push(Node {
                name: n.get("name").and_then(Value::as_str).unwrap_or("").chars().take(48).collect(),
                parent,
                translation,
                rotation,
                scale,
            });
            let me = nodes.len() - 1;
            for c in arr(n, "children").iter().rev() {
                stack.push((index(c, "node", raw.len())?, Some(me)));
            }
        }
        // `stack` visits depth-first, so a parent always comes before its children.
        Ok((nodes, order))
    }

    fn primitives(&self, order: &HashMap<usize, usize>, materials: usize) -> Result<Vec<Primitive>, String> {
        let meshes = arr(self.doc, "meshes");
        let mut out = Vec::new();
        let mut triangles = 0;
        for (gi, n) in arr(self.doc, "nodes").iter().enumerate() {
            let (Some(&node), Some(m)) = (order.get(&gi), n.get("mesh")) else {
                continue;
            };
            let mesh = &meshes[index(m, "mesh", meshes.len())?];
            if mesh.get("weights").is_some() {
                return Err("morph targets are not supported".into());
            }
            for prim in arr(mesh, "primitives") {
                if out.len() >= MAX_PRIMITIVES {
                    return Err(format!("more than {MAX_PRIMITIVES} mesh primitives"));
                }
                let p = self.primitive(node, prim, materials)?;
                triangles += p.indices.len() / 3;
                if triangles > MAX_TRIANGLES {
                    return Err(format!("more than {MAX_TRIANGLES} triangles"));
                }
                out.push(p);
            }
        }
        Ok(out)
    }

    fn primitive(&self, node: usize, prim: &Value, materials: usize) -> Result<Primitive, String> {
        if prim.get("mode").and_then(Value::as_u64).unwrap_or(4) != 4 {
            return Err("only triangle lists are supported".into());
        }
        if prim.get("targets").is_some() {
            return Err("morph targets are not supported".into());
        }
        let attrs = prim.get("attributes").and_then(Value::as_object).ok_or("a primitive has no attributes")?;
        for k in attrs.keys() {
            if !matches!(k.as_str(), "POSITION" | "NORMAL" | "TANGENT" | "TEXCOORD_0" | "TEXCOORD_1" | "COLOR_0") {
                return Err(format!("attribute {k} is not supported"));
            }
        }
        let acc = |k: &str| attrs.get(k).and_then(Value::as_u64).map(|i| i as usize);
        let position = acc("POSITION").ok_or("a primitive has no POSITION")?;
        let mut positions: Vec<[f32; 3]> = self.vecs::<3>(position, false)?;
        let n = positions.len();
        if positions.iter().flatten().any(|c| c.abs() > MAX_COORD) {
            return Err(format!("a position is more than {MAX_COORD} pixels from the origin"));
        }
        let mut normals = acc("NORMAL").map(|i| self.vecs::<3>(i, false)).transpose()?;
        let mut uvs = acc("TEXCOORD_0").map(|i| self.vecs::<2>(i, true)).transpose()?;
        let mut colors = match acc("COLOR_0") {
            Some(i) => Some(self.colors(i)?),
            None => None,
        };
        for len in [normals.as_ref().map(Vec::len), uvs.as_ref().map(Vec::len), colors.as_ref().map(Vec::len)]
            .into_iter()
            .flatten()
        {
            if len != n {
                return Err("a primitive's attributes have different lengths".into());
            }
        }
        let mut indices: Vec<u32> = match prim.get("indices").and_then(Value::as_u64) {
            Some(i) => self.indices(i as usize)?,
            None => (0..n as u32).collect(),
        };
        if indices.len() % 3 != 0 || indices.is_empty() {
            return Err("a primitive's index count is not a multiple of 3".into());
        }
        if indices.iter().any(|&i| i as usize >= n) {
            return Err("an index points past the vertices".into());
        }
        let material = match prim.get("material") {
            Some(m) => Some(index(m, "material", materials)?),
            None => None,
        };
        // Without normals: flat shading, so every triangle needs its own vertices.
        if normals.is_none() {
            let pick = |v: &Vec<_>| -> Vec<_> { indices.iter().map(|&i| v[i as usize]).collect() };
            let flat_pos: Vec<[f32; 3]> = pick(&positions);
            uvs = uvs.map(|u| indices.iter().map(|&i| u[i as usize]).collect());
            colors = colors.map(|c| indices.iter().map(|&i| c[i as usize]).collect());
            let mut flat = Vec::with_capacity(flat_pos.len());
            for t in flat_pos.chunks_exact(3) {
                let nn = face_normal(t[0], t[1], t[2]);
                flat.extend([nn, nn, nn]);
            }
            positions = flat_pos;
            indices = (0..positions.len() as u32).collect();
            normals = Some(flat);
        }
        let normals: Vec<[f32; 3]> = normals
            .unwrap_or_default()
            .into_iter()
            .map(|v| {
                let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                if len < 1e-6 { [0.0, 1.0, 0.0] } else { v.map(|x| x / len) }
            })
            .collect();
        Ok(Primitive {
            node,
            material,
            normals,
            uvs: uvs.unwrap_or_else(|| vec![[0.0; 2]; positions.len()]),
            colors: colors.unwrap_or_else(|| vec![[1.0; 4]; positions.len()]),
            positions,
            indices,
        })
    }

    /// Where an accessor's elements are: (bytes, count, element size, stride, component type, normalized).
    #[allow(clippy::type_complexity)]
    fn accessor(&self, i: usize, kind: &str, components: &[u64]) -> Result<(&[u8], usize, usize, usize, u64, bool), String> {
        let accessors = arr(self.doc, "accessors");
        let views = arr(self.doc, "bufferViews");
        let a = accessors.get(i).ok_or("an accessor does not exist")?;
        if a.get("sparse").is_some() {
            return Err("sparse accessors are not supported".into());
        }
        if a.get("type").and_then(Value::as_str) != Some(kind) {
            return Err(format!("an accessor must be {kind}"));
        }
        let comp = a.get("componentType").and_then(Value::as_u64).ok_or("accessor has no componentType")?;
        if !components.contains(&comp) {
            return Err(format!("accessor component type {comp} is not supported here"));
        }
        let count = a.get("count").and_then(Value::as_u64).ok_or("accessor has no count")? as usize;
        if count == 0 || count > MAX_ACCESSOR {
            return Err(format!("an accessor has {count} elements (1 to {MAX_ACCESSOR})"));
        }
        let view = &views[index(a.get("bufferView").ok_or("accessor has no bufferView")?, "bufferView", views.len())?];
        let bytes = self.view_bytes(view)?;
        let ncomp = match kind {
            "SCALAR" => 1,
            "VEC2" => 2,
            "VEC3" => 3,
            "VEC4" => 4,
            _ => return Err("unsupported accessor type".into()),
        };
        let csize = match comp {
            5121 => 1,
            5123 => 2,
            _ => 4,
        };
        let elem = ncomp * csize;
        let stride = view.get("byteStride").and_then(Value::as_u64).map_or(elem, |s| s as usize);
        if stride < elem || stride > 252 {
            return Err("bad byteStride".into());
        }
        let off = a.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
        let end = off + stride * (count - 1) + elem;
        if end > bytes.len() {
            return Err("an accessor runs past its bufferView".into());
        }
        let normalized = a.get("normalized").and_then(Value::as_bool).unwrap_or(false);
        Ok((&bytes[off..end], count, elem, stride, comp, normalized))
    }

    /// `N`-component float vectors (also normalized 8/16-bit integers when allowed).
    fn vecs<const N: usize>(&self, i: usize, allow_int: bool) -> Result<Vec<[f32; N]>, String> {
        let kind = format!("VEC{N}");
        let comps: &[u64] = if allow_int { &[5126, 5121, 5123] } else { &[5126] };
        let (b, count, elem, stride, comp, normalized) = self.accessor(i, &kind, comps)?;
        if comp != 5126 && !normalized {
            return Err("integer attributes must be normalized".into());
        }
        let mut out = Vec::with_capacity(count);
        for e in 0..count {
            let at = e * stride;
            out.push(read_vec::<N>(&b[at..at + elem], comp)?);
        }
        Ok(out)
    }

    /// A list of single floats (animation times).
    fn scalars(&self, i: usize) -> Result<Vec<f32>, String> {
        let (b, count, _, stride, _, _) = self.accessor(i, "SCALAR", &[5126])?;
        let mut out = Vec::with_capacity(count);
        for e in 0..count {
            let v = f32::from_le_bytes([b[e * stride], b[e * stride + 1], b[e * stride + 2], b[e * stride + 3]]);
            if !v.is_finite() {
                return Err("an animation time is not a finite number".into());
            }
            out.push(v);
        }
        Ok(out)
    }

    fn colors(&self, i: usize) -> Result<Vec<[f32; 4]>, String> {
        let accessors = arr(self.doc, "accessors");
        let kind = accessors
            .get(i)
            .and_then(|a| a.get("type"))
            .and_then(Value::as_str)
            .ok_or("an accessor does not exist")?;
        match kind {
            "VEC4" => self.vecs::<4>(i, true),
            "VEC3" => Ok(self.vecs::<3>(i, true)?.into_iter().map(|c| [c[0], c[1], c[2], 1.0]).collect()),
            _ => Err("COLOR_0 must be VEC3 or VEC4".into()),
        }
    }

    fn indices(&self, i: usize) -> Result<Vec<u32>, String> {
        let (b, count, elem, stride, comp, _) = self.accessor(i, "SCALAR", &[5121, 5123, 5125])?;
        let mut out = Vec::with_capacity(count);
        for e in 0..count {
            let s = &b[e * stride..e * stride + elem];
            out.push(match comp {
                5121 => u32::from(s[0]),
                5123 => u32::from(u16::from_le_bytes([s[0], s[1]])),
                _ => u32::from_le_bytes([s[0], s[1], s[2], s[3]]),
            });
        }
        Ok(out)
    }

    fn animation(&self, order: &HashMap<usize, usize>, nodes: usize) -> Result<Option<Animation>, String> {
        let list = arr(self.doc, "animations");
        if list.len() > 1 {
            return Err("only one animation is supported".into());
        }
        let Some(a) = list.first() else {
            return Ok(None);
        };
        let samplers = arr(a, "samplers");
        let mut channels = Vec::new();
        let mut length = 0.0f32;
        for c in arr(a, "channels") {
            if channels.len() >= MAX_CHANNELS {
                return Err(format!("more than {MAX_CHANNELS} animation channels"));
            }
            let target = c.get("target").ok_or("a channel has no target")?;
            let raw = index(target.get("node").ok_or("a channel has no node")?, "node", arr(self.doc, "nodes").len())?;
            let node = *order.get(&raw).ok_or("an animation moves a node that is not in the scene")?;
            debug_assert!(node < nodes);
            let path = match target.get("path").and_then(Value::as_str) {
                Some("translation") => Path::Translation,
                Some("rotation") => Path::Rotation,
                Some("scale") => Path::Scale,
                other => return Err(format!("animation path {other:?} is not supported")),
            };
            let s = &samplers[index(c.get("sampler").ok_or("a channel has no sampler")?, "sampler", samplers.len())?];
            let step = match s.get("interpolation").and_then(Value::as_str).unwrap_or("LINEAR") {
                "LINEAR" => false,
                "STEP" => true,
                other => return Err(format!("{other} interpolation is not supported (use LINEAR or STEP)")),
            };
            let input = index(s.get("input").ok_or("a sampler has no input")?, "accessor", arr(self.doc, "accessors").len())?;
            let times = self.scalars(input)?;
            if times.len() > MAX_KEYFRAMES
                || times.iter().any(|t| !t.is_finite() || *t < 0.0)
                || times.windows(2).any(|w| w[1] < w[0])
            {
                return Err("animation times must be increasing, finite, and at most 256 keys".into());
            }
            let out_acc = index(s.get("output").ok_or("a sampler has no output")?, "accessor", arr(self.doc, "accessors").len())?;
            let values: Vec<[f32; 4]> = if path == Path::Rotation {
                self.vecs::<4>(out_acc, false)?
            } else {
                self.vecs::<3>(out_acc, false)?.into_iter().map(|v| [v[0], v[1], v[2], 0.0]).collect()
            };
            if values.len() != times.len() {
                return Err("an animation sampler's input and output differ in length".into());
            }
            let values = if path == Path::Rotation {
                values
                    .into_iter()
                    .map(|q| {
                        let l = q.iter().map(|x| x * x).sum::<f32>().sqrt();
                        if l < 1e-6 { [0.0, 0.0, 0.0, 1.0] } else { q.map(|x| x / l) }
                    })
                    .collect()
            } else {
                if values.iter().flatten().any(|x| x.abs() > MAX_COORD) {
                    return Err("an animated translation or scale is too large".into());
                }
                values
            };
            length = length.max(*times.last().unwrap_or(&0.0));
            channels.push(Channel {
                node,
                path,
                step,
                times,
                values,
            });
        }
        if channels.is_empty() {
            return Ok(None);
        }
        if length <= 0.0 || length > MAX_ANIMATION_SECS {
            return Err(format!("the animation must last 0 to {MAX_ANIMATION_SECS} seconds"));
        }
        Ok(Some(Animation { length, channels }))
    }
}

fn read_vec<const N: usize>(b: &[u8], comp: u64) -> Result<[f32; N], String> {
    let mut out = [0.0; N];
    for (i, o) in out.iter_mut().enumerate() {
        *o = match comp {
            5126 => {
                let v = f32::from_le_bytes([b[i * 4], b[i * 4 + 1], b[i * 4 + 2], b[i * 4 + 3]]);
                if !v.is_finite() {
                    return Err("a coordinate is not a finite number".into());
                }
                v
            }
            5121 => f32::from(b[i]) / 255.0,
            _ => f32::from(u16::from_le_bytes([b[i * 2], b[i * 2 + 1]])) / 65535.0,
        };
    }
    Ok(out)
}

/// The unit normal of the triangle a, b, c (counter-clockwise).
pub fn face_normal(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if len < 1e-12 { [0.0, 1.0, 0.0] } else { n.map(|x| x / len) }
}

/// A one-triangle `.glb` on a node called `body`, for other crates' tests.
pub fn sample_glb() -> Vec<u8> {
    let mut bin = Vec::new();
    for v in [0.0f32, 0.0, 0.0, 10.0, 0.0, 0.0, 0.0, 10.0, 0.0] {
        bin.extend_from_slice(&v.to_le_bytes());
    }
    let doc = serde_json::json!({
        "asset": {"version": "2.0"},
        "scene": 0, "scenes": [{"nodes": [0]}],
        "nodes": [{"name": "body", "mesh": 0}],
        "meshes": [{"primitives": [{"attributes": {"POSITION": 0}}]}],
        "buffers": [{"byteLength": 36}],
        "bufferViews": [{"buffer": 0, "byteOffset": 0, "byteLength": 36}],
        "accessors": [{"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3"}],
    });
    let mut json = serde_json::to_vec(&doc).unwrap_or_default();
    while json.len() % 4 != 0 {
        json.push(b' ');
    }
    let total = 12 + 8 + json.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(&GLB_MAGIC.to_le_bytes());
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(&CHUNK_JSON.to_le_bytes());
    out.extend_from_slice(&json);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(&CHUNK_BIN.to_le_bytes());
    out.extend_from_slice(&bin);
    out
}

#[cfg(test)]
mod tests;
