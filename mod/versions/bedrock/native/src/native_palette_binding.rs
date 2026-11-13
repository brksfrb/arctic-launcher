//! Scoped, read-only palette indices from the pinned DataDrivenGeometry layout.
//! Native reads stay inside a dataDrivenDraw call. Copied part identities may
//! remain in its enclosing ActorScope, but never enter persistent history.
use crate::world_hooks::read;
use sha2::{Digest, Sha256};
use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    sync::Arc,
};

const PART_STRIDE: usize = 0x240;
const MAX_PARTS: usize = 256;
const MAX_GROUPS: usize = 64;
const MAX_OWNERS: usize = 64;
const MAX_ENTRIES: usize = 512;
const MAX_GUARDS: usize = 2048;
const MAX_GUARD_BYTES: usize = 128 * 1024;

#[derive(Clone, PartialEq)]
pub(crate) struct RigAnchor {
    pub part: u8,
    pub bone_index: u32,
    pub depth: u16,
}
#[derive(Clone)]
pub(crate) enum Seed {
    Inherit,
    Bound(RigAnchor),
    Canonical(RigAnchor),
    /// An explicit native binding with an unproved source must not inherit a
    /// different rig role from the geometry's local part parent.
    Blocked,
}
#[derive(Clone, PartialEq)]
pub(crate) struct Bone {
    pub name: String,
    pub index: u32,
    pub owner: u32,
    pub matrix: [f32; 16],
    pub anchor: Option<RigAnchor>,
    pub part: usize,
    pub cache: Arc<crate::native_emote_history::Slot>,
}
struct Guarded {
    address: usize,
    bytes: Vec<u8>,
}
struct Copies {
    guards: Vec<Guarded>,
    bytes: usize,
}
impl Copies {
    fn read(&mut self, address: usize, size: usize) -> Option<Vec<u8>> {
        if address < 0x10000
            || address.checked_add(size).is_none()
            || self.guards.len() >= MAX_GUARDS
            || self.bytes.checked_add(size)? > MAX_GUARD_BYTES
        {
            return None;
        }
        let mut bytes = vec![0; size];
        read(address, &mut bytes).ok()?;
        self.bytes += size;
        self.guards.push(Guarded {
            address,
            bytes: bytes.clone(),
        });
        Some(bytes)
    }
    fn unchanged(&self) -> Option<()> {
        for guard in &self.guards {
            let mut again = vec![0; guard.bytes.len()];
            read(guard.address, &mut again).ok()?;
            if again != guard.bytes {
                return None;
            }
        }
        Some(())
    }
    fn vector(&mut self, address: usize, stride: usize, limit: usize) -> Option<Vector> {
        let bytes = self.read(address, 24)?;
        let begin = word(&bytes, 0)?;
        let end = word(&bytes, 8)?;
        let capacity = word(&bytes, 16)?;
        let length = end.checked_sub(begin)?;
        if !length.is_multiple_of(stride)
            || capacity < end
            || capacity.checked_sub(begin)? > stride.checked_mul(4096)?
            || length / stride > limit
            || length != 0 && begin < 0x10000
        {
            return None;
        }
        Some(Vector {
            begin,
            count: length / stride,
        })
    }
}
#[derive(Clone, Copy)]
struct Vector {
    begin: usize,
    count: usize,
}
fn word(bytes: &[u8], offset: usize) -> Option<usize> {
    usize::try_from(u64::from_le_bytes(
        bytes.get(offset..offset.checked_add(8)?)?.try_into().ok()?,
    ))
    .ok()
}
fn integer(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset.checked_add(4)?)?.try_into().ok()?,
    ))
}
struct Part {
    group: u32,
    palette: u32,
    owner: u32,
    bone: u32,
    parent: Option<u32>,
    anchor: Option<RigAnchor>,
    cache: Arc<crate::native_emote_history::Slot>,
}
fn cache_slot(
    copies: &mut Copies,
    part: usize,
    group: u32,
    palette: u32,
) -> Option<Arc<crate::native_emote_history::Slot>> {
    // 0x1c21253 compares part+0's hash and part+8's MSVC string with
    // Actor+0x118's tree key. Preserve both, not a guessed bone-name alias.
    let key = copies.read(part, 40)?;
    let hash = u64::from_le_bytes(key[..8].try_into().ok()?);
    let string = &key[8..40];
    let length = word(string, 16)?;
    let capacity = word(string, 24)?;
    if length > 128 || capacity < length || capacity > 4096 {
        return None;
    }
    let bytes = if capacity < 16 {
        string.get(..length.checked_add(1)?)?.to_vec()
    } else {
        copies.read(word(string, 0)?, length.checked_add(1)?)?
    };
    if bytes.last() != Some(&0) {
        return None;
    }
    let name = std::str::from_utf8(&bytes[..length]).ok()?.to_owned();
    Some(Arc::new(crate::native_emote_history::Slot {
        hash,
        name,
        group,
        palette,
    }))
}
fn part_index(parts: Vector, pointer: usize) -> Option<usize> {
    let offset = pointer.checked_sub(parts.begin)?;
    if !offset.is_multiple_of(PART_STRIDE) || offset / PART_STRIDE >= parts.count {
        return None;
    }
    Some(offset / PART_STRIDE)
}
fn rig_part(name: &str) -> Option<u8> {
    ["head", "body", "rightarm", "leftarm", "rightleg", "leftleg"]
        .iter()
        .position(|part| *part == name)
        .and_then(|i| u8::try_from(i).ok())
}
/// Native update/render traversal follows part+0x188's child pointers, starting
/// from geometry+0x50's root indices. Use those real edges, not BoneOrientation's
/// unresolved parent defaults or cosmetic-name aliases such as "jacket".
fn topology(
    copies: &mut Copies,
    geometry: usize,
    parts: Vector,
    hierarchy: &crate::animation_pose::Hierarchy,
    seeds: Option<&[Seed]>,
) -> Option<Vec<Part>> {
    if seeds.is_some_and(|seeds| seeds.len() != hierarchy.bones.len()) {
        return None;
    }
    let mut indexed = Vec::with_capacity(parts.count);
    let mut parents = vec![None; parts.count];
    let mut edges = 0usize;
    for parent in 0..parts.count {
        let address = parts.begin.checked_add(parent.checked_mul(PART_STRIDE)?)?;
        let indices = copies.read(address.checked_add(0x84)?, 16)?;
        let bone = integer(&indices, 12)?;
        hierarchy.bones.get(usize::try_from(bone).ok()?)?;
        let group = integer(&indices, 0)?;
        let palette = integer(&indices, 4)?;
        indexed.push(Part {
            group,
            palette,
            owner: integer(&indices, 8)?,
            bone,
            parent: None,
            anchor: None,
            cache: cache_slot(copies, address, group, palette)?,
        });
        let children = copies.vector(address.checked_add(0x188)?, 8, MAX_PARTS)?;
        if children.count == 0 {
            continue;
        }
        let pointers = copies.read(children.begin, children.count.checked_mul(8)?)?;
        for child in 0..children.count {
            let pointer = word(&pointers, child.checked_mul(8)?)?;
            // Both inspected native recursive traversals skip null children.
            if pointer == 0 {
                continue;
            }
            let child = part_index(parts, pointer)?;
            if child == parent || parents[child].replace(parent).is_some() {
                return None;
            }
            edges = edges.checked_add(1)?;
            if edges >= parts.count {
                return None;
            }
        }
    }
    let roots = copies.vector(geometry.checked_add(0x50)?, 8, MAX_PARTS)?;
    if roots.count == 0 || roots.count > parts.count {
        return None;
    }
    let bytes = copies.read(roots.begin, roots.count.checked_mul(8)?)?;
    let mut root_indices = HashSet::new();
    for i in 0..roots.count {
        let root = word(&bytes, i.checked_mul(8)?)?;
        if root >= parts.count || parents[root].is_some() || !root_indices.insert(root) {
            return None;
        }
    }
    if edges.checked_add(root_indices.len())? != parts.count {
        return None;
    }
    let mut states = vec![0u8; parts.count];
    let mut blocked = vec![false; parts.count];
    let mut chain = Vec::with_capacity(parts.count);
    for part in 0..parts.count {
        if states[part] == 2 {
            continue;
        }
        let mut at = part;
        loop {
            match states[at] {
                2 => break,
                1 => return None,
                _ => {
                    states[at] = 1;
                    chain.push(at);
                }
            }
            match parents[at] {
                Some(parent) => {
                    at = parent;
                }
                None => {
                    if !root_indices.contains(&at) {
                        return None;
                    }
                    break;
                }
            }
        }
        // Resolve each node once, parent before child. None is a valid resolved
        // result for an unanchored root; states distinguishes it from unknown.
        for at in chain.drain(..).rev() {
            indexed[at].parent = parents[at].map(|parent| parent as u32);
            let bone_index = indexed[at].bone;
            let bone = hierarchy.bones.get(usize::try_from(bone_index).ok()?)?;
            let parent_blocked = parents[at].is_some_and(|parent| blocked[parent]);
            let (direct, inherit, barrier) = if let Some(seeds) = seeds {
                match seeds.get(usize::try_from(bone_index).ok()?)? {
                    Seed::Bound(anchor) => (Some(anchor.clone()), false, false),
                    Seed::Canonical(anchor) if !parent_blocked => {
                        (Some(anchor.clone()), false, false)
                    }
                    Seed::Inherit if !parent_blocked => (None, true, false),
                    _ => (None, false, true),
                }
            } else if let Some(part) = rig_part(&bone.name) {
                (
                    Some(RigAnchor {
                        part,
                        bone_index,
                        depth: 0,
                    }),
                    false,
                    false,
                )
            } else if matches!(bone.binding_method, 2 | 3) {
                // A helper's explicit native source takes priority over the
                // local part tree. This root-only reader has no such receipt.
                (None, false, true)
            } else {
                (None, true, false)
            };
            let anchor = if direct.is_some() {
                direct
            } else if !inherit {
                None
            } else if let Some(parent) = parents[at] {
                match indexed[parent].anchor.as_ref() {
                    Some(anchor) => Some(RigAnchor {
                        part: anchor.part,
                        bone_index: anchor.bone_index,
                        depth: anchor.depth.checked_add(1)?,
                    }),
                    None => None,
                }
            } else {
                None
            };
            indexed[at].anchor = anchor;
            blocked[at] = barrier;
            states[at] = 2;
        }
    }
    Some(indexed)
}
/// Root rig roles for every actual native part, including helper bones with no
/// mesh/palette owner. Only heap topology metadata is borrowed for this draw.
pub(crate) struct Rig {
    copies: Copies,
    anchors: Vec<Option<RigAnchor>>,
}
impl Rig {
    pub(crate) fn copy(
        geometry: usize,
        hierarchy: &crate::animation_pose::Hierarchy,
    ) -> Option<Self> {
        if hierarchy.bones.is_empty() || hierarchy.bones.len() > MAX_PARTS {
            return None;
        }
        let mut copies = Copies {
            guards: Vec::new(),
            bytes: 0,
        };
        if integer(&copies.read(geometry.checked_add(0xe8)?, 4)?, 0)? != hierarchy.index {
            return None;
        }
        let parts = copies.vector(geometry.checked_add(0x38)?, PART_STRIDE, MAX_PARTS)?;
        if parts.count == 0 {
            return None;
        }
        let indexed = topology(&mut copies, geometry, parts, hierarchy, None)?;
        let mut anchors = vec![None; hierarchy.bones.len()];
        let mut seen = HashSet::new();
        for part in indexed {
            let index = usize::try_from(part.bone).ok()?;
            if !seen.insert(index) && anchors[index] != part.anchor {
                return None;
            }
            anchors[index] = part.anchor;
        }
        copies.unchanged()?;
        Some(Self { copies, anchors })
    }
    pub(crate) fn anchor(&self, bone: usize) -> Option<RigAnchor> {
        self.anchors.get(bone)?.clone()
    }
    pub(crate) fn valid(&self) -> bool {
        self.copies.unchanged().is_some()
    }
}
struct Owner {
    current: Vector,
    previous: Vector,
    bones: Vec<Bone>,
}
pub(crate) struct Binding {
    frame: u64,
    hierarchy: u32,
    geometry: usize,
    definition: String,
    copies: Copies,
    owners: Vec<Owner>,
    attachment: Option<Arc<crate::native_attachment_binding::Binding>>,
    cache_owner: Option<crate::native_cache_binding::Binding>,
}
impl Binding {
    pub(crate) fn with_cache_owner(mut self, params: usize) -> Self {
        self.cache_owner = crate::native_transform::palette_writer_scope()
            .and_then(|stamp| {
                crate::native_cache_binding::Binding::copy(&stamp, params, self.geometry)
            })
            .filter(|owner| owner.matches(self.frame, self.hierarchy, self.geometry));
        self
    }
    pub(crate) fn with_attachment(
        mut self,
        attachment: Arc<crate::native_attachment_binding::Binding>,
    ) -> Option<Self> {
        if self.attachment.is_some()
            || !attachment.matches(self.frame, self.hierarchy)
            || !attachment.valid()
        {
            return None;
        }
        self.attachment = Some(attachment);
        Some(self)
    }
    fn unchanged(&self) -> Option<()> {
        self.copies.unchanged()?;
        if self
            .cache_owner
            .as_ref()
            .is_some_and(|owner| !owner.valid())
        {
            return None;
        }
        if let Some(attachment) = &self.attachment {
            if !attachment.valid() {
                return None;
            }
            self.copies.unchanged()?;
        }
        Some(())
    }
}
/// 0x1c36670 selects a 0x240-byte part; 0x1c21d4c uses part+0x90
/// as the index into the hierarchy returned by the geometry+0xe8 lookup.
/// 0x12ed29a/0x12ed2a0 associate the same part with palette/owner indices.
pub(crate) fn copy(
    geometry: usize,
    hierarchy: &crate::animation_pose::Hierarchy,
    frame: u64,
) -> Option<Binding> {
    copy_with_seeds(geometry, hierarchy, frame, None)
}
pub(crate) fn copy_anchored(
    geometry: usize,
    hierarchy: &crate::animation_pose::Hierarchy,
    frame: u64,
    anchors: &[Seed],
) -> Option<Binding> {
    copy_with_seeds(geometry, hierarchy, frame, Some(anchors))
}
fn copy_with_seeds(
    geometry: usize,
    hierarchy: &crate::animation_pose::Hierarchy,
    frame: u64,
    seeds: Option<&[Seed]>,
) -> Option<Binding> {
    if !crate::native_body::enabled()
        || hierarchy.bones.is_empty()
        || hierarchy.bones.len() > MAX_PARTS
    {
        return None;
    }
    let mut copies = Copies {
        guards: Vec::new(),
        bytes: 0,
    };
    let index = copies.read(geometry.checked_add(0xe8)?, 4)?;
    if integer(&index, 0)? != hierarchy.index {
        return None;
    }
    let parts = copies.vector(geometry.checked_add(0x38)?, PART_STRIDE, MAX_PARTS)?;
    let groups = copies.vector(geometry.checked_add(0x88)?, 0x60, MAX_GROUPS)?;
    if parts.count == 0 || groups.count == 0 {
        return None;
    }
    let indexed = topology(&mut copies, geometry, parts, hierarchy, seeds)?;
    let mut owners = Vec::new();
    let mut addresses = HashSet::new();
    let mut regions: Vec<(usize, usize)> = Vec::new();
    let mut entries = 0usize;
    for group_index in 0..groups.count {
        let group = groups.begin.checked_add(group_index.checked_mul(0x60)?)?;
        let instances = copies.vector(group.checked_add(0x48)?, 16, MAX_OWNERS)?;
        for owner_index in 0..instances.count {
            if owners.len() >= MAX_OWNERS {
                return None;
            }
            let entry = instances.begin.checked_add(owner_index.checked_mul(16)?)?;
            let owner = word(&copies.read(entry, 8)?, 0)?;
            if !addresses.insert(owner) {
                return None;
            }
            let members = copies.vector(owner.checked_add(8)?, 8, MAX_PARTS)?;
            let current = copies.vector(
                owner.checked_add(crate::native_scene_layout::palette_owner::CURRENT)?,
                64,
                MAX_PARTS,
            )?;
            let previous = copies.vector(
                owner.checked_add(crate::native_scene_layout::palette_owner::PREVIOUS)?,
                64,
                MAX_PARTS,
            )?;
            if members.count == 0
                || current.count != members.count
                || previous.count != members.count
            {
                return None;
            }
            // These are owning native vectors. A previous-pose buffer must
            // never alias an attested current buffer, including another owner.
            for vector in [current, previous] {
                let end = vector.begin.checked_add(vector.count.checked_mul(64)?)?;
                if regions
                    .iter()
                    .any(|(begin, old_end)| vector.begin < *old_end && *begin < end)
                {
                    return None;
                }
                regions.push((vector.begin, end));
            }
            entries = entries.checked_add(members.count)?;
            if entries > MAX_ENTRIES {
                return None;
            }
            let pointers = copies.read(members.begin, members.count.checked_mul(8)?)?;
            let mut bones = Vec::with_capacity(members.count);
            let mut seen = HashSet::new();
            for palette_index in 0..members.count {
                let part_address = word(&pointers, palette_index.checked_mul(8)?)?;
                let part_index = part_index(parts, part_address)?;
                if !seen.insert(part_address) {
                    return None;
                }
                let part = &indexed[part_index];
                if usize::try_from(part.group).ok()? != group_index
                    || usize::try_from(part.palette).ok()? != palette_index
                    || usize::try_from(part.owner).ok()? != owner_index
                {
                    return None;
                }
                let index = part.bone;
                let bone = hierarchy.bones.get(usize::try_from(index).ok()?)?;
                if bone.name.is_empty() || bone.name.len() > 64 {
                    return None;
                }
                bones.push(Bone {
                    name: bone.name.clone(),
                    index,
                    owner: part.owner,
                    matrix: bone.matrix,
                    anchor: part.anchor.clone(),
                    part: part_address,
                    cache: part.cache.clone(),
                });
            }
            owners.push(Owner {
                current,
                previous,
                bones,
            });
        }
    }
    if owners.is_empty() {
        return None;
    }
    copies.unchanged()?;
    // A deterministic skeleton/layout key, independent of heap addresses and
    // current animation. It is not a claim of full mesh/material identity.
    let mut digest = Sha256::new();
    digest.update(b"arctic-bedrock-palette-v1");
    digest.update(hierarchy.index.to_le_bytes());
    digest.update((hierarchy.bones.len() as u32).to_le_bytes());
    for bone in &hierarchy.bones {
        digest.update((bone.name.len() as u32).to_le_bytes());
        digest.update(bone.name.as_bytes());
        digest.update(bone.name_hash.to_le_bytes());
        digest.update([bone.binding_method]);
        if bone.binding_method == 2 {
            digest.update(bone.parent_hierarchy.to_le_bytes());
            digest.update(bone.parent_bone.to_le_bytes());
        }
        for value in bone.default_transform.iter().chain(&bone.pivot) {
            if !value.is_finite() {
                return None;
            }
            digest.update(value.to_bits().to_le_bytes());
        }
    }
    for part in &indexed {
        for value in [
            part.group,
            part.palette,
            part.owner,
            part.bone,
            part.parent.unwrap_or(u32::MAX),
        ] {
            digest.update(value.to_le_bytes());
        }
        digest.update(part.cache.hash.to_le_bytes());
        digest.update((part.cache.name.len() as u32).to_le_bytes());
        digest.update(part.cache.name.as_bytes());
    }
    let definition = format!("{:x}", digest.finalize());
    Some(Binding {
        frame,
        hierarchy: hierarchy.index,
        geometry,
        definition,
        copies,
        owners,
        attachment: None,
        cache_owner: None,
    })
}

