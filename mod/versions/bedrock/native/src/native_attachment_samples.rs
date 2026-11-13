//! Bounded, pointer-free final current-palette observations in root pixel space.
//! This records checked native copies, not proof of GPU or equipment coverage.
use crate::native_palette_binding::Named;
use serde::{Deserialize, Serialize};
/// Archive role only; native child-component slots remain strictly 0..7.
pub(crate) const CAPE_SLOT: u32 = 7;
/// A root wing layer has its own archive role, beyond native child slots.
pub(crate) const WING_SLOT: u32 = 8;

#[derive(Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Bone {
    pub name: String,
    pub index: u32,
    pub palette: u32,
    pub rig_part: Option<u8>,
    pub anchor_bone: Option<u32>,
    pub ancestor_depth: Option<u16>,
    pub matrix: [f32; 16],
}
#[derive(Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Palette {
    pub slot: u32,
    pub geometry: String,
    pub definition: String,
    pub hierarchy: u32,
    pub group: u32,
    pub owner: u32,
    pub bones: Vec<Bone>,
    #[serde(default)]
    pub native_cape: Option<String>,
    /// Current owned-output provenance; never stored in the binary archive.
    #[serde(default)]
    pub replay: Option<crate::native_emote_history::Replay>,
}
impl Palette {
    pub(crate) fn valid(&self) -> bool {
        if self.slot > WING_SLOT
            || self.group >= 64
            || self.owner >= 64
            || self.geometry.is_empty()
            || self.geometry.len() > 128
            || self.slot == WING_SLOT && self.geometry != "geometry.elytra"
            || self.geometry.chars().any(char::is_control)
            || self.definition.len() != 64
            || !self
                .definition
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || self.bones.is_empty()
            || self.bones.len() > 8
            || (self.slot == CAPE_SLOT) != self.native_cape.is_some()
            || self.native_cape.as_ref().is_some_and(|key| {
                !key.strip_prefix("engine:cape/").is_some_and(|hash| {
                    hash.len() == 64
                        && hash
                            .bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                })
            })
            || self.replay.as_ref().is_some_and(|replay| !replay.valid())
        {
            return false;
        }
        self.bones.iter().enumerate().all(|(i, b)| {
            b.index < 256
                && b.palette == i as u32
                && !b.name.is_empty()
                && b.name.len() <= 64
                && !b.name.chars().any(char::is_control)
                && (match (b.rig_part, b.anchor_bone, b.ancestor_depth) {
                    (None, None, None) => true,
                    (Some(part), Some(bone), Some(depth)) => part < 6 && bone < 256 && depth < 256,
                    _ => false,
                })
                && b.matrix
                    .iter()
                    .all(|v| v.is_finite() && v.abs() <= 1_000_000.)
                && [3, 7, 11].iter().all(|i| b.matrix[*i].abs() <= 0.000001)
                && (b.matrix[15] - 1.).abs() <= 0.000001
        })
    }
    /// The recorded baked root pose can select another parent from the live
    /// Molang result. Match static indexed layout; never borrow its old AST.
    pub(crate) fn matches_draw(&self, live: &Self) -> bool {
        self.same_key(live)
            && self.bones.len() == live.bones.len()
            && self
                .bones
                .iter()
                .zip(&live.bones)
                .all(|(a, b)| a.name == b.name && a.index == b.index && a.palette == b.palette)
    }
    pub(crate) fn copy(named: &Named, geometry: &str, matrices: &[[f32; 16]]) -> Option<Self> {
        let slot = named.attachment_slot?;
        if slot > WING_SLOT
            || geometry.is_empty()
            || geometry.len() > 128
            || geometry.chars().any(char::is_control)
            || named.bones.is_empty()
            || named.bones.len() > 8
            || matrices.len() != named.bones.len()
            || named.definition.len() != 64
        {
            return None;
        }
        let space = named.space.as_ref()?;
        let first = named.bones.first()?;
        let mut bones = Vec::with_capacity(matrices.len());
        for (index, (bone, matrix)) in named.bones.iter().zip(matrices).enumerate() {
            if bone.index >= 256
                || bone.cache.group != first.cache.group
                || bone.owner != first.owner
                || bone.cache.palette != index as u32
                || bone.name.is_empty()
                || bone.name.len() > 64
                || bone.name.chars().any(char::is_control)
            {
                return None;
            }
            bones.push(Bone {
                name: bone.name.clone(),
                index: bone.index,
                palette: bone.cache.palette,
                rig_part: bone.anchor.as_ref().map(|anchor| anchor.part),
                anchor_bone: bone.anchor.as_ref().map(|anchor| anchor.bone_index),
                ancestor_depth: bone.anchor.as_ref().map(|anchor| anchor.depth),
                matrix: space.root_matrix(matrix)?,
            });
        }
        let palette = Self {
            slot,
            geometry: geometry.into(),
            definition: named.definition.clone(),
            hierarchy: named.hierarchy,
            group: first.cache.group,
            owner: first.owner,
            bones,
            native_cape: named.native_cape.clone(),
            replay: None,
        };
        palette.valid().then_some(palette)
    }
    fn same_key(&self, other: &Self) -> bool {
        self.slot == other.slot
            && self.geometry == other.geometry
            && self.definition == other.definition
            && self.hierarchy == other.hierarchy
            && self.group == other.group
            && self.owner == other.owner
            && self.native_cape == other.native_cape
    }
    fn same_layout(&self, other: &Self) -> bool {
        self.same_key(other)
            && self.bones.len() == other.bones.len()
            && self.bones.iter().zip(&other.bones).all(|(a, b)| {
                a.name == b.name
                    && a.index == b.index
                    && a.palette == b.palette
                    && a.rig_part == b.rig_part
                    && a.anchor_bone == b.anchor_bone
                    && a.ancestor_depth == b.ancestor_depth
            })
    }
    pub(crate) fn bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.geometry.capacity()
            + self.definition.capacity()
            + self.native_cape.as_ref().map_or(0, String::capacity)
            + self
                .replay
                .as_ref()
                .map_or(0, |r| r.manifest_hash.capacity())
            + self.bones.capacity() * std::mem::size_of::<Bone>()
            + self
                .bones
                .iter()
                .map(|bone| bone.name.capacity())
                .sum::<usize>()
    }
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Frame {
    pub schema: u32,
    pub animation_frame: u64,
    pub complete: bool,
    pub coverage_verified: bool,
    pub palettes: Vec<Palette>,
}
impl Frame {
    pub(crate) fn valid(&self) -> bool {
        let mut keys = std::collections::HashSet::new();
        (1..=3).contains(&self.schema)
            && self.animation_frame <= i64::MAX as u64
            && !self.coverage_verified
            && self.palettes.len() <= 16
            && (self.complete || self.palettes.is_empty())
            && self.palettes.iter().map(|p| p.bones.len()).sum::<usize>() <= 128
            && self.palettes.iter().all(|p| {
                p.valid()
                    && (self.schema >= 2 || p.slot < CAPE_SLOT && p.replay.is_none())
                    && (self.schema >= 3 || p.slot < WING_SLOT)
                    && keys.insert((
                        p.slot,
                        p.geometry.as_str(),
                        p.definition.as_str(),
                        p.hierarchy,
                        p.group,
                        p.owner,
                    ))
            })
    }
    pub(crate) fn bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.palettes.capacity() * std::mem::size_of::<Palette>()
            + self.palettes.iter().map(Palette::bytes).sum::<usize>()
    }
}
struct Entry {
    reference: Palette,
    supplied: Option<Palette>,
    pending: u32,
}
#[derive(Default)]
pub(crate) struct Ledger {
    frame: Option<u64>,
    entries: Vec<Entry>,
    invalid: bool,
}
impl Ledger {
    pub(crate) fn advance_frame(&mut self) {
        self.entries.clear();
        self.frame = None;
    }
    pub(crate) fn new(observing: bool) -> Self {
        Self {
            invalid: !observing,
            ..Self::default()
        }
    }
    pub(crate) fn invalidate(&mut self) {
        self.invalid = true;
        self.entries.clear();
    }
    fn accept(&mut self, frame: u64) -> bool {
        if self.invalid {
            return false;
        }
        if self.frame.is_some_and(|old| old != frame) {
            self.invalidate();
            return false;
        }
        self.frame = Some(frame);
        true
    }
    pub(crate) fn begin(&mut self, frame: u64, reference: Palette, pending: bool) -> bool {
        if !self.accept(frame) {
            return false;
        }
        let index = if let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.reference.same_key(&reference))
        {
            if self.entries[index].reference != reference {
                self.invalidate();
                return false;
            }
            index
        } else {
            let bones = self
                .entries
                .iter()
                .map(|entry| entry.reference.bones.len())
                .sum::<usize>();
            if self.entries.len() >= 16 || bones + reference.bones.len() > 128 {
                self.invalidate();
                return false;
            }
            self.entries.push(Entry {
                reference: reference.clone(),
                supplied: None,
                pending: 0,
            });
            self.entries.len() - 1
        };
        let entry = &mut self.entries[index];
        if pending {
            let Some(count) = entry.pending.checked_add(1).filter(|count| *count <= 512) else {
                self.invalidate();
                return false;
            };
            entry.pending = count;
        } else if entry.supplied.as_ref().is_some_and(|old| old != &reference) {
            self.invalidate();
            return false;
        } else {
            entry.supplied = Some(reference);
        }
        if self
            .entries
            .iter()
            .map(|entry| {
                entry.reference.bytes() + entry.supplied.as_ref().map_or(0, Palette::bytes)
            })
            .sum::<usize>()
            > 256 * 1024
        {
            self.invalidate();
            return false;
        }
        true
    }
    pub(crate) fn supply(&mut self, frame: u64, reference: &Palette, supplied: Palette) -> bool {
        if !self.accept(frame) || !reference.same_layout(&supplied) {
            self.invalidate();
            return false;
        }
        let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| entry.reference.same_key(reference))
        else {
            self.invalidate();
            return false;
        };
        if entry.reference != *reference
            || entry.pending == 0
            || entry.supplied.as_ref().is_some_and(|old| old != &supplied)
        {
            self.invalidate();
            return false;
        }
        entry.pending -= 1;
        entry.supplied = Some(supplied);
        true
    }
    pub(crate) fn finish(self, frame: u64) -> Frame {
        let complete = !self.invalid
            && self.frame.is_none_or(|observed| observed == frame)
            && self
                .entries
                .iter()
                .all(|entry| entry.pending == 0 && entry.supplied.is_some());
        let mut palettes = if complete {
            self.entries
                .into_iter()
                .filter_map(|entry| entry.supplied)
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        palettes.sort_by(|a, b| {
            (
                &a.slot,
                &a.geometry,
                &a.definition,
                &a.hierarchy,
                &a.group,
                &a.owner,
            )
                .cmp(&(
                    &b.slot,
                    &b.geometry,
                    &b.definition,
                    &b.hierarchy,
                    &b.group,
                    &b.owner,
                ))
        });
        let mut result = Frame {
            schema: 3,
            animation_frame: frame,
            complete,
            coverage_verified: false,
            palettes,
        };
        if !result.valid() {
            result.complete = false;
            result.palettes.clear();
        }
        result
    }
}
