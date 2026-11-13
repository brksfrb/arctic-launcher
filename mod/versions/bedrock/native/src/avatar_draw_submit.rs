//! Submission to caller-owned native targets. The installed engine's target
//! lease producer is not implemented here; COM references alone are not leases.
use crate::{
    avatar_draw_order::{FeatureGroup, Plan},
    avatar_gpu_upload::Uploaded,
    avatar_material::{Constants, Pipelines, Target},
};
use hudhook::windows::Win32::{
    Foundation::RECT,
    Graphics::{Direct3D12::*, Dxgi::Common::*},
};
use std::{any::Any, mem::ManuallyDrop, sync::Arc};

pub struct States {
    pub color: D3D12_RESOURCE_STATES,
    pub depth: D3D12_RESOURCE_STATES,
    pub stencil: Option<D3D12_RESOURCE_STATES>,
}
pub struct Targets {
    session: String,
    sequence: u64,
    world_scope: u64,
    color: ID3D12Resource,
    depth: ID3D12Resource,
    states: States,
    _native_owner: Box<dyn Any + Send + Sync>,
}
impl Targets {
    /// # Safety
    /// The adapter independently owns these complete color/depth surfaces for
    /// the identified native world scope and packet, including their current
    /// per-plane states. native_owner pins that actual ownership against engine
    /// writes/reuse until dropped on any thread. The constructor does not
    /// discover or prove a lease from the identifiers or COM references.
    pub unsafe fn from_owned(
        session: String,
        sequence: u64,
        world_scope: u64,
        color: ID3D12Resource,
        depth: ID3D12Resource,
        states: States,
        native_owner: Box<dyn Any + Send + Sync>,
    ) -> Self {
        Self {
            session,
            sequence,
            world_scope,
            color,
            depth,
            states,
            _native_owner: native_owner,
        }
    }
}
struct Owners {
    targets: Targets,
    _uploaded: Uploaded,
    _constants: Constants,
    _pipelines: Arc<Pipelines>,
    _rtv: ID3D12DescriptorHeap,
    _dsv: ID3D12DescriptorHeap,
    ranges: usize,
}
static FLIGHTS: crate::gpu_flight::Pool<Owners, 3> = crate::gpu_flight::Pool::new();
pub struct Ticket {
    flight: crate::gpu_flight::Ticket<Owners, 3>,
}
/// Private-fence retirement for the supplied target scope, not pixel-fidelity
/// verification or independent proof of the caller's native lease assertion.
pub struct Retired {
    pub session: String,
    pub sequence: u64,
    pub world_scope: u64,
    pub draw_ranges: usize,
}
impl Ticket {
    pub fn take_completed(&mut self) -> Result<Option<Retired>, String> {
        Ok(self.flight.take_completed()?.map(|owners| Retired {
            session: owners.targets.session.clone(),
            sequence: owners.targets.sequence,
            world_scope: owners.targets.world_scope,
            draw_ranges: owners.ranges,
        }))
    }
}
pub(crate) fn collect_retired() {
    FLIGHTS.collect();
}

