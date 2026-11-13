//! Passive inspection of the exact build's deferred BGFX submission. A backend
//! submission ID identifies actual GPU recording; it is not a world-pass scope.
use crate::native_scene_layout::{
    encoder_submit as submit_layout, frame_view as view_layout, producer_frame as layout,
};
use crate::world_hooks::read;
use hudhook::mh::{MH_CreateHook, MH_EnableHook, MH_STATUS};
use serde_json::{Value, json};
use std::cell::Cell;
use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};

#[path = "native_backend_profile.rs"]
mod backend_profile;
#[path = "native_producer_frame_profile.rs"]
mod producer_profile;
#[path = "native_view_profile.rs"]
mod view_profile;
pub(crate) use backend_profile::{BackendBinding, BatchResetAbi};

static INSTALL: OnceLock<Result<(), String>> = OnceLock::new();
static BACKEND_ENABLED: AtomicBool = AtomicBool::new(false);

type Submit = unsafe extern "system" fn(usize, usize, usize, usize, usize, usize);
static ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static CALLS: AtomicU64 = AtomicU64::new(0);
static SAMPLE: Mutex<Option<(Instant, Value)>> = Mutex::new(None);
type SetTransform = unsafe extern "system" fn(usize, usize, usize, u8, usize, usize, usize, usize);
static TRANSFORM: AtomicUsize = AtomicUsize::new(0);
static TRANSFORM_ENABLED: AtomicBool = AtomicBool::new(false);
static VIEW_SNAPSHOT_READS: AtomicU64 = AtomicU64::new(0);
static TRANSFORM_CALLS: AtomicU64 = AtomicU64::new(0);
static WORLD_TRANSFORMS: AtomicU64 = AtomicU64::new(0);
static TRANSFORM_CALLERS: OnceLock<Vec<usize>> = OnceLock::new();
static DEFERRED_CALLERS: OnceLock<Vec<usize>> = OnceLock::new();
static FRAME_RANGES: Mutex<Vec<usize>> = Mutex::new(Vec::new());
static DIRECT_TRANSFORMS: AtomicU64 = AtomicU64::new(0);
static BASE: AtomicUsize = AtomicUsize::new(0);
static PRODUCER_READS: AtomicU64 = AtomicU64::new(0);
static PRODUCER_COPIES: AtomicU64 = AtomicU64::new(0);
static WORLD_FRAMES: Mutex<Vec<WorldFrame>> = Mutex::new(Vec::new());
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct FrameKey {
    pub(crate) address: usize,
    pub(crate) number: u32,
}
struct WorldFrame {
    key: FrameKey,
    scope: u64,
    ambiguous: bool,
    view: [f32; 16],
    projection: [f32; 16],
}

fn integer(address: usize) -> Option<u32> {
    let mut bytes = [0u8; 4];
    read(address, &mut bytes).ok()?;
    Some(u32::from_le_bytes(bytes))
}
fn producer_from_global(global: usize) -> Option<FrameKey> {
    use crate::world_hooks::pointer;
    let context = pointer(global)?;
    let submit = pointer(context.checked_add(layout::CONTEXT_SUBMIT)?)?;
    let render = pointer(context.checked_add(layout::CONTEXT_RENDER)?)?;
    let first = context.checked_add(layout::FIRST_FRAME)?;
    let second = first.checked_add(layout::FRAME_STRIDE)?;
    if submit == render || ![first, second].contains(&submit) || ![first, second].contains(&render)
    {
        return None;
    }
    let encoder = pointer(context.checked_add(layout::CONTEXT_ENCODER)?)?;
    if pointer(encoder)? != submit {
        return None;
    }
    let number = integer(submit.checked_add(layout::FRAME_NUMBER)?)?;
    if pointer(global)? != context
        || pointer(context.checked_add(layout::CONTEXT_SUBMIT)?)? != submit
        || pointer(context.checked_add(layout::CONTEXT_RENDER)?)? != render
        || pointer(context.checked_add(layout::CONTEXT_ENCODER)?)? != encoder
        || pointer(encoder)? != submit
        || integer(submit.checked_add(layout::FRAME_NUMBER)?)? != number
    {
        return None;
    }
    Some(FrameKey {
        address: submit,
        number,
    })
}
fn backend_frame(address: usize) -> Option<FrameKey> {
    let global = producer_profile::base()?.checked_add(layout::CONTEXT_GLOBAL)?;
    let context = crate::world_hooks::pointer(global)?;
    let first = context.checked_add(layout::FIRST_FRAME)?;
    if address != first && address != first.checked_add(layout::FRAME_STRIDE)? {
        return None;
    }
    let number = integer(address.checked_add(layout::FRAME_NUMBER)?)?;
    if crate::world_hooks::pointer(global)? != context
        || integer(address.checked_add(layout::FRAME_NUMBER)?)? != number
    {
        return None;
    }
    Some(FrameKey { address, number })
}

