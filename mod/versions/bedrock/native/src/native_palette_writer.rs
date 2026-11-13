//! Read-only receipts for explicit current fallbacks and observed cached poses.
//! Receipts live in ActorScope, never in serialized frames or persistent history.
use crate::native_scene_layout::palette_owner;
use crate::world_hooks::{PaletteWriterBinding, PreviousMatrixInput, read};
use std::sync::{
    OnceLock,
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};

static INSTALLED_BINDING: OnceLock<PaletteWriterBinding> = OnceLock::new();
static ENABLED: AtomicBool = AtomicBool::new(false);
const MAX_RECORDS: usize = 512;
const MAX_BYTES: usize = 256 * 1024;
type WritePalette = unsafe extern "system" fn(usize, i32, i32, u8, usize, u32, usize);
static ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static CALLS: AtomicU64 = AtomicU64::new(0);
static OBSERVED: AtomicU64 = AtomicU64::new(0);
static REJECTED: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, PartialEq)]
pub(crate) struct Stamp {
    pub actor: usize,
    pub context: usize,
    pub scope: u64,
    pub frame: u64,
    /// Process-local observation version, distinct from Bedrock's frame value.
    /// Observed frame ABA must not revive a borrow from an earlier model phase.
    pub observation: u64,
    pub actor_id: String,
}
struct Copied {
    address: usize,
    bytes: Vec<u8>,
}
impl Copied {
    fn read(address: usize, size: usize) -> Option<Self> {
        if address < 0x10000 || size > 128 || address.checked_add(size).is_none() {
            return None;
        }
        let mut bytes = vec![0; size];
        read(address, &mut bytes).ok()?;
        Some(Self { address, bytes })
    }
    fn unchanged(&self) -> bool {
        let mut after = vec![0; self.bytes.len()];
        read(self.address, &mut after).is_ok() && after == self.bytes
    }
}
fn word(bytes: &[u8], offset: usize) -> Option<usize> {
    usize::try_from(u64::from_le_bytes(
        bytes.get(offset..offset.checked_add(8)?)?.try_into().ok()?,
    ))
    .ok()
}
fn vector(bytes: &[u8], stride: usize, limit: usize) -> Option<(usize, usize)> {
    let begin = word(bytes, 0)?;
    let end = word(bytes, 8)?;
    let capacity = word(bytes, 16)?;
    let size = end.checked_sub(begin)?;
    if begin < 0x10000
        || !size.is_multiple_of(stride)
        || size / stride > limit
        || capacity < end
        || capacity.checked_sub(begin)? > stride.checked_mul(4096)?
    {
        return None;
    }
    Some((begin, size / stride))
}
struct Input {
    owner: usize,
    slot: usize,
    current: usize,
    previous: usize,
    heap: Vec<Copied>,
    inputs: Vec<Copied>,
    expected_current: [u8; 64],
    expected_previous: [u8; 64],
    prior: Option<Candidate>,
}
#[derive(Clone, PartialEq)]
pub(crate) enum Prior {
    Current,
    // Sealed only after the engine copied the corroborated input into its
    // previous palette. Later cache advancement cannot change this copy's clock.
    Historical(crate::native_emote_history::Pose),
}
pub(crate) enum Candidate {
    Current,
    Historical(crate::native_emote_cache_history::Receipt),
}
impl Candidate {
    fn current(&self) -> bool {
        match self {
            Self::Current => true,
            Self::Historical(receipt) => receipt.current(),
        }
    }
    fn seal(self) -> Prior {
        match self {
            Self::Current => Prior::Current,
            Self::Historical(receipt) => Prior::Historical(receipt.pose),
        }
    }
}
impl Input {
    fn copy(
        group: usize,
        owner_index: i32,
        slot: i32,
        update: u8,
        matrix: usize,
        previous: usize,
        previous_kind: PreviousMatrixInput,
    ) -> Option<Self> {
        let owner_index = usize::try_from(owner_index).ok()?;
        let slot = usize::try_from(slot).ok()?;
        if owner_index >= 64 || slot >= 256 {
            return None;
        }
        let groups = Copied::read(group.checked_add(0x48)?, 24)?;
        let (begin, count) = vector(&groups.bytes, 16, 64)?;
        if owner_index >= count {
            return None;
        }
        let entry = Copied::read(begin.checked_add(owner_index.checked_mul(16)?)?, 8)?;
        let owner = word(&entry.bytes, 0)?;
        let members = Copied::read(owner.checked_add(8)?, 24)?;
        let (members_begin, members_count) = vector(&members.bytes, 8, 256)?;
        let current_palette = Copied::read(owner.checked_add(palette_owner::CURRENT)?, 24)?;
        let previous_palette = Copied::read(owner.checked_add(palette_owner::PREVIOUS)?, 24)?;
        let (current_begin, current_count) = vector(&current_palette.bytes, 64, 256)?;
        let (previous_begin, previous_count) = vector(&previous_palette.bytes, 64, 256)?;
        if slot >= current_count
            || current_count != previous_count
            || current_count != members_count
        {
            return None;
        }
        let member = Copied::read(members_begin.checked_add(slot.checked_mul(8)?)?, 8)?;
        let part = word(&member.bytes, 0)?;
        let indices = Copied::read(part.checked_add(0x84)?, 16)?;
        if u32::from_le_bytes(indices.bytes[4..8].try_into().ok()?) as usize != slot
            || u32::from_le_bytes(indices.bytes[8..12].try_into().ok()?) as usize != owner_index
        {
            return None;
        }
        let current = current_begin.checked_add(slot.checked_mul(64)?)?;
        let previous_entry = previous_begin.checked_add(slot.checked_mul(64)?)?;
        let mut inputs = Vec::new();
        let (expected_current, expected_previous, prior) = if update != 0 {
            let matrix = Copied::read(matrix, 64)?;
            // The two native families keep the same argument widths, but not
            // the pointee layout of argument 7. Copy only the header bytes the
            // selected writer reads; never interpret the reference as 64 floats.
            let (header, flag_value, previous_matrix) = match previous_kind {
                PreviousMatrixInput::Inline => {
                    let flag = Copied::read(previous.checked_add(64)?, 1)?;
                    let present = flag.bytes[0];
                    (flag, present, previous)
                }
                PreviousMatrixInput::Reference => {
                    let header = Copied::read(previous, 9)?;
                    let present = header.bytes[8];
                    // An absent optional need not contain an initialized pointer.
                    let matrix = if present == 1 {
                        word(&header.bytes, 0)?
                    } else {
                        0
                    };
                    (header, present, matrix)
                }
            };
            if flag_value > 1 {
                return None;
            }
            let current: [u8; 64] = matrix.bytes.as_slice().try_into().ok()?;
            let previous = if flag_value == 1 {
                let prior = Copied::read(previous_matrix, 64)?;
                let value = prior.bytes.as_slice().try_into().ok()?;
                inputs.push(prior);
                value
            } else {
                current
            };
            inputs.push(matrix);
            inputs.push(header);
            let prior = if flag_value == 0 {
                Some(Candidate::Current)
            } else {
                crate::native_emote_cache::previous_for_writer(
                    part,
                    group,
                    owner_index,
                    slot,
                    &current,
                    &previous,
                    match previous_kind {
                        PreviousMatrixInput::Inline => None,
                        PreviousMatrixInput::Reference => Some(previous_matrix),
                    },
                )
            };
            (current, previous, prior)
        } else {
            // A skipped native write cannot inherit a prior fallback receipt.
            let current = Copied::read(current, 64)?;
            let previous = Copied::read(previous_entry, 64)?;
            (
                current.bytes.as_slice().try_into().ok()?,
                previous.bytes.as_slice().try_into().ok()?,
                None,
            )
        };
        let result = Self {
            owner,
            slot,
            current,
            previous: previous_entry,
            heap: vec![
                groups,
                entry,
                members,
                current_palette,
                previous_palette,
                member,
                indices,
            ],
            inputs,
            expected_current,
            expected_previous,
            prior,
        };
        result.unchanged().then_some(result)
    }
    fn unchanged(&self) -> bool {
        self.heap.iter().chain(&self.inputs).all(Copied::unchanged)
    }
    fn complete(self) -> Option<Record> {
        if !self.unchanged() || self.prior.as_ref().is_some_and(|prior| !prior.current()) {
            return None;
        }
        let mut current = [0; 64];
        let mut previous = [0; 64];
        read(self.current, &mut current).ok()?;
        read(self.previous, &mut previous).ok()?;
        if current != self.expected_current
            || previous != self.expected_previous
            || !self.unchanged()
            || self.prior.as_ref().is_some_and(|prior| !prior.current())
        {
            return None;
        }
        // Do not retain input source addresses: those may have pointed into the
        // writer caller's stack. Only owning heap metadata survives this call.
        Some(Record {
            owner: self.owner,
            slot: self.slot,
            current: self.current,
            previous: self.previous,
            matrix: current,
            previous_matrix: previous,
            prior: self.prior.map(Candidate::seal),
            heap: self.heap,
        })
    }
}
pub(crate) struct Record {
    owner: usize,
    slot: usize,
    current: usize,
    previous: usize,
    matrix: [u8; 64],
    previous_matrix: [u8; 64],
    prior: Option<Prior>,
    heap: Vec<Copied>,
}
impl Record {
    fn heap_bytes(&self) -> usize {
        self.heap.capacity() * std::mem::size_of::<Copied>()
            + self
                .heap
                .iter()
                .map(|copy| copy.bytes.capacity())
                .sum::<usize>()
            + self.prior.as_ref().map_or(0, |prior| match prior {
                Prior::Current => 0,
                Prior::Historical(pose) => pose
                    .emote
                    .as_ref()
                    .map_or(0, |emote| emote.token.capacity()),
            })
    }
    fn prior(&self) -> Option<Prior> {
        let prior = self.prior.as_ref()?;
        if !self.heap.iter().all(Copied::unchanged) {
            return None;
        }
        let mut current = [0; 64];
        let mut previous = [0; 64];
        if read(self.current, &mut current).is_err()
            || read(self.previous, &mut previous).is_err()
            || current != self.matrix
            || previous != self.previous_matrix
            || !self.heap.iter().all(Copied::unchanged)
        {
            return None;
        }
        Some(prior.clone())
    }
}
#[derive(Default)]
pub(crate) struct Ledger {
    records: Vec<Record>,
    invalid: bool,
}
impl Ledger {
    pub(crate) fn advance_frame(&mut self) {
        self.records.clear();
    }
    pub(crate) fn invalidate(&mut self, key: Option<(usize, usize)>) {
        if let Some((owner, slot)) = key {
            self.records.retain(|r| r.owner != owner || r.slot != slot);
        } else {
            self.records.clear();
            self.invalid = true;
        }
    }
    pub(crate) fn record(&mut self, record: Option<Record>) -> bool {
        if self.invalid {
            return false;
        }
        let Some(record) = record else {
            self.records.clear();
            self.invalid = true;
            return false;
        };
        if let Some(old) = self
            .records
            .iter_mut()
            .find(|old| old.owner == record.owner && old.slot == record.slot)
        {
            *old = record;
        } else if self.records.len() < MAX_RECORDS {
            self.records.push(record);
        } else {
            self.records.clear();
            self.invalid = true;
            return false;
        }
        let bytes = self.records.capacity() * std::mem::size_of::<Record>()
            + self.records.iter().map(Record::heap_bytes).sum::<usize>();
        if bytes > MAX_BYTES {
            self.records.clear();
            self.invalid = true;
            return false;
        }
        true
    }
    pub(crate) fn proves(&self, current: usize, previous: usize, count: usize) -> bool {
        self.previous(current, previous, count)
            .is_some_and(|entries| entries.iter().all(|entry| matches!(entry, Prior::Current)))
    }
    pub(crate) fn previous(
        &self,
        current: usize,
        previous: usize,
        count: usize,
    ) -> Option<Vec<Prior>> {
        if self.invalid || !(1..=8).contains(&count) {
            return None;
        }
        (0..count)
            .map(|index| {
                let current = current.checked_add(index * 64)?;
                let previous = previous.checked_add(index * 64)?;
                let mut found = self
                    .records
                    .iter()
                    .filter(|r| r.current == current && r.previous == previous);
                let record = found.next()?;
                if found.next().is_some() {
                    return None;
                }
                record.prior()
            })
            .collect()
    }
}
unsafe extern "system" fn write_palette(
    group: usize,
    owner: i32,
    slot: i32,
    update: u8,
    matrix: usize,
    count: u32,
    previous: usize,
) {
    CALLS.fetch_add(1, Ordering::Relaxed);
    let active = crate::native_transform::palette_write_scopes_active();
    let stamp = if active {
        std::panic::catch_unwind(crate::native_transform::palette_writer_scope)
            .ok()
            .flatten()
    } else {
        None
    };
    let input = if active {
        std::panic::catch_unwind(|| {
            Input::copy(
                group,
                owner,
                slot,
                update,
                matrix,
                previous,
                INSTALLED_BINDING.get()?.previous,
            )
        })
        .ok()
        .flatten()
    } else {
        None
    };
    if active {
        let _ = std::panic::catch_unwind(|| {
            crate::native_transform::invalidate_shared_palette_writes(
                stamp.as_ref(),
                input.as_ref().map(|input| (input.owner, input.slot)),
            )
        });
    }
    unsafe {
        let original: WritePalette = std::mem::transmute(ORIGINAL.load(Ordering::Acquire));
        original(group, owner, slot, update, matrix, count, previous);
    }
    if let Some(stamp) = stamp {
        let record = std::panic::catch_unwind(|| input.and_then(Input::complete))
            .ok()
            .flatten();
        let accepted = std::panic::catch_unwind(|| {
            crate::native_transform::record_palette_write(&stamp, record)
        })
        .unwrap_or(false);
        if accepted {
            OBSERVED.fetch_add(1, Ordering::Relaxed);
        } else {
            REJECTED.fetch_add(1, Ordering::Relaxed);
        }
    }
}
pub(crate) fn install(base: usize) -> Result<(), String> {
    if crate::world_hooks::image_base() != Some(base) {
        return Err("unvalidated palette writer image".into());
    }
    let binding = crate::world_hooks::palette_writer_binding()?;
    let target = base
        .checked_add(binding.rva)
        .ok_or("palette writer address overflow")?;
    let expected = binding.previous.writer_prefix();
    let mut actual = [0; 27];
    read(target, &mut actual)?;
    if actual != expected {
        return Err("palette writer prologue differs from exact profile".into());
    }
    INSTALLED_BINDING
        .set(binding)
        .map_err(|_| "palette writer installation already attempted")?;
    use hudhook::mh::{MH_CreateHook, MH_EnableHook, MH_STATUS};
    let mut original = std::ptr::null_mut();
    let status =
        unsafe { MH_CreateHook(target as _, write_palette as *const () as _, &mut original) };
    if status != MH_STATUS::MH_OK {
        return Err(format!("create palette writer observer: {status:?}"));
    }
    ORIGINAL.store(original as usize, Ordering::Release);
    let status = unsafe { MH_EnableHook(target as _) };
    if status != MH_STATUS::MH_OK {
        return Err(format!("enable palette writer observer: {status:?}"));
    }
    ENABLED.store(true, Ordering::Release);
    Ok(())
}
pub(crate) fn status() -> serde_json::Value {
    serde_json::json!({"installed":ENABLED.load(Ordering::Acquire),"hookRva":INSTALLED_BINDING.get().map(|b|b.rva),
        "previousMatrixInput":INSTALLED_BINDING.get().map(|b|b.previous.name()),
        "calls":CALLS.load(Ordering::Relaxed),"observed":OBSERVED.load(Ordering::Relaxed),
        "rejected":REJECTED.load(Ordering::Relaxed),"maxRecordsPerActorScope":MAX_RECORDS,
        "maxBytesPerActorScope":MAX_BYTES,"historicalWriterReceiptsImplemented":true,
        "sealedPreviousPosesSurviveCacheAdvance":true,"historicalPreviousPoseVerified":false})
}
