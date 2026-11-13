//! Checked child animation components for native equipment draws. These heap
//! metadata borrows last only for the current dataDrivenDraw. No slot address,
//! RenderParams pointer or native model enters a replay or persistent history.
use crate::native_palette_binding::{RigAnchor, Seed};
use crate::native_scene_layout::{
    attachment as layout, hierarchy_query as query_layout, model_body, skin_selection as selection,
};
use crate::world_hooks::read;
use std::collections::HashSet;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};

#[path = "native_attachment_profile.rs"]
mod profile;
#[path = "native_hierarchy_query_profile.rs"]
mod query_profile;

static COPIES: AtomicU64 = AtomicU64::new(0);
static LAST: Mutex<Option<serde_json::Value>> = Mutex::new(None);
struct Guard {
    address: usize,
    bytes: Vec<u8>,
}
pub(crate) struct Copies {
    guards: Vec<Guard>,
    bytes: usize,
    refs: Vec<usize>,
}
impl Copies {
    pub(crate) fn new() -> Self {
        Self {
            guards: Vec::new(),
            bytes: 0,
            refs: Vec::new(),
        }
    }
    pub(crate) fn retained_bytes(&self) -> usize {
        self.guards.capacity() * std::mem::size_of::<Guard>()
            + self
                .guards
                .iter()
                .map(|guard| guard.bytes.capacity())
                .sum::<usize>()
            + self.refs.capacity() * std::mem::size_of::<usize>()
    }
    pub(crate) fn read(&mut self, address: usize, size: usize) -> Option<Vec<u8>> {
        if address < 0x10000
            || address.checked_add(size).is_none()
            || self.guards.len() >= 2048
            || self.bytes.checked_add(size)? > 128 * 1024
        {
            return None;
        }
        let mut bytes = vec![0; size];
        read(address, &mut bytes).ok()?;
        self.bytes += size;
        self.guards.push(Guard {
            address,
            bytes: bytes.clone(),
        });
        Some(bytes)
    }
    pub(crate) fn live(&mut self, control: usize) -> Option<()> {
        live(control)?;
        if self.refs.len() >= 64 {
            return None;
        }
        self.refs.push(control);
        Some(())
    }
    pub(crate) fn vector(
        &mut self,
        address: usize,
        stride: usize,
        limit: usize,
    ) -> Option<(usize, usize)> {
        let bytes = self.read(address, 24)?;
        let begin = word(&bytes, 0)?;
        let end = word(&bytes, 8)?;
        let capacity = word(&bytes, 16)?;
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
    pub(crate) fn unchanged(&self) -> Option<()> {
        for guard in &self.guards {
            let mut bytes = vec![0; guard.bytes.len()];
            read(guard.address, &mut bytes).ok()?;
            if bytes != guard.bytes {
                return None;
            }
        }
        for &control in &self.refs {
            live(control)?;
        }
        Some(())
    }
    pub(crate) fn hierarchies(
        &mut self,
        component: usize,
    ) -> Option<Vec<(usize, u32, usize, usize)>> {
        let header = self.read(component.checked_add(layout::HIERARCHY_MAP)?, 24)?;
        let head = word(&header, 8)?;
        let count = word(&header, 16)?;
        if count == 0 || count > 16 {
            return None;
        }
        let sentinel = self.read(head, 16)?;
        let mut node = word(&sentinel, 0)?;
        let mut previous = head;
        let mut seen = HashSet::new();
        let mut ids = HashSet::new();
        let mut result = Vec::with_capacity(count);
        for _ in 0..count {
            if node == head || !seen.insert(node) {
                return None;
            }
            let entry = self.read(node, 24)?;
            let index = u32::from_le_bytes(entry[16..20].try_into().ok()?);
            if word(&entry, 8)? != previous || !ids.insert(index) {
                return None;
            }
            let (begin, length) = self.vector(node.checked_add(24)?, layout::BONE_BYTES, 256)?;
            if length == 0 {
                return None;
            }
            result.push((node, index, begin, length));
            previous = node;
            node = word(&entry, 0)?;
        }
        if node != head || word(&sentinel, 8)? != previous {
            return None;
        }
        Some(result)
    }
}
fn word(bytes: &[u8], offset: usize) -> Option<usize> {
    usize::try_from(u64::from_le_bytes(
        bytes.get(offset..offset.checked_add(8)?)?.try_into().ok()?,
    ))
    .ok()
}
fn live(control: usize) -> Option<()> {
    (strong(control)? > 0).then_some(())
}
fn strong(control: usize) -> Option<i32> {
    if control < 0x10000 {
        return None;
    }
    let mut bytes = [0; 4];
    read(control.checked_add(8)?, &mut bytes).ok()?;
    let count = i32::from_le_bytes(bytes);
    (count >= 0).then_some(count)
}
struct ParentRig {
    hierarchy: crate::animation_pose::Hierarchy,
    rig: crate::native_palette_binding::Rig,
}
struct ParentLink {
    bone: u32,
    hierarchy: u32,
    source: u32,
    part: u8,
    method: u8,
}
/// Capture the target hierarchy's vector once, including map membership and
/// uniqueness. The returned address stays in this draw and is never serialized.
fn bone_vector(copies: &mut Copies, child: usize, hierarchy: u32, length: usize) -> Option<usize> {
    let map = copies.read(child.checked_add(layout::HIERARCHY_MAP)?, 24)?;
    let head = word(&map, 8)?;
    let count = word(&map, 16)?;
    if count == 0 || count > 16 {
        return None;
    }
    let sentinel = copies.read(head, 16)?;
    let mut node = word(&sentinel, 0)?;
    let mut previous = head;
    let mut visited = HashSet::new();
    let mut ids = HashSet::new();
    let mut selected = None;
    for _ in 0..count {
        if node == head || !visited.insert(node) {
            return None;
        }
        let entry = copies.read(node, 24)?;
        if word(&entry, 8)? != previous {
            return None;
        }
        let id = u32::from_le_bytes(entry.get(16..20)?.try_into().ok()?);
        if !ids.insert(id) {
            return None;
        }
        if id == hierarchy {
            let (begin, count) = copies.vector(node.checked_add(24)?, layout::BONE_BYTES, 256)?;
            if count != length {
                return None;
            }
            selected = Some(begin);
        }
        previous = node;
        node = word(&entry, 0)?;
    }
    if node != head || word(&sentinel, 8)? != previous {
        return None;
    }
    selected
}
fn constant_hash(
    copies: &mut Copies,
    begin: usize,
    index: usize,
    bone: &crate::animation_pose::Bone,
) -> Option<u64> {
    let bytes = copies.read(
        begin.checked_add(index.checked_mul(layout::BONE_BYTES)?)?,
        layout::BONE_BYTES,
    )?;
    if bytes[layout::BONE_METHOD] != layout::EXPRESSION_METHOD
        || bytes[layout::BONE_OVERRIDE] != 1
        || u64::from_le_bytes(
            bytes[layout::BONE_HASH..layout::BONE_HASH + 8]
                .try_into()
                .ok()?,
        ) != bone.name_hash
    {
        return None;
    }
    let matrix: [f32; 16] = std::array::from_fn(|index| {
        f32::from_le_bytes(
            bytes[layout::BONE_MATRIX + index * 4..layout::BONE_MATRIX + index * 4 + 4]
                .try_into()
                .expect("four bytes"),
        )
    });
    if matrix != bone.matrix {
        return None;
    }
    crate::native_molang_constant::hash(
        &bytes[layout::BONE_EXPRESSION..layout::BONE_EXPRESSION + 16],
        |address, size| copies.read(address, size),
    )
}
/// The inspected expression branch uses the first hierarchy returned by this
/// exact getter. Read its backing vector instead of invoking the virtual call.
fn query_hierarchy(copies: &mut Copies, root: usize) -> Option<u32> {
    let binding = query_profile::binding()?;
    let table = word(&copies.read(root, 8)?, 0)?;
    if word(
        &copies.read(table.checked_add(query_layout::SLOT_BYTES)?, 8)?,
        0,
    )? != binding.method
    {
        return None;
    }
    // The native getter forms a span from begin/end. Its capacity is irrelevant
    // to this read and is not established by the getter's code evidence.
    let header = copies.read(root.checked_add(query_layout::VECTOR_OFFSET)?, 16)?;
    let begin = word(&header, 0)?;
    let bytes = word(&header, 8)?.checked_sub(begin)?;
    if begin < 0x10000
        || bytes == 0
        || !bytes.is_multiple_of(query_layout::ELEMENT_BYTES)
        || bytes / query_layout::ELEMENT_BYTES > 64
    {
        return None;
    }
    Some(u32::from_le_bytes(
        copies
            .read(begin, query_layout::ELEMENT_BYTES)?
            .try_into()
            .ok()?,
    ))
}
/// Find one selected root geometry for the source hierarchy. Repeated layers
/// of the same geometry are fine; different geometry trees are ambiguous.
fn parent_geometry(copies: &mut Copies, root: usize, hierarchy: u32) -> Option<usize> {
    let (begin, count) = copies.vector(
        root.checked_add(selection::COMPONENT_PAIRS)?,
        selection::PAIR_STRIDE,
        64,
    )?;
    let entries = copies.read(begin, count.checked_mul(selection::PAIR_STRIDE)?)?;
    let mut selected = None;
    for entry in entries.chunks_exact(selection::PAIR_STRIDE) {
        if word(entry, selection::CONTROLLER)? == 0 {
            continue;
        }
        let control = word(entry, selection::GEOMETRY_CONTROL)?;
        if control == 0 || strong(control)? == 0 {
            continue;
        }
        let geometry = word(entry, selection::GEOMETRY)?;
        let index = copies.read(geometry.checked_add(model_body::GEOMETRY_HIERARCHY)?, 4)?;
        if u32::from_le_bytes(index.try_into().ok()?) != hierarchy {
            continue;
        }
        if let Some(previous) = selected {
            if previous != geometry {
                return None;
            }
        } else {
            copies.live(control)?;
            selected = Some(geometry);
        }
    }
    selected
}
fn parent_anchor(
    copies: &mut Copies,
    rigs: &mut Vec<ParentRig>,
    root: usize,
    pose: &crate::animation_pose::Pose,
    skin: &crate::native_body::SkinPose,
    hierarchy: u32,
    index: usize,
) -> Option<RigAnchor> {
    if !rigs.iter().any(|rig| rig.hierarchy.index == hierarchy) {
        if rigs.len() >= 16
            || crate::animation_pose::standard_parts(pose, hierarchy, skin.native_slim)?
                != skin.parts
        {
            return None;
        }
        let geometry = parent_geometry(copies, root, hierarchy)?;
        let hierarchy = pose
            .hierarchies
            .iter()
            .find(|source| source.index == hierarchy)?
            .clone();
        let rig = crate::native_palette_binding::Rig::copy(geometry, &hierarchy)?;
        rigs.push(ParentRig { hierarchy, rig });
    }
    rigs.iter()
        .find(|rig| rig.hierarchy.index == hierarchy)?
        .rig
        .anchor(index)
}

pub(crate) struct Binding {
    actor: usize,
    context: usize,
    root: usize,
    child: usize,
    model: usize,
    reload: u64,
    slot: u32,
    frame: u64,
    child_frame: u64,
    skin: crate::native_body::SkinPose,
    space: [[f32; 16]; 3],
    pub pose_space: crate::native_emote_space::Space,
    pub hierarchy: crate::animation_pose::Hierarchy,
    anchors: Vec<Seed>,
    parent_rigs: Vec<ParentRig>,
    parent_links: Vec<ParentLink>,
    dynamic_sources: Vec<crate::native_expression_history::Source>,
    copies: Copies,
}
impl Binding {
    pub(crate) fn slot(&self) -> u32 {
        self.slot
    }
    pub(crate) fn matches(&self, frame: u64, hierarchy: u32) -> bool {
        self.frame == frame && self.hierarchy.index == hierarchy
    }
    pub(crate) fn draw_matrix(&self) -> [f32; 16] {
        self.space[0]
    }
    pub(crate) fn valid(&self) -> bool {
        (|| {
            self.copies.unchanged()?;
            if self.child_frame != self.frame
                && crate::native_transform::attachment_setup_frame(
                    self.child,
                    self.model,
                    self.reload,
                ) != Some(self.child_frame)
            {
                return None;
            }
            for source in &self.dynamic_sources {
                if crate::native_transform::expression_binding(
                    self.child,
                    self.model,
                    self.reload,
                    self.child_frame,
                    source.hierarchy,
                    source.bone,
                    source.expression,
                )
                .as_ref()
                    != Some(source)
                {
                    return None;
                }
            }
            if crate::native_transform::draw_space(self.context)? != self.space {
                return None;
            }
            let root = crate::animation_pose::actor(self.actor, self.context)?;
            if root.update_frame != self.frame
                || crate::animation_pose::standard_parts(
                    &root,
                    self.skin.hierarchy,
                    self.skin.native_slim,
                )? != self.skin.parts
            {
                return None;
            }
            for parent in &self.parent_rigs {
                if !parent.rig.valid()
                    || !root
                        .hierarchies
                        .iter()
                        .any(|hierarchy| hierarchy == &parent.hierarchy)
                {
                    return None;
                }
            }
            let child = crate::animation_pose::attachment_at(
                self.actor,
                self.context,
                self.root,
                self.child,
                self.frame,
                self.child_frame,
            )?;
            if child.update_frame != self.child_frame
                || !child
                    .hierarchies
                    .iter()
                    .any(|hierarchy| hierarchy == &self.hierarchy)
            {
                return None;
            }
            if crate::native_transform::draw_space(self.context)? != self.space {
                return None;
            }
            self.copies.unchanged()
        })()
        .is_some()
    }
}
fn copy(
    actor: usize,
    context: usize,
    global: &crate::native_transform::Global,
    model: usize,
    params: usize,
    controller: usize,
    geometry: usize,
) -> Option<Arc<Binding>> {
    // The root renderer's attachable path calls the same controller/draw pair,
    // but passes a child component's RenderParams. Other callers cannot acquire
    // this binding by copying a few plausible pointer fields.
    let binding = profile::binding()?;
    let callers = crate::world_hooks::game_callers();
    if callers.len() < 2 || callers[0] != binding.body_return || callers[1] != binding.draw_return {
        return None;
    }
    let skin = global.skin_pose.as_ref()?;
    let draw = global.draw.as_ref()?;
    if skin.animation_frame != global.animation_frame {
        return None;
    }
    let space = crate::native_transform::draw_space(context)?;
    if space[1] != draw.view || space[2] != draw.projection {
        return None;
    }
    let pose_space = crate::native_emote_space::Space::between(&draw.matrix, &space[0])?;
    let mut copies = Copies {
        guards: Vec::new(),
        bytes: 0,
        refs: Vec::new(),
    };
    let root = word(
        &copies.read(
            actor.checked_add(crate::native_scene_layout::actor::ANIMATION_COMPONENT)?,
            8,
        )?,
        0,
    )?;
    let slot = u32::from_le_bytes(
        copies
            .read(params.checked_add(layout::SLOT_IN_PARAMS)?, 4)?
            .try_into()
            .ok()?,
    );
    if usize::try_from(slot).ok()? >= layout::SLOTS {
        return None;
    }
    // ChildAnimationComponentInfo is 80 bytes; its shared_ptr is at +0x40.
    let (children, count) = copies.vector(
        root.checked_add(layout::CHILDREN)?,
        layout::CHILD_STRIDE,
        layout::SLOTS,
    )?;
    if count != layout::SLOTS {
        return None;
    }
    // The intervening MolangVariableMap is mutable during native rendering.
    // Guard slot identity and child ownership, not its unrelated query caches.
    let mut children_by_slot = Vec::with_capacity(count);
    for index in 0..count {
        let entry = children.checked_add(index.checked_mul(layout::CHILD_STRIDE)?)?;
        copies.read(entry, 8)?;
        let shared = copies.read(entry.checked_add(layout::CHILD_SHARED)?, 16)?;
        children_by_slot.push((word(&shared, 0)?, word(&shared, 8)?));
    }
    let &(child, control) = children_by_slot.get(usize::try_from(slot).ok()?)?;
    copies.live(control)?;
    if child == root
        || params != child.checked_add(selection::COMPONENT_PARAMS)?
        || children_by_slot
            .iter()
            .filter(|(component, _)| *component == child)
            .count()
            != 1
    {
        return None;
    }
    let values = copies.read(params, 0x40)?;
    if word(&values, 0)? != context
        || word(&values, 0x10)? != child
        || word(&values, 0x18)? != root
        || word(&values, 0x20)? != model
        || word(&values, 0x38)? != actor
    {
        return None;
    }
    copies.live(word(&values, 0x28)?)?;
    if word(&copies.read(params.checked_add(0x78)?, 8)?, 0)? < 0x10000 {
        return None;
    }
    let root_frame = u64::from_le_bytes(
        copies
            .read(root.checked_add(selection::COMPONENT_FRAME)?, 8)?
            .try_into()
            .ok()?,
    );
    let child_frame = u64::from_le_bytes(
        copies
            .read(child.checked_add(selection::COMPONENT_FRAME)?, 8)?
            .try_into()
            .ok()?,
    );
    let reload = u64::from_le_bytes(
        copies
            .read(child.checked_add(layout::COMPONENT_RELOAD)?, 8)?
            .try_into()
            .ok()?,
    );
    if root_frame != global.animation_frame
        || child_frame != root_frame
            && crate::native_transform::attachment_setup_frame(child, model, reload)
                != Some(child_frame)
    {
        return None;
    }
    let (pairs, count) = copies.vector(
        child.checked_add(selection::COMPONENT_PAIRS)?,
        selection::PAIR_STRIDE,
        64,
    )?;
    let entries = copies.read(pairs, count.checked_mul(selection::PAIR_STRIDE)?)?;
    let mut selected = entries
        .chunks_exact(selection::PAIR_STRIDE)
        .filter(|entry| {
            word(entry, selection::CONTROLLER) == Some(controller)
                && word(entry, selection::GEOMETRY) == Some(geometry)
        });
    let entry = selected.next()?;
    if selected.next().is_some() {
        return None;
    }
    copies.live(word(entry, selection::CONTROLLER_CONTROL)?)?;
    copies.live(word(entry, selection::GEOMETRY_CONTROL)?)?;
    let hierarchy = u32::from_le_bytes(
        copies
            .read(geometry.checked_add(model_body::GEOMETRY_HIERARCHY)?, 4)?
            .try_into()
            .ok()?,
    );
    let mut pose =
        crate::animation_pose::attachment_at(actor, context, root, child, root_frame, child_frame)?;
    if pose.update_frame != child_frame {
        return None;
    }
    let selected = pose
        .hierarchies
        .iter()
        .position(|candidate| candidate.index == hierarchy)?;
    let hierarchy = pose.hierarchies.swap_remove(selected);
    let parent_pose = crate::animation_pose::actor(actor, context)?;
    if parent_pose.update_frame != global.animation_frame {
        return None;
    }
    // Follow explicit indexed parent copies before considering local rig names.
    // Source topology resolves a helper bone to its actual canonical ancestor.
    // No slot-to-hand mapping or coincident-matrix search supplies that role.
    let names = ["head", "body", "rightarm", "leftarm", "rightleg", "leftleg"];
    let mut anchors = Vec::with_capacity(hierarchy.bones.len());
    let mut parent_rigs = Vec::new();
    let mut parent_links = Vec::new();
    let mut dynamic_sources = Vec::new();
    let native_bones = if hierarchy.bones.iter().any(|bone| bone.binding_method == 3) {
        bone_vector(&mut copies, child, hierarchy.index, hierarchy.bones.len())
    } else {
        None
    };
    for (index, bone) in hierarchy.bones.iter().enumerate() {
        let bone_index = u32::try_from(index).ok()?;
        let seed = match bone.binding_method {
            2 | 3 => (|| {
                if !bone.override_stack {
                    return None;
                }
                let mut observed = None;
                let (source_hierarchy, source) = if bone.binding_method == 2 {
                    (
                        bone.parent_hierarchy,
                        usize::try_from(bone.parent_bone).ok()?,
                    )
                } else {
                    let raw = copies.read(
                        native_bones?
                            .checked_add(index.checked_mul(layout::BONE_BYTES)?)?
                            .checked_add(layout::BONE_EXPRESSION)?,
                        16,
                    )?;
                    if raw[8] != 0 {
                        return None;
                    }
                    let expression = word(&raw, 0)?;
                    if let Some(source) = crate::native_transform::expression_binding(
                        child,
                        model,
                        reload,
                        child_frame,
                        hierarchy.index,
                        bone_index,
                        expression,
                    ) {
                        let parent = parent_pose
                            .hierarchies
                            .iter()
                            .find(|parent| parent.index == source.source_hierarchy)?;
                        let selected = parent.bones.get(source.source_bone as usize)?;
                        if selected.name_hash != source.source_hash
                            || selected.matrix != source.matrix
                        {
                            return None;
                        }
                        observed = Some(source.clone());
                        (source.source_hierarchy, source.source_bone as usize)
                    } else {
                        let hash = constant_hash(&mut copies, native_bones?, index, bone)?;
                        let hierarchy = query_hierarchy(&mut copies, root)?;
                        let parent = parent_pose
                            .hierarchies
                            .iter()
                            .find(|parent| parent.index == hierarchy)?;
                        (
                            hierarchy,
                            parent
                                .bones
                                .iter()
                                .position(|bone| bone.name_hash == hash)?,
                        )
                    }
                };
                let parent = parent_pose
                    .hierarchies
                    .iter()
                    .find(|parent| parent.index == source_hierarchy)?;
                let parent_bone = parent.bones.get(source)?;
                // This exact native branch copies all sixteen values. Final
                // value agreement also rejects subsequent unproved transforms.
                if parent_bone.matrix != bone.matrix
                    || !pose_space.matches_anchor(&parent_bone.matrix, &bone.matrix)
                {
                    return None;
                }
                let mut anchor = parent_anchor(
                    &mut copies,
                    &mut parent_rigs,
                    root,
                    &parent_pose,
                    skin,
                    source_hierarchy,
                    source,
                )?;
                if let Some(source) = observed {
                    dynamic_sources.push(source);
                }
                parent_links.push(ParentLink {
                    bone: bone_index,
                    hierarchy: source_hierarchy,
                    source: u32::try_from(source).ok()?,
                    part: anchor.part,
                    method: bone.binding_method,
                });
                anchor.bone_index = bone_index;
                anchor.depth = anchor.depth.checked_add(1)?;
                Some(Seed::Bound(anchor))
            })()
            .unwrap_or(Seed::Blocked),
            _ => match names.iter().position(|name| bone.name == *name) {
                Some(part)
                    if pose_space.matches_anchor(
                        &crate::native_emote_palette::native_matrix(&skin.parts[part])?,
                        &bone.matrix,
                    ) =>
                {
                    Seed::Canonical(RigAnchor {
                        part: u8::try_from(part).ok()?,
                        bone_index,
                        depth: 0,
                    })
                }
                _ => Seed::Inherit,
            },
        };
        anchors.push(seed);
    }
    if !anchors
        .iter()
        .any(|anchor| matches!(anchor, Seed::Bound(_) | Seed::Canonical(_)))
    {
        return None;
    }
    let binding = Arc::new(Binding {
        actor,
        context,
        root,
        child,
        model,
        reload,
        slot,
        frame: global.animation_frame,
        child_frame,
        skin: skin.clone(),
        space,
        pose_space,
        hierarchy,
        anchors,
        parent_rigs,
        parent_links,
        dynamic_sources,
        copies,
    });
    binding.valid().then_some(binding)
}

pub(crate) fn prepare(
    model: usize,
    params: usize,
    controller: usize,
    geometry: usize,
) -> Option<(
    crate::native_submit::Tag,
    Option<crate::native_palette_binding::Binding>,
)> {
    if !crate::native_body::enabled() {
        return None;
    }
    let (actor, context, global) = crate::native_transform::attachment_parent()?;
    let owner = copy(actor, context, &global, model, params, controller, geometry)?;
    let name = crate::players::string(geometry.checked_add(16)?, 128)?;
    let palette = crate::native_palette_binding::copy_anchored(
        geometry,
        &owner.hierarchy,
        owner.frame,
        &owner.anchors,
    )?
    .with_attachment(owner.clone())?;
    if !owner.valid() {
        return None;
    }
    COPIES.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut last) = LAST.try_lock() {
        *last = Some(
            serde_json::json!({"actorId":global.actor_id,"worldScope":global.world_scope,
            "slot":owner.slot,"geometry":name,"hierarchy":owner.hierarchy.index,"frame":owner.frame,"nativeChildFrame":owner.child_frame,
            "sameDrawSpace":owner.pose_space.identity(),
            "parentLinks":owner.parent_links.iter().map(|link|serde_json::json!({"bone":link.bone,
                "sourceHierarchy":link.hierarchy,"sourceBone":link.source,"rigPart":link.part,
                "bindingMethod":link.method})).collect::<Vec<_>>(),
            "blockedBindings":owner.anchors.iter().filter(|seed|matches!(seed,Seed::Blocked)).count(),
            "observedExpressionSources":owner.dynamic_sources.len(),
            "nativeEquipmentRenderingVerified":false}),
        );
    }
    Some((
        crate::native_submit::Tag {
            scope: global.world_scope,
            actor: global.actor_id,
            geometry: name,
            replacement: None,
            role: crate::native_submit::MeshRole::Other,
        },
        Some(palette),
    ))
}
pub(crate) fn validate_profile(base: usize) -> Result<(), String> {
    query_profile::install(base)?;
    crate::native_molang_constant::validate_profile(base)?;
    profile::install(base)
}
pub(crate) fn status() -> serde_json::Value {
    serde_json::json!({"copies":COPIES.load(Ordering::Relaxed),"last":LAST.try_lock().ok().and_then(|last|last.clone()),
        "hierarchyQueryReaderInstalled":query_profile::binding().is_some(),"hierarchyQueryReaderSourceImplemented":true,
        "constantExpressionReaderInstalled":crate::native_molang_constant::reader_installed(),
        "equipmentReaderProfileInstalled":profile::binding().is_some(),"equipmentReaderProfileSourceImplemented":true,
        "bodyReturnRva":profile::binding().map(|binding|binding.body_return),
        "equipmentDrawReturnRva":profile::binding().map(|binding|binding.draw_return),
        "childPaletteBindingImplemented":true,"differentDrawSpaceConversionImplemented":true,
        "indexedParentBindingsImplemented":true,"constantExpressionParentBindingsImplemented":true,
        "dynamicExpressionParentObservationImplemented":true,
        "dynamicExpressionObserver":crate::native_expression_binding::status(),
        "nativeEquipmentRenderingVerified":false})
}
