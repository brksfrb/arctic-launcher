//! Public descriptor-write provenance and retained referenced objects.
//! This does not freeze heap contents or reserve an engine recording phase.
use crate::native_graphics_state::DescriptorSpan;
use hudhook::windows::{
    Win32::Graphics::Direct3D12::*,
    core::{IUnknown, Interface},
};
use serde_json::{Value as Json, json};
use std::{
    collections::{HashMap, HashSet},
    ffi::c_void,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

const MAX_HEAPS: usize = 32;
const MAX_HEAP_BYTES: u64 = 128 * 1024 * 1024;
const MAX_ENTRIES: usize = 4096;
const MAX_SPANS: usize = 256;
const MAX_INFLIGHT: u64 = 512;
const MAX_REFERENCED_RESOURCES: usize = 128;
const MAX_REFERENCED_BYTES: u64 = 256 * 1024 * 1024;
struct Heap {
    id: u64,
    identity: usize,
    device: usize,
    _object: ID3D12DescriptorHeap,
    _device: ID3D12Device,
    kind: D3D12_DESCRIPTOR_HEAP_TYPE,
    count: u32,
    step: u32,
    cpu: usize,
    gpu: u64,
    bytes: u64,
    visible: bool,
}
struct Contents {
    kind: D3D12_DESCRIPTOR_RANGE_TYPE,
    _resources: Vec<ID3D12Resource>,
    _allocations: Vec<Arc<crate::native_committed_resources::Owner>>,
    allocations_known: bool,
    references: Vec<(usize, u64)>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Key {
    heap: u64,
    index: u32,
}
#[derive(Default)]
struct Entry {
    revision: u64,
    active: Vec<u64>,
    conflicted: bool,
    value: Option<Arc<Contents>>,
}
#[derive(Default)]
struct State {
    serial: u64,
    faulted: bool,
    inflight: u64,
    blocked: bool,
    heaps: Vec<Arc<Heap>>,
    bytes: u64,
    entries: HashMap<Key, Entry>,
    writes: u64,
    conflicts: u64,
    invalidations: u64,
}
#[derive(Default)]
struct Retired {
    entries: Vec<Entry>,
    maps: Vec<HashMap<Key, Entry>>,
    heaps: Vec<Arc<Heap>>,
    values: Vec<Arc<Contents>>,
}
static STATE: OnceLock<Mutex<State>> = OnceLock::new();
static DEVICE: OnceLock<(ID3D12Device, usize)> = OnceLock::new();
static LIVE: AtomicBool = AtomicBool::new(false);
fn state() -> &'static Mutex<State> {
    STATE.get_or_init(Mutex::default)
}
fn enabled() -> bool {
    LIVE.load(Ordering::Acquire) && crate::native_committed_resources::enabled()
}
pub(crate) fn observing() -> bool {
    enabled()
}
pub(crate) fn prepare(device: &ID3D12Device) -> Result<(), String> {
    let identity = device
        .cast::<IUnknown>()
        .map_err(|error| error.to_string())?;
    DEVICE
        .set((device.clone(), identity.as_raw() as usize))
        .map_err(|_| "descriptor device already published".into())
}
pub(crate) fn activate() {
    LIVE.store(true, Ordering::Release);
}
fn admitted(device: &ID3D12Device) -> bool {
    enabled()
        && DEVICE.get().is_some_and(|(_, identity)| {
            device
                .cast::<IUnknown>()
                .ok()
                .is_some_and(|actual| actual.as_raw() as usize == *identity)
        })
}
fn heap_kind(kind: D3D12_DESCRIPTOR_RANGE_TYPE) -> Option<D3D12_DESCRIPTOR_HEAP_TYPE> {
    match kind {
        D3D12_DESCRIPTOR_RANGE_TYPE_CBV
        | D3D12_DESCRIPTOR_RANGE_TYPE_SRV
        | D3D12_DESCRIPTOR_RANGE_TYPE_UAV => Some(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV),
        D3D12_DESCRIPTOR_RANGE_TYPE_SAMPLER => Some(D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER),
        _ => None,
    }
}
unsafe fn device(raw: *mut c_void) -> Option<ID3D12Device> {
    let device = unsafe { IUnknown::from_raw_borrowed(&raw) }?
        .cast::<ID3D12Device>()
        .ok()?;
    admitted(&device).then_some(device)
}
impl State {
    fn resource_budget(&self, incoming: &[Option<Arc<Contents>>]) -> bool {
        let mut resources = HashMap::new();
        let mut total = 0u64;
        for value in self
            .entries
            .values()
            .filter_map(|entry| entry.value.as_ref())
            .chain(incoming.iter().filter_map(Option::as_ref))
        {
            for &(identity, bytes) in &value.references {
                let previous = resources.get(&identity).copied().unwrap_or(0);
                if bytes > previous {
                    let Some(next) = total.checked_add(bytes - previous) else {
                        return false;
                    };
                    total = next;
                    resources.insert(identity, bytes);
                    if resources.len() > MAX_REFERENCED_RESOURCES || total > MAX_REFERENCED_BYTES {
                        return false;
                    }
                }
            }
        }
        true
    }
    fn next(&mut self) -> Option<u64> {
        if self.faulted {
            return None;
        }
        self.serial = match self.serial.checked_add(1) {
            Some(value) => value,
            None => {
                self.faulted = true;
                return None;
            }
        };
        Some(self.serial)
    }
    fn invalidate(&mut self, retired: &mut Retired) {
        if !self.entries.is_empty() {
            retired.maps.push(std::mem::take(&mut self.entries));
        }
        self.blocked |= self.inflight != 0;
        self.invalidations = self.invalidations.saturating_add(1);
    }
    fn forget_heap(&mut self, index: usize, retired: &mut Retired) {
        let heap = self.heaps.remove(index);
        self.bytes -= heap.bytes;
        let keys: Vec<_> = self
            .entries
            .keys()
            .filter(|key| key.heap == heap.id)
            .copied()
            .collect();
        for key in keys {
            if let Some(entry) = self.entries.remove(&key) {
                retired.entries.push(entry);
            }
        }
        retired.heaps.push(heap);
    }
    fn locate(
        &self,
        device: usize,
        kind: D3D12_DESCRIPTOR_HEAP_TYPE,
        handle: usize,
    ) -> Option<(Key, Arc<Heap>)> {
        let mut found = None;
        for heap in &self.heaps {
            if heap.device != device || heap.kind != kind {
                continue;
            }
            let Some(at) = handle.checked_sub(heap.cpu) else {
                continue;
            };
            if u64::try_from(at).ok()? >= heap.bytes {
                continue;
            }
            if !at.is_multiple_of(heap.step as usize) || found.is_some() {
                return None;
            }
            found = Some((
                Key {
                    heap: heap.id,
                    index: u32::try_from(at / (heap.step as usize)).ok()?,
                },
                heap.clone(),
            ));
        }
        found
    }
}

/// Actual successful CreateDescriptorHeap output only; no pointer lookup.
pub(crate) unsafe fn created_heap(raw_device: *mut c_void, out: *mut c_void) {
    if !enabled() {
        collect();
        return;
    }
    let Some(device) = (unsafe { device(raw_device) }) else {
        return;
    };
    let Some(object) = (unsafe { IUnknown::from_raw_borrowed(&out) }) else {
        return;
    };
    let Ok(heap) = object.cast::<ID3D12DescriptorHeap>() else {
        return;
    };
    let Ok(identity) = heap.cast::<IUnknown>() else {
        return;
    };
    let Ok(device_id) = device.cast::<IUnknown>() else {
        return;
    };
    let mut owner: Option<ID3D12Device> = None;
    if unsafe { heap.GetDevice(&mut owner) }.is_err() {
        return;
    }
    let Some(owner) = owner.and_then(|owner| owner.cast::<IUnknown>().ok()) else {
        return;
    };
    if owner.as_raw() != device_id.as_raw() {
        return;
    }
    let desc = unsafe { heap.GetDesc() };
    if !matches!(
        desc.Type,
        D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV | D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER
    ) || desc.NumDescriptors == 0
    {
        return;
    }
    let step = unsafe { device.GetDescriptorHandleIncrementSize(desc.Type) };
    if step == 0 {
        return;
    }
    let Some(bytes) = u64::from(desc.NumDescriptors)
        .checked_mul(u64::from(step))
        .filter(|bytes| *bytes <= MAX_HEAP_BYTES)
    else {
        return;
    };
    let cpu = unsafe { heap.GetCPUDescriptorHandleForHeapStart() }.ptr;
    if cpu == 0
        || usize::try_from(bytes)
            .ok()
            .and_then(|bytes| cpu.checked_add(bytes))
            .is_none()
    {
        return;
    }
    let visible = desc.Flags.0 & D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE.0 != 0;
    let gpu = if visible {
        unsafe { heap.GetGPUDescriptorHandleForHeapStart() }.ptr
    } else {
        0
    };
    if visible && (gpu == 0 || gpu.checked_add(bytes).is_none()) {
        return;
    }
    let mut heap = Heap {
        id: 0,
        identity: identity.as_raw() as usize,
        device: device_id.as_raw() as usize,
        _object: heap,
        _device: device,
        kind: desc.Type,
        count: desc.NumDescriptors,
        step,
        cpu,
        gpu,
        bytes,
        visible,
    };
    let mut retired = Retired::default();
    if let Ok(mut state) = state().lock()
        && let Some(id) = state.next()
    {
        heap.id = id;
        if let Some(index) = state
            .heaps
            .iter()
            .position(|old| old.identity == heap.identity)
        {
            state.forget_heap(index, &mut retired);
        }
        while state.heaps.len() >= MAX_HEAPS || state.bytes > MAX_HEAP_BYTES - bytes {
            state.forget_heap(0, &mut retired);
        }
        state.bytes += bytes;
        state.heaps.push(Arc::new(heap));
    }
    drop(retired);
}

pub(crate) struct Write {
    token: u64,
    keys: Vec<Key>,
    values: Vec<Option<Arc<Contents>>>,
    sources: Vec<(Key, u64)>,
    _heaps: Vec<Arc<Heap>>,
    finished: bool,
}
impl Write {
    fn complete(&mut self, publish: bool) {
        self.finished = true;
        let mut retired = Retired::default();
        if let Ok(mut state) = state().lock() {
            if !publish {
                state.invalidate(&mut retired);
            }
            if publish && !state.resource_budget(&self.values) {
                state.invalidate(&mut retired);
            }
            let known = publish
                && !state.faulted
                && !state.blocked
                && self.sources.iter().all(|(key, revision)| {
                    state.entries.get(key).is_some_and(|entry| {
                        entry.revision == *revision
                            && entry.active.is_empty()
                            && entry.value.is_some()
                    })
                });
            for (key, value) in self.keys.iter().zip(&self.values) {
                let Some(entry) = state.entries.get_mut(key) else {
                    continue;
                };
                let Some(index) = entry.active.iter().position(|token| *token == self.token) else {
                    continue;
                };
                entry.active.swap_remove(index);
                if known
                    && entry.active.is_empty()
                    && !entry.conflicted
                    && entry.revision == self.token
                {
                    entry.value = value.clone();
                }
            }
            state.inflight = state.inflight.saturating_sub(1);
            if state.inflight == 0 {
                state.blocked = false;
            }
        }
        drop(retired);
    }
    pub(crate) fn finish(mut self) {
        self.complete(true);
    }
}
impl Drop for Write {
    fn drop(&mut self) {
        if !self.finished {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.complete(false)));
        }
    }
}
/// Keep an observation failure unknown through the actual driver call, including
/// overlap with later writers. A query failure must not leave old slots known.
pub(crate) fn uncertain_write() -> Option<Write> {
    if !enabled() {
        collect();
        return None;
    }
    let mut retired = Retired::default();
    let mut write = Write {
        token: 0,
        keys: Vec::new(),
        values: Vec::new(),
        sources: Vec::new(),
        _heaps: Vec::new(),
        finished: true,
    };
    {
        let mut state = state().lock().ok()?;
        state.inflight = match state.inflight.checked_add(1) {
            Some(value) => value,
            None => {
                state.faulted = true;
                return None;
            }
        };
        write.token = state.next().unwrap_or(0);
        write.finished = false;
        state.invalidate(&mut retired);
    }
    drop(retired);
    Some(write)
}
fn begin(
    device: &ID3D12Device,
    kind: D3D12_DESCRIPTOR_HEAP_TYPE,
    dst: Option<Vec<usize>>,
    src: Option<Vec<usize>>,
    value: Option<Arc<Contents>>,
    copy: bool,
) -> Option<Write> {
    if !enabled() {
        collect();
        return None;
    }
    if !admitted(device) {
        return None;
    }
    let identity = device.cast::<IUnknown>().ok()?;
    let identity = identity.as_raw() as usize;
    let mut retired = Retired::default();
    // Declare the guard before locking. Unwinding must unlock State before its
    // Drop can abandon an admitted write and acquire that same mutex.
    let mut write = Write {
        token: 0,
        keys: Vec::new(),
        values: Vec::new(),
        sources: Vec::new(),
        _heaps: Vec::new(),
        finished: true,
    };
    let result = (|| {
        let mut state = state().lock().ok()?;
        state.inflight = match state.inflight.checked_add(1) {
            Some(value) => value,
            None => {
                state.faulted = true;
                return None;
            }
        };
        let token = state.next().unwrap_or(0);
        write.token = token;
        write.finished = false;
        if state.faulted || state.inflight > MAX_INFLIGHT {
            state.invalidate(&mut retired);
            return Some(());
        }
        let Some(dst) =
            dst.filter(|dst| dst.len() <= MAX_SPANS && dst.iter().all(|handle| *handle != 0))
        else {
            state.invalidate(&mut retired);
            return Some(());
        };
        let destinations: Option<Vec<_>> = dst
            .iter()
            .map(|&handle| state.locate(identity, kind, handle))
            .collect();
        let Some(destinations) = destinations else {
            state.invalidate(&mut retired);
            return Some(());
        };
        let unique: HashSet<_> = destinations.iter().map(|(key, _)| *key).collect();
        if unique.len() != destinations.len() {
            state.invalidate(&mut retired);
            return Some(());
        }
        if copy {
            let Some(src) = src.filter(|src| src.len() == dst.len() && src.len() <= MAX_SPANS)
            else {
                state.invalidate(&mut retired);
                return Some(());
            };
            for handle in src {
                let Some((key, heap)) = state.locate(identity, kind, handle) else {
                    state.invalidate(&mut retired);
                    return Some(());
                };
                // CopyDescriptors sources must not be shader visible or overlap destinations.
                if heap.visible || unique.contains(&key) {
                    state.invalidate(&mut retired);
                    return Some(());
                }
                let value = state.entries.get(&key).and_then(|entry| {
                    if !entry.active.is_empty() {
                        return None;
                    }
                    let value = entry.value.clone()?;
                    write.sources.push((key, entry.revision));
                    Some(value)
                });
                write.values.push(value);
                write._heaps.push(heap);
            }
        } else {
            write.values = vec![value.clone(); destinations.len()];
        }
        if state
            .entries
            .len()
            .checked_add(destinations.len())
            .is_none_or(|count| count > MAX_ENTRIES)
        {
            state.invalidate(&mut retired);
        }
        for (key, heap) in destinations {
            let blocked = state.blocked;
            let conflicted = {
                let entry = state.entries.entry(key).or_default();
                if let Some(value) = entry.value.take() {
                    retired.values.push(value);
                }
                entry.conflicted = !entry.active.is_empty();
                if !blocked {
                    entry.active.push(token);
                }
                entry.revision = token;
                entry.conflicted
            };
            if conflicted {
                state.conflicts = state.conflicts.saturating_add(1);
            }
            write.keys.push(key);
            write._heaps.push(heap);
        }
        state.writes = state.writes.saturating_add(1);
        Some(())
    })();
    drop(retired);
    result.map(|_| write)
}
fn resources(
    device: &ID3D12Device,
    kind: D3D12_DESCRIPTOR_RANGE_TYPE,
    args: &[*mut c_void],
    valid: bool,
) -> Option<Arc<Contents>> {
    if !valid {
        return None;
    }
    let device_id = device.cast::<IUnknown>().ok()?;
    let mut resources = Vec::new();
    let mut allocations = Vec::new();
    let mut references = Vec::new();
    let mut known = true;
    for raw in args {
        if raw.is_null() {
            continue;
        }
        let object = unsafe { IUnknown::from_raw_borrowed(raw) }?;
        let resource = object.cast::<ID3D12Resource>().ok()?;
        let mut owner: Option<ID3D12Device> = None;
        unsafe { resource.GetDevice(&mut owner) }.ok()?;
        if owner?.cast::<IUnknown>().ok()?.as_raw() != device_id.as_raw() {
            return None;
        }
        let identity = resource.cast::<IUnknown>().ok()?;
        let mut properties = D3D12_HEAP_PROPERTIES::default();
        unsafe { resource.GetHeapProperties(Some(&mut properties), None) }.ok()?;
        let info = unsafe {
            device.GetResourceAllocationInfo(properties.CreationNodeMask, &[resource.GetDesc()])
        };
        if info.SizeInBytes == 0 || info.SizeInBytes > MAX_REFERENCED_BYTES {
            return None;
        }
        references.push((identity.as_raw() as usize, info.SizeInBytes));
        if let Some(owner) = crate::native_committed_resources::matching_any(&resource) {
            allocations.push(owner);
        } else {
            known = false;
        }
        resources.push(resource);
    }
    Some(Arc::new(Contents {
        kind,
        _resources: resources,
        _allocations: allocations,
        allocations_known: known,
        references,
    }))
}
pub(crate) unsafe fn srv(
    raw: *mut c_void,
    resource: *mut c_void,
    desc: *const D3D12_SHADER_RESOURCE_VIEW_DESC,
    handle: usize,
) -> Option<Write> {
    let device = unsafe { device(raw) }?;
    let valid = if desc.is_null() {
        !resource.is_null()
    } else {
        matches!(unsafe { (*desc).ViewDimension }.0, 1..=10)
    };
    let value = resources(&device, D3D12_DESCRIPTOR_RANGE_TYPE_SRV, &[resource], valid);
    begin(
        &device,
        D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
        Some(vec![handle]),
        None,
        value,
        false,
    )
}
pub(crate) unsafe fn uav(
    raw: *mut c_void,
    resource: *mut c_void,
    counter: *mut c_void,
    desc: *const D3D12_UNORDERED_ACCESS_VIEW_DESC,
    handle: usize,
) -> Option<Write> {
    let device = unsafe { device(raw) }?;
    let valid = if desc.is_null() {
        !resource.is_null()
    } else {
        matches!(unsafe { (*desc).ViewDimension }.0, 1..=5 | 8)
    };
    let value = resources(
        &device,
        D3D12_DESCRIPTOR_RANGE_TYPE_UAV,
        &[resource, counter],
        valid && (!resource.is_null() || counter.is_null()),
    );
    begin(
        &device,
        D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
        Some(vec![handle]),
        None,
        value,
        false,
    )
}
pub(crate) unsafe fn feedback(
    raw: *mut c_void,
    target: *mut c_void,
    feedback: *mut c_void,
    handle: usize,
) -> Option<Write> {
    let device = unsafe { device(raw) }?;
    let value = resources(
        &device,
        D3D12_DESCRIPTOR_RANGE_TYPE_UAV,
        &[target, feedback],
        !target.is_null() && !feedback.is_null(),
    );
    begin(
        &device,
        D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
        Some(vec![handle]),
        None,
        value,
        false,
    )
}
pub(crate) unsafe fn cbv(
    raw: *mut c_void,
    desc: *const D3D12_CONSTANT_BUFFER_VIEW_DESC,
    handle: usize,
) -> Option<Write> {
    let device = unsafe { device(raw) }?;
    let value = (|| {
        let desc = unsafe { desc.as_ref() }?;
        if desc.SizeInBytes == 0
            || !desc.SizeInBytes.is_multiple_of(256)
            || !desc.BufferLocation.is_multiple_of(256)
        {
            return None;
        }
        let owners = if desc.BufferLocation == 0 {
            Some(Vec::new())
        } else {
            crate::native_committed_resources::buffers(
                &device,
                &[crate::native_committed_resources::BufferSpan {
                    address: desc.BufferLocation,
                    bytes: u64::from(desc.SizeInBytes),
                }],
            )
        };
        let known = owners.is_some();
        let owners = owners.unwrap_or_default();
        let references = owners
            .iter()
            .map(|owner| owner.allocation_reference())
            .collect();
        Some(Arc::new(Contents {
            kind: D3D12_DESCRIPTOR_RANGE_TYPE_CBV,
            _resources: Vec::new(),
            _allocations: owners,
            allocations_known: known,
            references,
        }))
    })();
    begin(
        &device,
        D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
        Some(vec![handle]),
        None,
        value,
        false,
    )
}
pub(crate) unsafe fn sampler(raw: *mut c_void, valid: bool, handle: usize) -> Option<Write> {
    let device = unsafe { device(raw) }?;
    let value = resources(&device, D3D12_DESCRIPTOR_RANGE_TYPE_SAMPLER, &[], valid);
    begin(
        &device,
        D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER,
        Some(vec![handle]),
        None,
        value,
        false,
    )
}
pub(crate) fn copy(
    device: &ID3D12Device,
    kind: D3D12_DESCRIPTOR_HEAP_TYPE,
    dst: Option<Vec<usize>>,
    src: Option<Vec<usize>>,
) -> Option<Write> {
    begin(device, kind, dst, src, None, true)
}

