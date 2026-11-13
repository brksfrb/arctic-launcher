//! Owned animation output from the exact client's native actor renderer.
//! These matrices remain observations until their space and global transform
//! have been corroborated; reading six names alone is not a complete body pose.
use crate::world_hooks::{pointer, read};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::{
    Mutex,
    atomic::{AtomicU32, AtomicU64, Ordering},
};
static ATTEMPTS: AtomicU64 = AtomicU64::new(0);
static COPIES: AtomicU64 = AtomicU64::new(0);
static HEADER: Mutex<Option<serde_json::Value>> = Mutex::new(None);
static STEP: AtomicU32 = AtomicU32::new(0);
static LINKS: Mutex<Option<serde_json::Value>> = Mutex::new(None);
pub(crate) fn status() -> serde_json::Value {
    serde_json::json!({"attempts":ATTEMPTS.load(Ordering::Relaxed),"copies":COPIES.load(Ordering::Relaxed),"step":STEP.load(Ordering::Relaxed),"header":HEADER.try_lock().ok().and_then(|v|v.clone()),"parentLinks":LINKS.try_lock().ok().and_then(|v|v.clone())})
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pose {
    pub update_frame: u64,
    pub parent_links_resolved: bool,
    pub hierarchies: Vec<Hierarchy>,
}
#[derive(Clone, Deserialize, Serialize, PartialEq)]
pub struct Hierarchy {
    pub index: u32,
    pub bones: Vec<Bone>,
}
#[derive(Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Bone {
    pub name: String,
    #[serde(skip)]
    pub(crate) name_hash: u64,
    pub parent_bone: i32,
    pub parent_hierarchy: u32,
    pub matrix: [f32; 16],
    pub transform: [f32; 9],
    pub default_transform: [f32; 9],
    pub pivot: [f32; 3],
    /// Copied native binding state; absent from serialized pose/history data.
    #[serde(skip)]
    pub(crate) binding_method: u8,
    #[serde(skip)]
    pub(crate) override_stack: bool,
}

/// Copy completed matrices from the hierarchy selected by an actual native
/// skin draw or a guarded root-controller selection. Rest metadata identifies
/// the standard rig independently of the
/// current crouch/swim/ride/attack animation. Bone names alone do not qualify a
/// persona or armor hierarchy, and an unknown parent is never traversed.
pub(crate) fn standard_parts(pose: &Pose, selected: u32, slim: bool) -> Option<[[f32; 12]; 6]> {
    let mut candidates = pose.hierarchies.iter().filter(|h| h.index == selected);
    let hierarchy = candidates.next()?;
    if candidates.next().is_some() {
        return None;
    }
    let names = ["head", "body", "rightarm", "leftarm", "rightleg", "leftleg"];
    let mut defaults = [
        [0., 0., 0.],
        [0., 12., 0.],
        [-5., -2., 0.],
        [5., -2., 0.],
        [-1.9, 12., 0.],
        [1.9, 12., 0.],
    ];
    let mut pivots = [
        [0., 24., 0.],
        [0., 24., 0.],
        [-5., 22., 0.],
        [5., 22., 0.],
        [-1.9, 12., 0.],
        [1.9, 12., 0.],
    ];
    // The vanilla slim rig's shoulders are half a pixel lower. Use the
    // observed skin's arm model, rather than accepting either rest layout
    // for an unrelated skin or silently rejecting every slim hierarchy.
    if slim {
        for i in [2, 3] {
            defaults[i][1] = -2.5;
            pivots[i][1] = 21.5;
        }
    }
    let mut result = [[0.; 12]; 6];
    for (i, name) in names.iter().enumerate() {
        let mut matches = hierarchy.bones.iter().filter(|b| b.name == *name);
        let bone = matches.next()?;
        if matches.next().is_some() {
            return None;
        }
        let m = &bone.matrix;
        if bone
            .default_transform
            .iter()
            .chain(&bone.pivot)
            .chain(m)
            .any(|v| !v.is_finite())
            || (0..3).any(|r| {
                (bone.default_transform[r] - defaults[i][r]).abs() > 0.002
                    || (bone.pivot[r] - pivots[i][r]).abs() > 0.002
                    || bone.default_transform[r + 3].abs() > 0.002
                    || (bone.default_transform[r + 6] - 1.).abs() > 0.002
                    || m[12 + r].abs() > 4096.
            })
            || [3, 7, 11].iter().any(|r| m[*r].abs() > 0.0001)
            || (m[15] - 1.).abs() > 0.0001
        {
            return None;
        }
        for a in 0..3 {
            for b in 0..3 {
                let dot: f32 = (0..3).map(|r| m[a * 4 + r] * m[b * 4 + r]).sum();
                if (dot - if a == b { 1. } else { 0. }).abs() > 0.001 {
                    return None;
                }
            }
        }
        let det = m[0] * (m[5] * m[10] - m[9] * m[6]) - m[4] * (m[1] * m[10] - m[9] * m[2])
            + m[8] * (m[1] * m[6] - m[5] * m[2]);
        if (det - 1.).abs() > 0.001 {
            return None;
        }
        for r in 0..3 {
            for c in 0..3 {
                result[i][r * 4 + c] = m[c * 4 + r];
            }
            result[i][r * 4 + 3] = m[12 + r] / 16.;
        }
    }
    Some(result)
}

