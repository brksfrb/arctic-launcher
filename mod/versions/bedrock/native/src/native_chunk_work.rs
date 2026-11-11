//! Pending render-chunk changes for the actual primary camera's dimension.
//! This observes metadata; it never drains work or certifies terrain readiness.
use crate::native_scene_layout as scene;
use crate::native_world_work::Copies;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::native_scene_layout::{
    level_renderer::{APP_VTABLE as APP_TABLE, VTABLE as RENDER_TABLE},
    player_renderer::{OWNER_VTABLE as CAMERA_OWNER_TABLE, VTABLE as CAMERA_TABLE},
};
const COORDINATOR_TABLE: usize = 0xe7d5b80;
const COORDINATOR_OWNER_TABLE: usize = 0xe6daf00;
const DIMENSION_GETTER: usize = 0x230e560;
const MAX_COORDINATORS: usize = 8;
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
fn integer(bytes: &[u8], at: usize) -> Option<i32> {
    Some(i32::from_le_bytes(
        bytes.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}
fn owned(copies: &mut Copies, base: usize, bytes: &[u8], table: usize) -> Option<usize> {
    let object = pointer(bytes, 0)?;
    let control = pointer(bytes, 8)?;
    // Both inspected factories place their polymorphic object at control+0x10.
    if control.checked_add(0x10)? != object
        || pointer(&copies.copy(control, 8)?, 0)? != base.checked_add(table)?
    {
        return None;
    }
    copies.live(control)?;
    Some(object)
}
fn map_count(copies: &mut Copies, address: usize) -> Option<usize> {
    let header = copies.copy(address, 64)?;
    let head = pointer(&header, 8)?;
    let count = word(&header, 16)?;
    if count > LIMIT {
        return None;
    }
    let links = copies.copy(head, 16)?;
    let next = pointer(&links, 0)?;
    let previous = pointer(&links, 8)?;
    if count == 0 && (next != head || previous != head)
        || count != 0 && (next == head || previous == head)
    {
        return None;
    }
    Some(count)
}
#[derive(PartialEq, Eq)]
pub(crate) struct Sample {
    renderer: usize,
    client: usize,
    level: usize,
    epoch: u64,
    tick: u64,
    dimension: i32,
    coordinators: usize,
    visibility_chunks: usize,
    actor_chunks: usize,
    block_entity_chunks: usize,
    shared: Option<crate::native_shared_chunk_work::Sample>,
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
            || pointer(
                &copies.copy(renderer.checked_add(scene::level_renderer::LEVEL)?, 8)?,
                0,
            )? != level
            || pointer(
                &copies.copy(renderer.checked_add(scene::level_renderer::CLIENT)?, 8)?,
                0,
            )? != client.instance
        {
            return None;
        }
        let camera_owner = copies.copy(
            renderer.checked_add(scene::level_renderer::PLAYER_OWNER)?,
            16,
        )?;
        let camera = owned(&mut copies, base, &camera_owner, CAMERA_OWNER_TABLE)?;
        if pointer(&copies.copy(camera, 8)?, 0)? != base.checked_add(CAMERA_TABLE)? {
            return None;
        }
        let camera_links = copies.copy(camera.checked_add(scene::player_renderer::LINKS)?, 0x30)?;
        if pointer(&camera_links, 0)? != level
            || pointer(
                &camera_links,
                scene::player_renderer::LEVEL_RENDERER - scene::player_renderer::LINKS,
            )? != renderer
        {
            return None;
        }
        let dimension = pointer(
            &camera_links,
            scene::player_renderer::DIMENSION - scene::player_renderer::LINKS,
        )?;
        let dimension_table = pointer(&copies.copy(dimension, 8)?, 0)?;
        // The actual virtual slot calls the pinned trivial out-value getter.
        // Copy the same field; never invoke the virtual method or its body.
        if pointer(&copies.copy(dimension_table.checked_add(0x10)?, 8)?, 0)?
            != base.checked_add(DIMENSION_GETTER)?
        {
            return None;
        }
        let dimension = integer(&copies.copy(dimension.checked_add(0x1a0)?, 4)?, 0)?;

        let map = copies.copy(renderer.checked_add(0x28)?, 64)?;
        let head = pointer(&map, 8)?;
        let coordinators = word(&map, 16)?;
        if !(1..=MAX_COORDINATORS).contains(&coordinators) {
            return None;
        }
        let links = copies.copy(head, 16)?;
        let mut node = pointer(&links, 0)?;
        let last = pointer(&links, 8)?;
        let mut previous = head;
        let mut visited = Vec::with_capacity(coordinators);
        let mut dimensions = Vec::with_capacity(coordinators);
        let mut selected = None;
        for _ in 0..coordinators {
            if node == head || visited.contains(&node) {
                return None;
            }
            let entry = copies.copy(node, 0x28)?;
            if pointer(&entry, 8)? != previous {
                return None;
            }
            let key = integer(&entry, 0x10)?;
            if dimensions.contains(&key) {
                return None;
            }
            if key == dimension {
                selected = Some(entry.get(0x18..0x28)?.to_vec());
            }
            visited.push(node);
            dimensions.push(key);
            previous = node;
            node = pointer(&entry, 0)?;
        }
        if node != head || previous != last {
            return None;
        }
        let selected = selected?;
        let coordinator = owned(&mut copies, base, &selected, COORDINATOR_OWNER_TABLE)?;
        if pointer(&copies.copy(coordinator, 8)?, 0)? != base.checked_add(COORDINATOR_TABLE)? {
            return None;
        }
        let owner_links = copies.copy(coordinator.checked_add(0x58)?, 0x2c)?;
        if pointer(&owner_links, 0)? != level
            || pointer(&owner_links, 8)? != renderer
            || integer(&owner_links, 0x28)? != dimension
        {
            return None;
        }
        // These are maps keyed by chunk position. Counts describe pending
        // chunk keys, not the number of messages in their notification vectors.
        let visibility_chunks = map_count(&mut copies, coordinator.checked_add(0x88)?)?;
        let actor_chunks = map_count(&mut copies, coordinator.checked_add(0xc8)?)?;
        let block_entity_chunks = map_count(&mut copies, coordinator.checked_add(0x108)?)?;
        let sample = Self {
            renderer,
            client: client.instance,
            level,
            epoch,
            tick,
            dimension,
            coordinators,
            visibility_chunks,
            actor_chunks,
            block_entity_chunks,
            copies,
            shared: crate::native_shared_chunk_work::Sample::copy(
                coordinator,
                renderer,
                level,
                epoch,
                tick,
                dimension,
            ),
        };
        sample.live().then_some(sample)
    }
    pub(crate) fn live(&self) -> bool {
        self.copies.unchanged().is_some()
            && crate::players::session() == Some((self.level, self.epoch))
            && crate::world_hooks::client_completed_tick(self.level) == Some(self.tick)
            && crate::world_hooks::primary_client(self.level)
                .is_some_and(|client| client.instance == self.client)
            && self.shared.as_ref().is_none_or(|shared| shared.live())
            && self.copies.unchanged().is_some()
    }
    pub(crate) fn idle(&self) -> bool {
        self.visibility_chunks == 0
            && self.actor_chunks == 0
            && self.block_entity_chunks == 0
            && self.shared.as_ref().is_some_and(|shared| shared.idle())
    }
    pub(crate) fn receipt(&self) -> Value {
        json!({"observed":true,"dimensionId":self.dimension,"coordinatorsOwned":self.coordinators,
            "visibilityDirtyChunks":self.visibility_chunks,"actorNotificationChunks":self.actor_chunks,
            "blockEntityNotificationChunks":self.block_entity_chunks,
            "sharedChunkWork":self.shared.as_ref().map(|shared|shared.receipt()),
            "observedIdle":self.idle(),"terrainReady":false})
    }
}
pub(crate) fn install(base: usize) -> Result<(), String> {
    if crate::world_hooks::image_base() != Some(base) {
        return Err("unvalidated chunk-work image".into());
    }
    let sites: &[(usize, &[u8])] = &[
        (
            0x47d83cd,
            &[
                0x48, 0x23, 0x57, 0x58, 0x48, 0x8b, 0x4f, 0x30, 0x4c, 0x8b, 0x47, 0x40,
            ],
        ),
        (
            0x47d846e,
            &[
                0x48, 0x8d, 0x05, 0x8b, 0x2a, 0xf0, 0x09, 0x49, 0x89, 0x04, 0x24, 0x4d, 0x89, 0xe6,
                0x49, 0x83, 0xc6, 0x10, 0x4c, 0x89, 0xf1, 0x4c, 0x89, 0xfa, 0x49, 0x89, 0xf8, 0x41,
                0x89, 0xd9, 0x4c, 0x89, 0x65, 0xe8, 0xe8, 0x1b, 0x24, 0x4c, 0xfd,
            ],
        ),
        (
            0x47d84c3,
            &[
                0x4c, 0x89, 0x70, 0x18, 0x48, 0x8b, 0x58, 0x20, 0x48, 0x89, 0x48, 0x20,
            ],
        ),
        (
            0x1c9a8e7,
            &[0x48, 0x8d, 0x05, 0x92, 0xb2, 0xb3, 0x0c, 0x48, 0x89, 0x01],
        ),
        (0x1c9aa3d, &[0x4c, 0x89, 0x76, 0x58, 0x48, 0x89, 0x5e, 0x60]),
        (
            0x1c9aa59,
            &[
                0x89, 0xbe, 0x80, 0, 0, 0, 0x48, 0x8d, 0x8e, 0x88, 0, 0, 0, 0xc7, 0x45, 0xb0, 0, 0,
                0, 0, 0x48, 0x8d, 0x7d, 0xb0, 0x48, 0x89, 0x8d, 0x80, 0, 0, 0, 0x48, 0x89, 0xfa,
                0xe8, 0x60, 0x6b, 0xe9, 0xff,
            ],
        ),
        (0x1c9aacb, &[0x48, 0x89, 0x9e, 0xd0, 0, 0, 0]),
        (0x1c9ac2e, &[0x48, 0x89, 0x9e, 0x10, 0x01, 0, 0]),
        (0x1b31600, &[0x0f, 0x57, 0xf6, 0x0f, 0x11, 0x71, 0x08]),
        (
            0x1b31634,
            &[
                0x48, 0x89, 0x3f, 0x48, 0x89, 0x7f, 0x08, 0x48, 0x89, 0x7e, 0x08,
            ],
        ),
        // The coordinator consumer tests pending visibility and removes consumed
        // keys from all three maps; these are not permanent cache sizes.
        (
            0x1c9ce84,
            &[
                0x48, 0x83, 0xb9, 0x98, 0, 0, 0, 0, 0x0f, 0x84, 0xb1, 0x1d, 0, 0,
            ],
        ),
        (
            0x1c9cf75,
            &[
                0x48, 0x8d, 0x96, 0xc8, 0, 0, 0, 0x48, 0x89, 0x55, 0xb8, 0x48, 0x8d, 0x96, 0x08,
                0x01, 0, 0,
            ],
        ),
        (0x1c9d253, &[0x48, 0xff, 0x88, 0x98, 0, 0, 0]),
        (
            0x1c9e354,
            &[
                0x49, 0x8b, 0x45, 0, 0x48, 0xff, 0x8e, 0xd8, 0, 0, 0, 0x49, 0x8b, 0x4d, 0x08, 0x48,
                0x89, 0x01, 0x49, 0x8b, 0x4d, 0x08, 0x48, 0x89, 0x48, 0x08,
            ],
        ),
        (
            0x1c9e964,
            &[
                0x49, 0x8b, 0x45, 0, 0x48, 0xff, 0x8e, 0x18, 0x01, 0, 0, 0x49, 0x8b, 0x4d, 0x08,
                0x48, 0x89, 0x01, 0x49, 0x8b, 0x4d, 0x08, 0x48, 0x89, 0x48, 0x08,
            ],
        ),
        (
            0x469728b,
            &[0x48, 0x8d, 0x05, 0x1e, 0x74, 0x24, 0x0a, 0x48, 0x89, 0x01],
        ),
        (
            0x466da9c,
            &[
                0x49, 0x89, 0x9f, 0x78, 0x09, 0, 0, 0x49, 0x8d, 0x87, 0x80, 0x09, 0, 0, 0x48, 0x89,
                0x45, 0x50, 0x41, 0x0f, 0x11, 0xb7, 0x80, 0x09, 0, 0, 0x49, 0xc7, 0x87, 0x90, 0x09,
                0, 0, 0, 0, 0, 0, 0x49, 0x89, 0xbf, 0xa0, 0x09, 0, 0,
            ],
        ),
        (0x466dcb1, &[0x49, 0x89, 0x9f, 0x90, 0x09, 0, 0]),
        (
            0x46716c7,
            &[
                0x48, 0x8b, 0x89, 0x90, 0x09, 0, 0, 0x48, 0x8b, 0xbe, 0xa0, 0x09, 0, 0, 0x48, 0x8b,
                0x01, 0x48, 0x8b, 0x40, 0x10, 0x48, 0x8d, 0x55, 0xf4, 0xff, 0x15, 0xc2, 0xa3, 0xa8,
                0x0a, 0x44, 0x8b, 0x45, 0xf4, 0x48, 0x8d, 0x55, 0xe0, 0x48, 0x89, 0xf9, 0xe8, 0x6a,
                0x6c, 0x16, 0,
            ],
        ),
        (
            0x47e6470,
            &[0x48, 0x8d, 0x05, 0xe9, 0xdd, 0x0f, 0x0a, 0x49, 0x89, 0x07],
        ),
        (
            0x47e6545,
            &[
                0x49, 0x89, 0x9f, 0x68, 0x04, 0, 0, 0x48, 0x8b, 0x95, 0xf0, 0x04, 0, 0, 0x49, 0x89,
                0x97, 0x70, 0x04, 0, 0,
            ],
        ),
        (
            DIMENSION_GETTER,
            &[
                0x48, 0x89, 0xd0, 0x8b, 0x89, 0xa0, 0x01, 0, 0, 0x89, 0x0a, 0xc3,
            ],
        ),
    ];
    for &(rva, expected) in sites {
        let mut actual = vec![0; expected.len()];
        crate::world_hooks::read(
            base.checked_add(rva).ok_or("chunk-work address overflow")?,
            &mut actual,
        )?;
        if actual != expected {
            return Err(format!("chunk-work profile differs at {rva:x}"));
        }
    }
    for (table, target) in [(CAMERA_TABLE, 0x46fab50), (COORDINATOR_TABLE, 0x1cba900)] {
        if crate::world_hooks::pointer(base.checked_add(table).ok_or("chunk-work table overflow")?)
            != base.checked_add(target)
        {
            return Err("chunk-work table differs".into());
        }
    }
    crate::native_shared_chunk_work::install(base)?;
    PROFILE.store(base, Ordering::Release);
    Ok(())
}