fn rectangle_copy(bytes: &[u8]) -> Option<[u16; 4]> {
    let values = bytes.get(view_layout::RECTANGLE..view_layout::RECTANGLE.checked_add(8)?)?;
    Some(std::array::from_fn(|i| {
        u16::from_le_bytes(values[i * 2..i * 2 + 2].try_into().expect("two bytes"))
    }))
}
fn matrix_copy(bytes: &[u8], at: usize) -> Option<[f32; 16]> {
    let values = bytes.get(at..at.checked_add(64)?)?;
    let matrix: [f32; 16] = std::array::from_fn(|i| {
        f32::from_le_bytes(values[i * 4..i * 4 + 4].try_into().expect("four bytes"))
    });
    matrix
        .iter()
        .all(|v| v.is_finite() && v.abs() <= 1_000_000.)
        .then_some(matrix)
}
pub(crate) fn scope_for_view(key: FrameKey, index: u16) -> Option<u64> {
    if view_profile::current().is_none()
        || usize::from(index) >= view_layout::COUNT
        || integer(key.address.checked_add(layout::FRAME_NUMBER)?)? != key.number
    {
        return None;
    }
    let (scope, expected_view, expected_projection) = WORLD_FRAMES
        .try_lock()
        .ok()?
        .iter()
        .find(|f| f.key == key && !f.ambiguous)
        .map(|f| (f.scope, f.view, f.projection))?;
    if !crate::world_hooks::has_snapshot(scope) {
        return None;
    }
    let address = key
        .address
        .checked_add(view_layout::FRAME_BASE)?
        .checked_add(usize::from(index).checked_mul(view_layout::STRIDE)?)?;
    let mut bytes = vec![0u8; view_layout::PROJECTION.checked_add(64)?];
    read(address, &mut bytes).ok()?;
    let rect = rectangle_copy(&bytes)?;
    let width = integer(key.address.checked_add(view_layout::FRAME_WIDTH)?)?;
    let height = integer(key.address.checked_add(view_layout::FRAME_HEIGHT)?)?;
    if rect[0] != 0 || rect[1] != 0 || u32::from(rect[2]) != width || u32::from(rect[3]) != height {
        return None;
    }
    let view = matrix_copy(&bytes, view_layout::VIEW)?;
    let projection = matrix_copy(&bytes, view_layout::PROJECTION)?;
    let mut after = vec![0u8; bytes.len()];
    read(address, &mut after).ok()?;
    if bytes != after {
        return None;
    }
    let stable = WORLD_FRAMES.try_lock().ok()?.iter().any(|frame| {
        frame.key == key
            && !frame.ambiguous
            && frame.scope == scope
            && frame.view == expected_view
            && frame.projection == expected_projection
    });
    if !stable {
        return None;
    }
    VIEW_SNAPSHOT_READS.fetch_add(1, Ordering::Relaxed);
    let matches =
        |a: &[f32; 16], b: &[f32; 16]| a.iter().zip(b).all(|(x, y)| (x - y).abs() <= 0.0001);
    (matches(&view, &expected_view)
        && matches(&projection, &expected_projection)
        && crate::world_hooks::has_snapshot(scope)
        && integer(key.address.checked_add(view_layout::FRAME_WIDTH)?)? == width
        && integer(key.address.checked_add(view_layout::FRAME_HEIGHT)?)? == height
        && integer(key.address.checked_add(layout::FRAME_NUMBER)?)? == key.number)
        .then_some(scope)
}
pub(crate) fn producer_frame() -> Option<FrameKey> {
    let base = producer_profile::base()?;
    PRODUCER_READS.fetch_add(1, Ordering::Relaxed);
    // Constructor, actual frame/swap call, frame-number reset/increment and
    // encoder frame rebinding are admitted independently of backend detours.
    let result = producer_from_global(base.checked_add(layout::CONTEXT_GLOBAL)?)?;
    PRODUCER_COPIES.fetch_add(1, Ordering::Relaxed);
    Some(result)
}
pub(crate) fn record_world(
    scope: u64,
    start: Option<FrameKey>,
    end: Option<FrameKey>,
    view: [f32; 16],
    projection: [f32; 16],
) {
    if let Ok(mut frames) = WORLD_FRAMES.try_lock() {
        let stable = start.is_some() && start == end;
        for key in [start, end.filter(|key| Some(*key) != start)]
            .into_iter()
            .flatten()
        {
            if let Some(previous) = frames.iter_mut().find(|frame| frame.key == key) {
                previous.ambiguous |= !stable || previous.scope != scope;
            } else {
                if frames.len() == 8 {
                    frames.remove(0);
                }
                frames.push(WorldFrame {
                    key,
                    scope,
                    ambiguous: !stable,
                    view,
                    projection: crate::live_frame::directx_clip_projection(&projection),
                });
            }
        }
    }
}
/// Resolve only a stable world-to-producer binding already observed by the
/// native world callback. Callers must not infer it from submission order.
pub(crate) fn producer_for_scope(scope: u64) -> Option<FrameKey> {
    let frames = WORLD_FRAMES.try_lock().ok()?;
    let mut matches = frames.iter().filter(|f| f.scope == scope && !f.ambiguous);
    let frame = matches.next()?;
    if matches.next().is_some()
        || integer(frame.key.address.checked_add(layout::FRAME_NUMBER)?)? != frame.key.number
    {
        return None;
    }
    Some(frame.key)
}
thread_local! { static SUBMISSION: Cell<Option<u64>> = const {Cell::new(None)}; }

