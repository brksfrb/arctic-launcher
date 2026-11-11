//! Java quad sorting and feature-group consolidation over owned geometry.
//! Group/reference/equality IDs are supplied by an owning adapter; texture or
//! material resemblance alone does not prove Java RenderType identity.
use crate::avatar_geometry::{Frame, Pass, PositionSpace};
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};

#[derive(Clone, Copy, Debug)]
pub enum Sorting {
    Keep,
    Distance {
        origin: [f32; 3],
    },
    /// Literal Java -centroid.z; vertices must already use the sorter's space.
    /// A native camera's world-space Z is not inferred to be equivalent.
    OrthographicZ,
}
impl Sorting {
    pub fn perspective(frame: &Frame) -> Result<Self, &'static str> {
        let origin = match frame.space {
            PositionSpace::CameraRelative => [0.; 3],
            PositionSpace::World => frame.camera.map(|value| value as f32),
            PositionSpace::Unknown => {
                return Err("quad sorting requires a declared coordinate space");
            }
        };
        let value = Self::Distance { origin };
        value.validate()?;
        Ok(value)
    }
    fn validate(self) -> Result<(), &'static str> {
        if let Self::Distance { origin } = self
            && !origin.iter().all(|value| value.is_finite())
        {
            return Err("nonfinite quad sort origin");
        }
        Ok(())
    }
    fn same(self, other: Self) -> bool {
        match (self, other) {
            (Self::Keep, Self::Keep) | (Self::OrthographicZ, Self::OrthographicZ) => true,
            (Self::Distance { origin: a }, Self::Distance { origin: b }) => {
                a.map(f32::to_bits) == b.map(f32::to_bits)
            }
            _ => false,
        }
    }
    fn key(self, center: [f32; 3]) -> f32 {
        match self {
            Self::Keep => 0.,
            Self::OrthographicZ => -center[2],
            Self::Distance { origin } => {
                // JOML default non-FMA distanceSquared: origin minus centroid,
                // then dx*dx+(dy*dy+dz*dz). Preserve the float grouping.
                let x = origin[0] - center[0];
                let y = origin[1] - center[1];
                let z = origin[2] - center[2];
                x * x + (y * y + z * z)
            }
        }
    }
}

/// Logical identities within this frame, not retained game pointers. Separate
/// object-reference and PreparedRenderType-equality IDs reproduce Java's two
/// distinct consolidation checks. Every staged batch, including empty ones,
/// must occur exactly once in the supplied feature-group sequence.
pub struct Submit {
    pub batch: usize,
    pub render_type: u64,
    pub prepared_type: u64,
    pub consolidate: bool,
    pub sorting: Sorting,
}
pub struct FeatureGroup {
    pub can_reorder: bool,
    pub submits: Vec<Submit>,
}

/// One already ordered Output.accept/acceptFeatureGroup delivery from Java's
/// phase sorter. feature_type is reference identity supplied by the owner,
/// not a RenderType/prepared identity, texture key, pass number or actor ID.
/// A multi-submit delivery must contain only this feature's submissions.
pub struct PhaseOutput {
    pub feature_type: u64,
    pub strictly_ordered: bool,
    pub submits: Vec<Submit>,
}
/// A distinct phase drain. The producer must supply the real phase order and
/// include every participating feature boundary. owned_avatars consumes the
/// Java producer's own HashMap traversal; foreign features and debug shuffle
/// remain separate inputs. Model sorting needs its owning view-pose inputs.
pub struct Phase {
    pub outputs: Vec<PhaseOutput>,
}
/// Source phase location, not a proof that the engine is currently there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhaseKey {
    pub order: i32,
    pub stage: u8,
}
pub struct OrderedPhase {
    pub key: PhaseKey,
    pub phase: Phase,
}

