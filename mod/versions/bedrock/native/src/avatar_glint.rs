//! Explicit-input armor glint on owned wing geometry. This module neither
//! selects enchanted equipment nor supplies native world/phase ownership.
use crate::{
    avatar_draw_order::{self, ModelViewPose, PhaseKey},
    avatar_geometry::{Frame, GlintSelection, GlintUniforms, Pass},
    avatar_gpu_upload::Uploaded,
    avatar_material::{self, Kind, Target},
};
use hudhook::windows::{
    Win32::Graphics::{Direct3D::Fxc::*, Direct3D::*, Direct3D12::*, Dxgi::Common::*},
    core::{PCSTR, s},
};
use std::{
    collections::{BTreeMap, HashSet},
    mem::ManuallyDrop,
    slice,
};

/// WingsLayer starts order 0. EquipmentLayerRenderer increments for the glint
/// submit; its GLINT blend routes it to translucentModels, not solid/custom.
pub const WING_PHASE: PhaseKey = PhaseKey { order: 1, stage: 7 };
/// Complete original phase sequence, including every other participating model.
/// Foreign tokens are opaque owning-adapter identities, never engine pointers.
pub enum WingPhaseModel {
    Wing(ModelViewPose),
    Foreign { token: u64, translation: [f32; 3] },
}
pub enum WingPhaseStep {
    WingRun(Vec<u16>),
    Foreign(u64),
}
/// Own poses/uniforms come from this exact packet; only foreign deliveries
/// still need their owning producer's opaque identity and submitted view pose.
pub enum PacketParticipant {
    Wing(u16),
    Foreign { token: u64, translation: [f32; 3] },
}
/// Validate owned selection before an upload/artwork request. Unknown is not
/// treated as an absent pass; an empty result means only known owned absence.
pub fn packet_observations(frame: &Frame) -> Result<BTreeMap<u16, GlintUniforms>, &'static str> {
    let mut selection = BTreeMap::new();
    for batch in &frame.batches {
        if batch.pass != Pass::WingCutout {
            continue;
        }
        let model = batch
            .model_submission
            .ok_or("packet wing model boundary is unknown")?;
        let glint = model
            .glint
            .ok_or("packet wing glint selection is unknown")?;
        if selection
            .get(&model.id)
            .is_some_and(|previous| *previous != glint)
        {
            return Err("packet wing glint changes within a model");
        }
        selection.insert(model.id, glint);
    }
    Ok(selection
        .into_iter()
        .filter_map(|(id, selection)| match selection {
            GlintSelection::Active(values) => Some((id, values)),
            GlintSelection::Inactive => None,
        })
        .collect())
}
pub fn packet_schedule(
    frame: &Frame,
    participants: Vec<PacketParticipant>,
) -> Result<(WingSchedule, BTreeMap<u16, GlintUniforms>), &'static str> {
    let active = packet_observations(frame)?;
    if participants.len() > 2048 {
        return Err("joint glint model phase budget exceeded");
    }
    let mut seen = HashSet::new();
    let mut models = Vec::with_capacity(participants.len());
    for participant in participants {
        models.push(match participant {
            PacketParticipant::Wing(id) => {
                let values = active
                    .get(&id)
                    .ok_or("joint glint phase selects an inactive/missing packet model")?;
                if !seen.insert(id) {
                    return Err("joint glint phase repeats a packet model");
                }
                WingPhaseModel::Wing(ModelViewPose {
                    model: id,
                    translation: values.view_translation(),
                })
            }
            PacketParticipant::Foreign { token, translation } => {
                WingPhaseModel::Foreign { token, translation }
            }
        });
    }
    if seen.len() != active.len() {
        return Err("joint glint phase omits an active packet model");
    }
    Ok((WingSchedule::new(frame, models)?, active))
}
pub struct WingSchedule {
    steps: Vec<WingPhaseStep>,
}
impl WingSchedule {
    pub fn new(frame: &Frame, models: Vec<WingPhaseModel>) -> Result<Self, &'static str> {
        if models.len() > 2048 {
            return Err("joint glint model phase budget exceeded");
        }
        let mut poses = Vec::new();
        let mut foreign = HashSet::new();
        let mut translations = Vec::with_capacity(models.len());
        for model in &models {
            match model {
                WingPhaseModel::Wing(pose) => {
                    poses.push(ModelViewPose {
                        model: pose.model,
                        translation: pose.translation,
                    });
                    translations.push(pose.translation);
                }
                WingPhaseModel::Foreign { token, translation } => {
                    if *token == 0 || !foreign.insert(*token) {
                        return Err("joint glint phase has an unknown/repeated foreign token");
                    }
                    translations.push(*translation);
                }
            }
        }
        // Validation only: never sort this selected subset. Foreign entries
        // influence unstable ties and must participate in the one full sort.
        validate_wing_models(frame, &poses)?;
        let order = avatar_draw_order::model_order(&translations)?;
        let mut steps = Vec::with_capacity(models.len());
        for index in order {
            match &models[index] {
                WingPhaseModel::Wing(pose) => {
                    if let Some(WingPhaseStep::WingRun(run)) = steps.last_mut() {
                        run.push(pose.model);
                    } else {
                        steps.push(WingPhaseStep::WingRun(vec![pose.model]));
                    }
                }
                WingPhaseModel::Foreign { token, .. } => steps.push(WingPhaseStep::Foreign(*token)),
            }
        }
        Ok(Self { steps })
    }
    pub fn steps(&self) -> &[WingPhaseStep] {
        &self.steps
    }
    pub fn has_foreign(&self) -> bool {
        self.steps
            .iter()
            .any(|step| matches!(step, WingPhaseStep::Foreign(_)))
    }
    pub fn wing_models(&self) -> Vec<u16> {
        self.steps
            .iter()
            .flat_map(|step| match step {
                WingPhaseStep::WingRun(models) => models.as_slice(),
                _ => &[],
            })
            .copied()
            .collect()
    }
}
/// Selected source wing models in original submission order, with actual
/// Java-equivalent layer view poses. No selection/pose is inferred here.
pub fn order_wing_models(frame: &Frame, poses: &[ModelViewPose]) -> Result<Vec<u16>, &'static str> {
    validate_wing_models(frame, poses)?;
    let indices = avatar_draw_order::model_order(
        &poses
            .iter()
            .map(|pose| pose.translation)
            .collect::<Vec<_>>(),
    )?;
    Ok(indices
        .into_iter()
        .map(|index| poses[index].model)
        .collect())
}
fn validate_wing_models(frame: &Frame, poses: &[ModelViewPose]) -> Result<(), &'static str> {
    if poses.len() > 2048 || poses.windows(2).any(|pair| pair[0].model >= pair[1].model) {
        return Err("glint model selection changes source order or repeats");
    }
    for pose in poses {
        let mut found = false;
        for batch in &frame.batches {
            if batch
                .model_submission
                .is_none_or(|model| model.id != pose.model)
            {
                continue;
            }
            if batch.pass != Pass::WingCutout || !batch.layer_visibility_known {
                return Err("glint selection is not a known wing model");
            }
            found = true;
        }
        if !found {
            return Err("glint selection has no source model");
        }
    }
    Ok(())
}

