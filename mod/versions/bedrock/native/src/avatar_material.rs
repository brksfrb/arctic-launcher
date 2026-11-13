//! Explicit-input Java entity material and D3D12 pipeline source port.
//! No live lightmap/fog/overlay equivalence is inferred from Bedrock constants.
use crate::{
    avatar_draw_order::{DrawRange, Plan},
    avatar_geometry::{Batch, Frame, Pass},
    avatar_gpu_upload::Uploaded,
};
use hudhook::windows::{
    Win32::Graphics::{Direct3D::Fxc::*, Direct3D::*, Direct3D12::*, Dxgi::Common::*},
    core::{PCSTR, s},
};
use std::{mem::ManuallyDrop, slice};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    CosmeticBase,
    CosmeticGlow,
    CapeSolid,
    WingCutout,
    SkinTranslucent,
}
impl TryFrom<Pass> for Kind {
    type Error = &'static str;
    fn try_from(pass: Pass) -> Result<Self, Self::Error> {
        match pass {
            Pass::CosmeticBase | Pass::BentSkin => Ok(Self::CosmeticBase),
            Pass::CosmeticGlow => Ok(Self::CosmeticGlow),
            Pass::CapeSolid => Ok(Self::CapeSolid),
            Pass::WingCutout => Ok(Self::WingCutout),
            Pass::SkinTranslucent => Ok(Self::SkinTranslucent),
            Pass::General => Err("general skin draws need their exact Java material variant"),
        }
    }
}
impl Kind {
    fn index(self) -> usize {
        match self {
            Self::CosmeticBase | Self::SkinTranslucent => 0,
            Self::CosmeticGlow => 1,
            Self::CapeSolid => 2,
            Self::WingCutout => 3,
        }
    }
    fn per_face(self) -> bool {
        self != Self::CapeSolid
    }
    fn blended(self) -> bool {
        matches!(
            self,
            Self::CosmeticBase | Self::CosmeticGlow | Self::SkinTranslucent
        )
    }
    fn cutout(self) -> f32 {
        if self == Self::CapeSolid { 0. } else { 0.1 }
    }
}

