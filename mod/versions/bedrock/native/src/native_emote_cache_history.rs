//! Owned cache-write history. Native identities are opaque process-local hashes;
//! no native address is retained or dereferenced through a historical record.
use crate::native_emote_history::{Pose, Slot};
use std::{
    cell::{Cell, RefCell},
    collections::hash_map::RandomState,
    hash::BuildHasher,
    marker::PhantomData,
    rc::{Rc, Weak},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
};

const MAX_RECORDS: usize = 4096;
const MAX_PENDING: usize = 512;
const MAX_PRODUCER_WRITES: usize = 1024;
const MAX_BYTES: usize = 4 * 1024 * 1024;
static GENERATION: AtomicU64 = AtomicU64::new(1);
static INVALIDATIONS: AtomicU64 = AtomicU64::new(0);
static OWNER_INVALIDATIONS: AtomicU64 = AtomicU64::new(0);
static COMMITTED: AtomicU64 = AtomicU64::new(0);
static ACTIVE_PRODUCERS: AtomicUsize = AtomicUsize::new(0);
static HASHER: OnceLock<RandomState> = OnceLock::new();
static HISTORY: OnceLock<Mutex<History>> = OnceLock::new();
static RELOCATED: AtomicU64 = AtomicU64::new(0);
static CROSS_CONTEXT_INVALIDATIONS: AtomicU64 = AtomicU64::new(0);
thread_local! {static PRODUCER_DEPTH:Cell<usize>=const{Cell::new(0)};}

