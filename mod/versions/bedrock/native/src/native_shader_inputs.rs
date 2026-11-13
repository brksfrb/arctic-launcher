//! Read-only, named CPU shader-constant copies from checked native contexts.
//! These are not a GPU submission or rendered-frame ownership proof.
use crate::native_scene_layout as scene;
use crate::world_hooks::{pointer, read};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

// Ordered archive layout. No matrices, camera values, resource handles or
// addresses belong here. The separate native snapshot contains additional
// fields whose meanings are deliberately not inferred.
// ActorConstants getter at 0x5958320 supplies names and primitive tags;
// 0x5959eda places its fields at globals+c0. The native deferred snapshot
// at 0x12ec803..0x12eca08 copies these slots through constant+0x40.
const ACTOR_FIELDS: &[(usize, &str, u8, usize)] = &[
    (0xd0, "OVERLAY_COLOR", 4, 4),
    (0xd8, "TILE_LIGHT_COLOR", 4, 4),
    (0xe0, "TILE_LIGHT_INTENSITY", 2, 2),
    (0xe8, "BLOCK_LIGHT_COLOR", 4, 4),
    (0xf0, "CHANGE_COLOR", 4, 4),
    (0xf8, "GLINT_COLOR", 4, 4),
    (0x100, "UV_ANIM", 4, 4),
    (0x110, "UV_OFFSET", 2, 2),
    (0x118, "UV_ROTATION", 2, 2),
    (0x120, "GLINT_UV_SCALE", 2, 2),
];
// The skinned visitor at 0x5e4f7cd..0x5e4f937 copies these additional
// named values into the same local material snapshot. Actor getter store
// 0x59588fd identifies globals+108; the others are PerFrame/HUD constants.
const CONTEXT_FIELDS: &[(usize, &str, u8, usize)] = &[
    (0x108, "MULTIPLICATIVE_TINT_CHANGE_COLOR", 4, 4),
    (0x70, "FOG_COLOR", 4, 4),
    (0x78, "FOG_CONTROL", 2, 2),
    (0x80, "RENDER_DISTANCE", 1, 1),
    (0x150, "HUD_OPACITY", 1, 1),
];
/// Fingerprint of declared path metadata, not shader assets or compiled code.
#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct MaterialPaths {
    pub sha256: String,
    pub stages: [bool; 3],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variation_sha256: Option<String>,
}
impl MaterialPaths {
    fn valid(&self) -> bool {
        fn hash(value: &str) -> bool {
            value.len() == 64
                && value
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        }
        hash(&self.sha256)
            && self
                .variation_sha256
                .as_ref()
                .is_none_or(|value| hash(value))
    }
    /// Legacy metadata may omit variation, but a recorded known variation
    /// cannot match an unknown or different live define/state cohort.
    pub(crate) fn matches(&self, actual: &Self) -> bool {
        self.sha256 == actual.sha256
            && self.stages == actual.stages
            && self
                .variation_sha256
                .as_ref()
                .is_none_or(|value| actual.variation_sha256.as_ref() == Some(value))
    }
}
/// Named CPU inputs in one selected layer's draw scope. These values do not
/// prove that a particular shader used them or that a GPU draw completed.
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ActorInputs {
    pub values: [f32; 32],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material_context: Option<[f32; 12]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material_paths: Option<MaterialPaths>,
}
impl ActorInputs {
    pub(crate) fn valid(&self) -> bool {
        self.values
            .iter()
            .chain(self.material_context.iter().flatten())
            .all(|v| v.is_finite() && v.abs() <= 1_000_000_000.)
            && self
                .material_paths
                .as_ref()
                .is_none_or(MaterialPaths::valid)
    }
    pub(crate) fn same(&self, other: &Self) -> bool {
        self.values.map(f32::to_bits) == other.values.map(f32::to_bits)
            && self.material_context.map(|v| v.map(f32::to_bits))
                == other.material_context.map(|v| v.map(f32::to_bits))
            && self.material_paths == other.material_paths
    }
}
fn material_context(globals: usize) -> Option<([usize; 5], [f32; 12])> {
    let mut slots = [0; 5];
    let mut values = [0f32; 12];
    let mut at = 0;
    for (index, &(offset, name, primitive, count)) in CONTEXT_FIELDS.iter().enumerate() {
        let slot = pointer(globals.checked_add(offset)?)?;
        let value = constant(slot, name, primitive, count)?;
        slots[index] = slot;
        values[at..at + count].copy_from_slice(&value);
        at += count;
    }
    (at == values.len()).then_some((slots, values))
}
impl PartialEq for ActorInputs {
    fn eq(&self, other: &Self) -> bool {
        self.same(other)
    }
}

