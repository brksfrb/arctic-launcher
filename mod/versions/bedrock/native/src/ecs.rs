//! Checked copies of EnTT components for the pinned executable. Storage traversal
//! was checked against installed code at RVAs 0x19f1e90 and 0x1eb704 (see profile).
//! These functions never call the engine's throwing `get` path or borrow its data.
use crate::native_scene_layout::entity_registry as registry_layout;
use crate::world_hooks::{pointer, read};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
static WEAK_ATTEMPTS: AtomicU64 = AtomicU64::new(0);
static WEAK_RESOLVED: AtomicU64 = AtomicU64::new(0);
static WEAK_STEP: AtomicU32 = AtomicU32::new(0);
pub(crate) fn weak_status() -> serde_json::Value {
    serde_json::json!({"attempts":WEAK_ATTEMPTS.load(Ordering::Relaxed),"resolved":WEAK_RESOLVED.load(Ordering::Relaxed),"step":WEAK_STEP.load(Ordering::Relaxed)})
}

pub const fn hash(name: &str) -> u32 {
    let mut value = 2166136261u32;
    let mut i = 0;
    while i < name.len() {
        value = (value ^ name.as_bytes()[i] as u32).wrapping_mul(16777619);
        i += 1;
    }
    value
}
fn u32_at(address: usize) -> Result<u32, ()> {
    let mut bytes = [0u8; 4];
    read(address, &mut bytes).map_err(|_| ())?;
    Ok(u32::from_le_bytes(bytes))
}
fn u64_at(address: usize) -> Result<u64, ()> {
    let mut bytes = [0u8; 8];
    read(address, &mut bytes).map_err(|_| ())?;
    Ok(u64::from_le_bytes(bytes))
}
fn at(base: usize, offset: usize) -> Result<usize, ()> {
    base.checked_add(offset).ok_or(())
}
fn vector(base: usize, offset: usize, stride: usize, max: usize) -> Result<(usize, usize), ()> {
    let start = u64_at(at(base, offset)?)? as usize;
    let end = u64_at(at(base, offset + 8)?)? as usize;
    let bytes = end.checked_sub(start).ok_or(())?;
    if !bytes.is_multiple_of(stride) || bytes / stride > max {
        return Err(());
    }
    Ok((start, bytes / stride))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Context {
    pub registry: usize,
    pub entity: u32,
}
/// Native EntityRegistry construction assigns this ID from a global atomic
/// counter (0x25BFCD5 -> owner+0x160). It distinguishes reused registry memory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RegistryKey {
    pub registry: usize,
    pub id: u32,
}
pub(crate) fn registry_key(actor: usize) -> Option<RegistryKey> {
    let context = actor_context(actor).ok()?;
    let owner = context.registry.checked_sub(registry_layout::REGISTRY)?;
    let key = owner_registry_key(owner, context.entity)?;
    if actor_context(actor).ok()? != context || owner_registry_key(owner, context.entity)? != key {
        return None;
    }
    Some(key)
}
/// Remember a registry construction identity across callbacks. This reads and
/// rechecks liveness/ID; it does not acquire a reference or an ownership lease.
pub(crate) fn owner_registry_key(owner: usize, entity: u32) -> Option<RegistryKey> {
    let id = u32_at(owner.checked_add(registry_layout::ID)?).ok()?;
    weak_alive(owner, entity)?;
    if u32_at(owner.checked_add(registry_layout::ID)?).ok()? != id {
        return None;
    }
    Some(RegistryKey {
        registry: owner.checked_add(registry_layout::REGISTRY)?,
        id,
    })
}
pub(crate) fn actor_context(actor: usize) -> Result<Context, ()> {
    Ok(Context {
        registry: pointer(at(actor, 0x10)?).ok_or(())?,
        entity: u32_at(at(actor, 0x18)?)?,
    })
}
fn storage(registry: usize, kind: u32) -> Result<Option<usize>, ()> {
    let (buckets, bucket_count) = vector(registry, 0x48, 8, 16384)?;
    if bucket_count == 0 {
        return Ok(None);
    }
    if !bucket_count.is_power_of_two() {
        return Err(());
    }
    let (nodes, node_count) = vector(registry, 0x68, 32, 4096)?;
    let mut index = u64_at(at(buckets, (kind as usize & (bucket_count - 1)) * 8)?)?;
    let mut steps = 0;
    let storage = loop {
        if index == u64::MAX {
            return Ok(None);
        }
        if index as usize >= node_count || steps >= node_count.min(256) {
            return Err(());
        }
        steps += 1;
        let node = at(nodes, index as usize * 32)?;
        if u32_at(at(node, 8)?)? == kind {
            break pointer(at(node, 16)?).ok_or(())?;
        }
        index = u64_at(node)?;
    };
    Ok(Some(storage))
}
fn slot(context: Context, kind: u32) -> Result<Option<(usize, u32)>, ()> {
    let Context { registry, entity } = context;
    let Some(storage) = storage(registry, kind)? else {
        return Ok(None);
    };
    let entity_index = entity & 0x3ffff;
    let (sparse, page_count) = vector(storage, 8, 8, 128)?;
    let page = (entity_index >> 11) as usize;
    if page >= page_count {
        return Ok(None);
    }
    let Some(sparse_page) = pointer(at(sparse, page * 8)?) else {
        return Ok(None);
    };
    let packed = u32_at(at(sparse_page, (entity_index & 0x7ff) as usize * 4)?)?;
    if ((entity & 0xfffc0000) ^ packed) > 0x3fffe {
        return Ok(None);
    }
    Ok(Some((storage, packed & 0x3ffff)))
}

