//! Exact-build pass-through engine observers, joined to checked client/player
//! data and matching GPU depth. Bone-space and renderer suppression remain
//! unverified; the limited producer is not a complete cosmetics replacement.
//! Signatures and callback ABIs: Latite 9f746351 (GPL-3.0), see THIRD_PARTY.md.
use crate::native_scene_layout as scene;
use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::atomic::{AtomicI32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::native_hash::{Sha256, hex};
use hudhook::mh::{MH_CreateHook, MH_EnableHook, MH_STATUS};
use hudhook::windows::Win32::System::Diagnostics::Debug::ReadProcessMemory;
use hudhook::windows::Win32::System::LibraryLoader::GetModuleHandleW;
use hudhook::windows::Win32::System::Threading::GetCurrentProcess;
use serde::Serialize;
use serde_json::{Value, json};

const PROFILE: &str = include_str!("../inspection/1.26.5203.0.json");
#[path = "native_engine_entry_profile.rs"]
mod engine_entry_profile;
#[path = "native_transform_profile.rs"]
mod transform_profile;
pub(crate) use transform_profile::{TransformBinding, TransformObservation, transform_binding};
#[path = "native_model_body_profile.rs"]
mod model_body_profile;
pub(crate) use model_body_profile::{ModelBodyBinding, model_body_binding};
const MAX_ACTORS: usize = 128;
type WorldFn = unsafe extern "system" fn(usize, usize, usize);
type TickFn = unsafe extern "system" fn(usize);
type ActorFn = unsafe extern "system" fn(usize, usize, usize, usize, usize, usize, u8);
static WORLD: AtomicUsize = AtomicUsize::new(0);
static ACTOR: AtomicUsize = AtomicUsize::new(0);
static TICK: AtomicUsize = AtomicUsize::new(0);
static TICKS: AtomicU64 = AtomicU64::new(0);
#[derive(Default)]
struct ClientClock {
    level: usize,
    ticks: u64,
    elapsed_ms: f64,
    step_ms: f64,
}
impl ClientClock {
    fn select(&mut self, level: usize, tps: f32) -> Option<(u64, f64)> {
        if level == 0 || !tps.is_finite() || !(1.0..=1000.0).contains(&tps) {
            return None;
        }
        if self.level != level {
            self.level = level;
            self.ticks = 0;
            self.elapsed_ms = 0.;
        }
        self.step_ms = 1000. / tps as f64;
        Some((self.ticks, self.elapsed_ms))
    }
    fn tick(&mut self, level: usize) -> bool {
        if level == 0 || self.level != level {
            return false;
        }
        self.ticks = self.ticks.saturating_add(1);
        self.elapsed_ms += self.step_ms;
        true
    }
}
static CLIENT_CLOCK: Mutex<ClientClock> = Mutex::new(ClientClock {
    level: 0,
    ticks: 0,
    elapsed_ms: 0.,
    step_ms: 0.,
});
pub(crate) fn client_completed_tick(level: usize) -> Option<u64> {
    let clock = CLIENT_CLOCK.lock().ok()?;
    (level != 0 && clock.level == level).then_some(clock.ticks)
}
pub(crate) fn completed_tick_clock(level: usize, tick: u64) -> Option<f64> {
    let clock = CLIENT_CLOCK.lock().ok()?;
    (level != 0
        && clock.level == level
        && clock.ticks == tick
        && clock.elapsed_ms.is_finite()
        && clock.elapsed_ms >= 0.)
        .then_some(clock.elapsed_ms)
}
pub(crate) fn client_sample_seconds(level: usize, partial: f32, tps: f32) -> Option<f64> {
    if !partial.is_finite()
        || !(0. ..1.).contains(&partial)
        || !tps.is_finite()
        || !(1. ..=1000.).contains(&tps)
    {
        return None;
    }
    let clock = CLIENT_CLOCK.lock().ok()?;
    if clock.level != level || (clock.step_ms - 1000. / f64::from(tps)).abs() > 0.00001 {
        return None;
    }
    let seconds = (clock.elapsed_ms + f64::from(partial) * clock.step_ms) / 1000.;
    (seconds.is_finite() && (0. ..=1_000_000_000.).contains(&seconds)).then_some(seconds)
}
static PASSES: AtomicU64 = AtomicU64::new(0);
static NEXT_PASS: AtomicU64 = AtomicU64::new(0);
static ACTORS: AtomicU64 = AtomicU64::new(0);
static PLATFORM_GLOBAL: AtomicUsize = AtomicUsize::new(0);
static ACTIVE_HOOKS: AtomicUsize = AtomicUsize::new(0);
#[path = "world_history.rs"]
mod history;
static HISTORY: OnceLock<Mutex<history::History>> = OnceLock::new();
static CODE_IMAGE: OnceLock<(usize, usize, usize)> = OnceLock::new();
pub(crate) fn image_base() -> Option<usize> {
    CODE_IMAGE.get().map(|image| image.0)
}

#[derive(Clone, Copy)]
pub(crate) enum TimerAdvanceAbi {
    TimerFloat,
    TimerFloatSingleTick,
}
#[derive(Clone, Copy)]
pub(crate) struct TimerBinding {
    pub(crate) rva: usize,
    pub(crate) abi: TimerAdvanceAbi,
}
/// Resolve only from the profile whose full image hash and layout family have
/// already passed. Candidate inspection reports are never read as profiles.
fn exact_function_candidate(
    name: &str,
    prefix_bytes: usize,
) -> Result<(usize, Value, usize), String> {
    let &(_, start, end) = CODE_IMAGE.get().ok_or("engine image profile unavailable")?;
    let profile: Value = serde_json::from_str(PROFILE).map_err(|error| error.to_string())?;
    if profile["productVersion"].as_str() != Some(scene::PRODUCT_VERSION) {
        return Err("engine profile and scene-layout family differ".into());
    }
    let candidate = &profile["candidates"][name];
    let rva = candidate["targetRva"]
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| format!("missing {name} target"))?;
    if candidate["matches"] != 1
        || rva < start
        || rva.checked_add(prefix_bytes).is_none_or(|last| last > end)
    {
        return Err(format!("invalid exact-build {name} target"));
    }
    let image_size = profile["imageSize"]
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or("missing engine image size")?;
    Ok((rva, candidate.clone(), image_size))
}
/// Bounded immutable code evidence for an embedded reader binding. This never
/// executes a supplied address and remains behind complete image acceptance.
pub(crate) fn read_profile_function(rva: usize, length: usize) -> Result<Vec<u8>, String> {
    let &(base, start, end) = CODE_IMAGE.get().ok_or("engine image profile unavailable")?;
    if !(1..=65536).contains(&length)
        || rva < start
        || rva.checked_add(length).is_none_or(|last| last > end)
    {
        return Err("reader function lies outside accepted code image".into());
    }
    let mut bytes = vec![0u8; length];
    read(
        base.checked_add(rva)
            .ok_or("reader code address overflow")?,
        &mut bytes,
    )?;
    Ok(bytes)
}
pub(crate) fn timer_binding() -> Result<TimerBinding, String> {
    let (rva, candidate, _) = exact_function_candidate("timerAdvance", 17)?;
    let abi = match candidate["abi"].as_str() {
        Some("timerFloat") => TimerAdvanceAbi::TimerFloat,
        Some("timerFloatSingleTick") => TimerAdvanceAbi::TimerFloatSingleTick,
        _ => return Err("unsupported exact-build timer ABI".into()),
    };
    Ok(TimerBinding { rva, abi })
}

#[derive(Clone, Copy)]
pub(crate) struct CameraBinding {
    pub(crate) update_rva: usize,
    pub(crate) iterator_rva: usize,
}
fn function_witness(candidate: &Value, target: usize) -> Result<(), String> {
    let &(base, start, end) = CODE_IMAGE.get().ok_or("engine image profile unavailable")?;
    let site = candidate["witnessRva"]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .filter(|site| *site >= start && site.checked_add(5).is_some_and(|last| last <= end))
        .ok_or("invalid native function witness")?;
    let opcode = match candidate["witnessKind"].as_str() {
        Some("call") => 0xe8,
        Some("tailJump") => 0xe9,
        _ => return Err("unsupported native function witness".into()),
    };
    let mut bytes = [0u8; 5];
    read(
        base.checked_add(site)
            .ok_or("native witness address overflow")?,
        &mut bytes,
    )?;
    let displacement =
        i32::from_le_bytes(bytes[1..].try_into().map_err(|_| "short native witness")?);
    if bytes[0] != opcode
        || site
            .checked_add(5)
            .and_then(|next| next.checked_add_signed(displacement as isize))
            != Some(target)
    {
        return Err("native witness does not reference its declared target".into());
    }
    Ok(())
}
pub(crate) fn camera_binding() -> Result<CameraBinding, String> {
    let (update_rva, update, _) = exact_function_candidate("cameraUpdate", 19)?;
    let (iterator_rva, iterator, _) = exact_function_candidate("cameraIterate", 16)?;
    if update_rva == iterator_rva
        || update["abi"].as_str() != Some("cameraComponentModeActor")
        || iterator["abi"].as_str() != Some("cameraIteratorComponent")
    {
        return Err("unsupported native camera callback ABI".into());
    }
    function_witness(&update, update_rva)?;
    function_witness(&iterator, iterator_rva)?;
    Ok(CameraBinding {
        update_rva,
        iterator_rva,
    })
}

#[derive(Clone, Copy)]
pub(crate) struct CameraConsumerBinding {
    pub(crate) consumer_rva: usize,
    pub(crate) consumer_stack_bytes: u32,
    pub(crate) matrix_rva: usize,
}
pub(crate) fn camera_consumer_binding() -> Result<CameraConsumerBinding, String> {
    let (consumer_rva, consumer, _) = exact_function_candidate("cameraConsumer", 16)?;
    let (matrix_rva, matrix, _) = exact_function_candidate("cameraMatrix", 12)?;
    let stack = consumer["stackBytes"]
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .filter(|n| matches!(*n, 0x108 | 0xe8))
        .ok_or("unsupported camera consumer stack")?;
    if consumer_rva == matrix_rva
        || consumer["abi"].as_str() != Some("rendererCameraPartial")
        || matrix["abi"].as_str() != Some("threePointers")
    {
        return Err("unsupported camera consumer/matrix ABI".into());
    }
    function_witness(&consumer, consumer_rva)?;
    function_witness(&matrix, matrix_rva)?;
    Ok(CameraConsumerBinding {
        consumer_rva,
        consumer_stack_bytes: stack,
        matrix_rva,
    })
}
#[derive(Clone, Copy)]
pub(crate) struct CameraPreRenderBinding {
    pub(crate) rva: usize,
    pub(crate) stack_bytes: u32,
}
pub(crate) fn camera_pre_render_binding() -> Result<CameraPreRenderBinding, String> {
    let (rva, candidate, _) = exact_function_candidate("cameraPreRender", 19)?;
    let stack = candidate["stackBytes"]
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .filter(|n| matches!(*n, 0xe8 | 0xf8))
        .ok_or("unsupported camera pre-render stack")?;
    if candidate["abi"].as_str() != Some("cameraScreenFrame") {
        return Err("unsupported camera pre-render ABI".into());
    }
    function_witness(&candidate, rva)?;
    Ok(CameraPreRenderBinding {
        rva,
        stack_bytes: stack,
    })
}

