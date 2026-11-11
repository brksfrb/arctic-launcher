//! Observe the pinned native values-vector reallocator. It inserts an identity
//! between copied matrix ranges; key insertion has already happened. Only
//! verified storage movement can rebind an existing owned pose's witness.
use super::{Reads, RelocationContext, flat, relocation_context, word};
use crate::native_emote_cache_history::{
    self as history, FlatRelocation, FlatRelocationRow, FlatRelocationTarget, Witness,
};
use crate::world_hooks::{CacheMatrixInsertBinding, read};
use std::{
    cell::Cell,
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
};

type Insert = unsafe extern "system" fn(usize, usize) -> usize;
static ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static CALLS: AtomicU64 = AtomicU64::new(0);
static VERIFIED: AtomicU64 = AtomicU64::new(0);
static REJECTED: AtomicU64 = AtomicU64::new(0);
static DEFERRED: AtomicU64 = AtomicU64::new(0);
const MAX_COPY_BYTES: usize = 128 * 1024;
const MAX_RETAINED_BYTES: usize = 4 * 1024 * 1024;
thread_local! {static RETAINED:Cell<usize>=const{Cell::new(0)};}
pub(super) struct Budget(usize);
impl Budget {
    pub(super) fn take(bytes: usize) -> Option<Self> {
        RETAINED
            .try_with(|used| {
                let next = used.get().checked_add(bytes)?;
                if next > MAX_RETAINED_BYTES {
                    return None;
                }
                used.set(next);
                Some(Self(bytes))
            })
            .ok()
            .flatten()
    }
}
impl Drop for Budget {
    fn drop(&mut self) {
        let _ = RETAINED.try_with(|used| used.set(used.get().saturating_sub(self.0)));
    }
}
fn overlap(a: usize, n: usize, b: usize, m: usize) -> bool {
    match (a.checked_add(n), b.checked_add(m)) {
        (Some(end), Some(other)) => a < other && b < end,
        _ => true,
    }
}
fn header(address: usize) -> Option<[u8; 24]> {
    let mut bytes = [0; 24];
    read(address, &mut bytes).ok()?;
    Some(bytes)
}
fn vector(bytes: &[u8; 24]) -> Option<(usize, usize, usize)> {
    let begin = word(bytes, 0)?;
    let end = word(bytes, 8)?;
    let capacity = word(bytes, 16)?;
    let length = end.checked_sub(begin)?;
    if !length.is_multiple_of(flat::MATRIX_BYTES)
        || capacity < end
        || !capacity
            .checked_sub(begin)?
            .is_multiple_of(flat::MATRIX_BYTES)
        || capacity.checked_sub(begin)? > 4096 * flat::MATRIX_BYTES
        || length != 0 && begin < 0x10000
    {
        return None;
    }
    Some((begin, length / flat::MATRIX_BYTES, capacity))
}
fn identity() -> [u8; 64] {
    let mut bytes = [0u8; 64];
    for diagonal in [0, 5, 10, 15] {
        bytes[diagonal * 4..diagonal * 4 + 4].copy_from_slice(&1f32.to_le_bytes());
    }
    bytes
}
struct Observation {
    context: RelocationContext,
    keys: Vec<flat::KeyEntry>,
    keys_begin: usize,
    key_header: usize,
    value_header: usize,
    key_guards: Reads,
    old: Vec<u8>,
    count: usize,
    insertion: usize,
    rows: Vec<FlatRelocationRow>,
    relocation: FlatRelocation,
    _budget: Budget,
    complete: bool,
}
impl Observation {
    fn begin(values: usize, position: usize) -> Option<Self> {
        let context = relocation_context(values)?;
        let key_header = context.stamp.actor.checked_add(flat::KEYS)?;
        let value_header = context.stamp.actor.checked_add(flat::VALUES)?;
        if value_header != values {
            return None;
        }
        let mut key_guards = Reads::default();
        let (keys_begin, key_count) =
            key_guards.vector(key_header, flat::KEY_BYTES, flat::MAX_ENTRIES)?;
        let before = header(values)?;
        let (old_begin, count, capacity) = vector(&before)?;
        // The actual caller enters this helper only for a full values vector.
        // Its newly inserted key is already present in the other vector.
        if key_count != count.checked_add(1)? || word(&before, 8)? != capacity {
            return None;
        }
        let offset = position.checked_sub(old_begin)?;
        if !offset.is_multiple_of(flat::MATRIX_BYTES) || offset / flat::MATRIX_BYTES > count {
            return None;
        }
        let insertion = offset / flat::MATRIX_BYTES;
        let key_bytes = key_count.checked_mul(flat::KEY_BYTES)?;
        let old_bytes = count.checked_mul(flat::MATRIX_BYTES)?;
        if overlap(keys_begin, key_bytes, old_begin, old_bytes)
            || overlap(keys_begin, key_bytes, key_header, 24)
            || overlap(keys_begin, key_bytes, value_header, 24)
            || overlap(old_begin, old_bytes, key_header, 24)
            || overlap(old_begin, old_bytes, value_header, 24)
        {
            return None;
        }
        let keys = flat::copy_keys(&mut key_guards, keys_begin, key_count)?;
        if keys[insertion].slot.as_ref() != context.key.slot()
            || key_guards.bytes.checked_add(24)?.checked_add(old_bytes)? > MAX_COPY_BYTES
        {
            return None;
        }
        let mut old = vec![0u8; old_bytes];
        if old_bytes != 0 {
            read(old_begin, &mut old).ok()?;
        }
        let mut rows = Vec::with_capacity(count);
        for index in 0..count {
            let native: [f32; 16] = std::array::from_fn(|i| {
                f32::from_le_bytes(
                    old[index * 64 + i * 4..index * 64 + i * 4 + 4]
                        .try_into()
                        .expect("four bytes"),
                )
            });
            if native
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 1_000_000.)
            {
                return None;
            }
            let key_index = index + usize::from(index >= insertion);
            rows.push(FlatRelocationRow {
                slot: keys[key_index].slot.clone(),
                old_matrix: old_begin.checked_add(index.checked_mul(64)?)?,
                native,
            });
        }
        let mut check = vec![0u8; old_bytes];
        if old_bytes != 0 {
            read(old_begin, &mut check).ok()?;
        }
        if check != old || header(values)? != before || !context.current() {
            return None;
        }
        key_guards.unchanged()?;
        // The scope supplies the actual observed producer key/ticket. These
        // rows came from the real transitional native header and copied bytes.
        let relocation = unsafe {
            FlatRelocation::begin(
                &context.key,
                key_header,
                value_header,
                &rows,
                context.generation,
            )
        }?;
        let bytes = key_guards.guards.capacity()*std::mem::size_of::<(usize,Vec<u8>)>()
            +key_guards.guards.iter().map(|(_,bytes)|bytes.capacity()).sum::<usize>()
            +keys.capacity()*std::mem::size_of::<flat::KeyEntry>()
            +keys.iter().map(|key|std::mem::size_of::<crate::native_emote_history::Slot>()+2*std::mem::size_of::<usize>()+key.slot.name.capacity()).sum::<usize>()
            +old.capacity()+rows.capacity()*std::mem::size_of::<FlatRelocationRow>()+relocation.retained_bytes()
            +context.stamp.actor_id.capacity()
            // Reserve completion's two matrix copies and target-witness array
            // before entering native code, including nested allocator callbacks.
            +key_count*(128+std::mem::size_of::<FlatRelocationTarget>());
        let budget = Budget::take(bytes)?;
        Some(Self {
            context,
            keys,
            keys_begin,
            key_header,
            value_header,
            key_guards,
            old,
            count,
            insertion,
            rows,
            relocation,
            _budget: budget,
            complete: false,
        })
    }
    fn finish(&mut self, result: usize) -> Option<()> {
        if !self.context.current() {
            return None;
        }
        self.key_guards.unchanged()?;
        let after = header(self.value_header)?;
        let (begin, count, _) = vector(&after)?;
        if count != self.count.checked_add(1)?
            || begin < 0x10000
            || result != begin.checked_add(self.insertion.checked_mul(64)?)?
        {
            return None;
        }
        let bytes = count.checked_mul(64)?;
        if overlap(
            begin,
            bytes,
            self.keys_begin,
            self.keys.len().checked_mul(flat::KEY_BYTES)?,
        ) || overlap(begin, bytes, self.key_header, 24)
            || overlap(begin, bytes, self.value_header, 24)
        {
            return None;
        }
        let mut copied = vec![0u8; bytes];
        read(begin, &mut copied).ok()?;
        let split = self.insertion.checked_mul(64)?;
        if copied[..split] != self.old[..split]
            || copied[split..split + 64] != identity()
            || copied[split + 64..] != self.old[split..]
        {
            return None;
        }
        let mut targets = Vec::with_capacity(self.count);
        for index in 0..self.count {
            let key_index = index + usize::from(index >= self.insertion);
            let key = &self.keys[key_index];
            targets.push(FlatRelocationTarget {
                slot: self.rows[index].slot.clone(),
                witness: Witness::flat(
                    self.key_header,
                    self.value_header,
                    self.keys_begin,
                    begin,
                    key.address,
                    begin.checked_add(key_index.checked_mul(64)?)?,
                ),
            });
        }
        let mut check = vec![0u8; bytes];
        read(begin, &mut check).ok()?;
        if check != copied || header(self.value_header)? != after || !self.context.current() {
            return None;
        }
        self.key_guards.unchanged()?;
        if !self.relocation.commit(&targets) {
            return None;
        }
        self.complete = true;
        Some(())
    }
}
impl Drop for Observation {
    fn drop(&mut self) {
        if !self.complete {
            crate::native_transform::invalidate_owner_frame_uploads(self.context.stamp.actor);
            // The sealed history ticket invalidates its opaque owner on drop.
        }
    }
}
fn invalidate(values: usize) {
    if let Some(owner) = values
        .checked_sub(flat::VALUES)
        .filter(|owner| (0x10000..0x0000_8000_0000_0000).contains(owner))
    {
        crate::native_transform::invalidate_owner_frame_uploads(owner);
        history::invalidate_owner(owner);
    } else {
        crate::native_transform::invalidate_other_frame_uploads(None);
        history::invalidate();
    }
}
unsafe extern "system" fn insert(values: usize, position: usize) -> usize {
    CALLS.fetch_add(1, Ordering::Relaxed);
    let original: Insert = unsafe { std::mem::transmute(ORIGINAL.load(Ordering::Acquire)) };
    // An ancestor journal may still hold the old locations while earlier
    // siblings have performed in-place shifts. Its complete before/after map
    // owns witness updates; a narrower capture must not revoke those records.
    if super::producer_storage::tracks_vector(values) {
        DEFERRED.fetch_add(1, Ordering::Relaxed);
        return unsafe { original(values, position) };
    }
    let mut observation = std::panic::catch_unwind(|| Observation::begin(values, position))
        .ok()
        .flatten();
    if observation.is_none() {
        let _ = std::panic::catch_unwind(|| invalidate(values));
    }
    let result = unsafe { original(values, position) };
    let accepted = observation.as_mut().is_some_and(|observation| {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| observation.finish(result)))
            .ok()
            .flatten()
            .is_some()
    });
    if accepted {
        VERIFIED.fetch_add(1, Ordering::Relaxed);
    } else {
        REJECTED.fetch_add(1, Ordering::Relaxed);
    }
    result
}
pub(super) fn hook(
    binding: CacheMatrixInsertBinding,
) -> (usize, *const (), &'static AtomicUsize, &'static [u8]) {
    (
        binding.rva,
        insert as *const (),
        &ORIGINAL,
        binding.prefix(),
    )
}
pub(super) fn status() -> serde_json::Value {
    serde_json::json!({"sourceImplemented":true,"calls":CALLS.load(Ordering::Relaxed),
        "verifiedCopies":VERIFIED.load(Ordering::Relaxed),"rejected":REJECTED.load(Ordering::Relaxed),
        "deferredToProducerJournal":DEFERRED.load(Ordering::Relaxed),
        "maximumInputSnapshotBytes":MAX_COPY_BYTES,"maximumRetainedBytesPerThread":MAX_RETAINED_BYTES,
        "copyBudgetScope":"owned input snapshot; repeated guards and post-call comparisons perform additional bounded reads",
        "retainsNativePointersInHistory":false,"changesNativeMatrices":false,"changesRecordedClock":false,
        "completeMutationCoverage":false,"runtimeVerified":false})
}
