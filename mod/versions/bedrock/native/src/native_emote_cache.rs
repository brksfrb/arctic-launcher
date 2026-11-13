//! Observe actual native cache writes and their matching current upload.
//! Historical replacement requires checked consumer branches and writer copies;
//! the latest diagnostic JSON is never a source of upload authorization.
use crate::native_scene_layout::{self as scene, EmoteCacheStorage};
use crate::world_hooks::{CacheObserverBinding, read};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
};

static BINDING: OnceLock<CacheObserverBinding> = OnceLock::new();
const MAX_DEPTH: usize = 256;
#[path = "native_emote_cache_flat.rs"]
mod flat;
#[path = "native_emote_cache_lifetime.rs"]
mod lifetime;
#[path = "native_emote_cache_producer_storage.rs"]
mod producer_storage;
#[path = "native_emote_cache_relocation.rs"]
mod relocation;
type Produce = unsafe extern "system" fn(usize, usize, usize, usize, usize);
type Predicate = unsafe extern "system" fn(usize, usize, usize) -> u8;
type Erase = unsafe extern "system" fn(usize);
type Prepare = unsafe extern "system" fn(usize, usize, usize, usize, usize, u8);
type Lookup = unsafe extern "system" fn(usize, usize, usize) -> usize;
static PRODUCER: AtomicUsize = AtomicUsize::new(0);
static PREDICATE: AtomicUsize = AtomicUsize::new(0);
static ERASE: AtomicUsize = AtomicUsize::new(0);
static PREPARE: AtomicUsize = AtomicUsize::new(0);
static LOOKUP: AtomicUsize = AtomicUsize::new(0);
static ENABLED: AtomicBool = AtomicBool::new(false);
static INSTALL: OnceLock<Result<(), String>> = OnceLock::new();
static SERIAL: AtomicU64 = AtomicU64::new(0);
static CALLS: AtomicU64 = AtomicU64::new(0);
static PAIRED: AtomicU64 = AtomicU64::new(0);
static REJECTED: AtomicU64 = AtomicU64::new(0);
static LAST: Mutex<Option<(u64, Value)>> = Mutex::new(None);