#[derive(Clone, Copy)]
pub(crate) enum SkinnedDescriptionFrame {
    LegacyRbpMinus8,
    CompactRbpZero,
}
impl SkinnedDescriptionFrame {
    pub(crate) fn prefix(self) -> &'static [u8] {
        match self {
            Self::LegacyRbpMinus8 => &[
                0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x83, 0xec, 0x60,
                0x48, 0x8d, 0x6c, 0x24, 0x60, 0x48, 0xc7, 0x45, 0xf8, 0xfe, 0xff, 0xff, 0xff,
            ],
            Self::CompactRbpZero => &[
                0x55, 0x41, 0x57, 0x41, 0x56, 0x56, 0x57, 0x53, 0x48, 0x83, 0xec, 0x58, 0x48, 0x8d,
                0x6c, 0x24, 0x50, 0x48, 0xc7, 0x45, 0x00, 0xfe, 0xff, 0xff, 0xff,
            ],
        }
    }
}
#[derive(Clone, Copy)]
pub(crate) struct SkinnedDescriptionBinding {
    pub(crate) rva: usize,
    pub(crate) frame: SkinnedDescriptionFrame,
}
pub(crate) fn skinned_description_binding() -> Result<SkinnedDescriptionBinding, String> {
    let (rva, candidate, _) = exact_function_candidate("skinnedDescription", 27)?;
    if candidate["abi"].as_str() != Some("skinnedDescription21") {
        return Err("unsupported skinned description ABI".into());
    }
    // A matching calling convention/prologue does not establish compatible
    // output members. Color/material/flags moved by different amounts in the
    // compact constructor. Keep the selected entry coupled to its scene family.
    if candidate["frameLayout"].as_str()
        != Some(crate::native_scene_layout::skinned_description::CONSTRUCTOR_FRAME_LAYOUT)
    {
        return Err("skinned description frame does not match the scene layout family".into());
    }
    let frame = match candidate["frameLayout"].as_str() {
        Some("legacyRbpMinus8") => SkinnedDescriptionFrame::LegacyRbpMinus8,
        Some("compactRbpZero") => SkinnedDescriptionFrame::CompactRbpZero,
        _ => return Err("unsupported skinned description stack frame".into()),
    };
    function_witness(&candidate, rva)?;
    Ok(SkinnedDescriptionBinding { rva, frame })
}

#[derive(Clone, Copy)]
pub(crate) enum PreviousMatrixInput {
    Inline,
    Reference,
}
impl PreviousMatrixInput {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Inline => "inlineOptionalMatrix",
            Self::Reference => "optionalMatrixReference",
        }
    }
    pub(crate) fn writer_prefix(self) -> [u8; 27] {
        let mut bytes = [
            0x48, 0x63, 0xc2, 0x48, 0x8b, 0x51, 0x48, 0x4c, 0x8b, 0x51, 0x50, 0x49, 0x29, 0xd2,
            0x49, 0xc1, 0xfa, 0x04, 0x49, 0x39, 0xc2, 0x0f, 0x86, 0, 0, 0, 0,
        ];
        let displacement: i32 = match self {
            Self::Inline => 0x280,
            Self::Reference => 0x23f,
        };
        bytes[23..].copy_from_slice(&displacement.to_le_bytes());
        bytes
    }
}
#[derive(Clone, Copy)]
pub(crate) struct PaletteWriterBinding {
    pub(crate) rva: usize,
    pub(crate) previous: PreviousMatrixInput,
}
pub(crate) fn palette_writer_binding() -> Result<PaletteWriterBinding, String> {
    let (rva, candidate, _) = exact_function_candidate("paletteWriter", 27)?;
    if candidate["abi"].as_str() != Some("paletteWriter7") {
        return Err("unsupported palette writer ABI".into());
    }
    let previous = match candidate["previousMatrixInput"].as_str() {
        Some("inlineOptionalMatrix") => PreviousMatrixInput::Inline,
        Some("optionalMatrixReference") => PreviousMatrixInput::Reference,
        _ => return Err("unsupported palette writer previous matrix input".into()),
    };
    if previous.name() != crate::native_scene_layout::palette_owner::PREVIOUS_MATRIX_INPUT {
        return Err("palette writer input does not match the scene layout family".into());
    }
    function_witness(&candidate, rva)?;
    Ok(PaletteWriterBinding { rva, previous })
}

#[derive(Clone, Copy)]
pub(crate) struct SkinnedMaterialConsumerBinding {
    pub(crate) insert_rva: usize,
    pub(crate) material_rva: usize,
    pub(crate) vtable_rva: usize,
    pub(crate) material_stack_bytes: u32,
}
pub(crate) fn skinned_material_consumer_binding() -> Result<SkinnedMaterialConsumerBinding, String>
{
    let (insert_rva, insert, image_size) = exact_function_candidate("skinnedInsert", 19)?;
    let (material_rva, material, _) = exact_function_candidate("skinnedMaterial", 19)?;
    let layout = crate::native_scene_layout::skinned_material_consumer::INPUT_LAYOUT;
    let vtable_rva = insert["vtableRva"]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .filter(|n| *n == crate::native_scene_layout::skinned_material_consumer::VTABLE)
        .ok_or("skinned consumer vtable does not match the scene layout family")?;
    let slot = insert["vtableOffset"]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .filter(|n| *n == crate::native_scene_layout::skinned_material_consumer::INSERT_SLOT)
        .ok_or("skinned consumer slot does not match the scene layout family")?;
    let material_stack_bytes = material["stackBytes"]
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .filter(|n| matches!(*n, 0xed8 | 0xe38))
        .ok_or("unsupported skinned material stack")?;
    if insert_rva == material_rva
        || insert["abi"].as_str() != Some("frameBuilderVariant2")
        || material["abi"].as_str() != Some("skinnedMaterial5")
        || material["inputLayout"].as_str() != Some(layout)
    {
        return Err("unsupported skinned material consumer ABI/input layout".into());
    }
    let entry = vtable_rva
        .checked_add(slot)
        .filter(|n| n.checked_add(8).is_some_and(|end| end <= image_size))
        .ok_or("skinned consumer vtable entry outside image")?;
    let base = image_base().ok_or("skinned consumer image unavailable")?;
    let mut bytes = [0u8; 8];
    read(
        base.checked_add(entry)
            .ok_or("skinned consumer vtable overflow")?,
        &mut bytes,
    )?;
    if u64::from_le_bytes(bytes)
        != base
            .checked_add(insert_rva)
            .ok_or("skinned consumer target overflow")? as u64
    {
        return Err("skinned consumer vtable entry differs".into());
    }
    function_witness(&material, material_rva)?;
    Ok(SkinnedMaterialConsumerBinding {
        insert_rva,
        material_rva,
        vtable_rva,
        material_stack_bytes,
    })
}

#[derive(Clone, Copy)]
pub(crate) struct PalettePackBinding {
    pub(crate) rva: usize,
    pub(crate) cookie_rva: usize,
}
impl PalettePackBinding {
    pub(crate) fn prefix(self) -> Result<[u8; 25], String> {
        let next = self
            .rva
            .checked_add(11)
            .ok_or("palette pack RIP overflow")?;
        let displacement = i32::try_from(self.cookie_rva as i64 - next as i64)
            .map_err(|_| "palette pack cookie outside RIP range")?;
        let mut prefix = [
            0x48, 0x83, 0xec, 0x48, 0x48, 0x8b, 0x05, 0, 0, 0, 0, 0x48, 0x31, 0xe0, 0x48, 0x89,
            0x44, 0x24, 0x40, 0x48, 0x8b, 0x01, 0x48, 0x85, 0xc0,
        ];
        prefix[7..11].copy_from_slice(&displacement.to_le_bytes());
        Ok(prefix)
    }
}
pub(crate) fn palette_pack_binding() -> Result<PalettePackBinding, String> {
    let (rva, candidate, image_size) = exact_function_candidate("palettePack", 25)?;
    if candidate["abi"].as_str() != Some("boneSpanOutput2")
        || candidate["spanLayout"].as_str() != Some("countPointer")
        || candidate["matrixBytes"].as_u64() != Some(64)
        || candidate["paddedMatrices"].as_u64() != Some(8)
    {
        return Err("unsupported palette pack ABI/layout".into());
    }
    let cookie_rva = candidate["securityCookieRva"]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .filter(|n| n.checked_add(8).is_some_and(|end| end <= image_size))
        .ok_or("palette pack security cookie outside image")?;
    function_witness(&candidate, rva)?;
    Ok(PalettePackBinding { rva, cookie_rva })
}

