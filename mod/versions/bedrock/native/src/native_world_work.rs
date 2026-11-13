//! Read-only network/subchunk/renderer work at both ends of one world pass.
//! These samples do not certify full application, mesh or GPU readiness.
use crate::native_scene_layout as scene;
use crate::world_hooks::read;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicUsize, Ordering};

const LEVEL_TABLE: usize = 0xe7adab0;
const MANAGER_TABLE: usize = 0xe7ae8b8;
const TASK_TABLE: usize = 0xe76b260;
const LIMIT: usize = 1_000_000;
static PROFILE: AtomicUsize = AtomicUsize::new(0);

#[derive(PartialEq, Eq)]
struct Guard {
    address: usize,
    bytes: Vec<u8>,
}
#[derive(Default, PartialEq, Eq)]
pub(crate) struct Copies {
    guards: Vec<Guard>,
    bytes: usize,
    refs: Vec<usize>,
}
impl Copies {
    pub(crate) fn copy(&mut self, address: usize, count: usize) -> Option<Vec<u8>> {
        if address < 0x10000
            || address.checked_add(count).is_none()
            || count > 128
            || self.guards.len() >= 32
            || self.bytes.checked_add(count)? > 2048
        {
            return None;
        }
        let mut bytes = vec![0; count];
        read(address, &mut bytes).ok()?;
        self.bytes += count;
        self.guards.push(Guard {
            address,
            bytes: bytes.clone(),
        });
        Some(bytes)
    }
    pub(crate) fn live(&mut self, control: usize) -> Option<()> {
        strong(control)?;
        if self.refs.len() >= 8 {
            return None;
        }
        self.refs.push(control);
        Some(())
    }
    pub(crate) fn unchanged(&self) -> Option<()> {
        for guard in &self.guards {
            let mut bytes = vec![0; guard.bytes.len()];
            read(guard.address, &mut bytes).ok()?;
            if bytes != guard.bytes {
                return None;
            }
        }
        for &control in &self.refs {
            strong(control)?;
        }
        Some(())
    }
}
fn strong(control: usize) -> Option<()> {
    if !(0x10000..0x0000_8000_0000_0000).contains(&control) {
        return None;
    }
    let mut bytes = [0; 4];
    read(control.checked_add(8)?, &mut bytes).ok()?;
    (i32::from_le_bytes(bytes) > 0).then_some(())
}
fn word(bytes: &[u8], at: usize) -> Option<usize> {
    usize::try_from(u64::from_le_bytes(
        bytes.get(at..at.checked_add(8)?)?.try_into().ok()?,
    ))
    .ok()
}
fn pointer(bytes: &[u8], at: usize) -> Option<usize> {
    word(bytes, at).filter(|value| (0x10000..0x0000_8000_0000_0000).contains(value))
}
#[derive(PartialEq, Eq)]
pub(crate) struct Sample {
    renderer: usize,
    frame_object: usize,
    client: usize,
    level: usize,
    epoch: u64,
    tick: u64,
    pending_changes: usize,
    queued: usize,
    tasks: usize,
    state: u32,
    leaving: bool,
    activity: crate::native_task_activity::Witness,
    network: Option<crate::native_network_work::Sample>,
    renderer_work: Option<crate::native_render_work::Sample>,
    frame_input: Option<crate::native_frame_input::Sample>,
    copies: Copies,
}
impl Sample {
    pub(crate) fn copy(renderer: usize, frame_object: usize) -> Option<Self> {
        let base = PROFILE.load(Ordering::Acquire);
        if base == 0 || crate::world_hooks::image_base() != Some(base) {
            return None;
        }
        let (level, epoch) = crate::players::session()?;
        crate::world_hooks::current_registry(level)?;
        let tick = crate::world_hooks::client_completed_tick(level)?;
        let client = crate::world_hooks::primary_client(level)?;
        let mut copies = Copies::default();
        if pointer(
            &copies.copy(
                client.instance.checked_add(scene::client::LEVEL_RENDERER)?,
                8,
            )?,
            0,
        )? != renderer
        {
            return None;
        }
        if pointer(&copies.copy(level, 8)?, 0)? != base.checked_add(LEVEL_TABLE)? {
            return None;
        }
        // UniqueOwnerPointer's object and ownership-control identities stay
        // local to this callback. Do not dereference a guessed control layout.
        let owner = copies.copy(level.checked_add(0x7b0)?, 16)?;
        pointer(&owner, 0)?;
        let manager = pointer(&owner, 8)?;
        if pointer(&copies.copy(manager, 8)?, 0)? != base.checked_add(MANAGER_TABLE)? {
            return None;
        }
        let insert = manager.checked_add(0x48)?;
        let group = insert.checked_add(0x18)?;
        if pointer(&copies.copy(group, 8)?, 0)? != base.checked_add(TASK_TABLE)? {
            return None;
        }
        // Acquire the activity identity before sampling mutable queue/count
        // fields. Recheck it around every later metadata/lifetime check.
        let activity = crate::native_task_activity::sample(group)?;
        let changes = copies.copy(manager.checked_add(8)?, 64)?;
        let head = pointer(&changes, 8)?;
        let pending_changes = word(&changes, 16)?;
        if pending_changes > LIMIT {
            return None;
        }
        let links = copies.copy(head, 16)?;
        if pending_changes == 0 && (word(&links, 0)? != head || word(&links, 8)? != head) {
            return None;
        }
        let queue = copies.copy(insert, 24)?;
        let begin = word(&queue, 0)?;
        let end = word(&queue, 8)?;
        let capacity = word(&queue, 16)?;
        let length = end.checked_sub(begin)?;
        let allocated = capacity.checked_sub(begin)?;
        if !length.is_multiple_of(0x20)
            || !allocated.is_multiple_of(0x20)
            || capacity < end
            || allocated / 0x20 > LIMIT
            || allocated != 0 && !(0x10000..0x0000_8000_0000_0000).contains(&begin)
        {
            return None;
        }
        let queued = length / 0x20;
        let state = u32::from_le_bytes(copies.copy(group.checked_add(0xb0)?, 4)?.try_into().ok()?);
        let tasks = word(&copies.copy(group.checked_add(0xc8)?, 8)?, 0)?;
        let leaving = copies.copy(insert.checked_add(0x178)?, 1)?[0];
        if state > 3 || tasks > LIMIT || leaving > 1 {
            return None;
        }
        let sample = Self {
            renderer,
            frame_object,
            client: client.instance,
            level,
            epoch,
            tick,
            pending_changes,
            queued,
            tasks,
            state,
            leaving: leaving != 0,
            activity,
            copies,
            network: crate::native_network_work::Sample::copy(level, epoch, tick),
            renderer_work: crate::native_render_work::Sample::copy(renderer, level, epoch, tick),
            frame_input: crate::native_frame_input::Sample::copy(
                renderer,
                frame_object,
                level,
                epoch,
                tick,
            ),
        };
        sample.live().then_some(sample)
    }
    fn live(&self) -> bool {
        self.activity.current()
            && self.copies.unchanged().is_some()
            && crate::players::session() == Some((self.level, self.epoch))
            && crate::world_hooks::client_completed_tick(self.level) == Some(self.tick)
            && crate::world_hooks::primary_client(self.level)
                .is_some_and(|client| client.instance == self.client)
            && crate::world_hooks::current_registry(self.level).is_some()
            && self.network.as_ref().is_none_or(|network| network.live())
            && self
                .renderer_work
                .as_ref()
                .is_none_or(|renderer| renderer.live())
            && self.frame_input.as_ref().is_none_or(|input| input.live())
            && self.copies.unchanged().is_some()
            && self.activity.current()
    }
    /// Pair beginning/end observations, not a later presentation-time sample.
    /// Observed TaskGroup callbacks must retain the same quiescent activity
    /// identity. Other queue mutations and task paths still need coverage.
    pub(crate) fn completed(self, after: Self, world_sequence: u64) -> Option<Value> {
        if self != after || !after.live() || world_sequence == 0 {
            return None;
        }
        Some(
            json!({"schema":1,"session":format!("native_{}_{}",std::process::id(),after.epoch),
            "ticks":after.tick,"worldSequence":world_sequence,"matchingEndpoints":true,
            "taskActivityObserved":true,"taskActivitySequence":after.activity.sequence(),
            "taskActivityGeneration":after.activity.generation(),
            "networkChunkIO":after.network.as_ref().map(|network|network.receipt()),
            "rendererTasks":after.renderer_work.as_ref().map(|renderer|renderer.receipt()),
            "frameInput":after.frame_input.as_ref().map(|input|input.receipt()),
            "subchunkInsertion":{"pendingChanges":after.pending_changes,"queuedTasks":after.queued,
                "inFlightTasks":after.tasks,"taskGroupState":after.state,"leaving":after.leaving,
                "observedIdle":after.pending_changes==0&&after.queued==0&&after.tasks==0&&after.state==0&&!after.leaving},
            "packetApplicationVerified":false,"terrainReady":false,"deterministicVideoReady":false}),
        )
    }
}
pub(crate) fn install(base: usize) -> Result<(), String> {
    if crate::world_hooks::image_base() != Some(base) {
        return Err("unvalidated world-work image".into());
    }
    let sites: &[(usize, &[u8])] = &[
        (
            0x12efe65,
            &[0x48, 0x8d, 0x05, 0x44, 0xdc, 0x4b, 0x0d, 0x48, 0x89, 0x07],
        ),
        (
            0x12f040f,
            &[0x48, 0x8d, 0x05, 0xa2, 0xe4, 0x4b, 0x0d, 0x48, 0x89, 0x06],
        ),
        (0x12f0556, &[0x48, 0x8d, 0x73, 0x48]),
        (0x12f05ce, &[0xe8, 0x8d, 0x6b, 0xe8, 0x04]),
        (
            0x12f0a9c,
            &[
                0x48, 0x89, 0x8b, 0xb0, 0x07, 0, 0, 0x48, 0x8b, 0x85, 0xc0, 0, 0, 0, 0x48, 0x89,
                0x83, 0xb8, 0x07, 0, 0,
            ],
        ),
        (
            0x12f4ad3,
            &[
                0x48, 0x8b, 0x8e, 0xb8, 0x07, 0, 0, 0x48, 0x83, 0xc1, 0x48, 0xe8, 0x5d, 0x2d, 0xe8,
                0x04,
            ],
        ),
        (
            0x61778a8,
            &[
                0x48, 0x8b, 0x47, 0x08, 0x48, 0x2b, 0x07, 0x48, 0xc1, 0xf8, 0x05,
            ],
        ),
        (0x617792a, &[0x48, 0x8b, 0xb7, 0xe0, 0, 0, 0]),
        (0x6177942, &[0x80, 0xbf, 0x78, 0x01, 0, 0, 0]),
        (0x6177281, &[0x48, 0x8d, 0x48, 0x18]),
        (
            0x6b409b,
            &[0x48, 0x8d, 0x05, 0xbe, 0x71, 0x0b, 0x0e, 0x48, 0x89, 0x01],
        ),
        (
            0x6b4239,
            &[
                0x0f, 0x11, 0xb7, 0xb8, 0, 0, 0, 0x0f, 0x11, 0xb7, 0xc8, 0, 0, 0,
            ],
        ),
        (0x6410f0, &[0x48, 0x8d, 0x81, 0x70, 0x03, 0, 0, 0xc3]),
    ];
    for &(rva, expected) in sites {
        let mut actual = vec![0; expected.len()];
        read(
            base.checked_add(rva).ok_or("world-work address overflow")?,
            &mut actual,
        )?;
        if actual != expected {
            return Err(format!("world-work profile differs at {rva:x}"));
        }
    }
    for (table, offset, function) in [
        (LEVEL_TABLE, 0xc68, 0x12f4940),
        (LEVEL_TABLE, 0xcc0, 0x12f4b70),
        (MANAGER_TABLE, 8, 0x6410f0),
        (TASK_TABLE, 0, 0x6ba2c0),
    ] {
        if crate::world_hooks::pointer(
            base.checked_add(table)
                .and_then(|p| p.checked_add(offset))
                .ok_or("world-work vtable overflow")?,
        ) != base.checked_add(function)
        {
            return Err("world-work vtable differs from exact profile".into());
        }
    }
    crate::native_task_activity::install(base)?;
    crate::native_network_work::install(base)?;
    crate::native_render_work::install(base)?;
    crate::native_frame_input::install(base)?;
    PROFILE.store(base, Ordering::Release);
    Ok(())
}
