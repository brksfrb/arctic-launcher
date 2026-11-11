//! Owners for commands recorded into an engine-owned direct list. Public GPU
//! completion alone does not make a native closed list cease to be replayable.
use hudhook::windows::{
    Win32::Graphics::Direct3D12::*,
    core::{IUnknown, Interface},
};
use serde_json::{Value, json};
use std::{
    any::Any,
    collections::HashSet,
    ffi::c_void,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
};

const MAX_SLOTS: usize = 16;
const MAX_ASSET_CHUNKS: usize = 4096;
pub(crate) type Assets = Box<dyn Any + Send + Sync>;
pub(crate) struct RetainError {
    pub reason: String,
    pub assets: Assets,
}
struct Owner {
    list: usize,
    native_list: usize,
    generation: u64,
    replayable: bool,
    device: ID3D12Device,
    fence: ID3D12Fence,
    token: crate::native_graphics_state::ListToken,
    _assets: Vec<Assets>,
    _queue: Option<ID3D12CommandQueue>,
    queue: usize,
    submission: u64,
    pending: bool,
    failed: bool,
}
struct Pool {
    slots: [Option<Owner>; MAX_SLOTS],
    generation: u64,
    retained: u64,
    retired: u64,
    native_retired: u64,
}
impl Default for Pool {
    fn default() -> Self {
        Self {
            slots: std::array::from_fn(|_| None),
            generation: 0,
            retained: 0,
            retired: 0,
            native_retired: 0,
        }
    }
}
static POOL: OnceLock<Mutex<Pool>> = OnceLock::new();
static OWNERS: [AtomicUsize; MAX_SLOTS] = [const { AtomicUsize::new(0) }; MAX_SLOTS];
static ALIASES: [AtomicUsize; MAX_SLOTS] = [const { AtomicUsize::new(0) }; MAX_SLOTS];
static GENERATIONS: [AtomicU64; MAX_SLOTS] = [const { AtomicU64::new(0) }; MAX_SLOTS];
static RELEASED: [AtomicU64; MAX_SLOTS] = [const { AtomicU64::new(0) }; MAX_SLOTS];
fn pool() -> &'static Mutex<Pool> {
    POOL.get_or_init(Mutex::default)
}
enum Append {
    Added(u64),
    New(Assets),
}
fn try_append(
    pool: &mut Pool,
    device: &ID3D12Device,
    key: usize,
    raw: usize,
    token: crate::native_graphics_state::ListToken,
    assets: Assets,
) -> Result<Append, RetainError> {
    let Some((index, owner)) = pool.slots.iter_mut().enumerate().find_map(|(index, slot)| {
        let owner = slot.as_mut()?;
        (owner.list == key && owner.native_list == raw && owner.token == token)
            .then_some((index, owner))
    }) else {
        return Ok(Append::New(assets));
    };
    if owner.device != *device
        || !owner.replayable
        || owner.failed
        || owner.pending
        || owner.submission != 0
        || owner.queue != 0
        || RELEASED[index].load(Ordering::Acquire) == owner.generation
    {
        return Err(RetainError {
            reason: "native recording lifetime is no longer appendable".into(),
            assets,
        });
    }
    if owner._assets.len() >= MAX_ASSET_CHUNKS {
        return Err(RetainError {
            reason: "native recording asset chunk budget exceeded".into(),
            assets,
        });
    }
    owner._assets.push(assets);
    Ok(Append::Added(owner.generation))
}