/// No implicit wall clock, replay time, option, texture matrix or fog fallback.
pub struct Inputs {
    pub texture_matrix: [f32; 16],
    pub fog_position: [f32; 12],
    pub color_modulator: [f32; 4],
    pub fog_ranges: [f32; 4],
    pub glint_alpha: f32,
}
impl Inputs {
    pub fn from_packet(uniforms: GlintUniforms) -> Self {
        let values = uniforms.values();
        Self {
            texture_matrix: std::array::from_fn(|index| values[index]),
            fog_position: std::array::from_fn(|index| values[16 + index]),
            color_modulator: std::array::from_fn(|index| values[28 + index]),
            fog_ranges: std::array::from_fn(|index| values[32 + index]),
            glint_alpha: values[36],
        }
    }
    fn encode(&self, projection: [f32; 16]) -> Result<[u8; 256], &'static str> {
        if !projection
            .iter()
            .chain(&self.texture_matrix)
            .chain(&self.fog_position)
            .chain(&self.color_modulator)
            .chain(&self.fog_ranges)
            .all(|value| value.is_finite())
            || !self.glint_alpha.is_finite()
            || !(0. ..=1.).contains(&self.glint_alpha)
            || self.fog_ranges[1] < self.fog_ranges[0]
            || self.fog_ranges[3] < self.fog_ranges[2]
        {
            return Err("invalid explicit armor glint inputs");
        }
        let mut bytes = [0u8; 256];
        let values = projection
            .iter()
            .chain(&self.texture_matrix)
            .chain(&self.fog_position)
            .chain(&self.color_modulator)
            .chain(&self.fog_ranges)
            .copied()
            .chain(std::iter::once(self.glint_alpha));
        for (output, value) in bytes.chunks_exact_mut(4).zip(values) {
            output.copy_from_slice(&value.to_le_bytes());
        }
        Ok(bytes)
    }
}
/// The immutable constant allocation is joined to the actual uploaded buffers
/// and descriptor heap. A matching session/sequence alone cannot substitute.
pub struct Constants {
    resource: ID3D12Resource,
    address: u64,
    node: u32,
    projection: [u32; 16],
    vertices: ID3D12Resource,
    indices: ID3D12Resource,
    descriptors: ID3D12DescriptorHeap,
}
struct Mapping<'a> {
    resource: &'a ID3D12Resource,
    size: usize,
}
impl Drop for Mapping<'_> {
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
    /// Creates only an Arctic upload-heap buffer. No GPU commands are issued.
    /// The supplied input cohort must belong to these selected wing submits.
    pub fn for_uploaded(
        device: &ID3D12Device,
        target: Target,
        uploaded: &Uploaded,
        input: &Inputs,
    ) -> Result<Self, String> {
        Self::for_profiles(device, target, uploaded, std::slice::from_ref(input))?
            .pop()
            .ok_or_else(|| "missing glint constant profile".into())
    }
    /// Pack independently selected uniform profiles into one aligned arena.
    /// Reuse depends on caller-proven exact profile equality, not resemblance.
    pub fn for_profiles(
        device: &ID3D12Device,
        target: Target,
        uploaded: &Uploaded,
        inputs: &[Inputs],
    ) -> Result<Vec<Self>, String> {
        let _private = crate::depth_tracker::private_commands();
        avatar_material::validate_target(&target)?;
        if inputs.is_empty() || inputs.len() > 2048 {
            return Err("invalid glint constant profile count".into());
        }
        if uploaded.glint_texture_handle().is_none() {
            return Err("verified armor glint artwork is not uploaded".into());
        }
        if resource_device(uploaded.vertex_resource())? != *device {
            return Err("glint constants use another upload device".into());
        }
        let projection =
            avatar_material::projection(uploaded.frame(), Kind::WingCutout, target.reverse_z)
                .map_err(str::to_owned)?;
        let bytes: Vec<[u8; 256]> = inputs
            .iter()
            .map(|input| input.encode(projection).map_err(str::to_owned))
            .collect::<Result<_, _>>()?;
        let size = bytes.len() * 256;
        let nodes = unsafe { device.GetNodeCount() };
        let node = if target.node_mask == 0 {
            1
        } else {
            target.node_mask
        };
        if !(1..=32).contains(&nodes) || node.count_ones() != 1 || (node as u64) >= (1u64 << nodes)
        {
            return Err("glint constants require a valid device node".into());
        }
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
        .map_err(|error| error.to_string())?;
        let resource: ID3D12Resource = resource.ok_or("missing glint constant allocation")?;
        let mut pointer = std::ptr::null_mut();
        unsafe {
            resource.Map(
                0,
                Some(&D3D12_RANGE { Begin: 0, End: 0 }),
                Some(&mut pointer),
            )
        }
        .map_err(|error| error.to_string())?;
        let mapping = Mapping {
            resource: &resource,
            size,
        };
        if pointer.is_null() {
            return Err("null glint constant mapping".into());
        }
        for (index, bytes) in bytes.iter().enumerate() {
            unsafe {
                std::ptr::copy_nonoverlapping(
                    bytes.as_ptr(),
                    (pointer as *mut u8).add(index * 256),
                    256,
                );
            }
        }
        drop(mapping);
        let address = unsafe { resource.GetGPUVirtualAddress() };
        if address == 0 || !address.is_multiple_of(256) {
            return Err("invalid glint constant GPU address".into());
        }
        let mut result = Vec::with_capacity(inputs.len());
        for index in 0..inputs.len() {
            let at = address
                .checked_add(index as u64 * 256)
                .ok_or("glint constant address overflow")?;
            result.push(Self {
                resource: resource.clone(),
                address: at,
                node,
                projection: projection.map(f32::to_bits),
                vertices: uploaded.vertex_resource().clone(),
                indices: uploaded.index_resource().clone(),
                descriptors: uploaded.descriptor_heap().clone(),
            });
        }
        Ok(result)
    }
    pub fn resource(&self) -> &ID3D12Resource {
        &self.resource
    }
}
fn resource_device(resource: &ID3D12Resource) -> Result<ID3D12Device, String> {
    let mut device = None;
    unsafe { resource.GetDevice(&mut device) }.map_err(|error| error.to_string())?;
    device.ok_or_else(|| "glint resource has no device".into())
}