struct Scope {
    binding: Binding,
    scope: u64,
    actor: String,
    geometry: String,
    attested: HashSet<[usize; 2]>,
}
thread_local! {
    static ACTIVE: RefCell<Vec<Option<Scope>>> = const { RefCell::new(Vec::new()) };
    static SKIPPED: Cell<usize> = const { Cell::new(0) };
}
pub(crate) struct Guard(bool);
impl Drop for Guard {
    fn drop(&mut self) {
        if self.0 {
            ACTIVE.with(|active| {
                active.borrow_mut().pop();
            });
        } else {
            SKIPPED.with(|skipped| skipped.set(skipped.get().saturating_sub(1)));
        }
    }
}
pub(crate) fn begin(binding: Option<Binding>, tag: Option<&crate::native_submit::Tag>) -> Guard {
    let value = binding.zip(tag).map(|(binding, tag)| Scope {
        binding,
        scope: tag.scope,
        actor: tag.actor.clone(),
        geometry: tag.geometry.clone(),
        attested: HashSet::new(),
    });
    let pushed = ACTIVE.with(|active| {
        let mut active = active.borrow_mut();
        if active.len() >= 8 {
            return false;
        }
        active.push(value);
        true
    });
    if !pushed {
        SKIPPED.with(|skipped| skipped.set(skipped.get().saturating_add(1)));
    }
    Guard(pushed)
}
#[derive(Clone, PartialEq)]
pub(crate) struct Named {
    pub hierarchy: u32,
    pub definition: String,
    pub attachment_slot: Option<u32>,
    pub native_cape: Option<String>,
    pub bones: Vec<Bone>,
    pub space: Option<crate::native_emote_space::Space>,
    pub cache_owner: Option<crate::native_cache_binding::Identity>,
}
/// Root cape/wing layers use archive roles 7/8. These are not native child
/// indices or inferred equipment slots. A real child binding retains its space.
fn archived_layer(
    active: &Scope,
    tag: &crate::native_submit::Tag,
    frame: u64,
) -> Option<(u32, Option<String>, crate::native_emote_space::Space)> {
    if let Some(attachment) = active.binding.attachment.as_ref() {
        if let crate::native_submit::MeshRole::Elytra { animation_frame } = &tag.role {
            if tag.replacement.is_some()
                || *animation_frame != frame
                || tag.geometry != "geometry.elytra"
            {
                return None;
            }
            return Some((
                crate::native_attachment_samples::WING_SLOT,
                None,
                attachment.pose_space.clone(),
            ));
        }
        return Some((attachment.slot(), None, attachment.pose_space.clone()));
    }
    if tag.replacement.is_some() {
        return None;
    }
    let (animation_frame, slot, cape) = match &tag.role {
        crate::native_submit::MeshRole::Cape {
            animation_frame,
            native_cape: Some(cape),
        } => (
            animation_frame,
            crate::native_attachment_samples::CAPE_SLOT,
            Some(cape.clone()),
        ),
        crate::native_submit::MeshRole::Elytra { animation_frame }
            if tag.geometry == "geometry.elytra" =>
        {
            (
                animation_frame,
                crate::native_attachment_samples::WING_SLOT,
                None,
            )
        }
        _ => return None,
    };
    if *animation_frame != frame {
        return None;
    }
    let draw = crate::native_transform::current_draw(tag.scope, &tag.actor)?;
    let space = crate::native_emote_space::Space::between(&draw.matrix, &draw.matrix)?;
    Some((slot, cape, space))
}
/// Expected native model matrix for a proved child draw. This is independent
/// of constructor receipts so the constructor can attest its own copied root.
pub(crate) fn draw_matrix(tag: &crate::native_submit::Tag, frame: u64) -> Option<[f32; 16]> {
    if SKIPPED.with(Cell::get) != 0 {
        return None;
    }
    ACTIVE
        .try_with(|active| {
            let active = active.try_borrow().ok()?;
            let active = active.last()?.as_ref()?;
            if active.scope != tag.scope
                || active.actor != tag.actor
                || active.geometry != tag.geometry
                || active.binding.frame != frame
                || crate::world_hooks::current_pass() != Some(tag.scope)
            {
                return None;
            }
            active.binding.unchanged()?;
            if let Some(attachment) = active.binding.attachment.as_ref() {
                Some(attachment.draw_matrix())
            } else {
                archived_layer(active, tag, frame)?;
                Some(crate::native_transform::current_draw(tag.scope, &tag.actor)?.matrix)
            }
        })
        .ok()
        .flatten()
}
#[derive(PartialEq)]
pub(crate) struct Packed {
    pub named: Named,
    pub previous: Option<Vec<crate::native_palette_writer::Prior>>,
    pub previous_fallback: bool,
    // Native source regions stay local to the synchronous copy callback.
    pub regions: [[usize; 2]; 2],
}
/// Identify a previous source without requiring its historical pose to be
/// available. Missing history must not prevent recording this frame's current
/// upload and leave the cache permanently unable to recover on the next frame.
pub(crate) fn span_is_previous(
    source: usize,
    tag: &crate::native_submit::Tag,
    frame: u64,
) -> Option<bool> {
    if SKIPPED.with(Cell::get) != 0 {
        return None;
    }
    ACTIVE
        .try_with(|active| {
            let active = active.try_borrow().ok()?;
            let active = active.last()?.as_ref()?;
            if active.scope != tag.scope
                || active.actor != tag.actor
                || active.geometry != tag.geometry
                || active.binding.frame != frame
                || crate::world_hooks::current_pass() != Some(tag.scope)
            {
                return None;
            }
            let mut span = [0; 16];
            read(source, &mut span).ok()?;
            let key = [word(&span, 0)?, word(&span, 8)?];
            let mut matches = active.binding.owners.iter().filter_map(|owner| {
                if !active
                    .attested
                    .contains(&[owner.current.count, owner.current.begin])
                {
                    return None;
                }
                if key == [owner.current.count, owner.current.begin] {
                    Some(false)
                } else if key == [owner.previous.count, owner.previous.begin] {
                    Some(true)
                } else {
                    None
                }
            });
            let previous = matches.next()?;
            if matches.next().is_some() {
                return None;
            }
            active.binding.unchanged()?;
            let mut again = [0; 16];
            read(source, &mut again).ok()?;
            (span == again).then_some(previous)
        })
        .ok()
        .flatten()
}
/// Match both original constructor spans to exactly one owner. The return
/// value contains owned keys/matrices plus scalar part identities. Those identities
/// stay in this draw or its enclosing ActorScope and are never dereferenced.
pub(crate) fn current(
    sources: [usize; 2],
    tag: &crate::native_submit::Tag,
    frame: u64,
) -> Option<Named> {
    if SKIPPED.with(Cell::get) != 0 {
        return None;
    }
    ACTIVE
        .try_with(|active| {
            let active = active.try_borrow().ok()?;
            let active = active.last()?.as_ref()?;
            if active.scope != tag.scope
                || active.actor != tag.actor
                || active.geometry != tag.geometry
                || active.binding.frame != frame
                || crate::world_hooks::current_pass() != Some(tag.scope)
            {
                return None;
            }
            let mut spans = [[0; 16]; 2];
            for i in 0..2 {
                read(sources[i], &mut spans[i]).ok()?;
            }
            let matches = |span: &[u8], vector: Vector| {
                word(span, 0) == Some(vector.count) && word(span, 8) == Some(vector.begin)
            };
            let mut owners = active.binding.owners.iter().filter(|owner| {
                matches(&spans[0], owner.current) && matches(&spans[1], owner.previous)
            });
            let owner = owners.next()?;
            if owners.next().is_some() {
                return None;
            }
            active.binding.unchanged()?;
            for i in 0..2 {
                let mut again = [0; 16];
                read(sources[i], &mut again).ok()?;
                if again != spans[i] {
                    return None;
                }
            }
            let layer = archived_layer(active, tag, frame);
            Some(Named {
                hierarchy: active.binding.hierarchy,
                definition: active.binding.definition.clone(),
                attachment_slot: layer.as_ref().map(|layer| layer.0),
                native_cape: layer.as_ref().and_then(|layer| layer.1.clone()),
                bones: owner.bones.clone(),
                cache_owner: active
                    .binding
                    .cache_owner
                    .as_ref()
                    .map(|owner| owner.identity()),
                space: layer.map(|layer| layer.2),
            })
        })
        .ok()
        .flatten()
}
/// A later description must not inherit an earlier description's root/tint receipt.
pub(crate) fn reset_receipts(tag: &crate::native_submit::Tag) {
    let _ = ACTIVE.try_with(|active| {
        let Ok(mut active) = active.try_borrow_mut() else {
            return;
        };
        let Some(active) = active.last_mut().and_then(Option::as_mut) else {
            return;
        };
        if active.scope == tag.scope && active.actor == tag.actor && active.geometry == tag.geometry
        {
            active.attested.clear();
        }
    });
}
/// Called only after the description observer checked its native root and tint.
/// The receipt is bounded by the owners in this draw scope, and disappears with it.
pub(crate) fn attest(sources: [usize; 2], tag: &crate::native_submit::Tag, frame: u64) -> bool {
    if current(sources, tag, frame).is_none() {
        return false;
    }
    ACTIVE
        .try_with(|active| {
            let mut active = active.try_borrow_mut().ok()?;
            let active = active.last_mut()?.as_mut()?;
            if active.scope != tag.scope
                || active.actor != tag.actor
                || active.geometry != tag.geometry
                || active.binding.frame != frame
                || crate::world_hooks::current_pass() != Some(tag.scope)
            {
                return None;
            }
            let mut current = [0; 16];
            let mut previous = [0; 16];
            read(sources[0], &mut current).ok()?;
            read(sources[1], &mut previous).ok()?;
            let key = [word(&current, 0)?, word(&current, 8)?];
            let matches = |owner: &&Owner| {
                key == [owner.current.count, owner.current.begin]
                    && word(&previous, 0) == Some(owner.previous.count)
                    && word(&previous, 8) == Some(owner.previous.begin)
            };
            let mut owners = active.binding.owners.iter().filter(matches);
            owners.next()?;
            if owners.next().is_some()
                || active.attested.len() >= MAX_OWNERS && !active.attested.contains(&key)
            {
                return None;
            }
            active.binding.unchanged()?;
            active.attested.insert(key);
            Some(())
        })
        .ok()
        .flatten()
        .is_some()
}
/// Check the constructor's copied spans even when no emote candidate is sampled.
/// A matching root/tint alone must not attest a different palette description.
pub(crate) fn attest_description(
    description: usize,
    sources: [usize; 2],
    tag: &crate::native_submit::Tag,
    frame: u64,
) -> bool {
    let copied = (|| {
        let mut spans = [[0; 16]; 2];
        for i in 0..2 {
            read(sources[i], &mut spans[i]).ok()?;
            let mut output = [0; 16];
            read(
                description
                    .checked_add(crate::native_scene_layout::skinned_description::BONE_SPANS[i])?,
                &mut output,
            )
            .ok()?;
            if output != spans[i] {
                return None;
            }
        }
        if !attest(sources, tag, frame) {
            return None;
        }
        for i in 0..2 {
            let mut source = [0; 16];
            let mut output = [0; 16];
            read(sources[i], &mut source).ok()?;
            read(
                description
                    .checked_add(crate::native_scene_layout::skinned_description::BONE_SPANS[i])?,
                &mut output,
            )
            .ok()?;
            if source != spans[i] || output != spans[i] {
                return None;
            }
        }
        Some(())
    })();
    if copied.is_none() {
        reset_receipts(tag);
    }
    copied.is_some()
}
/// The eight-matrix copy helper receives just one span. Authorize that span only
/// if the matching description was attested earlier in this same native draw.
/// Every previous entry needs a fresh observed writer receipt identifying its
/// current fallback or captured historical pose. Equal matrices are insufficient.
pub(crate) fn packed(source: usize, tag: &crate::native_submit::Tag, frame: u64) -> Option<Packed> {
    if SKIPPED.with(Cell::get) != 0 {
        return None;
    }
    ACTIVE
        .try_with(|active| {
            let active = active.try_borrow().ok()?;
            let active = active.last()?.as_ref()?;
            if active.scope != tag.scope
                || active.actor != tag.actor
                || active.geometry != tag.geometry
                || active.binding.frame != frame
                || crate::world_hooks::current_pass() != Some(tag.scope)
            {
                return None;
            }
            let mut span = [0; 16];
            read(source, &mut span).ok()?;
            let key = [word(&span, 0)?, word(&span, 8)?];
            let mut owners = active.binding.owners.iter().filter_map(|owner| {
                if key == [owner.current.count, owner.current.begin] {
                    Some((owner, false))
                } else if key == [owner.previous.count, owner.previous.begin] {
                    Some((owner, true))
                } else {
                    None
                }
            });
            let (owner, is_previous) = owners.next()?;
            if owners.next().is_some() {
                return None;
            }
            if !active
                .attested
                .contains(&[owner.current.count, owner.current.begin])
            {
                return None;
            }
            let previous = if is_previous {
                Some(crate::native_transform::previous_palette(
                    tag,
                    frame,
                    owner.current.begin,
                    owner.previous.begin,
                    owner.current.count,
                )?)
            } else {
                None
            };
            let previous_fallback = previous.as_ref().is_some_and(|entries| {
                entries
                    .iter()
                    .all(|entry| matches!(entry, crate::native_palette_writer::Prior::Current))
            });
            active.binding.unchanged()?;
            let mut again = [0; 16];
            read(source, &mut again).ok()?;
            if span != again {
                return None;
            }
            let layer = archived_layer(active, tag, frame);
            Some(Packed {
                named: Named {
                    hierarchy: active.binding.hierarchy,
                    definition: active.binding.definition.clone(),
                    attachment_slot: layer.as_ref().map(|layer| layer.0),
                    native_cape: layer.as_ref().and_then(|layer| layer.1.clone()),
                    bones: owner.bones.clone(),
                    cache_owner: active
                        .binding
                        .cache_owner
                        .as_ref()
                        .map(|owner| owner.identity()),
                    space: layer.map(|layer| layer.2),
                },
                previous,
                previous_fallback,
                regions: [
                    [owner.current.begin, owner.current.count.checked_mul(64)?],
                    [owner.previous.begin, owner.previous.count.checked_mul(64)?],
                ],
            })
        })
        .ok()
        .flatten()
}