/// # Safety
/// This is the live, engine-owned, recording direct list intercepted by the
/// installed observer. The caller serializes this list against reset/execute,
/// and assets pin everything its inserted commands can reference. Register
/// before recording the first such command. No state restoration or native
/// target ownership is asserted by this function.
/// token comes from the locked current graphics-state shadow for this exact
/// recording lifetime. Repeated chunks may share its unsubmitted reservation.
/// On error the assets are returned, so the caller can release them outside its
/// own observer locks. Successful ownership remains in the pool, not the caller.
pub(crate) unsafe fn retain_recording(
    list: &ID3D12GraphicsCommandList,
    token: crate::native_graphics_state::ListToken,
    assets: Assets,
) -> Result<u64, RetainError> {
    let prepared = (|| -> Result<(ID3D12Device, usize), String> {
        if !crate::depth_tracker::native_asset_hooks_available() {
            return Err("native asset observers are unavailable".into());
        }
        if unsafe { list.GetType() } != D3D12_COMMAND_LIST_TYPE_DIRECT {
            return Err("native assets require a direct recording list".into());
        }
        let mut device: Option<ID3D12Device> = None;
        unsafe { list.GetDevice(&mut device) }.map_err(|error| error.to_string())?;
        let device = device.ok_or("native recording list has no device")?;
        unsafe { device.GetDeviceRemovedReason() }.map_err(|error| error.to_string())?;
        let key = list
            .cast::<IUnknown>()
            .map_err(|error| error.to_string())?
            .as_raw() as usize;
        Ok((device, key))
    })();
    let (device, key) = match prepared {
        Ok(value) => value,
        Err(reason) => return Err(RetainError { reason, assets }),
    };
    let raw = list.as_raw() as usize;
    let assets = {
        let mut pool = match pool().lock() {
            Ok(value) => value,
            Err(_) => {
                return Err(RetainError {
                    reason: "native asset pool is poisoned".into(),
                    assets,
                });
            }
        };
        match try_append(&mut pool, &device, key, raw, token, assets)? {
            Append::Added(generation) => return Ok(generation),
            Append::New(assets) => assets,
        }
    };
    // Do not call the driver under the pool lock, or allocate one unused fence
    // for every interleaved run. Recheck after allocation before reserving.
    let fence: ID3D12Fence = match unsafe { device.CreateFence(0, D3D12_FENCE_FLAG_NONE) } {
        Ok(fence) => fence,
        Err(error) => {
            return Err(RetainError {
                reason: error.to_string(),
                assets,
            });
        }
    };
    let mut pool = match pool().lock() {
        Ok(value) => value,
        Err(_) => {
            return Err(RetainError {
                reason: "native asset pool is poisoned".into(),
                assets,
            });
        }
    };
    let assets = match try_append(&mut pool, &device, key, raw, token, assets)? {
        Append::Added(generation) => return Ok(generation),
        Append::New(assets) => assets,
    };
    let Some(index) = pool.slots.iter().position(Option::is_none) else {
        return Err(RetainError {
            reason: "native command assets are still referenced or in flight".into(),
            assets,
        });
    };
    let Some(generation) = pool.generation.checked_add(1) else {
        return Err(RetainError {
            reason: "native asset generation exhausted".into(),
            assets,
        });
    };
    pool.generation = generation;
    pool.retained = pool.retained.saturating_add(1);
    pool.slots[index] = Some(Owner {
        list: key,
        native_list: list.as_raw() as usize,
        generation,
        replayable: true,
        device,
        fence,
        token,
        _assets: vec![assets],
        _queue: None,
        queue: 0,
        submission: 0,
        pending: false,
        failed: false,
    });
    GENERATIONS[index].store(generation, Ordering::Release);
    ALIASES[index].store(list.as_raw() as usize, Ordering::Release);
    OWNERS[index].store(key, Ordering::Release);
    Ok(generation)
}

