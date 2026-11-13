//! The borrowed FrameRenderObject argument of this exact world callback.
//! View/queue metadata and bounded entries: no visible-set or GPU proof.
use crate::native_scene_layout as scene;
use crate::world_hooks::read;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::native_scene_layout::{
    level_renderer::{APP_VTABLE as APP_TABLE, VTABLE as RENDER_TABLE},
    player_renderer::{OWNER_VTABLE as CAMERA_OWNER_TABLE, VTABLE as CAMERA_TABLE},
};
const VIEW_SIZE: usize = 0x3300;
const MAX_VIEWS: usize = 16;
const LIMIT: usize = 1_000_000;
static PROFILE: AtomicUsize = AtomicUsize::new(0);

fn word(bytes: &[u8], at: usize) -> Option<usize> {
    usize::try_from(u64::from_le_bytes(
        bytes.get(at..at.checked_add(8)?)?.try_into().ok()?,
    ))
    .ok()
}
fn pointer(bytes: &[u8], at: usize) -> Option<usize> {
    word(bytes, at).filter(|value| (0x10000..0x0000_8000_0000_0000).contains(value))
}
fn strong(control: usize) -> Option<()> {
    let mut bytes = [0; 4];
    read(control.checked_add(8)?, &mut bytes).ok()?;
    (i32::from_le_bytes(bytes) > 0).then_some(())
}
#[derive(PartialEq, Eq)]
struct Guard {
    address: usize,
    bytes: Vec<u8>,
}
#[derive(Default, PartialEq, Eq)]
struct Copies {
    guards: Vec<Guard>,
    bytes: usize,
    refs: Vec<usize>,
}
impl Copies {
    fn copy(&mut self, address: usize, length: usize) -> Option<Vec<u8>> {
        if length == 0
            || length > 128
            || self.guards.len() >= 64
            || self.bytes.checked_add(length)? > 8192
            || address < 0x10000
            || address.checked_add(length)? > 0x0000_8000_0000_0000
        {
            return None;
        }
        let mut bytes = vec![0; length];
        read(address, &mut bytes).ok()?;
        self.bytes += length;
        self.guards.push(Guard {
            address,
            bytes: bytes.clone(),
        });
        Some(bytes)
    }
    fn blob(&mut self, address: usize, length: usize) -> Option<Vec<u8>> {
        if length == 0 || length > 1280 {
            return None;
        }
        let mut bytes = Vec::with_capacity(length);
        for offset in (0..length).step_by(128) {
            bytes.extend_from_slice(
                &self.copy(address.checked_add(offset)?, 128.min(length - offset))?,
            );
        }
        Some(bytes)
    }
    fn live(&mut self, control: usize) -> Option<()> {
        if !(0x10000..0x0000_8000_0000_0000).contains(&control) || self.refs.len() >= 4 {
            return None;
        }
        strong(control)?;
        self.refs.push(control);
        Some(())
    }
    fn unchanged(&self) -> Option<()> {
        for &control in &self.refs {
            strong(control)?;
        }
        let mut bytes = [0; 128];
        for guard in &self.guards {
            read(guard.address, &mut bytes[..guard.bytes.len()]).ok()?;
            if bytes[..guard.bytes.len()] != guard.bytes[..] {
                return None;
            }
        }
        for &control in &self.refs {
            strong(control)?;
        }
        Some(())
    }
}
fn span(bytes: &[u8], at: usize, stride: usize, limit: usize) -> Option<usize> {
    let begin = word(bytes, at)?;
    let end = word(bytes, at.checked_add(8)?)?;
    let capacity = word(bytes, at.checked_add(16)?)?;
    if begin == 0 {
        return (end == 0 && capacity == 0).then_some(0);
    }
    for offset in [0, 8, 16] {
        pointer(bytes, at.checked_add(offset)?)?;
    }
    let length = end.checked_sub(begin)?;
    let allocated = capacity.checked_sub(begin)?;
    if !begin.is_multiple_of(8)
        || capacity < end
        || !length.is_multiple_of(stride)
        || !allocated.is_multiple_of(stride)
        || allocated / stride > LIMIT
        || length / stride > limit
    {
        return None;
    }
    Some(length / stride)
}
fn pose(bytes: &[u8]) -> Option<[u32; 6]> {
    let mut values = [0; 6];
    for (index, value) in values.iter_mut().enumerate() {
        *value = u32::from_le_bytes(bytes.get(index * 4..index * 4 + 4)?.try_into().ok()?);
        let number = f32::from_bits(*value);
        if !number.is_finite() || number.abs() > 32_000_000. {
            return None;
        }
    }
    Some(values)
}
#[derive(PartialEq, Eq)]
pub(crate) struct Sample {
    frame: usize,
    renderer: usize,
    client: usize,
    level: usize,
    epoch: u64,
    tick: u64,
    views: usize,
    shadows: usize,
    other_clients: usize,
    primary_index: usize,
    view_pose: [u32; 6],
    renderer_pose: [u32; 6],
    chunks: usize,
    layers: [usize; 51],
    layer_entries: usize,
    maximum_vertices: u32,
    payload: Option<crate::native_frame_chunks::Sample>,
    copies: Copies,
}
impl Sample {
    pub(crate) fn copy(
        renderer: usize,
        frame: usize,
        level: usize,
        epoch: u64,
        tick: u64,
    ) -> Option<Self> {
        let base = PROFILE.load(Ordering::Acquire);
        if base == 0
            || crate::world_hooks::image_base() != Some(base)
            || crate::players::session() != Some((level, epoch))
        {
            return None;
        }
        let client = crate::world_hooks::primary_client(level)?;
        let mut copies = Copies::default();
        if pointer(
            &copies.copy(
                client.instance.checked_add(scene::client::LEVEL_RENDERER)?,
                8,
            )?,
            0,
        )? != renderer
        {
            return None;
        }
        let tables = copies.copy(renderer, 16)?;
        if pointer(&tables, 0)? != base.checked_add(RENDER_TABLE)?
            || pointer(&tables, 8)? != base.checked_add(APP_TABLE)?
            || pointer(
                &copies.copy(renderer.checked_add(scene::level_renderer::LEVEL)?, 8)?,
                0,
            )? != level
            || pointer(
                &copies.copy(renderer.checked_add(scene::level_renderer::CLIENT)?, 8)?,
                0,
            )? != client.instance
        {
            return None;
        }
        let owner = copies.copy(
            renderer.checked_add(scene::level_renderer::PLAYER_OWNER)?,
            16,
        )?;
        let camera = pointer(&owner, 0)?;
        let control = pointer(&owner, 8)?;
        if control.checked_add(0x10)? != camera
            || pointer(&copies.copy(control, 8)?, 0)? != base.checked_add(CAMERA_OWNER_TABLE)?
            || pointer(&copies.copy(camera, 8)?, 0)? != base.checked_add(CAMERA_TABLE)?
        {
            return None;
        }
        copies.live(control)?;
        let camera_links = copies.copy(camera.checked_add(scene::player_renderer::LINKS)?, 0x30)?;
        if pointer(&camera_links, 0)? != level
            || pointer(
                &camera_links,
                scene::player_renderer::LEVEL_RENDERER - scene::player_renderer::LINKS,
            )? != renderer
        {
            return None;
        }
        let renderer_pose = pose(&copies.copy(
            camera.checked_add(scene::player_renderer::PREVIOUS_ORIGIN)?,
            24,
        )?)?;
        // LinearAllocator's shared pool precedes the three vector pointers.
        let header = copies.copy(frame, 40)?;
        pointer(&header, 0)?;
        copies.live(pointer(&header, 8)?)?;
        let views = span(&header, 16, VIEW_SIZE, MAX_VIEWS)?;
        let begin = pointer(&header, 16)?;
        let mut shadows = 0;
        let mut other_clients = 0;
        let mut primary = None;
        for index in 0..views {
            let view = begin.checked_add(index.checked_mul(VIEW_SIZE)?)?;
            let data = copies.blob(view, 0x90)?;
            if data[0x68] > 1 || data[0x7c..=0x88].iter().any(|flag| *flag > 1) {
                return None;
            }
            if data[0x8c] != 0 {
                other_clients += 1;
            } else if data[0x86] != 0 {
                shadows += 1;
            } else {
                if primary.is_some() {
                    return None;
                }
                primary = Some((index, view, pose(&data)?));
            }
        }
        let (primary_index, view, view_pose) = primary?;
        let collection_address = view.checked_add(0xe0)?;
        let collection = copies.blob(collection_address, 1280)?;
        let chunks = span(&collection, 0x18, 0x48, LIMIT)?;
        let mut layers = [0; 51];
        let mut layer_entries = 0usize;
        for (index, count) in layers.iter_mut().enumerate() {
            *count = span(&collection, 0x30 + index * 24, 0x28, LIMIT)?;
            layer_entries = layer_entries.checked_add(*count)?;
        }
        let maximum_vertices = u32::from_le_bytes(collection[0x4f8..0x4fc].try_into().ok()?);
        let payload = crate::native_frame_chunks::Sample::copy(collection_address, chunks, layers);
        let sample = Self {
            frame,
            renderer,
            client: client.instance,
            level,
            epoch,
            tick,
            views,
            shadows,
            other_clients,
            primary_index,
            view_pose,
            renderer_pose,
            chunks,
            layers,
            layer_entries,
            maximum_vertices,
            payload,
            copies,
        };
        sample.live().then_some(sample)
    }
    pub(crate) fn live(&self) -> bool {
        self.copies.unchanged().is_some()
            && crate::players::session() == Some((self.level, self.epoch))
            && crate::world_hooks::client_completed_tick(self.level) == Some(self.tick)
            && crate::world_hooks::primary_client(self.level)
                .is_some_and(|client| client.instance == self.client)
            && self.payload.as_ref().is_none_or(|payload| payload.live())
            && self.copies.unchanged().is_some()
    }
    pub(crate) fn receipt(&self) -> Value {
        json!({"observed":true,"worldArgumentObserved":true,"primaryViewMatched":self.view_pose==self.renderer_pose,
            "viewCount":self.views,"primaryViewCount":1,"primaryViewIndex":self.primary_index,
            "shadowViews":self.shadows,"otherClientViews":self.other_clients,
            "viewCamera":self.view_pose.map(f32::from_bits),"rendererCamera":self.renderer_pose.map(f32::from_bits),
            "chunkQueueCount":self.chunks,"layerQueueCounts":self.layers.as_slice(),"layerEntryCount":self.layer_entries,
            "maximumChunkVertexCount":self.maximum_vertices,"queueHeadersObserved":true,
            "queuePayloadObserved":self.payload.is_some(),"chunkPayload":self.payload.as_ref().map(|payload|payload.receipt()),
            "resourceOwnershipObserved":false,"projectionObserved":false,
            "visibleSetVerified":false,"terrainReady":false})
    }
}
pub(crate) fn install(base: usize) -> Result<(), String> {
    if crate::world_hooks::image_base() != Some(base) {
        return Err("unvalidated frame-input image".into());
    }
    let sites: &[(usize, &[u8])] = &[
        (
            0x47f85d4,
            &[0x4d, 0x89, 0xc6, 0x48, 0x89, 0xd6, 0x48, 0x89, 0xcb],
        ),
        (0x47f88f9, &[0x4c, 0x89, 0x7d, 0x70, 0x4c, 0x89, 0x75, 0x78]),
        (
            0x47aee69,
            &[
                0x48, 0x89, 0xce, 0x48, 0x8b, 0x79, 0x10, 0x48, 0x85, 0xff, 0x74, 0x3d, 0x48, 0x8d,
                0x5e, 0x10, 0x4c, 0x8b, 0x76, 0x18, 0x4c, 0x39, 0xf7, 0x74, 0x22,
            ],
        ),
        (
            0x47aee90,
            &[
                0x48, 0x89, 0xf9, 0xe8, 0xf8, 0x28, 0x05, 0, 0x48, 0x81, 0xc7, 0, 0x33, 0, 0, 0x4c,
                0x39, 0xf7, 0x75, 0xec,
            ],
        ),
        (
            0x4800fbb,
            &[0x48, 0x89, 0xce, 0x4c, 0x69, 0xfa, 0, 0x33, 0, 0],
        ),
        (
            0x480127d,
            &[0x0f, 0xb6, 0x82, 0x8c, 0, 0, 0, 0x88, 0x81, 0x8c, 0, 0, 0],
        ),
        (0x46b01d9, &[0xc6, 0x85, 0x56, 0x39, 0, 0, 0]),
        (
            0x480134c,
            &[
                0x48, 0x81, 0xc1, 0xe0, 0, 0, 0, 0x48, 0x81, 0xc2, 0xe0, 0, 0, 0, 0x48, 0x89, 0x4d,
                0xc8, 0xe8, 0xed, 0x9b, 0xae, 0, 0x48, 0x8d, 0x86, 0xe0, 0x05, 0, 0, 0x48, 0x89,
                0x45, 0xd0,
            ],
        ),
        (
            0x52eaf84,
            &[
                0x48, 0x8b, 0x42, 0x28, 0x48, 0xc7, 0x42, 0x28, 0, 0, 0, 0, 0x0f, 0x10, 0x42, 0x18,
                0x0f, 0x11, 0x72, 0x18, 0x0f, 0x11, 0x41, 0x18, 0x48, 0x89, 0x41, 0x28, 0x48, 0x8d,
                0x59, 0x30, 0x41, 0xb8, 0xc8, 0x04, 0, 0, 0x48, 0x89, 0xd9, 0x31, 0xd2, 0xe8, 0x2c,
                0xb5, 0x2b, 0x09,
            ],
        ),
        (
            0x52eafb4,
            &[0x8b, 0x87, 0xf8, 0x04, 0, 0, 0x89, 0x86, 0xf8, 0x04, 0, 0],
        ),
        (
            0x52eaed0,
            &[
                0x48, 0x89, 0xf9, 0xe8, 0xa8, 0x8d, 0x41, 0xff, 0x48, 0x83, 0xc7, 0x48,
            ],
        ),
        (
            0x468f98d,
            &[
                0x0f, 0x10, 0x4d, 0xe0, 0x0f, 0x11, 0x4a, 0x10, 0x0f, 0x11, 0x02, 0x48, 0x83, 0x41,
                0x08, 0x28,
            ],
        ),
        (
            0x46afec7,
            &[
                0xf3, 0x41, 0x0f, 0x10, 0x85, 0x54, 0x06, 0, 0, 0xf3, 0x0f, 0x11, 0x85, 0xd0, 0x38,
                0, 0, 0xf3, 0x41, 0x0f, 0x10, 0x85, 0x58, 0x06, 0, 0, 0xf3, 0x0f, 0x11, 0x85, 0xd4,
                0x38, 0, 0, 0xf3, 0x41, 0x0f, 0x10, 0x85, 0x5c, 0x06, 0, 0, 0xf3, 0x0f, 0x11, 0x85,
                0xd8, 0x38, 0, 0, 0xf3, 0x41, 0x0f, 0x10, 0x85, 0x60, 0x06, 0, 0, 0xf3, 0x0f, 0x11,
                0x85, 0xdc, 0x38, 0, 0, 0xf3, 0x0f, 0x11, 0x8d, 0xe0, 0x38, 0, 0, 0xf3, 0x41, 0x0f,
                0x10, 0x85, 0x68, 0x06, 0, 0, 0xf3, 0x0f, 0x11, 0x85, 0xe4, 0x38, 0, 0,
            ],
        ),
    ];
    for &(rva, expected) in sites {
        let mut actual = vec![0; expected.len()];
        read(
            base.checked_add(rva)
                .ok_or("frame-input address overflow")?,
            &mut actual,
        )?;
        if actual != expected {
            return Err(format!("frame-input profile differs at {rva:x}"));
        }
    }
    crate::native_frame_chunks::install(base)?;
    PROFILE.store(base, Ordering::Release);
    Ok(())
}
