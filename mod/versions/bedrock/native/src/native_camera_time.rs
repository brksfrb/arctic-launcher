//! Controlled delta for the owned level-renderer camera's preRenderUpdate.
//! The native zero/negative-time branch otherwise retains its last positive delta.
use crate::native_scene_layout as scene;
use crate::world_hooks::{floats, pointer, read};
use serde_json::{Value, json};
use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};

type Update = unsafe extern "system" fn(usize, usize, usize);
static INSTALLED_BINDING: OnceLock<crate::world_hooks::CameraPreRenderBinding> = OnceLock::new();
static ENABLED: AtomicBool = AtomicBool::new(false);
static ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static CALLS: AtomicU64 = AtomicU64::new(0);
static APPLIED: AtomicU64 = AtomicU64::new(0);
struct State {
    previous: Option<(String, usize, u64, f32)>,
    last: Option<Value>,
    captured: Vec<(crate::render_bridge::FrameKey, Option<Captured>)>,
}
#[derive(Clone)]
struct Captured {
    renderer: usize,
    camera: usize,
    session: String,
    generation: u64,
    clock: Value,
    expected: [u32; 2],
}
fn same_clock(a: &Value, b: &Value) -> bool {
    a == b
        && ["partial", "ticksPerSecond"].iter().all(|key| {
            a[*key]
                .as_f64()
                .zip(b[*key].as_f64())
                .is_some_and(|(a, b)| (a as f32).to_bits() == (b as f32).to_bits())
        })
}
impl Captured {
    fn same(&self, other: &Self) -> bool {
        self.renderer == other.renderer
            && self.camera == other.camera
            && self.session == other.session
            && self.generation == other.generation
            && self.expected == other.expected
            && same_clock(&self.clock, &other.clock)
    }
}
impl State {
    fn record(&mut self, key: crate::render_bridge::FrameKey, value: Option<Captured>) {
        if let Some((_, previous)) = self.captured.iter_mut().find(|(old, _)| *old == key) {
            // A missing or different observation makes this producer frame
            // ambiguous. Later success cannot erase an earlier failure.
            if !previous
                .as_ref()
                .zip(value.as_ref())
                .is_some_and(|(a, b)| a.same(b))
            {
                *previous = None;
            }
        } else {
            if self.captured.len() >= 8 {
                self.captured.remove(0);
            }
            self.captured.push((key, value));
        }
    }
}
static STATE: Mutex<State> = Mutex::new(State {
    previous: None,
    last: None,
    captured: Vec::new(),
});

