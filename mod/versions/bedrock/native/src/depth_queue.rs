//! Observe the pinned native queue returning completed contexts to its free ring.
//! A closed pooled list remains a COM object, but alloc() must reset it before
//! another execution. Retirement also requires the context's deferred resource
//! vector to be drained by that handoff; this is not an allocation/scene lease.
use super::*;
use crate::native_scene_layout::native_queue as layout;
use crate::world_hooks::{pointer, read};
use std::sync::atomic::{AtomicBool, AtomicUsize};
#[path = "native_queue_profile.rs"]
mod profile;

use layout::{
    CAPACITY, CONTEXT_DEFERRED, CONTEXT_FENCE, CONTEXT_SUBMITTED, FENCE_CONTEXT, FREE, FREE_READ,
    FREE_WRITE, USED, USED_READ, USED_WRITE,
};
type Consume = unsafe extern "system" fn(usize, u64, u32) -> u8;
static ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static INSTALL: OnceLock<Result<(), String>> = OnceLock::new();
static ENABLED: AtomicBool = AtomicBool::new(false);
static CALLS: AtomicU64 = AtomicU64::new(0);
static CANDIDATES: AtomicU64 = AtomicU64::new(0);
static HANDOFFS: AtomicU64 = AtomicU64::new(0);
static ASSET_HANDOFFS: AtomicU64 = AtomicU64::new(0);
static REJECTED: AtomicU64 = AtomicU64::new(0);

