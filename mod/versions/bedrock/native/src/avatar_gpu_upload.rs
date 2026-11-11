//! D3D12 upload ownership for copied avatar geometry. No draw is submitted here.
//! The caller must independently join the packet to the current native world,
//! camera, depth and producer scope before using these resources for rendering.
use crate::avatar_geometry::Frame;
use hudhook::windows::Win32::Graphics::{Direct3D12::*, Dxgi::Common::*};
use sha2::{Digest, Sha256};
use std::mem::ManuallyDrop;

const MAX_UPLOAD_BYTES: u64 = 128 * 1024 * 1024;
const WHITE_ARGB: [u8; 4] = [255; 4];
pub const GLINT_RGBA_BYTES: usize = 128 * 128 * 4;
/// Pinned straight RGBA asset pixels, independent of a material selector.
pub struct GlintArtwork {
    rgba: Vec<u8>,
}
impl GlintArtwork {
    pub fn from_rgba(rgba: Vec<u8>) -> Result<Self, &'static str> {
        const HASH: [u8; 32] = [
            0x0c, 0xdd, 0x3e, 0xe2, 0xf5, 0xb3, 0x53, 0x82, 0xfa, 0xd3, 0xf7, 0xa1, 0x68, 0x86,
            0x22, 0x1d, 0xe3, 0x23, 0xad, 0x8b, 0x7f, 0x41, 0xcf, 0x67, 0x2a, 0x14, 0x34, 0x9c,
            0x10, 0xae, 0x68, 0x6b,
        ];
        if rgba.len() != GLINT_RGBA_BYTES || Sha256::digest(&rgba)[..] != HASH[..] {
            return Err("armor glint pixels differ from the pinned Java reference");
        }
        Ok(Self { rgba })
    }
}
// One upload ticket at a time. An abandoned or unsignaled GPU submission holds
// this permit until its fence retires or device removal is actually reported.
static FLIGHTS: crate::gpu_flight::Pool<UploadOwners, 1> = crate::gpu_flight::Pool::new();

pub struct DrawBuffers {
    pub vertex: D3D12_VERTEX_BUFFER_VIEW,
    pub index: D3D12_INDEX_BUFFER_VIEW,
}
pub struct Uploaded {
    frame: Frame,
    draws: Vec<DrawBuffers>,
    vertices: ID3D12Resource,
    indices: ID3D12Resource,
    textures: Vec<ID3D12Resource>,
    descriptors: ID3D12DescriptorHeap,
    descriptor_step: u32,
    glint_slot: Option<usize>,
}
impl Uploaded {
    pub fn frame(&self) -> &Frame {
        &self.frame
    }
    pub fn draw_buffers(&self, batch: usize) -> Option<&DrawBuffers> {
        self.draws.get(batch)
    }
    /// Borrow only. A rendering owner must retain this entire Uploaded value
    /// through its own draw fence; upload completion does not retire later draws.
    pub fn descriptor_heap(&self) -> &ID3D12DescriptorHeap {
        &self.descriptors
    }
    /// Slot zero is the white fallback; packet texture N occupies slot N+1.
    pub fn texture_handle(&self, texture: Option<usize>) -> Option<D3D12_GPU_DESCRIPTOR_HANDLE> {
        if texture.is_some_and(|index| index >= self.frame.textures.len()) {
            return None;
        }
        let slot = texture.map_or(Some(0), |index| index.checked_add(1))?;
        self.slot_handle(slot)
    }
    pub(crate) fn glint_texture_handle(&self) -> Option<D3D12_GPU_DESCRIPTOR_HANDLE> {
        self.slot_handle(self.glint_slot?)
    }
    fn slot_handle(&self, slot: usize) -> Option<D3D12_GPU_DESCRIPTOR_HANDLE> {
        if slot >= self.textures.len() {
            return None;
        }
        let start = unsafe { self.descriptors.GetGPUDescriptorHandleForHeapStart() };
        Some(D3D12_GPU_DESCRIPTOR_HANDLE {
            ptr: start
                .ptr
                .checked_add((slot as u64) * self.descriptor_step as u64)?,
        })
    }
    pub fn vertex_resource(&self) -> &ID3D12Resource {
        &self.vertices
    }
    pub fn index_resource(&self) -> &ID3D12Resource {
        &self.indices
    }
    pub(crate) fn draw_resources(&self) -> impl Iterator<Item = &ID3D12Resource> {
        std::iter::once(&self.vertices)
            .chain(std::iter::once(&self.indices))
            .chain(self.textures.iter())
    }
}