fn serial() -> Option<u64> {
    SERIAL
        .try_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_add(1))
        .ok()?
        .checked_add(1)
}
fn word(bytes: &[u8], at: usize) -> Option<usize> {
    usize::try_from(u64::from_le_bytes(
        bytes.get(at..at.checked_add(8)?)?.try_into().ok()?,
    ))
    .ok()
}
fn integer(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}
#[derive(Default)]
struct Reads {
    guards: Vec<(usize, Vec<u8>)>,
    bytes: usize,
}
impl Reads {
    fn copy(&mut self, address: usize, length: usize) -> Option<Vec<u8>> {
        if address < 0x10000
            || address.checked_add(length).is_none()
            || self.guards.len() >= 2048
            || self.bytes.checked_add(length)? > 128 * 1024
        {
            return None;
        }
        let mut bytes = vec![0; length];
        read(address, &mut bytes).ok()?;
        self.bytes += length;
        self.guards.push((address, bytes.clone()));
        Some(bytes)
    }
    fn unchanged(&self) -> Option<()> {
        for (address, bytes) in &self.guards {
            let mut again = vec![0; bytes.len()];
            read(*address, &mut again).ok()?;
            if again != *bytes {
                return None;
            }
        }
        Some(())
    }
    fn vector(&mut self, address: usize, stride: usize, max: usize) -> Option<(usize, usize)> {
        let bytes = self.copy(address, 24)?;
        let begin = word(&bytes, 0)?;
        let end = word(&bytes, 8)?;
        let capacity = word(&bytes, 16)?;
        let length = end.checked_sub(begin)?;
        if !length.is_multiple_of(stride)
            || length / stride > max
            || capacity < end
            || capacity.checked_sub(begin)? > stride.checked_mul(4096)?
            || length != 0 && begin < 0x10000
        {
            return None;
        }
        Some((begin, length / stride))
    }
    fn key(&mut self, address: usize) -> Option<(u64, String)> {
        let bytes = self.copy(address, 40)?;
        self.key_bytes(&bytes)
    }
    fn key_bytes(&mut self, bytes: &[u8]) -> Option<(u64, String)> {
        let bytes = bytes.get(..40)?;
        let hash = u64::from_le_bytes(bytes[..8].try_into().ok()?);
        let string = &bytes[8..];
        let length = word(string, 16)?;
        let capacity = word(string, 24)?;
        if length > 128 || length > capacity || capacity > 4096 {
            return None;
        }
        let text = if capacity < 16 {
            string.get(..length.checked_add(1)?)?.to_vec()
        } else {
            self.copy(word(string, 0)?, length.checked_add(1)?)?
        };
        if text.last() != Some(&0) {
            return None;
        }
        Some((hash, std::str::from_utf8(&text[..length]).ok()?.to_owned()))
    }
}
fn invalidate_owner(params: usize) {
    let owner = (|| {
        let mut bytes = [0; 8];
        read(params.checked_add(0x38)?, &mut bytes).ok()?;
        word(&bytes, 0)
    })();
    // The pinned producer explicitly skips a null owner and its children use
    // these same params. A known null is not an unidentifiable cache write.
    if owner == Some(0) {
        return;
    }
    if let Some(owner) = owner.filter(|owner| (0x10000..0x0000_8000_0000_0000).contains(owner)) {
        crate::native_transform::invalidate_owner_frame_uploads(owner);
        crate::native_emote_cache_history::invalidate_owner(owner);
    } else {
        crate::native_emote_cache_history::invalidate();
    }
}
struct PartInput {
    part: usize,
    group: usize,
    owner: usize,
    slot: Arc<crate::native_emote_history::Slot>,
    key: Arc<crate::native_emote_cache_history::Key>,
    native: [f32; 16],
    native_address: usize,
    pose: Option<crate::native_emote_history::Pose>,
}
struct Input {
    stamp: crate::native_palette_writer::Stamp,
    owner: crate::native_cache_binding::Binding,
    // None is an explicitly observed hidden part, not an unknown source.
    part: Option<PartInput>,
    guards: Reads,
}
impl Input {
    fn copy(part: usize, params: usize, bones: usize, geometry: usize) -> Option<Self> {
        let stamp = crate::native_transform::palette_writer_scope()?;
        let owner_binding = crate::native_cache_binding::Binding::copy(&stamp, params, geometry)?;
        let mut guards = Reads::default();
        // The header may be a caller-owned native stack span. Only its heap
        // vector can authorize the current component's indexed bone matrix.
        let span = guards.copy(bones, 16)?;
        if !owner_binding.matches_span(&span) {
            return None;
        }
        let (parts, count) = guards.vector(geometry.checked_add(0x38)?, 0x240, 256)?;
        let offset = part.checked_sub(parts)?;
        if !offset.is_multiple_of(0x240) || offset / 0x240 >= count {
            return None;
        }
        // The original skips a hidden part before evaluating its predicate.
        if guards.copy(part.checked_add(0x1b8)?, 1)?.as_slice() != [0] {
            guards.unchanged()?;
            if crate::native_transform::palette_writer_scope().as_ref() != Some(&stamp) {
                return None;
            }
            return owner_binding.valid().then_some(Self {
                stamp,
                owner: owner_binding,
                part: None,
                guards,
            });
        }
        let indices = guards.copy(part.checked_add(0x84)?, 16)?;
        let group = integer(&indices, 0)?;
        let palette = integer(&indices, 4)?;
        let index = usize::try_from(integer(&indices, 12)?).ok()?;
        let owner = usize::try_from(integer(&indices, 8)?).ok()?;
        if group >= 64 || palette >= 256 || owner >= 64 {
            return None;
        }
        let (groups, group_count) = guards.vector(geometry.checked_add(0x88)?, 0x60, 64)?;
        if usize::try_from(group).ok()? >= group_count {
            return None;
        }
        let group_address = groups.checked_add(usize::try_from(group).ok()?.checked_mul(0x60)?)?;
        let (hash, name) = guards.key(part)?;
        let slot = Arc::new(crate::native_emote_history::Slot {
            hash,
            name,
            group,
            palette,
        });
        let bone_count = word(&span, 0)?;
        if bone_count == 0 || bone_count > 256 || index >= bone_count {
            return None;
        }
        let address = word(&span, 8)?
            .checked_add(index.checked_mul(0xe8)?)?
            .checked_add(0x38)?;
        let bytes = guards.copy(address, 64)?;
        let native = std::array::from_fn(|i| {
            f32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().expect("four bytes"))
        });
        let key = crate::native_emote_cache_history::Key::copy(&stamp, slot.clone())?;
        // Observe writes without a render receipt too: they must invalidate
        // older supplied poses even when the native matrix remains identical.
        let pose = crate::native_transform::frame_upload(
            &stamp,
            &owner_binding.identity(),
            part,
            &slot,
            &native,
        );
        guards.unchanged()?;
        if !owner_binding.valid()
            || crate::native_transform::palette_writer_scope().as_ref() != Some(&stamp)
        {
            return None;
        }
        Some(Self {
            stamp,
            owner: owner_binding,
            part: Some(PartInput {
                part,
                group: group_address,
                owner,
                slot,
                key,
                native,
                native_address: address,
                pose,
            }),
            guards,
        })
    }
    fn cached(&self, generation: u64) -> Option<crate::native_emote_cache_history::Witness> {
        if generation == 0 || generation != crate::native_emote_cache_history::generation() {
            return None;
        }
        let part = self.part.as_ref()?;
        if !self.owner.valid() {
            return None;
        }
        self.guards.unchanged()?;
        let snapshot = CacheSnapshot::copy(self.stamp.actor, &part.slot, None)?;
        if !same_matrix(&snapshot.native, &part.native) {
            return None;
        }
        snapshot.guards.unchanged()?;
        self.guards.unchanged()?;
        if crate::native_transform::frame_upload(
            &self.stamp,
            &self.owner.identity(),
            part.part,
            &part.slot,
            &part.native,
        ) != part.pose
            || !self.owner.valid()
            || crate::native_transform::palette_writer_scope().as_ref() != Some(&self.stamp)
            || generation != crate::native_emote_cache_history::generation()
        {
            return None;
        }
        Some(snapshot.witness)
    }
}
fn same_matrix(a: &[f32; 16], b: &[f32; 16]) -> bool {
    a.iter().zip(b).all(|(a, b)| a.to_bits() == b.to_bits())
}
struct CacheSnapshot {
    guards: Reads,
    native: [f32; 16],
    witness: crate::native_emote_cache_history::Witness,
}
impl CacheSnapshot {
    fn retained_bytes(&self) -> usize {
        self.guards.guards.capacity() * std::mem::size_of::<(usize, Vec<u8>)>()
            + self
                .guards
                .guards
                .iter()
                .map(|(_, bytes)| bytes.capacity())
                .sum::<usize>()
    }
    fn copy(
        actor: usize,
        slot: &crate::native_emote_history::Slot,
        expected_node: Option<usize>,
    ) -> Option<Self> {
        if scene::emote_cache::STORAGE == EmoteCacheStorage::FlatParallelVectors {
            // A tree node is not a flat-cache entry or a matrix pointer.
            if expected_node.is_some() {
                return None;
            }
            let lookup = flat::Lookup::copy(actor, slot)?;
            let flat::Lookup {
                guards,
                value: Some(value),
            } = lookup
            else {
                return None;
            };
            return Some(Self {
                guards,
                native: value.native,
                witness: value.witness,
            });
        }
        let mut guards = Reads::default();
        let map = guards.copy(actor.checked_add(scene::actor::CACHE)?, 16)?;
        let head = word(&map, 0)?;
        let count = word(&map, 8)?;
        if count == 0 || count > 256 {
            return None;
        }
        let sentinel = guards.copy(head, 32)?;
        if sentinel[0x19] != 1 {
            return None;
        }
        let mut pending = vec![(word(&sentinel, 8)?, head)];
        let mut seen = HashSet::new();
        let mut found = None;
        while let Some((node, parent)) = pending.pop() {
            if node == head {
                continue;
            }
            if seen.len() >= count || !seen.insert(node) {
                return None;
            }
            let links = guards.copy(node, 32)?;
            if links[0x19] != 0 || word(&links, 8)? != parent {
                return None;
            }
            pending.push((word(&links, 0)?, node));
            pending.push((word(&links, 16)?, node));
            let (hash, name) = guards.key(node.checked_add(0x20)?)?;
            if hash != slot.hash || name != slot.name {
                continue;
            }
            if found.replace(node).is_some() {
                return None;
            }
        }
        if seen.len() != count {
            return None;
        }
        let node = found?;
        if expected_node.is_some_and(|expected| expected != node) {
            return None;
        }
        let (groups, group_count) = guards.vector(node.checked_add(0x50)?, 24, 64)?;
        let group = usize::try_from(slot.group).ok()?;
        if group >= group_count {
            return None;
        }
        let (matrices, matrix_count) =
            guards.vector(groups.checked_add(group.checked_mul(24)?)?, 64, 256)?;
        let palette = usize::try_from(slot.palette).ok()?;
        if palette >= matrix_count {
            return None;
        }
        let matrix = guards.copy(matrices.checked_add(palette.checked_mul(64)?)?, 64)?;
        let native = std::array::from_fn(|i| {
            f32::from_le_bytes(matrix[i * 4..i * 4 + 4].try_into().expect("four bytes"))
        });
        guards.unchanged()?;
        Some(Self {
            guards,
            native,
            witness: crate::native_emote_cache_history::Witness::copy(head, node, groups, matrices),
        })
    }
}
struct Event {
    id: u64,
    args: [usize; 3],
    mask: usize,
    frame: u64,
    key: Option<Arc<crate::native_emote_cache_history::Key>>,
    predicate: Option<(u64, u8)>,
    ambiguous: bool,
    ticket: Option<crate::native_emote_cache_history::Ticket>,
}
const MAX_CONSUMER_BYTES: usize = 1024 * 1024;
struct Consumer {
    input: Input,
    predicate_key: usize,
    lookups: usize,
    frozen: bool,
    cached_branch: Option<bool>,
    ambiguous: bool,
    used: bool,
    snapshot: Option<CacheSnapshot>,
    receipt: Option<crate::native_emote_cache_history::Receipt>,
    flat_lookup: Option<flat::Lookup>,
}
thread_local! {
    static CONSUMERS: RefCell<Vec<Option<Consumer>>> = const { RefCell::new(Vec::new()) };
    static SKIPPED_CONSUMERS: Cell<usize> = const { Cell::new(0) };
    static CONSUMER_BYTES: Cell<usize> = const { Cell::new(0) };
}
fn discard_snapshot(consumer: &mut Consumer) {
    if let Some(snapshot) = consumer.snapshot.take() {
        CONSUMER_BYTES
            .with(|bytes| bytes.set(bytes.get().saturating_sub(snapshot.retained_bytes())));
    }
    if let Some(lookup) = consumer.flat_lookup.take() {
        CONSUMER_BYTES.with(|bytes| bytes.set(bytes.get().saturating_sub(lookup.retained_bytes())));
    }
}
struct ConsumerScope {
    pushed: bool,
    skipped: bool,
}
impl ConsumerScope {
    fn begin(
        part: usize,
        params: usize,
        bones: usize,
        key: usize,
        geometry: usize,
        history: u8,
    ) -> Self {
        let input = if crate::native_body::enabled() && history != 0 {
            Input::copy(part, params, bones, geometry)
        } else {
            None
        };
        let consumer = input.map(|input| Consumer {
            input,
            predicate_key: key,
            lookups: 0,
            frozen: false,
            cached_branch: None,
            ambiguous: false,
            used: false,
            snapshot: None,
            receipt: None,
            flat_lookup: None,
        });
        let pushed = CONSUMERS
            .try_with(|consumers| {
                let Ok(mut consumers) = consumers.try_borrow_mut() else {
                    return false;
                };
                if consumers.len() >= MAX_DEPTH {
                    return false;
                }
                consumers.push(consumer);
                true
            })
            .unwrap_or(false);
        if !pushed {
            SKIPPED_CONSUMERS.with(|skipped| skipped.set(skipped.get().saturating_add(1)));
        }
        Self {
            pushed,
            skipped: !pushed,
        }
    }
    fn masked() -> Self {
        SKIPPED_CONSUMERS.with(|skipped| skipped.set(skipped.get().saturating_add(1)));
        Self {
            pushed: false,
            skipped: true,
        }
    }
}
impl Drop for ConsumerScope {
    fn drop(&mut self) {
        if self.pushed {
            let _ = CONSUMERS.try_with(|consumers| {
                let Ok(mut consumers) = consumers.try_borrow_mut() else {
                    return;
                };
                if let Some(Some(mut consumer)) = consumers.pop() {
                    discard_snapshot(&mut consumer);
                }
            });
        }
        if self.skipped {
            SKIPPED_CONSUMERS.with(|skipped| skipped.set(skipped.get().saturating_sub(1)));
        }
    }
}
fn freeze_consumer(part: usize, params: usize, key: usize) {
    if SKIPPED_CONSUMERS.with(Cell::get) != 0 {
        return;
    }
    let _ = CONSUMERS.try_with(|consumers| {
        let Ok(mut consumers) = consumers.try_borrow_mut() else {
            return;
        };
        let Some(consumer) = consumers.last_mut().and_then(Option::as_mut) else {
            return;
        };
        if consumer
            .input
            .part
            .as_ref()
            .is_none_or(|input| input.part != part)
            || consumer.input.owner.params() != params
            || !consumer.input.owner.valid()
            || consumer.predicate_key != key
        {
            return;
        }
        if consumer.frozen {
            consumer.ambiguous = true;
            return;
        }
        consumer.frozen = true;
        let Some(binding) = BINDING.get() else {
            consumer.ambiguous = true;
            return;
        };
        if scene::emote_cache::STORAGE == EmoteCacheStorage::FlatParallelVectors {
            let copied = (|| {
                if binding.common_predicate_return.is_none()
                    || crate::world_hooks::game_callers().first().copied()
                        != binding.common_predicate_return
                    || consumer.lookups != 0
                {
                    return None;
                }
                consumer.input.guards.unchanged()?;
                let input = consumer.input.part.as_ref()?;
                let lookup = flat::Lookup::copy(consumer.input.stamp.actor, &input.slot)?;
                let receipt = lookup.value.as_ref().and_then(|value| {
                    crate::native_emote_cache_history::read_pose(
                        &input.key,
                        consumer.input.stamp.frame,
                        &value.witness,
                        &value.native,
                    )
                });
                let size = lookup.retained_bytes();
                let total = CONSUMER_BYTES.with(Cell::get).checked_add(size)?;
                if total > MAX_CONSUMER_BYTES || !consumer.input.owner.valid() {
                    return None;
                }
                consumer.input.guards.unchanged()?;
                lookup.guards.unchanged()?;
                consumer.cached_branch = Some(lookup.value.is_some());
                consumer.receipt = receipt;
                consumer.flat_lookup = Some(lookup);
                CONSUMER_BYTES.with(|bytes| bytes.set(total));
                Some(())
            })();
            if copied.is_none() {
                consumer.ambiguous = true;
            }
            return;
        }
        // Require the actual pinned branch's predicate call site. Lost lookup
        // observations cannot turn an equal historical matrix into a fallback.
        let Some(tree) = binding.tree else {
            consumer.ambiguous = true;
            return;
        };
        consumer.cached_branch = match crate::world_hooks::game_callers().first().copied() {
            Some(site) if site == tree.fallback_predicate_return && consumer.lookups < 3 => {
                Some(false)
            }
            Some(site) if site == tree.cached_predicate_return && consumer.lookups == 3 => {
                Some(true)
            }
            _ => {
                consumer.ambiguous = true;
                None
            }
        };
    });
}
fn observe_lookup(map: usize, output: usize, part: usize, result: usize) {
    if scene::emote_cache::STORAGE != EmoteCacheStorage::Tree {
        return;
    }
    let Some(tree) = BINDING.get().and_then(|binding| binding.tree) else {
        return;
    };
    if SKIPPED_CONSUMERS.with(Cell::get) != 0 {
        return;
    }
    let _ = CONSUMERS.try_with(|consumers| {
        let Ok(mut consumers) = consumers.try_borrow_mut() else {
            return;
        };
        let Some(consumer) = consumers.last_mut().and_then(Option::as_mut) else {
            return;
        };
        let Some(input) = consumer.input.part.as_ref() else {
            return;
        };
        if input.part != part
            || consumer.input.stamp.actor.checked_add(scene::actor::CACHE) != Some(map)
        {
            return;
        }
        if consumer.frozen || consumer.used {
            consumer.ambiguous = true;
            return;
        }
        let expected = tree.lookup_returns.get(consumer.lookups).copied();
        if expected.is_none() || crate::world_hooks::game_callers().first().copied() != expected {
            consumer.ambiguous = true;
            return;
        }
        let actor = consumer.input.stamp.actor;
        let frame = consumer.input.stamp.frame;
        let slot = input.slot.clone();
        let key = input.key.clone();
        consumer.lookups += 1;
        if consumer.lookups > 3 || result != output {
            consumer.ambiguous = true;
            return;
        }
        let copied = (|| {
            let mut node = [0; 8];
            let mut inserted = [0; 1];
            read(output, &mut node).ok()?;
            read(output.checked_add(8)?, &mut inserted).ok()?;
            if inserted != [0] {
                return None;
            }
            if consumer.lookups != 3 {
                return Some(());
            }
            let snapshot = CacheSnapshot::copy(actor, &slot, Some(word(&node, 0)?))?;
            let receipt = crate::native_emote_cache_history::read_pose(
                &key,
                frame,
                &snapshot.witness,
                &snapshot.native,
            );
            let size = snapshot.retained_bytes();
            let total = CONSUMER_BYTES.with(Cell::get).checked_add(size)?;
            if total > MAX_CONSUMER_BYTES {
                return None;
            }
            CONSUMER_BYTES.with(|bytes| bytes.set(total));
            consumer.snapshot = Some(snapshot);
            consumer.receipt = receipt;
            Some(())
        })();
        if copied.is_none() {
            consumer.ambiguous = true;
        }
    });
}
/// Called synchronously while the exact part preparer and its input spans live.
/// No stack or native-cache address escapes in the returned owned proof.
pub(crate) fn previous_for_writer(
    part: usize,
    group: usize,
    owner: usize,
    slot: usize,
    current: &[u8; 64],
    previous: &[u8; 64],
    previous_source: Option<usize>,
) -> Option<crate::native_palette_writer::Candidate> {
    if SKIPPED_CONSUMERS.with(Cell::get) != 0 {
        return None;
    }
    CONSUMERS
        .try_with(|consumers| {
            let mut consumers = consumers.try_borrow_mut().ok()?;
            let consumer = consumers.last_mut()?.as_mut()?;
            let input = consumer.input.part.as_ref()?;
            if consumer.ambiguous
                || !consumer.frozen
                || consumer.used
                || input.part != part
                || input.group != group
                || input.owner != owner
                || usize::try_from(input.slot.palette).ok()? != slot
                || crate::native_transform::palette_writer_scope().as_ref()
                    != Some(&consumer.input.stamp)
                || crate::world_hooks::game_callers().first().copied()
                    != Some(BINDING.get()?.writer_return)
            {
                return None;
            }
            consumer.used = true;
            let result = (|| {
                if !consumer.input.owner.valid() {
                    return None;
                }
                consumer.input.guards.unchanged()?;
                for (i, value) in input.native.iter().enumerate() {
                    if current[i * 4..i * 4 + 4] != value.to_le_bytes() {
                        return None;
                    }
                }
                let prior = if scene::emote_cache::STORAGE == EmoteCacheStorage::FlatParallelVectors
                {
                    let lookup = consumer.flat_lookup.as_ref()?;
                    lookup.guards.unchanged()?;
                    if consumer.lookups != 0 {
                        return None;
                    }
                    if let Some(value) = lookup.value.as_ref() {
                        if consumer.cached_branch != Some(true)
                            || previous_source != Some(value.address)
                            || value.address == input.native_address
                        {
                            return None;
                        }
                        for (i, value) in value.native.iter().enumerate() {
                            if previous[i * 4..i * 4 + 4] != value.to_le_bytes() {
                                return None;
                            }
                        }
                        let receipt = consumer.receipt.as_ref()?;
                        if !receipt.current() {
                            return None;
                        }
                        crate::native_palette_writer::Candidate::Historical(receipt.clone())
                    } else {
                        if consumer.cached_branch != Some(false)
                            || previous_source != Some(input.native_address)
                            || current != previous
                        {
                            return None;
                        }
                        crate::native_palette_writer::Candidate::Current
                    }
                } else if consumer.cached_branch == Some(false) && consumer.lookups < 3 {
                    if current != previous {
                        return None;
                    }
                    crate::native_palette_writer::Candidate::Current
                } else if consumer.cached_branch == Some(true) && consumer.lookups == 3 {
                    let snapshot = consumer.snapshot.as_ref()?;
                    snapshot.guards.unchanged()?;
                    for (i, value) in snapshot.native.iter().enumerate() {
                        if previous[i * 4..i * 4 + 4] != value.to_le_bytes() {
                            return None;
                        }
                    }
                    let receipt = consumer.receipt.as_ref()?;
                    if !receipt.current() {
                        return None;
                    }
                    crate::native_palette_writer::Candidate::Historical(receipt.clone())
                } else {
                    return None;
                };
                consumer.input.guards.unchanged()?;
                if let Some(lookup) = consumer.flat_lookup.as_ref() {
                    lookup.guards.unchanged()?;
                }
                if !consumer.input.owner.valid() {
                    return None;
                }
                Some(prior)
            })();
            discard_snapshot(consumer);
            result
        })
        .ok()
        .flatten()
}
thread_local! {
    static EVENTS: RefCell<Vec<Event>> = const { RefCell::new(Vec::new()) };
    static SKIPPED: Cell<usize> = const { Cell::new(0) };
}
struct RelocationContext {
    stamp: crate::native_palette_writer::Stamp,
    event: u64,
    serial: u64,
    predicate: u8,
    generation: u64,
    key: Arc<crate::native_emote_cache_history::Key>,
    params: usize,
}
impl RelocationContext {
    fn current(&self) -> bool {
        let Some(owner_address) = self.params.checked_add(0x38) else {
            return false;
        };
        if !ENABLED.load(Ordering::Acquire)
            || !crate::native_body::enabled()
            || crate::native_transform::palette_writer_scope().as_ref() != Some(&self.stamp)
            || crate::native_emote_cache_history::generation() != self.generation
            || crate::world_hooks::pointer(owner_address) != Some(self.stamp.actor)
        {
            return false;
        }
        EVENTS
            .try_with(|events| {
                let events = events.try_borrow().ok()?;
                let event = events.last()?;
                (!event.ambiguous
                    && event.id == self.event
                    && event.args[1] == self.params
                    && event.frame == self.stamp.frame
                    && event.mask == SKIPPED.with(Cell::get)
                    && event.key.as_ref() == Some(&self.key)
                    && event.predicate == Some((self.serial, self.predicate))
                    && event
                        .ticket
                        .as_ref()
                        .is_some_and(|ticket| ticket.generation() == self.generation))
                .then_some(())
            })
            .ok()
            .flatten()
            .is_some()
    }
}
fn relocation_context(vector: usize) -> Option<RelocationContext> {
    if !ENABLED.load(Ordering::Acquire)
        || !crate::native_body::enabled()
        || !flat::selected_family()
    {
        return None;
    }
    let binding = BINDING.get()?.matrix_insert?;
    if crate::world_hooks::game_callers().first().copied() != Some(binding.producer_return) {
        return None;
    }
    let stamp = crate::native_transform::palette_writer_scope()?;
    if stamp.actor.checked_add(flat::VALUES)? != vector {
        return None;
    }
    let context = EVENTS
        .try_with(|events| {
            let events = events.try_borrow().ok()?;
            let event = events.last()?;
            let (serial, result) = event.predicate?;
            if event.ambiguous
                || result == 0
                || event.mask != SKIPPED.with(Cell::get)
                || event.frame != stamp.frame
            {
                return None;
            }
            Some(RelocationContext {
                stamp,
                event: event.id,
                serial,
                predicate: result,
                generation: event.ticket.as_ref()?.generation(),
                key: event.key.clone()?,
                params: event.args[1],
            })
        })
        .ok()
        .flatten()?;
    context.current().then_some(context)
}
struct Scope {
    id: Option<u64>,
    input: Option<Input>,
    skipped: bool,
    storage: Option<producer_storage::Observation>,
}
impl Scope {
    fn masked() -> Self {
        SKIPPED.with(|skipped| skipped.set(skipped.get().saturating_add(1)));
        Self {
            id: None,
            input: None,
            skipped: true,
            storage: None,
        }
    }
    fn begin(args: [usize; 3], bones: usize, geometry: usize) -> Self {
        let input = Input::copy(args[0], args[1], bones, geometry);
        let id = input.as_ref().and_then(|_| serial()).and_then(|id| {
            EVENTS
                .try_with(|events| {
                    let mut events = events.try_borrow_mut().ok()?;
                    if events.len() >= MAX_DEPTH {
                        return None;
                    }
                    let input = input.as_ref()?;
                    events.push(Event {
                        id,
                        args,
                        mask: SKIPPED.with(Cell::get),
                        frame: input.stamp.frame,
                        key: input.part.as_ref().map(|part| part.key.clone()),
                        predicate: None,
                        ambiguous: false,
                        ticket: None,
                    });
                    Some(id)
                })
                .ok()
                .flatten()
        });
        if id.is_none() {
            invalidate_owner(args[1]);
            SKIPPED.with(|skipped| skipped.set(skipped.get().saturating_add(1)));
        }
        // Own the event before snapshot allocation/reading can unwind. The
        // normal scope drop must remove it even when capture never completes.
        let mut scope = Self {
            id,
            input,
            skipped: id.is_none(),
            storage: None,
        };
        scope.storage =
            id.and_then(|event| producer_storage::Observation::begin(scope.input.as_ref()?, event));
        scope
    }
    fn finish(&mut self) -> Option<()> {
        let id = self.id?;
        if let Some(mut storage) = self.storage.take() {
            storage.finish(self.input.as_ref()?)?;
        }
        let mut event = EVENTS
            .try_with(|events| {
                let mut events = events.try_borrow_mut().ok()?;
                if events.last()?.id != id {
                    return None;
                }
                events.pop()
            })
            .ok()
            .flatten()?;
        self.id = None;
        if event.ambiguous {
            invalidate_owner(event.args[1]);
            return None;
        }
        let input = self.input.as_ref()?;
        let Some((serial, predicate)) = event.predicate else {
            if input.part.is_none()
                && input.owner.valid()
                && input.guards.unchanged().is_some()
                && crate::native_transform::palette_writer_scope().as_ref() == Some(&input.stamp)
            {
                return Some(());
            }
            invalidate_owner(event.args[1]);
            return None;
        };
        if predicate == 0 {
            return Some(());
        }
        let ticket = event.ticket.as_mut()?;
        let part = input.part.as_ref()?;
        let Some(pose) = part.pose.as_ref() else {
            return Some(());
        };
        let witness = input.cached(ticket.generation())?;
        if !ticket.complete(witness, pose.clone()) {
            return None;
        }
        // A parent finishes after its children. Never replace a later observed
        // predicate/write pair with that parent's older diagnostic sequence.
        if let Ok(mut last) = LAST.try_lock()
            && last.as_ref().is_none_or(|(old, _)| *old < serial)
        {
            *last = Some((
                serial,
                json!({"observedWriteSequence":serial,
                    "worldScope":input.stamp.scope,"actorId":input.stamp.actor_id,
                    "animationFrame":input.stamp.frame,"cacheKeyHash":format!("{:016x}",part.slot.hash),
                    "cacheKey":part.slot.name,"group":part.slot.group,"paletteSlot":part.slot.palette,
                    "native":pose.native,"supplied":pose.supplied,
                    "emoteToken":pose.emote.as_ref().map(|e|e.token.as_str()),
                    "timeMs":pose.emote.as_ref().map(|e|e.time_ms),
                    "seconds":pose.emote.as_ref().map(|e|e.seconds),
                    "historicalUploadAuthorized":false,"nativeRenderingVerified":false}),
            ));
        }
        PAIRED.fetch_add(1, Ordering::Relaxed);
        Some(())
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        if let Some(id) = self.id {
            let _ = EVENTS.try_with(|events| {
                let Ok(mut events) = events.try_borrow_mut() else {
                    return;
                };
                if events.last().is_some_and(|event| event.id == id) {
                    events.pop();
                } else {
                    events.clear();
                }
            });
        }
        if self.skipped {
            SKIPPED.with(|skipped| skipped.set(skipped.get().saturating_sub(1)));
        }
    }
}
unsafe extern "system" fn predicate(part: usize, params: usize, key: usize) -> u8 {
    let args = [part, params, key];
    let id = std::panic::catch_unwind(|| {
        if !ENABLED.load(Ordering::Acquire) || !crate::native_body::enabled() {
            return None;
        }
        // Freeze before the original Molang predicate can run callbacks. Only
        // the native preparer's earlier lookups establish matrix provenance.
        freeze_consumer(part, params, key);
        EVENTS
            .try_with(|events| {
                let events = events.try_borrow().ok()?;
                let event = events.last()?;
                (event.args == args && event.mask == SKIPPED.with(Cell::get)).then_some(event.id)
            })
            .ok()
            .flatten()
    })
    .ok()
    .flatten();
    let result = unsafe {
        let original: Predicate = std::mem::transmute(PREDICATE.load(Ordering::Acquire));
        original(part, params, key)
    };
    if let Some(id) = id {
        let observed = std::panic::catch_unwind(|| {
            let _ = EVENTS.try_with(|events| {
                let Ok(mut events) = events.try_borrow_mut() else {
                    return;
                };
                let Some(event) = events.last_mut() else {
                    return;
                };
                if event.id != id || event.args != args {
                    return;
                }
                if event.predicate.is_some() {
                    event.ambiguous = true;
                    return;
                }
                if let Some(serial) = serial() {
                    event.predicate = Some((serial, result));
                    if result != 0 {
                        event.ticket = event.key.as_ref().and_then(|key| {
                            crate::native_emote_cache_history::reserve(
                                key.clone(),
                                serial,
                                event.frame,
                            )
                        });
                        if event.ticket.is_none() {
                            event.ambiguous = true;
                        }
                    }
                } else {
                    event.ambiguous = true;
                }
            });
        });
        if observed.is_err() {
            invalidate_owner(params);
        }
    }
    result
}
unsafe extern "system" fn produce(
    part: usize,
    params: usize,
    bones: usize,
    key: usize,
    geometry: usize,
) {
    // Historical lookup is ineligible throughout every original producer,
    // including unsupported or unobservable producers and nested callbacks.
    let _producer = crate::native_emote_cache_history::producer();
    CALLS.fetch_add(1, Ordering::Relaxed);
    let mut scope = if ENABLED.load(Ordering::Acquire) && crate::native_body::enabled() {
        Some(
            std::panic::catch_unwind(|| Scope::begin([part, params, key], bones, geometry))
                .unwrap_or_else(|_| {
                    invalidate_owner(params);
                    Scope::masked()
                }),
        )
    } else {
        // A probe-disabled write still changes the native cache. Re-enabling
        // the observer must not resurrect an older sampled emote clock.
        invalidate_owner(params);
        None
    };
    unsafe {
        let original: Produce = std::mem::transmute(PRODUCER.load(Ordering::Acquire));
        original(part, params, bones, key, geometry);
    }
    if let Some(scope) = scope.as_mut() {
        let completed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| scope.finish()));
        if !matches!(completed, Ok(Some(()))) {
            REJECTED.fetch_add(1, Ordering::Relaxed);
        }
    }
}
unsafe extern "system" fn erase(node: usize) {
    // Includes nil-root calls from empty-map destruction, so allocator reuse
    // cannot carry a former owner's supplied pose into a new native cache.
    let _ = std::panic::catch_unwind(crate::native_emote_cache_history::invalidate);
    unsafe {
        let original: Erase = std::mem::transmute(ERASE.load(Ordering::Acquire));
        original(node);
    }
}
unsafe extern "system" fn prepare(
    part: usize,
    params: usize,
    bones: usize,
    key: usize,
    geometry: usize,
    history: u8,
) {
    let _scope = std::panic::catch_unwind(|| {
        if ENABLED.load(Ordering::Acquire) {
            ConsumerScope::begin(part, params, bones, key, geometry, history)
        } else {
            ConsumerScope::masked()
        }
    })
    .unwrap_or_else(|_| ConsumerScope::masked());
    unsafe {
        let original: Prepare = std::mem::transmute(PREPARE.load(Ordering::Acquire));
        original(part, params, bones, key, geometry, history);
    }
}
unsafe extern "system" fn lookup(map: usize, output: usize, part: usize) -> usize {
    let result = unsafe {
        let original: Lookup = std::mem::transmute(LOOKUP.load(Ordering::Acquire));
        original(map, output, part)
    };
    if !ENABLED.load(Ordering::Acquire) {
        return result;
    }
    let observed = std::panic::catch_unwind(|| observe_lookup(map, output, part, result));
    if observed.is_err() {
        let _ = CONSUMERS.try_with(|consumers| {
            if let Ok(mut consumers) = consumers.try_borrow_mut()
                && let Some(consumer) = consumers.last_mut().and_then(Option::as_mut)
            {
                consumer.ambiguous = true;
            }
        });
    }
    result
}
pub(crate) fn install(base: usize) -> Result<(), String> {
    INSTALL.get_or_init(||{
        if crate::world_hooks::image_base() != Some(base) { return Err("unvalidated cache observer image".into()); }
        // The new reader/consumer/producer bindings and destructor observers
        // do not establish complete flat-cache mutation coverage.
        if scene::emote_cache::STORAGE!=EmoteCacheStorage::Tree {
            return Err("flat emote cache mutation coverage is incomplete".into());
        }
        let binding=crate::world_hooks::cache_observer_binding()?;
        let mut hooks:Vec<(usize,*const (),&AtomicUsize,&[u8])>=Vec::new();
        if let Some(tree)=binding.tree {hooks.push((tree.erase_rva,erase as *const (),&ERASE,tree.erase_prefix()));}
        hooks.push((binding.predicate_rva,predicate as *const (),&PREDICATE,binding.predicate_prefix()));
        hooks.push((binding.producer_rva,produce as *const (),&PRODUCER,binding.producer_prefix()));
        if let Some(tree)=binding.tree {hooks.push((tree.lookup_rva,lookup as *const (),&LOOKUP,tree.lookup_prefix()));}
        hooks.push((binding.prepare_rva,prepare as *const (),&PREPARE,binding.prepare_prefix()));
        if let Some(insert)=binding.matrix_insert{hooks.push(relocation::hook(insert));}
        let mut targets=vec![0usize;hooks.len()];
        // Lifetime invalidation, cache writes and consumer provenance form one
        // observer. A partial set must not authorize any historical receipt.
        for(i,(rva,_,_,expected))in hooks.iter().enumerate(){
            let address=base.checked_add(*rva).ok_or("cache observer address overflow")?;
            let mut bytes=vec![0;expected.len()];read(address,&mut bytes)?;
            if bytes.as_slice()!=*expected{return Err(format!("cache observer {rva:x} prologue differs"));}
            targets[i]=address;
        }
        lifetime::install(base)?;
        BINDING.set(binding).map_err(|_|"cache observation installation already attempted")?;
        use hudhook::mh::{MH_CreateHook,MH_EnableHook,MH_DisableHook,MH_STATUS};
        for(i,(rva,detour,original,_))in hooks.iter().enumerate(){
            let mut trampoline=std::ptr::null_mut();
            let status=unsafe{MH_CreateHook(targets[i] as _,*detour as _,&mut trampoline)};
            if status!=MH_STATUS::MH_OK{return Err(format!("create cache observer {rva:x}: {status:?}"));}
            original.store(trampoline as usize,Ordering::Release);
        }
        for(i,target)in targets.iter().enumerate(){
            let status=unsafe{MH_EnableHook(*target as _)};
            if status!=MH_STATUS::MH_OK{
                let mut failures=Vec::new();
                for previous in targets[..i].iter().rev(){
                    let rollback=unsafe{MH_DisableHook(*previous as _)};
                    if rollback!=MH_STATUS::MH_OK{failures.push(format!("{previous:x}: {rollback:?}"));}
                }
                return Err(format!("enable cache observer {target:x}: {status:?}; rollback failures: {failures:?}"));
            }
        }
        ENABLED.store(true,Ordering::Release);Ok(())
    }).clone()
}
pub(crate) fn status() -> Value {
    json!({"installed":ENABLED.load(Ordering::Acquire),
        "producerRva":BINDING.get().map(|b|b.producer_rva),"predicateRva":BINDING.get().map(|b|b.predicate_rva),
        "storage":BINDING.get().map(|b|b.storage_name()),"experimentalEnabled":crate::native_body::enabled(),
        "calls":CALLS.load(Ordering::Relaxed),"observedCacheUploadPairs":PAIRED.load(Ordering::Relaxed),
        "rejected":REJECTED.load(Ordering::Relaxed),"maxNestedScopes":MAX_DEPTH,
        "last":LAST.try_lock().ok().and_then(|last|last.as_ref().map(|(_, value)|value.clone())),
        "retainsNativeAddressesInLastPair":false,"writesNativeCache":false,
        "eraseTreeRva":BINDING.get().and_then(|b|b.tree.map(|tree|tree.erase_rva)),"history":crate::native_emote_cache_history::status(),
        "prepareRva":BINDING.get().map(|b|b.prepare_rva),"lookupRva":BINDING.get().and_then(|b|b.tree.map(|tree|tree.lookup_rva)),
        "writerReturnRva":BINDING.get().map(|b|b.writer_return),"maxConsumerSnapshotBytes":MAX_CONSUMER_BYTES,
        "lastPairIsDiagnosticOnly":true,"persistentHistoricalLookupImplemented":true,
        "historicalUploadImplemented":true,"consumerBranchProvenanceImplemented":true,
        "childCacheOwnershipImplemented":true,"componentBoneSpanMembershipRequired":true,
        "flatCache":flat::status(),
        "destructionObservers":lifetime::status(),
        "matrixRelocation":relocation::status(),
        "producerStorage":producer_storage::status(),
        "cacheLifetimeVerified":false,"nativeRenderingVerified":false})
}
