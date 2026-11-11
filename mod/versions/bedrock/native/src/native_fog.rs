//! Replay evaluation of native fog from one starting state per timestamp.
//! setupFog contains fixed per-render blends even when mDeltaTime is zero.
use crate::native_scene_layout as scene;
use crate::world_hooks::{floats, pointer, read};
use serde_json::{Value, json};
use std::sync::{
    Mutex,
    atomic::{AtomicU64, AtomicUsize, Ordering},
};

const RVA: usize = 0x4682200;
type Setup = unsafe extern "system" fn(usize, usize, f32);
static ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static CALLS: AtomicU64 = AtomicU64::new(0);
static EVALUATIONS: AtomicU64 = AtomicU64::new(0);
static REPEATS: AtomicU64 = AtomicU64::new(0);
static STATE: Mutex<State> = Mutex::new(State {
    history: None,
    last: None,
});

// Exact-image scalar fields only. ParticleRenderData starts at +4e0 and is
// deliberately excluded, as are camera/terrain resources and native pointers.
const RANGES: [(usize, usize); 3] = [(0x408, 4), (0x42c, 0xb4), (0x5c0, 5)];
const BYTES: usize = 4 + 0xb4 + 5;
#[derive(Clone, PartialEq, Eq)]
struct Snapshot([u8; BYTES]);
impl Snapshot {
    fn capture(camera: usize) -> Option<Self> {
        floats::<1>(camera.checked_add(0x408)?, 1_000_000.)?;
        floats::<6>(camera.checked_add(0x42c)?, 1_000_000.)?;
        floats::<6>(camera.checked_add(0x448)?, 1_000_000.)?;
        floats::<29>(camera.checked_add(0x464)?, 1_000_000.)?;
        floats::<1>(camera.checked_add(0x4dc)?, 1_000_000.)?;
        floats::<1>(camera.checked_add(0x5c0)?, 1_000_000.)?;
        let mut data = [0; BYTES];
        let mut at = 0;
        for (offset, size) in RANGES {
            read(camera.checked_add(offset)?, &mut data[at..at + size]).ok()?;
            at += size;
        }
        // Four material flags and the transition flag are native bools.
        let flags = 4 + 0x4d8 - 0x42c;
        (data[flags..flags + 4].iter().all(|v| *v <= 1) && data[BYTES - 1] <= 1)
            .then_some(Self(data))
    }
    fn write(&self, camera: usize) -> bool {
        let mut at = 0;
        for (offset, size) in RANGES {
            let Some(address) = camera.checked_add(offset) else {
                return false;
            };
            if !crate::native_camera_time::write(address, &self.0[at..at + size]) {
                return false;
            }
            at += size;
        }
        Self::capture(camera).as_ref() == Some(self)
    }
    fn hash(&self) -> String {
        use sha2::{Digest, Sha256};
        crate::native_hash::hex(&Sha256::digest(self.0))
    }
}
#[derive(Clone, PartialEq, Eq)]
struct Owner {
    session: String,
    generation: u64,
    camera: usize,
}
#[derive(Clone)]
struct History {
    owner: Owner,
    seconds: f64,
    baseline: Snapshot,
}
impl History {
    fn next(
        previous: Option<&Self>,
        owner: Owner,
        seconds: f64,
        native: Snapshot,
    ) -> Option<(Self, bool)> {
        if !seconds.is_finite() || seconds < 0. {
            return None;
        }
        if let Some(old) = previous.filter(|h| h.owner == owner) {
            if seconds < old.seconds {
                return None;
            }
            if seconds.to_bits() == old.seconds.to_bits() {
                return Some((old.clone(), true));
            }
        }
        Some((
            Self {
                owner,
                seconds,
                baseline: native,
            },
            false,
        ))
    }
}
struct State {
    history: Option<History>,
    last: Option<Value>,
}
fn owner(camera: usize, screen: usize) -> Option<(Owner, f64)> {
    let clock = pointer(screen.checked_add(scene::screen::CLOCK)?)?;
    let renderer = clock.checked_sub(scene::game_renderer::CLOCK)?;
    let (session, seconds, _, generation) = crate::native_clock::render_control(renderer)?;
    if crate::native_clock::owned_player_renderer(renderer)? != camera
        || floats::<1>(clock, 1_000_000_000.)?[0].to_bits()
            != crate::native_render_clock::scaled_time(renderer, &session, generation, seconds)?
                .to_bits()
    {
        return None;
    }
    Some((
        Owner {
            session,
            generation,
            camera,
        },
        seconds,
    ))
}
struct Evaluation {
    history: History,
    before: Snapshot,
    repeated: bool,
}
fn prepare(camera: usize, screen: usize) -> Option<Evaluation> {
    let (owner, seconds) = owner(camera, screen)?;
    let before = Snapshot::capture(camera)?;
    let state = STATE.lock().ok()?;
    let (history, repeated) =
        History::next(state.history.as_ref(), owner, seconds, before.clone())?;
    drop(state);
    if repeated && !history.baseline.write(camera) {
        let _ = before.write(camera);
        return None;
    }
    Some(Evaluation {
        history,
        before,
        repeated,
    })
}
fn finished(evaluation: Evaluation, screen: usize) {
    let h = evaluation.history;
    let after = Snapshot::capture(h.owner.camera);
    let current = owner(h.owner.camera, screen);
    let verified = after.is_some()
        && current
            .as_ref()
            .is_some_and(|(o, s)| *o == h.owner && s.to_bits() == h.seconds.to_bits());
    if verified {
        EVALUATIONS.fetch_add(1, Ordering::Relaxed);
        if evaluation.repeated {
            REPEATS.fetch_add(1, Ordering::Relaxed);
        }
    }
    if let Ok(mut state) = STATE.lock() {
        state.last = Some(
            json!({"session":h.owner.session,"generation":h.owner.generation,
            "seconds":h.seconds,"repeated":evaluation.repeated,"startingStateRestored":evaluation.repeated,
            "baselineSha256":h.baseline.hash(),"beforeSha256":evaluation.before.hash(),
            "afterSha256":after.as_ref().map(Snapshot::hash),"verified":verified,
            "nativeFunctionEvaluated":true,"shaderConstantsOverridden":false,
            "deterministicVideoVerified":false}),
        );
        state.history = verified.then_some(h);
    }
}
unsafe extern "system" fn setup(camera: usize, screen: usize, sky: f32) {
    CALLS.fetch_add(1, Ordering::Relaxed);
    let evaluation = std::panic::catch_unwind(|| prepare(camera, screen))
        .ok()
        .flatten();
    let original: Setup = unsafe { std::mem::transmute(ORIGINAL.load(Ordering::Acquire)) };
    unsafe { original(camera, screen, sky) };
    if let Some(evaluation) = evaluation {
        let _ = std::panic::catch_unwind(|| finished(evaluation, screen));
    }
}
pub(crate) unsafe fn install() -> Result<(), String> {
    if !crate::native_clock::control_enabled() {
        return Ok(());
    }
    let target = crate::world_hooks::image_base().ok_or("fog image unavailable")? + RVA;
    let mut bytes = [0; 19];
    read(target, &mut bytes)?;
    if bytes
        != [
            0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x81,
            0xec, 0x88, 2, 0, 0,
        ]
    {
        return Err("native fog setup prologue differs".into());
    }
    use hudhook::mh::{MH_CreateHook, MH_EnableHook, MH_STATUS};
    let mut trampoline = std::ptr::null_mut();
    let result = unsafe { MH_CreateHook(target as _, setup as *const () as _, &mut trampoline) };
    if result != MH_STATUS::MH_OK {
        return Err(format!("create fog hook: {result:?}"));
    }
    ORIGINAL.store(trampoline as usize, Ordering::Release);
    let result = unsafe { MH_EnableHook(target as _) };
    if result != MH_STATUS::MH_OK {
        return Err(format!("enable fog hook: {result:?}"));
    }
    Ok(())
}
pub(crate) fn status() -> Value {
    json!({"installed":ORIGINAL.load(Ordering::Acquire)!=0,"hookRva":RVA,
        "calls":CALLS.load(Ordering::Relaxed),"evaluations":EVALUATIONS.load(Ordering::Relaxed),
        "repeatedEvaluations":REPEATS.load(Ordering::Relaxed),
        "last":STATE.lock().ok().and_then(|s|s.last.clone()),"deterministicVideoVerified":false})
}
pub(crate) fn sample(screen: usize) -> Option<Value> {
    let clock = pointer(screen.checked_add(scene::screen::CLOCK)?)?;
    let renderer = clock.checked_sub(scene::game_renderer::CLOCK)?;
    let camera = crate::native_clock::owned_player_renderer(renderer)?;
    let (current, seconds) = owner(camera, screen)?;
    let state = STATE.lock().ok()?;
    let history = state.history.as_ref()?;
    (history.owner == current && history.seconds.to_bits() == seconds.to_bits())
        .then(|| state.last.clone())
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_evaluations_reuse_starting_state_but_new_time_and_owner_do_not() {
        let owner = Owner {
            session: "world".into(),
            generation: 3,
            camera: 100,
        };
        let native = Snapshot([0; BYTES]);
        let changed = Snapshot([1; BYTES]);
        let (first, repeated) = History::next(None, owner.clone(), 10., native.clone()).unwrap();
        assert!(!repeated);
        let (held, repeated) =
            History::next(Some(&first), owner.clone(), 10., changed.clone()).unwrap();
        assert!(repeated);
        assert!(held.baseline == native);
        let (step, repeated) =
            History::next(Some(&held), owner.clone(), 10.05, changed.clone()).unwrap();
        assert!(!repeated);
        assert!(step.baseline == changed);
        assert!(History::next(Some(&step), owner.clone(), 10., native.clone()).is_none());
        for new in [
            Owner {
                generation: 4,
                ..owner.clone()
            },
            Owner {
                camera: 200,
                ..owner.clone()
            },
            Owner {
                session: "next".into(),
                ..owner
            },
        ] {
            let (reset, repeated) = History::next(Some(&step), new, 1., native.clone()).unwrap();
            assert!(!repeated);
            assert!(reset.baseline == native);
        }
    }
    #[test]
    fn scalar_restore_excludes_particle_resources_and_checks_native_values() {
        let mut memory = vec![0u8; 0x980];
        memory[0x4e0..0x5c0].fill(0xa5);
        let address = memory.as_ptr() as usize;
        let baseline = Snapshot::capture(address).unwrap();
        memory[0x42c..0x430].copy_from_slice(&0.75f32.to_le_bytes());
        memory[0x5c0..0x5c4].copy_from_slice(&1.5f32.to_le_bytes());
        assert!(Snapshot::capture(address).unwrap() != baseline);
        assert!(baseline.write(address));
        assert!(memory[0x4e0..0x5c0].iter().all(|v| *v == 0xa5));
        memory[0x4db] = 2;
        assert!(Snapshot::capture(address).is_none());
        memory[0x4db] = 0;
        memory[0x42c..0x430].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(Snapshot::capture(address).is_none());
        assert!(Snapshot::capture(0).is_none());
    }
    #[test]
    fn native_fog_detour_forwards_the_two_pointers_and_float_once() {
        static ARGS: Mutex<Option<(usize, usize, u32)>> = Mutex::new(None);
        static COUNT: AtomicU64 = AtomicU64::new(0);
        #[inline(never)]
        unsafe extern "system" fn host(a: usize, b: usize, c: f32) {
            *ARGS.lock().unwrap() = Some((a, b, c.to_bits()));
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
            unsafe { MH_CreateHook(target as _, setup as *const () as _, &mut trampoline) },
            MH_STATUS::MH_OK
        );
        ORIGINAL.store(trampoline as usize, Ordering::Release);
        assert_eq!(unsafe { MH_EnableHook(target as _) }, MH_STATUS::MH_OK);
        let call: Setup = std::hint::black_box(host);
        let sky = f32::from_bits(0x7fc01234);
        unsafe { call(17, 23, sky) };
        assert_eq!(unsafe { MH_DisableHook(target as _) }, MH_STATUS::MH_OK);
        ORIGINAL.store(target, Ordering::Release);
        assert_eq!(*ARGS.lock().unwrap(), Some((17, 23, sky.to_bits())));
        assert_eq!(COUNT.load(Ordering::Relaxed), 1);
    }
}