/// One owned model submit in original emission order. The owning phase caller
/// must supply the actual Java-equivalent submitted view-pose translation;
/// neither a packet root nor a world/camera-relative actor position proves it.
pub struct TranslucentModel {
    pub model: u16,
    pub view_translation: [f32; 3],
    pub submits: Vec<Submit>,
}
pub struct ModelViewPose {
    pub model: u16,
    pub translation: [f32; 3],
}
/// Preparation identities and view poses still come from the owning adapter.
/// Epoch joins the selected source collector; it is not a native phase lease.
pub struct AvatarPhaseInputs {
    pub epoch: u64,
    pub model_feature: u64,
    pub custom_feature: u64,
    /// Exactly one preparation per staged batch, in original storage order.
    pub submits: Vec<Submit>,
    /// Exactly one pose per staged translucent model, in original model order.
    pub model_poses: Vec<ModelViewPose>,
}
impl Phase {
    /// Assemble this source renderer's complete supported avatar scope:
    /// order 0 solid(0), translucent models(7), custom(8), then order 1 glow(8).
    /// Real world insertion must still join other participating features and
    /// scopes rather than treating this private collector as the engine's one.
    pub fn owned_avatars(
        frame: &Frame,
        inputs: AvatarPhaseInputs,
    ) -> Result<Vec<Self>, &'static str> {
        Ok(Self::owned_avatar_ordered(frame, inputs)?
            .into_iter()
            .map(|value| value.phase)
            .collect())
    }
    pub fn owned_avatar_ordered(
        frame: &Frame,
        inputs: AvatarPhaseInputs,
    ) -> Result<Vec<OrderedPhase>, &'static str> {
        let metadata = frame
            .simple_phases
            .as_ref()
            .ok_or("avatar collector phase metadata is unknown")?;
        if inputs.epoch != metadata.epoch {
            return Err("avatar collector epoch changed");
        }
        if inputs.model_feature == inputs.custom_feature {
            return Err("model and custom feature references must differ");
        }
        if inputs.submits.len() != frame.batches.len()
            || !inputs
                .submits
                .iter()
                .enumerate()
                .all(|(index, submit)| submit.batch == index)
        {
            return Err("avatar preparation does not cover original storage order");
        }
        let mut submits: Vec<Option<Submit>> = inputs.submits.into_iter().map(Some).collect();
        let mut simple: [Option<Phase>; 3] = [None, None, None];
        for phase in &metadata.phases {
            let slot = match (phase.order, phase.stage) {
                (0, 0) => 0,
                (0, 8) => 1,
                (1, 8) => 2,
                _ => return Err("unsupported avatar phase"),
            };
            let mut ordered = Vec::with_capacity(phase.batches.len());
            for &index in &phase.batches {
                ordered.push(
                    submits
                        .get_mut(index)
                        .and_then(Option::take)
                        .ok_or("avatar phase repeats/misses a prepared submit")?,
                );
            }
            let feature_type = if phase.stage == 0 {
                inputs.model_feature
            } else {
                inputs.custom_feature
            };
            simple[slot] = Some(Phase {
                outputs: vec![PhaseOutput {
                    feature_type,
                    strictly_ordered: false,
                    submits: ordered,
                }],
            });
        }
        let mut poses = inputs.model_poses.into_iter();
        let mut models: Vec<TranslucentModel> = Vec::new();
        for (index, batch) in frame.batches.iter().enumerate() {
            if batch.pass != Pass::SkinTranslucent {
                continue;
            }
            let id = batch
                .model_submission
                .ok_or("skin model has no submit boundary")?
                .id;
            if !models.last().is_some_and(|model| model.model == id) {
                let pose = poses.next().ok_or("missing translucent model view pose")?;
                if pose.model != id {
                    return Err("translucent view poses change model order");
                }
                models.push(TranslucentModel {
                    model: id,
                    view_translation: pose.translation,
                    submits: Vec::new(),
                });
            }
            let submit = submits[index]
                .take()
                .ok_or("model phase repeats a prepared submit")?;
            models
                .last_mut()
                .ok_or("missing translucent model")?
                .submits
                .push(submit);
        }
        if poses.next().is_some() || submits.iter().any(Option::is_some) {
            return Err("unused avatar pose/preparation input");
        }
        let translucent = Self::translucent_models(frame, inputs.model_feature, models)?;
        let mut phases = Vec::with_capacity(4);
        if let Some(solid) = simple[0].take() {
            phases.push(OrderedPhase {
                key: PhaseKey { order: 0, stage: 0 },
                phase: solid,
            });
        }
        if !translucent.outputs.is_empty() {
            phases.push(OrderedPhase {
                key: PhaseKey { order: 0, stage: 7 },
                phase: translucent,
            });
        }
        if let Some(custom) = simple[1].take() {
            phases.push(OrderedPhase {
                key: PhaseKey { order: 0, stage: 8 },
                phase: custom,
            });
        }
        if let Some(glow) = simple[2].take() {
            phases.push(OrderedPhase {
                key: PhaseKey { order: 1, stage: 8 },
                phase: glow,
            });
        }
        Ok(phases)
    }
    /// TranslucentFeatureRenderPhase for the supported staged skin models.
    /// This returns one phase only. The owner must join foreign submissions,
    /// other numeric orders/phases and preparation/resource ownership itself.
    pub fn translucent_models(
        frame: &Frame,
        feature_type: u64,
        models: Vec<TranslucentModel>,
    ) -> Result<Self, &'static str> {
        if models.len() > 2048 {
            return Err("translucent model budget exceeded");
        }
        let mut expected: Vec<(u16, Vec<usize>)> = Vec::new();
        for (index, batch) in frame.batches.iter().enumerate() {
            if batch.pass != Pass::SkinTranslucent {
                continue;
            }
            let model = batch
                .model_submission
                .ok_or("skin model has no submit boundary")?;
            if expected.last().is_some_and(|(id, _)| *id == model.id) {
                expected.last_mut().unwrap().1.push(index);
            } else {
                expected.push((model.id, vec![index]));
            }
        }
        if models.len() != expected.len() {
            return Err("translucent phase omitted or added a model");
        }
        for (model, (id, batches)) in models.iter().zip(&expected) {
            if model.model != *id
                || model.submits.len() != batches.len()
                || !model
                    .submits
                    .iter()
                    .zip(batches)
                    .all(|(submit, batch)| submit.batch == *batch)
            {
                return Err("translucent model differs from original submission order");
            }
            let first = model.submits.first().ok_or("empty model metadata")?;
            if model.submits.iter().any(|submit| {
                submit.render_type != first.render_type
                    || submit.prepared_type != first.prepared_type
                    || submit.consolidate != first.consolidate
                    || !submit.sorting.same(first.sorting)
            }) {
                return Err("model parts disagree on their single vertex builder");
            }
        }
        let indices = model_order(
            &models
                .iter()
                .map(|model| model.view_translation)
                .collect::<Vec<_>>(),
        )?;
        let mut owned: Vec<Option<TranslucentModel>> = models.into_iter().map(Some).collect();
        let mut outputs = Vec::with_capacity(indices.len());
        for index in indices {
            let model = owned[index].take().ok_or("model sort repeated an index")?;
            outputs.push(PhaseOutput {
                feature_type,
                strictly_ordered: true,
                submits: model.submits,
            });
        }
        Ok(Self { outputs })
    }
}