#[derive(Clone, Copy)]
pub(crate) struct ActorCacheLifetimeBinding {
    pub(crate) actor_rva: usize,
    pub(crate) cache_rva: usize,
    pub(crate) vtable_rva: usize,
    pub(crate) cache_offset: usize,
    pub(crate) storage: crate::native_scene_layout::EmoteCacheStorage,
}
impl ActorCacheLifetimeBinding {
    pub(crate) fn actor_prefix(self) -> Result<[u8; 16], String> {
        let next = self
            .actor_rva
            .checked_add(16)
            .ok_or("actor destructor RIP overflow")?;
        let displacement = i32::try_from(self.vtable_rva as i64 - next as i64)
            .map_err(|_| "actor destructor vtable outside RIP range")?;
        let mut bytes = [
            0x56, 0x57, 0x48, 0x83, 0xec, 0x28, 0x48, 0x89, 0xce, 0x48, 0x8d, 0x05, 0, 0, 0, 0,
        ];
        bytes[12..].copy_from_slice(&displacement.to_le_bytes());
        Ok(bytes)
    }
    pub(crate) fn cache_prefix(self) -> &'static [u8] {
        match self.storage {
            crate::native_scene_layout::EmoteCacheStorage::Tree => {
                &[0x56, 0x48, 0x83, 0xec, 0x20, 0x48, 0x89, 0xce]
            }
            crate::native_scene_layout::EmoteCacheStorage::FlatParallelVectors => {
                &[0x56, 0x57, 0x48, 0x83, 0xec, 0x28, 0x48, 0x8b, 0x41, 0x18]
            }
        }
    }
}
pub(crate) fn actor_cache_lifetime_binding() -> Result<ActorCacheLifetimeBinding, String> {
    use crate::native_scene_layout::{EmoteCacheStorage, actor, emote_cache};
    let (actor_rva, destructor, image_size) = exact_function_candidate("actorDestructor", 16)?;
    let (cache_rva, cache, _) = exact_function_candidate("actorCacheDestructor", 10)?;
    let vtable_rva = destructor["ownerVtableRva"]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .filter(|n| *n == actor::VTABLE)
        .ok_or("actor destructor vtable differs from scene family")?;
    let storage = match cache["storage"].as_str() {
        Some("tree") => EmoteCacheStorage::Tree,
        Some("flatParallelVectors") => EmoteCacheStorage::FlatParallelVectors,
        _ => return Err("unsupported actor cache storage".into()),
    };
    let cache_offset = cache["ownerOffset"]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .filter(|n| *n == actor::CACHE)
        .ok_or("actor cache offset differs from scene family")?;
    if storage != emote_cache::STORAGE
        || actor_rva == cache_rva
        || destructor["abi"].as_str() != Some("actorDestructor1")
        || cache["abi"].as_str() != Some("actorCacheDestructor1")
    {
        return Err("unsupported actor/cache destructor family or ABI".into());
    }
    let wrapper = destructor["deletingDestructorRva"]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or("missing actor deleting destructor")?;
    let (_, code_start, code_end) = *CODE_IMAGE
        .get()
        .ok_or("actor destructor image unavailable")?;
    if wrapper < code_start
        || wrapper.checked_add(16).is_none_or(|end| end > code_end)
        || destructor["witnessRva"].as_u64() != wrapper.checked_add(11).map(|n| n as u64)
    {
        return Err("actor deleting destructor witness outside wrapper".into());
    }
    let entry = vtable_rva
        .checked_add(actor::DELETING_DESTRUCTOR_SLOT * 8)
        .filter(|n| n.checked_add(8).is_some_and(|end| end <= image_size))
        .ok_or("actor deleting destructor slot outside image")?;
    let base = image_base().ok_or("actor destructor image unavailable")?;
    let mut pointer = [0u8; 8];
    read(
        base.checked_add(entry)
            .ok_or("actor vtable address overflow")?,
        &mut pointer,
    )?;
    if u64::from_le_bytes(pointer)
        != base
            .checked_add(wrapper)
            .ok_or("actor wrapper address overflow")? as u64
    {
        return Err("actor deleting destructor vtable entry differs".into());
    }
    let mut prefix = [0u8; 11];
    read(
        base.checked_add(wrapper)
            .ok_or("actor wrapper address overflow")?,
        &mut prefix,
    )?;
    if prefix
        != [
            0x56, 0x57, 0x48, 0x83, 0xec, 0x28, 0x89, 0xd7, 0x48, 0x89, 0xce,
        ]
    {
        return Err("actor deleting destructor prefix differs".into());
    }
    function_witness(&destructor, actor_rva)?;
    function_witness(&cache, cache_rva)?;
    Ok(ActorCacheLifetimeBinding {
        actor_rva,
        cache_rva,
        vtable_rva,
        cache_offset,
        storage,
    })
}

#[path = "native_emote_cache_profile.rs"]
mod cache_profile;
pub(crate) use cache_profile::{
    CacheMatrixInsertBinding, CacheObserverBinding, cache_observer_binding,
};

#[derive(Clone, Copy)]
pub(crate) struct ScreenContextBinding {
    pub(crate) rva: usize,
    pub(crate) stack_bytes: u32,
    pub(crate) world_return_rva: usize,
}
/// A return-site filter is part of the binding: require its actual direct call
/// to target this exact function, rather than accepting an arbitrary code RVA.
fn direct_return_rva(candidate: &Value, field: &str, target_rva: usize) -> Result<usize, String> {
    let &(base, start, end) = CODE_IMAGE.get().ok_or("engine image profile unavailable")?;
    let return_rva = candidate[field]
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| format!("missing {field}"))?;
    let site = return_rva
        .checked_sub(5)
        .filter(|site| *site >= start && return_rva <= end)
        .ok_or_else(|| format!("invalid {field} code address"))?;
    let mut bytes = [0u8; 5];
    read(
        base.checked_add(site)
            .ok_or("engine call address overflow")?,
        &mut bytes,
    )?;
    let displacement = i32::from_le_bytes(
        bytes[1..]
            .try_into()
            .map_err(|_| "short engine direct call")?,
    );
    if bytes[0] != 0xe8 || return_rva.checked_add_signed(displacement as isize) != Some(target_rva)
    {
        return Err(format!(
            "{field} does not call its declared exact-build target"
        ));
    }
    Ok(return_rva)
}
pub(crate) fn screen_context_binding() -> Result<ScreenContextBinding, String> {
    let (rva, candidate, _) = exact_function_candidate("makeScreenContext", 19)?;
    if candidate["abi"].as_str() != Some("rendererScreenFloat") {
        return Err("unsupported screen-context producer ABI".into());
    }
    let stack_bytes = candidate["stackBytes"]
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .ok_or("missing screen-context stack size")?;
    if !matches!(stack_bytes, 0x1a8 | 0x178) {
        return Err("unrecognized screen-context producer stack layout".into());
    }
    let world_return_rva = direct_return_rva(&candidate, "worldReturnRva", rva)?;
    Ok(ScreenContextBinding {
        rva,
        stack_bytes,
        world_return_rva,
    })
}

#[derive(Clone, Copy)]
pub(crate) struct FmodBinding {
    pub(crate) rva: usize,
    pub(crate) iat_rva: usize,
    pub(crate) total_return_rva: usize,
    pub(crate) delta_return_rva: usize,
    pub(crate) world_return_rva: usize,
}
pub(crate) fn fmod_binding() -> Result<FmodBinding, String> {
    let (rva, candidate, image_size) = exact_function_candidate("fmodThunk", 6)?;
    if candidate["abi"].as_str() != Some("doubleDouble") {
        return Err("unsupported shader-time import ABI".into());
    }
    let iat_rva = candidate["iatRva"]
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or("missing shader-time import slot")?;
    if iat_rva.checked_add(8).is_none_or(|end| end > image_size) {
        return Err("shader-time import slot is outside the engine image".into());
    }
    let total_return_rva = direct_return_rva(&candidate, "totalReturnRva", rva)?;
    let delta_return_rva = direct_return_rva(&candidate, "deltaReturnRva", rva)?;
    let world_return_rva = direct_return_rva(&candidate, "worldReturnRva", rva)?;
    if !(total_return_rva < delta_return_rva && delta_return_rva < world_return_rva) {
        return Err("shader-time caller roles are duplicated or out of order".into());
    }
    Ok(FmodBinding {
        rva,
        iat_rva,
        total_return_rva,
        delta_return_rva,
        world_return_rva,
    })
}

#[derive(Clone, Copy)]
pub(crate) struct PresentationClockBinding {
    pub(crate) epoch_rva: usize,
    pub(crate) guard_rva: usize,
}
/// Identify the data used by the inspected clock routine. The routine itself
/// is never invoked: native_presentation_clock uses public QPC APIs instead.
pub(crate) fn presentation_clock_binding() -> Result<PresentationClockBinding, String> {
    let (rva, candidate, image_size) = exact_function_candidate("presentationClock", 321)?;
    if candidate["abi"].as_str() != Some("qpcNanosecondsEpoch") {
        return Err("unsupported presentation clock conversion".into());
    }
    let number = |field: &str| {
        candidate[field]
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| format!("missing presentation clock {field}"))
    };
    let epoch_rva = number("epochRva")?;
    let guard_rva = number("guardRva")?;
    if epoch_rva % 8 != 0
        || guard_rva % 4 != 0
        || epoch_rva.checked_add(8).is_none_or(|end| end > image_size)
        || guard_rva.checked_add(4).is_none_or(|end| end > image_size)
    {
        return Err("invalid presentation clock storage".into());
    }
    let base = image_base().ok_or("presentation clock image unavailable")?;
    for (offset, opcode, target) in [
        (0xau32, &[0x8b, 0x05][..], guard_rva),
        (0x2f, &[0x48, 0x8b, 0x1d][..], epoch_rva),
    ] {
        let at = rva
            .checked_add(offset as usize)
            .ok_or("presentation clock code overflow")?;
        let mut bytes = vec![0u8; opcode.len() + 4];
        read(
            base.checked_add(at)
                .ok_or("presentation clock address overflow")?,
            &mut bytes,
        )?;
        let displacement = i32::from_le_bytes(
            bytes[opcode.len()..]
                .try_into()
                .map_err(|_| "short clock operand")?,
        );
        if &bytes[..opcode.len()] != opcode
            || at
                .checked_add(bytes.len())
                .and_then(|value| value.checked_add_signed(displacement as isize))
                != Some(target)
        {
            return Err("presentation clock storage reference differs".into());
        }
    }
    let _ = direct_return_rva(&candidate, "shaderTimeReturnRva", rva)?;
    Ok(PresentationClockBinding {
        epoch_rva,
        guard_rva,
    })
}
static PUBLISHED: AtomicU64 = AtomicU64::new(0);
static PUBLISHED_SCOPE: AtomicU64 = AtomicU64::new(0);
static PUBLISHED_SEQUENCE: AtomicU64 = AtomicU64::new(0);
static PUBLISH_ERROR: AtomicI32 = AtomicI32::new(i32::MIN);
static DELIVERED: AtomicU64 = AtomicU64::new(0);
static DELIVERED_SEQUENCE: AtomicU64 = AtomicU64::new(0);
pub(crate) fn delivered(sequence: Option<u64>) {
    if let Some(sequence) = sequence {
        DELIVERED.fetch_add(1, Ordering::Relaxed);
        DELIVERED_SEQUENCE.store(sequence, Ordering::Relaxed);
    }
}

