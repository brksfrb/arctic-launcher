//! Owners obtained from successful public CreateCommittedResource results.
//! A COM reference keeps committed storage alive; it does not freeze its contents,
//! reserve an engine phase or own a placed-resource allocator's suballocation.
use hudhook::windows::{
    Win32::Graphics::Direct3D12::*,
    core::{IUnknown, Interface},
};
use serde_json::{Value, json};
use std::{
    ffi::c_void,
    sync::{Arc, Mutex, OnceLock},
};

const MAX_TARGETS: usize = 16;
const MAX_BUFFERS: usize = 64;
const MAX_BYTES: u64 = 256 * 1024 * 1024;
const MAX_BUFFER_SPANS: usize = 66; // 64 root arguments plus vertex/index slot zero.
#[derive(Clone, Copy)]
struct Buffer {
    address: u64,
    bytes: u64,
}
/// A root descriptor supplies a point; IA views supply their complete byte span.
/// This range proves allocation coverage, not shader read bounds/content validity.
pub(crate) struct BufferSpan {
    pub address: u64,
    pub bytes: u64,
}
pub(crate) struct Owner {
    _resource: ID3D12Resource,
    _device: ID3D12Device,
    identity: usize,
    device: usize,
    desc: D3D12_RESOURCE_DESC,
    bytes: u64,
    buffer: Option<Buffer>,
}
impl Owner {
    pub(crate) fn allocation_reference(&self) -> (usize, u64) {
        (self.identity, self.bytes)
    }
}
#[derive(Default)]
struct Pool {
    owners: Vec<Arc<Owner>>,
    bytes: u64,
    observed: u64,
    evicted: u64,
}
#[derive(Default)]
struct Cache {
    targets: Pool,
    buffers: Pool,
}
static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
fn cache() -> &'static Mutex<Cache> {
    CACHE.get_or_init(Mutex::default)
}
pub(crate) fn enabled() -> bool {
    crate::native_body::enabled() || crate::native_clock::probe_enabled()
}
pub(crate) fn collect() {
    if !enabled() {
        clear();
    }
}
fn clear() {
    let removed = if let Ok(mut cache) = cache().lock() {
        cache.targets.bytes = 0;
        cache.buffers.bytes = 0;
        (
            std::mem::take(&mut cache.targets.owners),
            std::mem::take(&mut cache.buffers.owners),
        )
    } else {
        (Vec::new(), Vec::new())
    };
    // Driver Release callbacks must never run under this cache's mutex.
    drop(removed);
}

/// Only a successful actual public API output is accepted. Never call this
/// with a copied registry pointer or an allocator helper's return value.
/// Caller excludes Arctic's private commands and contains unwinding.
pub(crate) unsafe fn created(device: *mut c_void, result: *mut c_void) {
    if !enabled() {
        clear();
        return;
    }
    let Some(actual_device) = (unsafe { ID3D12Device::from_raw_borrowed(&device) }) else {
        return;
    };
    let Some(object) = (unsafe { IUnknown::from_raw_borrowed(&result) }) else {
        return;
    };
    // The requested output may be any supported resource interface. Query the
    // real returned COM object rather than assuming that interface's layout.
    let Ok(resource) = object.cast::<ID3D12Resource>() else {
        return;
    };
    let Ok(identity) = resource.cast::<IUnknown>() else {
        return;
    };
    let mut owner: Option<ID3D12Device> = None;
    if unsafe { resource.GetDevice(&mut owner) }.is_err() {
        return;
    }
    let Some(device) = owner else {
        return;
    };
    let Ok(device_id) = device.cast::<IUnknown>() else {
        return;
    };
    let Ok(actual_device_id) = actual_device.cast::<IUnknown>() else {
        return;
    };
    if device_id.as_raw() != actual_device_id.as_raw() {
        return;
    }
    let desc = unsafe { resource.GetDesc() };
    let mut properties = D3D12_HEAP_PROPERTIES::default();
    if unsafe { resource.GetHeapProperties(Some(&mut properties), None) }.is_err() {
        return;
    }
    let buffer = if desc.Dimension == D3D12_RESOURCE_DIMENSION_BUFFER {
        if desc.Width == 0
            || desc.Width > MAX_BYTES
            || desc.Height != 1
            || desc.DepthOrArraySize != 1
            || desc.MipLevels != 1
            || desc.Layout != D3D12_TEXTURE_LAYOUT_ROW_MAJOR
            || desc.SampleDesc.Count != 1
            || desc.SampleDesc.Quality != 0
            || properties.Type == D3D12_HEAP_TYPE_READBACK
        {
            return;
        }
        let address = unsafe { resource.GetGPUVirtualAddress() };
        if address == 0 || address.checked_add(desc.Width).is_none() {
            return;
        }
        Some(Buffer {
            address,
            bytes: desc.Width,
        })
    } else {
        if desc.Dimension != D3D12_RESOURCE_DIMENSION_TEXTURE2D
            || desc.DepthOrArraySize != 1
            || desc.MipLevels != 1
            || desc.Width == 0
            || desc.Width > 16384
            || desc.Height == 0
            || desc.Height > 16384
            || desc.SampleDesc.Count == 0
            || desc.Flags.0
                & (D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET.0
                    | D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL.0)
                == 0
        {
            return;
        }
        None
    };
    let info =
        unsafe { actual_device.GetResourceAllocationInfo(properties.CreationNodeMask, &[desc]) };
    if info.SizeInBytes == 0 || info.SizeInBytes > MAX_BYTES {
        return;
    }
    let receipt = Arc::new(Owner {
        _resource: resource,
        _device: device,
        identity: identity.as_raw() as usize,
        device: device_id.as_raw() as usize,
        desc,
        bytes: info.SizeInBytes,
        buffer,
    });
    let mut removed = Vec::new();
    if let Ok(mut cache) = cache().lock() {
        let (pool, maximum) = if buffer.is_some() {
            (&mut cache.buffers, MAX_BUFFERS)
        } else {
            (&mut cache.targets, MAX_TARGETS)
        };
        if let Some(index) = pool
            .owners
            .iter()
            .position(|old| old.identity == receipt.identity)
        {
            let old = pool.owners.remove(index);
            pool.bytes -= old.bytes;
            removed.push(old);
        }
        while pool.owners.len() >= maximum || pool.bytes > MAX_BYTES - receipt.bytes {
            let old = pool.owners.remove(0);
            pool.bytes -= old.bytes;
            pool.evicted = pool.evicted.saturating_add(1);
            removed.push(old);
        }
        pool.bytes += receipt.bytes;
        pool.observed = pool.observed.saturating_add(1);
        pool.owners.push(receipt);
    }
    // Eviction removes discovery ownership only. Bound frames/draw assets keep
    // their own Arcs; a held actual COM identity cannot be recycled by a cache miss.
    drop(removed);
}