// PerFrameConstants::getShaderConstants at 0x595ce10 validates these names,
// primitive tags and field slots. World matrices use tag 11 (0x595a0d2).
// Native setters at 0x595ae59 and 0x47a9336 dereference constant+0x40 for data.
const FIELDS: &[(usize, &str, u8, usize)] = &[
    (0x30, "WORLDVIEWPROJ", 11, 16),
    (0x38, "WORLD", 11, 16),
    (0x40, "WORLDVIEW", 11, 16),
    (0x48, "PROJ", 11, 16),
    (0x60, "TIME", 1, 1),
    (0x68, "VIEW_POS", 3, 3),
    (0x70, "FOG_COLOR", 4, 4),
    (0x78, "FOG_CONTROL", 2, 2),
    (0x80, "RENDER_DISTANCE", 1, 1),
    (0x88, "FAR_CHUNKS_DISTANCE", 1, 1),
    (0x90, "OCCLUSION_HEIGHT_OFFSET", 1, 1),
    (0x138, "CURRENT_COLOR", 4, 4),
    (0x140, "DARKEN", 4, 4),
    (0x148, "TEXTURE_DIMENSIONS", 3, 3),
    (0x150, "HUD_OPACITY", 1, 1),
    (0x238, "TOTAL_REAL_WORLD_TIME", 1, 1),
];
fn constant(address: usize, name: &str, primitive: u8, count: usize) -> Option<Vec<f32>> {
    if !(1..=16).contains(&count) {
        return None;
    }
    let mut header = [0; 0x48];
    read(address, &mut header).ok()?;
    let name_header: &[u8; 32] = header[8..40].try_into().ok()?;
    if crate::players::string_header(name_header, 64)? != name
        // Installed run e1a36c74516e40d0a9f97f96118e4bcf verified zero for
        // every named non-array constant. Do not interpret it as an empty value.
        || u32::from_le_bytes(header[40..44].try_into().ok()?) != 0
        || header[48] != primitive
    {
        return None;
    }
    let values = raw_values(&header, count)?;
    let mut after = [0; 0x48];
    read(address, &mut after).ok()?;
    (header == after).then_some(values)
}
fn raw_values(header: &[u8; 0x48], count: usize) -> Option<Vec<f32>> {
    if !(1..=16).contains(&count) {
        return None;
    }
    let data = usize::try_from(u64::from_le_bytes(header[64..72].try_into().ok()?)).ok()?;
    let mut bytes = vec![0; count * 4];
    read(data, &mut bytes).ok()?;
    let values: Vec<f32> = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|v| f32::from_le_bytes(*v))
        .collect();
    if values
        .iter()
        .any(|v| !v.is_finite() || v.abs() > 1_000_000_000.)
    {
        return None;
    }
    Some(values)
}
fn rejected(address: Option<usize>, expected: &str, primitive: u8, count: usize) -> Value {
    let Some(address) = address else {
        return json!({"reason":"missing slot"});
    };
    let mut header = [0; 0x48];
    if read(address, &mut header).is_err() {
        return json!({"reason":"unreadable header"});
    }
    let name = crate::players::string_header(header[8..40].try_into().unwrap(), 64);
    let candidate = (name.as_deref() == Some(expected) && header[48] == primitive)
        .then(|| raw_values(&header, count))
        .flatten();
    json!({"observedName":name,"primitive":header[48],
        "declaredElementCount":u32::from_le_bytes(header[40..44].try_into().unwrap()),
        "firstElementCandidate":candidate,"accepted":false})
}
/// Copy a complete cohort while the selected layer's checked constructor is
/// active. Re-read every slot and its values after collecting the whole cohort;
/// a stable header alone cannot detect an in-place constant update.
pub(crate) fn actor(tag: &crate::native_submit::Tag) -> Option<ActorInputs> {
    use crate::native_submit::MeshRole;
    let frame = match &tag.role {
        MeshRole::Cape {
            animation_frame,
            native_cape: Some(_),
        }
        | MeshRole::Elytra { animation_frame } => *animation_frame,
        _ => return None,
    };
    let stamp = crate::native_transform::palette_writer_scope()?;
    if stamp.scope != tag.scope || stamp.actor_id != tag.actor || stamp.frame != frame {
        return None;
    }
    let screen = pointer(stamp.context.checked_add(0x28)?)?;
    let globals = pointer(screen.checked_add(scene::screen::GLOBAL_CONSTANTS)?)?;
    let mut values = [0.; 32];
    let mut slots = [0usize; 10];
    let mut at = 0;
    for (index, &(offset, name, primitive, count)) in ACTOR_FIELDS.iter().enumerate() {
        let address = pointer(globals.checked_add(offset)?)?;
        let value = constant(address, name, primitive, count)?;
        values.get_mut(at..at + count)?.copy_from_slice(&value);
        slots[index] = address;
        at += count;
    }
    if at != values.len() {
        return None;
    }
    let context = material_context(globals);
    at = 0;
    for (index, &(offset, name, primitive, count)) in ACTOR_FIELDS.iter().enumerate() {
        if pointer(globals.checked_add(offset)?)? != slots[index] {
            return None;
        }
        let again = constant(slots[index], name, primitive, count)?;
        if !values[at..at + count]
            .iter()
            .zip(&again)
            .all(|(a, b)| a.to_bits() == b.to_bits())
        {
            return None;
        }
        at += count;
    }
    let context_after = material_context(globals);
    if crate::native_transform::palette_writer_scope().as_ref() != Some(&stamp)
        || pointer(stamp.context.checked_add(0x28)?)? != screen
        || pointer(screen.checked_add(0x20)?)? != globals
    {
        return None;
    }
    if context
        .as_ref()
        .map(|(slots, values)| (*slots, values.map(f32::to_bits)))
        != context_after
            .as_ref()
            .map(|(slots, values)| (*slots, values.map(f32::to_bits)))
    {
        return None;
    }
    let result = ActorInputs {
        values,
        material_context: context.map(|(_, values)| values),
        material_paths: None,
    };
    result.valid().then_some(result)
}
/// Bind optional declared-path metadata to the same description as the copied
/// constants. A missing reader leaves metadata unknown, never synthesized.
/// # Safety
/// The active native callback owns description and its MaterialPtr during this
/// synchronous copy. No pointer from a packet/archive can meet that contract.
pub(crate) unsafe fn actor_description(
    tag: &crate::native_submit::Tag,
    description: usize,
) -> Option<ActorInputs> {
    let mut result = actor(tag)?;
    if let Some(material) =
        unsafe { crate::native_frame_material::SkinnedMaterial::copy(description) }
    {
        if !actor(tag).is_some_and(|after| result.same(&after)) || !material.current() {
            return None;
        }
        result.material_paths = Some(material.paths());
    }
    Some(result)
}
pub(crate) fn sample(screen: usize) -> Option<Value> {
    // The exact constructor stores clock at +b0; makeScreenContext supplies
    // GameRenderer+1f0. render_sample revalidates the controlled primary client.
    let clock = pointer(screen.checked_add(scene::screen::CLOCK)?)?;
    let renderer = clock.checked_sub(scene::game_renderer::CLOCK)?;
    let (session, seconds, _) = crate::native_clock::render_sample(renderer)?;
    let globals = pointer(screen.checked_add(0x20)?)?;
    // renderCurrentFrame also reads this exact global-buffer reference at +290.
    if pointer(renderer.checked_add(scene::game_renderer::GLOBAL_CONSTANTS)?)? != globals {
        return None;
    }
    let mut values = serde_json::Map::new();
    let mut missing = Vec::new();
    let mut rejected_slots = serde_json::Map::new();
    for &(offset, name, primitive, count) in FIELDS.iter().chain(ACTOR_FIELDS) {
        let address = globals.checked_add(offset).and_then(pointer);
        let value = address.and_then(|address| constant(address, name, primitive, count));
        match value {
            Some(value) => {
                values.insert(name.into(), json!(value));
            }
            None => {
                missing.push(name);
                rejected_slots.insert(name.into(), rejected(address, name, primitive, count));
            }
        }
    }
    if pointer(screen.checked_add(scene::screen::CLOCK)?)? != clock
        || pointer(screen.checked_add(scene::screen::GLOBAL_CONSTANTS)?)? != globals
        || !crate::native_clock::render_sample(renderer)
            .is_some_and(|(s, t, _)| s == session && t.to_bits() == seconds.to_bits())
    {
        return None;
    }
    Some(
        json!({"session":session,"seconds":seconds,"values":values,"missing":missing,"rejected":rejected_slots,
        "observerOnly":true,"gpuSubmissionVerified":false}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn named(name: &[u8], primitive: u8, values: &[f32]) -> (Vec<u8>, Vec<f32>) {
        assert!(name.len() <= 15);
        let data = values.to_vec();
        let mut header = vec![0; 0x48];
        header[8..8 + name.len()].copy_from_slice(name);
        header[24..32].copy_from_slice(&(name.len() as u64).to_le_bytes());
        header[32..40].copy_from_slice(&15u64.to_le_bytes());
        header[40..44].copy_from_slice(&0u32.to_le_bytes());
        header[48] = primitive;
        header[64..72].copy_from_slice(&(data.as_ptr() as u64).to_le_bytes());
        (header, data)
    }
    #[test]
    fn rejected_shape_reports_candidates_without_accepting_an_unknown_element_count() {
        let (mut header, _data) = named(b"TIME", 1, &[9.5]);
        let at = header.as_ptr() as usize;
        header[40..44].copy_from_slice(&1u32.to_le_bytes());
        std::hint::black_box(&header);
        assert!(constant(at, "TIME", 1, 1).is_none());
        let detail = rejected(Some(at), "TIME", 1, 1);
        assert_eq!(detail["declaredElementCount"], 1);
        assert_eq!(detail["firstElementCandidate"], json!([9.5]));
        assert_eq!(detail["accepted"], false);
        assert!(rejected(Some(at), "VIEW_POS", 3, 3)["firstElementCandidate"].is_null());
    }
    #[test]
    fn constant_copies_named_typed_values_without_changing_native_data() {
        let (header, data) = named(b"VIEW_POS", 3, &[1.25, -2., 3.]);
        let before = header.clone();
        assert_eq!(
            constant(header.as_ptr() as usize, "VIEW_POS", 3, 3),
            Some(data.clone())
        );
        assert_eq!(header, before);
        assert_eq!(data, [1.25, -2., 3.]);
        assert!(constant(header.as_ptr() as usize, "FOG_COLOR", 3, 3).is_none());
        assert!(constant(header.as_ptr() as usize, "VIEW_POS", 4, 3).is_none());
    }
    #[test]
    fn constant_rejects_arrays_invalid_counts_nonfinite_and_unreadable_data() {
        let (mut header, _data) = named(b"TIME", 1, &[f32::NAN]);
        let at = header.as_ptr() as usize;
        assert!(constant(at, "TIME", 1, 1).is_none());
        assert!(constant(at, "TIME", 1, 17).is_none());
        header[40..44].copy_from_slice(&2u32.to_le_bytes());
        std::hint::black_box(&header);
        assert!(constant(at, "TIME", 1, 1).is_none());
        header[40..44].copy_from_slice(&0u32.to_le_bytes());
        header[64..72].fill(0);
        std::hint::black_box(&header);
        assert!(constant(at, "TIME", 1, 1).is_none());
    }
}