/// Bounded code-address diagnostics for the deferred rendering handoff. No
/// stack bytes, engine pointers or addresses from other modules are retained.
pub(crate) fn game_callers() -> Vec<usize> {
    let Some(&(base, start, end)) = CODE_IMAGE.get() else {
        return Vec::new();
    };
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn RtlCaptureStackBackTrace(
            skip: u32,
            count: u32,
            frames: *mut usize,
            hash: *mut u32,
        ) -> u16;
    }
    let mut frames = [0usize; 32];
    let count = unsafe {
        RtlCaptureStackBackTrace(
            0,
            frames.len() as u32,
            frames.as_mut_ptr(),
            std::ptr::null_mut(),
        )
    };
    frames[..usize::from(count).min(frames.len())]
        .iter()
        .filter_map(|pc| pc.checked_sub(base))
        .filter(|rva| (start..end).contains(rva))
        .collect()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ActorSample {
    // These are the values supplied to the dispatcher, not inferred world feet
    // coordinates or a stable identity. Tokens are local to this render pass.
    token: usize,
    camera_target: Option<[f32; 3]>,
    render_position: [f32; 3],
    render_rotation: [f32; 2],
    ignore_lighting: bool,
    pub(crate) player: Option<crate::players::Actor>,
    #[serde(skip)]
    pub(crate) animation: Option<crate::animation_pose::Pose>,
    pub(crate) cape_motion: Option<crate::native_capes::Motion>,
    pub(crate) cape_state: Option<crate::native_capes::Snapshot>,
    pub(crate) global_model: Option<crate::native_transform::Global>,
}
struct Pass {
    scope: u64,
    actors: Vec<(usize, ActorSample)>,
    rejected: u32,
}
pub(crate) struct Observation {
    scope: u64,
    pub(crate) actors: Vec<ActorSample>,
    pub(crate) camera: Option<CameraSample>,
    rejected: u32,
    pub(crate) sequence: u64,
    pub(crate) tick_count: u64,
    at: Instant,
    pub(crate) depth: Option<crate::depth::Depth>,
}
impl Observation {
    pub(crate) fn scope(&self) -> u64 {
        self.scope
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CameraSample {
    pub(crate) origin: [f32; 3],
    pub(crate) view: [f32; 16],
    pub(crate) projection: [f32; 16],
    pub(crate) partial: Option<f32>,
    pub(crate) ticks_per_second: Option<f32>,
    pub(crate) client_ticks: Option<u64>,
    pub(crate) client_elapsed_ms: Option<f64>,
    pub(crate) local_player: Option<String>,
    #[serde(skip)]
    pub(crate) local_actor: Option<crate::players::Actor>,
    pub(crate) local_cape_motion: Option<crate::native_capes::Motion>,
    pub(crate) local_cape_state: Option<crate::native_capes::Snapshot>,
    pub(crate) epoch: Option<u64>,
    pub(crate) render_clock: Option<[f32; 4]>,
    pub(crate) renderer_timing: Option<Value>,
    pub(crate) real_timer: Option<Value>,
    pub(crate) native_camera: Option<Value>,
    pub(crate) resource_stack: Option<crate::native_resource_packs::Snapshot>,
}

pub(crate) fn pointer(address: usize) -> Option<usize> {
    let mut bytes = [0u8; 8];
    read(address, &mut bytes).ok()?;
    let value = u64::from_le_bytes(bytes) as usize;
    (0x10000..0x0000_8000_0000_0000)
        .contains(&value)
        .then_some(value)
}

fn observe_camera(renderer: usize) -> Option<CameraSample> {
    let global = PLATFORM_GLOBAL.load(Ordering::Acquire);
    camera_from(renderer, global)
}

fn camera_from(renderer: usize, global: usize) -> Option<CameraSample> {
    let game = game_from_global(global)?;
    camera_from_game(renderer, game)
}
fn game_from_global(global: usize) -> Option<usize> {
    if global == 0 {
        return None;
    }
    // Exact-build layout from the attributed SDK. This reads copies only; it
    // never constructs objects, invokes virtual methods or writes timer/camera
    // fields. Matrix semantics still require live-world validation.
    let win_main = pointer(global)?;
    let platform = pointer(win_main.checked_add(8)?)?;
    let holder = pointer(platform.checked_add(0x118)?)?;
    pointer(holder.checked_add(0x48)?)
}
/// Re-read the active primary client and require its currently owned level.
pub(crate) fn primary_client(level: usize) -> Option<crate::live_client::Client> {
    let game = game_from_global(PLATFORM_GLOBAL.load(Ordering::Acquire))?;
    let primary = crate::live_client::primary(game)?;
    let renderer = pointer(primary.checked_add(scene::client::LEVEL_RENDERER)?)?;
    let client = crate::live_client::read_client(game, renderer)?;
    (client.level == level).then_some(client)
}

/// Read-only primary-renderer clock identity; unlike render_control this does
/// not require or acquire a frozen clock. Raw addresses remain inside native
/// callbacks and are never part of an archived/transported clock sample.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct RendererClockIdentity {
    pub(crate) client: usize,
    pub(crate) timer: usize,
    pub(crate) real_timer: Option<usize>,
    pub(crate) level_renderer: usize,
    pub(crate) game_renderer: usize,
    pub(crate) level: usize,
    pub(crate) epoch: u64,
    pub(crate) tick: u64,
    pub(crate) partial_bits: u32,
    pub(crate) rate_bits: u32,
    pub(crate) elapsed_bits: u64,
}
pub(crate) fn renderer_clock_identity(game_renderer: usize) -> Option<RendererClockIdentity> {
    image_base()?;
    let game = game_from_global(PLATFORM_GLOBAL.load(Ordering::Acquire))?;
    let primary = crate::live_client::primary(game)?;
    let renderer = pointer(primary.checked_add(scene::client::LEVEL_RENDERER)?)?;
    let before = crate::live_client::read_client(game, renderer)?;
    let (level, epoch) = crate::players::session()?;
    if before.level != level
        || !(0. ..1.).contains(&before.partial)
        || pointer(game.checked_add(scene::game::RENDERER)?)? != game_renderer
        || pointer(renderer.checked_add(scene::level_renderer::GAME_RENDERER)?)? != game_renderer
    {
        return None;
    }
    let tick = client_completed_tick(level)?;
    let elapsed = completed_tick_clock(level, tick)?;
    if tick > 1_000_000_000_000
        || !elapsed.is_finite()
        || !(0. ..=1_000_000_000_000_000.).contains(&elapsed)
    {
        return None;
    }
    let after = crate::live_client::read_client(game, renderer)?;
    if before.instance != after.instance
        || before.level != after.level
        || before.sim_timer != after.sim_timer
        || before.real_timer != after.real_timer
        || before.partial.to_bits() != after.partial.to_bits()
        || before.tps.to_bits() != after.tps.to_bits()
        || crate::players::session() != Some((level, epoch))
        || client_completed_tick(level)? != tick
        || completed_tick_clock(level, tick)?.to_bits() != elapsed.to_bits()
        || pointer(game.checked_add(scene::game::RENDERER)?)? != game_renderer
        || pointer(renderer.checked_add(scene::level_renderer::GAME_RENDERER)?)? != game_renderer
    {
        return None;
    }
    Some(RendererClockIdentity {
        client: before.instance,
        timer: before.sim_timer,
        real_timer: before.real_timer,
        level_renderer: renderer,
        game_renderer,
        level,
        epoch,
        tick,
        partial_bits: before.partial.to_bits(),
        rate_bits: before.tps.to_bits(),
        elapsed_bits: elapsed.to_bits(),
    })
}
/// The live local-player registry is independent of cached player-list freshness.
pub(crate) fn current_registry(level: usize) -> Option<crate::ecs::RegistryKey> {
    let client = primary_client(level)?;
    let actor = client.local_player?;
    if client.level != level || pointer(actor.checked_add(0x1d8)?)? != level {
        return None;
    }
    crate::ecs::registry_key(actor)
}
pub(crate) fn current_resource_stack() -> Option<crate::native_resource_packs::Snapshot> {
    let game = game_from_global(PLATFORM_GLOBAL.load(Ordering::Acquire))?;
    let primary = crate::live_client::primary(game)?;
    let renderer = pointer(primary.checked_add(scene::client::LEVEL_RENDERER)?)?;
    let client = crate::live_client::read_client(game, renderer)?;
    Some(crate::native_resource_packs::sample(client.instance))
}
pub(crate) fn attachment_timing() -> Option<(String, Value)> {
    let game = game_from_global(PLATFORM_GLOBAL.load(Ordering::Acquire))?;
    let primary = crate::live_client::primary(game)?;
    let renderer = pointer(primary.checked_add(scene::client::LEVEL_RENDERER)?)?;
    emote_timing(crate::live_client::read_client(game, renderer)?.partial)
}
pub(crate) fn attachment_origin() -> Option<[f32; 3]> {
    let game = game_from_global(PLATFORM_GLOBAL.load(Ordering::Acquire))?;
    let primary = crate::live_client::primary(game)?;
    let renderer = pointer(primary.checked_add(scene::client::LEVEL_RENDERER)?)?;
    let game_renderer = pointer(game.checked_add(scene::game::RENDERER)?)?;
    let camera = crate::native_clock::owned_player_renderer(game_renderer)?;
    if pointer(renderer.checked_add(scene::level_renderer::PLAYER_OWNER)?)? != camera {
        return None;
    }
    let origin = floats(
        camera.checked_add(scene::player_renderer::ORIGIN)?,
        30_000_000.,
    )?;
    if pointer(renderer.checked_add(scene::level_renderer::PLAYER_OWNER)?)? != camera
        || crate::native_clock::owned_player_renderer(game_renderer)? != camera
    {
        return None;
    }
    Some(origin)
}
pub(crate) fn emote_timing(partial: f32) -> Option<(String, Value)> {
    if !partial.is_finite() || !(0. ..1.).contains(&partial) {
        return None;
    }
    let game = game_from_global(PLATFORM_GLOBAL.load(Ordering::Acquire))?;
    let primary = crate::live_client::primary(game)?;
    let renderer = pointer(primary.checked_add(scene::client::LEVEL_RENDERER)?)?;
    let client = crate::live_client::read_client(game, renderer)?;
    if partial.to_bits() != client.partial.to_bits() {
        return None;
    }
    let epoch = crate::players::epoch(client.level)?;
    let tick = client_completed_tick(client.level)?;
    let elapsed = completed_tick_clock(client.level, tick)?;
    client_sample_seconds(client.level, partial, client.tps)?;
    let session = format!("native_{}_{}", std::process::id(), epoch);
    let timing = json!({"ticks":tick,"elapsedMs":elapsed,"partial":partial,"ticksPerSecond":client.tps,
        "captureControls":{"clock":crate::native_clock::capture_sample(&session,tick,partial),
            "packetDispatch":crate::native_packet::capture_sample(&session,tick)}});
    let after = crate::live_client::read_client(game, renderer)?;
    if after.level != client.level
        || after.sim_timer != client.sim_timer
        || after.partial.to_bits() != partial.to_bits()
        || after.tps.to_bits() != client.tps.to_bits()
        || client_completed_tick(client.level) != Some(tick)
        || crate::players::epoch(client.level) != Some(epoch)
    {
        return None;
    }
    Some((session, timing))
}
fn camera_from_game(renderer: usize, game: usize) -> Option<CameraSample> {
    let client = crate::live_client::read_client(game, renderer);
    let client_time = client.as_ref().and_then(|client| {
        CLIENT_CLOCK
            .lock()
            .ok()
            .and_then(|mut clock| clock.select(client.level, client.tps))
    });
    if let Some(client) = &client {
        crate::players::refresh_if_needed(client.level);
    }
    let local_actor = client
        .as_ref()
        .and_then(|client| client.local_player)
        .and_then(crate::players::actor);
    crate::native_capes::local(local_actor.as_ref().map(|a| a.identity.id.as_str()));
    if let Some((address, actor)) = client
        .as_ref()
        .and_then(|client| client.local_player)
        .zip(local_actor.as_ref())
    {
        // First-person cameras can omit the local actor's model draw. The
        // validated camera identity still keeps its completed-tick filter live.
        crate::native_capes::remember(address, actor);
    }
    let game_renderer = pointer(game.checked_add(scene::game::RENDERER)?)?;
    let player_renderer = pointer(renderer.checked_add(scene::level_renderer::PLAYER_OWNER)?)?;
    let origin = floats(
        player_renderer.checked_add(scene::player_renderer::ORIGIN)?,
        30_000_000.,
    )?;
    let view: [f32; 16] = floats(
        game_renderer.checked_add(scene::game_renderer::VIEW)?,
        1_000_000.,
    )?;
    let projection: [f32; 16] = floats(
        game_renderer.checked_add(scene::game_renderer::PROJECTION)?,
        1_000_000.,
    )?;
    if !view.iter().any(|f| f.abs() > 0.000001) || !projection.iter().any(|f| f.abs() > 0.000001) {
        return None;
    }
    if let Some(client) = &client
        && let Some(epoch) = crate::players::epoch(client.level)
    {
        crate::native_clock::bind(
            client.level,
            client.sim_timer,
            epoch,
            game,
            renderer,
            game_renderer,
            client.real_timer,
        );
        if let Some(actor) = local_actor.as_ref() {
            crate::native_camera::bind(game, renderer, client, epoch, &actor.identity.id);
        }
    }
    Some(CameraSample {
        resource_stack: client
            .as_ref()
            .map(|c| crate::native_resource_packs::sample(c.instance)),
        native_camera: client
            .as_ref()
            .and_then(|c| crate::players::epoch(c.level))
            .and_then(crate::native_camera::snapshot),
        real_timer: client
            .as_ref()
            .and_then(|c| c.real_timer)
            .and_then(crate::native_clock::timer_sample),
        render_clock: floats(
            game_renderer.checked_add(scene::game_renderer::CLOCK)?,
            1_000_000_000.,
        ),
        renderer_timing: (|| {
            let mut ticks = [0u8; 4];
            read(
                game_renderer.checked_add(scene::game_renderer::TICK)?,
                &mut ticks,
            )
            .ok()?;
            let tick = u32::from_le_bytes(ticks);
            if tick > 1_000_000_000 {
                return None;
            }
            let last = floats::<1>(
                game_renderer.checked_add(scene::game_renderer::LAST_CLOCK)?,
                1_000_000_000.,
            )?[0];
            Some(
                json!({"tick":tick,"lastClockTime":last,"clockMapping":crate::native_render_clock::mapping(game_renderer)}),
            )
        })(),
        origin,
        view,
        projection,
        partial: client.as_ref().map(|client| client.partial),
        ticks_per_second: client.as_ref().map(|client| client.tps),
        client_ticks: client_time.map(|v| v.0),
        client_elapsed_ms: client_time.map(|v| v.1),
        epoch: client
            .as_ref()
            .and_then(|client| crate::players::epoch(client.level)),
        local_player: local_actor.as_ref().map(|actor| actor.identity.id.clone()),
        local_cape_motion: (|| {
            let c = client.as_ref()?;
            let epoch = crate::players::epoch(c.level)?;
            crate::native_capes::sample(local_actor.as_ref()?, epoch, c.partial, client_time?.0)
        })(),
        local_cape_state: (|| {
            let c = client.as_ref()?;
            let epoch = crate::players::epoch(c.level)?;
            crate::native_capes::snapshot(local_actor.as_ref()?, epoch, client_time?.0)
        })(),
        local_actor,
    })
}

pub(crate) fn publish_world(width: u32, height: u32) {
    if ACTIVE_HOOKS.load(Ordering::Acquire) != 7 {
        return;
    }
    let accepted = snapshots().try_lock().ok().and_then(|state| {
        let observation = state.selected()?;
        if observation.at.elapsed() >= Duration::from_millis(500) {
            return None;
        }
        crate::live_frame::build(observation, width, height)
            .map(|(json, depth)| (observation.scope, observation.sequence, json, depth))
    });
    if let Some((scope, sequence, json, depth)) = accepted {
        match crate::engine::submit(&json, &depth) {
            Ok(()) => {
                PUBLISHED.fetch_add(1, Ordering::Relaxed);
                PUBLISHED_SCOPE.store(scope, Ordering::Relaxed);
                PUBLISHED_SEQUENCE.store(sequence, Ordering::Relaxed);
                PUBLISH_ERROR.store(0, Ordering::Relaxed);
            }
            Err(code) => {
                PUBLISH_ERROR.store(code, Ordering::Relaxed);
            }
        }
    } else {
        crate::engine::arctic_bedrock_clear_world_v1();
    }
}
thread_local! {
    // A stack keeps nested/off-screen world passes separate. No TLS borrow is
    // held while calling the engine or a trampoline.
    static COLLECTING: RefCell<Vec<Pass>> = const { RefCell::new(Vec::new()) };
    static SKIPPED: Cell<usize> = const { Cell::new(0) };
}

fn snapshots() -> &'static Mutex<history::History> {
    HISTORY.get_or_init(Mutex::default)
}
pub(crate) fn has_snapshot(scope: u64) -> bool {
    snapshots()
        .try_lock()
        .is_ok_and(|history| history.get(scope).is_some())
}
pub(crate) fn native_session(epoch: u64) -> String {
    format!("native_{}_{}", std::process::id(), epoch)
}
pub(crate) struct DrawCamera {
    pub session: String,
    pub sequence: u64,
    pub origin: [f64; 3],
    pub projection: [f32; 16],
}
/// Read one exact retained observation. A recent unrelated world is not an
/// identity or camera fallback for a native list's scope.
pub(crate) fn draw_camera(scope: u64, reverse_z: bool) -> Option<DrawCamera> {
    let history = snapshots().try_lock().ok()?;
    let frame = history.get(scope)?;
    let camera = frame.camera.as_ref()?;
    Some(DrawCamera {
        session: native_session(camera.epoch?),
        sequence: frame.sequence,
        origin: camera.origin.map(f64::from),
        projection: crate::live_frame::packet_projection(camera, reverse_z),
    })
}
/// Copied input for the serialized Java geometry producer. It does not select
/// the presentation world or retain a native object reference/depth buffer.
/// The producer FrameKey is a guarded numeric identity, not an owning pointer.
pub(crate) struct GeometryInput {
    scope: u64,
    producer: crate::render_bridge::FrameKey,
    json: String,
    session: String,
    sequence: u64,
    width: u32,
    height: u32,
    origin: [f64; 3],
    projection: [f32; 16],
    reverse_z: bool,
}
impl GeometryInput {
    pub(crate) fn scope(&self) -> u64 {
        self.scope
    }
    pub(crate) fn surface(&self) -> (u32, u32, bool) {
        (self.width, self.height, self.reverse_z)
    }
    pub(crate) fn json(&self) -> &str {
        &self.json
    }
    pub(crate) fn session(&self) -> &str {
        &self.session
    }
    pub(crate) fn sequence(&self) -> u64 {
        self.sequence
    }
    pub(crate) fn current(&self) -> bool {
        let identity = snapshots().try_lock().ok().and_then(|history| {
            let frame = history.get(self.scope)?;
            (frame.sequence == self.sequence
                && native_session(frame.camera.as_ref()?.epoch?) == self.session)
                .then_some(())
        });
        identity.is_some()
            && crate::render_bridge::producer_for_scope(self.scope) == Some(self.producer)
    }
    pub(crate) fn matches(&self, frame: &crate::avatar_geometry::Frame) -> bool {
        frame.session == self.session
            && frame.sequence == self.sequence
            && frame.width == self.width
            && frame.height == self.height
            && frame.space == crate::avatar_geometry::PositionSpace::CameraRelative
            && frame.camera.map(f64::to_bits) == self.origin.map(f64::to_bits)
            && frame.view_projection.map(f32::to_bits) == self.projection.map(f32::to_bits)
    }
}
pub(crate) fn geometry_input(
    scope: u64,
    width: u32,
    height: u32,
    reverse_z: bool,
) -> Option<GeometryInput> {
    let producer = crate::render_bridge::producer_for_scope(scope)?;
    let input = {
        let history = snapshots().try_lock().ok()?;
        let frame = history.get(scope)?;
        let json = crate::live_frame::geometry_input(frame, width, height, reverse_z)?;
        if json.len() > 1024 * 1024 {
            return None;
        }
        let camera = frame.camera.as_ref()?;
        GeometryInput {
            scope,
            producer,
            json,
            session: native_session(camera.epoch?),
            sequence: frame.sequence,
            width,
            height,
            origin: camera.origin.map(f64::from),
            projection: crate::live_frame::packet_projection(camera, reverse_z),
            reverse_z,
        }
    };
    input.current().then_some(input)
}
pub(crate) fn begin_gpu_frame() {
    if let Ok(mut history) = snapshots().lock() {
        history.begin_frame();
    }
}

