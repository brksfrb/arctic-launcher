//! Renderer clock sampled from the owned primary client's controlled native time.
//! This does not yet control every world simulation/texture clock or replay state.
use crate::native_scene_layout as scene;
use crate::world_hooks::{floats, read};
use serde_json::{Value, json};
use std::cell::Cell;
use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicU64, AtomicUsize, Ordering},
};

type MakeContext = unsafe extern "system" fn(usize, usize, f32) -> usize;
static CONTEXT_BINDING: OnceLock<crate::world_hooks::ScreenContextBinding> = OnceLock::new();
static ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static CALLS: AtomicU64 = AtomicU64::new(0);
static OVERRIDES: AtomicU64 = AtomicU64::new(0);
static WORLD_CALLS: AtomicU64 = AtomicU64::new(0);
static LAST: Mutex<Option<Value>> = Mutex::new(None);
#[path = "render_timeline.rs"]
mod epoch;
use epoch::Timeline;
#[derive(Clone)]
struct TotalAnchor {
    replay: f64,
    native: f64,
    wrapped: f64,
}
impl TotalAnchor {
    fn sample(&self, replay: f64) -> Option<(f64, f64)> {
        let delta = replay - self.replay;
        if !replay.is_finite() || delta < 0. {
            return None;
        }
        let mapped = self.native + delta;
        let wrapped = total_time(mapped, 3600.)?;
        Some((mapped, if delta == 0. { self.wrapped } else { wrapped }))
    }
}
#[derive(Clone)]
struct Control {
    session: String,
    renderer: usize,
    generation: u64,
    timeline: Timeline,
    total: Option<TotalAnchor>,
}
impl Control {
    fn matches(&self, renderer: usize, session: &str, generation: u64) -> bool {
        self.renderer == renderer && self.session == session && self.generation == generation
    }
}
static PREVIOUS: Mutex<Option<Control>> = Mutex::new(None);
type Fmod = unsafe extern "system" fn(f64, f64) -> f64;
static FMOD_BINDING: OnceLock<crate::world_hooks::FmodBinding> = OnceLock::new();
static FMOD_ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static TOTAL_OVERRIDES: AtomicU64 = AtomicU64::new(0);
static LAST_TOTAL: Mutex<Option<Value>> = Mutex::new(None);
static DELTA_OVERRIDES: AtomicU64 = AtomicU64::new(0);
static LAST_DELTA: Mutex<Option<Value>> = Mutex::new(None);
static WORLD_TIME_OBSERVATIONS: AtomicU64 = AtomicU64::new(0);
static LAST_WORLD_TIME: Mutex<Option<Value>> = Mutex::new(None);
#[derive(Clone, Copy, PartialEq, Eq)]
enum ShaderTimeRole {
    Total,
    Delta,
    World,
}
#[derive(Clone, Copy)]
struct ShaderFrameContext {
    renderer: usize,
    generation: u64,
    replay_seconds: f64,
    replay_delta: f64,
    next: ShaderTimeRole,
}
thread_local! {
    // Set by the exact renderCurrentFrame context. Only its declared, ordered
    // total/delta/world call sites can consume it. World accumulation may be
    // skipped by the game; the next frame always replaces this context.
    static SHADER_CONTEXT: Cell<Option<ShaderFrameContext>> = const { Cell::new(None) };
}