pub(crate) fn current_submission() -> Option<u64> {
    SUBMISSION.try_with(Cell::get).ok().flatten()
}
struct Restore(Option<u64>);
impl Drop for Restore {
    fn drop(&mut self) {
        let _ = SUBMISSION.try_with(|s| s.set(self.0));
    }
}

fn views(frame: usize) -> Option<Value> {
    let integer = |offset: usize| -> Option<u32> {
        let mut bytes = [0u8; 4];
        read(frame.checked_add(offset)?, &mut bytes).ok()?;
        Some(u32::from_le_bytes(bytes))
    };
    // Native Frame::m_debug and cache/resource accesses corroborate this
    // published Frame layout, independently of the backend's large context.
    let width = integer(view_layout::FRAME_WIDTH)?;
    let height = integer(view_layout::FRAME_HEIGHT)?;
    let draws = integer(submit_layout::ITEMS_COUNT)?;
    let number = integer(layout::FRAME_NUMBER)?;
    if !(1..=16384).contains(&width)
        || !(1..=16384).contains(&height)
        || usize::try_from(draws).ok()? > submit_layout::ITEM_CAPACITY
    {
        return None;
    }
    if let Ok(mut frames) = FRAME_RANGES.try_lock()
        && !frames.contains(&frame)
    {
        if frames.len() == 8 {
            frames.remove(0);
        }
        frames.push(frame);
    }
    // The selected native swap copies this complete View array into Frame.
    // Only owned copies of rectangle and base view/projection are interpreted.
    let array_bytes = view_layout::COUNT.checked_mul(view_layout::STRIDE)?;
    if array_bytes > 128 * 1024 || view_layout::STRIDE < view_layout::PROJECTION.checked_add(64)? {
        return None;
    }
    let address = frame.checked_add(view_layout::FRAME_BASE)?;
    let mut raw = vec![0u8; array_bytes];
    read(address, &mut raw).ok()?;
    let camera = crate::world_hooks::diagnostic_camera_matrices();
    let matched = WORLD_FRAMES.try_lock().ok().and_then(|frames| {
        frames
            .iter()
            .find(|w| {
                w.key
                    == FrameKey {
                        address: frame,
                        number,
                    }
                    && !w.ambiguous
            })
            .map(|w| (w.scope, w.view, w.projection))
    });
    let mut result = Vec::new();
    let mut perspective = 0;
    for (id, b) in raw.chunks_exact(view_layout::STRIDE).enumerate() {
        let rect = rectangle_copy(b)?;
        if !(64..=16384).contains(&rect[2]) || !(64..=16384).contains(&rect[3]) {
            continue;
        }
        let Some(view) = matrix_copy(b, view_layout::VIEW) else {
            continue;
        };
        let Some(projection) = matrix_copy(b, view_layout::PROJECTION) else {
            continue;
        };
        if projection[11].abs() < 0.5 || projection[5].abs() < 0.0001 {
            continue;
        }
        perspective += 1;
        if result.len() < 16 {
            let matches = |a: &[f32; 16], b: &[f32; 16]| {
                a.iter().zip(b).all(|(x, y)| (x - y).abs() <= 0.0001)
            };
            result.push(
                json!({"id":id,"rect":rect,"view":view,"projection":projection,
                "worldViewMatches":camera.as_ref().map(|c|matches(&view,&c.0)),
                "ownedFrameViewMatches":matched.as_ref().map(|w|matches(&view,&w.1)),
                "ownedFrameProjectionMatches":matched.as_ref().map(|w|matches(&projection,&w.2)),
                "worldProjectionDepthConvertedMatches":camera.as_ref().map(|c|matches(&projection,&crate::live_frame::directx_clip_projection(&c.1))),
                "worldProjectionMatches":camera.as_ref().map(|c|matches(&projection,&c.1))}),
            );
        }
    }
    let mut after = vec![0u8; raw.len()];
    read(address, &mut after).ok()?;
    if raw != after
        || integer(layout::FRAME_NUMBER)? != number
        || integer(submit_layout::ITEMS_COUNT)? != draws
        || integer(view_layout::FRAME_WIDTH)? != width
        || integer(view_layout::FRAME_HEIGHT)? != height
    {
        return None;
    }
    VIEW_SNAPSHOT_READS.fetch_add(1, Ordering::Relaxed);
    Some(
        json!({"frameNumber":number,"width":width,"height":height,"renderItems":draws,"matchedWorldScope":matched.map(|w|w.0),"matchedSnapshotAvailable":matched.map(|w|crate::world_hooks::has_snapshot(w.0)),
        "perspectiveViews":perspective,"views":result,"worldScopeAuthorized":false}),
    )
}

