//! Bounded shared-chunk build state, not visible geometry or GPU readiness.
//! All observations belong to the selected coordinator and one world pass.
use crate::world_hooks::read;
use serde_json::{Value, json};
use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};

const COORDINATOR_TABLE: usize = 0xe7d5b80;
const SHARED_OWNER_TABLE: usize = 0xe77c3d0;
const MAX_CHUNKS: usize = 32_768;
const MAX_READS: usize = MAX_CHUNKS * 4 + 8;
const MAX_BYTES: usize = MAX_CHUNKS * 112 + 512;
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
#[derive(PartialEq, Eq)]
struct Guard {
    address: usize,
    offset: usize,
    length: usize,
}
#[derive(PartialEq, Eq)]
struct Owner {
    control: usize,
    live: bool,
}
fn owner_live(control: usize) -> Option<bool> {
    let mut counts = [0; 8];
    read(control.checked_add(8)?, &mut counts).ok()?;
    let strong = integer(&counts, 0)?;
    let weak = integer(&counts, 4)?;
    if strong < 0 || weak <= 0 {
        return None;
    }
    Some(strong > 0)
}
// A separate, packed budget for a bounded weak map. Do not increase the small
// network/task metadata copier's budget or allocate one Vec for every field.
#[derive(Default, PartialEq, Eq)]
struct Reads {
    guards: Vec<Guard>,
    bytes: Vec<u8>,
    owners: Vec<Owner>,
}
impl Reads {
    fn copy<const N: usize>(&mut self, address: usize) -> Option<[u8; N]> {
        if N == 0
            || N > 64
            || address < 0x10000
            || address.checked_add(N)? > 0x0000_8000_0000_0000
            || self.guards.len() >= MAX_READS
            || self.bytes.len().checked_add(N)? > MAX_BYTES
        {
            return None;
        }
        let mut bytes = [0; N];
        read(address, &mut bytes).ok()?;
        let offset = self.bytes.len();
        self.bytes.extend_from_slice(&bytes);
        self.guards.push(Guard {
            address,
            offset,
            length: N,
        });
        Some(bytes)
    }
    fn owner(&mut self, control: usize) -> Option<bool> {
        if self.owners.len() >= MAX_CHUNKS {
            return None;
        }
        let live = owner_live(control)?;
        self.owners.push(Owner { control, live });
        Some(live)
    }
    fn unchanged(&self) -> Option<()> {
        self.owners_unchanged()?;
        let mut buffer = [0; 64];
        for guard in &self.guards {
            read(guard.address, &mut buffer[..guard.length]).ok()?;
            if buffer[..guard.length]
                != self.bytes[guard.offset..guard.offset.checked_add(guard.length)?]
            {
                return None;
            }
        }
        self.owners_unchanged()
    }
    fn owners_unchanged(&self) -> Option<()> {
        // Check before and after payload guards. This does not acquire a strong
        // owner; native drops racing a read still invalidate the observation.
        // Counts may change while remaining positive; store only the category.
        for owner in &self.owners {
            if owner_live(owner.control)? != owner.live {
                return None;
            }
        }
        Some(())
    }
}
#[derive(PartialEq, Eq)]
pub(crate) struct Sample {
    coordinator: usize,
    renderer: usize,
    level: usize,
    epoch: u64,
    tick: u64,
    dimension: i32,
    keys: usize,
    live: usize,
    expired: usize,
    empty: usize,
    building: usize,
    visibility_building: usize,
    visibility_dirty: usize,
    visibility_ready: usize,
    reads: Reads,
}
impl Sample {
    pub(crate) fn copy(
        coordinator: usize,
        renderer: usize,
        level: usize,
        epoch: u64,
        tick: u64,
        dimension: i32,
    ) -> Option<Self> {
        let base = PROFILE.load(Ordering::Acquire);
        if base == 0
            || crate::world_hooks::image_base() != Some(base)
            || crate::players::session() != Some((level, epoch))
        {
            return None;
        }
        let mut reads = Reads::default();
        if pointer(&reads.copy::<8>(coordinator)?, 0)? != base.checked_add(COORDINATOR_TABLE)? {
            return None;
        }
        let owner = reads.copy::<0x2c>(coordinator.checked_add(0x58)?)?;
        if pointer(&owner, 0)? != level
            || pointer(&owner, 8)? != renderer
            || integer(&owner, 0x28)? != dimension
        {
            return None;
        }
        let map = reads.copy::<64>(coordinator.checked_add(0x18)?)?;
        let head = pointer(&map, 8)?;
        let keys = word(&map, 16)?;
        if keys > MAX_CHUNKS {
            return None;
        }
        let links = reads.copy::<16>(head)?;
        let mut node = pointer(&links, 0)?;
        let last = pointer(&links, 8)?;
        let mut previous = head;
        reads.guards.reserve(keys * 4);
        reads.bytes.reserve(keys * 112);
        reads.owners.reserve(keys);
        let mut positions = HashSet::with_capacity(keys);
        let mut live = 0;
        let mut expired = 0;
        let mut empty = 0;
        let mut building = 0;
        let mut visibility_building = 0;
        let mut visibility_dirty = 0;
        let mut visibility_ready = 0;
        for _ in 0..keys {
            if node == head {
                return None;
            }
            let entry = reads.copy::<0x30>(node)?;
            if pointer(&entry, 8)? != previous {
                return None;
            }
            let position = [
                integer(&entry, 0x10)?,
                integer(&entry, 0x14)?,
                integer(&entry, 0x18)?,
            ];
            if !positions.insert(position) {
                return None;
            }
            let object = word(&entry, 0x20)?;
            let control = word(&entry, 0x28)?;
            if object == 0 && control == 0 {
                empty += 1;
            } else {
                pointer(&entry, 0x20)?;
                pointer(&entry, 0x28)?;
                if control.checked_add(0x10)? != object
                    || pointer(&reads.copy::<8>(control)?, 0)?
                        != base.checked_add(SHARED_OWNER_TABLE)?
                {
                    return None;
                }
                if reads.owner(control)? {
                    let state = reads.copy::<0x24>(object)?;
                    // Native constructor stores the chunk's minimum block pos.
                    // This also distinguishes a copied/aliased wrong map value.
                    for (axis, at) in [0x14, 0x18, 0x1c].into_iter().enumerate() {
                        if integer(&state, at)? != position[axis].checked_mul(16)? {
                            return None;
                        }
                    }
                    // Assigned from a native atomic sequence in the constructor.
                    // Guard it independently of reusable addresses and states.
                    if word(&reads.copy::<8>(object.checked_add(0x80)?)?, 0)? == 0 {
                        return None;
                    }
                    if state[..4].iter().any(|flag| *flag > 1) {
                        return None;
                    }
                    let build = u32::from_le_bytes(state[4..8].try_into().ok()?);
                    let visibility = u32::from_le_bytes(state[0x20..0x24].try_into().ok()?);
                    if build > 1 || visibility > 3 {
                        return None;
                    }
                    live += 1;
                    if build == 0 {
                        building += 1;
                    }
                    match visibility {
                        0 => visibility_dirty += 1,
                        1 | 2 => visibility_building += 1,
                        3 => visibility_ready += 1,
                        _ => return None,
                    }
                } else {
                    // No payload/identity read after the last strong owner died.
                    expired += 1;
                }
            }
            previous = node;
            node = pointer(&entry, 0)?;
        }
        if node != head || previous != last {
            return None;
        }
        let sample = Self {
            coordinator,
            renderer,
            level,
            epoch,
            tick,
            dimension,
            keys,
            live,
            expired,
            empty,
            building,
            visibility_building,
            visibility_dirty,
            visibility_ready,
            reads,
        };
        sample.live().then_some(sample)
    }
    pub(crate) fn live(&self) -> bool {
        self.reads.unchanged().is_some()
            && crate::players::session() == Some((self.level, self.epoch))
            && crate::world_hooks::client_completed_tick(self.level) == Some(self.tick)
            && self.reads.unchanged().is_some()
    }
    pub(crate) fn idle(&self) -> bool {
        self.building == 0 && self.visibility_building == 0
    }
    pub(crate) fn receipt(&self) -> Value {
        json!({"observed":true,"chunkKeys":self.keys,"liveChunks":self.live,
            "expiredWeakChunks":self.expired,"emptyWeakChunks":self.empty,
            "buildingOrSortingChunks":self.building,"visibilityBuildingChunks":self.visibility_building,
            "visibilityDirtyChunks":self.visibility_dirty,"visibilityReadyChunks":self.visibility_ready,
            "observedIdle":self.idle(),"visibleSetVerified":false,"terrainReady":false})
    }
}
pub(crate) fn install(base: usize) -> Result<(), String> {
    if crate::world_hooks::image_base() != Some(base) {
        return Err("unvalidated shared-chunk-work image".into());
    }
    let sites: &[(usize, &[u8])] = &[
        (
            0x1ca19b9,
            &[
                0x48, 0xb8, 0x01, 0, 0, 0, 0x01, 0, 0, 0, 0x48, 0x89, 0x46, 0x08, 0x48, 0x8d, 0x05,
                0x02, 0xaa, 0xad, 0x0c, 0x48, 0x89, 0x06, 0x49, 0x89, 0xf6, 0x49, 0x83, 0xc6, 0x10,
                0x4c, 0x89, 0xf1, 0x4c, 0x89, 0xea, 0x48, 0x89, 0x75, 0xf8, 0xe8, 0xa9, 0x49, 0, 0,
            ],
        ),
        (
            0x1ca1a2c,
            &[
                0x48, 0x8b, 0x46, 0x08, 0x48, 0x85, 0xc0, 0x74, 0x09, 0x4c, 0x8b, 0x36, 0xf0, 0xff,
                0x40, 0x0c, 0xeb, 0x05, 0x31, 0xc0, 0x45, 0x31, 0xf6, 0x4c, 0x89, 0x73, 0x20, 0x48,
                0x8b, 0x4b, 0x28, 0x48, 0x89, 0x43, 0x28,
            ],
        ),
        (
            0x1ca63ae,
            &[
                0xf2, 0x0f, 0x10, 0x05, 0x0a, 0xf7, 0xb2, 0x0c, 0x0f, 0x11, 0x01,
            ],
        ),
        (0xe7d5ac0, &[0, 0, 0, 0, 0x01, 0, 0, 0]),
        (
            0x1ca63c0,
            &[
                0x8b, 0x42, 0x08, 0xc1, 0xe0, 0x04, 0x8b, 0x0a, 0x8b, 0x52, 0x04, 0xc1, 0xe2, 0x04,
                0xc1, 0xe1, 0x04, 0x41, 0x89, 0x4f, 0x14, 0x41, 0x89, 0x57, 0x18, 0x41, 0x89, 0x47,
                0x1c, 0x41, 0xc7, 0x47, 0x20, 0, 0, 0, 0,
            ],
        ),
        (
            0x1ca6417,
            &[
                0xb8, 0x01, 0, 0, 0, 0xf0, 0x48, 0x0f, 0xc1, 0x05, 0x0b, 0xd3, 0x0a, 0x10, 0x4d,
                0x8d, 0x67, 0x28, 0x48, 0xff, 0xc0, 0x49, 0x89, 0x87, 0x80, 0, 0, 0,
            ],
        ),
        (
            0x1ca7d92,
            &[
                0x31, 0xc9, 0xb8, 0x01, 0, 0, 0, 0xf0, 0x0f, 0xb1, 0x4f, 0x04,
            ],
        ),
        (
            0x1c9da18,
            &[
                0xb8, 0x01, 0, 0, 0, 0xba, 0x02, 0, 0, 0, 0xf0, 0x0f, 0xb1, 0x51, 0x20,
            ],
        ),
        (
            0x1c9da31,
            &[
                0x48, 0x8b, 0x4d, 0x30, 0xb8, 0x03, 0, 0, 0, 0x31, 0xd2, 0xf0, 0x0f, 0xb1, 0x51,
                0x20, 0x75, 0x0c,
            ],
        ),
        (
            0x1ca02f5,
            &[
                0x48, 0x8b, 0x4e, 0x20, 0x48, 0x8b, 0x11, 0x45, 0x31, 0xff, 0x41, 0xb8, 0, 0, 0, 0,
                0x48, 0x39, 0xca,
            ],
        ),
        (
            0x1ca032c,
            &[
                0x4c, 0x8b, 0x4a, 0x28, 0x4d, 0x85, 0xc9, 0x74, 0xeb, 0x41, 0x83, 0x79, 0x08, 0,
                0x74, 0xe4, 0x41, 0xff, 0xc0,
            ],
        ),
    ];
    for &(rva, expected) in sites {
        let mut actual = vec![0; expected.len()];
        read(
            base.checked_add(rva)
                .ok_or("shared-chunk-work address overflow")?,
            &mut actual,
        )?;
        if actual != expected {
            return Err(format!("shared-chunk-work profile differs at {rva:x}"));
        }
    }
    PROFILE.store(base, Ordering::Release);
    Ok(())
}