#[derive(Clone)]
pub(crate) struct Snapshot {
    entries: Vec<(Key, u64, Arc<Contents>)>,
    _heaps: Vec<Arc<Heap>>,
    allocations_known: bool,
}
impl Snapshot {
    pub(crate) fn stable(&self) -> bool {
        let Ok(state) = state().lock() else {
            return false;
        };
        !state.faulted
            && !state.blocked
            && self.entries.iter().all(|(key, revision, value)| {
                state.entries.get(key).is_some_and(|entry| {
                    entry.revision == *revision
                        && entry.active.is_empty()
                        && entry
                            .value
                            .as_ref()
                            .is_some_and(|current| Arc::ptr_eq(current, value))
                })
            })
    }
    pub(crate) fn allocations_known(&self) -> bool {
        self.allocations_known
    }
}
pub(crate) fn capture(spans: &[DescriptorSpan]) -> Option<Snapshot> {
    if !enabled() || spans.len() > MAX_SPANS {
        return None;
    }
    let mut inputs = Vec::new();
    let mut total = 0usize;
    for span in spans {
        total = total.checked_add(span.count as usize)?;
        if total > MAX_SPANS {
            return None;
        }
        let identity = span.heap.cast::<IUnknown>().ok()?;
        inputs.push((span, identity.as_raw() as usize));
    }
    let state = state().lock().ok()?;
    if state.faulted || state.blocked {
        return None;
    }
    let mut result = Snapshot {
        entries: Vec::new(),
        _heaps: Vec::new(),
        allocations_known: true,
    };
    let mut seen = HashSet::new();
    for (span, identity) in inputs {
        let heap = state.heaps.iter().find(|heap| heap.identity == identity)?;
        if !heap.visible
            || heap_kind(span.kind) != Some(heap.kind)
            || span.increment != heap.step
            || span.first.checked_add(span.count)? > heap.count
            || heap
                .cpu
                .checked_add((span.first as usize).checked_mul(heap.step as usize)?)?
                != span.cpu
            || heap
                .gpu
                .checked_add(u64::from(span.first).checked_mul(u64::from(heap.step))?)?
                != span.gpu
        {
            return None;
        }
        result._heaps.push(heap.clone());
        for index in span.first..span.first.checked_add(span.count)? {
            let key = Key {
                heap: heap.id,
                index,
            };
            let entry = state.entries.get(&key)?;
            if !entry.active.is_empty() {
                return None;
            }
            let value = entry.value.as_ref()?;
            if value.kind != span.kind {
                return None;
            }
            result.allocations_known &= value.allocations_known;
            if seen.insert(key) {
                result.entries.push((key, entry.revision, value.clone()));
            }
        }
    }
    Some(result)
}
pub(crate) fn collect() {
    if enabled() {
        return;
    }
    let mut retired = Retired::default();
    if let Ok(mut state) = state().lock()
        && (!state.entries.is_empty() || !state.heaps.is_empty() || state.inflight != 0)
    {
        state.invalidate(&mut retired);
        retired.heaps = std::mem::take(&mut state.heaps);
        state.bytes = 0;
    }
    drop(retired);
}
pub(crate) fn status() -> Json {
    let Ok(state) = state().lock() else {
        return json!({"available":false});
    };
    json!({"available":true,"hooksLive":LIVE.load(Ordering::Relaxed),"heaps":state.heaps.len(),"heapDescriptorBytes":state.bytes,"slots":state.entries.len(),
        "inflight":state.inflight,"blocked":state.blocked,"faulted":state.faulted,"writes":state.writes,
        "conflicts":state.conflicts,"invalidations":state.invalidations,"maximumHeaps":MAX_HEAPS,"maximumSlots":MAX_ENTRIES,
        "maximumReferencedResources":MAX_REFERENCED_RESOURCES,"maximumReferencedBytes":MAX_REFERENCED_BYTES,
        "contentsFrozen":false,"runtimeVerified":false})
}