pub struct Pipeline {
    root: ID3D12RootSignature,
    state: ID3D12PipelineState,
    target: Target,
}
struct Draw {
    vertex: D3D12_VERTEX_BUFFER_VIEW,
    index: D3D12_INDEX_BUFFER_VIEW,
    count: u32,
}
/// Borrows all owned shader/resource inputs through recording. Their native
/// draw owner must separately retain them through the GPU retirement fence.
pub struct Prepared<'a> {
    pipeline: &'a Pipeline,
    uploaded: &'a Uploaded,
    constants: &'a Constants,
    texture: D3D12_GPU_DESCRIPTOR_HANDLE,
    draws: Vec<Draw>,
}
impl Prepared<'_> {
    pub fn draws(&self) -> usize {
        self.draws.len()
    }
    /// # Safety
    /// Caller owns/serializes this recording direct list and its matching
    /// device/node/targets/viewport/scissor. The selected base wing geometry
    /// has already written this exact depth with the same projection/poses.
    /// Caller supplies actual glint selection and phase order, restores native
    /// graphics state, and pins every owner/descriptor until draw retirement.
    /// This does not reset, close, execute or establish a native target lease.
    pub unsafe fn record(&self, list: &ID3D12GraphicsCommandList) {
        if self.draws.is_empty() {
            return;
        }
        let _private = crate::depth_tracker::private_commands();
        unsafe {
            list.SetDescriptorHeaps(&[Some(self.uploaded.descriptor_heap().clone())]);
            list.SetGraphicsRootSignature(&self.pipeline.root);
            list.SetPipelineState(&self.pipeline.state);
            list.SetGraphicsRootConstantBufferView(0, self.constants.address);
            list.SetGraphicsRootDescriptorTable(1, self.texture);
            list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            for draw in &self.draws {
                list.IASetVertexBuffers(0, Some(&[draw.vertex]));
                list.IASetIndexBuffer(Some(&draw.index));
                list.DrawIndexedInstanced(draw.count, 1, 0, 0, 0);
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
    const SOURCE: &str = include_str!("shaders/avatar_glint.hlsl");
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
    output.ok_or_else(|| "glint compiler returned no bytecode".into())
}
impl Pipeline {
    pub fn target(&self) -> Target {
        self.target
    }
    /// Source construction only; no shader compilation occurs until a future
    /// owning runtime consumer explicitly calls this constructor.
    pub fn new(device: &ID3D12Device, target: Target) -> Result<Self, String> {
        avatar_material::validate_target(&target)?;
        unsafe {
            let range = D3D12_DESCRIPTOR_RANGE {
                RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_SRV,
                NumDescriptors: 1,
                BaseShaderRegister: 0,
                RegisterSpace: 0,
                OffsetInDescriptorsFromTableStart: 0,
            };
            let parameters = [
                D3D12_ROOT_PARAMETER {
                    ParameterType: D3D12_ROOT_PARAMETER_TYPE_CBV,
                    ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL,
                    Anonymous: D3D12_ROOT_PARAMETER_0 {
                        Descriptor: D3D12_ROOT_DESCRIPTOR {
                            ShaderRegister: 0,
                            RegisterSpace: 0,
                        },
                    },
                },
                D3D12_ROOT_PARAMETER {
                    ParameterType: D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE,
                    ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL,
                    Anonymous: D3D12_ROOT_PARAMETER_0 {
                        DescriptorTable: D3D12_ROOT_DESCRIPTOR_TABLE {
                            NumDescriptorRanges: 1,
                            pDescriptorRanges: &range,
                        },
                    },
                },
            ];
            let sampler = D3D12_STATIC_SAMPLER_DESC {
                Filter: D3D12_FILTER_MIN_MAG_LINEAR_MIP_POINT,
                AddressU: D3D12_TEXTURE_ADDRESS_MODE_WRAP,
                AddressV: D3D12_TEXTURE_ADDRESS_MODE_WRAP,
                AddressW: D3D12_TEXTURE_ADDRESS_MODE_WRAP,
                MaxAnisotropy: 1,
                ComparisonFunc: D3D12_COMPARISON_FUNC_ALWAYS,
                ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL,
                MaxLOD: 0.,
                ..Default::default()
            };
            let description = D3D12_ROOT_SIGNATURE_DESC {
                NumParameters: 2,
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
                &description,
                D3D_ROOT_SIGNATURE_VERSION_1_0,
                &mut serialized,
                Some(&mut error),
            )
            .map_err(|value| blob_error(error, value.to_string()))?;
            let serialized =
                serialized.ok_or("glint root signature serialization returned no data")?;
            let root: ID3D12RootSignature = device
                .CreateRootSignature(
                    target.node_mask,
                    slice::from_raw_parts(
                        serialized.GetBufferPointer() as *const u8,
                        serialized.GetBufferSize(),
                    ),
                )
                .map_err(|error| error.to_string())?;
            let vertex = compile(true)?;
            let pixel = compile(false)?;
            let element = |name, format, offset| D3D12_INPUT_ELEMENT_DESC {
                SemanticName: name,
                SemanticIndex: 0,
                Format: format,
                InputSlot: 0,
                AlignedByteOffset: offset,
                InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
                InstanceDataStepRate: 0,
            };
            let layout = [
                element(s!("POSITION"), DXGI_FORMAT_R32G32B32_FLOAT, 0),
                element(s!("TEXCOORD"), DXGI_FORMAT_R32G32_FLOAT, 24),
            ];
            let mut blend = [D3D12_RENDER_TARGET_BLEND_DESC::default(); 8];
            blend[0] = D3D12_RENDER_TARGET_BLEND_DESC {
                BlendEnable: true.into(),
                LogicOpEnable: false.into(),
                SrcBlend: D3D12_BLEND_SRC_COLOR,
                DestBlend: D3D12_BLEND_ONE,
                BlendOp: D3D12_BLEND_OP_ADD,
                SrcBlendAlpha: D3D12_BLEND_ZERO,
                DestBlendAlpha: D3D12_BLEND_ONE,
                BlendOpAlpha: D3D12_BLEND_OP_ADD,
                RenderTargetWriteMask: D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8,
                ..Default::default()
            };
            let stencil = D3D12_DEPTH_STENCILOP_DESC {
                StencilFailOp: D3D12_STENCIL_OP_KEEP,
                StencilDepthFailOp: D3D12_STENCIL_OP_KEEP,
                StencilPassOp: D3D12_STENCIL_OP_KEEP,
                StencilFunc: D3D12_COMPARISON_FUNC_ALWAYS,
            };
            let mut formats = [DXGI_FORMAT_UNKNOWN; 8];
            formats[0] = target.color;
            let description = PipelineDescription(D3D12_GRAPHICS_PIPELINE_STATE_DESC {
                pRootSignature: ManuallyDrop::new(Some(root.clone())),
                NodeMask: target.node_mask,
                VS: D3D12_SHADER_BYTECODE {
                    pShaderBytecode: vertex.GetBufferPointer(),
                    BytecodeLength: vertex.GetBufferSize(),
                },
                PS: D3D12_SHADER_BYTECODE {
                    pShaderBytecode: pixel.GetBufferPointer(),
                    BytecodeLength: pixel.GetBufferSize(),
                },
                InputLayout: D3D12_INPUT_LAYOUT_DESC {
                    pInputElementDescs: layout.as_ptr(),
                    NumElements: 2,
                },
                PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
                SampleMask: u32::MAX,
                NumRenderTargets: 1,
                RTVFormats: formats,
                DSVFormat: target.depth,
                SampleDesc: target.samples,
                BlendState: D3D12_BLEND_DESC {
                    RenderTarget: blend,
                    ..Default::default()
                },
                RasterizerState: D3D12_RASTERIZER_DESC {
                    FillMode: D3D12_FILL_MODE_SOLID,
                    CullMode: D3D12_CULL_MODE_NONE,
                    FrontCounterClockwise: target.front_counter_clockwise.into(),
                    DepthClipEnable: true.into(),
                    MultisampleEnable: (target.samples.Count > 1).into(),
                    ..Default::default()
                },
                DepthStencilState: D3D12_DEPTH_STENCIL_DESC {
                    DepthEnable: true.into(),
                    DepthWriteMask: D3D12_DEPTH_WRITE_MASK_ZERO,
                    DepthFunc: D3D12_COMPARISON_FUNC_EQUAL,
                    StencilEnable: false.into(),
                    StencilReadMask: 255,
                    StencilWriteMask: 255,
                    FrontFace: stencil,
                    BackFace: stencil,
                },
                ..Default::default()
            });
            let state = device
                .CreateGraphicsPipelineState(&description.0)
                .map_err(|error| error.to_string())?;
            Ok(Self {
                root,
                state,
                target,
            })
        }
    }
    /// Select whole wing models using the owning glint phase's observed order.
    /// Reuses all their base batch buffers verbatim, including UVs and indices.
    /// Nothing is inferred from enchantments, base textures, tint or glow.
    pub fn prepare_wings<'a>(
        &'a self,
        uploaded: &'a Uploaded,
        constants: &'a Constants,
        models: &[u16],
    ) -> Result<Prepared<'a>, String> {
        if models.len() > 2048 {
            return Err("glint model budget exceeded".into());
        }
        let node = if self.target.node_mask == 0 {
            1
        } else {
            self.target.node_mask
        };
        let projection =
            avatar_material::projection(uploaded.frame(), Kind::WingCutout, self.target.reverse_z)
                .map_err(str::to_owned)?;
        if constants.vertices != *uploaded.vertex_resource()
            || constants.indices != *uploaded.index_resource()
            || constants.descriptors != *uploaded.descriptor_heap()
            || constants.node != node
            || constants.projection != projection.map(f32::to_bits)
        {
            return Err("glint constants belong to different geometry/projection/node".into());
        }
        let mut device: Option<ID3D12Device> = None;
        unsafe { self.root.GetDevice(&mut device) }.map_err(|error| error.to_string())?;
        let device = device.ok_or("missing glint pipeline device")?;
        if resource_device(&constants.resource)? != device {
            return Err("glint pipeline uses another constant device".into());
        }
        for resource in [
            uploaded.vertex_resource(),
            uploaded.index_resource(),
            constants.resource(),
        ] {
            let mut properties = D3D12_HEAP_PROPERTIES::default();
            unsafe { resource.GetHeapProperties(Some(&mut properties), None) }
                .map_err(|error| error.to_string())?;
            let visible = if properties.VisibleNodeMask == 0 {
                1
            } else {
                properties.VisibleNodeMask
            };
            if visible & node != node {
                return Err("glint resource is not visible to its target node".into());
            }
        }
        let heap = unsafe { uploaded.descriptor_heap().GetDesc() };
        let heap_node = if heap.NodeMask == 0 { 1 } else { heap.NodeMask };
        if heap_node & node != node {
            return Err("glint descriptor heap uses another target node".into());
        }
        let texture = uploaded
            .glint_texture_handle()
            .ok_or("verified armor glint artwork is not uploaded")?;
        let mut seen = HashSet::new();
        let mut draws = Vec::new();
        for &model in models {
            if !seen.insert(model) {
                return Err("glint phase repeats a model".into());
            }
            let mut found = false;
            for (index, batch) in uploaded.frame().batches.iter().enumerate() {
                if batch
                    .model_submission
                    .is_none_or(|submit| submit.id != model)
                {
                    continue;
                }
                if batch.pass != Pass::WingCutout || !batch.layer_visibility_known {
                    return Err("glint model has no known wing-layer eligibility".into());
                }
                found = true;
                if batch.index_count == 0 {
                    continue;
                }
                let buffers = uploaded
                    .draw_buffers(index)
                    .ok_or("missing glint base buffers")?;
                if buffers.vertex.StrideInBytes != 40
                    || buffers.vertex.SizeInBytes as usize != batch.vertex_count * 40
                    || buffers.index.Format != DXGI_FORMAT_R32_UINT
                    || buffers.index.SizeInBytes as usize != batch.index_count * 4
                {
                    return Err("glint base buffers disagree with their owned geometry".into());
                }
                draws.push(Draw {
                    vertex: buffers.vertex,
                    index: buffers.index,
                    count: batch.index_count as u32,
                });
            }
            if !found {
                return Err("glint phase selected an unknown wing model".into());
            }
        }
        Ok(Prepared {
            pipeline: self,
            uploaded,
            constants,
            texture,
            draws,
        })
    }
}