/// No defaults: these must be explicit, owned inputs for the selected draw.
pub struct Inputs {
    pub view_projection: [f32; 16],
    pub fog_position: [f32; 12],
    pub color_modulator: [f32; 4],
    pub light_directions: [[f32; 3]; 2],
    pub fog_color: [f32; 4],
    pub environmental_fog: [f32; 2],
    pub distance_fog: [f32; 2],
    /// Selected NO_OVERLAY texel for Arctic's constant overlay UV. The wing
    /// material omits overlay; other roles require its actual normalized texel.
    pub overlay_color: [f32; 4],
}
pub struct DrawConstants {
    kind: Kind,
    bytes: [u8; 256],
}
pub struct LightmapConstants([u8; 4096]);
impl Inputs {
    /// Use the exact material cohort copied with this geometry frame. Wing
    /// projection remains its independently captured equipment-layer matrix.
    pub fn for_frame(frame: &Frame, kind: Kind, reverse_z: bool) -> Result<Self, &'static str> {
        if kind == Kind::SkinTranslucent {
            return Err("skin material needs its batch-specific overlay texel");
        }
        Self::cohort(frame, kind, reverse_z)
    }
    pub fn for_batch(frame: &Frame, batch: &Batch, reverse_z: bool) -> Result<Self, &'static str> {
        let kind = Kind::try_from(batch.pass)?;
        let mut input = Self::cohort(frame, kind, reverse_z)?;
        if kind == Kind::SkinTranslucent {
            input.overlay_color = batch
                .skin_overlay
                .ok_or("skin material has no copied overlay texel")?;
        } else if batch.skin_overlay.is_some() {
            return Err("skin overlay is attached to another material role");
        }
        Ok(input)
    }
    fn cohort(frame: &Frame, kind: Kind, reverse_z: bool) -> Result<Self, &'static str> {
        let material = frame
            .material
            .as_ref()
            .ok_or("geometry has no copied entity material cohort")?;
        Ok(Self {
            view_projection: projection(frame, kind, reverse_z)?,
            fog_position: material.fog_position,
            color_modulator: material.color_modulator,
            light_directions: material.light_directions,
            fog_color: material.fog_color,
            environmental_fog: [material.fog_ranges[0], material.fog_ranges[1]],
            distance_fog: [material.fog_ranges[2], material.fog_ranges[3]],
            overlay_color: material.overlay_color,
        })
    }
    /// One 256-byte draw cbuffer, encoded without Rust/C++ layout assumptions.
    pub fn encode(&self, kind: Kind, tint: [f32; 4]) -> Result<DrawConstants, &'static str> {
        if !self
            .view_projection
            .iter()
            .chain(&self.fog_position)
            .chain(&self.color_modulator)
            .chain(self.light_directions.iter().flatten())
            .chain(&self.fog_color)
            .chain(&self.environmental_fog)
            .chain(&self.distance_fog)
            .chain(&self.overlay_color)
            .all(|value| value.is_finite())
            || !tint
                .iter()
                .all(|value| value.is_finite() && (0. ..=1.).contains(value))
            || self.environmental_fog[1] < self.environmental_fog[0]
            || self.distance_fog[1] < self.distance_fog[0]
            || !self
                .overlay_color
                .iter()
                .all(|value| (0. ..=1.).contains(value))
        {
            return Err("missing/invalid explicit entity material inputs");
        }
        let mut words = [0u32; 64];
        let mut at = 0;
        let light0 = [
            self.light_directions[0][0],
            self.light_directions[0][1],
            self.light_directions[0][2],
            0.,
        ];
        let light1 = [
            self.light_directions[1][0],
            self.light_directions[1][1],
            self.light_directions[1][2],
            0.,
        ];
        let fog = [
            self.environmental_fog[0],
            self.environmental_fog[1],
            self.distance_fog[0],
            self.distance_fog[1],
        ];
        for values in [
            &self.view_projection[..],
            &self.fog_position[..],
            &self.color_modulator[..],
            &tint[..],
            &light0[..],
            &light1[..],
            &self.fog_color[..],
            &fog[..],
            &self.overlay_color[..],
        ] {
            for value in values {
                words[at] = value.to_bits();
                at += 1;
            }
        }
        words[56] = u32::from(kind.per_face());
        words[57] = 1;
        words[58] = u32::from(kind == Kind::CosmeticGlow);
        words[59] = u32::from(kind != Kind::WingCutout);
        words[60] = kind.cutout().to_bits();
        let mut bytes = [0u8; 256];
        for (output, word) in bytes.chunks_exact_mut(4).zip(words) {
            output.copy_from_slice(&word.to_le_bytes());
        }
        Ok(DrawConstants { kind, bytes })
    }
}
/// Exact normalized 16x16 lightmap snapshot. This is not a full-bright fallback
/// or a mapping from Bedrock TILE_LIGHT_COLOR into Java's lightmap semantics.
pub fn encode_lightmap(pixels: &[[f32; 4]; 256]) -> Result<LightmapConstants, &'static str> {
    if !pixels
        .iter()
        .flatten()
        .all(|value| value.is_finite() && (0. ..=1.).contains(value))
    {
        return Err("invalid owned Java lightmap");
    }
    let mut result = [0u8; 4096];
    for (output, value) in result.chunks_exact_mut(4).zip(pixels.iter().flatten()) {
        output.copy_from_slice(&value.to_le_bytes());
    }
    Ok(LightmapConstants(result))
}

/// Packet matrices use forward DirectX depth. Convert their complete Z row for
/// a reverse-Z DSV; never change only the projection's diagonal element.
/// Native floating-point equivalence of this algebraic round trip is unverified.
pub fn projection(frame: &Frame, kind: Kind, reverse_z: bool) -> Result<[f32; 16], &'static str> {
    let mut matrix = if kind == Kind::WingCutout {
        frame
            .equipment_projection
            .ok_or("wing material needs its layered camera")?
    } else {
        frame.view_projection
    };
    if !matrix.iter().all(|value| value.is_finite()) {
        return Err("nonfinite entity projection");
    }
    if reverse_z {
        for column in 0..4 {
            let at = column * 4;
            matrix[at + 2] = matrix[at + 3] - matrix[at + 2];
        }
    }
    if !matrix.iter().all(|value| value.is_finite()) {
        return Err("entity reverse-Z conversion overflow");
    }
    Ok(matrix)
}