struct Barrier(D3D12_RESOURCE_BARRIER);
impl Barrier {
    fn new(
        resource: &ID3D12Resource,
        before: D3D12_RESOURCE_STATES,
        after: D3D12_RESOURCE_STATES,
        plane: u32,
    ) -> Self {
        Self(D3D12_RESOURCE_BARRIER {
            Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
            Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
            Anonymous: D3D12_RESOURCE_BARRIER_0 {
                Transition: ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                    pResource: ManuallyDrop::new(Some(resource.clone())),
                    Subresource: plane,
                    StateBefore: before,
                    StateAfter: after,
                }),
            },
        })
    }
}
impl Drop for Barrier {
    fn drop(&mut self) {
        unsafe {
            ManuallyDrop::drop(&mut (*self.0.Anonymous.Transition).pResource);
        }
    }
}
fn add_transition(
    before: &mut Vec<Barrier>,
    after: &mut Vec<Barrier>,
    resource: &ID3D12Resource,
    current: D3D12_RESOURCE_STATES,
    wanted: D3D12_RESOURCE_STATES,
    plane: u32,
) {
    if current != wanted {
        before.push(Barrier::new(resource, current, wanted, plane));
        after.push(Barrier::new(resource, wanted, current, plane));
    }
}
fn resource_device(resource: &ID3D12Resource) -> Result<ID3D12Device, String> {
    let mut value = None;
    unsafe { resource.GetDevice(&mut value) }.map_err(|error| error.to_string())?;
    value.ok_or_else(|| "native avatar target has no device".into())
}
fn depth_format(resource: DXGI_FORMAT, view: DXGI_FORMAT) -> Option<bool> {
    match view {
        DXGI_FORMAT_D32_FLOAT
            if matches!(resource, DXGI_FORMAT_D32_FLOAT | DXGI_FORMAT_R32_TYPELESS) =>
        {
            Some(false)
        }
        DXGI_FORMAT_D16_UNORM
            if matches!(resource, DXGI_FORMAT_D16_UNORM | DXGI_FORMAT_R16_TYPELESS) =>
        {
            Some(false)
        }
        DXGI_FORMAT_D24_UNORM_S8_UINT
            if matches!(
                resource,
                DXGI_FORMAT_D24_UNORM_S8_UINT | DXGI_FORMAT_R24G8_TYPELESS
            ) =>
        {
            Some(true)
        }
        DXGI_FORMAT_D32_FLOAT_S8X24_UINT
            if matches!(
                resource,
                DXGI_FORMAT_D32_FLOAT_S8X24_UINT | DXGI_FORMAT_R32G8X24_TYPELESS
            ) =>
        {
            Some(true)
        }
        _ => None,
    }
}
fn color_format(resource: DXGI_FORMAT, view: DXGI_FORMAT) -> bool {
    match view {
        DXGI_FORMAT_R8G8B8A8_UNORM => matches!(
            resource,
            DXGI_FORMAT_R8G8B8A8_UNORM | DXGI_FORMAT_R8G8B8A8_TYPELESS
        ),
        DXGI_FORMAT_B8G8R8A8_UNORM => matches!(
            resource,
            DXGI_FORMAT_B8G8R8A8_UNORM | DXGI_FORMAT_B8G8R8A8_TYPELESS
        ),
        _ => false,
    }
}
fn shape(desc: &D3D12_RESOURCE_DESC, width: u32, height: u32, target: &Target) -> bool {
    desc.Dimension == D3D12_RESOURCE_DIMENSION_TEXTURE2D
        && desc.Width == width as u64
        && desc.Height == height
        && desc.DepthOrArraySize == 1
        && desc.MipLevels == 1
        && desc.SampleDesc == target.samples
}
fn descriptor_heap(
    device: &ID3D12Device,
    kind: D3D12_DESCRIPTOR_HEAP_TYPE,
    node: u32,
) -> Result<ID3D12DescriptorHeap, String> {
    unsafe {
        device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
            Type: kind,
            NumDescriptors: 1,
            Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE,
            NodeMask: node,
        })
    }
    .map_err(|error| error.to_string())
}