/// The installed updateBoneTransform at 0x1bc3a30 stores the completed
/// bone-to-entity matrix at BoneOrientation+0x38 (stores 0x1bc3f22..0x1bc3f2e).
/// Parent mapping is not traversed. Authorize only a corroborated standard
/// standing rig: exact pivots, rigid matrices, identity torso and matching look.
pub(crate) fn standing_parts(pose: &Pose, pitch: f32, yaw: f32) -> Option<[[f32; 12]; 6]> {
    let names = ["head", "body", "rightarm", "leftarm", "rightleg", "leftleg"];
    let pivots = [
        [0., 0., 0.],
        [0., 0., 0.],
        [-5., 2., 0.],
        [5., 2., 0.],
        [-1.9, 12., 0.],
        [1.9, 12., 0.],
    ];
    let mut candidates = pose.hierarchies.iter().filter(|h| {
        names
            .iter()
            .all(|name| h.bones.iter().any(|b| b.name == *name))
    });
    let hierarchy = candidates.next()?;
    if candidates.next().is_some() || !pitch.is_finite() || !yaw.is_finite() {
        return None;
    }
    let (sx, cx) = pitch.sin_cos();
    let (sy, cy) = yaw.sin_cos();
    let head = [cy, sy * sx, sy * cx, 0., cx, -sx, -sy, cy * sx, cy * cx];
    let identity = [1., 0., 0., 0., 1., 0., 0., 0., 1.];
    let mut result = [[0.; 12]; 6];
    for (i, name) in names.iter().enumerate() {
        let bone = hierarchy.bones.iter().find(|b| b.name == *name)?;
        let m = &bone.matrix;
        if m.iter().any(|v| !v.is_finite())
            || (0..3).any(|r| (m[12 + r] - pivots[i][r]).abs() > 0.002)
        {
            return None;
        }
        for a in 0..3 {
            for b in 0..3 {
                let dot: f32 = (0..3).map(|r| m[a * 4 + r] * m[b * 4 + r]).sum();
                if (dot - if a == b { 1. } else { 0. }).abs() > 0.001 {
                    return None;
                }
            }
        }
        let det = m[0] * (m[5] * m[10] - m[9] * m[6]) - m[4] * (m[1] * m[10] - m[9] * m[2])
            + m[8] * (m[1] * m[6] - m[5] * m[2]);
        if (det - 1.).abs() > 0.001 {
            return None;
        }
        for r in 0..3 {
            for c in 0..3 {
                let v = m[c * 4 + r];
                if i < 2
                    && (v - if i == 0 {
                        head[r * 3 + c]
                    } else {
                        identity[r * 3 + c]
                    })
                    .abs()
                        > 0.002
                {
                    return None;
                }
                result[i][r * 4 + c] = v;
            }
            result[i][r * 4 + 3] = m[12 + r] / 16.;
        }
    }
    Some(result)
}
fn word(address: usize) -> Option<u64> {
    let mut bytes = [0u8; 8];
    read(address, &mut bytes).ok()?;
    Some(u64::from_le_bytes(bytes))
}
fn integer(address: usize) -> Option<u32> {
    let mut bytes = [0u8; 4];
    read(address, &mut bytes).ok()?;
    Some(u32::from_le_bytes(bytes))
}
fn values<const N: usize>(bytes: &[u8], offset: usize, bound: f32) -> Option<[f32; N]> {
    let data = bytes.get(offset..offset.checked_add(N * 4)?)?;
    let result = std::array::from_fn(|i| {
        f32::from_le_bytes(data[i * 4..i * 4 + 4].try_into().expect("four bytes"))
    });
    result
        .iter()
        .all(|v| v.is_finite() && v.abs() <= bound)
        .then_some(result)
}

