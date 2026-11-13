//! Pending native network chunk work, owned by the current primary session.
//! Samples only: no complete packet application, terrain or GPU claim.
use crate::native_world_work::Copies;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicUsize, Ordering};

const CALLBACK_TABLE: usize = 0xe7af110;
const TASK_TABLE: usize = 0xe76b260;
const LIMIT: usize = 1_000_000;
static PROFILE: AtomicUsize = AtomicUsize::new(0);
fn word(bytes: &[u8], at: usize) -> Option<usize> {
    usize::try_from(u64::from_le_bytes(
        bytes.get(at..at.checked_add(8)?)?.try_into().ok()?,
    ))
    .ok()
}
fn pointer(bytes: &[u8], at: usize) -> Option<usize> {
    word(bytes, at).filter(|value| (0x10000..0x0000_8000_0000_0000).contains(value))
}
fn non_owner(copies: &mut Copies, bytes: &[u8], expected: usize) -> Option<()> {
    if pointer(bytes, 16)? != expected {
        return None;
    }
    copies.live(pointer(bytes, 8)?)?;
    (copies.copy(pointer(bytes, 0)?, 1)? == [1]).then_some(())
}
fn map_count(copies: &mut Copies, map: usize) -> Option<usize> {
    let header = copies.copy(map, 64)?;
    let head = pointer(&header, 8)?;
    let count = word(&header, 16)?;
    if count > LIMIT {
        return None;
    }
    let links = copies.copy(head, 16)?;
    if count == 0 && (word(&links, 0)? != head || word(&links, 8)? != head) {
        return None;
    }
    Some(count)
}
fn io_name(copies: &mut Copies, group: usize) -> Option<()> {
    let name = b"ClientNetworkHandler IO Group";
    let header = copies.copy(group.checked_add(0x38)?, 32)?;
    let length = word(&header, 16)?;
    let capacity = word(&header, 24)?;
    if length != name.len() || capacity < length || capacity > 4096 {
        return None;
    }
    let text = if capacity < 16 {
        header.get(..length.checked_add(1)?)?.to_vec()
    } else {
        copies.copy(pointer(&header, 0)?, length.checked_add(1)?)?
    };
    (text.last() == Some(&0) && text.get(..length)? == name).then_some(())
}
#[derive(PartialEq, Eq)]
struct Insertion {
    issued: usize,
    consumed: usize,
    ready: usize,
}
impl Insertion {
    fn copy(copies: &mut Copies, handler: usize) -> Option<Self> {
        // The constructor uniquely owns this 0x30-byte queue at +0x298;
        // its first word points back to this exact legacy callback.
        let queue = pointer(&copies.copy(handler.checked_add(0x298)?, 8)?, 0)?;
        let header = copies.copy(queue, 0x30)?;
        if pointer(&header, 0)? != handler {
            return None;
        }
        let issued = word(&header, 8)?;
        let consumed = word(&header, 0x10)?;
        let outstanding = issued.checked_sub(consumed)?;
        let begin = word(&header, 0x18)?;
        let end = word(&header, 0x20)?;
        let capacity = word(&header, 0x28)?;
        if begin == 0 {
            if end != 0 || capacity != 0 {
                return None;
            }
        } else {
            for at in [0x18, 0x20, 0x28] {
                pointer(&header, at)?;
            }
            if !begin.is_multiple_of(8) {
                return None;
            }
        }
        let length = end.checked_sub(begin)?;
        let allocated = capacity.checked_sub(begin)?;
        // PendingChunk is 24 bytes; never borrow or traverse its owned chunk.
        if capacity < end
            || !length.is_multiple_of(0x18)
            || !allocated.is_multiple_of(0x18)
            || allocated / 0x18 > LIMIT
            || outstanding > LIMIT
        {
            return None;
        }
        let ready = length / 0x18;
        if ready > outstanding {
            return None;
        }
        Some(Self {
            issued,
            consumed,
            ready,
        })
    }
    fn idle(&self) -> bool {
        self.issued == self.consumed && self.ready == 0
    }
    fn receipt(&self) -> Value {
        // Consumed includes native discards, so it must not be called applied.
        json!({"observed":true,"issuedChunks":self.issued,"consumedChunks":self.consumed,
            "outstandingChunks":self.issued-self.consumed,"readyChunks":self.ready,
            "observedIdle":self.idle()})
    }
}
#[derive(PartialEq, Eq)]
pub(crate) struct Sample {
    level: usize,
    epoch: u64,
    tick: u64,
    client: usize,
    pending: usize,
    paused: usize,
    tasks: usize,
    state: u32,
    insertion: Insertion,
    activity: crate::native_task_activity::Witness,
    copies: Copies,
}
impl Sample {
    pub(crate) fn copy(level: usize, epoch: u64, tick: u64) -> Option<Self> {
        let base = PROFILE.load(Ordering::Acquire);
        if base == 0
            || crate::world_hooks::image_base() != Some(base)
            || crate::players::session() != Some((level, epoch))
        {
            return None;
        }
        let client = crate::world_hooks::primary_client(level)?;
        let mut copies = Copies::default();
        let minecraft = pointer(&copies.copy(client.instance.checked_add(0x1b0)?, 8)?, 0)?;
        let session = pointer(&copies.copy(minecraft.checked_add(0xc8)?, 8)?, 0)?;
        // The client constructor stores discriminator 0 and owns the legacy
        // callback at +0x50. The server callback at +0x48 must be absent.
        if copies.copy(session.checked_add(8)?, 1)? != [0]
            || copies.copy(session.checked_add(0x28)?, 1)? != [1]
        {
            return None;
        }
        let level_owner = copies.copy(session.checked_add(0x30)?, 24)?;
        non_owner(&mut copies, &level_owner, level)?;
        let handlers = copies.copy(session.checked_add(0x48)?, 16)?;
        if word(&handlers, 0)? != 0 {
            return None;
        }
        let handler = pointer(&handlers, 8)?;
        if pointer(&copies.copy(handler, 8)?, 0)? != base.checked_add(CALLBACK_TABLE)?
            || pointer(&copies.copy(handler.checked_add(0x58)?, 8)?, 0)? != client.instance
        {
            return None;
        }
        // getNetEventCallback returns this exact alive flag/shared owner.
        let existence = copies.copy(handler.checked_add(8)?, 16)?;
        copies.live(pointer(&existence, 8)?)?;
        if copies.copy(pointer(&existence, 0)?, 1)? != [1] {
            return None;
        }
        let level_owner = copies.copy(handler.checked_add(0xb0)?, 24)?;
        non_owner(&mut copies, &level_owner, level)?;
        let group = pointer(&copies.copy(handler.checked_add(0x168)?, 8)?, 0)?;
        if pointer(&copies.copy(group, 8)?, 0)? != base.checked_add(TASK_TABLE)? {
            return None;
        }
        let activity = crate::native_task_activity::sample(group)?;
        io_name(&mut copies, group)?;
        let pending = map_count(&mut copies, handler.checked_add(0xe8)?)?;
        let paused = map_count(&mut copies, handler.checked_add(0x128)?)?;
        let tasks = word(&copies.copy(group.checked_add(0xc8)?, 8)?, 0)?;
        let state = u32::from_le_bytes(copies.copy(group.checked_add(0xb0)?, 4)?.try_into().ok()?);
        if tasks > LIMIT || state > 3 {
            return None;
        }
        let insertion = Insertion::copy(&mut copies, handler)?;
        let sample = Self {
            level,
            epoch,
            tick,
            client: client.instance,
            pending,
            paused,
            tasks,
            state,
            insertion,
            activity,
            copies,
        };
        sample.live().then_some(sample)
    }
    pub(crate) fn live(&self) -> bool {
        self.activity.current()
            && self.copies.unchanged().is_some()
            && crate::players::session() == Some((self.level, self.epoch))
            && crate::world_hooks::client_completed_tick(self.level) == Some(self.tick)
            && crate::world_hooks::primary_client(self.level)
                .is_some_and(|client| client.instance == self.client)
            && self.copies.unchanged().is_some()
            && self.activity.current()
    }
    pub(crate) fn receipt(&self) -> Value {
        json!({"pendingChunks":self.pending,"pausedCallbacks":self.paused,
            "inFlightTasks":self.tasks,"taskGroupState":self.state,
            "taskActivityObserved":true,"taskActivitySequence":self.activity.sequence(),
            "taskActivityGeneration":self.activity.generation(),
            "legacyInsertion":self.insertion.receipt(),
            "observedIdle":self.pending==0&&self.paused==0&&self.tasks==0&&self.state==0&&self.insertion.idle()})
    }
}
pub(crate) fn install(base: usize) -> Result<(), String> {
    if crate::world_hooks::image_base() != Some(base) {
        return Err("unvalidated network-work image".into());
    }
    let sites: &[(usize, &[u8])] = &[
        (0x26b26cd, &[0x48, 0x89, 0x11, 0xc6, 0x41, 0x08, 0]),
        (
            0x26b26fa,
            &[0x49, 0xc7, 0, 0, 0, 0, 0, 0x4c, 0x89, 0x71, 0x50],
        ),
        (
            0x26b2b62,
            &[
                0x48, 0x8b, 0x79, 0x50, 0x0f, 0x57, 0xc0, 0x0f, 0x11, 0, 0x48, 0xc7, 0x40, 0x10, 0,
                0, 0, 0,
            ],
        ),
        (
            0x132c6ad,
            &[
                0x48, 0x8d, 0x05, 0x5c, 0x2a, 0x48, 0x0d, 0x49, 0x89, 0x04, 0x24,
            ],
        ),
        (0x132bee1, &[0x48, 0x8b, 0x46, 0x30, 0x48, 0x89, 0x42, 0x58]),
        (
            0x131f785,
            &[
                0x48, 0x8b, 0x81, 0xb0, 0, 0, 0, 0x48, 0x85, 0xc0, 0x0f, 0x84, 0xc1, 0, 0, 0, 0x80,
                0x38, 0x01,
            ],
        ),
        (
            0x131f79e,
            &[0x4c, 0x89, 0xc6, 0x48, 0x8b, 0x91, 0xc0, 0, 0, 0],
        ),
        (
            0x132c060,
            &[
                0x48, 0x8b, 0x45, 0x60, 0x48, 0x8d, 0x88, 0xf0, 0, 0, 0, 0x48, 0x89, 0x4d, 0x58,
                0x48, 0x89, 0x3f,
            ],
        ),
        (
            0x132c1cc,
            &[
                0x48, 0x8b, 0x45, 0x60, 0x48, 0x8d, 0x88, 0x30, 0x01, 0, 0, 0x48, 0x89, 0x4d, 0x58,
                0x48, 0x89, 0x3f,
            ],
        ),
        (0x132c480, &[0x49, 0x8d, 0x8c, 0x24, 0x68, 0x01, 0, 0]),
        (
            0x132c5d2,
            &[
                0x4c, 0x8d, 0x0d, 0x57, 0x34, 0x8e, 0x0d, 0x48, 0x8d, 0x4d, 0xd8, 0x49, 0x89, 0xf8,
                0xe8, 0x9b, 0x31, 0xda, 0xfe,
            ],
        ),
        (
            0xcf8b3,
            &[
                0x4c, 0x8d, 0x4d, 0xd0, 0x48, 0x89, 0xf9, 0x48, 0x89, 0xda, 0x4d, 0x89, 0xe0, 0xe8,
                0xbb, 0x47, 0x5e, 0,
            ],
        ),
        // Legacy queue ownership, request sequence and completed-chunk heap.
        (0x132c771, &[0xba, 0x30, 0, 0, 0]),
        (
            0x132c788,
            &[
                0x48, 0x8b, 0x4d, 0x60, 0x48, 0x8d, 0x81, 0x98, 0x02, 0, 0, 0x48, 0x89, 0x45, 0x18,
                0x48, 0x89, 0x0f, 0x0f, 0x57, 0xc0, 0x0f, 0x11, 0x47, 0x08, 0x0f, 0x11, 0x47, 0x18,
                0x48, 0xc7, 0x47, 0x28, 0, 0, 0, 0, 0x48, 0x89, 0xb9, 0x98, 0x02, 0, 0,
            ],
        ),
        (
            0x132d70a,
            &[
                0x48, 0x8b, 0xbe, 0x98, 0x02, 0, 0, 0x48, 0x85, 0xff, 0x74, 0x16, 0x48, 0x8d, 0x4f,
                0x18, 0xe8, 0xa1, 0x09, 0x04, 0,
            ],
        ),
        (
            0x13471aa,
            &[
                0x48, 0x8b, 0x86, 0x98, 0x02, 0, 0, 0x48, 0x8b, 0x58, 0x08, 0x48, 0x8d, 0x4b, 0x01,
                0x48, 0x89, 0x48, 0x08,
            ],
        ),
        (0x13473da, &[0x49, 0x89, 0x99, 0xe8, 0, 0, 0]),
        (0x13574ac, &[0x4c, 0x8b, 0xbe, 0x98, 0x02, 0, 0]),
        (
            0x13575d4,
            &[
                0x49, 0x89, 0x1c, 0x24, 0x49, 0x89, 0x44, 0x24, 0x08, 0x4d, 0x89, 0x6c, 0x24, 0x10,
                0x49, 0x8b, 0x7f, 0x18, 0x49, 0x8b, 0x4f, 0x20, 0x48, 0x83, 0xc1, 0x18, 0x49, 0x89,
                0x4f, 0x20,
            ],
        ),
        (
            0x1357ab0,
            &[
                0x49, 0x8b, 0x5f, 0x18, 0x49, 0x8b, 0x77, 0x20, 0x48, 0x39, 0xf3, 0x0f, 0x84, 0x3f,
                0x0d, 0, 0, 0x48, 0x8b, 0x03, 0x49, 0x3b, 0x47, 0x10, 0x0f, 0x85, 0x32, 0x0d, 0, 0,
            ],
        ),
        (0x1357d91, &[0x48, 0x83, 0xc0, 0xe8, 0x49, 0x89, 0x47, 0x20]),
        (0x1358130, &[0x49, 0xff, 0x47, 0x10]),
        (
            0x136e0e0,
            &[
                0x48, 0x83, 0xc7, 0x18, 0x4c, 0x39, 0xf7, 0x74, 0x20, 0x48, 0x8b, 0x5f, 0x08, 0x48,
                0x85, 0xdb, 0x74, 0xee,
            ],
        ),
    ];
    for &(rva, expected) in sites {
        let mut actual = vec![0; expected.len()];
        crate::world_hooks::read(
            base.checked_add(rva)
                .ok_or("network-work address overflow")?,
            &mut actual,
        )?;
        if actual != expected {
            return Err(format!("network-work profile differs at {rva:x}"));
        }
    }
    for (offset, target) in [(0x88, 0x131f770), (0x2b0, 0x134ae30), (0x5a8, 0x1346770)] {
        if crate::world_hooks::pointer(
            base.checked_add(CALLBACK_TABLE + offset)
                .ok_or("network-work vtable overflow")?,
        ) != base.checked_add(target)
        {
            return Err("network-work callback table differs".into());
        }
    }
    PROFILE.store(base, Ordering::Release);
    Ok(())
}