/// Pinned render-camera selection reads the first packed tag entity. Check
/// bounded storage headers and its sparse generation rather than calling get.
pub(crate) fn first_entity(registry: usize, kind: u32) -> Result<Option<u32>, ()> {
    let Some(storage) = storage(registry, kind)? else {
        return Ok(None);
    };
    let before = vector(storage, 0x20, 4, 128)?;
    if before.1 == 0 {
        return Ok(None);
    }
    let entity = u32_at(before.0)?;
    if slot(Context { registry, entity }, kind)? != Some((storage, 0))
        || vector(storage, 0x20, 4, 128)? != before
    {
        return Err(());
    }
    Ok(Some(entity))
}

pub fn contains(actor: usize, kind: u32) -> Result<bool, ()> {
    contains_context(actor_context(actor)?, kind)
}
pub(crate) fn contains_context(context: Context, kind: u32) -> Result<bool, ()> {
    Ok(slot(context, kind)?.is_some())
}

pub fn component(actor: usize, kind: u32, stride: usize) -> Result<Option<usize>, ()> {
    component_context(actor_context(actor)?, kind, stride)
}
pub(crate) fn component_context(
    context: Context,
    kind: u32,
    stride: usize,
) -> Result<Option<usize>, ()> {
    if stride == 0 || stride > 4096 {
        return Err(());
    }
    let Some((storage, index)) = slot(context, kind)? else {
        return Ok(None);
    };
    let pages = pointer(at(storage, 0x50)?).ok_or(())?;
    let page = pointer(at(pages, (index >> 7) as usize * 8)?).ok_or(())?;
    Ok(Some(at(page, (index & 0x7f) as usize * stride)?))
}