struct UploadOwners {
    uploaded: Uploaded,
    _upload: ID3D12Resource,
}
pub struct Ticket {
    flight: crate::gpu_flight::Ticket<UploadOwners, 1>,
}
pub(crate) fn collect_retired() {
    FLIGHTS.collect();
}
impl Ticket {
    /// Nonblocking; returns the resources once, only after the upload fence.
    pub fn take_completed(&mut self) -> Result<Option<Uploaded>, String> {
        Ok(self.flight.take_completed()?.map(|owners| owners.uploaded))
    }
}

struct Transition(D3D12_RESOURCE_BARRIER);
impl Transition {
    fn from_copy(resource: &ID3D12Resource, after: D3D12_RESOURCE_STATES) -> Self {
        Self(D3D12_RESOURCE_BARRIER {
            Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
            Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
            Anonymous: D3D12_RESOURCE_BARRIER_0 {
                Transition: ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                    pResource: ManuallyDrop::new(Some(resource.clone())),
                    Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                    StateBefore: D3D12_RESOURCE_STATE_COPY_DEST,
                    StateAfter: after,
                }),
            },
        })
    }
}
impl Drop for Transition {
    fn drop(&mut self) {
        unsafe {
            ManuallyDrop::drop(&mut (*self.0.Anonymous.Transition).pResource);
        }
    }
}
struct Location(D3D12_TEXTURE_COPY_LOCATION);
impl Drop for Location {
    fn drop(&mut self) {
        unsafe {
            ManuallyDrop::drop(&mut self.0.pResource);
        }
    }
}
struct Mapping<'a> {
    resource: &'a ID3D12Resource,
    pointer: *mut u8,
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
struct TexturePlan {
    desc: D3D12_RESOURCE_DESC,
    footprint: D3D12_PLACED_SUBRESOURCE_FOOTPRINT,
}
fn buffer_desc(size: u64) -> D3D12_RESOURCE_DESC {
    D3D12_RESOURCE_DESC {
        Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
        Width: size,
        Height: 1,
        DepthOrArraySize: 1,
        MipLevels: 1,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
        ..Default::default()
    }
}
fn create(
    device: &ID3D12Device,
    node: u32,
    kind: D3D12_HEAP_TYPE,
    desc: &D3D12_RESOURCE_DESC,
    state: D3D12_RESOURCE_STATES,
) -> Result<ID3D12Resource, String> {
    let mut value = None;
    unsafe {
        device.CreateCommittedResource(
            &D3D12_HEAP_PROPERTIES {
                Type: kind,
                CreationNodeMask: node,
                VisibleNodeMask: node,
                ..Default::default()
            },
            D3D12_HEAP_FLAG_NONE,
            desc,
            state,
            None,
            &mut value,
        )
    }
    .map_err(|e| e.to_string())?;
    value.ok_or_else(|| "missing avatar GPU resource".into())
}
fn align_texture(value: u64) -> Result<u64, String> {
    value
        .checked_add(511)
        .map(|value| value & !511)
        .ok_or_else(|| "avatar texture alignment overflow".into())
}