unsafe extern "system" fn set_transform(
    a: usize,
    b: usize,
    c: usize,
    d: u8,
    e: usize,
    f: usize,
    g: usize,
    h: usize,
) {
    let original: SetTransform = unsafe { std::mem::transmute(TRANSFORM.load(Ordering::Acquire)) };
    unsafe { original(a, b, c, d, e, f, g, h) };
    TRANSFORM_CALLS.fetch_add(1, Ordering::Relaxed);
    let _ = std::panic::catch_unwind(|| {
        if crate::world_hooks::current_pass().is_some() {
            WORLD_TRANSFORMS.fetch_add(1, Ordering::Relaxed);
            if let Ok(frames) = FRAME_RANGES.try_lock()
                && frames.iter().any(|frame| {
                    a.checked_sub(*frame).is_some_and(|offset| {
                        offset
                            .checked_sub(view_layout::FRAME_BASE)
                            .is_some_and(|slot| {
                                view_layout::COUNT
                                    .checked_mul(view_layout::STRIDE)
                                    .is_some_and(|size| slot < size)
                                    && slot.is_multiple_of(view_layout::STRIDE)
                            })
                    })
                })
            {
                DIRECT_TRANSFORMS.fetch_add(1, Ordering::Relaxed);
            }
            TRANSFORM_CALLERS.get_or_init(crate::world_hooks::game_callers);
        } else if DEFERRED_CALLERS.get().is_none()
            && crate::world_hooks::diagnostic_camera_matrices().is_some()
        {
            // Recent activity gates a single diagnostic stack, never a scope.
            DEFERRED_CALLERS.get_or_init(crate::world_hooks::game_callers);
        }
    });
}