pub(crate) fn write(address: usize, bytes: &[u8]) -> bool {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut core::ffi::c_void;
        fn WriteProcessMemory(
            p: *mut core::ffi::c_void,
            address: *mut core::ffi::c_void,
            bytes: *const u8,
            size: usize,
            count: *mut usize,
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
fn planned(current: f32, previous: Option<f32>) -> Option<([f32; 2], [f32; 2])> {
    if !current.is_finite() || !(0. ..=1_000_000_000.).contains(&current) {
        return None;
    }
    let last = previous.unwrap_or(current);
    if !last.is_finite() || current < last || current - last > 1000. {
        return None;
    }
    Some(([0., last], [current - last, current]))
}
struct Prepared {
    session: String,
    camera: usize,
    current: f32,
    before: [f32; 2],
    expected: [f32; 2],
    generation: u64,
    renderer: usize,
    producer: Option<crate::render_bridge::FrameKey>,
    clock: Option<Value>,
}
fn prepare(camera: usize, screen: usize) -> Option<Prepared> {
    let clock = pointer(screen.checked_add(scene::screen::CLOCK)?)?;
    let renderer = clock.checked_sub(scene::game_renderer::CLOCK)?;
    let (session, seconds, _, generation) = crate::native_clock::render_control(renderer)?;
    if crate::native_clock::owned_player_renderer(renderer)? != camera {
        return None;
    }
    let current = floats::<1>(clock, 1_000_000_000.)?[0];
    if current.to_bits()
        != crate::native_render_clock::scaled_time(renderer, &session, generation, seconds)?
            .to_bits()
    {
        return None;
    }
    let before = floats::<2>(
        camera.checked_add(scene::player_renderer::FRAME_TIME)?,
        1_000_000_000.,
    )?;
    let producer = crate::render_bridge::producer_frame();
    let clock = crate::native_clock::capture_renderer(renderer);
    let mut state = STATE.lock().ok()?;
    let previous = state
        .previous
        .as_ref()
        .filter(|(s, c, g, _)| s == &session && *c == camera && *g == generation)
        .map(|(_, _, _, t)| *t);
    let Some((prepared, expected)) = planned(current, previous) else {
        state.last = Some(
            json!({"session":session,"secondsScaled":current,"verified":false,
            "error":"camera timeline moved backwards or exceeded its step limit; world reconstruction is required"}),
        );
        return None;
    };
    let bytes: Vec<_> = prepared.into_iter().flat_map(f32::to_le_bytes).collect();
    if !write(
        camera.checked_add(scene::player_renderer::FRAME_TIME)?,
        &bytes,
    ) || floats::<2>(
        camera.checked_add(scene::player_renderer::FRAME_TIME)?,
        1_000_000_000.,
    )?
    .map(f32::to_bits)
        != prepared.map(f32::to_bits)
    {
        let original: Vec<_> = before.into_iter().flat_map(f32::to_le_bytes).collect();
        let _ = write(
            camera.checked_add(scene::player_renderer::FRAME_TIME)?,
            &original,
        );
        return None;
    }
    state.previous = Some((session.clone(), camera, generation, current));
    Some(Prepared {
        session,
        camera,
        current,
        before,
        expected,
        generation,
        renderer,
        producer,
        clock,
    })
}
fn finished(prepared: Prepared) {
    let actual = prepared
        .camera
        .checked_add(scene::player_renderer::FRAME_TIME)
        .and_then(|at| floats::<2>(at, 1_000_000_000.));
    let verified =
        actual.is_some_and(|v| v.map(f32::to_bits) == prepared.expected.map(f32::to_bits));
    if verified {
        APPLIED.fetch_add(1, Ordering::Relaxed);
    }
    let capture = prepared
        .clock
        .as_ref()
        .filter(|clock| {
            verified
                && crate::render_bridge::producer_frame() == prepared.producer
                && clock["freezeSequence"].as_u64() == Some(prepared.generation)
                && crate::native_clock::capture_renderer(prepared.renderer)
                    .as_ref()
                    .is_some_and(|after| same_clock(clock, after))
        })
        .map(|clock| Captured {
            renderer: prepared.renderer,
            camera: prepared.camera,
            session: prepared.session.clone(),
            generation: prepared.generation,
            clock: clock.clone(),
            expected: prepared.expected.map(f32::to_bits),
        });
    if let Ok(mut state) = STATE.lock() {
        if let Some(key) = prepared.producer {
            state.record(key, capture);
        }
        state.last = Some(
            json!({"session":prepared.session,"secondsScaled":prepared.current,
            "generation":prepared.generation,
            "before":prepared.before,"expected":prepared.expected,"after":actual,"verified":verified,
            "error":(!verified).then_some("native camera delta output differed"),"nativeSimulationVerified":false}),
        );
        if !verified {
            state.previous = None;
        }
    }
}
/// Copy only an unambiguous successful camera timing observation from this
/// exact producer, still on its acknowledged held clock and native outputs.
/// The numeric world sequence binds the copy, not an engine lifetime owner.
pub(crate) fn capture_sample(
    renderer: usize,
    session: &str,
    producer: crate::render_bridge::FrameKey,
    world_sequence: u64,
) -> Option<Value> {
    if world_sequence == 0 || crate::render_bridge::producer_frame() != Some(producer) {
        return None;
    }
    let captured = STATE
        .try_lock()
        .ok()?
        .captured
        .iter()
        .find(|(key, _)| *key == producer)?
        .1
        .clone()?;
    let (current_session, seconds, _, generation) = crate::native_clock::render_control(renderer)?;
    if captured.renderer != renderer
        || captured.session != session
        || current_session != session
        || captured.generation != generation
        || crate::native_clock::owned_player_renderer(renderer)? != captured.camera
        || !same_clock(
            &captured.clock,
            &crate::native_clock::capture_renderer(renderer)?,
        )
    {
        return None;
    }
    let scaled = crate::native_render_clock::scaled_time(renderer, session, generation, seconds)?;
    let actual = floats::<2>(
        captured
            .camera
            .checked_add(scene::player_renderer::FRAME_TIME)?,
        1_000_000_000.,
    )?;
    if actual.map(f32::to_bits) != captured.expected
        || scaled.to_bits() != captured.expected[1]
        || crate::render_bridge::producer_frame() != Some(producer)
    {
        return None;
    }
    let still_captured = STATE
        .try_lock()
        .ok()?
        .captured
        .iter()
        .find(|(key, _)| *key == producer)
        .and_then(|(_, value)| value.as_ref())
        .is_some_and(|value| value.same(&captured));
    if !still_captured
        || !same_clock(
            &captured.clock,
            &crate::native_clock::capture_renderer(renderer)?,
        )
        || crate::render_bridge::producer_frame() != Some(producer)
    {
        return None;
    }
    Some(
        json!({"schema":1,"session":session,"worldSequence":world_sequence,"producerFrame":producer.number,
        "clock":captured.clock,"freezeSequence":generation,"secondsScaled":scaled,"expected":captured.expected.map(f32::from_bits),"observed":actual,
        "outputMatched":true,"nativeSimulationVerified":false,"deterministicVideoVerified":false}),
    )
}
unsafe extern "system" fn update(camera: usize, screen: usize, parameters: usize) {
    CALLS.fetch_add(1, Ordering::Relaxed);
    let prepared = std::panic::catch_unwind(|| prepare(camera, screen))
        .ok()
        .flatten();
    let controlled = screen
        .checked_add(scene::screen::CLOCK)
        .and_then(pointer)
        .and_then(|c| c.checked_sub(scene::game_renderer::CLOCK))
        .is_some_and(|r| crate::native_clock::owned_player_renderer(r) == Some(camera));
    if prepared.is_none()
        && controlled
        && let Some(key) = crate::render_bridge::producer_frame()
        && let Ok(mut state) = STATE.lock()
    {
        state.record(key, None);
    }
    if prepared.is_none()
        && !controlled
        && let Ok(mut state) = STATE.lock()
        && state
            .previous
            .as_ref()
            .is_some_and(|(_, c, _, _)| *c == camera)
    {
        state.previous = None;
    }
    let original: Update = unsafe { std::mem::transmute(ORIGINAL.load(Ordering::Acquire)) };
    unsafe { original(camera, screen, parameters) };
    if let Some(prepared) = prepared {
        let _ = std::panic::catch_unwind(|| finished(prepared));
    }
}
pub(crate) unsafe fn install() -> Result<(), String> {
    if !crate::native_clock::control_enabled() {
        return Ok(());
    }
    let base = crate::world_hooks::image_base().ok_or("camera time image unavailable")?;
    let binding = crate::world_hooks::camera_pre_render_binding()?;
    let target = base
        .checked_add(binding.rva)
        .ok_or("camera time address overflow")?;
    let mut bytes = [0; 19];
    read(target, &mut bytes)?;
    let mut expected = vec![
        0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x81, 0xec,
    ];
    expected.extend_from_slice(&binding.stack_bytes.to_le_bytes());
    if bytes.as_slice() != expected.as_slice() {
        return Err("camera pre-render update prologue differs".into());
    }
    INSTALLED_BINDING
        .set(binding)
        .map_err(|_| "camera pre-render installation already attempted")?;
    use hudhook::mh::{MH_CreateHook, MH_EnableHook, MH_STATUS};
    let mut trampoline = std::ptr::null_mut();
    let result = unsafe { MH_CreateHook(target as _, update as *const () as _, &mut trampoline) };
    if result != MH_STATUS::MH_OK {
        return Err(format!("create camera delta hook: {result:?}"));
    }
    ORIGINAL.store(trampoline as usize, Ordering::Release);
    let result = unsafe { MH_EnableHook(target as _) };
    if result != MH_STATUS::MH_OK {
        return Err(format!("enable camera delta hook: {result:?}"));
    }
    ENABLED.store(true, Ordering::Release);
    Ok(())
}
pub(crate) fn status() -> Value {
    json!({"installed":ENABLED.load(Ordering::Acquire),"hookRva":INSTALLED_BINDING.get().map(|b|b.rva),
        "calls":CALLS.load(Ordering::Relaxed),"applied":APPLIED.load(Ordering::Relaxed),
        "last":STATE.lock().ok().and_then(|s|s.last.clone()),
        "producerFrameCaptureImplemented":true,"captureFrameCapacity":8,"deterministicVideoReady":false})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn held_time_clears_stale_delta_and_new_timeline_does_not_reuse_old_wall_time() {
        assert_eq!(planned(39.25, None), Some(([0., 39.25], [0., 39.25])));
        assert_eq!(
            planned(39.25, Some(39.25)),
            Some(([0., 39.25], [0., 39.25]))
        );
        let (before, after) = planned(39.3, Some(39.25)).unwrap();
        assert_eq!(before, [0., 39.25]);
        assert_eq!(after, [39.3f32 - 39.25, 39.3]);
        assert!(planned(39., Some(40.)).is_none());
        assert!(planned(f32::NAN, None).is_none());
    }
    #[test]
    fn real_pre_render_detour_forwards_all_three_native_arguments_once() {
        static RECEIVED: Mutex<Option<[usize; 3]>> = Mutex::new(None);
        static COUNT: AtomicU64 = AtomicU64::new(0);
        #[inline(never)]
        unsafe extern "system" fn host(a: usize, b: usize, c: usize) {
            *RECEIVED.lock().unwrap() = Some([a, b, c]);
            COUNT.fetch_add(1, Ordering::Relaxed);
        }
        use hudhook::mh::{MH_CreateHook, MH_DisableHook, MH_EnableHook, MH_Initialize, MH_STATUS};
        assert!(matches!(
            unsafe { MH_Initialize() },
            MH_STATUS::MH_OK | MH_STATUS::MH_ERROR_ALREADY_INITIALIZED
        ));
        let target = host as *const () as usize;
        let mut trampoline = std::ptr::null_mut();
        assert_eq!(
            unsafe { MH_CreateHook(target as _, update as *const () as _, &mut trampoline) },
            MH_STATUS::MH_OK
        );
        ORIGINAL.store(trampoline as usize, Ordering::Release);
        assert_eq!(unsafe { MH_EnableHook(target as _) }, MH_STATUS::MH_OK);
        let call: Update = std::hint::black_box(host);
        unsafe { call(17, 23, 47) };
        assert_eq!(unsafe { MH_DisableHook(target as _) }, MH_STATUS::MH_OK);
        ORIGINAL.store(target, Ordering::Release);
        assert_eq!(*RECEIVED.lock().unwrap(), Some([17, 23, 47]));
        assert_eq!(COUNT.load(Ordering::Relaxed), 1);
    }
}
