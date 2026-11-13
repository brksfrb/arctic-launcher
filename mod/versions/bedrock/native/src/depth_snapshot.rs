//! Copies recorded inside the native list before its world depth is destroyed.
//! Slots remain allocated for process lifetime. A slot cannot be reused until
//! the native list is reset AND its last submission fence has completed.
use super::*;
use std::mem::ManuallyDrop;

const MAX_SLOTS: usize = 16;
static OWNERS: [std::sync::atomic::AtomicUsize; MAX_SLOTS] =
    [const { std::sync::atomic::AtomicUsize::new(0) }; MAX_SLOTS];
static GENERATIONS: [AtomicU64; MAX_SLOTS] = [const { AtomicU64::new(0) }; MAX_SLOTS];
static RELEASED: [AtomicU64; MAX_SLOTS] = [const { AtomicU64::new(0) }; MAX_SLOTS];
#[derive(Clone, Copy)]
pub(super) struct Ticket {
    slot: usize,
    generation: u64,
}
#[derive(Clone, Copy)]
struct Metadata {
    source: u64,
    scope: u64,
    queue: usize,
    frame: u64,
    reverse: bool,
    order: u64,
}
struct Slot {
    device: usize,
    resource: ID3D12Resource,
    fence: ID3D12Fence,
    bytes: u64,
    owner: Option<usize>,
    source: u64,
    generation: u64,
    signal: u64,
    last_queue: usize,
    submitted_generation: u64,
    failed: bool,
    busy: bool,
    metadata: Option<Metadata>,
}
impl Slot {
    fn reusable(&self) -> bool {
        let completed = unsafe { self.fence.GetCompletedValue() };
        self.owner.is_none()
            && !self.failed
            && !self.busy
            && completed != u64::MAX
            && completed >= self.signal
    }
}
#[derive(Default)]
struct Pool {
    slots: Vec<Slot>,
    generation: u64,
    order: u64,
    recorded: u64,
    accepted: u64,
    rejected: u64,
    superseded: u64,
    exhausted: u64,
    captures: u64,
    last_captured_scope: Option<u64>,
    retired_releases: u64,
    retired_native: u64,
    last_error: Option<String>,
}
static POOL: OnceLock<Mutex<Pool>> = OnceLock::new();
fn pool() -> &'static Mutex<Pool> {
    POOL.get_or_init(Mutex::default)
}

pub(super) fn reset(list: usize) {
    if let Ok(mut p) = pool().lock() {
        for (i, slot) in p.slots.iter_mut().enumerate() {
            if slot.owner == Some(list) {
                slot.owner = None;
                OWNERS[i].store(0, Ordering::Release);
            }
        }
    }
}

// Capture before calling Release: a destroyed COM address may immediately be
// reused by another thread. No lock or engine dereference occurs after Release.
pub(super) fn release_candidates(list: usize) -> [u64; MAX_SLOTS] {
    std::array::from_fn(|i| {
        if OWNERS[i].load(Ordering::Acquire) != list {
            return 0;
        }
        let generation = GENERATIONS[i].load(Ordering::Acquire);
        if OWNERS[i].load(Ordering::Acquire) == list {
            generation
        } else {
            0
        }
    })
}
pub(super) fn released(generations: [u64; MAX_SLOTS]) {
    for (i, generation) in generations.into_iter().enumerate() {
        if generation != 0 {
            RELEASED[i].fetch_max(generation, Ordering::AcqRel);
        }
    }
}
/// Caller holds SUBMIT; every preceding native execution has its signal queued.
pub(super) fn retire_released() {
    let Ok(mut p) = pool().lock() else {
        return;
    };
    let mut retired = 0;
    for (i, slot) in p.slots.iter_mut().enumerate() {
        if slot.owner.is_some() && RELEASED[i].load(Ordering::Acquire) == slot.generation {
            slot.owner = None;
            OWNERS[i].store(0, Ordering::Release);
            retired += 1;
        }
    }
    p.retired_releases += retired;
}

struct Barrier(D3D12_RESOURCE_BARRIER);
impl Barrier {
    fn new(
        r: &ID3D12Resource,
        before: D3D12_RESOURCE_STATES,
        after: D3D12_RESOURCE_STATES,
    ) -> Self {
        Self(D3D12_RESOURCE_BARRIER {
            Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
            Anonymous: D3D12_RESOURCE_BARRIER_0 {
                Transition: ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                    pResource: ManuallyDrop::new(Some(r.clone())),
                    Subresource: 0,
                    StateBefore: before,
                    StateAfter: after,
                }),
            },
            ..Default::default()
        })
    }
}
impl Drop for Barrier {
    fn drop(&mut self) {
        unsafe { ManuallyDrop::drop(&mut (*self.0.Anonymous.Transition).pResource) }
    }
}