pub(crate) fn has_owners() -> bool {
    OWNERS
        .iter()
        .any(|owner| owner.load(Ordering::Acquire) != 0)
}
/// Capture before the actual COM Release. No object is dereferenced afterward.
pub(crate) fn release_candidates(list: usize) -> [u64; MAX_SLOTS] {
    std::array::from_fn(|index| {
        let matches = || {
            OWNERS[index].load(Ordering::Acquire) != 0
                && (OWNERS[index].load(Ordering::Acquire) == list
                    || ALIASES[index].load(Ordering::Acquire) == list)
        };
        if !matches() {
            return 0;
        }
        let generation = GENERATIONS[index].load(Ordering::Acquire);
        if matches() { generation } else { 0 }
    })
}
pub(crate) fn released(generations: [u64; MAX_SLOTS]) {
    for (index, generation) in generations.into_iter().enumerate() {
        if generation != 0 {
            RELEASED[index].fetch_max(generation, Ordering::AcqRel);
        }
    }
}
/// Called after a successful public Reset under the shared submission gate.
/// Reset removes replayability; it does not establish GPU completion.
pub(crate) unsafe fn reset(list: *mut c_void) {
    let Some(object) = (unsafe { IUnknown::from_raw_borrowed(&list) }) else {
        fault_all();
        return;
    };
    let Ok(key) = object.cast::<IUnknown>() else {
        fault_all();
        return;
    };
    let list = key.as_raw() as usize;
    let Ok(mut pool) = pool().lock() else {
        return;
    };
    for (index, owner) in pool.slots.iter_mut().enumerate() {
        if let Some(owner) = owner
            .as_mut()
            .filter(|owner| owner.list == list && owner.replayable)
        {
            owner.replayable = false;
            OWNERS[index].store(0, Ordering::Release);
        }
    }
}
pub(crate) fn fault_all() {
    if let Ok(mut pool) = pool().lock() {
        for owner in pool.slots.iter_mut().flatten() {
            owner.failed = true;
        }
    }
}
struct SubmitTicket {
    index: usize,
    generation: u64,
    submission: u64,
    fence: ID3D12Fence,
}
pub(crate) struct Submission {
    tickets: Vec<SubmitTicket>,
}
/// Caller holds the shared gate from here through the real Execute and signals.
pub(crate) fn before_submit(queue: &ID3D12CommandQueue, lists: &[ID3D12CommandList]) -> Submission {
    if !has_owners() {
        return Submission {
            tickets: Vec::new(),
        };
    }
    if lists.len() > 4096 {
        fault_all();
        return Submission {
            tickets: Vec::new(),
        };
    }
    let keys: Option<HashSet<usize>> = lists
        .iter()
        .map(|list| {
            list.cast::<IUnknown>()
                .ok()
                .map(|object| object.as_raw() as usize)
        })
        .collect();
    let Some(keys) = keys else {
        fault_all();
        return Submission {
            tickets: Vec::new(),
        };
    };
    let mut device: Option<ID3D12Device> = None;
    if unsafe { queue.GetDevice(&mut device) }.is_err()
        || unsafe { queue.GetDesc() }.Type != D3D12_COMMAND_LIST_TYPE_DIRECT
    {
        fault_all();
        return Submission {
            tickets: Vec::new(),
        };
    }
    let Ok(mut pool) = pool().lock() else {
        return Submission {
            tickets: Vec::new(),
        };
    };
    let mut tickets = Vec::new();
    for (index, owner) in pool.slots.iter_mut().enumerate() {
        let Some(owner) = owner
            .as_mut()
            .filter(|owner| owner.replayable && keys.contains(&owner.list))
        else {
            continue;
        };
        if owner.failed
            || owner.pending
            || device.as_ref() != Some(&owner.device)
            || owner.queue != 0 && owner.queue != queue.as_raw() as usize
        {
            // Cross-queue submissions cannot share one monotonically signaled
            // private fence without a separate dependency proof.
            owner.failed = true;
            continue;
        }
        let Some(submission) = owner
            .submission
            .checked_add(1)
            .filter(|value| *value != u64::MAX)
        else {
            owner.failed = true;
            continue;
        };
        owner.submission = submission;
        owner.pending = true;
        owner.queue = queue.as_raw() as usize;
        if owner._queue.is_none() {
            owner._queue = Some(queue.clone());
        }
        tickets.push(SubmitTicket {
            index,
            generation: owner.generation,
            submission,
            fence: owner.fence.clone(),
        });
    }
    Submission { tickets }
}
/// Runs after the actual Execute, under the same gate. Signal failure leaves
/// assets pinned; a timeout or dropped status sample cannot retire them.
pub(crate) fn after_submit(queue: &ID3D12CommandQueue, submission: Submission) {
    for ticket in submission.tickets {
        let success = unsafe { queue.Signal(&ticket.fence, ticket.submission) }.is_ok();
        let Ok(mut pool) = pool().lock() else {
            continue;
        };
        if let Some(owner) = pool
            .slots
            .get_mut(ticket.index)
            .and_then(Option::as_mut)
            .filter(|owner| {
                owner.generation == ticket.generation
                    && owner.submission == ticket.submission
                    && owner.queue == queue.as_raw() as usize
                    && owner.pending
            })
        {
            owner.pending = false;
            if !success {
                owner.failed = true;
            }
        }
    }
}
#[derive(Clone)]
pub(crate) struct NativeTicket {
    index: usize,
    generation: u64,
    list: usize,
    queue: usize,
    submission: u64,
}
pub(crate) fn native_candidates(queue: usize, list: usize) -> Vec<NativeTicket> {
    let Ok(pool) = pool().lock() else {
        return Vec::new();
    };
    pool.slots
        .iter()
        .enumerate()
        .filter_map(|(index, owner)| {
            let owner = owner.as_ref()?;
            (owner.replayable
                && !owner.failed
                && !owner.pending
                && owner.native_list == list
                && owner.queue == queue
                && owner.submission > 0)
                .then_some(NativeTicket {
                    index,
                    generation: owner.generation,
                    list: owner.list,
                    queue,
                    submission: owner.submission,
                })
        })
        .collect()
}
/// Only the pinned native used->free observer, after its full list/fence/ring
/// checks and under the shared submission gate, may call this entry point.
pub(crate) fn native_retired(tickets: &[NativeTicket]) -> usize {
    let Ok(mut pool) = pool().lock() else {
        return 0;
    };
    let mut count = 0;
    for ticket in tickets {
        if let Some(owner) = pool
            .slots
            .get_mut(ticket.index)
            .and_then(Option::as_mut)
            .filter(|owner| {
                owner.replayable
                    && !owner.failed
                    && !owner.pending
                    && owner.generation == ticket.generation
                    && owner.list == ticket.list
                    && owner.queue == ticket.queue
                    && owner.submission == ticket.submission
            })
        {
            owner.replayable = false;
            OWNERS[ticket.index].store(0, Ordering::Release);
            count += 1;
        }
    }
    pool.native_retired = pool.native_retired.saturating_add(count as u64);
    count
}
/// Invoke from the frame callback outside tracker/submission/pool locks. Taking
/// an owner out of the pool does not run its arbitrary payload destructor there.
pub(crate) fn collect() {
    let retired = {
        let Ok(mut pool) = pool().lock() else {
            return;
        };
        let mut retired = Vec::new();
        for (index, slot) in pool.slots.iter_mut().enumerate() {
            let Some(owner) = slot.as_mut() else {
                continue;
            };
            if RELEASED[index].load(Ordering::Acquire) == owner.generation {
                owner.replayable = false;
                OWNERS[index].store(0, Ordering::Release);
            }
            let removed = unsafe { owner.device.GetDeviceRemovedReason() }.is_err();
            let completed = unsafe { owner.fence.GetCompletedValue() };
            if removed
                || !owner.replayable
                    && !owner.failed
                    && !owner.pending
                    && (owner.submission == 0
                        || completed != u64::MAX && completed >= owner.submission)
            {
                OWNERS[index].store(0, Ordering::Release);
                if let Some(owner) = slot.take() {
                    retired.push(owner);
                }
            }
        }
        pool.retired = pool.retired.saturating_add(retired.len() as u64);
        retired
    };
    drop(retired);
}
pub(crate) fn status() -> Value {
    let Ok(pool) = pool().try_lock() else {
        return Value::Null;
    };
    json!({"capacity":MAX_SLOTS,"owners":pool.slots.iter().flatten().count(),
        "assetChunks":pool.slots.iter().flatten().map(|owner|owner._assets.len()).sum::<usize>(),
        "assetChunkLimitPerRecording":MAX_ASSET_CHUNKS,"recordingLifetimeCoalescingImplemented":true,
        "replayable":pool.slots.iter().flatten().filter(|owner|owner.replayable).count(),
        "failed":pool.slots.iter().flatten().filter(|owner|owner.failed).count(),
        "pendingSubmissions":pool.slots.iter().flatten().filter(|owner|owner.pending).count(),
        "retained":pool.retained,"retired":pool.retired,"nativeHandoffs":pool.native_retired,
        "nativeInsertionImplemented":false,"nativeDrawingVerified":false})
}