/// Read-only equivalent of the pinned WeakEntityRef liveness checks in
/// 0x25C0380. The registry owner and generation must still be live. This never
/// increments engine reference counts or constructs an EntityContext.
fn weak_header(reference: usize) -> Option<(usize, usize, u32)> {
    Some((
        pointer(reference)?,
        pointer(reference.checked_add(8)?)?,
        u32_at(reference.checked_add(16)?).ok()?,
    ))
}
fn weak_alive(owner: usize, entity: u32) -> Option<()> {
    let index = entity & 0x3ffff;
    if index == 0x3ffff {
        return None;
    }
    let (sparse, count) = vector(owner, 0x110, 8, 128).ok()?;
    let page = (index >> 11) as usize;
    if page >= count {
        return None;
    }
    let page = pointer(sparse.checked_add(page * 8)?)?;
    let packed = u32_at(page.checked_add((index & 0x7ff) as usize * 4)?).ok()?;
    if ((entity & 0xfffc0000) ^ packed) > 0x3fffe
        || u64_at(owner.checked_add(0x150)?).ok()? <= u64::from(packed & 0x3ffff)
    {
        return None;
    }
    Some(())
}
pub(crate) fn weak_context(reference: usize) -> Option<Context> {
    let before @ (owner, control, entity) = weak_header(reference)?;
    if !(1..=i32::MAX as u32).contains(&u32_at(control.checked_add(8)?).ok()?) {
        return None;
    }
    weak_alive(owner, entity)?;
    let context = Context {
        registry: owner.checked_add(registry_layout::REGISTRY)?,
        entity,
    };
    if weak_header(reference)? != before || u32_at(control.checked_add(8)?).ok()? == 0 {
        return None;
    }
    weak_alive(owner, entity)?;
    Some(context)
}
/// Resolve a registry-owner/entity pair supplied by an active native callback.
/// Sparse generation and packed membership are checked before any component read.
pub(crate) fn owner_context(owner: usize, entity: u32) -> Option<Context> {
    weak_alive(owner, entity)?;
    Some(Context {
        registry: owner.checked_add(registry_layout::REGISTRY)?,
        entity,
    })
}
pub(crate) fn weak_local_player(reference: usize) -> Option<usize> {
    WEAK_ATTEMPTS.fetch_add(1, Ordering::Relaxed);
    WEAK_STEP.store(0, Ordering::Relaxed);
    let before @ (owner, control, entity) = weak_header(reference)?;
    WEAK_STEP.store(1, Ordering::Relaxed);
    if !(1..=i32::MAX as u32).contains(&u32_at(control.checked_add(8)?).ok()?) {
        return None;
    }
    WEAK_STEP.store(2, Ordering::Relaxed);
    weak_alive(owner, entity)?;
    WEAK_STEP.store(3, Ordering::Relaxed);
    let context = Context {
        registry: owner.checked_add(registry_layout::REGISTRY)?,
        entity,
    };
    // The installed getLocalPlayer checks this tag, then ActorOwnerComponent.
    if !contains_context(context, hash("LocalPlayerComponent")).ok()? {
        return None;
    }
    WEAK_STEP.store(4, Ordering::Relaxed);
    let component = component_context(context, hash("ActorOwnerComponent"), 8).ok()??;
    let actor = pointer(component)?;
    WEAK_STEP.store(5, Ordering::Relaxed);
    if actor_context(actor).ok()? != context {
        return None;
    }
    WEAK_STEP.store(6, Ordering::Relaxed);
    if actor_type(actor)? != 319 {
        return None;
    }
    WEAK_STEP.store(7, Ordering::Relaxed);
    if weak_header(reference)? != before || u32_at(control.checked_add(8)?).ok()? == 0 {
        return None;
    }
    weak_alive(owner, entity)?;
    WEAK_STEP.store(8, Ordering::Relaxed);
    WEAK_RESOLVED.fetch_add(1, Ordering::Relaxed);
    Some(actor)
}

pub fn actor_type(actor: usize) -> Option<u32> {
    u32_at(component(actor, hash("ActorTypeComponent"), 4).ok()??).ok()
}