pub(crate) fn current_pass() -> Option<u64> {
    #[cfg(test)]
    if let Some(scope) = TEST_SCOPE.with(Cell::get) {
        return Some(scope);
    }
    if SKIPPED.try_with(Cell::get).ok()? != 0 {
        return None;
    }
    COLLECTING
        .try_with(|stack| stack.try_borrow().ok()?.last().map(|pass| pass.scope))
        .ok()
        .flatten()
}

#[cfg(test)]
thread_local! {static TEST_SCOPE:Cell<Option<u64>>=const{Cell::new(None)};}
#[cfg(test)]
pub(crate) fn with_test_scope<T>(scope: u64, run: impl FnOnce() -> T) -> T {
    struct Restore(Option<u64>);
    impl Drop for Restore {
        fn drop(&mut self) {
            TEST_SCOPE.with(|slot| slot.set(self.0));
        }
    }
    let _restore = Restore(TEST_SCOPE.with(|slot| slot.replace(Some(scope))));
    run()
}

pub(crate) fn latest_scope() -> Option<u64> {
    let state = snapshots().try_lock().ok()?;
    let state = state.latest()?;
    (state.at.elapsed() < Duration::from_millis(500) && state.camera.is_some())
        .then_some(state.scope)
}
pub(crate) fn world_present() -> bool {
    snapshots()
        .lock()
        .ok()
        .and_then(|h| {
            let latest = h.latest()?;
            let camera = latest.camera.as_ref()?;
            (latest.at.elapsed() < Duration::from_millis(500)
                && camera.epoch.is_some()
                && camera.local_player.is_some())
            .then_some(())
        })
        .is_some()
}

pub(crate) fn diagnostic_camera_matrices() -> Option<([f32; 16], [f32; 16])> {
    let state = snapshots().try_lock().ok()?;
    let state = state.latest()?;
    if state.at.elapsed() >= Duration::from_millis(500) {
        return None;
    }
    let camera = state.camera.as_ref()?;
    Some((camera.view, camera.projection))
}

pub(crate) fn accept_depth(scope: u64, depth: crate::depth::Depth) {
    if let Ok(mut history) = snapshots().try_lock() {
        history.accept(scope, depth);
    }
}