/// Shared Java translucent-model order, including glint's blended model phase.
/// The caller supplies actual submitted view translations, not actor positions.
pub(crate) fn model_order(translations: &[[f32; 3]]) -> Result<Vec<usize>, &'static str> {
    if translations.len() > 2048 {
        return Err("translucent model budget exceeded");
    }
    let mut distances = Vec::with_capacity(translations.len());
    for &[x, y, z] in translations {
        if ![x, y, z].iter().all(|value| value.is_finite()) {
            return Err("nonfinite model sorting pose");
        }
        // Mth.lengthSquared(FFF), unlike the JOML quad-key grouping above.
        distances.push((x * x + y * y) + z * z);
    }
    let mut indices: Vec<usize> = (0..translations.len()).collect();
    model_unstable_sort(&mut indices, &distances);
    Ok(indices)
}

// Independent port of the cached fastutil 8.5.18 comparator quicksort used
// by IntArrays.unstableSort. Selection-sort cutoff 16, median-of-nine above
// 128, equal-key partitions and range swaps matter: stable sort changes ties.
fn model_unstable_sort(indices: &mut [usize], distances: &[f32]) {
    let compare = |a: usize, b: usize| java_float_compare(distances[b], distances[a]);
    let length = indices.len();
    if length < 16 {
        for i in 0..length.saturating_sub(1) {
            let mut m = i;
            for j in i + 1..length {
                if compare(indices[j], indices[m]) == Ordering::Less {
                    m = j;
                }
            }
            if m != i {
                indices.swap(i, m);
            }
        }
        return;
    }
    let median = |a: usize, b: usize, c: usize| {
        let ab = compare(indices[a], indices[b]);
        let ac = compare(indices[a], indices[c]);
        let bc = compare(indices[b], indices[c]);
        if ab == Ordering::Less {
            if bc == Ordering::Less {
                b
            } else if ac == Ordering::Less {
                c
            } else {
                a
            }
        } else if bc == Ordering::Greater {
            b
        } else if ac == Ordering::Greater {
            c
        } else {
            a
        }
    };
    let (mut left, mut middle, mut right) = (0, length / 2, length - 1);
    if length > 128 {
        let step = length / 8;
        left = median(left, left + step, left + 2 * step);
        middle = median(middle - step, middle, middle + step);
        right = median(right - 2 * step, right - step, right);
    }
    let pivot = indices[median(left, middle, right)];
    // Signed right cursors preserve Java's -1 sentinel without usize wrap.
    let (mut a, mut b, mut c, mut d) = (0isize, 0isize, length as isize - 1, length as isize - 1);
    loop {
        while b <= c {
            let order = compare(indices[b as usize], pivot);
            if order == Ordering::Greater {
                break;
            }
            if order == Ordering::Equal {
                indices.swap(a as usize, b as usize);
                a += 1;
            }
            b += 1;
        }
        while c >= b {
            let order = compare(indices[c as usize], pivot);
            if order == Ordering::Less {
                break;
            }
            if order == Ordering::Equal {
                indices.swap(c as usize, d as usize);
                d -= 1;
            }
            c -= 1;
        }
        if b > c {
            break;
        }
        indices.swap(b as usize, c as usize);
        b += 1;
        c -= 1;
    }
    let n = a.min(b - a) as usize;
    for i in 0..n {
        indices.swap(i, b as usize - n + i);
    }
    let n = (d - c).min(length as isize - d - 1) as usize;
    for i in 0..n {
        indices.swap(b as usize + i, length - n + i);
    }
    let low = (b - a) as usize;
    let high = (d - c) as usize;
    if low > 1 {
        model_unstable_sort(&mut indices[..low], distances);
    }
    if high > 1 {
        model_unstable_sort(&mut indices[length - high..], distances);
    }
}

