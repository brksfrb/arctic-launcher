//! Read-only renderer tasks and chunk changes for the actual primary world draw.
//! Idle observed groups do not prove complete terrain or GPU readiness.
use crate::native_scene_layout as scene;
use crate::native_world_work::Copies;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::native_scene_layout::level_renderer::{APP_VTABLE as APP_TABLE, VTABLE as RENDER_TABLE};
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
fn name(copies: &mut Copies, group: usize, expected: &[u8]) -> Option<()> {
    let header = copies.copy(group.checked_add(0x38)?, 32)?;
    let length = word(&header, 16)?;
    let capacity = word(&header, 24)?;
    if length != expected.len() || capacity < length || capacity > 4096 {
        return None;
    }
    let text = if capacity < 16 {
        header.get(..length.checked_add(1)?)?.to_vec()
    } else {
        copies.copy(pointer(&header, 0)?, length.checked_add(1)?)?
    };
    (text.last() == Some(&0) && text.get(..length)? == expected).then_some(())
}
#[derive(PartialEq, Eq)]
struct Group {
    tasks: usize,
    state: u32,
    activity: crate::native_task_activity::Witness,
}
impl Group {
    fn copy(copies: &mut Copies, base: usize, group: usize, expected: &[u8]) -> Option<Self> {
        if pointer(&copies.copy(group, 8)?, 0)? != base.checked_add(TASK_TABLE)? {
            return None;
        }
        let activity = crate::native_task_activity::sample(group)?;
        name(copies, group, expected)?;
        let tasks = word(&copies.copy(group.checked_add(0xc8)?, 8)?, 0)?;
        let state = u32::from_le_bytes(copies.copy(group.checked_add(0xb0)?, 4)?.try_into().ok()?);
        if tasks > LIMIT || state > 3 {
            return None;
        }
        Some(Self {
            tasks,
            state,
            activity,
        })
    }
    fn idle(&self) -> bool {
        self.tasks == 0 && self.state == 0
    }
    fn receipt(&self) -> Value {
        json!({"inFlightTasks":self.tasks,"taskGroupState":self.state,
            "taskActivityObserved":true,"taskActivitySequence":self.activity.sequence(),
            "taskActivityGeneration":self.activity.generation(),"observedIdle":self.idle()})
    }
}
#[derive(PartialEq, Eq)]
pub(crate) struct Sample {
    renderer: usize,
    client: usize,
    level: usize,
    epoch: u64,
    tick: u64,
    background: Group,
    main_thread: Group,
    changes: Option<crate::native_chunk_work::Sample>,
    copies: Copies,
}
impl Sample {
    pub(crate) fn copy(renderer: usize, level: usize, epoch: u64, tick: u64) -> Option<Self> {
        let base = PROFILE.load(Ordering::Acquire);
        if base == 0
            || crate::world_hooks::image_base() != Some(base)
            || crate::players::session() != Some((level, epoch))
        {
            return None;
        }
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
        let tables = copies.copy(renderer, 16)?;
        if pointer(&tables, 0)? != base.checked_add(RENDER_TABLE)?
            || pointer(&tables, 8)? != base.checked_add(APP_TABLE)?
        {
            return None;
        }
        // Constructor-owned groups, game renderer, level, options owner and
        // client back-reference. Never adopt their native ownership.
        let owners = copies.copy(
            renderer.checked_add(scene::level_renderer::TASK_OWNERS)?,
            0x38,
        )?;
        if pointer(
            &owners,
            scene::level_renderer::LEVEL - scene::level_renderer::TASK_OWNERS,
        )? != level
            || pointer(
                &owners,
                scene::level_renderer::CLIENT - scene::level_renderer::TASK_OWNERS,
            )? != client.instance
        {
            return None;
        }
        pointer(
            &owners,
            scene::level_renderer::GAME_RENDERER - scene::level_renderer::TASK_OWNERS,
        )?;
        let background = pointer(&owners, 0)?;
        let main_thread = pointer(&owners, 8)?;
        if background == main_thread {
            return None;
        }
        let background = Group::copy(&mut copies, base, background, b"A LevelRenderer group")?;
        let main_thread = Group::copy(&mut copies, base, main_thread, b"Main thread tasks group")?;
        let sample = Self {
            renderer,
            client: client.instance,
            level,
            epoch,
            tick,
            background,
            main_thread,
            copies,
            changes: crate::native_chunk_work::Sample::copy(renderer, level, epoch, tick),
        };
        sample.live().then_some(sample)
    }
    pub(crate) fn live(&self) -> bool {
        self.background.activity.current()
            && self.main_thread.activity.current()
            && self.copies.unchanged().is_some()
            && crate::players::session() == Some((self.level, self.epoch))
            && crate::world_hooks::client_completed_tick(self.level) == Some(self.tick)
            && crate::world_hooks::primary_client(self.level)
                .is_some_and(|client| client.instance == self.client)
            && self.changes.as_ref().is_none_or(|changes| changes.live())
            && self.copies.unchanged().is_some()
            && self.background.activity.current()
            && self.main_thread.activity.current()
    }
    pub(crate) fn receipt(&self) -> Value {
        json!({"observed":true,"background":self.background.receipt(),"mainThread":self.main_thread.receipt(),
            "chunkChanges":self.changes.as_ref().map(|changes|changes.receipt()),
            "observedIdle":self.background.idle()&&self.main_thread.idle()&&self.changes.as_ref().is_some_and(|changes|changes.idle()),
            "terrainReady":false})
    }
}
pub(crate) fn install(base: usize) -> Result<(), String> {
    if crate::world_hooks::image_base() != Some(base) {
        return Err("unvalidated renderer-work image".into());
    }
    let sites: &[(usize, &[u8])] = &[
        (
            0x47e54a0,
            &[
                0x48, 0x8d, 0x05, 0x49, 0xe7, 0x0f, 0x0a, 0x48, 0x89, 0x07, 0x48, 0x8d, 0x05, 0x3f,
                0xe8, 0x0f, 0x0a, 0x48, 0x89, 0x47, 0x08,
            ],
        ),
        (
            0x47e6326,
            &[
                0x48, 0x8d, 0x88, 0xd8, 0x03, 0, 0, 0x48, 0x8b, 0x15, 0x9c, 0x80, 0x54, 0x0d, 0x4c,
                0x8d, 0x0d, 0xb5, 0x7f, 0x42, 0x0a, 0x48, 0x89, 0x8d, 0xb0, 0x01, 0, 0, 0x49, 0x89,
                0xd8, 0xe8, 0x36, 0x94, 0x8e, 0xfb,
            ],
        ),
        (
            0x47e6357,
            &[
                0x48, 0x8b, 0x8d, 0x30, 0x05, 0, 0, 0x4c, 0x8d, 0xb9, 0xe0, 0x03, 0, 0, 0x48, 0x8b,
                0x50, 0x50, 0x4c, 0x8d, 0x0d, 0x68, 0x7f, 0x42, 0x0a, 0x4c, 0x89, 0xf9, 0x49, 0x89,
                0xd8, 0xe8, 0x05, 0x94, 0x8e, 0xfb,
            ],
        ),
        (
            0x47e6389,
            &[
                0x48, 0x89, 0x88, 0xe8, 0x03, 0, 0, 0x48, 0x89, 0xb0, 0xf0, 0x03, 0, 0,
            ],
        ),
        (0x47e63d8, &[0x4c, 0x89, 0xb2, 0x08, 0x04, 0, 0]),
        (
            0x47e6545,
            &[
                0x49, 0x89, 0x9f, 0x68, 0x04, 0, 0, 0x48, 0x8b, 0x95, 0xf0, 0x04, 0, 0, 0x49, 0x89,
                0x97, 0x70, 0x04, 0, 0,
            ],
        ),
        (
            0x1c9f4ae,
            &[0x48, 0x8b, 0x46, 0x60, 0x48, 0x8b, 0xb8, 0xd8, 0x03, 0, 0],
        ),
        (
            0x47f02a8,
            &[
                0x48, 0x8b, 0x8e, 0xe0, 0x03, 0, 0, 0x48, 0x85, 0xc9, 0x74, 0x11, 0x48, 0x8b, 0x01,
                0x48, 0x8b, 0, 0xba, 0x01, 0, 0, 0, 0xff, 0x15, 0xe3, 0xb7, 0x90, 0x0a, 0x48, 0x8b,
                0x8e, 0xd8, 0x03, 0, 0,
            ],
        ),
    ];
    for &(rva, expected) in sites {
        let mut actual = vec![0; expected.len()];
        crate::world_hooks::read(
            base.checked_add(rva)
                .ok_or("renderer-work address overflow")?,
            &mut actual,
        )?;
        if actual != expected {
            return Err(format!("renderer-work profile differs at {rva:x}"));
        }
    }
    for (table, offset, target) in [
        (RENDER_TABLE, 0, 0x47fea30),
        (APP_TABLE, 0, 0x47fea60),
        (TASK_TABLE, 0x18, 0x6b73d0),
        (TASK_TABLE, 0x38, 0x6b7120),
    ] {
        if crate::world_hooks::pointer(
            base.checked_add(table + offset)
                .ok_or("renderer-work vtable overflow")?,
        ) != base.checked_add(target)
        {
            return Err("renderer-work table differs".into());
        }
    }
    crate::native_chunk_work::install(base)?;
    PROFILE.store(base, Ordering::Release);
    Ok(())
}
