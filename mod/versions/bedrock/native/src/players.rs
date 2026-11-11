//! Owned player-list and actor samples for the exact supported client build.
//! No engine constructors, virtual calls, RTTI casts, or guessed entity UUIDs.
//! Layout sources are attributed in THIRD_PARTY.md. Every foreign read is checked.
use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::world_hooks::{floats, pointer, read};

const MAX_PLAYERS: usize = 128;

#[derive(Clone, Serialize)]
pub struct Identity {
    pub id: String,
    pub name: String,
    #[serde(skip)]
    _skin_impl: usize,
}
#[cfg(test)]
impl Identity {
    pub(crate) fn fixture(id: &str, name: &str) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            _skin_impl: 0,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Actor {
    pub identity: Identity,
    pub dimension: Option<&'static str>,
    pub position: [f32; 3],
    pub old_position: [f32; 3],
    pub velocity: [f32; 3],
    pub rotation: [f32; 2],
    pub old_rotation: [f32; 2],
    pub components: Option<Components>,
    pub appearance: Option<crate::native_skins::Appearance>,
    pub chest_empty: Option<bool>,
    pub chest_item: Option<String>,
    pub equipment: Option<crate::native_equipment::Equipment>,
    pub movement: Option<crate::native_input::Movement>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Components {
    pub id: String,
    pub body_yaw: [f32; 2],
    pub head_yaw: [f32; 2],
    pub on_ground: bool,
    pub dead: bool,
    /// ActorGameTypeComponent decision, with a guarded default-mode fallback.
    /// Missing/unrecognized modes stay unknown rather than implying a
    /// nonspectator player.
    pub spectator: Option<bool>,
    pub flags: [u64; 3],
    pub bounds: [f32; 6],
}
impl Components {
    /// ActorFlags::Invisible (5), over the already copied component words.
    /// This says nothing about spectator mode or a player's cape preference.
    pub(crate) fn invisible(&self) -> bool {
        self.flags[0] & (1u64 << 5) != 0
    }
}

/// Copy the native four-byte ActorGameTypeComponent, independently of render
/// culling or selected skin geometry. Cached native consumers at 0x209D80 and
/// 0x19E0840 use hash 0x88D3EDDF and four-byte packed elements. The latter
/// resolves Default (5) through the level's virtual getter. The optional
/// native_game_mode reader copies the pinned getter's data without calling it.
fn spectator(actor: usize) -> Option<bool> {
    use crate::ecs::{actor_context, component_context, hash, registry_key};
    let context = actor_context(actor).ok()?;
    let registry = registry_key(actor)?;
    let kind = hash("ActorGameTypeComponent");
    let source = component_context(context, kind, 4).ok()??;
    let mut before = [0u8; 4];
    read(source, &mut before).ok()?;
    if actor_context(actor).ok()? != context
        || registry_key(actor)? != registry
        || component_context(context, kind, 4).ok()? != Some(source)
    {
        return None;
    }
    let result = match i32::from_le_bytes(before) {
        0..=2 => Some(false), // Survival, Creative, Adventure
        6 => Some(true),
        5 => crate::native_game_mode::spectator(actor),
        _ => None,
    };
    let mut after = [0u8; 4];
    read(source, &mut after).ok()?;
    if before != after
        || actor_context(actor).ok()? != context
        || registry_key(actor)? != registry
        || component_context(context, kind, 4).ok()? != Some(source)
    {
        return None;
    }
    result
}

fn components(actor: usize) -> Option<Components> {
    use crate::ecs::{actor_type, component, contains, hash};
    if actor_type(actor)? != 319 {
        return None;
    }
    let identity = component(actor, hash("UserEntityIdentifierComponent"), 0x250).ok()??;
    let id = uuid(identity.checked_add(0xb8)?)?;
    let body = component(actor, hash("MobBodyRotationComponent"), 8).ok()??;
    let head = component(actor, hash("ActorHeadRotationComponent"), 8).ok()??;
    let flags = component(actor, hash("ActorDataFlagComponent"), 24).ok()??;
    let flags = [
        word(flags)?,
        word(flags.checked_add(8)?)?,
        word(flags.checked_add(16)?)?,
    ];
    let bounds = floats::<6>(
        component(actor, hash("AABBShapeComponent"), 32).ok()??,
        32_000_000.,
    )?;
    if (0..3).any(|i| bounds[i + 3] < bounds[i] || bounds[i + 3] - bounds[i] > 64.) {
        return None;
    }
    Some(Components {
        id,
        body_yaw: floats(body, 360_000.)?,
        head_yaw: floats(head, 360_000.)?,
        on_ground: contains(actor, hash("OnGroundFlagComponent")).ok()?,
        dead: contains(actor, hash("IsDeadFlagComponent")).ok()?,
        spectator: spectator(actor),
        flags,
        bounds,
    })
}

struct Directory {
    level: usize,
    registry: crate::ecs::RegistryKey,
    valid: bool,
    entries: Vec<Identity>,
    at: Instant,
    epoch: u64,
}
impl Directory {
    fn fresh(&self, age: Duration) -> bool {
        self.valid && self.at.elapsed() < age
    }
}
static DIRECTORY: OnceLock<Mutex<Option<Directory>>> = OnceLock::new();
static EPOCH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn directory() -> &'static Mutex<Option<Directory>> {
    DIRECTORY.get_or_init(Mutex::default)
}

fn word(address: usize) -> Option<u64> {
    let mut bytes = [0u8; 8];
    read(address, &mut bytes).ok()?;
    Some(u64::from_le_bytes(bytes))
}

pub(crate) fn string(address: usize, limit: usize) -> Option<String> {
    let mut bytes = [0u8; 32];
    read(address, &mut bytes).ok()?;
    string_header(&bytes, limit)
}
pub(crate) fn string_header(bytes: &[u8; 32], limit: usize) -> Option<String> {
    let length = u64::from_le_bytes(bytes[16..24].try_into().ok()?) as usize;
    let capacity = u64::from_le_bytes(bytes[24..32].try_into().ok()?) as usize;
    if length == 0 || length > limit || capacity < length || capacity > 1024 * 1024 {
        return None;
    }
    let data = if capacity < 16 {
        bytes.get(..length)?.to_vec()
    } else {
        let source = u64::from_le_bytes(bytes[..8].try_into().ok()?) as usize;
        let mut data = vec![0; length];
        read(source, &mut data).ok()?;
        data
    };
    let string = String::from_utf8(data).ok()?;
    (!string.chars().any(char::is_control)).then_some(string)
}

fn uuid(address: usize) -> Option<String> {
    let a = word(address)?;
    let b = word(address.checked_add(8)?)?;
    if a == 0 && b == 0 {
        return None;
    }
    Some(format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        a >> 32,
        (a >> 16) & 0xffff,
        a & 0xffff,
        b >> 48,
        b & 0xffffffffffff
    ))
}