pub(crate) fn read(address: usize, output: &mut [u8]) -> Result<(), String> {
    if address == 0 || address.checked_add(output.len()).is_none() {
        return Err("invalid memory range".into());
    }
    let mut copied = 0;
    // SAFETY: ReadProcessMemory copies to an owned, sized byte buffer and reports
    // inaccessible source pages instead of dereferencing engine-owned pointers.
    unsafe {
        ReadProcessMemory(
            GetCurrentProcess(),
            address as *const c_void,
            output.as_mut_ptr().cast(),
            output.len(),
            Some(&mut copied),
        )
    }
    .map_err(|e| e.to_string())?;
    if copied != output.len() {
        return Err("short game-memory read".into());
    }
    Ok(())
}

pub(crate) fn floats<const N: usize>(address: usize, bound: f32) -> Option<[f32; N]> {
    let mut data = vec![0u8; N * 4];
    read(address, &mut data).ok()?;
    let result = std::array::from_fn(|i| {
        f32::from_le_bytes(data[i * 4..i * 4 + 4].try_into().expect("four bytes"))
    });
    result
        .iter()
        .all(|f| f.is_finite() && f.abs() <= bound)
        .then_some(result)
}

struct ActorBinding {
    dispatcher: usize,
    global_model: Option<crate::native_transform::Global>,
}
fn observe_actor(
    actor: usize,
    context: usize,
    target: usize,
    position: usize,
    rotation: usize,
    lit: bool,
    binding: ActorBinding,
) {
    if SKIPPED.with(Cell::get) != 0 {
        return;
    }
    COLLECTING.with(|stack| {
        let mut stack = stack.borrow_mut();
        let Some(pass) = stack.last_mut() else { return };
        if pass.actors.len() >= MAX_ACTORS {
            pass.rejected = pass.rejected.saturating_add(1);
            return;
        }
        let sample = (|| {
            let player = crate::players::actor(actor);
            if let Some(player) = &player {
                crate::native_capes::remember(actor, player);
                crate::native_transform::observe_renderer(binding.dispatcher);
            }
            Some(ActorSample {
                token: pass.actors.len(),
                // This exact dispatcher does not consume R9 before clobbering
                // it. LTO callers can omit that otherwise unused argument.
                // Keep it diagnostic-only; the real world camera is sampled
                // separately and a missing target must not discard the actor.
                camera_target: floats(target, 30_000_000.),
                render_position: floats(position, 30_000_000.)?,
                render_rotation: floats(rotation, 360_000.)?,
                ignore_lighting: lit,
                animation: player
                    .as_ref()
                    .and_then(|_| crate::animation_pose::actor(actor, context)),
                cape_motion: None,
                cape_state: None,
                global_model: binding.global_model,
                player,
            })
        })();
        if let Some(sample) = sample {
            if let Some(entry) = pass.actors.iter_mut().find(|entry| entry.0 == actor) {
                let token = entry.1.token;
                entry.1 = ActorSample { token, ..sample };
            } else if actor != 0 {
                pass.actors.push((actor, sample));
            }
        } else {
            pass.rejected = pass.rejected.saturating_add(1);
        }
    });
}

unsafe extern "system" fn world_hook(renderer: usize, screen: usize, frame_object: usize) {
    if ACTIVE_HOOKS.load(Ordering::Acquire) != 0b111 {
        let original: WorldFn = unsafe { std::mem::transmute(WORLD.load(Ordering::Acquire)) };
        unsafe {
            original(renderer, screen, frame_object);
        }
        return;
    }
    let producer = crate::render_bridge::producer_frame();
    // Select/refresh the primary client level before actors are sampled. Server
    // levels tick too in single player and must not advance the client clock.
    let clock_before =
        std::panic::catch_unwind(|| observe_camera(renderer).as_ref().and_then(capture_clock))
            .ok()
            .flatten();
    let work_before = std::panic::catch_unwind(|| {
        crate::capture::frame_options()
            .0
            .then(|| crate::native_world_work::Sample::copy(renderer, frame_object))
            .flatten()
    })
    .ok()
    .flatten();
    let pushed = COLLECTING.with(|stack| {
        let mut stack = stack.borrow_mut();
        if stack.len() >= 8 {
            return false;
        }
        stack.push(Pass {
            scope: NEXT_PASS.fetch_add(1, Ordering::Relaxed) + 1,
            actors: Vec::new(),
            rejected: 0,
        });
        true
    });
    if !pushed {
        SKIPPED.with(|depth| depth.set(depth.get() + 1));
    }
    // SAFETY: installation publishes the correctly typed trampoline before
    // enabling this hook. All arguments are forwarded unchanged, exactly once.
    unsafe {
        let original: WorldFn = std::mem::transmute(WORLD.load(Ordering::Acquire));
        original(renderer, screen, frame_object);
    }
    if !pushed {
        SKIPPED.with(|depth| depth.set(depth.get() - 1));
    }
    if pushed {
        COLLECTING.with(|stack| {
            let pass = stack.borrow_mut().pop().expect("balanced world callback");
            let sequence = PASSES.fetch_add(1, Ordering::Relaxed) + 1;
            let mut camera = observe_camera(renderer);
            let clock_after = camera.as_ref().and_then(capture_clock);
            let camera_time = camera.as_ref().and_then(|camera| {
                let producer =
                    producer.filter(|key| crate::render_bridge::producer_frame() == Some(*key))?;
                let session = native_session(camera.epoch?);
                crate::native_camera_time::capture_sample(renderer, &session, producer, sequence)
            });
            let work = std::panic::catch_unwind(|| {
                work_before.and_then(|before| {
                    crate::native_world_work::Sample::copy(renderer, frame_object)
                        .and_then(|after| before.completed(after, sequence))
                })
            })
            .ok()
            .flatten();
            let shader_clock = camera.as_ref().and_then(|camera| {
                crate::native_shader_clock::snapshot(renderer, screen, camera, sequence, pass.scope)
            });
            if let Some(timing) = camera.as_mut().and_then(|c| c.renderer_timing.as_mut()) {
                timing["shaderClock"] = json!(shader_clock);
                timing["worldWork"] = json!(work);
                timing["captureClock"] = if clock_before == clock_after {
                    json!(clock_after)
                } else {
                    Value::Null
                };
                timing["cameraTime"] = json!(camera_time);
                timing["contextPartial"] = json!(
                    screen
                        .checked_add(8)
                        .and_then(|at| floats::<1>(at, 1.))
                        .map(|v| v[0])
                );
                timing["flipbooks"] = json!(crate::native_flipbook::snapshot());
                timing["shaderInputs"] = json!(crate::native_shader_inputs::sample(screen));
                timing["fogEvaluation"] = json!(crate::native_fog::sample(screen));
            }
            let mut actors: Vec<_> = pass.actors.into_iter().map(|entry| entry.1).collect();
            if let Some((epoch, partial, completed_tick)) = camera
                .as_ref()
                .and_then(|c| Some((c.epoch?, c.partial?, c.client_ticks?)))
            {
                for sample in &mut actors {
                    sample.cape_motion = sample.player.as_ref().and_then(|actor| {
                        crate::native_capes::sample(actor, epoch, partial, completed_tick)
                    });
                    sample.cape_state = sample.player.as_ref().and_then(|actor| {
                        crate::native_capes::snapshot(actor, epoch, completed_tick)
                    });
                }
            }
            if let Some(camera) = &camera {
                crate::render_bridge::record_world(
                    pass.scope,
                    producer,
                    crate::render_bridge::producer_frame(),
                    camera.view,
                    camera.projection,
                );
            }
            if let Ok(mut history) = snapshots().try_lock() {
                history.push(Observation {
                    scope: pass.scope,
                    actors,
                    camera,
                    rejected: pass.rejected,
                    sequence,
                    tick_count: TICKS.load(Ordering::Acquire),
                    at: Instant::now(),
                    depth: None,
                });
            }
        });
    }
}

fn capture_clock(camera: &CameraSample) -> Option<Value> {
    let session = format!("native_{}_{}", std::process::id(), camera.epoch?);
    crate::native_clock::capture_sample(&session, camera.client_ticks?, camera.partial?)
}

unsafe extern "system" fn actor_hook(
    dispatcher: usize,
    context: usize,
    actor: usize,
    target: usize,
    position: usize,
    rotation: usize,
    lit: u8,
) {
    if ACTIVE_HOOKS.load(Ordering::Acquire) != 0b111 {
        let original: ActorFn = unsafe { std::mem::transmute(ACTOR.load(Ordering::Acquire)) };
        unsafe {
            original(dispatcher, context, actor, target, position, rotation, lit);
        }
        return;
    }
    ACTORS.fetch_add(1, Ordering::Relaxed);
    let transform = crate::native_transform::begin_actor(actor, context);
    // SAFETY: same publication/lifetime rule as world_hook; preserve the native
    // void return ABI and all bits of the seventh stack-passed argument. The
    // native MOVZX consumes a byte; convert to a boolean only for our sample.
    unsafe {
        let original: ActorFn = std::mem::transmute(ACTOR.load(Ordering::Acquire));
        original(dispatcher, context, actor, target, position, rotation, lit);
    };
    let global_model = transform.finish();
    // Sample after native animation evaluation, while callback arguments and
    // renderer context are still alive. Rejected reads never suppress a body.
    let _ = std::panic::catch_unwind(|| {
        observe_actor(
            actor,
            context,
            target,
            position,
            rotation,
            lit != 0,
            ActorBinding {
                dispatcher,
                global_model,
            },
        )
    });
}

unsafe extern "system" fn tick_hook(level: usize) {
    if ACTIVE_HOOKS.load(Ordering::Acquire) != 0b111 {
        let original: TickFn = unsafe { std::mem::transmute(TICK.load(Ordering::Acquire)) };
        unsafe {
            original(level);
        }
        return;
    }
    let movement = crate::native_cape_movement::begin(level);
    let cape_phase = crate::native_cape_phase::begin(level);
    // SAFETY: exact supported callback ABI and permanent original trampoline.
    unsafe {
        let original: TickFn = std::mem::transmute(TICK.load(Ordering::Acquire));
        original(level);
    }
    // Count completed engine callbacks. Do not assume these are always 20 Hz or
    // use render frame count as cape/simulation time.
    TICKS.fetch_add(1, Ordering::Release);
    let _ = std::panic::catch_unwind(|| {
        let completed_tick = CLIENT_CLOCK
            .lock()
            .ok()
            .and_then(|mut clock| clock.tick(level).then_some(clock.ticks));
        if let Some(completed_tick) = completed_tick {
            crate::players::tick(level);
            let batch = movement.finish(completed_tick);
            let phases = cape_phase.finish(completed_tick);
            if batch.is_some() || phases.is_some() {
                crate::native_capes::tick_with_inputs(
                    level,
                    completed_tick,
                    batch.as_ref(),
                    phases.as_ref(),
                );
            } else {
                crate::native_capes::tick(level, completed_tick);
            }
            crate::native_packet::completed_client_tick(level, completed_tick);
            crate::native_clock::completed_tick(level, completed_tick);
        }
    });
}