/// CPU-written upload-heap constants; the draw owner retains this alongside
/// Uploaded and Pipelines through its own submission fence. Mapping these new
/// resources submits no GPU work and needs no upload-ticket fence.
struct DrawIdentity {
    kind: Kind,
    projection: [u32; 16],
    tint: [u32; 4],
    bytes: [u8; 256],
}
pub struct Constants {
    resource: ID3D12Resource,
    base: u64,
    draws: Vec<DrawIdentity>,
    lightmap: [u8; 4096],
}
struct ConstantMapping<'a> {
    resource: &'a ID3D12Resource,
    size: usize,
}
impl Drop for ConstantMapping<'_> {
    fn drop(&mut self) {
        unsafe {
            self.resource.Unmap(
                0,
                Some(&D3D12_RANGE {
                    Begin: 0,
                    End: self.size,
                }),
            );
        }
    }
}
impl Constants {
    /// Assemble packet-owned inputs before allocating a GPU resource. The
    /// mapping retains one slot per source batch; empty submits have no CBV.
    /// No queue execution or recording-list mutation occurs here.
    pub fn from_frame(
        device: &ID3D12Device,
        node_mask: u32,
        frame: &Frame,
        reverse_z: bool,
    ) -> Result<(Self, Vec<usize>), String> {
        if frame.batches.is_empty() || frame.batches.len() > 2048 {
            return Err("invalid entity frame batch count".into());
        }
        let material = frame
            .material
            .as_ref()
            .ok_or("geometry has no copied entity material cohort")?;
        let lightmap = encode_lightmap(&material.lightmap).map_err(str::to_owned)?;
        let mut draws = Vec::new();
        let mut indices = vec![usize::MAX; frame.batches.len()];
        for (index, batch) in frame.batches.iter().enumerate() {
            if batch.index_count == 0 {
                continue;
            }
            let kind = Kind::try_from(batch.pass).map_err(str::to_owned)?;
            if batch.pass.skin_geometry() && !batch.skin_visibility_known {
                return Err("skin geometry has no copied visibility source".into());
            }
            if batch.pass.layer_geometry() && !batch.layer_visibility_known {
                return Err("layer geometry has no copied eligibility source".into());
            }
            if kind != Kind::CosmeticGlow && !batch.scene_light_known {
                return Err("entity batch has no corresponding packed scene-light input".into());
            }
            let draw = Inputs::for_batch(frame, batch, reverse_z)?
                .encode(kind, batch.tint)
                .map_err(str::to_owned)?;
            indices[index] = draws.len();
            draws.push(draw);
        }
        if draws.is_empty() {
            return Err("geometry has no nonempty entity material draws".into());
        }
        Ok((Self::new(device, node_mask, &draws, &lightmap)?, indices))
    }
    pub fn new(
        device: &ID3D12Device,
        node_mask: u32,
        draws: &[DrawConstants],
        lightmap: &LightmapConstants,
    ) -> Result<Self, String> {
        let _private = crate::depth_tracker::private_commands();
        if draws.is_empty() || draws.len() > 2048 {
            return Err("invalid entity constant-buffer count".into());
        }
        let nodes = unsafe { device.GetNodeCount() };
        let node = if node_mask == 0 { 1 } else { node_mask };
        if !(1..=32).contains(&nodes) || node.count_ones() != 1 || (node as u64) >= (1u64 << nodes)
        {
            return Err("entity constants require a valid device node".into());
        }
        let size = draws.len() * 256 + 4096;
        let mut resource = None;
        unsafe {
            device.CreateCommittedResource(
                &D3D12_HEAP_PROPERTIES {
                    Type: D3D12_HEAP_TYPE_UPLOAD,
                    CreationNodeMask: node,
                    VisibleNodeMask: node,
                    ..Default::default()
                },
                D3D12_HEAP_FLAG_NONE,
                &D3D12_RESOURCE_DESC {
                    Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
                    Width: size as u64,
                    Height: 1,
                    DepthOrArraySize: 1,
                    MipLevels: 1,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
                    ..Default::default()
                },
                D3D12_RESOURCE_STATE_GENERIC_READ,
                None,
                &mut resource,
            )
        }
        .map_err(|e| e.to_string())?;
        let resource: ID3D12Resource =
            resource.ok_or("entity constants resource was not created")?;
        let base = unsafe { resource.GetGPUVirtualAddress() };
        if base == 0 || !base.is_multiple_of(256) || base.checked_add(size as u64).is_none() {
            return Err("invalid entity constant-buffer GPU address".into());
        }
        let mut pointer = std::ptr::null_mut();
        unsafe {
            resource.Map(
                0,
                Some(&D3D12_RANGE { Begin: 0, End: 0 }),
                Some(&mut pointer),
            )
        }
        .map_err(|e| e.to_string())?;
        let mapping = ConstantMapping {
            resource: &resource,
            size,
        };
        if pointer.is_null() {
            return Err("null entity constant-buffer mapping".into());
        }
        let output = unsafe { slice::from_raw_parts_mut(pointer as *mut u8, size) };
        for (destination, draw) in output[..draws.len() * 256].chunks_exact_mut(256).zip(draws) {
            destination.copy_from_slice(&draw.bytes);
        }
        output[draws.len() * 256..].copy_from_slice(&lightmap.0);
        drop(mapping);
        let identities = draws
            .iter()
            .map(|draw| {
                let mut projection = [0u32; 16];
                let mut tint = [0u32; 4];
                for (value, bytes) in projection.iter_mut().zip(draw.bytes[..64].chunks_exact(4)) {
                    *value = u32::from_le_bytes(bytes.try_into().unwrap());
                }
                for (value, bytes) in tint.iter_mut().zip(draw.bytes[128..144].chunks_exact(4)) {
                    *value = u32::from_le_bytes(bytes.try_into().unwrap());
                }
                DrawIdentity {
                    kind: draw.kind,
                    projection,
                    tint,
                    bytes: draw.bytes,
                }
            })
            .collect();
        Ok(Self {
            resource,
            base,
            draws: identities,
            lightmap: lightmap.0,
        })
    }
    fn addresses(
        &self,
        index: usize,
        kind: Kind,
        frame: &Frame,
        batch: &Batch,
        reverse_z: bool,
    ) -> Result<(u64, u64), &'static str> {
        let draw = self
            .draws
            .get(index)
            .ok_or("entity constants index is missing")?;
        if draw.kind != kind
            || draw.tint != batch.tint.map(f32::to_bits)
            || draw.projection != projection(frame, kind, reverse_z)?.map(f32::to_bits)
        {
            return Err("entity constants disagree with packet material/camera/tint");
        }
        if kind == Kind::SkinTranslucent {
            let overlay = batch
                .skin_overlay
                .ok_or("skin material has no copied overlay texel")?;
            if overlay
                .iter()
                .zip(draw.bytes[208..224].chunks_exact(4))
                .any(|(value, bytes)| {
                    value.to_bits() != u32::from_le_bytes(bytes.try_into().unwrap())
                })
            {
                return Err("skin constants disagree with the copied body overlay");
            }
        }
        if let Some(material) = &frame.material {
            let expected = Inputs::for_batch(frame, batch, reverse_z)?.encode(kind, batch.tint)?;
            if draw.bytes != expected.bytes
                || self.lightmap != encode_lightmap(&material.lightmap)?.0
            {
                return Err("entity constants disagree with copied shader/lightmap inputs");
            }
        }
        Ok((
            self.base + index as u64 * 256,
            self.base + self.draws.len() as u64 * 256,
        ))
    }
    pub fn resource(&self) -> &ID3D12Resource {
        &self.resource
    }
}