/// Record and submit uploads on the supplied live direct queue. This function
/// touches only Arctic-created resources. It does not access/transition world
/// depth, back buffers, engine Mesh objects, native materials or player state.
pub fn begin(
    device: &ID3D12Device,
    queue: &ID3D12CommandQueue,
    frame: Frame,
) -> Result<Option<Ticket>, String> {
    begin_with_glint(device, queue, frame, None)
}
/// The optional verified artwork uses a dedicated owned descriptor, never a
/// packet texture alias. This does not select glint for any equipment/model.
pub fn begin_with_glint(
    device: &ID3D12Device,
    queue: &ID3D12CommandQueue,
    frame: Frame,
    glint: Option<GlintArtwork>,
) -> Result<Option<Ticket>, String> {
    let _private = crate::depth_tracker::private_commands();
    if frame.batches.is_empty() {
        return Ok(None);
    }
    let permit = FLIGHTS.claim()?;
    // All fallible preparation precedes ExecuteCommandLists. The permit and
    // ordinary COM owners are sufficient to clean up every preparation error.
    unsafe {
        let mut queue_device: Option<ID3D12Device> = None;
        queue
            .GetDevice(&mut queue_device)
            .map_err(|e| e.to_string())?;
        let queue_desc = queue.GetDesc();
        let node = if queue_desc.NodeMask == 0 {
            1
        } else {
            queue_desc.NodeMask
        };
        let nodes = device.GetNodeCount();
        if queue_device.as_ref() != Some(device)
            || queue_desc.Type != D3D12_COMMAND_LIST_TYPE_DIRECT
            || !(1..=32).contains(&nodes)
            || node.count_ones() != 1
            || (node as u64) >= (1u64 << nodes)
        {
            return Err("avatar upload requires a matching direct device/node queue".into());
        }
        if frame.batches.len() > 2048 || frame.textures.len() > 32 {
            return Err("avatar GPU input exceeds draw/texture budget".into());
        }
        let mut vertex_bytes = 0u64;
        let mut index_bytes = 0u64;
        let mut offsets = Vec::with_capacity(frame.batches.len());
        for (index, batch) in frame.batches.iter().enumerate() {
            let vertices = frame.attributes(index).ok_or("missing avatar attributes")?;
            let indices = frame.indices(index).ok_or("missing avatar indices")?;
            if !batch.vertex_count.is_multiple_of(4)
                || batch.vertex_count.checked_mul(40) != Some(vertices.len())
                || batch.vertex_count / 4 * 6 != batch.index_count
                || batch.index_count.checked_mul(4) != Some(indices.len())
            {
                return Err("avatar GPU input disagrees with decoded topology".into());
            }
            offsets.push((vertex_bytes, index_bytes));
            vertex_bytes = vertex_bytes
                .checked_add(vertices.len() as u64)
                .ok_or("avatar vertex byte overflow")?;
            index_bytes = index_bytes
                .checked_add(indices.len() as u64)
                .ok_or("avatar index byte overflow")?;
            if vertex_bytes > 400_000 * 40 || index_bytes > 600_000 * 4 {
                return Err("avatar GPU geometry budget exceeded".into());
            }
        }
        // Empty submits matter to grouping but need no GPU buffers or commands.
        if vertex_bytes == 0 {
            return Ok(None);
        }
        let index_start = vertex_bytes;
        let mut total = vertex_bytes
            .checked_add(index_bytes)
            .ok_or("avatar upload byte overflow")?;
        let glint_slot = glint.as_ref().map(|_| frame.textures.len() + 1);
        let texture_count = frame.textures.len() + 1 + usize::from(glint.is_some());
        let mut plans = Vec::with_capacity(texture_count);
        let mut pixels = 0usize;
        // Slot zero is a real uploaded white texel, shared by all untextured draws.
        for slot in 0..texture_count {
            let (width, height) = if slot == 0 {
                (1, 1)
            } else if Some(slot) == glint_slot {
                (128, 128)
            } else {
                let texture = &frame.textures[slot - 1];
                (texture.width, texture.height)
            };
            if !(1..=4096).contains(&width) || !(1..=4096).contains(&height) {
                return Err("invalid avatar GPU texture dimensions".into());
            }
            let count = width
                .checked_mul(height)
                .ok_or("avatar GPU pixel overflow")?;
            if slot > 0 && Some(slot) != glint_slot {
                pixels = pixels
                    .checked_add(count)
                    .ok_or("avatar GPU pixel budget overflow")?;
                if pixels > 16_777_216
                    || frame.texture_argb(slot - 1).map(|bytes| bytes.len()) != count.checked_mul(4)
                {
                    return Err("avatar GPU pixels disagree with decoded texture budget".into());
                }
            }
            let desc = D3D12_RESOURCE_DESC {
                Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
                Width: width as u64,
                Height: height as u32,
                DepthOrArraySize: 1,
                MipLevels: 1,
                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Layout: D3D12_TEXTURE_LAYOUT_UNKNOWN,
                ..Default::default()
            };
            let mut footprint = D3D12_PLACED_SUBRESOURCE_FOOTPRINT::default();
            let mut rows = 0;
            let mut row_bytes = 0;
            let mut size = 0;
            device.GetCopyableFootprints(
                &desc,
                0,
                1,
                0,
                Some(&mut footprint),
                Some(&mut rows),
                Some(&mut row_bytes),
                Some(&mut size),
            );
            let pitch = footprint.Footprint.RowPitch as u64;
            let required = pitch
                .checked_mul(height as u64 - 1)
                .and_then(|value| value.checked_add(width as u64 * 4))
                .ok_or("avatar texture footprint overflow")?;
            if footprint.Offset != 0
                || rows != height as u32
                || row_bytes != width as u64 * 4
                || pitch < row_bytes
                || !pitch.is_multiple_of(256)
                || size < required
                || size > MAX_UPLOAD_BYTES
                || footprint.Footprint.Width != width as u32
                || footprint.Footprint.Height != height as u32
                || footprint.Footprint.Depth != 1
                || footprint.Footprint.Format != DXGI_FORMAT_R8G8B8A8_UNORM
            {
                return Err("unsupported avatar texture upload footprint".into());
            }
            footprint.Offset = align_texture(total)?;
            total = footprint
                .Offset
                .checked_add(size)
                .ok_or("avatar texture upload overflow")?;
            if total > MAX_UPLOAD_BYTES {
                return Err("avatar upload allocation exceeds 128 MiB".into());
            }
            plans.push(TexturePlan { desc, footprint });
        }
        let upload = create(
            device,
            node,
            D3D12_HEAP_TYPE_UPLOAD,
            &buffer_desc(total),
            D3D12_RESOURCE_STATE_GENERIC_READ,
        )?;
        let vertices = create(
            device,
            node,
            D3D12_HEAP_TYPE_DEFAULT,
            &buffer_desc(vertex_bytes),
            D3D12_RESOURCE_STATE_COPY_DEST,
        )?;
        let indices = create(
            device,
            node,
            D3D12_HEAP_TYPE_DEFAULT,
            &buffer_desc(index_bytes),
            D3D12_RESOURCE_STATE_COPY_DEST,
        )?;
        let mut pointer = std::ptr::null_mut();
        upload
            .Map(
                0,
                Some(&D3D12_RANGE { Begin: 0, End: 0 }),
                Some(&mut pointer),
            )
            .map_err(|e| e.to_string())?;
        let mapping = Mapping {
            resource: &upload,
            pointer: pointer as *mut u8,
            size: total as usize,
        };
        if mapping.pointer.is_null() {
            return Err("null avatar upload mapping".into());
        }
        let mapped = std::slice::from_raw_parts_mut(mapping.pointer, mapping.size);
        mapped.fill(0);
        for (index, (vertex_offset, index_offset)) in offsets.iter().copied().enumerate() {
            let data = frame
                .attributes(index)
                .ok_or("missing avatar upload attributes")?;
            mapped[vertex_offset as usize..vertex_offset as usize + data.len()]
                .copy_from_slice(data);
            let data = frame
                .indices(index)
                .ok_or("missing avatar upload indices")?;
            let start = (index_start + index_offset) as usize;
            mapped[start..start + data.len()].copy_from_slice(data);
        }
        for (slot, plan) in plans.iter().enumerate() {
            let width = plan.desc.Width as usize;
            let height = plan.desc.Height as usize;
            let reference = if Some(slot) == glint_slot {
                Some(glint.as_ref().ok_or("missing glint owner")?.rgba.as_slice())
            } else {
                None
            };
            let source = if let Some(pixels) = reference {
                pixels
            } else if slot == 0 {
                &WHITE_ARGB[..]
            } else {
                frame
                    .texture_argb(slot - 1)
                    .ok_or("missing avatar upload pixels")?
            };
            for row in 0..height {
                let start = plan.footprint.Offset as usize
                    + row * plan.footprint.Footprint.RowPitch as usize;
                let destination = &mut mapped[start..start + width * 4];
                if reference.is_some() {
                    destination.copy_from_slice(&source[row * width * 4..(row + 1) * width * 4]);
                    continue;
                }
                for (output, input) in destination
                    .chunks_exact_mut(4)
                    .zip(source[row * width * 4..(row + 1) * width * 4].chunks_exact(4))
                {
                    output.copy_from_slice(&[input[2], input[1], input[0], input[3]]);
                }
            }
        }
        drop(mapping);
        let allocator: ID3D12CommandAllocator = device
            .CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)
            .map_err(|e| e.to_string())?;
        let list: ID3D12GraphicsCommandList = device
            .CreateCommandList(
                queue_desc.NodeMask,
                D3D12_COMMAND_LIST_TYPE_DIRECT,
                &allocator,
                None,
            )
            .map_err(|e| e.to_string())?;
        list.CopyBufferRegion(&vertices, 0, &upload, 0, vertex_bytes);
        list.CopyBufferRegion(&indices, 0, &upload, index_start, index_bytes);
        let descriptors: ID3D12DescriptorHeap = device
            .CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
                NumDescriptors: plans.len() as u32,
                Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
                NodeMask: queue_desc.NodeMask,
            })
            .map_err(|e| e.to_string())?;
        let descriptor_step =
            device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV);
        let descriptor_start = descriptors.GetCPUDescriptorHandleForHeapStart();
        let mut textures = Vec::with_capacity(plans.len());
        for (slot, plan) in plans.iter().enumerate() {
            let texture = create(
                device,
                node,
                D3D12_HEAP_TYPE_DEFAULT,
                &plan.desc,
                D3D12_RESOURCE_STATE_COPY_DEST,
            )?;
            let source = Location(D3D12_TEXTURE_COPY_LOCATION {
                pResource: ManuallyDrop::new(Some(upload.clone())),
                Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
                Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                    PlacedFootprint: plan.footprint,
                },
            });
            let destination = Location(D3D12_TEXTURE_COPY_LOCATION {
                pResource: ManuallyDrop::new(Some(texture.clone())),
                Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
                Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                    SubresourceIndex: 0,
                },
            });
            list.CopyTextureRegion(&destination.0, 0, 0, 0, &source.0, None);
            let barrier =
                Transition::from_copy(&texture, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE);
            list.ResourceBarrier(std::slice::from_ref(&barrier.0));
            let handle = D3D12_CPU_DESCRIPTOR_HANDLE {
                ptr: descriptor_start
                    .ptr
                    .checked_add(slot * descriptor_step as usize)
                    .ok_or("avatar descriptor overflow")?,
            };
            device.CreateShaderResourceView(
                &texture,
                Some(&D3D12_SHADER_RESOURCE_VIEW_DESC {
                    Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D,
                    Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING,
                    Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 {
                        Texture2D: D3D12_TEX2D_SRV {
                            MipLevels: 1,
                            ..Default::default()
                        },
                    },
                }),
                handle,
            );
            textures.push(texture);
        }
        let vertex_barrier =
            Transition::from_copy(&vertices, D3D12_RESOURCE_STATE_VERTEX_AND_CONSTANT_BUFFER);
        let index_barrier = Transition::from_copy(&indices, D3D12_RESOURCE_STATE_INDEX_BUFFER);
        // Each wrapper owns the COM reference inside its ManuallyDrop union.
        // Borrow the barriers; cloning raw unions would need separate cleanup.
        list.ResourceBarrier(std::slice::from_ref(&vertex_barrier.0));
        list.ResourceBarrier(std::slice::from_ref(&index_barrier.0));
        let vertex_address = vertices.GetGPUVirtualAddress();
        let index_address = indices.GetGPUVirtualAddress();
        if vertex_address == 0 || index_address == 0 {
            return Err("avatar buffer has no GPU address".into());
        }
        let mut draws = Vec::with_capacity(offsets.len());
        for (batch, (vertex_offset, index_offset)) in frame.batches.iter().zip(offsets) {
            draws.push(DrawBuffers {
                vertex: D3D12_VERTEX_BUFFER_VIEW {
                    BufferLocation: vertex_address
                        .checked_add(vertex_offset)
                        .ok_or("avatar vertex address overflow")?,
                    SizeInBytes: (batch.vertex_count * 40) as u32,
                    StrideInBytes: 40,
                },
                index: D3D12_INDEX_BUFFER_VIEW {
                    BufferLocation: index_address
                        .checked_add(index_offset)
                        .ok_or("avatar index address overflow")?,
                    SizeInBytes: (batch.index_count * 4) as u32,
                    Format: DXGI_FORMAT_R32_UINT,
                },
            });
        }
        list.Close().map_err(|e| e.to_string())?;
        let owners = UploadOwners {
            uploaded: Uploaded {
                frame,
                draws,
                vertices,
                indices,
                textures,
                descriptors,
                descriptor_step,
                glint_slot,
            },
            _upload: upload,
        };
        Ok(Some(Ticket {
            flight: permit.submit(device, queue, allocator, list, owners)?,
        }))
    }
}
