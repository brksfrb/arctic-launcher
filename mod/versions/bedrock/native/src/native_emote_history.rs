//! Owned current-upload records for pairing the native historical pose cache.
//! This is actor-scoped input to that pairing, not proof of a rendered frame.
use std::sync::Arc;

const MAX_UPLOADS: usize = 512;
const MAX_BYTES: usize = 256 * 1024;

/// The native cache compares HashedString hash/text, then group and palette slot.
/// These are copied values; no native node, string or matrix address is retained.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Slot {
    pub hash: u64,
    pub name: String,
    pub group: u32,
    pub palette: u32,
}
#[derive(Clone, PartialEq)]
pub(crate) struct Emote {
    pub token: String,
    pub time_ms: u64,
    pub sample_time_ms: f64,
    pub seconds: f32,
}
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Replay {
    pub manifest_hash: String,
    pub time_ms: u32,
    pub time_remainder: u32,
    pub time_denominator: u32,
}
impl Replay {
    pub(crate) fn valid(&self) -> bool {
        self.manifest_hash.len() == 64
            && self
                .manifest_hash
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            && self.time_ms <= 86_400_000
            && (1..=1000).contains(&self.time_denominator)
            && self.time_remainder < self.time_denominator
            && (self.time_ms < 86_400_000 || self.time_remainder == 0)
    }
}
#[derive(Clone, PartialEq)]
pub(crate) struct Pose {
    pub native: [f32; 16],
    pub supplied: [f32; 16],
    pub emote: Option<Emote>,
    pub replay: Option<Replay>,
}
struct Upload {
    // A scalar identity used only inside the current ActorScope. It is never
    // dereferenced, returned in a historical pose, or serialized.
    part: usize,
    owner: crate::native_cache_binding::Identity,
    slot: Arc<Slot>,
    pose: Pose,
}
#[derive(Default)]
pub(crate) struct FrameUploads {
    uploads: Vec<Upload>,
    invalid: bool,
    generation: u64,
}
fn affine(matrix: &[f32; 16]) -> bool {
    matrix
        .iter()
        .all(|v| v.is_finite() && v.abs() <= 1_000_000.)
        && [3, 7, 11].iter().all(|i| matrix[*i].abs() <= 0.0001)
        && (matrix[15] - 1.).abs() <= 0.0001
}
impl FrameUploads {
    /// A new observed native frame retires copies, without clearing a prior
    /// failure that disabled this actor scope.
    pub(crate) fn advance_frame(&mut self) {
        self.uploads.clear();
    }
    pub(crate) fn invalidate(&mut self) {
        self.uploads.clear();
        self.invalid = true;
    }
    pub(crate) fn record(
        &mut self,
        owner: crate::native_cache_binding::Identity,
        part: usize,
        slot: Arc<Slot>,
        pose: Pose,
    ) -> bool {
        if self.invalid {
            return false;
        }
        let generation = crate::native_emote_cache_history::generation();
        if generation == 0 {
            self.invalidate();
            return false;
        }
        if self.generation != generation {
            self.uploads.clear();
            self.generation = generation;
        }
        let valid = part >= 0x10000
            && slot.name.len() <= 128
            && slot.group < 64
            && slot.palette < 256
            && valid_pose(&pose);
        if !valid {
            self.uploads.clear();
            self.invalid = true;
            return false;
        }
        if let Some(old) = self
            .uploads
            .iter_mut()
            .find(|u| u.part == part && u.owner == owner)
        {
            // A later unmodified native copy replaces an earlier emote copy,
            // including when native matrices happened to stay identical.
            *old = Upload {
                owner,
                part,
                slot,
                pose,
            };
        } else if self.uploads.len() < MAX_UPLOADS {
            self.uploads.push(Upload {
                owner,
                part,
                slot,
                pose,
            });
        } else {
            self.uploads.clear();
            self.invalid = true;
            return false;
        }
        let bytes = self.uploads.capacity() * std::mem::size_of::<Upload>()
            + self
                .uploads
                .iter()
                .map(|u| {
                    std::mem::size_of::<Slot>()
                        + 2 * std::mem::size_of::<usize>()
                        + u.slot.name.capacity()
                        + u.pose
                            .emote
                            .as_ref()
                            .map_or(0, |emote| emote.token.capacity())
                        + u.pose
                            .replay
                            .as_ref()
                            .map_or(0, |replay| replay.manifest_hash.capacity())
                })
                .sum::<usize>();
        if bytes > MAX_BYTES {
            self.uploads.clear();
            self.invalid = true;
            return false;
        }
        true
    }
    pub(crate) fn get(
        &self,
        owner: &crate::native_cache_binding::Identity,
        part: usize,
        slot: &Slot,
        native: &[f32; 16],
    ) -> Option<Pose> {
        if self.invalid
            || self.generation == 0
            || self.generation != crate::native_emote_cache_history::generation()
        {
            return None;
        }
        let upload = self
            .uploads
            .iter()
            .find(|u| &u.owner == owner && u.part == part && u.slot.as_ref() == slot)?;
        // Matrix equality alone is not a history identity. The caller also
        // supplies the exact current-scope part and copied native cache key.
        if !upload
            .pose
            .native
            .iter()
            .zip(native)
            .all(|(a, b)| a.to_bits() == b.to_bits())
        {
            return None;
        }
        Some(upload.pose.clone())
    }
}
pub(crate) fn valid_pose(pose: &Pose) -> bool {
    affine(&pose.native)
        && affine(&pose.supplied)
        && !(pose.emote.is_some() && pose.replay.is_some())
        && (pose.emote.is_some()
            || pose.replay.is_some()
            || pose
                .native
                .iter()
                .zip(&pose.supplied)
                .all(|(a, b)| a.to_bits() == b.to_bits()))
        && pose.replay.as_ref().is_none_or(Replay::valid)
        && pose.emote.as_ref().is_none_or(|emote| {
            emote.token.len() == 64
                && emote
                    .token
                    .bytes()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
                && emote.time_ms <= 1_000_000_000_000
                && emote.seconds.is_finite()
                && emote.sample_time_ms.is_finite()
                && emote.sample_time_ms >= emote.time_ms as f64
                && emote.sample_time_ms < (emote.time_ms as f64) + 1.
                && emote.sample_time_ms <= 1_000_000_000_000.
                && (0. ..=86400.).contains(&emote.seconds)
        })
}