#[derive(Clone, Copy)]
pub struct Target {
    pub color: DXGI_FORMAT,
    pub depth: DXGI_FORMAT,
    pub samples: DXGI_SAMPLE_DESC,
    pub reverse_z: bool,
    /// Must match the owning target's post-viewport winding convention.
    pub front_counter_clockwise: bool,
    pub node_mask: u32,
}
pub(crate) fn validate_target(target: &Target) -> Result<(), String> {
    if !matches!(
        target.color,
        DXGI_FORMAT_R8G8B8A8_UNORM | DXGI_FORMAT_B8G8R8A8_UNORM
    ) || !matches!(
        target.depth,
        DXGI_FORMAT_D32_FLOAT
            | DXGI_FORMAT_D24_UNORM_S8_UINT
            | DXGI_FORMAT_D16_UNORM
            | DXGI_FORMAT_D32_FLOAT_S8X24_UINT
    ) || !matches!(target.samples.Count, 1 | 2 | 4 | 8 | 16)
    {
        return Err("unsupported entity material color/depth/sample format".into());
    }
    Ok(())
}
pub struct Pipelines {
    root: ID3D12RootSignature,
    states: [ID3D12PipelineState; 4],
    reverse_z: bool,
    target: Target,
}
struct PreparedBatch {
    kind: Kind,
    texture: D3D12_GPU_DESCRIPTOR_HANDLE,
    draw_constants: u64,
    lightmap_constants: u64,
    vertex: D3D12_VERTEX_BUFFER_VIEW,
    index: D3D12_INDEX_BUFFER_VIEW,
}
/// Immutable, borrowed commands after every fallible material/range check.
/// The borrows prevent their upload/constants/pipelines from being moved or
/// mutated before recording; the native owner separately pins their GPU use.
pub(crate) struct PreparedOrdered<'a> {
    pipelines: &'a Pipelines,
    uploaded: &'a Uploaded,
    _constants: &'a Constants,
    batches: Vec<Option<PreparedBatch>>,
    ranges: Vec<DrawRange>,
}
impl PreparedOrdered<'_> {
    pub(crate) fn ranges(&self) -> usize {
        self.ranges.len()
    }
    /// # Safety
    /// Same live recording list/target/cohort and lifetime contract as
    /// Pipelines::record_ordered. No reset/close/execute is performed here.
    pub(crate) unsafe fn record(&self, list: &ID3D12GraphicsCommandList) {
        if self.ranges.is_empty() {
            return;
        }
        unsafe {
            list.SetDescriptorHeaps(&[Some(self.uploaded.descriptor_heap().clone())]);
        }
        for range in &self.ranges {
            let batch = self.batches[range.batch]
                .as_ref()
                .expect("preflighted entity range");
            unsafe {
                self.pipelines
                    .emit(list, batch, range.first_index, range.index_count);
            }
        }
    }
}
struct PipelineDescription(D3D12_GRAPHICS_PIPELINE_STATE_DESC);
impl Drop for PipelineDescription {
    fn drop(&mut self) {
        unsafe {
            ManuallyDrop::drop(&mut self.0.pRootSignature);
        }
    }
}