fn opaque(value: impl std::hash::Hash) -> u64 {
    HASHER.get_or_init(RandomState::new).hash_one(value)
}
#[derive(PartialEq, Eq)]
pub(crate) struct Key {
    epoch: u64,
    actor: String,
    handle: u64,
    owner: u64,
    slot: Arc<Slot>,
}
impl Key {
    pub(crate) fn slot(&self) -> &Slot {
        &self.slot
    }
    pub(crate) fn copy(
        stamp: &crate::native_palette_writer::Stamp,
        slot: Arc<Slot>,
    ) -> Option<Arc<Self>> {
        if crate::native_transform::palette_writer_scope().as_ref() != Some(stamp)
            || stamp.actor_id.is_empty()
            || stamp.actor_id.len() > 128
            || slot.name.len() > 128
            || slot.group >= 64
            || slot.palette >= 256
        {
            return None;
        }
        let (level, epoch) = crate::players::session()?;
        if crate::world_hooks::pointer(
            stamp
                .actor
                .checked_add(crate::native_scene_layout::actor::LEVEL)?,
        )? != level
            || crate::players::epoch(level)? != epoch
        {
            return None;
        }
        let key = Self {
            epoch,
            actor: stamp.actor_id.clone(),
            slot,
            owner: opaque(stamp.actor),
            handle: opaque((stamp.actor, stamp.context, level, epoch)),
        };
        if crate::players::session() != Some((level, epoch))
            || crate::native_transform::palette_writer_scope().as_ref() != Some(stamp)
        {
            return None;
        }
        Some(Arc::new(key))
    }
}
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Witness {
    layout: u8,
    head: u64,
    node: u64,
    groups: u64,
    matrices: u64,
}
impl Witness {
    pub(crate) fn copy(head: usize, node: usize, groups: usize, matrices: usize) -> Self {
        Self {
            layout: 0,
            head: opaque(head),
            node: opaque(node),
            groups: opaque(groups),
            matrices: opaque(matrices),
        }
    }
    /// Flat-cache backing identities remain opaque and process-local. They
    /// cannot collide with the older tree's layout even at reused addresses.
    pub(crate) fn flat(
        key_header: usize,
        value_header: usize,
        keys: usize,
        values: usize,
        entry: usize,
        matrix: usize,
    ) -> Self {
        Self {
            layout: 1,
            head: opaque((key_header, value_header)),
            node: opaque((keys, values)),
            groups: opaque(entry),
            matrices: opaque(matrix),
        }
    }
}
struct Pending {
    serial: u64,
    thread: u64,
}
struct Record {
    key: Arc<Key>,
    serial: u64,
    frame: u64,
    pending: Vec<Pending>,
    conflicted: bool,
    witness: Option<Witness>,
    pose: Option<Pose>,
}
#[derive(Default)]
struct History {
    generation: u64,
    records: Vec<Record>,
}
impl History {
    fn sync(&mut self, generation: u64) {
        if self.generation != generation {
            self.records.clear();
            self.generation = generation;
        }
    }
    fn bytes(&self) -> usize {
        self.records.capacity() * std::mem::size_of::<Record>()
            + self
                .records
                .iter()
                .map(|record| {
                    std::mem::size_of::<Key>()
                        + 2 * std::mem::size_of::<usize>()
                        + record.key.actor.capacity()
                        + std::mem::size_of::<Slot>()
                        + 2 * std::mem::size_of::<usize>()
                        + record.key.slot.name.capacity()
                        + record.pending.capacity() * std::mem::size_of::<Pending>()
                        + record
                            .pose
                            .as_ref()
                            .and_then(|pose| pose.emote.as_ref())
                            .map_or(0, |emote| emote.token.capacity())
                        + record
                            .pose
                            .as_ref()
                            .and_then(|pose| pose.replay.as_ref())
                            .map_or(0, |replay| replay.manifest_hash.capacity())
                })
                .sum::<usize>()
    }
}
fn history() -> &'static Mutex<History> {
    HISTORY.get_or_init(Mutex::default)
}
pub(crate) fn generation() -> u64 {
    GENERATION.load(Ordering::Acquire)
}
pub(crate) struct Producer {
    counted: bool,
    local: bool,
    _thread: PhantomData<Rc<()>>,
}
pub(crate) fn producer() -> Producer {
    let local = PRODUCER_DEPTH
        .try_with(|depth| {
            depth.get().checked_add(1).is_some_and(|next| {
                depth.set(next);
                true
            })
        })
        .unwrap_or(false);
    let counted = ACTIVE_PRODUCERS
        .try_update(Ordering::AcqRel, Ordering::Acquire, |count| {
            count.checked_add(1)
        })
        .is_ok();
    if !counted || !local {
        invalidate();
    }
    Producer {
        counted,
        local,
        _thread: PhantomData,
    }
}
impl Drop for Producer {
    fn drop(&mut self) {
        if self.local {
            let _ = PRODUCER_DEPTH.try_with(|depth| depth.set(depth.get().saturating_sub(1)));
        }
        if self.counted {
            ACTIVE_PRODUCERS.fetch_sub(1, Ordering::AcqRel);
        }
    }
}
/// Invalidating before native destruction also covers allocator reuse. A lost
/// mutex race cannot preserve old eligibility: the next access syncs generation.
pub(crate) fn invalidate() {
    let _ = GENERATION.try_update(Ordering::AcqRel, Ordering::Acquire, |old| {
        Some(if old == 0 {
            0
        } else {
            old.checked_add(1).unwrap_or(0)
        })
    });
    INVALIDATIONS.fetch_add(1, Ordering::Relaxed);
    if let Some(history) = HISTORY.get()
        && let Ok(mut history) = history.try_lock()
    {
        history.sync(generation());
    }
}
/// Unsupported draws by other owners must not erase every player's history.
/// Losing this mutation's lock race falls back to global invalidation.
pub(crate) fn invalidate_owner(owner: usize) {
    invalidate_opaque_owner(opaque(owner));
}
fn invalidate_opaque_owner(owner: u64) {
    OWNER_INVALIDATIONS.fetch_add(1, Ordering::Relaxed);
    let Some(history) = HISTORY.get() else {
        return;
    };
    let Ok(mut history) = history.try_lock() else {
        invalidate();
        return;
    };
    history.sync(generation());
    history.records.retain(|record| record.key.owner != owner);
}