/// Owns a small real MSVC/EnTT-shaped memory graph for checked-reader tests.
/// The entity lives at packed index 3, so incorrect component strides do not
/// accidentally pass by reading the first payload element.
#[cfg(test)]
// Each one-element table is separately allocated so addresses already embedded
// in foreign memory remain stable when its owning vector reallocates.
#[allow(clippy::vec_box)]
pub(crate) struct Fixture {
    actor: Vec<u8>,
    _registry: Vec<u8>,
    _buckets: Vec<u8>,
    _nodes: Vec<u8>,
    _storage: Vec<Vec<u8>>,
    _sparse: Vec<Box<usize>>,
    _sparse_pages: Vec<Vec<u8>>,
    _pages: Vec<Box<usize>>,
    _payload: Vec<Vec<u8>>,
}
#[cfg(test)]
impl Fixture {
    pub fn new(components: &[(&str, &[u8])]) -> Self {
        fn point(b: &mut [u8], i: usize, v: usize) {
            b[i..i + 8].copy_from_slice(&(v as u64).to_le_bytes());
        }
        let mut actor = vec![0; 32];
        let mut registry = vec![0; 0x80];
        let mut buckets = vec![0xff; 8];
        let mut nodes = vec![0; components.len() * 32];
        let mut storage = Vec::new();
        let mut sparse = Vec::new();
        let mut sparse_pages = Vec::new();
        let mut pages = Vec::new();
        let mut payload = Vec::new();
        point(&mut actor, 0x10, registry.as_ptr() as usize);
        actor[0x18..0x1c].copy_from_slice(&0x40007u32.to_le_bytes());
        point(&mut registry, 0x48, buckets.as_ptr() as usize);
        point(&mut registry, 0x50, buckets.as_ptr() as usize + 8);
        point(&mut registry, 0x68, nodes.as_ptr() as usize);
        point(&mut registry, 0x70, nodes.as_ptr() as usize + nodes.len());
        if !components.is_empty() {
            point(&mut buckets, 0, 0);
        }
        for (i, (name, bytes)) in components.iter().enumerate() {
            let mut data = vec![0; bytes.len() * 128];
            data[bytes.len() * 3..bytes.len() * 4].copy_from_slice(bytes);
            let page = Box::new(data.as_ptr() as usize);
            let mut sparse_page = vec![0xff; 2048 * 4];
            sparse_page[28..32].copy_from_slice(&0x40003u32.to_le_bytes());
            let sparse_table = Box::new(sparse_page.as_ptr() as usize);
            let mut s = vec![0; 0x68];
            let address = &*sparse_table as *const usize as usize;
            point(&mut s, 8, address);
            point(&mut s, 16, address + 8);
            point(&mut s, 0x50, &*page as *const usize as usize);
            point(
                &mut nodes,
                i * 32,
                if i + 1 == components.len() {
                    usize::MAX
                } else {
                    i + 1
                },
            );
            nodes[i * 32 + 8..i * 32 + 12].copy_from_slice(&hash(name).to_le_bytes());
            point(&mut nodes, i * 32 + 16, s.as_ptr() as usize);
            storage.push(s);
            sparse.push(sparse_table);
            sparse_pages.push(sparse_page);
            pages.push(page);
            payload.push(data);
        }
        Self {
            actor,
            _registry: registry,
            _buckets: buckets,
            _nodes: nodes,
            _storage: storage,
            _sparse: sparse,
            _sparse_pages: sparse_pages,
            _pages: pages,
            _payload: payload,
        }
    }
    pub fn address(&self) -> usize {
        std::hint::black_box(self.actor.as_ptr() as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn point(bytes: &mut [u8], offset: usize, value: usize) {
        bytes[offset..offset + 8].copy_from_slice(&(value as u64).to_le_bytes())
    }
    #[test]
    fn render_camera_selection_checks_first_packed_tag_and_rejects_stale_sparse_generation() {
        let mut fixture = Fixture::new(&[("MinecraftCamera::RenderCameraComponent", &[0])]);
        let registry = actor_context(fixture.address()).unwrap().registry;
        let mut packed = [0x40007u32, 0x40008];
        point(&mut fixture._storage[0], 0x20, packed.as_ptr() as usize);
        point(&mut fixture._storage[0], 0x28, packed.as_ptr() as usize + 8);
        // Packed index 3 in the generic fixture is deliberately not the first.
        assert!(first_entity(registry, hash("MinecraftCamera::RenderCameraComponent")).is_err());
        fixture._sparse_pages[0][28..32].copy_from_slice(&0x40000u32.to_le_bytes());
        assert_eq!(
            first_entity(registry, hash("MinecraftCamera::RenderCameraComponent")),
            Ok(Some(0x40007))
        );
        packed[0] = 0x80007;
        std::hint::black_box(&packed);
        assert!(first_entity(registry, hash("MinecraftCamera::RenderCameraComponent")).is_err());
        point(&mut fixture._storage[0], 0x28, packed.as_ptr() as usize);
        assert_eq!(
            first_entity(registry, hash("MinecraftCamera::RenderCameraComponent")),
            Ok(None)
        );
    }
    #[test]
    fn weak_local_player_requires_live_generation_tag_owner_and_matching_actor() {
        let mut fixture = Fixture::new(&[
            ("LocalPlayerComponent", &[0]),
            ("ActorOwnerComponent", &[0; 8]),
            ("ActorTypeComponent", &319u32.to_le_bytes()),
        ]);
        let actor = fixture.actor.as_ptr() as usize;
        let mut owner = vec![0u8; 0x168];
        owner[0x160..0x164].copy_from_slice(&41u32.to_le_bytes());
        owner[0x30..0xb0].copy_from_slice(&fixture._registry);
        point(&mut fixture.actor, 0x10, owner.as_ptr() as usize + 0x30);
        point(&mut fixture._payload[1], 24, actor);
        let mut sparse_page = vec![0xffu8; 2048 * 4];
        sparse_page[28..32].copy_from_slice(&0x40003u32.to_le_bytes());
        let sparse = [sparse_page.as_ptr() as usize];
        point(&mut owner, 0x110, sparse.as_ptr() as usize);
        point(&mut owner, 0x118, sparse.as_ptr() as usize + 8);
        point(&mut owner, 0x150, 4);
        let mut control = [0u8; 16];
        control[8..12].copy_from_slice(&1u32.to_le_bytes());
        let mut reference = [0u8; 24];
        point(&mut reference, 0, owner.as_ptr() as usize);
        point(&mut reference, 8, control.as_ptr() as usize);
        reference[16..20].copy_from_slice(&0x40007u32.to_le_bytes());
        let at = reference.as_ptr() as usize;
        assert_eq!(weak_local_player(at), Some(actor));
        let context = actor_context(actor).unwrap();
        assert_eq!(weak_context(at), Some(context));
        let registry = registry_key(actor).unwrap();
        assert_eq!(registry.id, 41);
        owner[0x160..0x164].copy_from_slice(&42u32.to_le_bytes());
        std::hint::black_box(&owner);
        assert_ne!(registry_key(actor), Some(registry));
        control[8..12].fill(0);
        std::hint::black_box(&control);
        assert!(weak_local_player(at).is_none());
        assert!(weak_context(at).is_none());
        control[8..12].copy_from_slice(&1u32.to_le_bytes());
        reference[16..20].copy_from_slice(&0x80007u32.to_le_bytes());
        std::hint::black_box(&reference);
        assert!(weak_local_player(at).is_none());
        reference[16..20].copy_from_slice(&0x40007u32.to_le_bytes());
        point(&mut owner, 0x150, 3);
        std::hint::black_box(&owner);
        assert!(weak_local_player(at).is_none());
        assert!(registry_key(actor).is_none());
        point(&mut owner, 0x150, 4);
        point(
            &mut fixture.actor,
            0x10,
            fixture._registry.as_ptr() as usize,
        );
        assert!(weak_local_player(at).is_none());
        point(&mut fixture.actor, 0x10, owner.as_ptr() as usize + 0x30);
        point(&mut fixture._payload[1], 24, 0);
        assert!(weak_local_player(at).is_none());
        point(&mut fixture._payload[1], 24, actor);
        fixture._nodes[8..12].copy_from_slice(&hash("DifferentTag").to_le_bytes());
        assert!(weak_local_player(at).is_none());
        assert_eq!(weak_context(at), Some(context));
    }
    #[test]
    fn traverses_hash_collision_and_sparse_generation_without_calling_engine() {
        let kind = hash("ActorTypeComponent");
        assert_eq!(kind, 0x4f6ba419);
        assert_eq!(hash("UserEntityIdentifierComponent"), 0x0b845379);
        let mut actor = vec![0u8; 32];
        let mut registry = vec![0u8; 0x80];
        let mut buckets = vec![0xffu8; 64];
        let mut nodes = vec![0u8; 64];
        let mut storage = vec![0u8; 0x68];
        let mut sparse = [0usize; 1];
        let mut sparse_page = vec![0xffu8; 2048 * 4];
        let mut pages = [0usize; 1];
        let mut payload = vec![0u8; 128 * 4];
        point(&mut actor, 0x10, registry.as_ptr() as usize);
        actor[0x18..0x1c].copy_from_slice(&0x40007u32.to_le_bytes());
        point(&mut registry, 0x48, buckets.as_ptr() as usize);
        point(
            &mut registry,
            0x50,
            buckets.as_ptr() as usize + buckets.len(),
        );
        point(&mut registry, 0x68, nodes.as_ptr() as usize);
        point(&mut registry, 0x70, nodes.as_ptr() as usize + nodes.len());
        point(&mut buckets, (kind as usize & 7) * 8, 0);
        point(&mut nodes, 0, 1);
        nodes[8..12].copy_from_slice(&0x11111111u32.to_le_bytes());
        point(&mut nodes, 32, usize::MAX);
        nodes[40..44].copy_from_slice(&kind.to_le_bytes());
        point(&mut nodes, 48, storage.as_ptr() as usize);
        sparse[0] = sparse_page.as_ptr() as usize;
        point(&mut storage, 8, sparse.as_ptr() as usize);
        point(&mut storage, 16, sparse.as_ptr() as usize + 8);
        sparse_page[7 * 4..8 * 4].copy_from_slice(&0x40003u32.to_le_bytes());
        pages[0] = payload.as_ptr() as usize;
        point(&mut storage, 0x50, pages.as_ptr() as usize);
        payload[12..16].copy_from_slice(&319u32.to_le_bytes());
        let address = actor.as_ptr() as usize;
        assert_eq!(actor_type(address), Some(319));
        assert_eq!(
            component(address, kind, 4).unwrap(),
            Some(payload.as_ptr() as usize + 12)
        );
        assert!(!contains(address, hash("OnGroundFlagComponent")).unwrap());
        actor[0x18..0x1c].copy_from_slice(&0x80007u32.to_le_bytes());
        assert!(actor_type(address).is_none());
        actor[0x18..0x1c].copy_from_slice(&0x40007u32.to_le_bytes());
        point(&mut nodes, 0, 0);
        assert!(component(address, hash("SomethingElse"), 4).is_ok());
        nodes[40..44].copy_from_slice(&0u32.to_le_bytes());
        assert!(component(address, kind, 4).is_err());
    }
}