/// Record, restore target states, close and submit one fully owned draw list.
/// # Safety
/// Targets' native lease must still be current; the supplied group IDs, shader
/// inputs and reverse-Z/front-face convention belong to that exact world/depth
/// cohort. The owning adapter must synchronize engine work on these surfaces.
/// No current-target discovery or ownership attestation is inferred here.
pub unsafe fn submit(
    queue: &ID3D12CommandQueue,
    targets: Targets,
    uploaded: Uploaded,
    pipelines: Arc<Pipelines>,
    constants: Constants,
    groups: &[FeatureGroup],
    uniform_indices: &[usize],
) -> Result<Option<Ticket>, String> {
    let _private = crate::depth_tracker::private_commands();
    let permit = FLIGHTS.claim()?;
    let frame = uploaded.frame();
    let target = pipelines.target();
    if !(1..=3840).contains(&frame.width)
        || !(1..=2160).contains(&frame.height)
        || targets.world_scope == 0
        || targets.session != frame.session
        || targets.sequence != frame.sequence
        || targets.color == targets.depth
    {
        return Err("native avatar targets disagree with the geometry world scope".into());
    }
    let device = resource_device(&targets.color)?;
    if device != resource_device(&targets.depth)?
        || device != resource_device(uploaded.vertex_resource())?
        || device != resource_device(constants.resource())?
        || device != pipelines.device()?
    {
        return Err("native avatar draw resources belong to different devices".into());
    }
    let mut queue_device: Option<ID3D12Device> = None;
    unsafe { queue.GetDevice(&mut queue_device) }.map_err(|error| error.to_string())?;
    let queue_desc = unsafe { queue.GetDesc() };
    let node = |mask: u32| if mask == 0 { 1 } else { mask };
    let nodes = unsafe { device.GetNodeCount() };
    if queue_device.as_ref() != Some(&device)
        || queue_desc.Type != D3D12_COMMAND_LIST_TYPE_DIRECT
        || node(queue_desc.NodeMask) != node(target.node_mask)
        || !(1..=32).contains(&nodes)
        || node(target.node_mask).count_ones() != 1
        || (node(target.node_mask) as u64) >= (1u64 << nodes)
    {
        return Err("native avatar draw requires the matching direct device/node queue".into());
    }
    let color = unsafe { targets.color.GetDesc() };
    let depth = unsafe { targets.depth.GetDesc() };
    let stencil = depth_format(depth.Format, target.depth)
        .ok_or("native depth resource/view formats disagree")?;
    if !shape(&color, frame.width, frame.height, &target)
        || !shape(&depth, frame.width, frame.height, &target)
        || !color_format(color.Format, target.color)
        || color.Flags.0 & D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET.0 == 0
        || depth.Flags.0 & D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL.0 == 0
        || stencil != targets.states.stencil.is_some()
    {
        return Err("native avatar target dimensions, flags or plane states disagree".into());
    }
    // Build the CPU order before allocating descriptors or recording commands.
    let plan = Plan::new(frame, groups).map_err(str::to_owned)?;
    let ranges = plan.ranges().len();
    if ranges == 0 {
        return Ok(None);
    }
    unsafe {
        let rtv_heap = descriptor_heap(&device, D3D12_DESCRIPTOR_HEAP_TYPE_RTV, target.node_mask)?;
        let dsv_heap = descriptor_heap(&device, D3D12_DESCRIPTOR_HEAP_TYPE_DSV, target.node_mask)?;
        let rtv = rtv_heap.GetCPUDescriptorHandleForHeapStart();
        let dsv = dsv_heap.GetCPUDescriptorHandleForHeapStart();
        device.CreateRenderTargetView(
            &targets.color,
            Some(&D3D12_RENDER_TARGET_VIEW_DESC {
                Format: target.color,
                ViewDimension: if target.samples.Count == 1 {
                    D3D12_RTV_DIMENSION_TEXTURE2D
                } else {
                    D3D12_RTV_DIMENSION_TEXTURE2DMS
                },
                ..Default::default()
            }),
            rtv,
        );
        device.CreateDepthStencilView(
            &targets.depth,
            Some(&D3D12_DEPTH_STENCIL_VIEW_DESC {
                Format: target.depth,
                ViewDimension: if target.samples.Count == 1 {
                    D3D12_DSV_DIMENSION_TEXTURE2D
                } else {
                    D3D12_DSV_DIMENSION_TEXTURE2DMS
                },
                Flags: if stencil {
                    D3D12_DSV_FLAG_READ_ONLY_STENCIL
                } else {
                    D3D12_DSV_FLAG_NONE
                },
                ..Default::default()
            }),
            dsv,
        );
        let allocator: ID3D12CommandAllocator = device
            .CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)
            .map_err(|error| error.to_string())?;
        let list: ID3D12GraphicsCommandList = device
            .CreateCommandList(
                target.node_mask,
                D3D12_COMMAND_LIST_TYPE_DIRECT,
                &allocator,
                None,
            )
            .map_err(|error| error.to_string())?;
        let mut before = Vec::new();
        let mut after = Vec::new();
        add_transition(
            &mut before,
            &mut after,
            &targets.color,
            targets.states.color,
            D3D12_RESOURCE_STATE_RENDER_TARGET,
            0,
        );
        add_transition(
            &mut before,
            &mut after,
            &targets.depth,
            targets.states.depth,
            D3D12_RESOURCE_STATE_DEPTH_WRITE,
            0,
        );
        if let Some(state) = targets.states.stencil {
            add_transition(
                &mut before,
                &mut after,
                &targets.depth,
                state,
                D3D12_RESOURCE_STATE_DEPTH_READ,
                1,
            );
        }
        for barrier in &before {
            list.ResourceBarrier(std::slice::from_ref(&barrier.0));
        }
        list.OMSetRenderTargets(1, Some(&rtv), false, Some(&dsv));
        list.RSSetViewports(&[D3D12_VIEWPORT {
            TopLeftX: 0.,
            TopLeftY: 0.,
            Width: frame.width as f32,
            Height: frame.height as f32,
            MinDepth: 0.,
            MaxDepth: 1.,
        }]);
        list.RSSetScissorRects(&[RECT {
            left: 0,
            top: 0,
            right: frame.width as i32,
            bottom: frame.height as i32,
        }]);
        // A rejection leaves only this unsubmitted private list, which is
        // discarded. Target resources have not yet changed on the GPU.
        pipelines.record_ordered(&list, &uploaded, &plan, &constants, uniform_indices)?;
        drop(plan);
        for barrier in after.iter().rev() {
            list.ResourceBarrier(std::slice::from_ref(&barrier.0));
        }
        list.Close().map_err(|error| error.to_string())?;
        let owners = Owners {
            targets,
            _uploaded: uploaded,
            _constants: constants,
            _pipelines: pipelines,
            _rtv: rtv_heap,
            _dsv: dsv_heap,
            ranges,
        };
        Ok(Some(Ticket {
            flight: permit.submit(&device, queue, allocator, list, owners)?,
        }))
    }
}
