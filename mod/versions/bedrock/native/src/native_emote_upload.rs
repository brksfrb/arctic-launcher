//! Retarget the pinned renderer's owned eight-matrix copy, never a borrowed span.
//! Experimental source implementation; rendered equipment parity is unverified.
use crate::world_hooks::read;
use serde_json::{Value, json};
use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};

static INSTALLED_BINDING: OnceLock<crate::world_hooks::PalettePackBinding> = OnceLock::new();
static ENABLED: AtomicBool = AtomicBool::new(false);
const BYTES: usize = 8 * 64;
type Pack = unsafe extern "system" fn(usize, usize);
static ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static CALLS: AtomicU64 = AtomicU64::new(0);
static ATTEMPTS: AtomicU64 = AtomicU64::new(0);
static COPIES: AtomicU64 = AtomicU64::new(0);
static PREVIOUS_FALLBACK_COPIES: AtomicU64 = AtomicU64::new(0);
static HISTORICAL_COPIES: AtomicU64 = AtomicU64::new(0);
static RECORDED_CURRENT_COPIES: AtomicU64 = AtomicU64::new(0);
static REJECTED: AtomicU64 = AtomicU64::new(0);
static LAST: Mutex<Option<Value>> = Mutex::new(None);

fn word(bytes: &[u8], offset: usize) -> Option<usize> {
    usize::try_from(u64::from_le_bytes(
        bytes.get(offset..offset.checked_add(8)?)?.try_into().ok()?,
    ))
    .ok()
}
fn overlaps(a: usize, a_size: usize, b: usize, b_size: usize) -> bool {
    match (a.checked_add(a_size), b.checked_add(b_size)) {
        (Some(a_end), Some(b_end)) => a < b_end && b < a_end,
        _ => true,
    }
}
fn write(address: usize, bytes: &[u8]) -> bool {
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
    let mut written = 0;
    unsafe {
        WriteProcessMemory(
            GetCurrentProcess(),
            address as _,
            bytes.as_ptr(),
            bytes.len(),
            &mut written,
        ) != 0
            && written == bytes.len()
    }
}
struct Patch {
    destination: usize,
    original: [u8; BYTES],
    attempted: bool,
    committed: bool,
}
impl Drop for Patch {
    fn drop(&mut self) {
        if self.attempted && !self.committed {
            write(self.destination, &self.original);
        }
    }
}
struct Source {
    address: usize,
    header: [u8; 16],
    pointer: usize,
    count: usize,
    bytes: Vec<u8>,
}
impl Source {
    fn copy(address: usize) -> Option<Self> {
        let mut header = [0; 16];
        read(address, &mut header).ok()?;
        let count = word(&header, 0)?;
        let pointer = word(&header, 8)?;
        if !(1..=8).contains(&count) || pointer < 0x10000 {
            return None;
        }
        let mut bytes = vec![0; count.checked_mul(64)?];
        pointer.checked_add(bytes.len())?;
        read(pointer, &mut bytes).ok()?;
        let result = Self {
            address,
            header,
            pointer,
            count,
            bytes,
        };
        result.unchanged()?;
        Some(result)
    }
    fn unchanged(&self) -> Option<()> {
        let mut header = [0; 16];
        let mut bytes = vec![0; self.bytes.len()];
        read(self.address, &mut header).ok()?;
        if header != self.header {
            return None;
        }
        read(self.pointer, &mut bytes).ok()?;
        if bytes != self.bytes {
            return None;
        }
        read(self.address, &mut header).ok()?;
        (header == self.header).then_some(())
    }
    fn matrices(&self) -> Vec<[f32; 16]> {
        self.bytes
            .chunks_exact(64)
            .map(|bytes| {
                std::array::from_fn(|i| {
                    f32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().expect("four bytes"))
                })
            })
            .collect()
    }
    fn padded(&self) -> [u8; BYTES] {
        // Both inspected packers copy count complete 4x4 matrices and pad the
        // remaining entries to eight with exact identity values. Preserve that
        // output layout independently of the selected function address/cookie.
        let mut result = [0; BYTES];
        result[..self.bytes.len()].copy_from_slice(&self.bytes);
        for matrix in self.count..8 {
            for diagonal in [0, 5, 10, 15] {
                let at = matrix * 64 + diagonal * 4;
                result[at..at + 4].copy_from_slice(&1f32.to_le_bytes());
            }
        }
        result
    }
}
fn copied_output(
    source: &Source,
    destination: usize,
    packed: &crate::native_palette_binding::Packed,
) -> Option<[u8; BYTES]> {
    if destination < 0x10000
        || destination.checked_add(BYTES).is_none()
        || overlaps(destination, BYTES, source.address, 16)
        || overlaps(destination, BYTES, source.pointer, source.bytes.len())
        || packed
            .regions
            .iter()
            .any(|region| overlaps(destination, BYTES, region[0], region[1]))
    {
        return None;
    }
    let mut original = [0; BYTES];
    read(destination, &mut original).ok()?;
    if original != source.padded() {
        return None;
    }
    Some(original)
}
fn observe_current(
    source: usize,
    destination: usize,
    tag: &crate::native_submit::Tag,
    stamp: &crate::native_palette_writer::Stamp,
) -> Option<()> {
    if crate::native_palette_binding::span_is_previous(source, tag, stamp.frame)? {
        return Some(());
    }
    let packed = crate::native_palette_binding::packed(source, tag, stamp.frame)?;
    let source = Source::copy(source)?;
    copied_output(&source, destination, &packed)?;
    let matrices = source.matrices();
    source.unchanged()?;
    if crate::native_palette_binding::packed(source.address, tag, stamp.frame).as_ref()
        != Some(&packed)
        || crate::native_transform::palette_writer_scope().as_ref() != Some(stamp)
    {
        return None;
    }
    if !crate::native_transform::begin_attachment_sample(
        stamp,
        tag,
        &packed.named,
        &matrices,
        crate::native_transform::current_emote(tag.scope, &tag.actor).is_some()
            || crate::native_attachment_replay::active(),
    ) {
        return None;
    }
    if !crate::native_transform::record_frame_uploads(
        stamp,
        &packed.named,
        &matrices,
        &matrices,
        None,
        None,
    ) {
        return None;
    }
    RECORDED_CURRENT_COPIES.fetch_add(1, Ordering::Relaxed);
    Some(())
}
fn apply(
    source: usize,
    destination: usize,
    tag: &crate::native_submit::Tag,
    target: Option<&crate::native_emote_pose::Target>,
    stamp: &crate::native_palette_writer::Stamp,
) -> Option<(bool, bool)> {
    let packed = crate::native_palette_binding::packed(source, tag, stamp.frame)?;
    let named = &packed.named;
    let source = Source::copy(source)?;
    let original = copied_output(&source, destination, &packed)?;
    let native = source.matrices();
    if named.bones.len() != source.count {
        return None;
    }
    let replay_revision = crate::native_attachment_replay::revision();
    if replay_revision == 0 {
        return None;
    }
    let replay = crate::native_attachment_replay::query(tag, stamp.frame, named, &native);
    if matches!(&replay, crate::native_attachment_replay::Query::Blocked) {
        return None;
    }
    let replay_target = match &replay {
        crate::native_attachment_replay::Query::Ready(target) => Some(target),
        _ => None,
    };
    let historical = packed.previous.as_ref().is_some_and(|entries| {
        entries
            .iter()
            .any(|entry| matches!(entry, crate::native_palette_writer::Prior::Historical(_)))
    });
    let matrices = if let Some(entries) = packed.previous.as_ref() {
        if entries.len() != native.len() {
            return None;
        }
        entries
            .iter()
            .enumerate()
            .map(|(index, entry)| match entry {
                crate::native_palette_writer::Prior::Current => {
                    if let Some(replay) = replay_target {
                        replay.matrices.get(index).copied()
                    } else {
                        if named.native_cape.is_some()
                            || named.attachment_slot
                                == Some(crate::native_attachment_samples::WING_SLOT)
                        {
                            Some(native[index])
                        } else {
                            match target {
                                Some(target) => crate::native_emote_palette::retarget_entry(
                                    native[index],
                                    target,
                                    &named.bones[index],
                                    named.space.as_ref(),
                                ),
                                None => Some(native[index]),
                            }
                        }
                    }
                }
                crate::native_palette_writer::Prior::Historical(pose) => {
                    if !native[index]
                        .iter()
                        .zip(&pose.native)
                        .all(|(a, b)| a.to_bits() == b.to_bits())
                    {
                        return None;
                    }
                    Some(pose.supplied)
                }
            })
            .collect::<Option<Vec<_>>>()?
    } else if let Some(replay) = replay_target {
        replay.matrices.clone()
    }
    // Root capes and wings retain their own layer transforms. Wings are not
    // children of the emote torso; recorded palettes take precedence above.
    else if named.native_cape.is_some()
        || named.attachment_slot == Some(crate::native_attachment_samples::WING_SLOT)
    {
        native.clone()
    } else {
        crate::native_emote_palette::retarget_current(native.clone(), target?, named)?
    };
    if matrices.len() != source.count {
        return None;
    }
    let mut replacement = original;
    for (index, matrix) in matrices.iter().enumerate() {
        for (element, value) in matrix.iter().enumerate() {
            let at = index * 64 + element * 4;
            replacement[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
    }
    let current = || {
        crate::native_transform::current_emote(tag.scope, &tag.actor).as_ref() == target
            && crate::native_body::enabled()
            && crate::native_attachment_replay::revision() == replay_revision
            && replay_target.is_none_or(|replay| replay.current())
            && crate::native_transform::palette_writer_scope().as_ref() == Some(stamp)
            && crate::native_palette_binding::packed(source.address, tag, stamp.frame).as_ref()
                == Some(&packed)
    };
    source.unchanged()?;
    if !current() {
        return None;
    }
    let mut patch = Patch {
        destination,
        original,
        attempted: false,
        committed: false,
    };
    // Mark the attempt before writing: a failed OS copy may have changed a
    // prefix and must restore the original native output on every failure.
    patch.attempted = true;
    if !write(destination, &replacement) {
        return None;
    }
    let mut after = [0; BYTES];
    read(destination, &mut after).ok()?;
    if after != replacement {
        return None;
    }
    source.unchanged()?;
    if !current() {
        return None;
    }
    // Only native-owned output values changed. No descriptor points at Rust
    // memory and no source palette or source span was modified.
    patch.committed = true;
    if packed.previous.is_none() {
        crate::native_transform::finish_attachment_sample(
            stamp,
            tag,
            named,
            &native,
            &matrices,
            replay_target.map(|replay| replay.identity()),
        );
        let emote = if replay_target.is_none() && named.native_cape.is_none() {
            target.map(|target| crate::native_emote_history::Emote {
                token: target.token.clone(),
                time_ms: target.time_ms,
                sample_time_ms: target.sample_time_ms,
                seconds: target.seconds,
            })
        } else {
            None
        };
        crate::native_transform::record_frame_uploads(
            stamp,
            named,
            &native,
            &matrices,
            emote,
            replay_target.map(|replay| replay.identity()),
        );
    }
    if let Some(replay) = replay_target {
        replay.committed(stamp.frame);
    }
    if let Ok(mut last) = LAST.try_lock() {
        *last = Some(
            json!({"worldScope":tag.scope,"actorId":tag.actor,"geometry":tag.geometry,
            "animationFrame":stamp.frame,"timeMs":target.map(|target|target.time_ms),
            "sampleTimeMs":target.map(|target|target.sample_time_ms),
            "recordedAttachment":replay_target.map(|replay|replay.identity()),
            "hierarchy":named.hierarchy,"matrices":source.count,
            "convertedDrawSpace":named.space.as_ref().is_some_and(|space| !space.identity()),
            "rigBindings":named.bones.iter().map(|bone|json!({"bone":bone.name,"boneIndex":bone.index,
                "part":bone.anchor.as_ref().map(|anchor|anchor.part),
                "anchorBoneIndex":bone.anchor.as_ref().map(|anchor|anchor.bone_index),
                "ancestorDepth":bone.anchor.as_ref().map(|anchor|anchor.depth)})).collect::<Vec<_>>(),
            "outputChanged":replacement!=original,"previousPoseRetargeted":historical,
            "sourceIsPreviousFallback":packed.previous_fallback,
            "sourceIsPrevious":packed.previous.is_some(),
            "previousEntries":packed.previous.as_ref().map(|entries|entries.iter().map(|entry|match entry {
                crate::native_palette_writer::Prior::Current => json!({"source":"currentFallback",
                    "token":target.map(|target|target.token.as_str()),"timeMs":target.map(|target|target.time_ms),
                    "sampleTimeMs":target.map(|target|target.sample_time_ms),
                    "seconds":target.map(|target|target.seconds)}),
                crate::native_palette_writer::Prior::Historical(pose) => json!({"source":"observedCache",
                    "token":pose.emote.as_ref().map(|emote|emote.token.as_str()),
                    "timeMs":pose.emote.as_ref().map(|emote|emote.time_ms),
                    "sampleTimeMs":pose.emote.as_ref().map(|emote|emote.sample_time_ms),
                    "recordedAttachment":pose.replay,
                    "seconds":pose.emote.as_ref().map(|emote|emote.seconds)}),
            }).collect::<Vec<_>>()),
            "nativeEquipmentRenderingVerified":false}),
        );
    }
    Some((packed.previous_fallback, historical))
}
unsafe extern "system" fn pack(source: usize, destination: usize) {
    CALLS.fetch_add(1, Ordering::Relaxed);
    // Keep the exact native copy/padding operation and its error behavior.
    // The wrapper only inspects and changes the completed owned output.
    unsafe {
        let original: Pack = std::mem::transmute(ORIGINAL.load(Ordering::Acquire));
        original(source, destination);
    }
    if !crate::native_body::enabled() {
        return;
    }
    let observed = std::panic::catch_unwind(|| {
        let stamp = crate::native_transform::palette_writer_scope();
        crate::native_transform::invalidate_other_frame_uploads(stamp.as_ref());
        let Some(stamp) = stamp else {
            return;
        };
        let Some(tag) = crate::native_submit::active() else {
            crate::native_transform::invalidate_frame_uploads(&stamp);
            return;
        };
        if stamp.scope != tag.scope || stamp.actor_id != tag.actor {
            crate::native_transform::invalidate_frame_uploads(&stamp);
            return;
        }
        if observe_current(source, destination, &tag, &stamp).is_none() {
            crate::native_transform::invalidate_frame_uploads(&stamp);
        }
        let target = crate::native_transform::current_emote(tag.scope, &tag.actor);
        let packed = crate::native_palette_binding::packed(source, &tag, stamp.frame);
        let recorded = crate::native_attachment_replay::active()
            && packed
                .as_ref()
                .is_some_and(|packed| packed.named.attachment_slot.is_some());
        let historical = packed
            .as_ref()
            .and_then(|packed| packed.previous.as_ref())
            .is_some_and(|entries| {
                entries.iter().any(|entry| {
                    matches!(entry, crate::native_palette_writer::Prior::Historical(_))
                })
            });
        if target.is_none() && !historical && !recorded {
            return;
        }
        ATTEMPTS.fetch_add(1, Ordering::Relaxed);
        match apply(source, destination, &tag, target.as_ref(), &stamp) {
            Some((previous_fallback, historical)) => {
                COPIES.fetch_add(1, Ordering::Relaxed);
                if previous_fallback {
                    PREVIOUS_FALLBACK_COPIES.fetch_add(1, Ordering::Relaxed);
                }
                if historical {
                    HISTORICAL_COPIES.fetch_add(1, Ordering::Relaxed);
                }
            }
            None => {
                if crate::native_palette_binding::span_is_previous(source, &tag, stamp.frame)
                    != Some(true)
                {
                    crate::native_transform::invalidate_frame_uploads(&stamp);
                }
                REJECTED.fetch_add(1, Ordering::Relaxed);
            }
        }
    });
    if observed.is_err() {
        crate::native_transform::invalidate_other_frame_uploads(None);
    }
}
pub(crate) fn install(base: usize) -> Result<(), String> {
    if crate::world_hooks::image_base() != Some(base) {
        return Err("unvalidated emote upload image".into());
    }
    let binding = crate::world_hooks::palette_pack_binding()?;
    let target = base
        .checked_add(binding.rva)
        .ok_or("emote upload address overflow")?;
    let expected = binding.prefix()?;
    let mut actual = [0; 25];
    read(target, &mut actual)?;
    if actual != expected {
        return Err("emote upload prologue differs from exact profile".into());
    }
    INSTALLED_BINDING
        .set(binding)
        .map_err(|_| "emote upload installation already attempted")?;
    use hudhook::mh::{MH_CreateHook, MH_EnableHook, MH_STATUS};
    let mut original = std::ptr::null_mut();
    let status = unsafe { MH_CreateHook(target as _, pack as *const () as _, &mut original) };
    if status != MH_STATUS::MH_OK {
        return Err(format!("create emote upload hook: {status:?}"));
    }
    ORIGINAL.store(original as usize, Ordering::Release);
    let status = unsafe { MH_EnableHook(target as _) };
    if status != MH_STATUS::MH_OK {
        return Err(format!("enable emote upload hook: {status:?}"));
    }
    ENABLED.store(true, Ordering::Release);
    Ok(())
}
pub(crate) fn status() -> Value {
    json!({"installed":ENABLED.load(Ordering::Acquire),"hookRva":INSTALLED_BINDING.get().map(|b|b.rva),
        "experimentalEnabled":crate::native_body::enabled(),"calls":CALLS.load(Ordering::Relaxed),
        "attempts":ATTEMPTS.load(Ordering::Relaxed),"ownedOutputsRetargeted":COPIES.load(Ordering::Relaxed),
        "previousCurrentFallbackOutputs":PREVIOUS_FALLBACK_COPIES.load(Ordering::Relaxed),
        "historicalPreviousOutputs":HISTORICAL_COPIES.load(Ordering::Relaxed),
        "recordedCurrentCopies":RECORDED_CURRENT_COPIES.load(Ordering::Relaxed),
        "currentUploadRecordsActorScoped":true,"currentUploadRecordsSerialized":false,
        "rootAttachmentSamplingImplemented":true,"attachmentSampleCoverageVerified":false,
        "historicalCachePairingImplemented":true,"historicalCacheConsumerImplemented":true,
        "nativeCacheObserver":crate::native_emote_cache::status(),
        "attachmentReplay":crate::native_attachment_replay::status(),
        "paletteWriter":crate::native_palette_writer::status(),
        "rejected":REJECTED.load(Ordering::Relaxed),"last":LAST.try_lock().ok().and_then(|v|v.clone()),
        "writesBorrowedPalettes":false,"lendsReplacementMemory":false,"previousPoseRetargeted":HISTORICAL_COPIES.load(Ordering::Relaxed)!=0,
        "nativeEquipmentRenderingVerified":false,"deterministicVideoVerified":false})
}
