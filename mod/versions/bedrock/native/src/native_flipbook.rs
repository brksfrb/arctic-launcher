//! Read-only animation diagnostics for owned controlled world screen contexts.
use crate::native_scene_layout as scene;
use crate::world_hooks::{floats, read};
use serde::Serialize;
use serde_json::{Value, json};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

const RVA: usize = 0x661b740;
type Describe = unsafe extern "system" fn(usize, usize, usize, usize) -> usize;
static ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static ENABLED: AtomicBool = AtomicBool::new(false);
static CALLS: AtomicU64 = AtomicU64::new(0);
static MATCHED: AtomicU64 = AtomicU64::new(0);
static STAGE_CALLS: AtomicU64 = AtomicU64::new(0);
static CONTEXT_MATCHES: AtomicU64 = AtomicU64::new(0);
struct Context {
    renderer: usize,
    screen: usize,
    session: String,
    seconds: f64,
    header: [u8; 0x50],
    samples: Vec<Sample>,
    omitted: u32,
}
static CONTEXT: Mutex<Option<Context>> = Mutex::new(None);
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Sample {
    texture: String,
    phase: f32,
    frame_count: u32,
    sequence_count: u32,
    ticks_per_frame: u32,
    blending: bool,
    disabled: bool,
    context_partial: f32,
    description: [f32; 3],
    exact_context_copy: bool,
}
pub(crate) fn configure_probe(isolated: bool) {
    ENABLED.store(isolated && crate::native_body::enabled(), Ordering::Release);
}
pub(crate) fn begin_context(renderer: usize, screen: usize, controlled: bool) {
    if !ENABLED.load(Ordering::Acquire) || !controlled {
        return;
    }
    let next = controlled
        .then(|| crate::native_clock::render_sample(renderer))
        .flatten()
        .and_then(|(session, seconds, _)| {
            let mut header = [0; 0x50];
            read(screen, &mut header).ok()?;
            Some(Context {
                renderer,
                screen,
                session,
                seconds,
                header,
                samples: Vec::new(),
                omitted: 0,
            })
        });
    if let Ok(mut context) = CONTEXT.lock() {
        *context = next;
    }
}
fn sample(texture: usize, output: usize, screen: usize) -> Option<Sample> {
    let mut bytes = [0; 30];
    read(texture.checked_add(0x360)?, &mut bytes).ok()?;
    let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
    let phase = f32::from_bits(word(0));
    let frame_count = word(4);
    let sequence_count = word(8);
    let ticks_per_frame = word(12);
    if !phase.is_finite()
        || !(0. ..=1_000_000_000.).contains(&phase)
        || !(1..=65536).contains(&frame_count)
        || !(1..=65536).contains(&sequence_count)
        || !(1..=65536).contains(&ticks_per_frame)
        || bytes[28] > 1
        || bytes[29] > 1
    {
        return None;
    }
    Some(Sample {
        texture: format!("{texture:x}"),
        phase,
        frame_count,
        sequence_count,
        ticks_per_frame,
        blending: bytes[28] != 0,
        disabled: bytes[29] != 0,
        context_partial: floats::<1>(screen.checked_add(scene::screen::PARTIAL)?, 1.)?[0],
        description: floats::<3>(output.checked_add(0x20)?, 65536.)?,
        exact_context_copy: false,
    })
}
fn owns_context(context: &Context, screen: usize) -> bool {
    // renderCurrentFrame copies the newly constructed context into renderer+270
    // at 0x47a8f55 via 0x4460390. Check the copied fields, excluding padding.
    if screen != context.screen
        && context
            .renderer
            .checked_add(scene::game_renderer::SCREEN_CONTEXT)
            != Some(screen)
    {
        return false;
    }
    let mut header = [0; 0x50];
    read(screen, &mut header).is_ok()
        && header[..12] == context.header[..12]
        && header[16..] == context.header[16..]
}
fn renderer_clock_matches(renderer: usize, screen: usize) -> bool {
    let Some(clock) = renderer.checked_add(scene::game_renderer::CLOCK) else {
        return false;
    };
    screen
        .checked_add(scene::screen::CLOCK)
        .and_then(crate::world_hooks::pointer)
        == Some(clock)
}
fn observe(texture: usize, output: usize, screen: usize, result: usize) {
    if !ENABLED.load(Ordering::Acquire)
        || result != output
        || crate::world_hooks::game_callers().first() != Some(&0x46e8e53)
    {
        return;
    }
    STAGE_CALLS.fetch_add(1, Ordering::Relaxed);
    let Ok(mut context) = CONTEXT.lock() else {
        return;
    };
    let Some(context) = context.as_mut() else {
        return;
    };
    // The native constructor stores its clock reference at ScreenContext+b0
    // (0x4816018). This follows renderer-owned copies across render threads.
    // Clock ownership is diagnostic evidence, not a per-frame delivery proof.
    if !renderer_clock_matches(context.renderer, screen)
        || !crate::native_clock::render_sample(context.renderer).is_some_and(|(s, t, _)| {
            s == context.session && t.to_bits() == context.seconds.to_bits()
        })
    {
        return;
    }
    CONTEXT_MATCHES.fetch_add(1, Ordering::Relaxed);
    if context.samples.len() >= 128 {
        context.omitted = context.omitted.saturating_add(1);
        return;
    }
    if let Some(mut sample) = sample(texture, output, screen) {
        sample.exact_context_copy = owns_context(context, screen);
        context.samples.push(sample);
        MATCHED.fetch_add(1, Ordering::Relaxed);
    }
}
unsafe extern "system" fn describe(
    texture: usize,
    output: usize,
    screen: usize,
    viewport: usize,
) -> usize {
    CALLS.fetch_add(1, Ordering::Relaxed);
    let original: Describe = unsafe { std::mem::transmute(ORIGINAL.load(Ordering::Acquire)) };
    let result = unsafe { original(texture, output, screen, viewport) };
    let _ = std::panic::catch_unwind(|| observe(texture, output, screen, result));
    result
}
pub(crate) fn snapshot() -> Option<Value> {
    if !ENABLED.load(Ordering::Acquire) {
        return None;
    }
    {
        let context = CONTEXT.lock().ok()?;
        let context = context.as_ref()?;
        let (session, seconds, _) = crate::native_clock::render_sample(context.renderer)?;
        if context.session != session || context.seconds.to_bits() != seconds.to_bits() {
            return None;
        }
        Some(
            json!({"session":session,"seconds":seconds,"samples":context.samples,
            "omitted":context.omitted,"diagnosticOnly":true,"frameOwnershipVerified":false}),
        )
    }
}
pub(crate) unsafe fn install() -> Result<(), String> {
    if !ENABLED.load(Ordering::Acquire) {
        return Ok(());
    }
    let target =
        crate::world_hooks::image_base().ok_or("flipbook image profile unavailable")? + RVA;
    let mut bytes = [0; 16];
    read(target, &mut bytes)?;
    if bytes
        != [
            0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x83,
            0xec, 0x68,
        ]
    {
        return Err("native flipbook description prologue differs".into());
    }
    use hudhook::mh::{MH_CreateHook, MH_EnableHook, MH_STATUS};
    let mut trampoline = std::ptr::null_mut();
    let status = unsafe { MH_CreateHook(target as _, describe as *const () as _, &mut trampoline) };
    if status != MH_STATUS::MH_OK {
        return Err(format!("create flipbook observer: {status:?}"));
    }
    ORIGINAL.store(trampoline as usize, Ordering::Release);
    let status = unsafe { MH_EnableHook(target as _) };
    if status != MH_STATUS::MH_OK {
        return Err(format!("enable flipbook observer: {status:?}"));
    }
    Ok(())
}
pub(crate) fn status() -> Value {
    json!({"enabled":ENABLED.load(Ordering::Acquire),"installed":ORIGINAL.load(Ordering::Acquire)!=0,
        "calls":CALLS.load(Ordering::Relaxed),"stageCalls":STAGE_CALLS.load(Ordering::Relaxed),
        "contextMatches":CONTEXT_MATCHES.load(Ordering::Relaxed),"matched":MATCHED.load(Ordering::Relaxed),"hookRva":RVA,"observerOnly":true})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn renderer_clock_reference_rejects_foreign_null_and_overflowing_contexts() {
        let renderer = vec![0u8; 0x220];
        let mut screen = vec![0u8; 0xb8];
        let r = renderer.as_ptr() as usize;
        let s = screen.as_ptr() as usize;
        screen[0xb0..0xb8].copy_from_slice(&((r + 0x1f0) as u64).to_le_bytes());
        assert!(renderer_clock_matches(r, s));
        assert!(!renderer_clock_matches(r + 8, s));
        assert!(!renderer_clock_matches(usize::MAX, s));
        assert!(!renderer_clock_matches(r, usize::MAX));
        screen[0xb0..0xb8].fill(0);
        std::hint::black_box(&screen);
        assert!(!renderer_clock_matches(r, s));
    }
    #[test]
    fn context_ownership_accepts_only_the_original_or_verified_renderer_copy() {
        let screen = [3u8; 0x50];
        let mut renderer = vec![0u8; 0x2c0];
        renderer[0x270..0x2c0].copy_from_slice(&screen);
        let context = Context {
            renderer: renderer.as_ptr() as usize,
            screen: screen.as_ptr() as usize,
            session: String::new(),
            seconds: 0.,
            header: screen,
            samples: Vec::new(),
            omitted: 0,
        };
        assert!(owns_context(&context, context.screen));
        assert!(owns_context(&context, context.renderer + 0x270));
        let foreign = screen;
        assert!(!owns_context(&context, foreign.as_ptr() as usize));
        renderer[0x27c] = 9;
        std::hint::black_box(&renderer);
        assert!(owns_context(&context, context.renderer + 0x270)); // padding
        renderer[0x278] = 9;
        std::hint::black_box(&renderer);
        assert!(!owns_context(&context, context.renderer + 0x270));
    }
    #[test]
    fn reads_phase_and_blend_without_mutating_native_data_and_rejects_invalid_values() {
        let mut texture = vec![0; 0x380];
        texture[0x360..0x364].copy_from_slice(&9.5f32.to_le_bytes());
        for at in [0x364, 0x368, 0x36c] {
            texture[at..at + 4].copy_from_slice(&16u32.to_le_bytes());
        }
        texture[0x37c] = 1;
        let mut output = [0; 0x44];
        for (i, value) in [0.25f32, 0.5, 0.75].iter().enumerate() {
            output[0x20 + i * 4..0x24 + i * 4].copy_from_slice(&value.to_le_bytes());
        }
        let screen = [0f32, 0., 0.25];
        let a = texture.as_ptr() as usize;
        let b = output.as_ptr() as usize;
        let c = screen.as_ptr() as usize;
        let before = texture.clone();
        let before_output = output;
        let s = sample(a, b, c).unwrap();
        assert_eq!(s.phase, 9.5);
        assert_eq!(s.description, [0.25, 0.5, 0.75]);
        assert_eq!(s.context_partial, 0.25);
        assert!(s.blending);
        assert_eq!(texture, before);
        assert_eq!(output, before_output);
        texture[0x37d] = 2;
        std::hint::black_box(&texture);
        assert!(sample(a, b, c).is_none());
        texture[0x37d] = 0;
        texture[0x360..0x364].copy_from_slice(&f32::NAN.to_le_bytes());
        std::hint::black_box(&texture);
        assert!(sample(a, b, c).is_none());
    }
    #[test]
    fn real_description_detour_preserves_four_arguments_and_full_return() {
        use std::sync::Mutex;
        static RECEIVED: Mutex<Option<[usize; 4]>> = Mutex::new(None);
        #[inline(never)]
        unsafe extern "system" fn host(a: usize, b: usize, c: usize, d: usize) -> usize {
            *RECEIVED.lock().unwrap() = Some([a, b, c, d]);
            b ^ 0xdeadbeef12345678usize
        }
        use hudhook::mh::{MH_CreateHook, MH_DisableHook, MH_EnableHook, MH_Initialize, MH_STATUS};
        assert!(matches!(
            unsafe { MH_Initialize() },
            MH_STATUS::MH_OK | MH_STATUS::MH_ERROR_ALREADY_INITIALIZED
        ));
        let target = host as *const () as usize;
        let mut trampoline = std::ptr::null_mut();
        assert_eq!(
            unsafe { MH_CreateHook(target as _, describe as *const () as _, &mut trampoline) },
            MH_STATUS::MH_OK
        );
        ORIGINAL.store(trampoline as usize, Ordering::Release);
        assert_eq!(unsafe { MH_EnableHook(target as _) }, MH_STATUS::MH_OK);
        let call: Describe = std::hint::black_box(host);
        assert_eq!(
            unsafe { call(17, 23, 31, 47) },
            23 ^ 0xdeadbeef12345678usize
        );
        assert_eq!(unsafe { MH_DisableHook(target as _) }, MH_STATUS::MH_OK);
        ORIGINAL.store(target, Ordering::Release);
        assert_eq!(*RECEIVED.lock().unwrap(), Some([17, 23, 31, 47]));
    }
}
