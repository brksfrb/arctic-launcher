//! Owned palette copies and emote retarget candidates at the native draw boundary.
//! A numeric matrix match is diagnostic evidence, not an equipment bone binding.
//! Nothing in this module changes a native description, mesh, or borrowed span.
use crate::native_scene_layout::skinned_description as desc;
use crate::world_hooks::read;
use serde_json::{Value, json};
use std::sync::{
    Mutex,
    atomic::{AtomicU64, Ordering},
};

const MAX_BONES: usize = 256;
type Matrix = [f32; 16];
static ATTEMPTS: AtomicU64 = AtomicU64::new(0);
static COPIES: AtomicU64 = AtomicU64::new(0);
static REJECTED: AtomicU64 = AtomicU64::new(0);
static LAST: Mutex<Option<Observation>> = Mutex::new(None);

// These addresses exist only during a synchronous copy. Observation and every
// retained palette contain owned values; none retain a foreign pointer.
struct BorrowedSpan {
    output: usize,
    source: usize,
    header: [u8; 16],
    data: usize,
    bytes: Vec<u8>,
}
impl BorrowedSpan {
    fn copy(output: usize, source: usize) -> Option<Self> {
        let mut header = [0; 16];
        let mut original = [0; 16];
        read(output, &mut header).ok()?;
        read(source, &mut original).ok()?;
        if header != original {
            return None;
        }
        let count = usize::try_from(u64::from_le_bytes(header[..8].try_into().ok()?)).ok()?;
        let data = usize::try_from(u64::from_le_bytes(header[8..].try_into().ok()?)).ok()?;
        if count > MAX_BONES {
            return None;
        }
        let length = count.checked_mul(64)?;
        if count != 0 && (data < 0x10000 || data.checked_add(length).is_none()) {
            return None;
        }
        let mut bytes = vec![0; length];
        if length != 0 {
            read(data, &mut bytes).ok()?;
        }
        Some(Self {
            output,
            source,
            header,
            data,
            bytes,
        })
    }
    fn unchanged(&self) -> Option<()> {
        let mut output = [0; 16];
        let mut source = [0; 16];
        read(self.output, &mut output).ok()?;
        read(self.source, &mut source).ok()?;
        if output != self.header || source != self.header {
            return None;
        }
        // Headers alone cannot detect an in-place animation update. Compare the
        // bounded matrix bytes too, before retaining this snapshot.
        if !self.bytes.is_empty() {
            let mut again = vec![0; self.bytes.len()];
            read(self.data, &mut again).ok()?;
            if again != self.bytes {
                return None;
            }
        }
        read(self.output, &mut output).ok()?;
        read(self.source, &mut source).ok()?;
        (output == self.header && source == self.header).then_some(())
    }
    fn matrices(&self) -> Option<Vec<Matrix>> {
        self.bytes
            .chunks_exact(64)
            .map(|bytes| {
                let matrix = std::array::from_fn(|i| {
                    f32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().expect("four bytes"))
                });
                affine(&matrix).then_some(matrix)
            })
            .collect()
    }
}
struct Palette {
    current: Vec<Matrix>,
    previous: Vec<Matrix>,
}
impl Palette {
    fn copy(description: usize, sources: [usize; 2]) -> Option<Self> {
        let current =
            BorrowedSpan::copy(description.checked_add(desc::CURRENT_BONES)?, sources[0])?;
        let previous =
            BorrowedSpan::copy(description.checked_add(desc::PREVIOUS_BONES)?, sources[1])?;
        if current.bytes.is_empty() {
            return None;
        }
        let owned = Self {
            current: current.matrices()?,
            previous: previous.matrices()?,
        };
        current.unchanged()?;
        previous.unchanged()?;
        Some(owned)
    }
    fn bytes(&self) -> usize {
        (self.current.capacity() + self.previous.capacity()) * std::mem::size_of::<Matrix>()
    }
}
fn affine(matrix: &Matrix) -> bool {
    matrix
        .iter()
        .all(|v| v.is_finite() && v.abs() <= 1_000_000.)
        && [3, 7, 11].iter().all(|i| matrix[*i].abs() <= 0.0001)
        && (matrix[15] - 1.).abs() <= 0.0001
}
pub(crate) fn native_matrix(part: &[f32; 12]) -> Option<Matrix> {
    // The shared rig uses row-major 3x4 matrices in blocks. The observed
    // BoneOrientation matrix uses column-major 4x4 matrices in pixels.
    let mut result = [0.; 16];
    result[15] = 1.;
    for row in 0..3 {
        for column in 0..3 {
            result[column * 4 + row] = part[row * 4 + column];
        }
        result[12 + row] = part[row * 4 + 3] * 16.;
    }
    affine(&result).then_some(result)
}
fn multiply(a: &Matrix, b: &Matrix) -> Option<Matrix> {
    let result = std::array::from_fn(|index| {
        let row = index % 4;
        let column = index / 4;
        (0..4).map(|k| a[k * 4 + row] * b[column * 4 + k]).sum()
    });
    affine(&result).then_some(result)
}
fn rigid_inverse(matrix: &Matrix) -> Option<Matrix> {
    if !affine(matrix) {
        return None;
    }
    for a in 0..3 {
        for b in 0..3 {
            let dot: f32 = (0..3).map(|r| matrix[a * 4 + r] * matrix[b * 4 + r]).sum();
            if (dot - if a == b { 1. } else { 0. }).abs() > 0.001 {
                return None;
            }
        }
    }
    let determinant = matrix[0] * (matrix[5] * matrix[10] - matrix[9] * matrix[6])
        - matrix[4] * (matrix[1] * matrix[10] - matrix[9] * matrix[2])
        + matrix[8] * (matrix[1] * matrix[6] - matrix[5] * matrix[2]);
    if (determinant - 1.).abs() > 0.001 {
        return None;
    }
    let mut result = [0.; 16];
    result[15] = 1.;
    for column in 0..3 {
        for row in 0..3 {
            result[column * 4 + row] = matrix[row * 4 + column];
        }
    }
    for row in 0..3 {
        result[12 + row] = -(0..3)
            .map(|k| result[k * 4 + row] * matrix[12 + k])
            .sum::<f32>();
    }
    affine(&result).then_some(result)
}
fn attachment(
    reference: &Matrix,
    desired: &Matrix,
    original: &Matrix,
    space: Option<&crate::native_emote_space::Space>,
) -> Option<Matrix> {
    // Preserve the native attachment's local offset and scale: T * R^-1 * A.
    // The reference/target are in root entity/pixel space. A proved child
    // binding supplies its observed coordinate change. The caller requires an
    // indexed bone binding and exact palette/native-bone agreement.
    if desired == reference {
        return Some(*original);
    }
    if let Some(space) = space.filter(|space| !space.identity()) {
        let delta = multiply(desired, &rigid_inverse(reference)?)?;
        return space.retarget(&delta, original);
    }
    if original == reference {
        return Some(*desired);
    }
    multiply(&multiply(desired, &rigid_inverse(reference)?)?, original)
}
struct Candidate {
    // Current palette only: a previous palette requires its own historical
    // target and clock. Reusing the current target would invent motion data.
    current: Option<Vec<Matrix>>,
    matched: usize,
    unknown: usize,
    ambiguous: usize,
    named_hierarchy: Option<u32>,
    named_entries: usize,
    non_direct: usize,
    inherited: usize,
}
fn candidate(
    palette: &Palette,
    target: &crate::native_emote_pose::Target,
    named: Option<&crate::native_palette_binding::Named>,
) -> Option<Candidate> {
    let reference: Vec<Matrix> = target
        .reference_parts
        .iter()
        .map(native_matrix)
        .collect::<Option<_>>()?;
    let desired: Vec<Matrix> = target
        .parts
        .iter()
        .map(native_matrix)
        .collect::<Option<_>>()?;
    if named.is_some_and(|n| n.bones.len() != palette.current.len()) {
        return None;
    }
    let mut result = Vec::with_capacity(palette.current.len());
    let mut matched = 0;
    let mut unknown = 0;
    let mut ambiguous = 0;
    let mut non_direct = 0;
    let mut inherited = 0;
    for (index, matrix) in palette.current.iter().enumerate() {
        if let Some(named) = named {
            let bone = &named.bones[index];
            if matrix != &bone.matrix {
                // An inverse-bind/deformation palette cannot be treated as a
                // direct bone-to-entity palette. Numeric fallback is invalid.
                non_direct += 1;
                unknown += 1;
                continue;
            }
            if let Some(anchor) = bone.anchor.as_ref() {
                let part = usize::from(anchor.part);
                result.push(attachment(
                    reference.get(part)?,
                    desired.get(part)?,
                    matrix,
                    named.space.as_ref(),
                )?);
                matched += 1;
                if anchor.depth != 0 {
                    inherited += 1;
                }
            } else {
                unknown += 1;
            }
            continue;
        }
        // Require exact element agreement with one copied native part. Never
        // assume six entries imply rig order, match only by translation, or
        // assign the torso to every identity matrix in a standing pose.
        let mut parts = reference
            .iter()
            .enumerate()
            .filter(|(_, part)| *part == matrix);
        match (parts.next(), parts.next()) {
            (Some((index, _)), None) => {
                result.push(desired[index]);
                matched += 1;
            }
            (None, _) => {
                unknown += 1;
            }
            _ => {
                ambiguous += 1;
            }
        }
    }
    // This is deliberately limited to direct bone-to-entity matrices. An
    // inverse-bind palette or unattached subtree needs a separately proven
    // mapping, even if one entry happens to match numerically.
    Some(Candidate {
        current: (unknown == 0 && ambiguous == 0).then_some(result),
        matched,
        unknown,
        ambiguous,
        named_hierarchy: named.map(|n| n.hierarchy),
        named_entries: named.map_or(0, |n| n.bones.len()),
        non_direct,
        inherited,
    })
}
pub(crate) fn retarget_current(
    current: Vec<[f32; 16]>,
    target: &crate::native_emote_pose::Target,
    named: &crate::native_palette_binding::Named,
) -> Option<Vec<[f32; 16]>> {
    if current.is_empty() || current.len() > 8 || !current.iter().all(affine) {
        return None;
    }
    candidate(
        &Palette {
            current,
            previous: Vec::new(),
        },
        target,
        Some(named),
    )?
    .current
}
pub(crate) fn retarget_entry(
    current: [f32; 16],
    target: &crate::native_emote_pose::Target,
    bone: &crate::native_palette_binding::Bone,
    space: Option<&crate::native_emote_space::Space>,
) -> Option<[f32; 16]> {
    if !affine(&current) || current != bone.matrix {
        return None;
    }
    let part = usize::from(bone.anchor.as_ref()?.part);
    let reference = native_matrix(target.reference_parts.get(part)?)?;
    let desired = native_matrix(target.parts.get(part)?)?;
    attachment(&reference, &desired, &current, space)
}
struct Observation {
    scope: u64,
    actor: String,
    geometry: String,
    target: crate::native_emote_pose::Target,
    palette: Palette,
    candidate: Candidate,
}
pub(crate) fn observe(description: usize, sources: [usize; 2], tag: &crate::native_submit::Tag) {
    if !crate::native_body::enabled() {
        return;
    }
    let Some(target) = crate::native_transform::current_emote(tag.scope, &tag.actor) else {
        return;
    };
    ATTEMPTS.fetch_add(1, Ordering::Relaxed);
    let copied = (|| {
        let named = crate::native_palette_binding::current(sources, tag, target.animation_frame);
        let palette = Palette::copy(description, sources)?;
        let candidate = candidate(&palette, &target, named.as_ref())?;
        // Actor scope, animation frame, token, time and baseline must still
        // agree after copying. Another actor's last observed pose is invalid.
        if crate::native_transform::current_emote(tag.scope, &tag.actor).as_ref() != Some(&target) {
            return None;
        }
        if crate::native_palette_binding::current(sources, tag, target.animation_frame) != named {
            return None;
        }
        if named.is_some() && candidate.current.is_some() {
            // This observer is called only after root/tint agreement. A later
            // native copy can use this scoped receipt, never a numeric guess.
            crate::native_palette_binding::attest(sources, tag, target.animation_frame);
        }
        Some(Observation {
            scope: tag.scope,
            actor: tag.actor.clone(),
            geometry: tag.geometry.clone(),
            target,
            palette,
            candidate,
        })
    })();
    match copied {
        Some(value) => {
            COPIES.fetch_add(1, Ordering::Relaxed);
            // Retain just the latest owned observation, not a per-draw or
            // per-frame allocation history. Nothing is lent back to Bedrock.
            if let Ok(mut last) = LAST.try_lock() {
                *last = Some(value);
            }
        }
        None => {
            REJECTED.fetch_add(1, Ordering::Relaxed);
        }
    }
}
pub(crate) fn status() -> Value {
    let last = LAST.try_lock().ok().and_then(|last| last.as_ref().map(|o| {
        json!({"worldScope":o.scope,"actorId":o.actor,"geometry":o.geometry,
            "animationFrame":o.target.animation_frame,"timeMs":o.target.time_ms,
            "sampleTimeMs":o.target.sample_time_ms,
            "currentBones":o.palette.current.len(),"previousBones":o.palette.previous.len(),
            "uniqueDirectMatches":o.candidate.matched,"unknownEntries":o.candidate.unknown,
            "ambiguousEntries":o.candidate.ambiguous,
            "namedHierarchy":o.candidate.named_hierarchy,"indexedBoneEntries":o.candidate.named_entries,
            "nonDirectEntries":o.candidate.non_direct,
            "inheritedRigEntries":o.candidate.inherited,
            "bindingSource":if o.candidate.named_hierarchy.is_some(){"native-part-indices"}else{"numeric-candidates-only"},
            "completeCurrentCandidate":o.candidate.current.is_some(),
            "ownedMatrixBytes":o.palette.bytes()+o.candidate.current.as_ref()
                .map_or(0,|p|p.capacity()*std::mem::size_of::<Matrix>()),
            "diagnosticOnly":true,"engineBoneBindingVerified":false,
            "previousTargetAvailable":false,"nativeEquipmentPoseApplied":false})
    }));
    json!({"attempts":ATTEMPTS.load(Ordering::Relaxed),"copies":COPIES.load(Ordering::Relaxed),
        "rejected":REJECTED.load(Ordering::Relaxed),"last":last,
        "ownedUpload":crate::native_emote_upload::status(),
        "nativeEquipmentPoseApplied":false,"paletteLifetimeVerified":false})
}