impl FeatureGroup {
    /// Port PhaseSubmitGrouper.addOrExtendGroup followed by the boolean
    /// inversion in RenderTypeFeatureRenderer.prepareGroup. This accepts
    /// ordered phase output, not raw actor-emission order. Empty collections
    /// are ignored; equal adjacent feature/strictness deliveries coalesce,
    /// including separate texture batches, but never across phase drains.
    pub fn from_phases(phases: Vec<Phase>) -> Result<Vec<Self>, &'static str> {
        if phases.len() > 2048 {
            return Err("feature phase budget exceeded");
        }
        let mut groups: Vec<Self> = Vec::new();
        let mut outputs = 0usize;
        let mut submits = 0usize;
        for phase in phases {
            let mut last: Option<(u64, bool)> = None;
            for mut output in phase.outputs {
                outputs = outputs
                    .checked_add(1)
                    .ok_or("feature output count overflow")?;
                if outputs > 4096 {
                    return Err("feature output budget exceeded");
                }
                submits = submits
                    .checked_add(output.submits.len())
                    .ok_or("feature submit count overflow")?;
                if submits > 2048 {
                    return Err("feature submit budget exceeded");
                }
                // PhaseSubmitGrouper.acceptFeatureGroup returns without
                // touching lastGroup for an empty collection.
                if output.submits.is_empty() {
                    continue;
                }
                let key = (output.feature_type, output.strictly_ordered);
                if last == Some(key) {
                    groups
                        .last_mut()
                        .ok_or("missing feature group")?
                        .submits
                        .append(&mut output.submits);
                } else {
                    if groups.len() >= 2048 {
                        return Err("feature group budget exceeded");
                    }
                    groups.push(Self {
                        can_reorder: !output.strictly_ordered,
                        submits: output.submits,
                    });
                }
                last = Some(key);
            }
        }
        Ok(groups)
    }
}
#[derive(Clone, Copy, Debug)]
pub struct DrawRange {
    pub batch: usize,
    pub first_index: u32,
    pub index_count: u32,
}
pub struct Plan<'frame> {
    frame: &'frame Frame,
    ranges: Vec<DrawRange>,
    group_ranges: Vec<std::ops::Range<usize>>,
}
struct Signature {
    pass: Pass,
    texture: Option<usize>,
    logical_texture: Option<String>,
    sorting: Sorting,
}
impl Signature {
    fn same(&self, other: &Self) -> bool {
        // Both explicit roles use RenderTypes.entityTranslucent. Overlay and
        // tint are per-vertex source attributes represented by each batch's
        // constants; they do not manufacture a different RenderType identity.
        let translucent = |pass| {
            matches!(
                pass,
                Pass::SkinTranslucent | Pass::BentSkin | Pass::CosmeticBase
            )
        };
        let same_pass =
            self.pass == other.pass || translucent(self.pass) && translucent(other.pass);
        same_pass
            && self.texture == other.texture
            && self.logical_texture == other.logical_texture
            && self.sorting.same(other.sorting)
    }
}
struct Object {
    prepared: u64,
    consolidate: bool,
}
struct Quad {
    batch: usize,
    first_index: u32,
    key: f32,
}
struct Draw {
    prepared: u64,
    sorting: Sorting,
    quads: Vec<Quad>,
}

