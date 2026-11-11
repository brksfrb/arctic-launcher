//! Owned Java model staging, not an engine Mesh or GPU upload receipt.
//! ARCGEO01..10 use explicit little-endian values; no native C++ layouts appear.
use std::{
    collections::{HashMap, HashSet},
    ops::Range,
};

pub const MAX_BYTES: usize = 96 * 1024 * 1024;
const MAX_VERTICES: usize = 400_000;
const MAX_PIXELS: usize = 16_777_216;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pass {
    General,
    CosmeticBase,
    CosmeticGlow,
    CapeSolid,
    WingCutout,
    SkinTranslucent,
    BentSkin,
}
impl Pass {
    pub fn skin_geometry(self) -> bool {
        matches!(self, Self::SkinTranslucent | Self::BentSkin)
    }
    pub fn layer_geometry(self) -> bool {
        matches!(
            self,
            Self::CosmeticBase
                | Self::CosmeticGlow
                | Self::CapeSolid
                | Self::WingCutout
                | Self::BentSkin
        )
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PositionSpace {
    Unknown,
    World,
    CameraRelative,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RenderFactory {
    EntityTranslucent,
    EntityTranslucentEmissive,
    EntitySolid,
    ArmorCutoutNoCull,
}
impl RenderFactory {
    fn for_pass(pass: Pass) -> Option<Self> {
        match pass {
            Pass::CosmeticBase | Pass::SkinTranslucent | Pass::BentSkin => {
                Some(Self::EntityTranslucent)
            }
            Pass::CosmeticGlow => Some(Self::EntityTranslucentEmissive),
            Pass::CapeSolid => Some(Self::EntitySolid),
            Pass::WingCutout => Some(Self::ArmorCutoutNoCull),
            Pass::General => None,
        }
    }
}
/// Owned factory-cache identity, not a native pointer or prepared-GPU equality.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderReference {
    pub factory: RenderFactory,
    pub id: u64,
}
/// Frame-local submitModel boundary. Translation uses the packet's position
/// space; it is not certified to be Java's submitted view-space pose.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModelSubmission {
    pub id: u16,
    pub translation: [f32; 3],
    pub glint: Option<GlintSelection>,
}
/// Exact f32 bit patterns from an explicit selected Java glint observation.
/// The last three words are the independently supplied model view translation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GlintUniforms {
    words: [u32; 40],
}
impl GlintUniforms {
    pub fn values(self) -> [f32; 40] {
        self.words.map(f32::from_bits)
    }
    pub fn shader_key(self) -> [u32; 37] {
        let mut key = [0; 37];
        key.copy_from_slice(&self.words[..37]);
        key
    }
    pub fn view_translation(self) -> [f32; 3] {
        [
            f32::from_bits(self.words[37]),
            f32::from_bits(self.words[38]),
            f32::from_bits(self.words[39]),
        ]
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GlintSelection {
    Inactive,
    Active(GlintUniforms),
}
/// Source-port collector output, not an observed engine phase/lease. Each
/// phase has one supported feature type; buckets have already been traversed
/// by the Java producer using its retained owned RenderType reference keys.
#[derive(Debug)]
pub struct SimplePhase {
    pub order: u8,
    pub stage: u8,
    pub batches: Vec<usize>,
}
#[derive(Debug)]
pub struct SimplePhases {
    pub epoch: u64,
    pub phases: Vec<SimplePhase>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReplayTime {
    pub time_ms: u64,
    pub remainder: u32,
    pub denominator: u32,
}

#[derive(Debug)]
pub struct Batch {
    pub actor: usize,
    pub pass: Pass,
    pub translucent: bool,
    pub emissive: bool,
    /// False for preview fallback and older packets without scene provenance.
    pub scene_light_known: bool,
    pub skin_visibility_known: bool,
    pub layer_visibility_known: bool,
    pub texture: Option<usize>,
    pub tint: [f32; 4],
    pub skin_overlay: Option<[f32; 4]>,
    pub logical_texture: Option<String>,
    pub render_reference: Option<RenderReference>,
    pub model_submission: Option<ModelSubmission>,
    pub vertex_count: usize,
    pub index_count: usize,
    attributes: Range<usize>,
    indices: Range<usize>,
}
#[derive(Debug)]
pub struct Texture {
    pub width: usize,
    pub height: usize,
    argb: Range<usize>,
}
/// Copied Java entity shader cohort, not a reinterpretation of Bedrock named
/// constants or proof of a live material/feature phase. ARCGEO02 and later.
#[derive(Debug)]
pub struct EntityMaterial {
    pub lightmap: [[f32; 4]; 256],
    pub color_modulator: [f32; 4],
    pub light_directions: [[f32; 3]; 2],
    pub fog_position: [f32; 12],
    pub overlay_color: [f32; 4],
    pub fog_color: [f32; 4],
    pub fog_ranges: [f32; 4],
}
#[derive(Debug)]
pub struct Frame {
    pub session: String,
    pub sequence: u64,
    pub clock_ms: u64,
    pub cosmetic_time_ms: f64,
    pub replay_time: Option<ReplayTime>,
    pub width: u32,
    pub height: u32,
    pub space: PositionSpace,
    pub dimension: String,
    pub camera: [f64; 3],
    pub view_projection: [f32; 16],
    pub equipment_projection: Option<[f32; 16]>,
    pub material: Option<EntityMaterial>,
    pub actors: Vec<String>,
    pub batches: Vec<Batch>,
    pub textures: Vec<Texture>,
    pub simple_phases: Option<SimplePhases>,
    bytes: Vec<u8>,
}

struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Cursor<'a> {
    fn take(&mut self, size: usize) -> Result<&'a [u8], &'static str> {
        let end = self
            .at
            .checked_add(size)
            .ok_or("geometry length overflow")?;
        let value = self
            .bytes
            .get(self.at..end)
            .ok_or("truncated geometry packet")?;
        self.at = end;
        Ok(value)
    }
    fn byte(&mut self) -> Result<u8, &'static str> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, &'static str> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, &'static str> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, &'static str> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn f32(&mut self) -> Result<f32, &'static str> {
        let value = f32::from_bits(self.u32()?);
        if !value.is_finite() {
            return Err("nonfinite geometry float");
        }
        Ok(value)
    }
    fn f64(&mut self) -> Result<f64, &'static str> {
        let value = f64::from_bits(self.u64()?);
        if !value.is_finite() {
            return Err("nonfinite geometry double");
        }
        Ok(value)
    }
    fn matrix(&mut self) -> Result<[f32; 16], &'static str> {
        let mut value = [0.; 16];
        for entry in &mut value {
            *entry = self.f32()?;
        }
        Ok(value)
    }
    fn floats<const N: usize>(&mut self) -> Result<[f32; N], &'static str> {
        let mut values = [0.; N];
        for value in &mut values {
            *value = self.f32()?;
        }
        Ok(values)
    }
    fn material(&mut self) -> Result<EntityMaterial, &'static str> {
        let mut lightmap = [[0.; 4]; 256];
        for texel in &mut lightmap {
            *texel = self.floats::<4>()?;
            if !texel.iter().all(|value| (0. ..=1.).contains(value)) {
                return Err("invalid geometry lightmap texel");
            }
        }
        let color_modulator = self.floats::<4>()?;
        let light_directions = [self.floats::<3>()?, self.floats::<3>()?];
        let fog_position = self.floats::<12>()?;
        let overlay_color = self.floats::<4>()?;
        let fog_color = self.floats::<4>()?;
        let fog_ranges = self.floats::<4>()?;
        if !overlay_color.iter().all(|value| (0. ..=1.).contains(value))
            || fog_ranges[1] < fog_ranges[0]
            || fog_ranges[3] < fog_ranges[2]
        {
            return Err("invalid geometry overlay/fog inputs");
        }
        Ok(EntityMaterial {
            lightmap,
            color_modulator,
            light_directions,
            fog_position,
            overlay_color,
            fog_color,
            fog_ranges,
        })
    }
    fn text(&mut self, max: usize) -> Result<String, &'static str> {
        let size = self.u32()? as usize;
        if size > max {
            return Err("geometry string exceeds bound");
        }
        Ok(std::str::from_utf8(self.take(size)?)
            .map_err(|_| "invalid geometry UTF-8")?
            .to_owned())
    }
}