fn u16_at(bytes: &[u8], at: usize) -> Result<u16, String> {
    Ok(u16::from_le_bytes(
        bytes
            .get(at..at + 2)
            .ok_or("short PE header")?
            .try_into()
            .unwrap(),
    ))
}
fn u32_at(bytes: &[u8], at: usize) -> Result<u32, String> {
    Ok(u32::from_le_bytes(
        bytes
            .get(at..at + 4)
            .ok_or("short PE header")?
            .try_into()
            .unwrap(),
    ))
}

fn image_layout(header: &[u8], profile: &Value) -> Result<(usize, usize), String> {
    if header.get(..2) != Some(b"MZ") {
        return Err("not a PE image".into());
    }
    let pe = u32_at(header, 60)? as usize;
    if pe > 2048
        || header.get(pe..pe + 4) != Some(b"PE\0\0")
        || u16_at(header, pe + 4)? != 0x8664
        || u16_at(header, pe + 24)? != 0x20b
    {
        return Err("engine hooks require a supported x64 PE32+ image".into());
    }
    let timestamp = u32_at(header, pe + 8)?;
    let image_size = u32_at(header, pe + 24 + 56)?;
    if Some(u64::from(timestamp)) != profile["timestamp"].as_u64()
        || Some(u64::from(image_size)) != profile["imageSize"].as_u64()
    {
        return Err(format!(
            "engine bindings target Bedrock {}; loaded image metadata differs (PE timestamp {timestamp}, image size {image_size}); version-specific hooks disabled",
            profile["productVersion"]
                .as_str()
                .unwrap_or("unknown pinned build")
        ));
    }
    let count = u16_at(header, pe + 6)? as usize;
    let optional = u16_at(header, pe + 20)? as usize;
    if count > 32 || optional > 512 {
        return Err("invalid section table".into());
    }
    for i in 0..count {
        let section = pe + 24 + optional + i * 40;
        if header.get(section..section + 8) == Some(b".text\0\0\0") {
            let rva = u32_at(header, section + 12)? as usize;
            let size = u32_at(header, section + 16)? as usize;
            if rva != 0x1000
                || size != 240_802_816
                || u32_at(header, section + 36)? & 0x20000000 == 0
            {
                return Err("unexpected executable section".into());
            }
            return Ok((rva, size));
        }
    }
    Err("no executable section".into())
}

fn resolve_loaded() -> Result<[usize; 3], String> {
    // SAFETY: null module name selects the current executable; handle is borrowed.
    let base = unsafe { GetModuleHandleW(None) }
        .map_err(|e| e.to_string())?
        .0 as usize;
    let mut header = [0u8; 4096];
    read(base, &mut header)?;
    let profile: Value = serde_json::from_str(PROFILE).map_err(|e| e.to_string())?;
    if profile["productVersion"].as_str() != Some(scene::PRODUCT_VERSION) {
        return Err("engine profile and scene-layout family differ; private hooks disabled".into());
    }
    let (rva, size) = image_layout(&header, &profile)?;
    let mut digest = Sha256::new()?;
    let mut buffer = vec![0u8; 1024 * 1024];
    for offset in (0..size).step_by(buffer.len()) {
        let length = buffer.len().min(size - offset);
        read(base + rva + offset, &mut buffer[..length])?;
        digest.update(&buffer[..length])?;
    }
    if hex(&digest.finish()?) != profile["textSha256"].as_str().ok_or("missing hash")? {
        return Err("executable section hash differs; engine hooks disabled".into());
    }
    crate::native_dimension::configure(base)?;
    // Default game-mode metadata is optional; direct actor modes still work
    // when this client's vtable/getter fingerprint cannot be established.
    let _ = crate::native_game_mode::configure(base);
    crate::native_shader_asset::configure();
    let image_size = usize::try_from(profile["imageSize"].as_u64().ok_or("missing image size")?)
        .map_err(|_| "engine image size overflow")?;
    let _ = CODE_IMAGE.set((base, rva, rva + size));
    crate::live_client::bind_profile(base, image_size)?;
    let global = profile["candidates"]["platformGlobal"]["targetRva"]
        .as_u64()
        .ok_or("missing platform global")? as usize;
    if global >= profile["imageSize"].as_u64().ok_or("missing image size")? as usize {
        return Err("platform global lies outside the image".into());
    }
    PLATFORM_GLOBAL.store(
        base.checked_add(global)
            .ok_or("platform address overflow")?,
        Ordering::Release,
    );
    // Complete entry/caller/constructor admission precedes every core detour.
    let entries = engine_entry_profile::entry_rvas(base, image_size)?;
    let mut targets = [0; 3];
    for (i, name) in ["renderWorld", "renderActor", "levelTick"]
        .iter()
        .enumerate()
    {
        let candidate = &profile["candidates"][name];
        let target = entries[i];
        if candidate["matches"] != 1
            || candidate["targetRva"].as_u64() != Some(target as u64)
            || target < rva
            || target >= rva + size
        {
            return Err("invalid exact-build hook target".into());
        }
        targets[i] = base.checked_add(target).ok_or("hook address overflow")?;
    }
    Ok(targets)
}

// Hooks and trampolines deliberately remain registered for the DLL lifetime.
// Never free a trampoline that an engine callback could still be executing.
unsafe fn install_targets(targets: [usize; 3]) -> Result<(), String> {
    let callbacks = [
        world_hook as *const () as usize,
        actor_hook as *const () as usize,
        tick_hook as *const () as usize,
    ];
    for (i, slot) in [&WORLD, &ACTOR, &TICK].iter().enumerate() {
        let mut trampoline = std::ptr::null_mut();
        // SAFETY: caller validated code addresses and callback signatures. A
        // newly created hook is disabled until its trampoline has been stored.
        let status = unsafe { MH_CreateHook(targets[i] as _, callbacks[i] as _, &mut trampoline) };
        if status != MH_STATUS::MH_OK {
            return Err(format!("create engine hook {i}: {status:?}"));
        }
        slot.store(trampoline as usize, Ordering::Release);
    }
    for (i, target) in targets.into_iter().enumerate() {
        // SAFETY: all three original trampolines have already been published.
        let status = unsafe { MH_EnableHook(target as _) };
        if status != MH_STATUS::MH_OK {
            return Err(format!(
                "enable engine hook {i}: {status:?}; earlier observers may be active"
            ));
        }
        ACTIVE_HOOKS.fetch_or(1 << i, Ordering::Release);
    }
    Ok(())
}