fn current_thread_producers_only() -> bool {
    PRODUCER_DEPTH
        .try_with(|depth| {
            depth.get() != 0 && depth.get() == ACTIVE_PRODUCERS.load(Ordering::Acquire)
        })
        .unwrap_or(false)
}
/// Borrowed addresses exist only in this synchronous mutation input. A ticket
/// seals opaque record identities and versions, never the addresses themselves.
pub(crate) struct FlatRelocationRow {
    pub(crate) slot: Arc<Slot>,
    pub(crate) old_matrix: usize,
    pub(crate) native: [f32; 16],
}
pub(crate) struct FlatRelocationTarget {
    pub(crate) slot: Arc<Slot>,
    pub(crate) witness: Witness,
}
pub(crate) struct FlatProducerTarget {
    pub(crate) slot: Arc<Slot>,
    pub(crate) value: Option<(Witness, [f32; 16])>,
}
pub(crate) struct FlatProducerRelocation {
    ticket: FlatRelocation,
    writes: Rc<RefCell<ProducerWrites>>,
}
struct RelocatedRecord {
    key: Arc<Key>,
    serial: u64,
    frame: u64,
    witness: Witness,
    native: [u32; 16],
    row: usize,
}
// Only successful, witnessed native writes can enter this journal. A serial
// watermark or equal final matrix alone cannot establish child-write authority.
struct ProducerWrites {
    generation: u64,
    epoch: u64,
    owner: u64,
    head: u64,
    records: Vec<RelocatedRecord>,
    failed: bool,
}
thread_local! {static FLAT_PRODUCER_WRITES:RefCell<Vec<Weak<RefCell<ProducerWrites>>>>=const{RefCell::new(Vec::new())};}
fn journal_completed_write(generation: u64, record: &Record) -> bool {
    let Some(witness) = record
        .witness
        .as_ref()
        .filter(|witness| witness.layout == 1)
    else {
        return true;
    };
    let Some(pose) = record.pose.as_ref() else {
        return false;
    };
    FLAT_PRODUCER_WRITES
        .try_with(|journals| {
            let mut journals = journals.try_borrow_mut().ok()?;
            journals.retain(|journal| journal.strong_count() != 0);
            for journal in journals.iter().filter_map(Weak::upgrade) {
                let mut journal = journal.try_borrow_mut().ok()?;
                if journal.owner != record.key.owner || journal.epoch != record.key.epoch {
                    continue;
                }
                if journal.failed
                    || journal.generation != generation
                    || journal.head != witness.head
                    || record.key.actor.capacity() > 128
                    || record.key.slot.name.capacity() > 128
                    || !current_thread_producers_only()
                {
                    journal.failed = true;
                    return None;
                }
                let saved = RelocatedRecord {
                    key: record.key.clone(),
                    serial: record.serial,
                    frame: record.frame,
                    witness: witness.clone(),
                    native: pose.native.map(f32::to_bits),
                    row: 0,
                };
                if let Some(old) = journal.records.iter_mut().find(|old| old.key == record.key) {
                    *old = saved;
                } else {
                    if journal.records.len() >= MAX_PRODUCER_WRITES {
                        journal.failed = true;
                        return None;
                    }
                    journal.records.push(saved);
                }
            }
            Some(())
        })
        .ok()
        .flatten()
        .is_some()
}
pub(crate) struct FlatRelocation {
    generation: u64,
    epoch: u64,
    owner: u64,
    rows: usize,
    records: Vec<RelocatedRecord>,
    closed: bool,
    _thread: PhantomData<Rc<()>>,
}
impl FlatRelocation {
    /// # Safety
    /// Called inside the exact native flat-cache producer/reallocator, with a
    /// live producer ticket for anchor. Rows must be the guarded old matrices
    /// paired to the already inserted, strictly ordered native key vector;
    /// the inserted key itself is omitted. No archived or synthetic input may
    /// establish this authority. The caller must validate the native copy and
    /// current callback/headers before committing.
    pub(crate) unsafe fn begin(
        anchor: &Arc<Key>,
        key_header: usize,
        value_header: usize,
        rows: &[FlatRelocationRow],
        expected_generation: u64,
    ) -> Option<Self> {
        Self::capture(anchor, key_header, value_header, rows, expected_generation)
    }
    /// # Safety
    /// Input belongs to the active exact native producer entry, before its
    /// original is invoked. The caller must hold the live component/actor scope,
    /// copy a stable complete native cache and recheck that scope at completion.
    /// This must enclose the whole outermost producer for this owner. Old
    /// committed records and subsequently witnessed child completions are sealed;
    /// this never authorizes a write or manufactures a predicate/current pose.
    pub(crate) unsafe fn begin_producer(
        anchor: &Arc<Key>,
        key_header: usize,
        value_header: usize,
        rows: &[FlatRelocationRow],
        expected_generation: u64,
    ) -> Option<FlatProducerRelocation> {
        let ticket = Self::capture(anchor, key_header, value_header, rows, expected_generation)?;
        let writes = Rc::new(RefCell::new(ProducerWrites {
            generation: expected_generation,
            epoch: anchor.epoch,
            owner: anchor.owner,
            head: opaque((key_header, value_header)),
            records: Vec::with_capacity(MAX_PRODUCER_WRITES),
            failed: false,
        }));
        FLAT_PRODUCER_WRITES
            .try_with(|journals| {
                let mut journals = journals.try_borrow_mut().ok()?;
                journals.retain(|journal| journal.strong_count() != 0);
                if journals.len() >= MAX_PENDING {
                    return None;
                }
                for journal in journals.iter().filter_map(Weak::upgrade) {
                    if journal.try_borrow().ok()?.owner == anchor.owner {
                        return None;
                    }
                }
                journals.push(Rc::downgrade(&writes));
                Some(())
            })
            .ok()
            .flatten()?;
        Some(FlatProducerRelocation { ticket, writes })
    }
    fn capture(
        anchor: &Arc<Key>,
        key_header: usize,
        value_header: usize,
        rows: &[FlatRelocationRow],
        expected_generation: u64,
    ) -> Option<Self> {
        if rows.len() > 1024
            || expected_generation == 0
            || expected_generation != generation()
            || !current_thread_producers_only()
            || !crate::players::session().is_some_and(|(_, epoch)| epoch == anchor.epoch)
        {
            return None;
        }
        let mut state = history().try_lock().ok()?;
        state.sync(expected_generation);
        let head = opaque((key_header, value_header));
        let mut records = Vec::new();
        // The physical cache belongs to the actor, even when recorded slots
        // came from different render contexts. A verified byte copy preserves
        // each eligible slot's original context/clock without merging keys.
        for record in state.records.iter_mut().filter(|record| {
            record.key.owner == anchor.owner
                && record.key.epoch == anchor.epoch
                && record.key.actor == anchor.actor
        }) {
            if record.conflicted || !record.pending.is_empty() {
                continue;
            }
            let Some(witness) = record.witness.as_ref() else {
                continue;
            };
            let Some(pose) = record.pose.as_ref() else {
                continue;
            };
            let row = rows
                .binary_search_by(|row| row.slot.as_ref().cmp(record.key.slot.as_ref()))
                .ok();
            let matched = row.is_some_and(|index| {
                witness.layout == 1
                    && witness.head == head
                    && witness.matrices == opaque(rows[index].old_matrix)
                    && pose.native.map(f32::to_bits) == rows[index].native.map(f32::to_bits)
            });
            if !matched {
                record.witness = None;
                record.pose = None;
                continue;
            }
            records.push(RelocatedRecord {
                key: record.key.clone(),
                serial: record.serial,
                frame: record.frame,
                witness: witness.clone(),
                native: pose.native.map(f32::to_bits),
                row: row?,
            });
        }
        if expected_generation != generation() || !current_thread_producers_only() {
            return None;
        }
        Some(Self {
            generation: expected_generation,
            epoch: anchor.epoch,
            owner: anchor.owner,
            rows: rows.len(),
            records,
            closed: false,
            _thread: PhantomData,
        })
    }
    pub(crate) fn retained_bytes(&self) -> usize {
        self.records.capacity() * std::mem::size_of::<RelocatedRecord>()
            + self
                .records
                .iter()
                .map(|saved| {
                    std::mem::size_of::<Key>()
                        + std::mem::size_of::<Slot>()
                        + 4 * std::mem::size_of::<usize>()
                        + saved.key.actor.capacity()
                        + saved.key.slot.name.capacity()
                })
                .sum::<usize>()
    }
    pub(crate) fn commit(&mut self, targets: &[FlatRelocationTarget]) -> bool {
        if self.closed
            || targets.len() != self.rows
            || self.generation != generation()
            || !current_thread_producers_only()
            || !crate::players::session().is_some_and(|(_, epoch)| epoch == self.epoch)
        {
            return false;
        }
        let Ok(mut state) = history().try_lock() else {
            return false;
        };
        if state.generation != self.generation {
            return false;
        }
        // Verify every sealed version before changing any witness. No pose,
        // serial, frame, emote phase or replay timestamp is recalculated here.
        for saved in &self.records {
            let Some(record) = state.records.iter().find(|record| record.key == saved.key) else {
                return false;
            };
            if record.serial != saved.serial
                || record.frame != saved.frame
                || record.conflicted
                || !record.pending.is_empty()
                || record.witness.as_ref() != Some(&saved.witness)
                || record
                    .pose
                    .as_ref()
                    .map(|pose| pose.native.map(f32::to_bits))
                    != Some(saved.native)
                || targets[saved.row].slot != saved.key.slot
            {
                return false;
            }
        }
        for saved in &self.records {
            let Some(record) = state
                .records
                .iter_mut()
                .find(|record| record.key == saved.key)
            else {
                return false;
            };
            record.witness = Some(targets[saved.row].witness.clone());
        }
        if self.generation != generation() || !current_thread_producers_only() {
            return false;
        }
        RELOCATED.fetch_add(self.records.len() as u64, Ordering::Relaxed);
        self.closed = true;
        true
    }
}
impl Drop for FlatRelocation {
    fn drop(&mut self) {
        if !self.closed {
            invalidate_opaque_owner(self.owner);
        }
    }
}
impl FlatProducerRelocation {
    pub(crate) fn retained_bytes(&self) -> usize {
        self.ticket.retained_bytes()+std::mem::size_of::<ProducerWrites>()+2*std::mem::size_of::<usize>()
            +self.writes.borrow().records.capacity()*std::mem::size_of::<RelocatedRecord>()
            // These keys can outlive their history entries during the original.
            // Reserve their largest admitted owned payload before any writes.
            +MAX_PRODUCER_WRITES*(std::mem::size_of::<Key>()+std::mem::size_of::<Slot>()
                +4*std::mem::size_of::<usize>()+256)
    }
    pub(crate) fn commit(&mut self, targets: &[FlatProducerTarget]) -> bool {
        let ticket = &mut self.ticket;
        if ticket.closed
            || targets.len() > 1024
            || targets.windows(2).any(|pair| pair[0].slot >= pair[1].slot)
            || ticket.generation != generation()
            || !current_thread_producers_only()
            || !crate::players::session().is_some_and(|(_, epoch)| epoch == ticket.epoch)
        {
            return false;
        }
        let Ok(writes) = self.writes.try_borrow() else {
            return false;
        };
        if writes.failed || writes.generation != ticket.generation {
            return false;
        }
        let Ok(mut state) = history().try_lock() else {
            return false;
        };
        if state.generation != ticket.generation {
            return false;
        }
        for target in targets {
            if let Some((witness, _)) = &target.value
                && (witness.layout != 1 || witness.head != writes.head)
            {
                return false;
            }
        }
        let mut moved = 0u64;
        let thread = opaque(std::thread::current().id());
        for (saved, completed_write) in ticket
            .records
            .iter()
            .map(|saved| (saved, false))
            .chain(writes.records.iter().map(|saved| (saved, true)))
        {
            let Some(record) = state
                .records
                .iter_mut()
                .find(|record| record.key == saved.key)
            else {
                continue;
            };
            // A newer child write supersedes the old seal. Its own successful
            // completion supplies a separate seal below; no revoked record is
            // restored and no serial/clock is inferred from the final bytes.
            // A completed child may supersede a still-open ancestor ticket for
            // this same slot. The whole native producer has already returned;
            // permit only those strictly older tickets on this thread. Reads
            // remain barred until every producer and pending ticket closes.
            if record.serial != saved.serial
                || record.frame != saved.frame
                || record.conflicted
                || record.pending.iter().any(|pending| {
                    !completed_write || pending.serial >= saved.serial || pending.thread != thread
                })
                || record
                    .pose
                    .as_ref()
                    .map(|pose| pose.native.map(f32::to_bits))
                    != Some(saved.native)
            {
                continue;
            }
            let Some(current) = record.witness.as_ref() else {
                continue;
            };
            if current.layout != 1 || current.head != saved.witness.head {
                continue;
            }
            let value = targets
                .binary_search_by(|target| target.slot.as_ref().cmp(saved.key.slot.as_ref()))
                .ok()
                .and_then(|index| targets[index].value.as_ref());
            match value {
                Some((witness, native)) if native.map(f32::to_bits) == saved.native => {
                    // A nested checked reallocator may already have updated
                    // this witness. Preserve its clock and move it to the final
                    // logical slot position without merging context identities.
                    if current != witness {
                        record.witness = Some(witness.clone());
                        moved += 1;
                    }
                }
                _ => {
                    record.witness = None;
                    record.pose = None;
                }
            }
        }
        if ticket.generation != generation() || !current_thread_producers_only() {
            return false;
        }
        RELOCATED.fetch_add(moved, Ordering::Relaxed);
        ticket.closed = true;
        true
    }
}
impl Drop for FlatProducerRelocation {
    fn drop(&mut self) {
        let _ = FLAT_PRODUCER_WRITES.try_with(|journals| {
            if let Ok(mut journals) = journals.try_borrow_mut() {
                journals.retain(|journal| {
                    journal.strong_count() != 0 && !journal.ptr_eq(&Rc::downgrade(&self.writes))
                });
            }
        });
    }
}
pub(crate) struct Ticket {
    generation: u64,
    key: Arc<Key>,
    serial: u64,
    closed: bool,
}
pub(crate) fn reserve(key: Arc<Key>, serial: u64, frame: u64) -> Option<Ticket> {
    let generation = generation();
    if generation == 0 || serial == 0 {
        return None;
    }
    let Ok(mut history) = history().try_lock() else {
        invalidate();
        return None;
    };
    history.sync(generation);
    // The native cache key has no render-context field. A write through a
    // different context still replaces this owner's same physical cache slot;
    // an equal matrix must not preserve the former context's emote clock.
    for other in history.records.iter_mut().filter(|record| {
        record.key.owner == key.owner && record.key.slot == key.slot && record.key != key
    }) {
        if other.witness.is_some() || other.pose.is_some() {
            CROSS_CONTEXT_INVALIDATIONS.fetch_add(1, Ordering::Relaxed);
        }
        other.witness = None;
        other.pose = None;
        if !other.pending.is_empty() {
            other.conflicted = true;
        }
    }
    if history
        .records
        .iter()
        .map(|record| record.pending.len())
        .sum::<usize>()
        >= MAX_PENDING
    {
        drop(history);
        invalidate();
        return None;
    }
    let index = if let Some(index) = history.records.iter().position(|record| record.key == key) {
        index
    } else {
        if history.records.len() >= MAX_RECORDS {
            let oldest = history
                .records
                .iter()
                .enumerate()
                .filter(|(_, record)| record.pending.is_empty())
                .min_by_key(|(_, record)| record.serial)
                .map(|(index, _)| index);
            let Some(oldest) = oldest else {
                drop(history);
                invalidate();
                return None;
            };
            history.records.swap_remove(oldest);
        }
        let index = history.records.len();
        history.records.push(Record {
            key: key.clone(),
            serial,
            frame,
            pending: Vec::new(),
            conflicted: false,
            witness: None,
            pose: None,
        });
        index
    };
    let record = &mut history.records[index];
    let thread = opaque(std::thread::current().id());
    if record.pending.is_empty() {
        record.conflicted = false;
    } else if record
        .pending
        .iter()
        .any(|pending| pending.thread != thread)
    {
        record.conflicted = true;
    }
    // A true native write invalidates the older pose before its bytes change,
    // including when no current upload can be paired with this write.
    record.witness = None;
    record.pose = None;
    if serial <= record.serial && !record.pending.is_empty() {
        record.conflicted = true;
    }
    record.serial = record.serial.max(serial);
    record.frame = frame;
    record.pending.push(Pending { serial, thread });
    if history.bytes() > MAX_BYTES || generation != self::generation() {
        drop(history);
        invalidate();
        return None;
    }
    Some(Ticket {
        generation,
        key,
        serial,
        closed: false,
    })
}
impl Ticket {
    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }
    pub(crate) fn complete(&mut self, witness: Witness, pose: Pose) -> bool {
        if self.closed {
            return false;
        }
        self.closed = true;
        finish(
            self.generation,
            &self.key,
            self.serial,
            Some((witness, pose)),
        )
    }
}
fn finish(generation: u64, key: &Arc<Key>, serial: u64, value: Option<(Witness, Pose)>) -> bool {
    if generation == 0 || generation != self::generation() {
        return false;
    }
    let Ok(mut history) = history().try_lock() else {
        invalidate();
        return false;
    };
    history.sync(generation);
    let Some(record) = history.records.iter_mut().find(|record| &record.key == key) else {
        return false;
    };
    let Some(index) = record
        .pending
        .iter()
        .position(|pending| pending.serial == serial)
    else {
        return false;
    };
    record.pending.swap_remove(index);
    if record.serial != serial || record.conflicted {
        return false;
    }
    let Some((witness, pose)) = value else {
        return false;
    };
    if !crate::native_emote_history::valid_pose(&pose) {
        return false;
    }
    record.witness = Some(witness);
    record.pose = Some(pose);
    if !journal_completed_write(generation, record) {
        drop(history);
        invalidate();
        return false;
    }
    if history.bytes() > MAX_BYTES || generation != self::generation() {
        drop(history);
        invalidate();
        return false;
    }
    COMMITTED.fetch_add(1, Ordering::Relaxed);
    true
}
impl Drop for Ticket {
    fn drop(&mut self) {
        if !self.closed {
            finish(self.generation, &self.key, self.serial, None);
        }
    }
}
#[derive(Clone, PartialEq)]
pub(crate) struct Receipt {
    generation: u64,
    serial: u64,
    key: Arc<Key>,
    witness: Witness,
    pub pose: Pose,
}
impl Receipt {
    pub(crate) fn current(&self) -> bool {
        if self.generation == 0
            || self.generation != generation()
            || ACTIVE_PRODUCERS.load(Ordering::Acquire) != 0
            || !crate::players::session().is_some_and(|(_, epoch)| epoch == self.key.epoch)
        {
            return false;
        }
        let Ok(history) = history().try_lock() else {
            return false;
        };
        if history.generation != self.generation {
            return false;
        }
        let Some(record) = history.records.iter().find(|record| record.key == self.key) else {
            return false;
        };
        record.serial == self.serial
            && !record.conflicted
            && record.pending.is_empty()
            && record.witness.as_ref() == Some(&self.witness)
            && record.pose.as_ref() == Some(&self.pose)
            && self.generation == generation()
            && ACTIVE_PRODUCERS.load(Ordering::Acquire) == 0
    }
}
/// Requires a freshly guarded native cache witness, not just an equal matrix.
pub(crate) fn read_pose(
    key: &Arc<Key>,
    frame: u64,
    witness: &Witness,
    native: &[f32; 16],
) -> Option<Receipt> {
    let generation = generation();
    if generation == 0 || ACTIVE_PRODUCERS.load(Ordering::Acquire) != 0 {
        return None;
    }
    let mut history = history().try_lock().ok()?;
    history.sync(generation);
    let record = history.records.iter().find(|record| &record.key == key)?;
    if record.conflicted
        || !record.pending.is_empty()
        || record.frame > frame
        || record.witness.as_ref() != Some(witness)
    {
        return None;
    }
    let pose = record.pose.as_ref()?;
    if !pose
        .native
        .iter()
        .zip(native)
        .all(|(a, b)| a.to_bits() == b.to_bits())
    {
        return None;
    }
    let receipt = Receipt {
        generation,
        serial: record.serial,
        key: record.key.clone(),
        witness: witness.clone(),
        pose: pose.clone(),
    };
    (generation == self::generation() && ACTIVE_PRODUCERS.load(Ordering::Acquire) == 0)
        .then_some(receipt)
}
pub(crate) fn status() -> serde_json::Value {
    let generation = generation();
    let (records, pending, bytes) = history()
        .try_lock()
        .ok()
        .map(|mut history| {
            history.sync(generation);
            (
                history.records.len(),
                history
                    .records
                    .iter()
                    .map(|record| record.pending.len())
                    .sum::<usize>(),
                history.bytes(),
            )
        })
        .unwrap_or((0, 0, 0));
    serde_json::json!({"generation":generation,"generationExhausted":generation==0,
        "invalidations":INVALIDATIONS.load(Ordering::Relaxed),"committedCacheWrites":COMMITTED.load(Ordering::Relaxed),
        "ownerInvalidations":OWNER_INVALIDATIONS.load(Ordering::Relaxed),
        "relocatedFlatCachePoses":RELOCATED.load(Ordering::Relaxed),
        "crossContextCacheWriteInvalidations":CROSS_CONTEXT_INVALIDATIONS.load(Ordering::Relaxed),
        "records":records,"pendingWrites":pending,"retainedBytes":bytes,
        "activeNativeProducers":ACTIVE_PRODUCERS.load(Ordering::Acquire),
        "maxRecords":MAX_RECORDS,"maxPendingWrites":MAX_PENDING,"maxBytes":MAX_BYTES,
        "maxJournaledCompletedChildWrites":MAX_PRODUCER_WRITES,
        "retainsNativeAddresses":false,"crossThreadConflictsRejectHistory":true,
        "historicalConsumerImplemented":true,"cacheLifetimeVerified":false,"nativeRenderingVerified":false})
}