/// Called only during an intercepted method on this live recording list.
pub(super) unsafe fn record(
    list: usize,
    source: &Resource,
    before: D3D12_RESOURCE_STATES,
) -> Result<Option<Ticket>, String> {
    let raw = list as *mut c_void;
    let command = unsafe { ID3D12GraphicsCommandList::from_raw_borrowed(&raw) }
        .ok_or("snapshot list missing")?;
    if unsafe { command.GetType() } != D3D12_COMMAND_LIST_TYPE_DIRECT {
        return Ok(None);
    }
    let mut p = pool().lock().map_err(|_| "snapshot pool lock failed")?;
    let desc = unsafe { source.resource.GetDesc() };
    let mut device: Option<ID3D12Device> = None;
    unsafe { source.resource.GetDevice(&mut device) }.map_err(|e| e.to_string())?;
    let device = device.ok_or("snapshot source device missing")?;
    // Multiple world/other-view boundaries can occur in one recording. They
    // overwrite the same COPY_DEST texture in GPU order. A new generation on
    // every copy makes only the final marker eligible at submission.
    let recording_slot = p
        .slots
        .iter()
        .position(|s| s.owner == Some(list) && s.source == source.id && !s.failed);
    let slot_index = recording_slot.or_else(|| {
        p.slots.iter().position(|s| {
            s.device == device.as_raw() as usize
                && s.reusable()
                && unsafe { s.resource.GetDesc() } == desc
        })
    });
    let index = if let Some(index) = slot_index {
        index
    } else {
        let replace = p.slots.iter().position(Slot::reusable);
        let replaced_bytes = replace.map(|i| p.slots[i].bytes).unwrap_or(0);
        if (replace.is_none() && p.slots.len() >= MAX_SLOTS)
            || p.slots.iter().map(|s| s.bytes).sum::<u64>() - replaced_bytes
                + source.estimated_bytes
                > MAX_RESOURCE_BYTES
        {
            p.exhausted += 1;
            return Ok(None);
        }
        let mut resource = None;
        unsafe {
            device.CreateCommittedResource(
                &D3D12_HEAP_PROPERTIES {
                    Type: D3D12_HEAP_TYPE_DEFAULT,
                    CreationNodeMask: 1,
                    VisibleNodeMask: 1,
                    ..Default::default()
                },
                D3D12_HEAP_FLAG_NONE,
                &desc,
                D3D12_RESOURCE_STATE_COPY_DEST,
                None,
                &mut resource,
            )
        }
        .map_err(|e| e.to_string())?;
        let fence =
            unsafe { device.CreateFence(0, D3D12_FENCE_FLAG_NONE) }.map_err(|e| e.to_string())?;
        let slot = Slot {
            device: device.as_raw() as usize,
            resource: resource.ok_or("snapshot allocation missing")?,
            fence,
            bytes: source.estimated_bytes,
            owner: None,
            source: 0,
            generation: 0,
            signal: 0,
            last_queue: 0,
            submitted_generation: 0,
            failed: false,
            busy: false,
            metadata: None,
        };
        if let Some(i) = replace {
            p.slots[i] = slot;
            i
        } else {
            p.slots.push(slot);
            p.slots.len() - 1
        }
    };
    p.generation += 1;
    let generation = p.generation;
    OWNERS[index].store(0, Ordering::Release);
    let slot = &mut p.slots[index];
    slot.owner = Some(list);
    slot.source = source.id;
    slot.generation = generation;
    slot.metadata = None;
    slot.submitted_generation = 0;
    GENERATIONS[index].store(generation, Ordering::Release);
    OWNERS[index].store(list, Ordering::Release);
    let source_before = Barrier::new(&source.resource, before, D3D12_RESOURCE_STATE_COPY_SOURCE);
    let source_after = Barrier::new(&source.resource, D3D12_RESOURCE_STATE_COPY_SOURCE, before);
    // Call the pass-through barrier trampoline: these balanced private copies
    // do not alter the native resource state recorded by the outer observer.
    let barrier = ORIGINALS
        .get()
        .ok_or("snapshot barrier trampoline missing")?
        .barrier;
    unsafe {
        if before != D3D12_RESOURCE_STATE_COPY_SOURCE {
            barrier(raw, 1, &source_before.0);
        }
        let mut src = D3D12_TEXTURE_COPY_LOCATION {
            pResource: ManuallyDrop::new(Some(source.resource.clone())),
            Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
            ..Default::default()
        };
        let mut dst = D3D12_TEXTURE_COPY_LOCATION {
            pResource: ManuallyDrop::new(Some(slot.resource.clone())),
            Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
            ..Default::default()
        };
        command.CopyTextureRegion(&dst, 0, 0, 0, &src, None);
        ManuallyDrop::drop(&mut src.pResource);
        ManuallyDrop::drop(&mut dst.pResource);
        if before != D3D12_RESOURCE_STATE_COPY_SOURCE {
            barrier(raw, 1, &source_after.0);
        }
    }
    p.recorded += 1;
    Ok(Some(Ticket {
        slot: index,
        generation,
    }))
}