pub(crate) fn matching(resource: &ID3D12Resource) -> Option<Arc<Owner>> {
    matching_in(resource, false)
}
pub(crate) fn matching_any(resource: &ID3D12Resource) -> Option<Arc<Owner>> {
    matching_in(resource, true)
}
fn matching_in(resource: &ID3D12Resource, include_buffers: bool) -> Option<Arc<Owner>> {
    if !enabled() {
        return None;
    }
    let identity = resource.cast::<IUnknown>().ok()?;
    let mut device: Option<ID3D12Device> = None;
    unsafe { resource.GetDevice(&mut device) }.ok()?;
    let device = device?.cast::<IUnknown>().ok()?;
    let desc = unsafe { resource.GetDesc() };
    let cache = cache().lock().ok()?;
    // Compare already canonicalized numeric identities under the mutex. The
    // windows IUnknown PartialEq implementation may QueryInterface on mismatch.
    let buffers = if include_buffers {
        cache.buffers.owners.as_slice()
    } else {
        &[]
    };
    cache
        .targets
        .owners
        .iter()
        .chain(buffers)
        .find(|owner| {
            owner.identity == identity.as_raw() as usize
                && owner.device == device.as_raw() as usize
                && owner.desc == desc
        })
        .cloned()
}
/// Capture the whole known address cohort under one cache lock. A missing,
/// partially overlapping or ambiguous allocation leaves the cohort unknown.
/// Numeric GPU addresses are never reinterpreted as CPU/COM pointers.
pub(crate) fn buffers(device: &ID3D12Device, spans: &[BufferSpan]) -> Option<Vec<Arc<Owner>>> {
    if !enabled() || spans.len() > MAX_BUFFER_SPANS {
        return None;
    }
    // The captured list shadow carries no authoritative linked-node mask yet.
    // Device identity and a numeric address alone must not infer that context.
    if unsafe { device.GetNodeCount() } != 1 {
        return None;
    }
    let identity = device.cast::<IUnknown>().ok()?;
    let device = identity.as_raw() as usize;
    let cache = cache().lock().ok()?;
    let mut owners = Vec::new();
    for span in spans {
        if span.address == 0 || span.bytes == 0 {
            return None;
        }
        let end = span.address.checked_add(span.bytes)?;
        let mut selected = None;
        for owner in &cache.buffers.owners {
            if owner.device != device {
                continue;
            }
            let buffer = owner.buffer?;
            let buffer_end = buffer.address.checked_add(buffer.bytes)?;
            if buffer.address < end && span.address < buffer_end {
                if selected.is_some() || span.address < buffer.address || end > buffer_end {
                    return None;
                }
                selected = Some(owner);
            }
        }
        let owner = selected?;
        if !owners.iter().any(|known| Arc::ptr_eq(known, owner)) {
            owners.push(owner.clone());
        }
    }
    Some(owners)
}
pub(crate) fn status() -> Value {
    let Ok(cache) = cache().lock() else {
        return json!({"available":false});
    };
    json!({"available":true,"retained":cache.targets.owners.len(),"retainedBytes":cache.targets.bytes,
        "observed":cache.targets.observed,"evicted":cache.targets.evicted,"maximumResources":MAX_TARGETS,"maximumBytes":MAX_BYTES,
        "buffers":{"retained":cache.buffers.owners.len(),"retainedBytes":cache.buffers.bytes,
            "observed":cache.buffers.observed,"evicted":cache.buffers.evicted,"maximumResources":MAX_BUFFERS,"maximumBytes":MAX_BYTES},
        "placedAllocationOwnerImplemented":false,"exclusiveSceneLeaseImplemented":false,"runtimeVerified":false})
}