fn blob_error(blob: Option<ID3DBlob>, fallback: String) -> String {
    match blob {
        Some(blob) => unsafe {
            String::from_utf8_lossy(slice::from_raw_parts(
                blob.GetBufferPointer() as *const u8,
                blob.GetBufferSize().min(65536),
            ))
            .trim_end_matches('\0')
            .to_owned()
        },
        None => fallback,
    }
}
fn compile(vertex: bool) -> Result<ID3DBlob, String> {
    const SOURCE: &str = include_str!("shaders/avatar_entity.hlsl");
    let (entry, profile): (PCSTR, PCSTR) = if vertex {
        (s!("vertex_main"), s!("vs_5_0"))
    } else {
        (s!("pixel_main"), s!("ps_5_0"))
    };
    let mut output = None;
    let mut error = None;
    unsafe {
        D3DCompile(
            SOURCE.as_ptr() as _,
            SOURCE.len(),
            None,
            None,
            None::<&ID3DInclude>,
            entry,
            profile,
            D3DCOMPILE_ENABLE_STRICTNESS | D3DCOMPILE_IEEE_STRICTNESS,
            0,
            &mut output,
            Some(&mut error),
        )
    }
    .map_err(|value| blob_error(error, value.to_string()))?;
    output.ok_or_else(|| "entity shader compiler returned no bytecode".into())
}
impl Pipelines {
    pub fn target(&self) -> Target {
        self.target
    }
    pub(crate) fn device(&self) -> Result<ID3D12Device, String> {
        let mut device = None;
        unsafe { self.root.GetDevice(&mut device) }.map_err(|error| error.to_string())?;
        device.ok_or_else(|| "entity pipeline device is missing".into())
    }
    /// Source construction only until explicitly called by a runtime consumer.
    pub fn new(device: &ID3D12Device, target: &Target) -> Result<Self, String> {
        validate_target(target)?;
        unsafe {
            let mut parameters = [D3D12_ROOT_PARAMETER::default(); 3];
            for (index, parameter) in parameters[..2].iter_mut().enumerate() {
                parameter.ParameterType = D3D12_ROOT_PARAMETER_TYPE_CBV;
                parameter.ShaderVisibility = if index == 0 {
                    D3D12_SHADER_VISIBILITY_ALL
                } else {
                    D3D12_SHADER_VISIBILITY_VERTEX
                };
                parameter.Anonymous.Descriptor = D3D12_ROOT_DESCRIPTOR {
                    ShaderRegister: index as u32,
                    RegisterSpace: 0,
                };
            }
            let range = D3D12_DESCRIPTOR_RANGE {
                RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_SRV,
                NumDescriptors: 1,
                BaseShaderRegister: 0,
                RegisterSpace: 0,
                OffsetInDescriptorsFromTableStart: 0,
            };
            parameters[2] = D3D12_ROOT_PARAMETER {
                ParameterType: D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE,
                ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL,
                Anonymous: D3D12_ROOT_PARAMETER_0 {
                    DescriptorTable: D3D12_ROOT_DESCRIPTOR_TABLE {
                        NumDescriptorRanges: 1,
                        pDescriptorRanges: &range,
                    },
                },
            };
            let sampler = D3D12_STATIC_SAMPLER_DESC {
                Filter: D3D12_FILTER_MIN_MAG_MIP_POINT,
                AddressU: D3D12_TEXTURE_ADDRESS_MODE_WRAP,
                AddressV: D3D12_TEXTURE_ADDRESS_MODE_WRAP,
                AddressW: D3D12_TEXTURE_ADDRESS_MODE_WRAP,
                MaxAnisotropy: 1,
                ComparisonFunc: D3D12_COMPARISON_FUNC_ALWAYS,
                ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL,
                MaxLOD: 0.,
                ..Default::default()
            };
            let desc = D3D12_ROOT_SIGNATURE_DESC {
                NumParameters: 3,
                pParameters: parameters.as_ptr(),
                NumStaticSamplers: 1,
                pStaticSamplers: &sampler,
                Flags: D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT
                    | D3D12_ROOT_SIGNATURE_FLAG_DENY_HULL_SHADER_ROOT_ACCESS
                    | D3D12_ROOT_SIGNATURE_FLAG_DENY_DOMAIN_SHADER_ROOT_ACCESS
                    | D3D12_ROOT_SIGNATURE_FLAG_DENY_GEOMETRY_SHADER_ROOT_ACCESS,
            };
            let mut serialized: Option<ID3DBlob> = None;
            let mut error = None;
            D3D12SerializeRootSignature(
                &desc,
                D3D_ROOT_SIGNATURE_VERSION_1_0,
                &mut serialized,
                Some(&mut error),
            )
            .map_err(|value| blob_error(error, value.to_string()))?;
            let serialized =
                serialized.ok_or("entity root signature serialization returned no data")?;
            let root: ID3D12RootSignature = device
                .CreateRootSignature(
                    target.node_mask,
                    slice::from_raw_parts(
                        serialized.GetBufferPointer() as *const u8,
                        serialized.GetBufferSize(),
                    ),
                )
                .map_err(|e| e.to_string())?;
            let vertex = compile(true)?;
            let pixel = compile(false)?;
            let make = |kind| create_pipeline(device, target, &root, &vertex, &pixel, kind);
            let states = [
                make(Kind::CosmeticBase)?,
                make(Kind::CosmeticGlow)?,
                make(Kind::CapeSolid)?,
                make(Kind::WingCutout)?,
            ];
            Ok(Self {
                root,
                states,
                reverse_z: target.reverse_z,
                target: *target,
            })
        }
    }