unsafe fn install_transform(target: usize) -> Result<(), String> {
    let mut original = std::ptr::null_mut();
    let created =
        unsafe { MH_CreateHook(target as _, set_transform as *const () as _, &mut original) };
    if created != MH_STATUS::MH_OK {
        return Err(format!("create view observer: {created:?}"));
    }
    TRANSFORM.store(original as usize, Ordering::Release);
    let enabled = unsafe { MH_EnableHook(target as _) };
    if enabled != MH_STATUS::MH_OK {
        return Err(format!("enable view observer: {enabled:?}"));
    }
    TRANSFORM_ENABLED.store(true, Ordering::Release);
    Ok(())
}

unsafe extern "system" fn submit(a: usize, b: usize, c: usize, d: usize, e: usize, f: usize) {
    if !BACKEND_ENABLED.load(Ordering::Acquire) {
        let original: Submit = unsafe { std::mem::transmute(ORIGINAL.load(Ordering::Acquire)) };
        unsafe { original(a, b, c, d, e, f) };
        return;
    }
    let sequence = CALLS.fetch_add(1, Ordering::Relaxed) + 1;
    let _restore = Restore(SUBMISSION.with(|s| s.replace(Some(sequence))));
    let _batch = crate::native_batch::begin_with_renderer(backend_frame(c), a, b);
    let _ = std::panic::catch_unwind(|| {
        let due = SAMPLE.try_lock().is_ok_and(|s| {
            s.as_ref()
                .is_none_or(|(at, _)| at.elapsed() >= Duration::from_millis(250))
        });
        if due
            // Both admitted entries save R8 (Frame) at backend stack +0x168.
            // RDX is the separate renderer context.
            && let Some(mut sample) = views(c)
            && let Ok(mut slot) = SAMPLE.try_lock()
        {
            sample["submission"] = json!(sequence);
            *slot = Some((Instant::now(), sample));
        }
    });
    // Both admitted native callers supply six pointer-width arguments, with
    // arguments five/six at entry RSP+0x28/+0x30. Forward all six unchanged.
    let original: Submit = unsafe { std::mem::transmute(ORIGINAL.load(Ordering::Acquire)) };
    unsafe { original(a, b, c, d, e, f) };
}

unsafe fn install_target(target: usize) -> Result<(), String> {
    let mut original = std::ptr::null_mut();
    let created = unsafe { MH_CreateHook(target as _, submit as *const () as _, &mut original) };
    if created != MH_STATUS::MH_OK {
        return Err(format!("create backend observer: {created:?}"));
    }
    ORIGINAL.store(original as usize, Ordering::Release);
    let enabled = unsafe { MH_EnableHook(target as _) };
    if enabled != MH_STATUS::MH_OK {
        return Err(format!("enable backend observer: {enabled:?}"));
    }
    BACKEND_ENABLED.store(true, Ordering::Release);
    Ok(())
}

/// Preflight metadata readers without creating or enabling any backend hook.
pub(crate) fn install_producer(base: usize) -> Result<(), String> {
    producer_profile::install(base)
}
pub(crate) fn backend_binding(base: usize) -> Result<BackendBinding, String> {
    backend_profile::binding(base)
}
pub(crate) fn current_backend_binding() -> Option<BackendBinding> {
    backend_profile::current()
}