pub(super) fn invalidate_source(source: u64) {
    if let Ok(mut p) = pool().lock() {
        for slot in &mut p.slots {
            if slot.metadata.is_some_and(|m| m.source == source) {
                slot.metadata = None;
            }
        }
    }
}
pub(super) fn invalidate_all() {
    if let Ok(mut p) = pool().lock() {
        for slot in &mut p.slots {
            slot.metadata = None;
        }
    }
}
pub(super) fn fault_all() {
    if let Ok(mut p) = pool().lock() {
        for slot in &mut p.slots {
            slot.failed = true;
            slot.metadata = None;
        }
    }
}
pub(super) fn apply(
    ticket: Ticket,
    state: &State,
    queue: usize,
    frame: u64,
    before: D3D12_RESOURCE_STATES,
) {
    let Ok(mut p) = pool().lock() else {
        return;
    };
    p.order += 1;
    let order = p.order;
    let valid = eligible(state, queue, frame, state.object.width, state.object.height)
        && state.state == Some(before)
        && state.scope.is_some();
    let Some(slot) = p
        .slots
        .get_mut(ticket.slot)
        .filter(|s| s.generation == ticket.generation)
    else {
        p.superseded += 1;
        return;
    };
    slot.metadata = if valid {
        Some(Metadata {
            source: state.object.id,
            scope: state.scope.unwrap(),
            queue,
            frame,
            reverse: state.reverse_z.unwrap(),
            order,
        })
    } else {
        None
    };
    if valid {
        p.accepted += 1;
    } else {
        p.rejected += 1;
    }
}

/// Must run after the actual ExecuteCommandLists driver call, under SUBMIT.
pub(super) fn submitted(queue: &ID3D12CommandQueue, lists: &[usize]) {
    let Ok(mut p) = pool().lock() else {
        return;
    };
    for slot in &mut p.slots {
        if slot.owner.is_some_and(|owner| lists.contains(&owner)) {
            slot.last_queue = queue.as_raw() as usize;
            slot.submitted_generation = slot.generation;
            slot.signal += 1;
            if unsafe { queue.Signal(&slot.fence, slot.signal) }.is_err() {
                slot.failed = true;
                slot.metadata = None;
            }
        }
    }
}

#[derive(Clone)]
pub(super) struct NativeTicket {
    slot: usize,
    generation: u64,
    owner: usize,
    queue: usize,
    signal: u64,
    submitted: bool,
}
pub(super) fn native_candidates(queue: usize, owner: usize) -> Vec<NativeTicket> {
    let Ok(p) = pool().lock() else {
        return Vec::new();
    };
    p.slots
        .iter()
        .enumerate()
        .filter_map(|(i, s)| {
            (s.owner == Some(owner)
                && !s.failed
                && (s.submitted_generation != s.generation || s.last_queue == queue))
                .then_some(NativeTicket {
                    slot: i,
                    generation: s.generation,
                    owner,
                    queue,
                    signal: s.signal,
                    submitted: s.submitted_generation == s.generation,
                })
        })
        .collect()
}
pub(super) fn has_owners() -> bool {
    OWNERS
        .iter()
        .any(|owner| owner.load(Ordering::Acquire) != 0)
}
/// The caller has proved used->free membership in the pinned native queue and
/// holds SUBMIT. Native alloc() must Reset this context before its next execute.
pub(super) fn native_retired(tickets: &[NativeTicket]) -> usize {
    let Ok(mut p) = pool().lock() else {
        return 0;
    };
    let mut retired = 0;
    for t in tickets {
        if let Some(s) = p.slots.get_mut(t.slot)
            && s.generation == t.generation
            && s.owner == Some(t.owner)
            && s.last_queue == t.queue
            && s.submitted_generation == t.generation
            && s.signal == t.signal + u64::from(!t.submitted)
            && !s.failed
        {
            s.owner = None;
            OWNERS[t.slot].store(0, Ordering::Release);
            retired += 1;
        }
    }
    p.retired_native += retired as u64;
    retired
}