// Java Float.compare, including signed zero and canonicalized NaNs. Rust's
// total_cmp orders negative NaNs differently and is not a substitute here.
fn java_float_compare(a: f32, b: f32) -> Ordering {
    if a < b {
        return Ordering::Less;
    }
    if a > b {
        return Ordering::Greater;
    }
    let bits = |value: f32| {
        if value.is_nan() {
            0x7fc0_0000i32
        } else {
            value.to_bits() as i32
        }
    };
    bits(a).cmp(&bits(b))
}
fn position(bytes: &[u8], at: usize) -> Result<[f32; 3], &'static str> {
    let mut result = [0.; 3];
    for (index, value) in result.iter_mut().enumerate() {
        *value = f32::from_le_bytes(
            bytes
                .get(at + index * 4..at + index * 4 + 4)
                .ok_or("quad position exceeds vertex data")?
                .try_into()
                .map_err(|_| "invalid quad position width")?,
        );
        if !value.is_finite() {
            return Err("nonfinite quad position");
        }
    }
    Ok(result)
}
impl<'frame> Plan<'frame> {
    pub fn new(frame: &'frame Frame, groups: &[FeatureGroup]) -> Result<Self, &'static str> {
        if frame.batches.len() > 2048 || groups.len() > 2048 {
            return Err("quad grouping budget exceeded");
        }
        let mut seen = vec![false; frame.batches.len()];
        let mut submits = 0usize;
        let mut quads = 0usize;
        let mut objects: HashMap<u64, Object> = HashMap::new();
        let mut prepared: HashMap<u64, Signature> = HashMap::new();
        let mut model_first: HashMap<u16, usize> = HashMap::new();
        let mut seen_models = HashSet::new();
        for (index, batch) in frame.batches.iter().enumerate() {
            if let Some(model) = batch.model_submission {
                model_first.entry(model.id).or_insert(index);
            }
        }
        let mut ranges: Vec<DrawRange> = Vec::new();
        let mut group_ranges = Vec::with_capacity(groups.len());
        for group in groups {
            let group_start = ranges.len();
            let mut draws: Vec<Draw> = Vec::new();
            let mut last: Option<(u64, usize)> = None;
            let mut last_model: Option<(u16, usize, usize, u64, u64, bool, Sorting)> = None;
            for submit in &group.submits {
                submits += 1;
                if submits > 2048 {
                    return Err("quad submit budget exceeded");
                }
                let batch = frame
                    .batches
                    .get(submit.batch)
                    .ok_or("quad grouping references a missing batch")?;
                if batch
                    .render_reference
                    .is_some_and(|reference| reference.id != submit.render_type)
                {
                    return Err("quad grouping differs from the Java factory-cache reference");
                }
                if std::mem::replace(&mut seen[submit.batch], true) {
                    return Err("quad grouping repeats a batch");
                }
                submit.sorting.validate()?;
                let signature = Signature {
                    pass: batch.pass,
                    texture: batch.texture,
                    logical_texture: batch.logical_texture.clone(),
                    sorting: submit.sorting,
                };
                if let Some(previous) = prepared.get(&submit.prepared_type) {
                    if !previous.same(&signature) {
                        return Err("prepared render identity has conflicting material/sort state");
                    }
                } else {
                    prepared.insert(submit.prepared_type, signature);
                }
                if let Some(previous) = objects.get(&submit.render_type) {
                    if previous.prepared != submit.prepared_type
                        || previous.consolidate != submit.consolidate
                    {
                        return Err(
                            "render object identity has conflicting preparation/consolidation state",
                        );
                    }
                } else {
                    objects.insert(
                        submit.render_type,
                        Object {
                            prepared: submit.prepared_type,
                            consolidate: submit.consolidate,
                        },
                    );
                }
                let model_draw = if let Some(model) = batch.model_submission {
                    if let Some((_, previous, draw, render, prepared, consolidate, sorting)) =
                        last_model.filter(|last| last.0 == model.id)
                    {
                        if previous + 1 != submit.batch
                            || render != submit.render_type
                            || prepared != submit.prepared_type
                            || consolidate != submit.consolidate
                            || !sorting.same(submit.sorting)
                        {
                            return Err("model parts change order or their single vertex builder");
                        }
                        Some(draw)
                    } else {
                        if !seen_models.insert(model.id)
                            || model_first.get(&model.id) != Some(&submit.batch)
                        {
                            return Err("model submission is split or begins with a later part");
                        }
                        None
                    }
                } else {
                    None
                };
                let consecutive = last
                    .filter(|(object, _)| *object == submit.render_type && submit.consolidate)
                    .map(|(_, index)| index);
                let reused = model_draw.or(consecutive).or_else(|| {
                    if group.can_reorder && submit.consolidate {
                        draws
                            .iter()
                            .position(|draw| draw.prepared == submit.prepared_type)
                    } else {
                        None
                    }
                });
                let draw_index = match reused {
                    Some(index) => index,
                    None => {
                        draws.push(Draw {
                            prepared: submit.prepared_type,
                            sorting: submit.sorting,
                            quads: Vec::new(),
                        });
                        draws.len() - 1
                    }
                };
                last = Some((submit.render_type, draw_index));
                // All part sinks of one submitModel use its one vertex builder,
                // even when that RenderType cannot consolidate *other* submits.
                last_model = batch.model_submission.map(|model| {
                    (
                        model.id,
                        submit.batch,
                        draw_index,
                        submit.render_type,
                        submit.prepared_type,
                        submit.consolidate,
                        submit.sorting,
                    )
                });
                let vertices = frame
                    .attributes(submit.batch)
                    .ok_or("quad batch has no owned attributes")?;
                if !batch.vertex_count.is_multiple_of(4)
                    || batch.vertex_count.checked_mul(40) != Some(vertices.len())
                    || batch.index_count != batch.vertex_count / 4 * 6
                {
                    return Err("quad batch topology disagrees with its attributes");
                }
                quads = quads
                    .checked_add(batch.vertex_count / 4)
                    .ok_or("quad count overflow")?;
                if quads > 100_000 {
                    return Err("quad sort budget exceeded");
                }
                for quad in 0..batch.vertex_count / 4 {
                    let a = position(vertices, quad * 160)?;
                    let c = position(vertices, quad * 160 + 80)?;
                    let center = [(a[0] + c[0]) / 2., (a[1] + c[1]) / 2., (a[2] + c[2]) / 2.];
                    draws[draw_index].quads.push(Quad {
                        batch: submit.batch,
                        first_index: (quad * 6) as u32,
                        key: submit.sorting.key(center),
                    });
                }
            }
            for mut draw in draws {
                if !matches!(draw.sorting, Sorting::Keep) {
                    // Stable sort retains source emission order on equal keys,
                    // matching the cached Java IntArrays.mergeSort comparator.
                    draw.quads.sort_by(|a, b| java_float_compare(b.key, a.key));
                }
                for quad in draw.quads {
                    if let Some(previous) = ranges.last_mut()
                        && previous.batch == quad.batch
                        && previous.first_index + previous.index_count == quad.first_index
                    {
                        previous.index_count += 6;
                        continue;
                    }
                    ranges.push(DrawRange {
                        batch: quad.batch,
                        first_index: quad.first_index,
                        index_count: 6,
                    });
                }
            }
            group_ranges.push(group_start..ranges.len());
        }
        if seen.iter().any(|seen| !*seen) {
            return Err("quad grouping omitted a staged submit");
        }
        Ok(Self {
            frame,
            ranges,
            group_ranges,
        })
    }
    pub fn frame(&self) -> &Frame {
        self.frame
    }
    pub fn ranges(&self) -> &[DrawRange] {
        &self.ranges
    }
    pub fn group_range(&self, index: usize) -> Option<std::ops::Range<usize>> {
        self.group_ranges.get(index).cloned()
    }
}