    /// # Safety
    /// Caller owns a recording direct list on the matching device/node and has
    /// bound the correct native world RTV/DSV, states, viewport and scissor.
    /// Constants contain Inputs/encode_lightmap data for this draw cohort. The
    /// caller must retain Constants, this pipeline set and Uploaded through its draw
    /// fence. Packet identity alone does not establish those ownership joins.
    pub unsafe fn record_batch(
        &self,
        list: &ID3D12GraphicsCommandList,
        uploaded: &Uploaded,
        batch: usize,
        constants: &Constants,
        uniform_index: usize,
    ) -> Result<(), String> {
        let metadata = uploaded
            .frame()
            .batches
            .get(batch)
            .ok_or("entity batch is missing")?;
        if metadata.index_count == 0 {
            return Ok(());
        }
        let prepared = self.prepare_batch(uploaded, batch, constants, uniform_index)?;
        let count = u32::try_from(metadata.index_count)
            .map_err(|_| "entity batch index count exceeds GPU range")?;
        unsafe {
            list.SetDescriptorHeaps(&[Some(uploaded.descriptor_heap().clone())]);
            self.emit(list, &prepared, 0, count);
        }
        Ok(())
    }

    /// Record Java's explicit feature-group/quad order. All material, camera,
    /// tint, descriptor and range checks finish before the first GPU command.
    /// # Safety
    /// Same target/list/cohort and draw-fence retention requirements as
    /// record_batch. The plan must borrow this exact uploaded frame. Group IDs,
    /// consolidation flags and sorting space must come from the owning adapter.
    pub unsafe fn record_ordered(
        &self,
        list: &ID3D12GraphicsCommandList,
        uploaded: &Uploaded,
        plan: &Plan<'_>,
        constants: &Constants,
        uniform_indices: &[usize],
    ) -> Result<(), String> {
        let prepared = self.prepare_ordered(uploaded, plan, constants, uniform_indices)?;
        unsafe {
            prepared.record(list);
        }
        Ok(())
    }
    pub(crate) fn prepare_ordered<'a>(
        &'a self,
        uploaded: &'a Uploaded,
        plan: &Plan<'_>,
        constants: &'a Constants,
        uniform_indices: &[usize],
    ) -> Result<PreparedOrdered<'a>, String> {
        if !std::ptr::eq(plan.frame(), uploaded.frame())
            || uniform_indices.len() != uploaded.frame().batches.len()
        {
            return Err("entity order/constant mapping belongs to another uploaded frame".into());
        }
        let mut prepared: Vec<Option<PreparedBatch>> = Vec::with_capacity(uniform_indices.len());
        for (batch, &uniform) in uniform_indices.iter().enumerate() {
            if uploaded.frame().batches[batch].index_count == 0 {
                prepared.push(None);
            } else {
                prepared.push(Some(
                    self.prepare_batch(uploaded, batch, constants, uniform)?,
                ));
            }
        }
        for range in plan.ranges() {
            let metadata = uploaded
                .frame()
                .batches
                .get(range.batch)
                .ok_or("ordered entity batch is missing")?;
            if range.index_count == 0
                || !range.first_index.is_multiple_of(6)
                || !range.index_count.is_multiple_of(6)
                || range
                    .first_index
                    .checked_add(range.index_count)
                    .is_none_or(|end| end as usize > metadata.index_count)
                || prepared
                    .get(range.batch)
                    .is_none_or(|batch| batch.is_none())
            {
                return Err("ordered entity index range exceeds its owned batch".into());
            }
        }
        Ok(PreparedOrdered {
            pipelines: self,
            uploaded,
            _constants: constants,
            batches: prepared,
            ranges: plan.ranges().to_vec(),
        })
    }
    pub(crate) fn prepare_groups<'a>(
        &'a self,
        uploaded: &'a Uploaded,
        plan: &Plan<'_>,
        constants: &'a Constants,
        uniform_indices: &[usize],
        groups: &[usize],
    ) -> Result<PreparedOrdered<'a>, String> {
        if groups.len() > 2048 || groups.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err("selected entity groups change source order or repeat".into());
        }
        let mut prepared = self.prepare_ordered(uploaded, plan, constants, uniform_indices)?;
        let mut ranges = Vec::new();
        for &index in groups {
            let range = plan
                .group_range(index)
                .ok_or("selected entity group is missing")?;
            ranges.extend_from_slice(
                plan.ranges()
                    .get(range)
                    .ok_or("entity group range exceeds its plan")?,
            );
        }
        prepared.ranges = ranges;
        Ok(prepared)
    }
    fn prepare_batch(
        &self,
        uploaded: &Uploaded,
        batch: usize,
        constants: &Constants,
        uniform_index: usize,
    ) -> Result<PreparedBatch, String> {
        let metadata = uploaded
            .frame()
            .batches
            .get(batch)
            .ok_or("entity batch is missing")?;
        let buffers = uploaded
            .draw_buffers(batch)
            .ok_or("entity GPU buffers are missing")?;
        let kind = Kind::try_from(metadata.pass).map_err(str::to_owned)?;
        if metadata.pass.skin_geometry() && !metadata.skin_visibility_known {
            return Err("skin geometry has no copied visibility source".into());
        }
        if metadata.pass.layer_geometry() && !metadata.layer_visibility_known {
            return Err("layer geometry has no copied eligibility source".into());
        }
        // A correct lightmap cannot repair invented full-bright vertex UV2.
        // Glow bypasses lightmap lookup; every lit role needs a scene sample.
        if kind != Kind::CosmeticGlow && !metadata.scene_light_known {
            return Err("entity batch has no corresponding packed scene-light input".into());
        }
        let texture = uploaded
            .texture_handle(metadata.texture)
            .ok_or("entity texture descriptor is missing")?;
        let (draw_constants, lightmap_constants) = constants
            .addresses(
                uniform_index,
                kind,
                uploaded.frame(),
                metadata,
                self.reverse_z,
            )
            .map_err(str::to_owned)?;
        Ok(PreparedBatch {
            kind,
            texture,
            draw_constants,
            lightmap_constants,
            vertex: buffers.vertex,
            index: buffers.index,
        })
    }
    unsafe fn emit(
        &self,
        list: &ID3D12GraphicsCommandList,
        batch: &PreparedBatch,
        first: u32,
        count: u32,
    ) {
        unsafe {
            list.SetGraphicsRootSignature(&self.root);
            list.SetPipelineState(&self.states[batch.kind.index()]);
            list.SetGraphicsRootConstantBufferView(0, batch.draw_constants);
            list.SetGraphicsRootConstantBufferView(1, batch.lightmap_constants);
            list.SetGraphicsRootDescriptorTable(2, batch.texture);
            list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            list.IASetVertexBuffers(0, Some(&[batch.vertex]));
            list.IASetIndexBuffer(Some(&batch.index));
            list.DrawIndexedInstanced(count, 1, first, 0, 0);
        }
    }
}