pub fn actor(actor: usize, context: usize) -> Option<Pose> {
    ATTEMPTS.fetch_add(1, Ordering::Relaxed);
    // The approved legacy Player::getAnimationComponent tail path supplies the
    // actor member pinned by the shared scene family (RVAs 0x210c69/0x1a28250).
    // No constructor, virtual method, or Molang expression is executed here.
    let component =
        pointer(actor.checked_add(crate::native_scene_layout::actor::ANIMATION_COMPONENT)?)?;
    if let Ok(mut header) = HEADER.try_lock() {
        *header = (|| {
            let mut initialized = [0u8];
            read(component.checked_add(0x358)?, &mut initialized).ok()?;
            let count = word(component.checked_add(0x328)?)?;
            Some(
                serde_json::json!({"initialized":(initialized[0]<=1).then_some(initialized[0]!=0),
                "contextMatches":pointer(component.checked_add(0x88)?)==Some(context),
                "actorMatches":pointer(component.checked_add(0xc0)?)==Some(actor),"hierarchies":(count<=16).then_some(count),
                "renderParams":crate::world_hooks::floats::<8>(component.checked_add(0x190)?,1e6)}),
            )
        })();
    }
    let pose = sample(component, actor, context)?;
    COPIES.fetch_add(1, Ordering::Relaxed);
    Some(pose)
}
/// The caller must prove the child belongs to the active actor's attachment
/// slot. Keep root-pointer and ordinary actor/context checks in the sampler;
/// never interpret a child as the root component selected by the actor family.
pub(crate) fn attachment(actor: usize, context: usize, root: usize, child: usize) -> Option<Pose> {
    let frame = word(root.checked_add(0x368)?)?;
    attachment_at(actor, context, root, child, frame, frame)
}
/// A completed native setup receipt can fence a deliberately invalidated child
/// counter separately from the root's current frame. Neither value is guessed.
pub(crate) fn attachment_at(
    actor: usize,
    context: usize,
    root: usize,
    child: usize,
    root_frame: u64,
    child_frame: u64,
) -> Option<Pose> {
    if pointer(actor.checked_add(crate::native_scene_layout::actor::ANIMATION_COMPONENT)?)? != root
        || child == root
        || pointer(child.checked_add(0xa0)?)? != root
    {
        return None;
    }
    let parent = sample(root, actor, context)?;
    let pose = sample_with_parent(child, actor, context, Some(&parent))?;
    if pointer(actor.checked_add(crate::native_scene_layout::actor::ANIMATION_COMPONENT)?)? != root
        || pointer(child.checked_add(0xa0)?)? != root
        || parent.update_frame != root_frame
        || pose.update_frame != child_frame
        || word(root.checked_add(0x368)?)? != parent.update_frame
    {
        return None;
    }
    Some(pose)
}
fn sample(component: usize, actor: usize, context: usize) -> Option<Pose> {
    sample_with_parent(component, actor, context, None)
}
fn sample_with_parent(
    component: usize,
    actor: usize,
    context: usize,
    parent: Option<&Pose>,
) -> Option<Pose> {
    STEP.store(0, Ordering::Relaxed);
    let mut initialized = [0];
    read(component.checked_add(0x358)?, &mut initialized).ok()?;
    if initialized[0] != 1
        || pointer(component.checked_add(0x88)?)? != context
        || pointer(component.checked_add(0xc0)?)? != actor
    {
        return None;
    }
    STEP.store(1, Ordering::Relaxed);
    let frame = word(component.checked_add(0x368)?)?;
    let map = component.checked_add(0x318)?;
    let head = pointer(map.checked_add(8)?)?;
    let count = word(map.checked_add(16)?)? as usize;
    if count == 0 || count > 16 {
        return None;
    }
    STEP.store(2, Ordering::Relaxed);
    let mut node = pointer(head)?;
    let mut previous = head;
    let mut visited = HashSet::new();
    let mut ids = HashSet::new();
    let mut hierarchies = Vec::with_capacity(count);
    let mut total = 0usize;
    for _ in 0..count {
        if node == head || !visited.insert(node) || pointer(node.checked_add(8)?)? != previous {
            return None;
        }
        let index = integer(node.checked_add(16)?)?;
        if !ids.insert(index) {
            return None;
        }
        STEP.store(3, Ordering::Relaxed);
        // node links(16), uint32 hierarchy key(4), alignment(4), vector(24).
        let start = pointer(node.checked_add(24)?)?;
        let end = word(node.checked_add(32)?)? as usize;
        let capacity = word(node.checked_add(40)?)? as usize;
        let bytes = end.checked_sub(start)?;
        if !bytes.is_multiple_of(0xe8) || capacity < end || capacity - start > 0xe8 * 4096 {
            return None;
        }
        let length = bytes / 0xe8;
        total = total.checked_add(length)?;
        if length == 0 || length > 256 || total > 512 {
            return None;
        }
        STEP.store(4, Ordering::Relaxed);
        let mut bones = Vec::with_capacity(length);
        let mut names = HashSet::new();
        // Copy the contiguous vector once. Reading every float through the OS
        // separately scales poorly in crowded worlds and widens the interval
        // over which a native animation update could be observed inconsistently.
        let mut storage = vec![0u8; bytes];
        read(start, &mut storage).ok()?;
        STEP.store(5, Ordering::Relaxed);
        for i in 0..length {
            let b = &storage[i * 0xe8..(i + 1) * 0xe8];
            if b[0xe4] > 3 || b[0xe6] > 1 {
                return None;
            }
            let name = crate::players::string_header(b.get(0x10..0x30)?.try_into().ok()?, 64)?;
            if !names.insert(name.clone()) {
                return None;
            }
            STEP.store(6, Ordering::Relaxed);
            let matrix = values::<16>(b, 0x38, 1_000_000.)?;
            STEP.store(7, Ordering::Relaxed);
            // GLM matrices are column-major. A perspective or non-affine value
            // is not usable as a native attachment transform.
            if matrix[3].abs() > 0.0001
                || matrix[7].abs() > 0.0001
                || matrix[11].abs() > 0.0001
                || (matrix[15] - 1.).abs() > 0.0001
            {
                return None;
            }
            STEP.store(8, Ordering::Relaxed);
            bones.push(Bone {
                name,
                name_hash: u64::from_le_bytes(b[8..16].try_into().ok()?),
                parent_bone: i32::from_le_bytes(b[..4].try_into().ok()?),
                parent_hierarchy: u32::from_le_bytes(b[4..8].try_into().ok()?),
                matrix,
                transform: values(b, 0x78, 1_000_000.)?,
                default_transform: values(b, 0x9c, 1_000_000.)?,
                pivot: values(b, 0xd8, 65536.)?,
                binding_method: b[0xe4],
                override_stack: b[0xe6] != 0,
            });
        }
        if pointer(node.checked_add(24)?)? != start
            || word(node.checked_add(32)?)? as usize != end
            || word(node.checked_add(40)?)? as usize != capacity
        {
            return None;
        }
        hierarchies.push(Hierarchy { index, bones });
        previous = node;
        node = pointer(node)?;
    }
    if node != head
        || pointer(head.checked_add(8)?)? != previous
        || pointer(map.checked_add(8)?)? != head
        || word(map.checked_add(16)?)? as usize != count
        || word(component.checked_add(0x368)?)? != frame
    {
        return None;
    }
    // Validate references only after every hierarchy is copied. These remain
    // observations; unresolved native links must never become invented roots.
    STEP.store(9, Ordering::Relaxed);
    if let Ok(mut links) = LINKS.try_lock() {
        *links = Some(serde_json::json!(hierarchies.iter().take(4).map(|h|serde_json::json!({"index":h.index,"bones":h.bones.len(),
            "links":h.bones.iter().take(8).map(|b|serde_json::json!({"parent":b.parent_bone,"hierarchy":b.parent_hierarchy})).collect::<Vec<_>>()
        })).collect::<Vec<_>>()));
    }
    let parent_links_resolved =
        validate_links(&hierarchies, parent.map(|pose| pose.hierarchies.as_slice()))?;
    STEP.store(10, Ordering::Relaxed);
    Some(Pose {
        update_frame: frame,
        parent_links_resolved,
        hierarchies,
    })
}