/// Caller must have verified the complete pinned executable's text hash.
pub(crate) unsafe fn install(base: usize) -> Result<(), String> {
    install_producer(base)?;
    let view_binding = view_profile::binding(base)?;
    let backend = backend_binding(base)?;
    crate::native_framebuffer::preflight(base)?;
    crate::depth_tracker::preflight_native_queue(base)?;
    INSTALL
        .get_or_init(|| {
            let target = base
                .checked_add(backend.submit_rva)
                .ok_or("backend address overflow")?;
            unsafe {
                install_target(target)?;
            }
            unsafe {
                if let Some(rva) = view_binding.setter_rva {
                    install_transform(
                        base.checked_add(rva)
                            .ok_or("view observer address overflow")?,
                    )?;
                }
                crate::native_batch::install(base)?;
                crate::native_framebuffer::install(base)?;
                crate::depth_tracker::observe_native_queue(base)?;
            }
            BASE.store(base, Ordering::Release);
            Ok(())
        })
        .clone()
}
pub(crate) fn status() -> Value {
    let sample = SAMPLE.try_lock().ok().and_then(|s| {
        s.as_ref()
            .map(|(at, v)| (at.elapsed().as_millis() as u64, v.clone()))
    });
    json!({"installed":BACKEND_ENABLED.load(Ordering::Acquire),"calls":CALLS.load(Ordering::Relaxed),
        "groupInstalled":BASE.load(Ordering::Acquire)!=0,"backendEntrySourceImplemented":true,
        "backendRva":backend_profile::current().map(|binding|binding.submit_rva),
        "producerReaderInstalled":producer_profile::base().is_some(),"producerReaderSourceImplemented":true,
        "viewReaderInstalled":view_profile::current().is_some(),"viewReaderSourceImplemented":true,
        "viewObservation":view_profile::current().map(|binding|binding.observation),
        "viewSnapshotReads":VIEW_SNAPSHOT_READS.load(Ordering::Relaxed),
        "sampleAgeMs":sample.as_ref().map(|s|s.0),"sample":sample.map(|s|s.1),
        "producerFrames":{"reads":PRODUCER_READS.load(Ordering::Relaxed),"copies":PRODUCER_COPIES.load(Ordering::Relaxed)},
        "viewTransforms":{"installed":TRANSFORM_ENABLED.load(Ordering::Acquire),"calls":TRANSFORM_CALLS.load(Ordering::Relaxed),"withinWorld":WORLD_TRANSFORMS.load(Ordering::Relaxed),"directFrameMatches":DIRECT_TRANSFORMS.load(Ordering::Relaxed),"gameCallerRvas":TRANSFORM_CALLERS.get(),"deferredGameCallerRvas":DEFERRED_CALLERS.get()}})
}

