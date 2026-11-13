//! Stable snapshots around the entire native producer cover key-only movement
//! and in-place matrix insertion. Both unchanged old poses and witnessed child
//! writes may follow their logical slots while retaining their own clocks.
use super::relocation::Budget;
use super::{ENABLED, EVENTS, Input, Reads, SKIPPED, flat};
use crate::native_emote_cache_history::{
    self as history, FlatProducerRelocation, FlatProducerTarget, FlatRelocation, FlatRelocationRow,
    Witness,
};
use std::{
    cell::RefCell,
    sync::atomic::{AtomicU64, Ordering},
};

const MAX_SNAPSHOT_BYTES: usize = 512 * 1024;
thread_local! {static OWNERS:RefCell<Vec<usize>>=const{RefCell::new(Vec::new())};}
static CAPTURED: AtomicU64 = AtomicU64::new(0);
static COMPLETED: AtomicU64 = AtomicU64::new(0);
static REJECTED: AtomicU64 = AtomicU64::new(0);
pub(super) fn tracks_vector(values: usize) -> bool {
    let Some(owner) = values.checked_sub(flat::VALUES) else {
        return false;
    };
    OWNERS
        .try_with(|owners| {
            owners
                .try_borrow()
                .ok()
                .is_some_and(|owners| owners.contains(&owner))
        })
        .unwrap_or(false)
}
struct Claim(usize);
impl Claim {
    fn take(owner: usize) -> Option<Option<Self>> {
        OWNERS
            .try_with(|owners| {
                let mut owners = owners.try_borrow_mut().ok()?;
                if owners.contains(&owner) {
                    return Some(None);
                }
                if owners.len() >= super::MAX_DEPTH {
                    return None;
                }
                owners.push(owner);
                Some(Some(Self(owner)))
            })
            .ok()
            .flatten()
    }
}
impl Drop for Claim {
    fn drop(&mut self) {
        let _ = OWNERS.try_with(|owners| {
            if let Ok(mut owners) = owners.try_borrow_mut()
                && let Some(index) = owners.iter().rposition(|owner| *owner == self.0)
            {
                owners.remove(index);
            }
        });
    }
}
fn overlaps(a: usize, n: usize, b: usize, m: usize) -> bool {
    match (a.checked_add(n), b.checked_add(m)) {
        (Some(end), Some(other)) => a < other && b < end,
        _ => true,
    }
}
struct Snapshot {
    key_header: usize,
    value_header: usize,
    keys_begin: usize,
    values_begin: usize,
    keys: Vec<flat::KeyEntry>,
    rows: Vec<FlatRelocationRow>,
    guards: Reads,
}
impl Snapshot {
    fn copy(actor: usize) -> Option<Self> {
        if !flat::selected_family() {
            return None;
        }
        let key_header = actor.checked_add(flat::KEYS)?;
        let value_header = actor.checked_add(flat::VALUES)?;
        let mut guards = Reads::default();
        let (keys_begin, count) = guards.vector(key_header, flat::KEY_BYTES, flat::MAX_ENTRIES)?;
        let (values_begin, value_count) = guards.vector(value_header, 64, flat::MAX_ENTRIES)?;
        if count != value_count {
            return None;
        }
        let key_bytes = count.checked_mul(flat::KEY_BYTES)?;
        let value_bytes = count.checked_mul(64)?;
        if overlaps(keys_begin, key_bytes, values_begin, value_bytes)
            || overlaps(keys_begin, key_bytes, key_header, 24)
            || overlaps(keys_begin, key_bytes, value_header, 24)
            || overlaps(values_begin, value_bytes, key_header, 24)
            || overlaps(values_begin, value_bytes, value_header, 24)
        {
            return None;
        }
        let keys = flat::copy_keys(&mut guards, keys_begin, count)?;
        let matrices = if value_bytes == 0 {
            Vec::new()
        } else {
            guards.copy(values_begin, value_bytes)?
        };
        let mut rows = Vec::with_capacity(count);
        for (index, key) in keys.iter().enumerate() {
            let native: [f32; 16] = std::array::from_fn(|i| {
                f32::from_le_bytes(
                    matrices[index * 64 + i * 4..index * 64 + i * 4 + 4]
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
            rows.push(FlatRelocationRow {
                slot: key.slot.clone(),
                old_matrix: values_begin.checked_add(index.checked_mul(64)?)?,
                native,
            });
        }
        guards.unchanged()?;
        let result = Self {
            key_header,
            value_header,
            keys_begin,
            values_begin,
            keys,
            rows,
            guards,
        };
        (result.retained_bytes() <= MAX_SNAPSHOT_BYTES).then_some(result)
    }
    fn retained_bytes(&self) -> usize {
        self.guards.guards.capacity() * std::mem::size_of::<(usize, Vec<u8>)>()
            + self
                .guards
                .guards
                .iter()
                .map(|(_, bytes)| bytes.capacity())
                .sum::<usize>()
            + self.keys.capacity() * std::mem::size_of::<flat::KeyEntry>()
            + self
                .keys
                .iter()
                .map(|key| {
                    std::mem::size_of::<crate::native_emote_history::Slot>()
                        + 2 * std::mem::size_of::<usize>()
                        + key.slot.name.capacity()
                })
                .sum::<usize>()
            + self.rows.capacity() * std::mem::size_of::<FlatRelocationRow>()
    }
    fn value(&self, slot: &crate::native_emote_history::Slot) -> Option<(Witness, [f32; 16])> {
        let index = self
            .keys
            .binary_search_by(|key| key.slot.as_ref().cmp(slot))
            .ok()?;
        let row = &self.rows[index];
        Some((
            Witness::flat(
                self.key_header,
                self.value_header,
                self.keys_begin,
                self.values_begin,
                self.keys[index].address,
                row.old_matrix,
            ),
            row.native,
        ))
    }
}
fn current(input: &Input, event_id: u64) -> bool {
    if !ENABLED.load(Ordering::Acquire)
        || !crate::native_body::enabled()
        || !flat::selected_family()
        || crate::native_transform::palette_writer_scope().as_ref() != Some(&input.stamp)
        || !input.owner.valid()
        || input.guards.unchanged().is_none()
    {
        return false;
    }
    EVENTS
        .try_with(|events| {
            let events = events.try_borrow().ok()?;
            let event = events.last()?;
            (event.id == event_id
                && !event.ambiguous
                && event.frame == input.stamp.frame
                && event.args[1] == input.owner.params()
                && event.mask == SKIPPED.with(std::cell::Cell::get)
                && event.key.as_ref() == input.part.as_ref().map(|part| &part.key))
            .then_some(())
        })
        .ok()
        .flatten()
        .is_some()
}
pub(super) struct Observation {
    owner: usize,
    event: u64,
    before: Snapshot,
    relocation: FlatProducerRelocation,
    _budget: Budget,
    _claim: Claim,
    complete: bool,
}
impl Observation {
    pub(super) fn begin(input: &Input, event: u64) -> Option<Self> {
        if !flat::selected_family() {
            return None;
        }
        // Hidden parents perform no own cache write; their visible descendants
        // can claim the owner. A claimed ancestor covers the whole recursion.
        let part = input.part.as_ref()?;
        let claim = match Claim::take(input.stamp.actor) {
            Some(Some(claim)) => claim,
            Some(None) => return None,
            None => {
                reject(input.stamp.actor);
                return None;
            }
        };
        let capture = (|| {
            if !current(input, event) {
                return None;
            }
            let before = Snapshot::copy(input.stamp.actor)?;
            let generation = history::generation();
            // The active producer owns these actual component/bone inputs and
            // the stable native cache snapshot. No predicate is invented here.
            let relocation = unsafe {
                FlatRelocation::begin_producer(
                    &part.key,
                    before.key_header,
                    before.value_header,
                    &before.rows,
                    generation,
                )
            }?;
            let bytes = before.retained_bytes()
                + relocation.retained_bytes()
                + MAX_SNAPSHOT_BYTES
                + flat::MAX_ENTRIES * std::mem::size_of::<FlatProducerTarget>();
            let budget = Budget::take(bytes)?;
            if !current(input, event) {
                return None;
            }
            CAPTURED.fetch_add(1, Ordering::Relaxed);
            Some(Self {
                owner: input.stamp.actor,
                event,
                before,
                relocation,
                _budget: budget,
                _claim: claim,
                complete: false,
            })
        })();
        if capture.is_none() {
            reject(input.stamp.actor);
        }
        capture
    }
    pub(super) fn finish(&mut self, input: &Input) -> Option<()> {
        if input.stamp.actor != self.owner || !current(input, self.event) {
            return None;
        }
        let after = Snapshot::copy(self.owner)?;
        // Include newly inserted slots as well as old ones. Successful child
        // completions have their own sealed history versions, and later sibling
        // insertions may have shifted those children's keys and matrices too.
        let targets: Vec<FlatProducerTarget> = after
            .rows
            .iter()
            .map(|row| FlatProducerTarget {
                slot: row.slot.clone(),
                value: after.value(&row.slot),
            })
            .collect();
        // Only the owned pre-call bytes are used here. Their native addresses
        // may have been freed/moved during the original and are never reread.
        after.guards.unchanged()?;
        if !current(input, self.event) || !self.relocation.commit(&targets) {
            return None;
        }
        self.complete = true;
        COMPLETED.fetch_add(1, Ordering::Relaxed);
        Some(())
    }
}
fn reject(owner: usize) {
    REJECTED.fetch_add(1, Ordering::Relaxed);
    crate::native_transform::invalidate_owner_frame_uploads(owner);
    history::invalidate_owner(owner);
}
impl Drop for Observation {
    fn drop(&mut self) {
        if !self.complete {
            REJECTED.fetch_add(1, Ordering::Relaxed);
            crate::native_transform::invalidate_owner_frame_uploads(self.owner);
            // The sealed ticket revokes owner history before the claim is released.
        }
    }
}
pub(super) fn status() -> serde_json::Value {
    serde_json::json!({"sourceImplemented":true,"captured":CAPTURED.load(Ordering::Relaxed),
        "completed":COMPLETED.load(Ordering::Relaxed),"rejected":REJECTED.load(Ordering::Relaxed),
        "oneSnapshotPerOutermostOwner":true,"maximumSnapshotBytes":MAX_SNAPSHOT_BYTES,
        "coversProducerKeyMovement":true,"coversProducerInPlaceMatrixInsertion":true,
        "tracksCompletedChildWrites":true,"maximumCompletedChildWrites":1024,
        "writesNativeCache":false,"completeMutationCoverage":false,"runtimeVerified":false})
}