fn total_time(seconds: f64, divisor: f64) -> Option<f64> {
    if !seconds.is_finite()
        || !(0. ..=1_000_000_000.).contains(&seconds)
        || divisor.to_bits() != 3600f64.to_bits()
    {
        return None;
    }
    Some(seconds % divisor)
}
fn controlled_shader_time(caller: usize, native: f64, divisor: f64, result: f64) -> Option<f64> {
    let context = SHADER_CONTEXT.get()?;
    let binding = FMOD_BINDING.get()?;
    let role = if caller == binding.total_return_rva {
        ShaderTimeRole::Total
    } else if caller == binding.delta_return_rva {
        ShaderTimeRole::Delta
    } else if caller == binding.world_return_rva {
        ShaderTimeRole::World
    } else {
        return None;
    };
    // A recognized but invalid/out-of-order call ends the frame's authority.
    // Unrelated fmod callers retain their normal result without consuming it.
    SHADER_CONTEXT.set(None);
    if role != context.next {
        return None;
    }
    if !native.is_finite() || native < 0. || !result.is_finite() {
        return None;
    }
    let (session, seconds, _, generation) = crate::native_clock::render_control(context.renderer)?;
    if generation != context.generation || seconds.to_bits() != context.replay_seconds.to_bits() {
        return None;
    }
    if divisor.to_bits() != 3600f64.to_bits() {
        return None;
    }
    let mut state = PREVIOUS.lock().ok()?;
    let control = state.as_mut().filter(|c| {
        c.matches(context.renderer, &session, generation)
            && c.timeline.last_replay.to_bits() == seconds.to_bits()
    })?;
    match role {
        ShaderTimeRole::Total => {
            let anchor = control.total.get_or_insert(TotalAnchor {
                replay: seconds,
                native,
                wrapped: result,
            });
            let (mapped, value) = anchor.sample(seconds)?;
            SHADER_CONTEXT.set(Some(ShaderFrameContext {
                next: ShaderTimeRole::Delta,
                ..context
            }));
            TOTAL_OVERRIDES.fetch_add(1, Ordering::Relaxed);
            if let Ok(mut last) = LAST_TOTAL.lock() {
                *last = Some(
                    json!({"session":session,"seconds":seconds,"nativeInput":native,
                    "divisor":divisor,"nativeResult":result,"controlledResult":value,
                    "generation":generation,"replayOrigin":anchor.replay,"nativeOrigin":anchor.native,
                    "mappedSeconds":mapped,"sourceCallerRva":caller}),
                );
            }
            Some(value)
        }
        ShaderTimeRole::Delta => {
            // This is unscaled replay time. The native frame subsequently
            // narrows it to float and applies its own world scale/skip policy.
            // Repeated frames at one replay timestamp contribute zero time.
            let value = total_time(context.replay_delta, divisor)?;
            SHADER_CONTEXT.set(Some(ShaderFrameContext {
                next: ShaderTimeRole::World,
                ..context
            }));
            DELTA_OVERRIDES.fetch_add(1, Ordering::Relaxed);
            if let Ok(mut last) = LAST_DELTA.lock() {
                *last = Some(
                    json!({"session":session,"seconds":seconds,"generation":generation,
                    "nativeInput":native,"nativeResult":result,"divisor":divisor,"controlledResult":value,
                    "replayFrameDelta":context.replay_delta,"sourceCallerRva":caller,
                    "nativeWorldScalePreserved":true,"nativeWorldSkipPolicyPreserved":true}),
                );
            }
            Some(value)
        }
        ShaderTimeRole::World => {
            // Observe the original calculation after the controlled delta. Its
            // initial accumulator phase is not an archived replay anchor yet.
            // Never claim that observing this return proves its later store.
            WORLD_TIME_OBSERVATIONS.fetch_add(1, Ordering::Relaxed);
            if let Ok(mut last) = LAST_WORLD_TIME.lock() {
                *last = Some(
                    json!({"session":session,"seconds":seconds,"generation":generation,
                    "nativeInput":native,"nativeResult":result,"divisor":divisor,
                    "replayFrameDelta":context.replay_delta,"sourceCallerRva":caller,
                    "deltaControlled":true,"worldTimeOverridden":false,"nativeStoreObserved":false,
                    "recordedWorldPhaseApplied":false,"deterministicVideoReady":false}),
                );
            }
            None
        }
    }
}
unsafe extern "system" fn sample_total(native: f64, divisor: f64) -> f64 {
    let original: Fmod = unsafe { std::mem::transmute(FMOD_ORIGINAL.load(Ordering::Acquire)) };
    let result = unsafe { original(native, divisor) };
    let caller = std::panic::catch_unwind(|| crate::world_hooks::game_callers().first().copied())
        .ok()
        .flatten();
    let controlled = std::panic::catch_unwind(|| {
        caller.and_then(|caller| controlled_shader_time(caller, native, divisor, result))
    })
    .ok()
    .flatten();
    let effective = controlled.unwrap_or(result);
    let _ = std::panic::catch_unwind(|| {
        if let Some(caller) = caller {
            crate::native_shader_clock::sample(
                caller,
                native,
                divisor,
                result,
                effective,
                controlled.is_some(),
            );
        }
    });
    effective
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
struct Patch {
    address: usize,
    bytes: [u8; 16],
    committed: bool,
}
impl Drop for Patch {
    fn drop(&mut self) {
        if !self.committed {
            write(self.address, &self.bytes);
        }
    }
}
fn apply(renderer: usize, value: [f32; 4]) -> Option<([f32; 4], [f32; 4])> {
    let address = renderer.checked_add(scene::game_renderer::CLOCK)?;
    let before = floats::<4>(address, 1_000_000_000.)?;
    if value
        .iter()
        .any(|v| !v.is_finite() || *v < 0. || v.abs() > 1_000_000_000.)
    {
        return None;
    }
    let mut bytes = [0u8; 16];
    for (part, value) in bytes.as_chunks_mut::<4>().0.iter_mut().zip(value) {
        part.copy_from_slice(&value.to_le_bytes());
    }
    let mut original = [0u8; 16];
    read(address, &mut original).ok()?;
    let mut patch = Patch {
        address,
        bytes: original,
        committed: false,
    };
    if !write(address, &bytes) {
        return None;
    }
    let after = floats::<4>(address, 1_000_000_000.)?;
    if after.map(f32::to_bits) != value.map(f32::to_bits) {
        return None;
    }
    patch.committed = true;
    Some((before, after))
}
fn override_clock(renderer: usize, incoming_partial: f32, world_context: bool) -> Option<f32> {
    // This method also builds UI/item contexts. Only this direct return site
    // belongs to renderCurrentFrame, including when its renderer is foreign.
    if !world_context {
        return None;
    }
    SHADER_CONTEXT.set(None);
    let Some((session, seconds, partial, generation)) =
        crate::native_clock::render_control(renderer)
    else {
        if let Ok(mut previous) = PREVIOUS.lock()
            && previous.as_ref().is_some_and(|c| c.renderer == renderer)
        {
            *previous = None;
        }
        return None;
    };
    WORLD_CALLS.fetch_add(1, Ordering::Relaxed);
    let Ok(mut previous) = PREVIOUS.lock() else {
        return None;
    };
    let before = floats::<4>(
        renderer.checked_add(scene::game_renderer::CLOCK)?,
        1_000_000_000.,
    )?;
    let scale = floats::<1>(
        renderer.checked_add(scene::game_renderer::CLOCK_SCALE)?,
        1000.,
    )?[0];
    let mut control = match previous
        .as_ref()
        .filter(|c| c.matches(renderer, &session, generation))
    {
        Some(c) => c.clone(),
        None => Control {
            session: session.clone(),
            renderer,
            generation,
            timeline: Timeline::start(seconds, before, scale)?,
            total: None,
        },
    };
    let replay_delta = seconds - control.timeline.last_replay;
    let planned = control.timeline.sample(seconds, scale);
    let result = planned.as_ref().and_then(|t| apply(renderer, t.value));
    let mapping = planned
        .as_ref()
        .map(|t| json!({"session":session,"generation":generation,"timeline":t}));
    if result.is_some() {
        SHADER_CONTEXT.set(Some(ShaderFrameContext {
            renderer,
            generation,
            replay_seconds: seconds,
            replay_delta,
            next: ShaderTimeRole::Total,
        }));
        control.timeline = planned.expect("successful clock plan");
        *previous = Some(control);
        OVERRIDES.fetch_add(1, Ordering::Relaxed);
    }
    if let Ok(mut value) = LAST.lock() {
        *value = Some(
            json!({"session":session,"seconds":seconds,"applied":result.is_some(),
            "generation":generation,"mapping":mapping,
            "error":result.is_none().then_some("renderer timeline invalid or moved backwards; reconstruction may be required"),
            "before":result.map(|v|v.0),"after":result.map(|v|v.1),"wallTimeFieldPreserved":true,
            "incomingPartial":incoming_partial,"contextPartial":result.map(|_|partial)}),
        );
    }
    result.map(|_| partial)
}
pub(crate) fn scaled_time(
    renderer: usize,
    session: &str,
    generation: u64,
    seconds: f64,
) -> Option<f32> {
    let state = PREVIOUS.lock().ok()?;
    let c = state.as_ref().filter(|c| {
        c.matches(renderer, session, generation)
            && c.timeline.last_replay.to_bits() == seconds.to_bits()
    })?;
    Some(c.timeline.value[0])
}
pub(crate) fn mapping(renderer: usize) -> Option<Value> {
    let (session, seconds, _, generation) = crate::native_clock::render_control(renderer)?;
    let state = PREVIOUS.lock().ok()?;
    let c = state.as_ref().filter(|c| {
        c.matches(renderer, &session, generation)
            && c.timeline.last_replay.to_bits() == seconds.to_bits()
    })?;
    let actual = floats::<4>(
        renderer.checked_add(scene::game_renderer::CLOCK)?,
        1_000_000_000.,
    )?;
    let scale = floats::<1>(
        renderer.checked_add(scene::game_renderer::CLOCK_SCALE)?,
        1000.,
    )?[0];
    Some(
        json!({"session":session,"generation":generation,"timeline":c.timeline,
        "verified":actual.map(f32::to_bits)==c.timeline.value.map(f32::to_bits) && scale.to_bits()==c.timeline.scale.to_bits()}),
    )
}
unsafe extern "system" fn make_context(renderer: usize, output: usize, partial: f32) -> usize {
    CALLS.fetch_add(1, Ordering::Relaxed);
    let world_context = std::panic::catch_unwind(|| {
        CONTEXT_BINDING.get().is_some_and(|binding| {
            crate::world_hooks::game_callers().first() == Some(&binding.world_return_rva)
        })
    })
    .unwrap_or(false);
    if world_context {
        crate::native_shader_clock::clear();
    }
    let controlled = std::panic::catch_unwind(|| override_clock(renderer, partial, world_context))
        .ok()
        .flatten();
    let partial = controlled.unwrap_or(partial);
    let original: MakeContext = unsafe { std::mem::transmute(ORIGINAL.load(Ordering::Acquire)) };
    // The exact owned world context uses the controlled simulation fraction.
    // Other contexts retain the incoming float bits, hidden output and return.
    // Native makeScreenContext forwards this value into ScreenContext::a (+8),
    // independently of the clock fields used by shader/material animation.
    let result = unsafe { original(renderer, output, partial) };
    if controlled.is_some() && result != output {
        SHADER_CONTEXT.set(None);
    }
    if world_context && result == output {
        let _ = std::panic::catch_unwind(|| {
            if let Some(binding) = FMOD_BINDING.get() {
                let _ = crate::native_shader_clock::begin(renderer, output, *binding);
            }
        });
    }
    crate::native_flipbook::begin_context(
        renderer,
        output,
        controlled.is_some() && result == output,
    );
    if let Some(expected) = controlled {
        let actual = output
            .checked_add(scene::screen::PARTIAL)
            .and_then(|at| floats::<1>(at, 1.))
            .map(|v| v[0]);
        if let Ok(mut last) = LAST.lock()
            && let Some(value) = last.as_mut()
        {
            value["observedContextPartial"] = json!(actual);
            value["contextPartialVerified"] =
                json!(actual.is_some_and(|a| a.to_bits() == expected.to_bits()));
            value["contextOutputMatchesReturn"] = json!(result == output);
        }
    }
    result
}
pub(crate) unsafe fn install() -> Result<(), String> {
    let base = crate::world_hooks::image_base().ok_or("render clock image profile unavailable")?;
    let binding = crate::world_hooks::screen_context_binding()?;
    let target = base
        .checked_add(binding.rva)
        .ok_or("screen-context target address overflow")?;
    let mut code = [0u8; 19];
    read(target, &mut code)?;
    let mut expected = [
        0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x81, 0xec,
        0, 0, 0, 0,
    ];
    expected[15..].copy_from_slice(&binding.stack_bytes.to_le_bytes());
    if code != expected {
        return Err("native makeScreenContext prologue differs".into());
    }
    use hudhook::mh::{MH_CreateHook, MH_EnableHook, MH_STATUS};
    let mut original = std::ptr::null_mut();
    let status =
        unsafe { MH_CreateHook(target as _, make_context as *const () as _, &mut original) };
    if status != MH_STATUS::MH_OK {
        return Err(format!("create renderer clock hook: {status:?}"));
    }
    ORIGINAL.store(original as usize, Ordering::Release);
    let _ = CONTEXT_BINDING.set(binding);
    let status = unsafe { MH_EnableHook(target as _) };
    if status != MH_STATUS::MH_OK {
        return Err(format!("enable renderer clock hook: {status:?}"));
    }
    if crate::native_clock::control_enabled() {
        // The approved image's own fmod import thunk and IAT slot are paired.
        // Its CRT export is not patched process-wide.
        let binding = crate::world_hooks::fmod_binding()?;
        let target = base
            .checked_add(binding.rva)
            .ok_or("shader-time thunk address overflow")?;
        let mut bytes = [0; 6];
        read(target, &mut bytes)?;
        let displacement = i32::from_le_bytes(
            bytes[2..]
                .try_into()
                .map_err(|_| "short shader-time thunk")?,
        );
        if bytes[..2] != [0xff, 0x25]
            || target
                .checked_add(6)
                .and_then(|address| address.checked_add_signed(displacement as isize))
                != base.checked_add(binding.iat_rva)
        {
            return Err("native fmod thunk differs".into());
        }
        let mut original = std::ptr::null_mut();
        let status =
            unsafe { MH_CreateHook(target as _, sample_total as *const () as _, &mut original) };
        if status != MH_STATUS::MH_OK {
            return Err(format!("create total shader clock hook: {status:?}"));
        }
        FMOD_ORIGINAL.store(original as usize, Ordering::Release);
        let _ = FMOD_BINDING.set(binding);
        let status = unsafe { MH_EnableHook(target as _) };
        if status != MH_STATUS::MH_OK {
            return Err(format!("enable total shader clock hook: {status:?}"));
        }
    }
    Ok(())
}
pub(crate) fn status() -> Value {
    json!({"installed":ORIGINAL.load(Ordering::Acquire)!=0,
        "hookRva":CONTEXT_BINDING.get().map(|binding|binding.rva),
        "worldReturnRva":CONTEXT_BINDING.get().map(|binding|binding.world_return_rva),"calls":CALLS.load(Ordering::Relaxed),
        "overrides":OVERRIDES.load(Ordering::Relaxed),"worldContextCalls":WORLD_CALLS.load(Ordering::Relaxed),"last":LAST.lock().ok().and_then(|v|v.clone()),
        "totalTime":{"installed":FMOD_ORIGINAL.load(Ordering::Acquire)!=0,
            "hookRva":FMOD_BINDING.get().map(|binding|binding.rva),"iatRva":FMOD_BINDING.get().map(|binding|binding.iat_rva),
            "totalReturnRva":FMOD_BINDING.get().map(|binding|binding.total_return_rva),
            "overrides":TOTAL_OVERRIDES.load(Ordering::Relaxed),"last":LAST_TOTAL.lock().ok().and_then(|v|v.clone())},
        "deltaTime":{"installed":FMOD_ORIGINAL.load(Ordering::Acquire)!=0,
            "sourceReturnRva":FMOD_BINDING.get().map(|binding|binding.delta_return_rva),
            "overrides":DELTA_OVERRIDES.load(Ordering::Relaxed),"last":LAST_DELTA.lock().ok().and_then(|v|v.clone()),
            "diagnosticOnly":true,"runtimeVerified":false},
        "worldTime":{"sourceReturnRva":FMOD_BINDING.get().map(|binding|binding.world_return_rva),
            "observations":WORLD_TIME_OBSERVATIONS.load(Ordering::Relaxed),
            "last":LAST_WORLD_TIME.lock().ok().and_then(|v|v.clone()),
            "diagnosticOnly":true,"recordedPhaseApplied":false,"worldTimeReplayVerified":false},
        "nativeSimulationVerified":false,"deterministicVideoReady":false})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn total_shader_time_uses_controlled_seconds_and_preserves_native_hour_wrapping() {
        assert_eq!(total_time(39.4625, 3600.), Some(39.4625));
        assert_eq!(total_time(3601.25, 3600.), Some(1.25));
        assert_eq!(total_time(3600., 3600.), Some(0.));
        for seconds in [-1., f64::NAN, f64::INFINITY] {
            assert!(total_time(seconds, 3600.).is_none());
        }
        for divisor in [0., -3600., 3599., f64::NAN] {
            assert!(total_time(1., divisor).is_none());
        }
    }
    #[test]
    fn real_total_time_detour_forwards_double_bits_once_without_owned_context() {
        static RECEIVED: Mutex<Option<(u64, u64)>> = Mutex::new(None);
        static CALLS: AtomicU64 = AtomicU64::new(0);
        #[inline(never)]
        unsafe extern "system" fn host(a: f64, b: f64) -> f64 {
            *RECEIVED.lock().unwrap() = Some((a.to_bits(), b.to_bits()));
            CALLS.fetch_add(1, Ordering::Relaxed);
            f64::from_bits(0x7ff8123456789abc)
        }
        use hudhook::mh::{MH_CreateHook, MH_DisableHook, MH_EnableHook, MH_Initialize, MH_STATUS};
        assert!(matches!(
            unsafe { MH_Initialize() },
            MH_STATUS::MH_OK | MH_STATUS::MH_ERROR_ALREADY_INITIALIZED
        ));
        let target = host as *const () as usize;
        let mut trampoline = std::ptr::null_mut();
        assert_eq!(
            unsafe { MH_CreateHook(target as _, sample_total as *const () as _, &mut trampoline) },
            MH_STATUS::MH_OK
        );
        FMOD_ORIGINAL.store(trampoline as usize, Ordering::Release);
        assert_eq!(unsafe { MH_EnableHook(target as _) }, MH_STATUS::MH_OK);
        let call: Fmod = std::hint::black_box(host);
        let a = f64::from_bits(0x7ff812340000abcd);
        assert_eq!(unsafe { call(a, -0.) }.to_bits(), 0x7ff8123456789abc);
        assert_eq!(unsafe { MH_DisableHook(target as _) }, MH_STATUS::MH_OK);
        FMOD_ORIGINAL.store(target, Ordering::Release);
        assert_eq!(
            *RECEIVED.lock().unwrap(),
            Some((a.to_bits(), (-0f64).to_bits()))
        );
        assert_eq!(CALLS.load(Ordering::Relaxed), 1);
    }
    #[test]
    fn renderer_clock_keeps_native_pause_scale_and_wall_bookkeeping() {
        let mut renderer = vec![0u8; 0x220];
        renderer[0x200..0x204].copy_from_slice(&0.5f32.to_le_bytes());
        renderer[0x204] = 1;
        renderer[0x218..0x21c].copy_from_slice(&1234f32.to_le_bytes());
        let address = renderer.as_ptr() as usize;
        let start = Timeline::start(10., [200., 0.1, 0.01, 1000.], 0.5).unwrap();
        assert_eq!(
            apply(address, start.value).unwrap().1,
            [200., 0., 0., 1000.]
        );
        let step = start.sample(10.5, 0.5).unwrap();
        assert_eq!(
            apply(address, step.value).unwrap().1,
            [200.25, 0.25, 0.0625, 1000.5]
        );
        assert_eq!(
            apply(address, step.sample(10.5, 0.5).unwrap().value)
                .unwrap()
                .1,
            [200.25, 0., 0., 1000.5]
        );
        assert_eq!(renderer[0x204], 1);
        assert_eq!(floats::<1>(address + 0x200, 2.), Some([0.5]));
        assert_eq!(floats::<1>(address + 0x218, 2000.), Some([1234.]));
        for seconds in [f64::NAN, -1., f64::INFINITY] {
            assert!(apply(address, [seconds as f32, 0., 0., 1000.]).is_none());
        }
    }
    #[test]
    fn total_clock_preserves_native_phase_and_wraps_replay_delta_at_the_native_hour() {
        let anchor = TotalAnchor {
            replay: 22.,
            native: 3599.75,
            wrapped: 3599.75,
        };
        assert_eq!(anchor.sample(22.), Some((3599.75, 3599.75)));
        assert_eq!(anchor.sample(22.5), Some((3600.25, 0.25)));
        assert!(anchor.sample(21.).is_none());
        assert!(anchor.sample(f64::NAN).is_none());
        let zero = TotalAnchor {
            replay: 10.,
            native: -0.,
            wrapped: -0.,
        };
        assert_eq!(zero.sample(10.).unwrap().1.to_bits(), (-0f64).to_bits());
    }
    #[test]
    fn real_context_detour_forwards_hidden_output_float_and_return() {
        static RECEIVED: Mutex<Option<(usize, usize, u32)>> = Mutex::new(None);
        #[inline(never)]
        unsafe extern "system" fn host(this: usize, output: usize, partial: f32) -> usize {
            *RECEIVED.lock().unwrap() = Some((this, output, partial.to_bits()));
            output ^ 0xdeadbeef
        }
        use hudhook::mh::{MH_CreateHook, MH_DisableHook, MH_EnableHook, MH_Initialize, MH_STATUS};
        assert!(matches!(
            unsafe { MH_Initialize() },
            MH_STATUS::MH_OK | MH_STATUS::MH_ERROR_ALREADY_INITIALIZED
        ));
        let target = host as *const () as usize;
        let mut trampoline = std::ptr::null_mut();
        assert_eq!(
            unsafe { MH_CreateHook(target as _, make_context as *const () as _, &mut trampoline) },
            MH_STATUS::MH_OK
        );
        ORIGINAL.store(trampoline as usize, Ordering::Release);
        assert_eq!(unsafe { MH_EnableHook(target as _) }, MH_STATUS::MH_OK);
        let call: MakeContext = std::hint::black_box(host);
        let value = f32::from_bits(0x7fc12345);
        assert_eq!(unsafe { call(17, 23, value) }, 23 ^ 0xdeadbeef);
        assert_eq!(unsafe { MH_DisableHook(target as _) }, MH_STATUS::MH_OK);
        ORIGINAL.store(target, Ordering::Release);
        assert_eq!(*RECEIVED.lock().unwrap(), Some((17, 23, value.to_bits())));
    }
}