#[cfg(test)]
mod tests {
    use super::*;
    static FORWARDED: Mutex<Option<[usize; 7]>> = Mutex::new(None);
    #[test]
    fn crossing_or_multiple_world_passes_never_authorize_a_unique_frame() {
        let a = FrameKey {
            address: 0x123456,
            number: 9876,
        };
        let b = FrameKey {
            address: 0x654321,
            number: 9877,
        };
        record_world(123, Some(a), Some(a), [0.; 16], [0.; 16]);
        assert!(
            !WORLD_FRAMES
                .lock()
                .unwrap()
                .iter()
                .find(|f| f.key == a)
                .unwrap()
                .ambiguous
        );
        record_world(124, Some(a), Some(b), [0.; 16], [0.; 16]);
        let frames = WORLD_FRAMES.lock().unwrap();
        assert!(frames.iter().find(|f| f.key == a).unwrap().ambiguous);
        assert!(frames.iter().find(|f| f.key == b).unwrap().ambiguous);
        drop(frames);
        record_world(125, Some(b), Some(b), [0.; 16], [0.; 16]);
        assert!(
            WORLD_FRAMES
                .lock()
                .unwrap()
                .iter()
                .find(|f| f.key == b)
                .unwrap()
                .ambiguous
        );
    }
    #[test]
    fn producer_frame_requires_embedded_frame_ownership_and_matching_encoder() {
        let mut context = vec![0u8; layout::CONTEXT_SUBMIT + 8];
        let base = context.as_ptr() as usize;
        let first = base + layout::FIRST_FRAME;
        let second = first + layout::FRAME_STRIDE;
        let mut encoder = [first];
        let set = |bytes: &mut [u8], at: usize, value: usize| {
            bytes[at..at + 8].copy_from_slice(&(value as u64).to_le_bytes())
        };
        set(&mut context, layout::CONTEXT_SUBMIT, first);
        set(&mut context, layout::CONTEXT_RENDER, second);
        set(
            &mut context,
            layout::CONTEXT_ENCODER,
            encoder.as_ptr() as usize,
        );
        let number_at = layout::FIRST_FRAME + layout::FRAME_NUMBER;
        context[number_at..number_at + 4].copy_from_slice(&456u32.to_le_bytes());
        let global = [base];
        let frame = producer_from_global(global.as_ptr() as usize).unwrap();
        assert_eq!(frame.address, first);
        assert_eq!(frame.number, 456);
        encoder[0] = second;
        std::hint::black_box(&encoder);
        assert!(producer_from_global(global.as_ptr() as usize).is_none());
        encoder[0] = first;
        std::hint::black_box(&encoder);
        set(&mut context, layout::CONTEXT_SUBMIT, second);
        std::hint::black_box(&context);
        assert!(producer_from_global(global.as_ptr() as usize).is_none());
    }
    #[inline(never)]
    unsafe extern "system" fn transform_host(
        a: usize,
        b: usize,
        c: usize,
        d: u8,
        e: usize,
        f: usize,
        g: usize,
        h: usize,
    ) {
        unsafe {
            *(a as *mut [usize; 7]) = std::hint::black_box([b, c, usize::from(d), e, f, g, h])
        };
    }
    #[test]
    fn view_observer_preserves_flags_and_all_four_stack_arguments() {
        let init = unsafe { hudhook::mh::MH_Initialize() };
        assert!(matches!(
            init,
            MH_STATUS::MH_OK | MH_STATUS::MH_ERROR_ALREADY_INITIALIZED
        ));
        unsafe { install_transform(transform_host as *const () as usize).unwrap() };
        let call: SetTransform = std::hint::black_box(transform_host);
        let mut output = [0usize; 7];
        unsafe {
            call(
                output.as_mut_ptr() as usize,
                2,
                3,
                0xa5,
                5,
                6,
                7,
                0x1234567890,
            )
        };
        assert_eq!(output, [2, 3, 0xa5, 5, 6, 7, 0x1234567890]);
    }
    #[inline(never)]
    unsafe extern "system" fn host(a: usize, b: usize, c: usize, d: usize, e: usize, f: usize) {
        *FORWARDED.lock().unwrap() = Some(std::hint::black_box([
            a,
            b,
            c,
            d,
            e,
            f,
            current_submission().unwrap_or(0) as usize,
        ]));
    }
    #[test]
    fn backend_observer_preserves_all_register_and_stack_arguments_and_restores_scope() {
        let init = unsafe { hudhook::mh::MH_Initialize() };
        assert!(matches!(
            init,
            MH_STATUS::MH_OK | MH_STATUS::MH_ERROR_ALREADY_INITIALIZED
        ));
        unsafe { install_target(host as *const () as usize).unwrap() };
        let call: Submit = std::hint::black_box(host);
        let mut frame = vec![0u8; layout::FRAME_NUMBER + 4];
        for (offset, value) in [
            (view_layout::FRAME_WIDTH, 640u32),
            (view_layout::FRAME_HEIGHT, 360),
            (submit_layout::ITEMS_COUNT, 17),
            (layout::FRAME_NUMBER, 1234),
        ] {
            frame[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        let rect = view_layout::FRAME_BASE + view_layout::RECTANGLE;
        frame[rect + 4..rect + 6].copy_from_slice(&640u16.to_le_bytes());
        frame[rect + 6..rect + 8].copy_from_slice(&360u16.to_le_bytes());
        for (offset, indices) in [
            (
                view_layout::FRAME_BASE + view_layout::VIEW,
                &[0usize, 5, 10, 15][..],
            ),
            (
                view_layout::FRAME_BASE + view_layout::PROJECTION,
                &[0usize, 5, 10, 11][..],
            ),
        ] {
            for i in indices {
                frame[offset + i * 4..offset + i * 4 + 4].copy_from_slice(&1f32.to_le_bytes());
            }
        }
        let address = frame.as_ptr() as usize;
        unsafe { call(11, 0, address, 44, 0x55667788, 0x1122334455667788) };
        let got = FORWARDED.lock().unwrap().unwrap();
        assert_eq!(
            &got[..6],
            &[11, 0, address, 44, 0x55667788, 0x1122334455667788]
        );
        assert!(got[6] > 0);
        assert_eq!(current_submission(), None);
        let sample = status()["sample"].clone();
        assert_eq!(sample["frameNumber"], 1234);
        assert_eq!(sample["perspectiveViews"], 1);
        assert_eq!(sample["worldScopeAuthorized"], false);
        assert!(views(0).is_none());
        frame[submit_layout::ITEMS_COUNT..submit_layout::ITEMS_COUNT + 4]
            .copy_from_slice(&65537u32.to_le_bytes());
        std::hint::black_box(&frame);
        assert!(views(address).is_none());
    }
}