impl Frame {
    /// Decode only for the requesting consumer's exact world/frame identity.
    /// The byte allocation remains Rust-owned. It must never be moved into an
    /// engine vector header or passed to a C++ destructor.
    pub fn decode(
        bytes: Vec<u8>,
        expected_session: &str,
        expected_sequence: u64,
    ) -> Result<Self, &'static str> {
        if bytes.len() > MAX_BYTES {
            return Err("geometry packet exceeds 96 MiB");
        }
        let mut input = Cursor {
            bytes: &bytes,
            at: 0,
        };
        let version = match input.take(8)? {
            b"ARCGEO01" => 1,
            b"ARCGEO02" => 2,
            b"ARCGEO03" => 3,
            b"ARCGEO04" => 4,
            b"ARCGEO05" => 5,
            b"ARCGEO06" => 6,
            b"ARCGEO07" => 7,
            b"ARCGEO08" => 8,
            b"ARCGEO09" => 9,
            b"ARCGEO10" => 10,
            _ => return Err("unsupported geometry packet"),
        };
        let session = input.text(64)?;
        if session.is_empty()
            || !session
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
            || session != expected_session
        {
            return Err("geometry belongs to another session");
        }
        let sequence = input.u64()?;
        let clock_ms = input.u64()?;
        let cosmetic_time_ms = input.f64()?;
        if sequence != expected_sequence
            || sequence > i64::MAX as u64
            || clock_ms > i64::MAX as u64
            || cosmetic_time_ms < 0.
            || cosmetic_time_ms > i64::MAX as f64
        {
            return Err("invalid or stale geometry clock/frame");
        }
        let replay_time = match input.byte()? {
            0 => {
                if cosmetic_time_ms.to_bits() != (clock_ms as f64).to_bits() {
                    return Err("live geometry clock disagrees");
                }
                None
            }
            1 => {
                let time_ms = input.u64()?;
                let remainder = input.u32()?;
                let denominator = input.u32()?;
                if time_ms > 86_400_000
                    || !(1..=1000).contains(&denominator)
                    || remainder >= denominator
                    || time_ms == 86_400_000 && remainder != 0
                {
                    return Err("invalid geometry replay fraction");
                }
                let exact = time_ms as f64 + remainder as f64 / denominator as f64;
                if exact.to_bits() != cosmetic_time_ms.to_bits() {
                    return Err("geometry replay math time disagrees");
                }
                Some(ReplayTime {
                    time_ms,
                    remainder,
                    denominator,
                })
            }
            _ => return Err("invalid geometry replay clock flag"),
        };
        let width = input.u32()?;
        let height = input.u32()?;
        if !(1..=3840).contains(&width) || !(1..=2160).contains(&height) {
            return Err("invalid geometry surface");
        }
        let space = match input.byte()? {
            0 => PositionSpace::Unknown,
            1 => PositionSpace::World,
            2 => PositionSpace::CameraRelative,
            _ => return Err("invalid geometry coordinate space"),
        };
        let dimension = input.text(256)?;
        if dimension.encode_utf16().count() > 64 {
            return Err("geometry dimension exceeds bound");
        }
        let mut camera = [0.; 3];
        for value in &mut camera {
            *value = input.f64()?;
            if value.abs() > 32_000_000. {
                return Err("invalid geometry camera");
            }
        }
        let view_projection = input.matrix()?;
        let equipment_projection = match input.byte()? {
            0 => None,
            1 => Some(input.matrix()?),
            _ => return Err("invalid equipment matrix flag"),
        };
        if equipment_projection.is_some() && space != PositionSpace::CameraRelative {
            return Err("equipment camera requires relative positions");
        }
        let material = if version >= 2 {
            match input.byte()? {
                0 => None,
                1 => Some(input.material()?),
                _ => return Err("invalid geometry material presence"),
            }
        } else {
            None
        };
        if material.is_some() && space == PositionSpace::Unknown {
            return Err("geometry material requires a declared position space");
        }
        let actor_count = input.u32()? as usize;
        if actor_count > 128 {
            return Err("geometry actor budget exceeded");
        }
        let mut actors = Vec::with_capacity(actor_count);
        let mut identities = HashSet::new();
        for _ in 0..actor_count {
            let raw = input.take(36)?;
            if !raw.iter().enumerate().all(|(i, &c)| {
                if [8, 13, 18, 23].contains(&i) {
                    c == b'-'
                } else {
                    c.is_ascii_digit() || (b'a'..=b'f').contains(&c)
                }
            }) {
                return Err("invalid geometry actor UUID");
            }
            let id = std::str::from_utf8(raw)
                .map_err(|_| "invalid geometry actor encoding")?
                .to_owned();
            if !identities.insert(id.clone()) {
                return Err("duplicate geometry actor");
            }
            actors.push(id);
        }
        let batch_count = input.u32()? as usize;
        let texture_count = input.u32()? as usize;
        if batch_count > 2048 || texture_count > 32 {
            return Err("geometry draw/texture budget exceeded");
        }
        let mut batches = Vec::with_capacity(batch_count);
        let mut total_vertices = 0usize;
        let mut logical_bindings: HashMap<String, Option<usize>> = HashMap::new();
        let mut render_objects: HashMap<u64, (RenderFactory, String)> = HashMap::new();
        let mut render_factories: HashMap<(RenderFactory, String), u64> = HashMap::new();
        let mut models: HashMap<u16, (usize, Pass, [u32; 3])> = HashMap::new();
        let mut model_glints: HashMap<u16, Option<GlintSelection>> = HashMap::new();
        let mut last_model = None;
        let mut highest_model = 0u16;
        let mut previous_actor = 0;
        for _ in 0..batch_count {
            let actor = input.u16()? as usize;
            if actor >= actor_count || actor < previous_actor {
                return Err("invalid geometry actor order");
            }
            previous_actor = actor;
            let pass = match input.byte()? {
                1 => Pass::General,
                2 => Pass::CosmeticBase,
                3 => Pass::CosmeticGlow,
                4 => Pass::CapeSolid,
                5 => Pass::WingCutout,
                6 if version >= 3 => Pass::SkinTranslucent,
                7 if version >= 4 => Pass::BentSkin,
                _ => return Err("invalid geometry pass"),
            };
            let flags = input.byte()?;
            let material_flags = flags & 3;
            let allowed_flags: u8 = if version >= 6 {
                31
            } else if version >= 4 {
                15
            } else {
                7
            };
            if flags & !allowed_flags != 0
                || flags & 8 != 0 && !pass.skin_geometry()
                || flags & 16 != 0 && !pass.layer_geometry()
                || matches!(
                    pass,
                    Pass::CosmeticBase | Pass::SkinTranslucent | Pass::BentSkin
                ) && material_flags != 1
                || pass == Pass::CosmeticGlow && material_flags != 3
                || matches!(pass, Pass::CapeSolid | Pass::WingCutout) && material_flags != 0
            {
                return Err("invalid geometry pass flags");
            }
            if pass.skin_geometry() && space == PositionSpace::Unknown {
                return Err("skin geometry requires declared position space");
            }
            let raw_texture = input.u32()?;
            let texture = if raw_texture == u32::MAX {
                None
            } else {
                let index = raw_texture as usize;
                if index >= texture_count {
                    return Err("invalid geometry texture index");
                }
                Some(index)
            };
            let mut tint = [0.; 4];
            for value in &mut tint {
                *value = input.f32()?;
                if !(0. ..=1.).contains(&*value) {
                    return Err("invalid geometry tint");
                }
            }
            let vertex_count = input.u32()? as usize;
            let index_count = input.u32()? as usize;
            let skin_overlay = if pass == Pass::SkinTranslucent {
                let value = input.floats::<4>()?;
                if !value.iter().all(|value| (0. ..=1.).contains(value)) {
                    return Err("invalid geometry skin overlay");
                }
                Some(value)
            } else {
                None
            };
            let logical_texture = if version >= 5 {
                match input.byte()? {
                    0 => None,
                    1 => {
                        let id = input.text(256)?;
                        let (namespace, path) = id
                            .split_once(':')
                            .ok_or("geometry texture identifier has no namespace")?;
                        let namespace_char = |c: u8| {
                            c.is_ascii_lowercase() || c.is_ascii_digit() || b"_.-".contains(&c)
                        };
                        if namespace.is_empty()
                            || path.is_empty()
                            || !namespace.bytes().all(&namespace_char)
                            || !path.bytes().all(|c| namespace_char(c) || c == b'/')
                        {
                            return Err("invalid geometry texture identifier");
                        }
                        Some(id)
                    }
                    _ => return Err("invalid geometry texture identifier presence"),
                }
            } else {
                None
            };
            if let Some(id) = &logical_texture {
                if let Some(previous) = logical_bindings.get(id) {
                    if *previous != texture {
                        return Err("geometry texture identifier has conflicting bindings");
                    }
                } else {
                    logical_bindings.insert(id.clone(), texture);
                }
            }
            let render_reference = if version >= 7 {
                match input.byte()? {
                    0 => None,
                    1 => {
                        let factory = match input.byte()? {
                            1 => RenderFactory::EntityTranslucent,
                            2 => RenderFactory::EntityTranslucentEmissive,
                            3 => RenderFactory::EntitySolid,
                            4 => RenderFactory::ArmorCutoutNoCull,
                            _ => return Err("invalid Java render-type factory"),
                        };
                        let id = input.u64()?;
                        if !(1..=65_536).contains(&id)
                            || RenderFactory::for_pass(pass) != Some(factory)
                        {
                            return Err(
                                "Java render reference disagrees with its material factory",
                            );
                        }
                        let texture = logical_texture
                            .as_ref()
                            .ok_or("Java render reference has no texture identifier")?;
                        let recipe = (factory, texture.clone());
                        if render_objects
                            .get(&id)
                            .is_some_and(|previous| previous != &recipe)
                            || render_factories
                                .get(&recipe)
                                .is_some_and(|previous| *previous != id)
                        {
                            return Err("Java render reference has conflicting factory arguments");
                        }
                        render_objects.insert(id, recipe.clone());
                        render_factories.insert(recipe, id);
                        Some(RenderReference { factory, id })
                    }
                    _ => return Err("invalid Java render reference presence"),
                }
            } else {
                None
            };
            let mut model_submission = if version >= 8 {
                match input.byte()? {
                    0 => None,
                    1 => {
                        let id = input.u16()?;
                        let translation = input.floats::<3>()?;
                        if !(1..=2048).contains(&id)
                            || !matches!(
                                pass,
                                Pass::SkinTranslucent | Pass::CapeSolid | Pass::WingCutout
                            )
                        {
                            return Err("invalid model submission identity/role");
                        }
                        let descriptor = (actor, pass, translation.map(f32::to_bits));
                        if let Some(previous) = models.get(&id) {
                            if *previous != descriptor || last_model != Some(id) {
                                return Err(
                                    "model submission changes descriptor or resumes after another submit",
                                );
                            }
                        } else {
                            if id <= highest_model {
                                return Err("invalid model submission order");
                            }
                            highest_model = id;
                            models.insert(id, descriptor);
                        }
                        Some(ModelSubmission {
                            id,
                            translation,
                            glint: None,
                        })
                    }
                    _ => return Err("invalid model submission presence"),
                }
            } else {
                None
            };
            if version >= 10 {
                let glint = match input.byte()? {
                    0 => None,
                    1 => Some(GlintSelection::Inactive),
                    2 => {
                        let values = input.floats::<40>()?;
                        if values[33] < values[32]
                            || values[35] < values[34]
                            || !(0. ..=1.).contains(&values[36])
                        {
                            return Err("invalid selected glint fog/strength");
                        }
                        Some(GlintSelection::Active(GlintUniforms {
                            words: values.map(f32::to_bits),
                        }))
                    }
                    _ => return Err("invalid model glint selection"),
                };
                if glint.is_some() && (pass != Pass::WingCutout || model_submission.is_none()) {
                    return Err("model glint has no matching wing submission");
                }
                if matches!(glint, Some(GlintSelection::Active(_)))
                    && (equipment_projection.is_none() || space == PositionSpace::Unknown)
                {
                    return Err("active model glint has no declared layered camera");
                }
                if let Some(model) = &mut model_submission {
                    if model_glints
                        .get(&model.id)
                        .is_some_and(|previous| *previous != glint)
                    {
                        return Err("glint selection/uniforms change within a model");
                    }
                    model_glints.insert(model.id, glint);
                    model.glint = glint;
                }
            }
            last_model = model_submission.map(|model| model.id);
            total_vertices = total_vertices
                .checked_add(vertex_count)
                .ok_or("geometry vertex overflow")?;
            if !vertex_count.is_multiple_of(4)
                || total_vertices > MAX_VERTICES
                || index_count != vertex_count / 4 * 6
            {
                return Err("invalid geometry topology/budget");
            }
            let start = input.at;
            let vertex_bytes = vertex_count
                .checked_mul(40)
                .ok_or("geometry attribute overflow")?;
            let attributes = input.take(vertex_bytes)?;
            for vertex in attributes.chunks_exact(40) {
                for component in vertex[..32].chunks_exact(4) {
                    if !f32::from_le_bytes(component.try_into().unwrap()).is_finite() {
                        return Err("nonfinite geometry attribute");
                    }
                }
            }
            let attributes = start..input.at;
            let start = input.at;
            for index in 0..index_count {
                let expected = index / 6 * 4 + [0, 1, 2, 2, 3, 0][index % 6];
                if input.u32()? as usize != expected {
                    return Err("noncanonical geometry quad indices");
                }
            }
            batches.push(Batch {
                actor,
                pass,
                translucent: flags & 1 != 0,
                emissive: flags & 2 != 0,
                scene_light_known: flags & 4 != 0,
                skin_visibility_known: flags & 8 != 0,
                layer_visibility_known: flags & 16 != 0,
                texture,
                tint,
                skin_overlay,
                logical_texture,
                render_reference,
                model_submission,
                vertex_count,
                index_count,
                attributes,
                indices: start..input.at,
            });
        }
        let mut textures = Vec::with_capacity(texture_count);
        let mut pixels = 0usize;
        for _ in 0..texture_count {
            let width = input.u32()? as usize;
            let height = input.u32()? as usize;
            if !(1..=4096).contains(&width) || !(1..=4096).contains(&height) {
                return Err("invalid geometry texture size");
            }
            let count = width
                .checked_mul(height)
                .ok_or("geometry texture overflow")?;
            pixels = pixels.checked_add(count).ok_or("geometry pixel overflow")?;
            if pixels > MAX_PIXELS {
                return Err("geometry pixel budget exceeded");
            }
            let start = input.at;
            input.take(
                count
                    .checked_mul(4)
                    .ok_or("geometry texture byte overflow")?,
            )?;
            textures.push(Texture {
                width,
                height,
                argb: start..input.at,
            });
        }
        let has_simple_phases = if version >= 10 {
            match input.byte()? {
                0 => false,
                1 => true,
                _ => return Err("invalid simple phase trailer presence"),
            }
        } else {
            version >= 9
        };
        let simple_phases = if has_simple_phases {
            let epoch = input.u64()?;
            let count = input.byte()? as usize;
            if epoch == 0 || epoch > i64::MAX as u64 || count > 3 {
                return Err("invalid simple phase epoch/count");
            }
            let key = |pass: Pass| match pass {
                Pass::CapeSolid | Pass::WingCutout => Some((0, 0)),
                Pass::CosmeticBase | Pass::BentSkin => Some((0, 8)),
                Pass::CosmeticGlow => Some((1, 8)),
                _ => None,
            };
            if batches.iter().any(|batch| {
                batch.pass == Pass::General
                    || batch.render_reference.is_none()
                    || matches!(batch.pass, Pass::CapeSolid | Pass::WingCutout)
                        && batch.model_submission.is_none()
            }) {
                return Err("simple phase metadata requires supported owned submissions");
            }
            let mut phases = Vec::with_capacity(count);
            let mut previous = None;
            let mut seen = vec![false; batches.len()];
            let mut total = 0usize;
            for _ in 0..count {
                let order = input.byte()?;
                let stage = input.byte()?;
                let phase_key = (order, stage);
                if ![(0, 0), (0, 8), (1, 8)].contains(&phase_key)
                    || previous.is_some_and(|old| old >= phase_key)
                {
                    return Err("invalid simple phase order/role");
                }
                previous = Some(phase_key);
                let length = input.u32()? as usize;
                total = total
                    .checked_add(length)
                    .ok_or("simple phase count overflow")?;
                if length == 0 || total > batches.len() {
                    return Err("simple phase submit budget exceeded");
                }
                let mut indices = Vec::with_capacity(length);
                let mut references = HashSet::new();
                let mut last = None;
                for _ in 0..length {
                    let index = input.u32()? as usize;
                    let batch = batches
                        .get(index)
                        .ok_or("simple phase references a missing batch")?;
                    if key(batch.pass) != Some(phase_key)
                        || std::mem::replace(&mut seen[index], true)
                    {
                        return Err("simple phase repeats a batch or changes its role");
                    }
                    let reference = batch
                        .render_reference
                        .ok_or("simple phase has no render reference")?
                        .id;
                    if let Some((old, previous_index)) = last {
                        if old == reference {
                            if previous_index >= index {
                                return Err("simple batch changes source submit order");
                            }
                        } else if !references.insert(reference) {
                            return Err("simple phase restarts a render bucket");
                        }
                    } else {
                        references.insert(reference);
                    }
                    last = Some((reference, index));
                    indices.push(index);
                }
                phases.push(SimplePhase {
                    order,
                    stage,
                    batches: indices,
                });
            }
            if batches
                .iter()
                .enumerate()
                .any(|(index, batch)| key(batch.pass).is_some() != seen[index])
            {
                return Err("simple phases do not cover their complete supported scope");
            }
            Some(SimplePhases { epoch, phases })
        } else {
            None
        };
        if input.at != bytes.len() {
            return Err("trailing geometry packet data");
        }
        Ok(Self {
            session,
            sequence,
            clock_ms,
            cosmetic_time_ms,
            replay_time,
            width,
            height,
            space,
            dimension,
            camera,
            view_projection,
            equipment_projection,
            material,
            actors,
            batches,
            textures,
            simple_phases,
            bytes,
        })
    }
    /// 40-byte records: eight LE floats followed by ARGB u32 and packed-light u32.
    pub fn attributes(&self, batch: usize) -> Option<&[u8]> {
        self.bytes.get(self.batches.get(batch)?.attributes.clone())
    }
    pub fn indices(&self, batch: usize) -> Option<&[u8]> {
        self.bytes.get(self.batches.get(batch)?.indices.clone())
    }
    /// Little-endian ARGB u32 pixels, not RGBA byte order.
    pub fn texture_argb(&self, texture: usize) -> Option<&[u8]> {
        self.bytes.get(self.textures.get(texture)?.argb.clone())
    }
}
