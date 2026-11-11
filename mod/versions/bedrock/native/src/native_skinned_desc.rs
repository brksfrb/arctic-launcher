//! Exact-image skinned draw descriptions, after native model/bone evaluation.
//! Experimental suppression changes only the owned output description.
use crate::native_scene_layout::skinned_description as desc;
use crate::world_hooks::{floats, pointer, read};
use serde_json::{Value, json};
use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};

static INSTALLED_BINDING: OnceLock<crate::world_hooks::SkinnedDescriptionBinding> = OnceLock::new();
static ENABLED: AtomicBool = AtomicBool::new(false);
// Exact zero may be culled before the GPU submission that owns world depth.
// The probe tests a positive sub-cutout value instead; GPU absence is unproven
// until separate native-body/depth comparison checks pass.
const SUPPRESSION_ALPHA: f32 = 0.0001;
// All 21 Windows x64 argument slots and the constructor's native this-return
// are corroborated by cached constructor/caller bytes for both scene families
// (inspection/static-skinned-description-layout-1.26.60.5.json). Output members
// still require the selected exact family. Small stack values retain their
// actual byte/word/dword widths.
type Construct = unsafe extern "system" fn(
    usize,
    usize,
    usize,
    usize,
    usize,
    u32,
    usize,
    usize,
    usize,
    usize,
    usize,
    u8,
    u8,
    u8,
    u8,
    u16,
    usize,
    usize,
    usize,
    u32,
    u8,
) -> usize;
static ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static CALLS: AtomicU64 = AtomicU64::new(0);
static ACTOR_CALLS: AtomicU64 = AtomicU64::new(0);
static MATCHED: AtomicU64 = AtomicU64::new(0);
static SUPPRESSED: AtomicU64 = AtomicU64::new(0);
static STEP: AtomicUsize = AtomicUsize::new(0);
static LAST: Mutex<Option<Value>> = Mutex::new(None);
#[derive(Clone, Copy)]
#[repr(C, align(16))]
struct CollapsedBone([f32; 16]);
static COLLAPSED: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
fn collapsed_palette() -> usize {
    *COLLAPSED.get_or_init(|| {
        // A borrowed span may outlive this callback. This bounded 16 KiB
        // allocation stays valid for the process lifetime, including deferred
        // native consumers. Never transfer its ownership to the engine.
        let mut matrix = [0.; 16];
        matrix[15] = 1.;
        Box::into_raw(Box::new([CollapsedBone(matrix); 256])) as usize
    })
}
struct BonePatch {
    addresses: [usize; 2],
    originals: [[u8; 8]; 2],
    written: [bool; 2],
    committed: bool,
}
impl Drop for BonePatch {
    fn drop(&mut self) {
        if !self.committed {
            for i in 0..2 {
                if self.written[i] {
                    write(self.addresses[i], &self.originals[i]);
                }
            }
        }
    }
}
fn suppress_bones(output: usize, sources: [usize; 2], accept: impl FnOnce() -> bool) -> bool {
    let mut counts = [0u64; 2];
    let mut originals = [[0u8; 8]; 2];
    for i in 0..2 {
        let mut copied = [0u8; 16];
        let mut source = [0u8; 16];
        let Some(address) = output.checked_add(desc::BONE_SPANS[i]) else {
            return false;
        };
        if read(address, &mut copied).is_err()
            || read(sources[i], &mut source).is_err()
            || copied != source
        {
            return false;
        }
        counts[i] = u64::from_le_bytes(copied[..8].try_into().unwrap());
        if counts[i] > 256 || i == 0 && counts[i] == 0 {
            return false;
        }
        originals[i].copy_from_slice(&copied[8..]);
        if counts[i] > 0 && pointer(address + 8).is_none() {
            return false;
        }
    }
    let replacement = collapsed_palette().to_le_bytes();
    let mut patch = BonePatch {
        addresses: [
            output + desc::CURRENT_BONES + 8,
            output + desc::PREVIOUS_BONES + 8,
        ],
        originals,
        written: [false; 2],
        committed: false,
    };
    for (i, count) in counts.into_iter().enumerate() {
        if count == 0 {
            continue;
        }
        // A failed native write can still report a partial copy. Restore the
        // saved pointer on every attempted write until ownership is committed.
        patch.written[i] = true;
        if !write(patch.addresses[i], &replacement) {
            return false;
        }
        let mut check = [0; 8];
        if read(patch.addresses[i], &mut check).is_err() || check != replacement {
            return false;
        }
    }
    if !accept() {
        return false;
    }
    patch.committed = true;
    true
}
pub(crate) fn write(address: usize, bytes: &[u8]) -> bool {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut core::ffi::c_void;
        fn WriteProcessMemory(
            process: *mut core::ffi::c_void,
            address: *mut core::ffi::c_void,
            buffer: *const u8,
            size: usize,
            written: *mut usize,
        ) -> i32;
    }
    let mut count = 0;
    unsafe {
        WriteProcessMemory(
            GetCurrentProcess(),
            address as _,
            bytes.as_ptr(),
            bytes.len(),
            &mut count,
        ) != 0
            && count == bytes.len()
    }
}
struct AlphaPatch {
    address: usize,
    original: [u8; 4],
    committed: bool,
}
impl Drop for AlphaPatch {
    fn drop(&mut self) {
        if !self.committed {
            write(self.address, &self.original);
        }
    }
}
fn suppress_alpha(address: usize, target: f32, accept: impl FnOnce() -> bool) -> bool {
    if !target.is_finite() || !(0. ..=0.01).contains(&target) {
        return false;
    }
    let mut original = [0; 4];
    if read(address, &mut original).is_err() {
        return false;
    }
    let alpha = f32::from_le_bytes(original);
    if !alpha.is_finite() || alpha <= target || alpha > 1. {
        return false;
    }
    let mut patch = AlphaPatch {
        address,
        original,
        committed: false,
    };
    if !write(address, &target.to_le_bytes()) {
        return false;
    }
    let mut check = [0; 4];
    if read(address, &mut check).is_err() || check != target.to_le_bytes() || !accept() {
        return false;
    }
    patch.committed = true;
    true
}
// Each inspected constructor copies its last byte argument to its family's
// DITHERING member, then returns this. Both replay fields belong to this output. Defer
// changes until after the original constructor, so a rejected plan need not
// leave a substituted constructor argument behind.
const DITHERING_OFFSET: usize = desc::DITHERING;
struct ColorPatch {
    address: usize,
    original: [u8; 16],
    flag_address: usize,
    original_flag: [u8; 1],
    attempted: [bool; 2],
    committed: bool,
}
impl Drop for ColorPatch {
    fn drop(&mut self) {
        if !self.committed {
            if self.attempted[1] {
                write(self.flag_address, &self.original_flag);
            }
            if self.attempted[0] {
                write(self.address, &self.original);
            }
        }
    }
}
fn boolean_flag(value: u8) -> Option<bool> {
    (value <= 1).then_some(value != 0)
}
fn overlaps(a: usize, bytes: usize, b: usize, other: usize) -> bool {
    match (a.checked_add(bytes), b.checked_add(other)) {
        (Some(end), Some(other_end)) => a < other_end && b < end,
        _ => true,
    }
}
// Safety: output and its strong MaterialPtr belong to the active native
// constructor/submission callback, including synchronous material acquisition.
unsafe fn replay_color(
    output: usize,
    color: usize,
    world: usize,
    sources: [usize; 2],
    original: [f32; 4],
    source_flag: u8,
    shader_inputs: Option<&crate::native_shader_inputs::ActorInputs>,
    tag: &crate::native_submit::Tag,
    target: &crate::native_attachment_replay::ColorTarget,
) -> Option<([f32; 4], Option<bool>)> {
    let address = output.checked_add(desc::COLOR)?;
    let flag_address = output.checked_add(DITHERING_OFFSET)?;
    // The copied output must not alias original color, root or input spans.
    if overlaps(address, 16, color, 16)
        || overlaps(address, 16, world, 64)
        || sources
            .iter()
            .any(|source| overlaps(address, 16, *source, 16))
        || overlaps(flag_address, 1, color, 16)
        || overlaps(flag_address, 1, world, 64)
        || sources
            .iter()
            .any(|source| overlaps(flag_address, 1, *source, 16))
    {
        return None;
    }
    let rgba = target.rgba();
    let mut before = [0u8; 16];
    let mut source = [0u8; 16];
    let mut before_flag = [0u8; 1];
    read(flag_address, &mut before_flag).ok()?;
    read(address, &mut before).ok()?;
    read(color, &mut source).ok()?;
    let expected: [u8; 16] = std::array::from_fn(|i| original[i / 4].to_le_bytes()[i % 4]);
    if before != expected
        || source != expected
        || before_flag != [source_flag]
        || !target.current()
        || !target.can_prepare_shader_inputs(shader_inputs)
    {
        return None;
    }
    let source_dithering = boolean_flag(source_flag);
    let requested = target.dithering();
    // Unknown native bytes are copied through; a requested boolean cannot
    // reinterpret one. Legacy plans without a flag may still preview RGBA.
    if requested.is_some() && source_dithering.is_none() {
        return None;
    }
    let replacement_flag = [requested.map(u8::from).unwrap_or(source_flag)];
    let dithering_enabled = boolean_flag(replacement_flag[0]);
    let replacement: [u8; 16] = std::array::from_fn(|i| rgba[i / 4].to_le_bytes()[i % 4]);
    let mut patch = ColorPatch {
        address,
        original: before,
        flag_address,
        original_flag: before_flag,
        attempted: [false; 2],
        committed: false,
    };
    // Mark before each call: a failed native write may have copied partially.
    if replacement != before {
        patch.attempted[0] = true;
        if !write(address, &replacement) {
            return None;
        }
    }
    if replacement_flag != before_flag {
        patch.attempted[1] = true;
        if !write(flag_address, &replacement_flag) {
            return None;
        }
    }
    let mut after = [0; 16];
    read(address, &mut after).ok()?;
    read(color, &mut source).ok()?;
    let mut after_flag = [0u8; 1];
    read(flag_address, &mut after_flag).ok()?;
    if let Some(before) = shader_inputs {
        let after = unsafe { crate::native_shader_inputs::actor_description(tag, output) };
        if !after.is_some_and(|after| before.same(&after)) {
            return None;
        }
    }
    if after != replacement
        || after_flag != replacement_flag
        || source != expected
        || !target.current()
        || !target.applied(
            original,
            source_dithering,
            dithering_enabled,
            shader_inputs.cloned(),
        )
    {
        return None;
    }
    if target.shader_inputs().is_some() {
        let ready = unsafe {
            crate::native_shader_consumer::prepare(
                output,
                tag,
                target.clone(),
                shader_inputs?,
                replacement_flag[0],
            )
        };
        if !ready {
            return None;
        }
    }
    patch.committed = true;
    Some((rgba, dithering_enabled))
}
fn raw_layer(tag: &crate::native_submit::Tag) -> bool {
    tag.replacement.is_none()
        && match &tag.role {
            crate::native_submit::MeshRole::Cape {
                native_cape: Some(_),
                ..
            } => true,
            crate::native_submit::MeshRole::Elytra { .. } => tag.geometry == "geometry.elytra",
            _ => false,
        }
}
#[allow(clippy::too_many_arguments)]
unsafe fn observe(
    output: usize,
    result: usize,
    world: usize,
    mesh: usize,
    count: u32,
    color: usize,
    source_bones: [usize; 2],
    tag: crate::native_submit::Tag,
    source_flag: u8,
) -> Option<()> {
    STEP.store(1, Ordering::Relaxed);
    let mut actual_count = [0; 4];
    read(output.checked_add(desc::DRAW_COUNT)?, &mut actual_count).ok()?;
    let mesh_matches = pointer(output.checked_add(desc::MESH)?)? == mesh;
    let copied_count = u32::from_le_bytes(actual_count);
    if let Ok(mut last) = LAST.lock() {
        *last = Some(json!({"worldScope":tag.scope,"actorId":tag.actor,
        "resultMatches":result==output,"meshMatches":mesh_matches,"vertices":count,
        "copiedVertices":copied_count,"bodySuppressed":false,"capeSuppressed":false,"alphaSuppressed":false}));
    }
    // This is an opaque native draw count. It never sizes a read/allocation or
    // changes a submission, so preserve and compare every u32 value exactly.
    if result != output || !mesh_matches || copied_count != count {
        return None;
    }
    // Prove the constructor copied the actual input byte before observing or
    // changing it. Argument values alone cannot certify output field state.
    let mut copied_flag = [0u8; 1];
    read(output.checked_add(DITHERING_OFFSET)?, &mut copied_flag).ok()?;
    if copied_flag != [source_flag] {
        return None;
    }
    let source_dithering = boolean_flag(source_flag);
    STEP.store(2, Ordering::Relaxed);
    let actual = floats::<16>(output, 1_000_000.)?;
    let source = floats::<16>(world, 1_000_000.)?;
    STEP.store(3, Ordering::Relaxed);
    let expected = crate::native_transform::current_draw(tag.scope, &tag.actor)?;
    let expected_matrix = crate::native_transform::palette_writer_scope()
        .filter(|stamp| stamp.scope == tag.scope && stamp.actor_id == tag.actor)
        .and_then(|stamp| crate::native_palette_binding::draw_matrix(&tag, stamp.frame))
        .unwrap_or(expected.matrix);
    STEP.store(4, Ordering::Relaxed);
    let matches =
        |a: &[f32; 16], b: &[f32; 16]| a.iter().zip(b).all(|(a, b)| (a - b).abs() <= 0.0001);
    let root_matches = matches(&actual, &source) && matches(&actual, &expected_matrix);
    let tint = floats::<4>(output.checked_add(desc::COLOR)?, 100.)?;
    let source_tint = floats::<4>(color, 100.)?;
    let tint_matches = tint == source_tint;
    STEP.store(5, Ordering::Relaxed);
    let mut described = false;
    let shader_inputs = if root_matches && tint_matches {
        unsafe { crate::native_shader_inputs::actor_description(&tag, output) }
    } else {
        None
    };
    if root_matches && tint_matches {
        MATCHED.fetch_add(1, Ordering::Relaxed);
        crate::native_transform::record_draw_color(
            &tag,
            tint,
            source_dithering,
            source_dithering,
            shader_inputs.clone(),
        );
        // A native, unchanged upload also needs a receipt when no current
        // emote is active. This does not authorize an unnamed/numeric match.
        if let Some(stamp) = crate::native_transform::palette_writer_scope()
            && stamp.scope == tag.scope
            && stamp.actor_id == tag.actor
        {
            described = crate::native_palette_binding::attest_description(
                output,
                source_bones,
                &tag,
                stamp.frame,
            );
        }
        // Copy the original native spans before experimental suppression can
        // redirect their description pointers to the collapsed palette.
        crate::native_emote_palette::observe(output, source_bones, &tag);
    }
    let is_raw_layer = raw_layer(&tag);
    if is_raw_layer && !described {
        crate::native_transform::invalidate_attachment_samples(&tag);
        crate::native_transform::invalidate_layer_draw_color(&tag);
    }
    let replayed_color = if root_matches && tint_matches && is_raw_layer && described {
        crate::native_transform::palette_writer_scope()
            .filter(|stamp| stamp.scope == tag.scope && stamp.actor_id == tag.actor)
            .and_then(|stamp| {
                crate::native_attachment_replay::layer_color(&tag, stamp.frame, source_bones)
            })
            .and_then(|target| unsafe {
                replay_color(
                    output,
                    color,
                    world,
                    source_bones,
                    tint,
                    source_flag,
                    shader_inputs.as_ref(),
                    &tag,
                    &target,
                )
            })
    } else {
        None
    };
    // One successful description must not stand in for a later failed or
    // unplanned layer description in the same actor's frame.
    if is_raw_layer && crate::native_attachment_replay::active() && replayed_color.is_none() {
        crate::native_transform::invalidate_replay_layer_color(&tag);
    }
    let target = tag
        .replacement
        .as_ref()
        .map_or(SUPPRESSION_ALPHA, |b| b.alpha());
    let method = tag
        .replacement
        .as_ref()
        .map_or("none", |b| b.method())
        .to_owned();
    let cape_draw = matches!(
        tag.replacement.as_ref(),
        Some(
            crate::native_submit::Replacement::Cape(_)
                | crate::native_submit::Replacement::HideCape(_)
        )
    );
    let hidden_cape_draw = matches!(
        tag.replacement.as_ref(),
        Some(crate::native_submit::Replacement::HideCape(_))
    );
    let elytra_draw = matches!(
        tag.replacement.as_ref(),
        Some(crate::native_submit::Replacement::Elytra(_))
    );
    let suppressed = if root_matches && tint_matches {
        tag.replacement
            .is_some_and(|replacement| match replacement {
                crate::native_submit::Replacement::Body(body)
                    if body.suppression_method == "bones" =>
                {
                    suppress_bones(output, source_bones, || {
                        crate::native_body::record_suppressed(body)
                    })
                }
                crate::native_submit::Replacement::Body(body) => {
                    suppress_alpha(output + desc::ALPHA, target, || {
                        crate::native_body::record_suppressed(body)
                    })
                }
                crate::native_submit::Replacement::Cape(cape) => {
                    suppress_bones(output, source_bones, || {
                        crate::native_capes::record_suppressed(cape)
                    })
                }
                crate::native_submit::Replacement::HideCape(cape) => {
                    suppress_bones(output, source_bones, || {
                        crate::native_capes::record_hidden(cape)
                    })
                }
                crate::native_submit::Replacement::Elytra(elytra) => {
                    suppress_bones(output, source_bones, || {
                        crate::native_elytra::record_suppressed(elytra)
                    })
                }
            })
    } else {
        false
    };
    if suppressed {
        SUPPRESSED.fetch_add(1, Ordering::Relaxed);
    }
    let (output_color, dithering_enabled) = replayed_color.unwrap_or((tint, source_dithering));
    let sample = json!({"worldScope":tag.scope,"actorId":tag.actor,"geometry":tag.geometry,
        "vertices":count,"worldMatrix":actual,"nativeRootMatches":root_matches,
        "color":tint,"sourceColorMatches":tint_matches,"bodySuppressed":suppressed&&!cape_draw&&!elytra_draw,
        "capeSuppressed":suppressed&&cape_draw,
        "capeHidden":suppressed&&hidden_cape_draw,
        "elytraSuppressed":suppressed&&elytra_draw,
        "alphaSuppressed":suppressed&&method=="alpha",
        "replayCapeColorApplied":replayed_color.is_some()&&matches!(&tag.role,crate::native_submit::MeshRole::Cape {..}),
        "replayWingColorApplied":replayed_color.is_some()&&matches!(&tag.role,crate::native_submit::MeshRole::Elytra {..}),"outputColor":output_color,
        "sourceDithering":source_dithering,"ditheringEnabled":dithering_enabled,
        "suppressionMethod":method,
        "outputAlpha":if suppressed&&method=="alpha"{target}else{output_color[3]}});
    if let Ok(mut last) = LAST.lock() {
        *last = Some(sample);
    }
    STEP.store(6, Ordering::Relaxed);
    Some(())
}
fn bone_span(output: usize, source: usize) -> Option<Value> {
    let mut copied = [0u8; 16];
    let mut original = [0u8; 16];
    read(output, &mut copied).ok()?;
    read(source, &mut original).ok()?;
    let count = u64::from_le_bytes(copied[..8].try_into().ok()?);
    let first = if (1..=256).contains(&count) {
        pointer(output.checked_add(8)?).and_then(|p| floats::<16>(p, 1_000_000.))
    } else {
        None
    };
    Some(json!({"count":count,"sourceMatches":copied==original,"firstMatrix":first}))
}
#[allow(clippy::too_many_arguments)]
unsafe extern "system" fn construct(
    a: usize,
    b: usize,
    c: usize,
    d: usize,
    e: usize,
    f: u32,
    g: usize,
    h: usize,
    i: usize,
    j: usize,
    k: usize,
    l: u8,
    m: u8,
    n: u8,
    o: u8,
    p: u16,
    q: usize,
    r: usize,
    s: usize,
    t: u32,
    u: u8,
) -> usize {
    CALLS.fetch_add(1, Ordering::Relaxed);
    crate::native_shader_consumer::reset_description();
    let tag = crate::native_submit::active();
    if let Some(tag) = tag.as_ref() {
        ACTOR_CALLS.fetch_add(1, Ordering::Relaxed);
        // A new description must pass its own root/tint checks. A previous
        // description sharing the palette cannot authorize a later draw.
        crate::native_palette_binding::reset_receipts(tag);
    }
    let original: Construct = unsafe { std::mem::transmute(ORIGINAL.load(Ordering::Acquire)) };
    // Forward every argument unchanged. Replay modifies only checked owned
    // output fields after construction, under one rollback guard.
    let result = unsafe {
        original(
            a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p, q, r, s, t, u,
        )
    };
    if let Some(tag) = tag {
        let observed = std::panic::catch_unwind(|| {
            // The original constructor returned this caller-owned description;
            // its MaterialPtr remains owned throughout this synchronous scope.
            unsafe { observe(a, result, b, e, f, i, [j, k], tag.clone(), u) }?;
            let spans = [
                bone_span(a.checked_add(desc::CURRENT_BONES)?, j),
                bone_span(a.checked_add(desc::PREVIOUS_BONES)?, k),
            ];
            if let Ok(mut last) = LAST.lock()
                && let Some(last) = last.as_mut()
            {
                last["boneSpans"] = json!(spans);
            }
            Some(())
        })
        .ok()
        .flatten()
        .is_some();
        if !observed && raw_layer(&tag) {
            crate::native_transform::invalidate_attachment_samples(&tag);
            crate::native_transform::invalidate_layer_draw_color(&tag);
            if crate::native_attachment_replay::active() {
                crate::native_transform::invalidate_replay_layer_color(&tag);
            }
        }
    }
    result
}
pub(crate) fn install(base: usize) -> Result<(), String> {
    if crate::world_hooks::image_base() != Some(base) {
        return Err("skinned description image unavailable".into());
    }
    let binding = crate::world_hooks::skinned_description_binding()?;
    let target = base
        .checked_add(binding.rva)
        .ok_or("skinned description address overflow")?;
    let expected = binding.frame.prefix();
    let mut actual = vec![0; expected.len()];
    read(target, &mut actual)?;
    if actual.as_slice() != expected {
        return Err("skinned description prologue differs".into());
    }
    crate::native_frame_material::install_skinned(base)?;
    INSTALLED_BINDING
        .set(binding)
        .map_err(|_| "skinned description installation already attempted")?;
    // A rejected constructor binding must not activate any of its dependent
    // material/palette/cape hooks as a side effect of checking this entry.
    crate::native_cape_movement::install(base)?;
    crate::native_cape_phase::install(base)?;
    crate::native_shader_consumer::install(base)?;
    crate::native_palette_writer::install(base)?;
    crate::native_emote_upload::install(base)?;
    crate::native_emote_cache::install(base)?;
    use hudhook::mh::{MH_CreateHook, MH_EnableHook, MH_STATUS};
    let mut original = std::ptr::null_mut();
    let status = unsafe { MH_CreateHook(target as _, construct as *const () as _, &mut original) };
    if status != MH_STATUS::MH_OK {
        return Err(format!("create skinned description hook: {status:?}"));
    }
    ORIGINAL.store(original as usize, Ordering::Release);
    let status = unsafe { MH_EnableHook(target as _) };
    if status != MH_STATUS::MH_OK {
        return Err(format!("enable skinned description hook: {status:?}"));
    }
    ENABLED.store(true, Ordering::Release);
    Ok(())
}
pub(crate) fn status() -> Value {
    json!({"installed":ENABLED.load(Ordering::Acquire),"hookRva":INSTALLED_BINDING.get().map(|b|b.rva),
        "calls":CALLS.load(Ordering::Relaxed),"actorCalls":ACTOR_CALLS.load(Ordering::Relaxed),
        "nativeRootMatches":MATCHED.load(Ordering::Relaxed),"suppressed":SUPPRESSED.load(Ordering::Relaxed),
        "step":STEP.load(Ordering::Relaxed),
        "last":LAST.lock().ok().and_then(|v|v.clone()),
        "emotePalette":crate::native_emote_palette::status(),"materialInputs":crate::native_shader_consumer::status(),"bodyReplacementVerified":false})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn borrowed_bone_overrides_have_stable_aligned_storage_and_roll_back_without_mutating_sources()
    {
        let mut description = vec![0u8; 0xe0];
        let matrix = [1f32; 16];
        let source = [1usize, matrix.as_ptr() as usize];
        let bytes = unsafe { std::slice::from_raw_parts(source.as_ptr() as *const u8, 16) };
        description[0xc0..0xd0].copy_from_slice(bytes);
        description[0xd0..0xe0].copy_from_slice(bytes);
        let output = description.as_ptr() as usize;
        let sources = [source.as_ptr() as usize; 2];
        assert!(!suppress_bones(output, sources, || false));
        assert_eq!(&description[0xc0..0xd0], bytes);
        assert!(
            std::panic::catch_unwind(|| suppress_bones(output, sources, || panic!("reject owner")))
                .is_err()
        );
        assert_eq!(&description[0xd0..0xe0], bytes);
        assert!(suppress_bones(output, sources, || true));
        let palette = pointer(output + 0xc8).unwrap();
        assert_eq!(palette, collapsed_palette());
        assert_eq!(palette % 16, 0);
        let value = floats::<16>(palette, 1.).unwrap();
        assert_eq!(value[15], 1.);
        assert!(value[..15].iter().all(|v| *v == 0.));
        assert_eq!(pointer(output + 0xd8), Some(palette));
        assert_eq!(source, [1, matrix.as_ptr() as usize]);
        assert_eq!(matrix, [1.; 16]);
        assert!(!suppress_bones(output, sources, || true));
    }
    #[test]
    fn alpha_is_restored_if_ownership_is_rejected_or_unwinds() {
        let mut alpha = 1f32;
        let address = &mut alpha as *mut f32 as usize;
        assert!(!suppress_alpha(address, SUPPRESSION_ALPHA, || false));
        assert_eq!(alpha, 1.);
        assert!(
            std::panic::catch_unwind(|| suppress_alpha(address, SUPPRESSION_ALPHA, || panic!(
                "ownership rejected"
            )))
            .is_err()
        );
        assert_eq!(alpha, 1.);
        assert!(suppress_alpha(address, SUPPRESSION_ALPHA, || true));
        assert_eq!(alpha, SUPPRESSION_ALPHA);
        assert!(!suppress_alpha(address, SUPPRESSION_ALPHA, || true));
        let captured: Value =
            serde_json::from_str(include_str!("../inspection/skinned-description.json")).unwrap();
        let native_alpha = captured["observation"]["color"][3].as_f64().unwrap() as f32;
        assert_eq!(native_alpha.to_bits(), 0.65f32.to_bits());
        alpha = native_alpha;
        std::hint::black_box(&alpha);
        assert!(!suppress_alpha(address, SUPPRESSION_ALPHA, || false));
        assert_eq!(alpha.to_bits(), 0.65f32.to_bits());
        assert!(suppress_alpha(address, SUPPRESSION_ALPHA, || true));
        assert_eq!(alpha, SUPPRESSION_ALPHA);
        for target in [f32::NAN, -0.1, 0.1] {
            assert!(!suppress_alpha(address, target, || true));
        }
    }
    #[test]
    fn constructor_detour_forwards_all_21_arguments_and_the_native_this_return() {
        static ARGS: Mutex<Vec<usize>> = Mutex::new(Vec::new());
        #[allow(clippy::too_many_arguments)]
        #[inline(never)]
        unsafe extern "system" fn original(
            a: usize,
            b: usize,
            c: usize,
            d: usize,
            e: usize,
            f: u32,
            g: usize,
            h: usize,
            i: usize,
            j: usize,
            k: usize,
            l: u8,
            m: u8,
            n: u8,
            o: u8,
            p: u16,
            q: usize,
            r: usize,
            s: usize,
            t: u32,
            u: u8,
        ) -> usize {
            *ARGS.lock().unwrap() = vec![
                a, b, c, d, e, f as usize, g, h, i, j, k, l as usize, m as usize, n as usize,
                o as usize, p as usize, q, r, s, t as usize, u as usize,
            ];
            a
        }
        use hudhook::mh::{MH_CreateHook, MH_DisableHook, MH_EnableHook, MH_Initialize, MH_STATUS};
        let status = unsafe { MH_Initialize() };
        assert!(matches!(
            status,
            MH_STATUS::MH_OK | MH_STATUS::MH_ERROR_ALREADY_INITIALIZED
        ));
        let target = original as *const () as usize;
        let mut trampoline = std::ptr::null_mut();
        assert_eq!(
            unsafe { MH_CreateHook(target as _, construct as *const () as _, &mut trampoline) },
            MH_STATUS::MH_OK
        );
        ORIGINAL.store(trampoline as usize, Ordering::Release);
        assert_eq!(unsafe { MH_EnableHook(target as _) }, MH_STATUS::MH_OK);
        let call: Construct = std::hint::black_box(original);
        let result = unsafe {
            call(
                1, 2, 3, 4, 5, 0x87654321, 7, 8, 9, 10, 11, 0x81, 0x82, 0x83, 0x84, 0xabcd, 17, 18,
                19, 0xabcdef12, 0xff,
            )
        };
        assert_eq!(unsafe { MH_DisableHook(target as _) }, MH_STATUS::MH_OK);
        ORIGINAL.store(target, Ordering::Release);
        assert_eq!(result, 1);
        assert_eq!(
            *ARGS.lock().unwrap(),
            vec![
                1, 2, 3, 4, 5, 0x87654321, 7, 8, 9, 10, 11, 0x81, 0x82, 0x83, 0x84, 0xabcd, 17, 18,
                19, 0xabcdef12, 0xff
            ]
        );
    }
}