/// Install observers only for the independently inspected executable hash.
/// The caller must have initialized MinHook and keep this DLL loaded.
pub fn start(directory: PathBuf) -> Result<(), String> {
    let is_game = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_owned()))
        .is_some_and(|n| {
            n.to_string_lossy()
                .eq_ignore_ascii_case("Minecraft.Windows.exe")
        });
    if !is_game {
        return Ok(());
    }
    let result = resolve_loaded().and_then(|targets| {
        // SAFETY: exact-image hash and section bounds were checked above.
        unsafe {
            install_targets(targets)?;
            let base = GetModuleHandleW(None).map_err(|e| e.to_string())?.0 as usize;
            crate::render_bridge::install(base)?;
            crate::native_transform::install()?;
            crate::native_body::install()?;
            crate::native_clock::install()?;
            crate::native_render_clock::install()?;
            crate::native_camera_time::install()?;
            crate::native_fog::install()?;
            crate::native_flipbook::install()?;
            crate::native_camera::install()?;
            crate::native_camera_control::install()?;
            crate::native_packet::install()?;
            crate::native_world_work::install(base)?;
            crate::native_resource_packs::install()?;
            crate::probe_isolation::observe_errors(base)
        }
    });
    let installation = match &result {
        Ok(()) => json!({"state":"observing","build":scene::PRODUCT_VERSION}),
        Err(error) => {
            json!({"state":if ACTIVE_HOOKS.load(Ordering::Acquire)==0 {"disabled"}else{"partial"},"error":error})
        }
    };
    std::thread::spawn(move || {
        let report = directory.join(format!("arctic-bedrock-{}.engine.json", std::process::id()));
        loop {
            let mut value = installation.clone();
            value["worldPasses"] = json!(PASSES.load(Ordering::Relaxed));
            value["activeHookMask"] = json!(ACTIVE_HOOKS.load(Ordering::Acquire));
            value["actorCalls"] = json!(ACTORS.load(Ordering::Relaxed));
            value["completedTickCallbacks"] = json!(TICKS.load(Ordering::Acquire));
            value["weakLocalPlayer"] = crate::ecs::weak_status();
            value["skinReader"] = crate::native_skins::status();
            value["equipmentReader"] = crate::native_equipment::status();
            value["modelTransform"] = crate::native_transform::status();
            value["bodyDraw"] = crate::native_body::status();
            value["nativeClock"] = crate::native_clock::status();
            value["nativePacket"] = crate::native_packet::status();
            value["nativeTaskActivity"] = crate::native_task_activity::status();
            value["nativeRenderClock"] = crate::native_render_clock::status();
            value["nativeCameraTime"] = crate::native_camera_time::status();
            value["nativeFog"] = crate::native_fog::status();
            value["nativeGpuConstants"] = crate::native_gpu_constants::status();
            value["nativeShaderLayout"] = crate::native_shader_layout::status();
            value["nativeSceneTarget"] = crate::native_scene_target::status();
            value["nativeGraphicsState"] = crate::depth_tracker::graphics_state_status();
            value["nativeAvatarRecording"] = crate::native_avatar_record::status();
            value["nativeFramebuffer"] = crate::native_framebuffer::status();
            value["nativeFlipbook"] = crate::native_flipbook::status();
            value["resourcePackObserver"] = crate::native_resource_packs::status();
            value["nativeEmotePose"] = crate::native_emote_pose::status();
            value["nativeCameraObserver"] = crate::native_camera::status();
            value["nativeCameraControl"] = crate::native_camera_control::status();
            value["animationReader"] = crate::animation_pose::status();
            value["playerDirectory"] = crate::players::status();
            value["depthTracker"] = crate::depth_tracker::status();
            value["renderBridge"] = crate::render_bridge::status();
            value["nativeBatch"] = crate::native_batch::status();
            let code = PUBLISH_ERROR.load(Ordering::Relaxed);
            value["worldProducer"] = json!({"accepted":PUBLISHED.load(Ordering::Relaxed),"lastScope":PUBLISHED_SCOPE.load(Ordering::Relaxed),"lastSequence":PUBLISHED_SEQUENCE.load(Ordering::Relaxed),"lastResult":(code!=i32::MIN).then_some(code),"delivered":DELIVERED.load(Ordering::Relaxed),"deliveredSequence":DELIVERED_SEQUENCE.load(Ordering::Relaxed)});
            value["probeIsolation"] = crate::probe_isolation::status();
            let renderer = hudhook::hooks::dx12::diagnostics();
            value["renderer"] = json!({"presents":renderer.presents,"frames":renderer.frames,"renderErrors":renderer.render_errors,"resets":renderer.resets,"lastError":renderer.last_error,"lastReset":renderer.last_reset,"deviceRemovedReason":renderer.device_removed_reason,"lastPresentResult":renderer.last_present_result});
            value["cosmeticsBound"] = json!(false);
            value["deterministicVideoReady"] = json!(false);
            if let Ok(history) = snapshots().lock()
                && let Some(observation) = history.latest()
            {
                value["worldHistory"] = history.status();
                let age = observation.at.elapsed();
                value["ageMs"] = json!(age.as_millis() as u64);
                value["sequence"] = json!(observation.sequence);
                value["worldScope"] = json!(observation.scope);
                value["tickCountAtWorldEnd"] = json!(observation.tick_count);
                value["rejectedActors"] = json!(observation.rejected);
                if age < Duration::from_millis(500) {
                    value["actors"] = json!(observation.actors);
                    value["animationSamples"] = json!(
                        observation
                            .actors
                            .iter()
                            .filter_map(|a| a
                                .animation
                                .as_ref()
                                .map(|pose| json!({"token":a.token,"pose":pose})))
                            .take(2)
                            .collect::<Vec<_>>()
                    );
                    value["camera"] = json!(observation.camera);
                    value["attachmentSamples"] = json!(observation.camera.as_ref().and_then(|c| c.partial.map(|partial| (c,partial))).map(|(camera,partial)| {
                        observation.actors.iter().filter_map(|sample| {
                            let actor=sample.player.as_ref()?;
                            let player=crate::live_frame::player(actor,&camera.origin,partial,true,sample.animation.as_ref())?;
                            Some(json!({"token":sample.token,"availableParts":player["availableParts"],"partMatrices":player["partMatrices"]}))
                        }).take(2).collect::<Vec<_>>()
                    }));
                    if let Some(depth) = &observation.depth {
                        value["terrainDepth"] = json!({"width":depth.width,"height":depth.height,"pixels":depth.values.len()});
                    }
                }
            }
            let temporary = report.with_extension("json.tmp");
            if std::fs::write(&temporary, value.to_string()).is_ok() {
                let _ = std::fs::rename(&temporary, &report);
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    });
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use hudhook::mh::MH_Initialize;
    #[test]
    fn client_clock_ignores_integrated_server_ticks_and_resets_when_primary_level_changes() {
        let mut clock = ClientClock::default();
        assert!(!clock.tick(7));
        assert_eq!(clock.select(7, 20.), Some((0, 0.)));
        assert!(clock.tick(7));
        for _ in 0..10 {
            assert!(!clock.tick(8));
        }
        assert_eq!(clock.select(7, 20.), Some((1, 50.)));
        assert!(clock.tick(7));
        assert_eq!(clock.select(7, 20.), Some((2, 100.)));
        assert_eq!(clock.select(8, 20.), Some((0, 0.)));
        assert!(!clock.tick(7));
        assert!(clock.tick(8));
        assert_eq!(clock.select(8, 20.), Some((1, 50.)));
    }
    #[test]
    fn client_clock_does_not_rescale_completed_time_when_tick_rate_changes() {
        let mut clock = ClientClock::default();
        assert!(clock.select(7, f32::NAN).is_none());
        assert_eq!(clock.select(7, 20.), Some((0, 0.)));
        for _ in 0..20 {
            assert!(clock.tick(7));
        }
        assert_eq!(clock.select(7, 10.), Some((20, 1000.)));
        for _ in 0..10 {
            assert!(clock.tick(7));
        }
        assert_eq!(clock.select(7, 40.), Some((30, 2000.)));
        for _ in 0..40 {
            assert!(clock.tick(7));
        }
        assert_eq!(clock.select(7, 20.), Some((70, 3000.)));
    }
    static FORWARDED: AtomicUsize = AtomicUsize::new(0);
    #[inline(never)]
    unsafe extern "system" fn host_actor(
        a: usize,
        b: usize,
        c: usize,
        d: usize,
        e: usize,
        f: usize,
        lit: u8,
    ) {
        std::hint::black_box((a, b, c, d, e, f));
        FORWARDED.fetch_add(if lit != 0 { 1 } else { 1000 }, Ordering::Relaxed);
    }
    #[inline(never)]
    unsafe extern "system" fn host_tick(level: usize) {
        FORWARDED.fetch_add(std::hint::black_box(level), Ordering::Relaxed);
    }
    #[inline(never)]
    unsafe extern "system" fn host_world(a: usize, b: usize, c: usize) {
        let position = [a as f32, b as f32, c as f32];
        let rotation = [25f32, 30.];
        let actor: ActorFn = std::hint::black_box(host_actor);
        // SAFETY: test-owned callback arguments live through the call.
        unsafe {
            actor(
                a,
                b,
                9,
                0, // the pinned dispatcher's unused camera-target argument
                position.as_ptr() as usize,
                rotation.as_ptr() as usize,
                1,
            );
        };
        FORWARDED.fetch_add(1, Ordering::Relaxed);
    }
    #[test]
    fn real_detours_forward_abi_and_collect_world_tick_inputs() {
        // SAFETY: no graphics hooks or other MinHook users in this test process.
        let init = unsafe { MH_Initialize() };
        assert!(matches!(
            init,
            MH_STATUS::MH_OK | MH_STATUS::MH_ERROR_ALREADY_INITIALIZED
        ));
        // SAFETY: these are permanent test functions with the exact callback ABI.
        unsafe {
            install_targets([
                host_world as *const () as usize,
                host_actor as *const () as usize,
                host_tick as *const () as usize,
            ])
        }
        .unwrap();
        let tick: TickFn = std::hint::black_box(host_tick);
        let world: WorldFn = std::hint::black_box(host_world);
        // SAFETY: test callbacks accept these scalar arguments.
        unsafe {
            tick(7);
            world(1, 2, 3)
        };
        assert_eq!(FORWARDED.load(Ordering::Relaxed), 9);
        assert_eq!(TICKS.load(Ordering::Acquire), 1);
        let history = snapshots().lock().unwrap();
        let sample = history.latest().unwrap();
        assert_eq!(sample.actors.len(), 1);
        assert_eq!(sample.actors[0].render_position, [1., 2., 3.]);
        assert_eq!(sample.actors[0].render_rotation, [25., 30.]);
        assert!(sample.actors[0].camera_target.is_none());
        assert_eq!(sample.tick_count, 1);
        assert!(floats::<3>(1, 30_000_000.).is_none());
    }
    #[test]
    fn unknown_or_truncated_image_is_refused() {
        let profile: Value = serde_json::from_str(PROFILE).unwrap();
        for bytes in [vec![], vec![0; 4096], b"MZ".to_vec()] {
            assert!(image_layout(&bytes, &profile).is_err());
        }
        assert_eq!(profile["candidates"]["levelTick"]["matches"], 1);
    }

    #[test]
    fn image_header_must_match_machine_timestamp_size_and_code_section() {
        let profile: Value = serde_json::from_str(PROFILE).unwrap();
        let mut header = vec![0u8; 4096];
        header[..2].copy_from_slice(b"MZ");
        header[60..64].copy_from_slice(&128u32.to_le_bytes());
        header[128..132].copy_from_slice(b"PE\0\0");
        header[132..134].copy_from_slice(&0x8664u16.to_le_bytes());
        header[134..136].copy_from_slice(&1u16.to_le_bytes());
        header[136..140]
            .copy_from_slice(&(profile["timestamp"].as_u64().unwrap() as u32).to_le_bytes());
        header[148..150].copy_from_slice(&240u16.to_le_bytes());
        header[152..154].copy_from_slice(&0x20bu16.to_le_bytes());
        header[208..212]
            .copy_from_slice(&(profile["imageSize"].as_u64().unwrap() as u32).to_le_bytes());
        let section = 128 + 24 + 240;
        header[section..section + 8].copy_from_slice(b".text\0\0\0");
        header[section + 12..section + 16].copy_from_slice(&4096u32.to_le_bytes());
        header[section + 16..section + 20].copy_from_slice(&240_802_816u32.to_le_bytes());
        header[section + 36..section + 40].copy_from_slice(&0x20000000u32.to_le_bytes());
        assert_eq!(
            image_layout(&header, &profile).unwrap(),
            (4096, 240_802_816)
        );
        for at in [132, 136, 152, 208, section + 12, section + 16, section + 39] {
            let mut changed = header.clone();
            changed[at] ^= 0xff;
            assert!(
                image_layout(&changed, &profile).is_err(),
                "accepted modified header offset {at}"
            );
        }
    }

    #[test]
    fn guarded_camera_reads_copy_matrices_and_reject_bad_pointers_or_nan() {
        fn point(bytes: &mut [u8], at: usize, value: usize) {
            bytes[at..at + 8].copy_from_slice(&(value as u64).to_le_bytes());
        }
        let mut win_main = vec![0u8; 16];
        let mut platform = vec![0u8; 0x120];
        let mut holder = vec![0u8; 0x50];
        let mut game = vec![0u8; 0x1448];
        let mut renderer = vec![0u8; 0x470];
        let mut player_renderer = vec![0u8; 0x66c];
        let mut game_renderer = vec![0u8; 0x448];
        let win_main_ptr = win_main.as_ptr() as usize;
        point(&mut win_main, 8, platform.as_ptr() as usize);
        point(&mut platform, 0x118, holder.as_ptr() as usize);
        point(&mut holder, 0x48, game.as_ptr() as usize);
        point(&mut game, 0x1440, game_renderer.as_ptr() as usize);
        point(&mut renderer, 0x468, player_renderer.as_ptr() as usize);
        for (i, f) in [1f32, 2., 3.].iter().enumerate() {
            player_renderer[0x660 + i * 4..0x664 + i * 4].copy_from_slice(&f.to_le_bytes());
        }
        for base in [0x388, 0x408] {
            for i in [0, 5, 10, 15] {
                game_renderer[base + i * 4..base + i * 4 + 4].copy_from_slice(&1f32.to_le_bytes())
            }
        }
        let global = &win_main_ptr as *const usize as usize;
        let sample = camera_from(renderer.as_ptr() as usize, global).unwrap();
        assert_eq!(sample.origin, [1., 2., 3.]);
        assert_eq!(sample.view[0], 1.);
        assert_eq!(sample.projection[15], 1.);
        game_renderer[0x388..0x38c].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(camera_from(renderer.as_ptr() as usize, global).is_none());
        point(&mut game, 0x1440, 1);
        assert!(camera_from(renderer.as_ptr() as usize, global).is_none());
        assert_eq!(
            sample.view[0], 1.,
            "previous observation owns its copied floats"
        );
    }
}