unsafe fn create_pipeline(
    device: &ID3D12Device,
    target: &Target,
    root: &ID3D12RootSignature,
    vertex: &ID3DBlob,
    pixel: &ID3DBlob,
    kind: Kind,
) -> Result<ID3D12PipelineState, String> {
    let element = |name, semantic, format, offset| D3D12_INPUT_ELEMENT_DESC {
        SemanticName: name,
        SemanticIndex: semantic,
        Format: format,
        InputSlot: 0,
        AlignedByteOffset: offset,
        InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
        InstanceDataStepRate: 0,
    };
    let layout = [
        element(s!("POSITION"), 0, DXGI_FORMAT_R32G32B32_FLOAT, 0),
        element(s!("NORMAL"), 0, DXGI_FORMAT_R32G32B32_FLOAT, 12),
        element(s!("TEXCOORD"), 0, DXGI_FORMAT_R32G32_FLOAT, 24),
        element(s!("COLOR"), 0, DXGI_FORMAT_R32_UINT, 32),
        element(s!("TEXCOORD"), 1, DXGI_FORMAT_R32_UINT, 36),
    ];
    let stencil = D3D12_DEPTH_STENCILOP_DESC {
        StencilFailOp: D3D12_STENCIL_OP_KEEP,
        StencilDepthFailOp: D3D12_STENCIL_OP_KEEP,
        StencilPassOp: D3D12_STENCIL_OP_KEEP,
        StencilFunc: D3D12_COMPARISON_FUNC_ALWAYS,
    };
    let mut color_targets = [D3D12_RENDER_TARGET_BLEND_DESC::default(); 8];
    color_targets[0] = D3D12_RENDER_TARGET_BLEND_DESC {
        BlendEnable: kind.blended().into(),
        LogicOpEnable: false.into(),
        SrcBlend: D3D12_BLEND_SRC_ALPHA,
        DestBlend: D3D12_BLEND_INV_SRC_ALPHA,
        BlendOp: D3D12_BLEND_OP_ADD,
        SrcBlendAlpha: D3D12_BLEND_ONE,
        DestBlendAlpha: D3D12_BLEND_INV_SRC_ALPHA,
        BlendOpAlpha: D3D12_BLEND_OP_ADD,
        RenderTargetWriteMask: D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8,
        ..Default::default()
    };
    let mut formats = [DXGI_FORMAT_UNKNOWN; 8];
    formats[0] = target.color;
    let desc = PipelineDescription(D3D12_GRAPHICS_PIPELINE_STATE_DESC {
        pRootSignature: ManuallyDrop::new(Some(root.clone())),
        NodeMask: target.node_mask,
        VS: D3D12_SHADER_BYTECODE {
            pShaderBytecode: unsafe { vertex.GetBufferPointer() },
            BytecodeLength: unsafe { vertex.GetBufferSize() },
        },
        PS: D3D12_SHADER_BYTECODE {
            pShaderBytecode: unsafe { pixel.GetBufferPointer() },
            BytecodeLength: unsafe { pixel.GetBufferSize() },
        },
        InputLayout: D3D12_INPUT_LAYOUT_DESC {
            pInputElementDescs: layout.as_ptr(),
            NumElements: layout.len() as u32,
        },
        PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
        SampleMask: u32::MAX,
        NumRenderTargets: 1,
        RTVFormats: formats,
        DSVFormat: target.depth,
        SampleDesc: target.samples,
        BlendState: D3D12_BLEND_DESC {
            RenderTarget: color_targets,
            ..Default::default()
        },
        RasterizerState: D3D12_RASTERIZER_DESC {
            FillMode: D3D12_FILL_MODE_SOLID,
            CullMode: if kind == Kind::CapeSolid {
                D3D12_CULL_MODE_BACK
            } else {
                D3D12_CULL_MODE_NONE
            },
            FrontCounterClockwise: target.front_counter_clockwise.into(),
            DepthClipEnable: true.into(),
            MultisampleEnable: (target.samples.Count > 1).into(),
            ..Default::default()
        },
        DepthStencilState: D3D12_DEPTH_STENCIL_DESC {
            DepthEnable: true.into(),
            DepthWriteMask: if kind == Kind::CosmeticGlow {
                D3D12_DEPTH_WRITE_MASK_ZERO
            } else {
                D3D12_DEPTH_WRITE_MASK_ALL
            },
            DepthFunc: if target.reverse_z {
                D3D12_COMPARISON_FUNC_GREATER_EQUAL
            } else {
                D3D12_COMPARISON_FUNC_LESS_EQUAL
            },
            StencilEnable: false.into(),
            StencilReadMask: 255,
            StencilWriteMask: 255,
            FrontFace: stencil,
            BackFace: stencil,
        },
        ..Default::default()
    });
    unsafe { device.CreateGraphicsPipelineState(&desc.0) }.map_err(|e| e.to_string())
}