/// The MSVC release unordered_map has its list head at +8 and size at +16.
/// Read only its linked list, never bucket/hash internals. Check both directions,
/// exact length and closure so an in-flight mutation is rejected wholesale.
fn read_directory(level: usize) -> Option<Vec<Identity>> {
    let map = pointer(level.checked_add(0x4e0)?)?;
    let head = pointer(map.checked_add(8)?)?;
    let count = word(map.checked_add(16)?)? as usize;
    if count > MAX_PLAYERS {
        return None;
    }
    let mut node = pointer(head)?;
    let mut previous = head;
    let mut visited = HashSet::new();
    let mut ids = HashSet::new();
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        if node == head || !visited.insert(node) || pointer(node.checked_add(8)?)? != previous {
            return None;
        }
        let id = uuid(node.checked_add(16)?)?;
        if !ids.insert(id.clone()) {
            return None;
        }
        // list node(16), UUID key(16), PlayerListEntry(0xA8).
        let entry = node.checked_add(32)?;
        let name = string(entry.checked_add(0x18)?, 64)?;
        let skin_impl = pointer(entry.checked_add(0x80)?)?;
        entries.push(Identity {
            id,
            name,
            _skin_impl: skin_impl,
        });
        previous = node;
        node = pointer(node)?;
    }
    if node != head
        || pointer(head.checked_add(8)?)? != previous
        || pointer(map.checked_add(8)?)? != head
        || word(map.checked_add(16)?)? as usize != count
    {
        return None;
    }
    Some(entries)
}