fn validate_links(hierarchies: &[Hierarchy], root: Option<&[Hierarchy]>) -> Option<bool> {
    let mut resolved = true;
    for h in hierarchies {
        for i in 0..h.bones.len() {
            // A child and its root may use the same numeric hierarchy key.
            // Keep their component namespaces distinct when following links.
            let mut parent = (false, h.index, i);
            let mut chain = HashSet::new();
            loop {
                if !chain.insert(parent) {
                    return None;
                }
                let source = if parent.0 { root? } else { hierarchies };
                let current = source
                    .iter()
                    .find(|x| x.index == parent.1)?
                    .bones
                    .get(parent.2)?;
                // Expression bindings copy a matrix directly and need not
                // update these indices. Old index values are not ancestry.
                if current.binding_method == 3 {
                    resolved = false;
                    break;
                }
                // Installed BoneOrientation constructors at 0x1bc28f0,
                // 0x1bc2a60 and 0x1bc3960 initialize the first eight bytes to
                // 0xfffffffffffffffe: parent -2, hierarchy Unknown (-1).
                // Real standard player bones retain this state. Preserve it
                // explicitly without authorizing attachment traversal.
                if current.parent_bone == -2 && current.parent_hierarchy == u32::MAX {
                    resolved = false;
                    break;
                }
                if current.parent_bone == -1 {
                    break;
                }
                if current.parent_bone < 0 {
                    return None;
                }
                parent = (
                    parent.0 || root.is_some() && current.binding_method == 2,
                    current.parent_hierarchy,
                    current.parent_bone as usize,
                );
            }
        }
    }
    Some(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recorded_player_matrices_map_six_parts_without_resolving_native_parent_defaults() {
        let json = include_str!("../inspection/standing-pose.json");
        let mut pose: Pose = serde_json::from_str(json).unwrap();
        let parts = standing_parts(&pose, 0., -50f32.to_radians()).unwrap();
        assert_eq!(parts[2][3], -5. / 16.);
        assert_eq!(parts[3][7], 2. / 16.);
        assert!((parts[4][3] + 1.9 / 16.).abs() < 0.00001);
        assert!(parts[2][1] < 0.); // native right arm's positive Z turn
        assert!(!pose.parent_links_resolved);
        assert!(standing_parts(&pose, 0., 0.).is_none());
        let leg = pose.hierarchies[0]
            .bones
            .iter_mut()
            .find(|b| b.name == "rightleg")
            .unwrap();
        leg.matrix[12] = -20.;
        assert!(standing_parts(&pose, 0., -50f32.to_radians()).is_none());
        let mut reflected: Pose = serde_json::from_str(json).unwrap();
        reflected.hierarchies[0].bones[0].matrix[0] *= -1.;
        assert!(standing_parts(&reflected, 0., -50f32.to_radians()).is_none());
        let mut ambiguous: Pose = serde_json::from_str(json).unwrap();
        ambiguous.hierarchies.push(
            serde_json::from_str::<Pose>(json)
                .unwrap()
                .hierarchies
                .remove(0),
        );
        assert!(standing_parts(&ambiguous, 0., -50f32.to_radians()).is_none());
    }
    fn point(data: &mut [u8], at: usize, value: usize) {
        data[at..at + 8].copy_from_slice(&(value as u64).to_le_bytes());
    }
    #[test]
    fn copies_animation_bones_after_render_and_rejects_foreign_context_bad_links_and_cycles() {
        let mut component = vec![0u8; 0x380];
        let mut head = vec![0u8; 16];
        let mut node = vec![0u8; 48];
        let mut bones = vec![0u8; 0xe8 * 2];
        let actor = 0x123450;
        let context = 0x234560;
        component[0x358] = 1;
        point(&mut component, 0x88, context);
        point(&mut component, 0xc0, actor);
        point(&mut component, 0x368, 123);
        point(&mut component, 0x320, head.as_ptr() as usize);
        point(&mut component, 0x328, 1);
        point(&mut head, 0, node.as_ptr() as usize);
        point(&mut head, 8, node.as_ptr() as usize);
        point(&mut node, 0, head.as_ptr() as usize);
        point(&mut node, 8, head.as_ptr() as usize);
        point(&mut node, 24, bones.as_ptr() as usize);
        point(&mut node, 32, bones.as_ptr() as usize + bones.len());
        point(&mut node, 40, bones.as_ptr() as usize + bones.len());
        for (i, name) in [b"body".as_slice(), b"head".as_slice()].iter().enumerate() {
            let at = i * 0xe8;
            let parent = if i == 0 { -1i32 } else { 0 };
            bones[at..at + 4].copy_from_slice(&parent.to_le_bytes());
            bones[at + 0x10..at + 0x10 + name.len()].copy_from_slice(name);
            point(&mut bones, at + 0x20, name.len());
            point(&mut bones, at + 0x28, 15);
            for j in [0, 5, 10, 15] {
                bones[at + 0x38 + j * 4..at + 0x3c + j * 4].copy_from_slice(&1f32.to_le_bytes());
            }
        }
        let copy = sample(component.as_ptr() as usize, actor, context).unwrap();
        assert_eq!(copy.hierarchies[0].bones[1].name, "head");
        assert_eq!(copy.update_frame, 123);
        assert!(copy.parent_links_resolved);
        assert!(sample(component.as_ptr() as usize, actor, context + 8).is_none());
        bones[0..4].copy_from_slice(&(-2i32).to_le_bytes());
        bones[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        std::hint::black_box(&bones);
        let unresolved = sample(component.as_ptr() as usize, actor, context).unwrap();
        assert!(!unresolved.parent_links_resolved);
        assert_eq!(unresolved.hierarchies[0].bones[0].parent_bone, -2);
        bones[0..4].copy_from_slice(&(-3i32).to_le_bytes());
        std::hint::black_box(&bones);
        assert!(sample(component.as_ptr() as usize, actor, context).is_none());
        bones[4..8].copy_from_slice(&0u32.to_le_bytes());
        bones[0..4].copy_from_slice(&1i32.to_le_bytes());
        std::hint::black_box(&bones);
        assert!(sample(component.as_ptr() as usize, actor, context).is_none());
        assert_eq!(copy.hierarchies[0].bones[0].parent_bone, -1);
        let node_address = node.as_ptr() as usize;
        point(&mut node, 0, node_address);
        std::hint::black_box(&node);
        assert!(sample(component.as_ptr() as usize, actor, context).is_none());
    }
}