/// Read submitted scope/surface facts without copying or mapping a depth plane.
/// This is geometry provenance, not ownership of the engine's scene surfaces.
pub(super) fn geometry_surface(
    queue: usize,
    frame: u64,
    width: u32,
    height: u32,
    accepts_scope: impl Fn(u64) -> bool,
) -> Result<Option<(u64, bool)>, String> {
    let p = pool().lock().map_err(|_| "snapshot pool lock failed")?;
    let mut selected = None;
    for slot in &p.slots {
        let Some(metadata) = slot.metadata else {
            continue;
        };
        if metadata.queue != queue
            || metadata.frame != frame
            || slot.failed
            || slot.busy
            || slot.signal == 0
            || slot.submitted_generation != slot.generation
            || slot.last_queue != queue
            || !accepts_scope(metadata.scope)
        {
            continue;
        }
        let desc = unsafe { slot.resource.GetDesc() };
        if desc.Width != width as u64 || desc.Height != height {
            continue;
        }
        let candidate = (metadata.source, metadata.scope, metadata.reverse);
        if selected.is_some_and(|previous| previous != candidate) {
            return Err("ambiguous submitted geometry depth convention/scope".into());
        }
        selected = Some(candidate);
    }
    Ok(selected.map(|(_, scope, reverse)| (scope, reverse)))
}

pub(super) fn capture(
    queue: &ID3D12CommandQueue,
    frame: u64,
    width: u32,
    height: u32,
    accepts_scope: impl Fn(u64) -> bool,
) -> Result<Option<(u64, crate::depth::Depth)>, String> {
    let (index, resource, metadata) = {
        let mut p = pool().lock().map_err(|_| "snapshot pool lock failed")?;
        let mut candidates = p
            .slots
            .iter()
            .enumerate()
            .filter_map(|(i, s)| {
                let m = s.metadata?;
                let desc = unsafe { s.resource.GetDesc() };
                (m.queue == queue.as_raw() as usize
                    && m.frame == frame
                    && !s.failed
                    && !s.busy
                    && s.signal > 0
                    && desc.Width == width as u64
                    && desc.Height == height
                    && accepts_scope(m.scope))
                .then_some((i, m))
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|(_, m)| m.order);
        let Some((i, m)) = candidates.last().copied() else {
            return Ok(None);
        };
        if candidates
            .iter()
            .any(|(_, other)| other.source != m.source || other.scope != m.scope)
        {
            return Err("ambiguous preserved world depth".into());
        }
        p.slots[i].busy = true;
        (i, p.slots[i].resource.clone(), m)
    };
    let result = crate::depth::dx12(
        resource,
        queue,
        D3D12_RESOURCE_STATE_COPY_DEST,
        metadata.reverse,
    );
    if let Ok(mut p) = pool().lock() {
        p.slots[index].busy = false;
        if let Err(error) = &result {
            p.slots[index].failed = true;
            p.last_error = Some(error.clone());
        } else {
            p.captures += 1;
            p.last_captured_scope = Some(metadata.scope);
        }
    }
    result.map(|depth| Some((metadata.scope, depth)))
}

pub(super) fn status() -> serde_json::Value {
    let Ok(p) = pool().try_lock() else {
        return serde_json::Value::Null;
    };
    serde_json::json!({"slots":p.slots.len(),"bytes":p.slots.iter().map(|s|s.bytes).sum::<u64>(),"recorded":p.recorded,"accepted":p.accepted,"superseded":p.superseded,"retiredReleases":p.retired_releases,"retiredNative":p.retired_native,"rejected":p.rejected,"exhausted":p.exhausted,"failed":p.slots.iter().filter(|s|s.failed).count(),"owned":p.slots.iter().filter(|s|s.owner.is_some()).count(),"retiredPending":p.slots.iter().filter(|s|s.owner.is_none() && unsafe{s.fence.GetCompletedValue()} < s.signal).count(),"reusable":p.slots.iter().filter(|s|s.reusable()).count(),"captures":p.captures,"lastCapturedScope":p.last_captured_scope,"lastError":p.last_error})
}

pub(super) fn owner_trace(tracker: &Tracker) -> serde_json::Value {
    let Ok(p) = pool().try_lock() else {
        return serde_json::Value::Null;
    };
    serde_json::json!(p.slots.iter().filter_map(|slot| {
        let owner = slot.owner?;
        let list = tracker.lists.get(&owner);
        Some(serde_json::json!({"listKnown":list.is_some(),"closed":list.map(|l|l.closed),"unknown":list.map(|l|l.unknown),"events":list.map(|l|l.events.len()),"listAgeFrames":list.map(|l|tracker.frame.saturating_sub(l.frame)),"copyAgeFrames":slot.metadata.map(|m|tracker.frame.saturating_sub(m.frame))}))
    }).collect::<Vec<_>>())
}