pub fn tick(level: usize) {
    refresh(level, crate::world_hooks::current_registry(level));
}
fn replace_directory(
    state: &mut Option<Directory>,
    level: usize,
    registry: Option<crate::ecs::RegistryKey>,
    entries: Option<Vec<Identity>>,
) {
    let Some(registry) = registry else {
        if let Some(state) = state.as_mut() {
            state.valid = false;
            state.entries.clear();
        }
        return;
    };
    let epoch = state
        .as_ref()
        .filter(|s| s.level == level && s.registry == registry)
        .map(|s| s.epoch)
        .unwrap_or_else(|| EPOCH.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
    *state = Some(Directory {
        level,
        registry,
        valid: entries.is_some(),
        entries: entries.unwrap_or_default(),
        at: Instant::now(),
        epoch,
    });
}
fn refresh(level: usize, registry: Option<crate::ecs::RegistryKey>) {
    let entries = registry.and_then(|_| read_directory(level));
    if let Ok(mut state) = directory().try_lock() {
        replace_directory(&mut state, level, registry, entries);
    }
}

pub fn epoch(level: usize) -> Option<u64> {
    let state = directory().try_lock().ok()?;
    let state = state.as_ref()?;
    (state.level == level && state.fresh(Duration::from_millis(500))).then_some(state.epoch)
}

pub fn refresh_if_needed(level: usize) {
    let registry = crate::world_hooks::current_registry(level);
    let fresh = directory().try_lock().ok().is_some_and(|state| {
        state.as_ref().is_some_and(|state| {
            state.level == level
                && Some(state.registry) == registry
                && state.fresh(Duration::from_millis(200))
        })
    });
    if !fresh {
        refresh(level, registry)
    }
}

pub fn actor(address: usize) -> Option<Actor> {
    let (mut actor, source) = {
        let state = directory().try_lock().ok()?;
        let state = state.as_ref()?;
        if !state.fresh(Duration::from_millis(500)) {
            return None;
        }
        if pointer(address.checked_add(0x1d8)?)? != state.level {
            return None;
        }
        if crate::ecs::registry_key(address)? != state.registry {
            return None;
        }
        let actor = actor_from(address, &state.entries)?;
        // Release the directory lock before copying/queuing texture assets.
        let source = actor.identity._skin_impl;
        (actor, source)
    };
    actor.appearance = crate::native_skins::sample(&actor.identity.id, source);
    // Owned render snapshots retain the copied texture key, not a stale native
    // SerializedSkinImpl address. Only the live directory owns that read hint.
    actor.identity._skin_impl = 0;
    Some(actor)
}

/// Lightweight identity join for native tick observers. Does not copy
/// equipment, appearance assets or texture data while an engine call runs.
pub(crate) fn identified_actor(address: usize, session: (usize, u64), id: &str) -> Option<()> {
    let state = directory().try_lock().ok()?;
    let state = state.as_ref()?;
    if state.level != session.0
        || state.epoch != session.1
        || !state.fresh(Duration::from_millis(500))
        || pointer(address.checked_add(0x1d8)?)? != state.level
        || crate::ecs::registry_key(address)? != state.registry
        || crate::ecs::actor_type(address)? != 319
    {
        return None;
    }
    let identity = crate::ecs::component(
        address,
        crate::ecs::hash("UserEntityIdentifierComponent"),
        0x250,
    )
    .ok()??;
    if uuid(identity.checked_add(0xb8)?)? != id
        || state.entries.iter().filter(|entry| entry.id == id).count() != 1
    {
        return None;
    }
    Some(())
}

pub fn session() -> Option<(usize, u64)> {
    let state = directory().try_lock().ok()?;
    let state = state.as_ref()?;
    state
        .fresh(Duration::from_millis(500))
        .then_some((state.level, state.epoch))
}

fn actor_from(address: usize, entries: &[Identity]) -> Option<Actor> {
    use crate::ecs::{component, hash};
    // Join the entity's native UUID to the authoritative level player list.
    // Names and shared skin pointers are neither unique nor stable identities.
    let components = components(address)?;
    let identity = {
        let mut matches = entries.iter().filter(|entry| entry.id == components.id);
        let result = matches.next()?.clone();
        if matches.next().is_some() {
            return None;
        }
        result
    };
    let vector = component(address, hash("StateVectorComponent"), 36).ok()??;
    let rotation = component(address, hash("ActorRotationComponent"), 16).ok()??;
    let equipment = crate::native_equipment::equipment(address);
    let chest = equipment
        .as_ref()
        .and_then(|equipment| equipment.chest.as_ref());
    Some(Actor {
        dimension: crate::native_dimension::sample(address),
        components: Some(components),
        appearance: None,
        chest_empty: chest.as_ref().map(|slot| slot.empty),
        chest_item: chest.and_then(|slot| slot.item.clone()),
        equipment,
        movement: crate::native_input::movement(address),
        identity,
        position: floats(vector, 32_000_000.)?,
        old_position: floats(vector.checked_add(12)?, 32_000_000.)?,
        velocity: floats(vector.checked_add(24)?, 1024.)?,
        rotation: floats(rotation, 360_000.)?,
        old_rotation: floats(rotation.checked_add(8)?, 360_000.)?,
    })
}

pub fn status() -> serde_json::Value {
    let Ok(state) = directory().try_lock() else {
        return serde_json::Value::Null;
    };
    let Some(state) = state.as_ref() else {
        return serde_json::Value::Null;
    };
    if !state.fresh(Duration::from_millis(500)) {
        return serde_json::Value::Null;
    }
    serde_json::json!({"players":state.entries,"levelObserved":state.level!=0,"epoch":state.epoch,"registryId":state.registry.id})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn verified_registry_lifetime_survives_stale_data_but_reused_registry_memory_does_not() {
        let key = crate::ecs::RegistryKey {
            registry: 0x20000,
            id: 7,
        };
        let mut state = None;
        replace_directory(&mut state, 0x10000, Some(key), Some(Vec::new()));
        let epoch = state.as_ref().unwrap().epoch;
        state.as_mut().unwrap().at = Instant::now() - Duration::from_secs(2);
        assert!(!state.as_ref().unwrap().fresh(Duration::from_millis(500)));
        replace_directory(&mut state, 0x10000, Some(key), Some(Vec::new()));
        assert_eq!(state.as_ref().unwrap().epoch, epoch);
        assert!(state.as_ref().unwrap().fresh(Duration::from_millis(500)));
        replace_directory(&mut state, 0x10000, Some(key), None);
        assert_eq!(state.as_ref().unwrap().epoch, epoch);
        assert!(!state.as_ref().unwrap().fresh(Duration::from_millis(500)));
        replace_directory(&mut state, 0x10000, None, None);
        assert!(!state.as_ref().unwrap().fresh(Duration::from_millis(500)));
        replace_directory(&mut state, 0x10000, Some(key), Some(Vec::new()));
        assert_eq!(state.as_ref().unwrap().epoch, epoch);
        let new_key = crate::ecs::RegistryKey { id: 8, ..key };
        replace_directory(&mut state, 0x10000, Some(new_key), Some(Vec::new()));
        assert_ne!(state.as_ref().unwrap().epoch, epoch);
        let new_epoch = state.as_ref().unwrap().epoch;
        replace_directory(&mut state, 0x10001, Some(new_key), Some(Vec::new()));
        assert_ne!(state.as_ref().unwrap().epoch, new_epoch);
    }
    fn point(data: &mut [u8], at: usize, value: usize) {
        data[at..at + 8].copy_from_slice(&(value as u64).to_le_bytes());
    }
    fn name(data: &mut [u8], at: usize, text: &[u8]) {
        data[at..at + text.len()].copy_from_slice(text);
        point(data, at + 16, text.len());
        point(data, at + 24, 15);
    }
    #[test]
    fn directory_copies_uuid_and_name_checks_list_integrity_and_matches_entity_uuid() {
        let mut level = vec![0u8; 0x4e8];
        let mut map = vec![0u8; 64];
        let mut head = vec![0u8; 16];
        let mut node = vec![0u8; 200];
        let skin = Box::new(0u64);
        let skin_ptr = &*skin as *const u64 as usize;
        point(&mut level, 0x4e0, map.as_ptr() as usize);
        point(&mut map, 8, head.as_ptr() as usize);
        point(&mut map, 16, 1);
        point(&mut head, 0, node.as_ptr() as usize);
        point(&mut head, 8, node.as_ptr() as usize);
        point(&mut node, 0, head.as_ptr() as usize);
        point(&mut node, 8, head.as_ptr() as usize);
        node[16..24].copy_from_slice(&0xb50ad385829d3141u64.to_le_bytes());
        node[24..32].copy_from_slice(&0xa2167e7d7539ba7fu64.to_le_bytes());
        name(&mut node, 32 + 0x18, b"ArcticTest");
        point(&mut node, 32 + 0x80, skin_ptr);
        let owned = read_directory(level.as_ptr() as usize).unwrap();
        assert_eq!(owned[0].id, "b50ad385-829d-3141-a216-7e7d7539ba7f");
        assert_eq!(owned[0].name, "ArcticTest");
        fn floats(values: &[f32]) -> Vec<u8> {
            values.iter().flat_map(|v| v.to_le_bytes()).collect()
        }
        let mut identity = vec![0u8; 0x250];
        identity[0xb8..0xc8].copy_from_slice(&node[16..32]);
        let state = floats(&[5., 0., 0., 4., 0., 0., 0., 0., 0.]);
        let rotation = floats(&[12., 34., 10., 30.]);
        let bounds = floats(&[4.7, 0., -0.3, 5.3, 1.8, 0.3, 0.6, 1.8]);
        let fixture = crate::ecs::Fixture::new(&[
            ("ActorTypeComponent", &319u32.to_le_bytes()),
            ("UserEntityIdentifierComponent", &identity),
            ("StateVectorComponent", &state),
            ("ActorRotationComponent", &rotation),
            ("AABBShapeComponent", &bounds),
            ("MobBodyRotationComponent", &[0; 8]),
            ("ActorHeadRotationComponent", &[0; 8]),
            ("ActorDataFlagComponent", &[0; 24]),
            ("IsDeadFlagComponent", &[]),
        ]);
        let sample = actor_from(fixture.address(), &owned).unwrap();
        assert_eq!(sample.position, [5., 0., 0.]);
        assert_eq!(sample.old_position, [4., 0., 0.]);
        assert!(sample.components.as_ref().unwrap().dead);
        let other = [Identity::fixture(
            "00000000-0000-0000-0000-000000000001",
            "ArcticTest",
        )];
        assert!(actor_from(fixture.address(), &other).is_none());
        assert!(actor_from(fixture.address(), &[owned[0].clone(), owned[0].clone()]).is_none());
        let node_address = node.as_ptr() as usize;
        point(&mut node, 0, node_address);
        assert!(read_directory(level.as_ptr() as usize).is_none());
        assert_eq!(sample.identity.name, "ArcticTest");
    }
    #[test]
    fn foreign_strings_reject_unbounded_lengths_controls_invalid_utf8_and_dead_heap_pointers() {
        let mut s = [0u8; 32];
        name(&mut s, 0, b"okay");
        assert_eq!(string(s.as_ptr() as usize, 64).as_deref(), Some("okay"));
        point(&mut s, 16, 1000);
        assert!(string(s.as_ptr() as usize, 64).is_none());
        name(&mut s, 0, b"bad\n");
        assert!(string(s.as_ptr() as usize, 64).is_none());
        s[0] = 255;
        assert!(string(s.as_ptr() as usize, 64).is_none());
        point(&mut s, 0, 1);
        point(&mut s, 16, 20);
        point(&mut s, 24, 31);
        assert!(string(s.as_ptr() as usize, 64).is_none());
    }
}