fn word(address: usize) -> Option<u64> {
    let mut bytes = [0; 8];
    read(address, &mut bytes).ok()?;
    Some(u64::from_le_bytes(bytes))
}
fn at(base: usize, offset: usize) -> Option<usize> {
    base.checked_add(offset)
}
#[derive(Clone, Copy)]
struct DeferredResources {
    begin: u64,
    end: u64,
    capacity: u64,
}
impl DeferredResources {
    fn copy(context: usize) -> Option<Self> {
        let address = at(context, CONTEXT_DEFERRED)?;
        let mut bytes = [0u8; layout::DEFERRED_BYTES];
        read(address, &mut bytes).ok()?;
        let value = Self {
            begin: u64::from_le_bytes(bytes[0..8].try_into().ok()?),
            end: u64::from_le_bytes(bytes[8..16].try_into().ok()?),
            capacity: u64::from_le_bytes(bytes[16..24].try_into().ok()?),
        };
        let empty = value.begin == 0 && value.end == 0 && value.capacity == 0;
        if !empty
            && (value.begin < 0x10000
                || value.begin > value.end
                || value.end > value.capacity
                || value.capacity >= 0x8000_0000_0000
                || (value.begin | value.end | value.capacity) & 7 != 0)
        {
            return None;
        }
        let mut check = [0u8; layout::DEFERRED_BYTES];
        read(address, &mut check).ok()?;
        // Metadata only: never dereference vector elements or acquire COM
        // ownership from these numeric addresses. A changing header is unknown.
        (bytes == check).then_some(value)
    }
    fn drained_from(self, before: Self) -> bool {
        self.begin == before.begin && self.capacity == before.capacity && self.end == self.begin
    }
}
fn ring(queue: usize, array: usize, head: usize, tail: usize) -> Option<Vec<usize>> {
    let head_at = at(queue, head)?;
    let tail_at = at(queue, tail)?;
    let first = word(head_at)? as usize;
    let last = word(tail_at)? as usize;
    let indices = ring_indices(first, last)?;
    let mut result = Vec::with_capacity(indices.len());
    for index in indices {
        result.push(pointer(at(
            queue,
            array.checked_add(index.checked_mul(8)?)?,
        )?)?);
    }
    (word(head_at)? as usize == first && word(tail_at)? as usize == last).then_some(result)
}
fn ring_indices(first: usize, last: usize) -> Option<Vec<usize>> {
    if first == CAPACITY && last == CAPACITY {
        return Some(Vec::new());
    }
    if first >= CAPACITY || last >= CAPACITY {
        return None;
    }
    let count = (last + CAPACITY - first) % CAPACITY + 1;
    Some((0..count).map(|i| (first + i) % CAPACITY).collect())
}
struct Candidate {
    context: usize,
    list: usize,
    fence: u64,
    deferred: DeferredResources,
    tickets: Vec<snapshot::NativeTicket>,
    assets: Vec<crate::native_draw_assets::NativeTicket>,
}
pub(super) struct Pending {
    queue: usize,
    fence_context: usize,
    fence_object: usize,
    candidates: Vec<Candidate>,
}
pub(super) fn before(queue: usize, wait: u64) -> Option<Pending> {
    if !snapshot::has_owners() && !crate::native_draw_assets::has_owners() {
        return None;
    }
    let actual_queue = pointer(at(queue, layout::QUEUE_OBJECT)?)?;
    let fence_context = pointer(at(queue, FENCE_CONTEXT)?)?;
    let fence_object = pointer(at(fence_context, layout::FENCE_OBJECT)?)?;
    let mut candidates = Vec::new();
    for context in ring(queue, USED, USED_READ, USED_WRITE)? {
        let fence = word(at(context, CONTEXT_FENCE)?)?;
        let mut submitted = [0];
        read(at(context, CONTEXT_SUBMITTED)?, &mut submitted).ok()?;
        if fence == u64::MAX || fence > wait || submitted != [1] {
            continue;
        }
        let list = pointer(at(context, layout::CONTEXT_LIST)?)?;
        let tickets = snapshot::native_candidates(actual_queue, list);
        let assets = crate::native_draw_assets::native_candidates(actual_queue, list);
        if !tickets.is_empty() || !assets.is_empty() {
            let Some(deferred) = DeferredResources::copy(context) else {
                continue;
            };
            candidates.push(Candidate {
                context,
                list,
                fence,
                deferred,
                tickets,
                assets,
            });
        }
    }
    if candidates.is_empty() {
        return None;
    }
    CANDIDATES.fetch_add(candidates.len() as u64, Ordering::Relaxed);
    Some(Pending {
        queue: actual_queue,
        fence_context,
        fence_object,
        candidates,
    })
}
pub(super) fn after(queue: usize, pending: Pending) -> Option<()> {
    if pointer(at(queue, layout::QUEUE_OBJECT)?)? != pending.queue
        || pointer(at(queue, FENCE_CONTEXT)?)? != pending.fence_context
        || pointer(at(pending.fence_context, layout::FENCE_OBJECT)?)? != pending.fence_object
    {
        return None;
    }
    let completed = word(at(pending.fence_context, layout::FENCE_COMPLETED)?)?;
    if completed == u64::MAX {
        return None;
    }
    let free = ring(queue, FREE, FREE_READ, FREE_WRITE)?;
    let used = ring(queue, USED, USED_READ, USED_WRITE)?;
    let mut tickets = Vec::new();
    let mut assets = Vec::new();
    for candidate in pending.candidates {
        if !free.contains(&candidate.context)
            || used.contains(&candidate.context)
            || candidate.fence > completed
            || pointer(at(candidate.context, layout::CONTEXT_LIST)?)? != candidate.list
            || word(at(candidate.context, CONTEXT_FENCE)?)? != candidate.fence
        {
            continue;
        }
        // Each admitted consumer clears the deferred end, releases the copied
        // references, and only then pushes completed contexts into the free ring.
        // Original success plus the existing fence/list/ring joins is still
        // required. Reuse, changed storage or unreadable metadata pins owners.
        if !matches!(
            DeferredResources::copy(candidate.context),
            Some(after) if after.drained_from(candidate.deferred)
        ) {
            continue;
        }
        tickets.extend(candidate.tickets);
        assets.extend(candidate.assets);
    }
    if !tickets.is_empty() || !assets.is_empty() {
        let _gate = SUBMIT.lock().ok()?;
        let retired = snapshot::native_retired(&tickets);
        HANDOFFS.fetch_add(retired as u64, Ordering::Relaxed);
        let retired = crate::native_draw_assets::native_retired(&assets);
        ASSET_HANDOFFS.fetch_add(retired as u64, Ordering::Relaxed);
    }
    Some(())
}
unsafe extern "system" fn consume(queue: usize, wait: u64, timeout: u32) -> u8 {
    let original: Consume = unsafe { std::mem::transmute(ORIGINAL.load(Ordering::Acquire)) };
    if !ENABLED.load(Ordering::Acquire) {
        return unsafe { original(queue, wait, timeout) };
    }
    CALLS.fetch_add(1, Ordering::Relaxed);
    let pending = std::panic::catch_unwind(|| before(queue, wait))
        .ok()
        .flatten();
    let result = unsafe { original(queue, wait, timeout) };
    if let Some(pending) = pending {
        if result != 0 {
            if !matches!(
                std::panic::catch_unwind(|| after(queue, pending)),
                Ok(Some(()))
            ) {
                REJECTED.fetch_add(1, Ordering::Relaxed);
            }
        } else {
            REJECTED.fetch_add(1, Ordering::Relaxed);
        }
    }
    result
}
/// Resolve immutable code before any backend dependency installs a detour.
pub(super) fn preflight(base: usize) -> Result<(), String> {
    profile::binding(base).map(|_| ())
}
/// Caller has admitted the full executable and this selected reader family.
pub(super) unsafe fn install(base: usize) -> Result<(), String> {
    let binding = profile::binding(base)?;
    INSTALL
        .get_or_init(|| {
            let target = base.checked_add(binding.rva).ok_or("queue hook overflow")?;
            unsafe { install_target(target) }
        })
        .clone()
}
unsafe fn install_target(target: usize) -> Result<(), String> {
    let mut original = std::ptr::null_mut();
    let status = unsafe { MH_CreateHook(target as _, consume as *const () as _, &mut original) };
    if status != MH_STATUS::MH_OK {
        return Err(format!("create native queue observer: {status:?}"));
    }
    ORIGINAL.store(original as usize, Ordering::Release);
    let status = unsafe { MH_EnableHook(target as _) };
    if status != MH_STATUS::MH_OK {
        return Err(format!("enable native queue observer: {status:?}"));
    }
    ENABLED.store(true, Ordering::Release);
    Ok(())
}
pub(super) fn status() -> serde_json::Value {
    serde_json::json!({"installed":ENABLED.load(Ordering::Acquire),"readerAdmitted":profile::current().is_some(),"hookRva":profile::current().map(|b|format!("0x{:x}",b.rva)),"runtimeVerified":false,"calls":CALLS.load(Ordering::Relaxed),"candidates":CANDIDATES.load(Ordering::Relaxed),"retired":HANDOFFS.load(Ordering::Relaxed),"assetHandoffs":ASSET_HANDOFFS.load(Ordering::Relaxed),"rejected":REJECTED.load(Ordering::Relaxed),"drawAssets":crate::native_draw_assets::status(),"committedResourceOwners":crate::native_committed_resources::status(),"descriptorResources":crate::native_descriptor_resources::status()})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ring_bounds_wrapping_and_empty_sentinel() {
        assert_eq!(ring_indices(CAPACITY, CAPACITY), Some(vec![]));
        assert_eq!(
            ring_indices(CAPACITY - 2, 1),
            Some(vec![CAPACITY - 2, CAPACITY - 1, 0, 1])
        );
        assert_eq!(ring_indices(1, 1), Some(vec![1]));
        assert!(ring_indices(CAPACITY, 0).is_none());
        assert!(ring_indices(0, CAPACITY + 1).is_none());
    }
    #[inline(never)]
    unsafe extern "system" fn host(queue: usize, wait: u64, timeout: u32) -> u8 {
        unsafe { *(queue as *mut [u64; 2]) = std::hint::black_box([wait, timeout as u64]) };
        std::hint::black_box(0xa5)
    }
    #[test]
    fn queue_hook_preserves_timeout_fence_and_full_return_byte() {
        let status = unsafe { hudhook::mh::MH_Initialize() };
        assert!(matches!(
            status,
            MH_STATUS::MH_OK | MH_STATUS::MH_ERROR_ALREADY_INITIALIZED
        ));
        unsafe { install_target(host as *const () as usize) }.unwrap();
        let function: Consume = std::hint::black_box(host);
        let mut out = [0u64; 2];
        assert_eq!(
            unsafe { function(out.as_mut_ptr() as usize, 0x123456789abcdef0, u32::MAX) },
            0xa5
        );
        assert_eq!(out, [0x123456789abcdef0, u32::MAX as u64]);
    }
}
