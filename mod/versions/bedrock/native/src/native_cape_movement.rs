//! Ordered finalized displacements within one primary-client tick. Observes
//! existing native calls; never changes requests, components or ownership.
use crate::{
    ecs,
    world_hooks::{floats, pointer, read},
};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
};
const RVA: usize = 0x63db090;
const MAX_CALLS: usize = 64;
type Finalize = unsafe extern "system" fn(usize, usize, usize, usize, usize, usize);
static ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static INSTALLED: AtomicBool = AtomicBool::new(false);
static INSTALL: OnceLock<Result<(), String>> = OnceLock::new();
static MATCHED: AtomicU64 = AtomicU64::new(0);
static BATCHES: AtomicU64 = AtomicU64::new(0);
static REJECTED: AtomicU64 = AtomicU64::new(0);
static APPLIED: AtomicU64 = AtomicU64::new(0);
static CLOAK_PHASES: AtomicU64 = AtomicU64::new(0);
struct Progress {
    last: [f32; 3],
    increments: Vec<f32>,
    busy: bool,
    invalid: bool,
}
struct Tracking {
    session: (usize, u64),
    previous: u64,
    id: String,
    address: usize,
    context: ecs::Context,
    key: ecs::RegistryKey,
    vector: usize,
    shape: usize,
    start: [f32; 3],
    start_feet: [f64; 3],
    active: AtomicBool,
    rejected: AtomicBool,
    progress: Mutex<Progress>,
}
impl Tracking {
    fn new(level: usize) -> Option<Self> {
        if !INSTALLED.load(Ordering::Acquire) || !crate::native_body::enabled() {
            return None;
        }
        let session = crate::players::session().filter(|value| value.0 == level)?;
        let previous = crate::world_hooks::client_completed_tick(level)?;
        let (id, address) = crate::native_capes::movement_target(session, previous)?;
        let actor = crate::players::actor(address).filter(|actor| actor.identity.id == id)?;
        let context = ecs::actor_context(address).ok()?;
        let key = ecs::registry_key(address)?;
        let vector = ecs::component(address, ecs::hash("StateVectorComponent"), 36).ok()??;
        let shape = ecs::component(address, ecs::hash("AABBShapeComponent"), 32).ok()??;
        let start = floats::<3>(vector, 32_000_000.)?;
        let bounds = floats::<6>(shape, 32_000_000.)?;
        if start.map(f32::to_bits) != actor.position.map(f32::to_bits)
            || bounds.map(f32::to_bits) != actor.components.as_ref()?.bounds.map(f32::to_bits)
        {
            return None;
        }
        // As in cape feet_position, use actual AABB minimum Y rather than
        // StateVector's stance/eye-height offset. Keep only owned coordinates.
        let start_feet = [
            f64::from(start[0]),
            f64::from(bounds[1]),
            f64::from(start[2]),
        ];
        let result = Self {
            session,
            previous,
            id,
            address,
            context,
            key,
            vector,
            shape,
            start,
            start_feet,
            active: AtomicBool::new(true),
            rejected: AtomicBool::new(false),
            progress: Mutex::new(Progress {
                last: start,
                increments: Vec::new(),
                busy: false,
                invalid: false,
            }),
        };
        if !result.current(previous)
            || floats::<3>(vector, 32_000_000.)?.map(f32::to_bits) != start.map(f32::to_bits)
            || floats::<6>(shape, 32_000_000.)?.map(f32::to_bits) != bounds.map(f32::to_bits)
        {
            return None;
        }
        Some(result)
    }
    fn current(&self, tick: u64) -> bool {
        let attempt = || -> Option<()> {
            if !self.active.load(Ordering::Acquire)
                || self.rejected.load(Ordering::Acquire)
                || crate::players::session() != Some(self.session)
                || crate::world_hooks::client_completed_tick(self.session.0) != Some(tick)
                || pointer(self.address.checked_add(0x1d8)?)? != self.session.0
                || ecs::registry_key(self.address) != Some(self.key)
                || ecs::actor_type(self.address)? != 319
            {
                return None;
            }
            let context = ecs::actor_context(self.address).ok()?;
            if context.registry != self.context.registry
                || context.entity != self.context.entity
                || ecs::component(self.address, ecs::hash("StateVectorComponent"), 36).ok()??
                    != self.vector
                || ecs::component(self.address, ecs::hash("AABBShapeComponent"), 32).ok()??
                    != self.shape
            {
                return None;
            }
            Some(())
        };
        attempt().is_some()
    }
    fn reject(&self) {
        self.rejected.store(true, Ordering::Release);
        if let Ok(mut state) = self.progress.try_lock() {
            state.invalid = true;
        }
        REJECTED.fetch_add(1, Ordering::Relaxed);
    }
    fn before(self: &Arc<Self>, entity: usize, shape: usize, vector: usize) -> Option<Call> {
        // Other entities are normal participants in the same query. Only the
        // actual local actor's StateVector address admits further inspection.
        if vector != self.vector {
            return None;
        }
        let attempt = || -> Option<Call> {
            let mut bytes = [0; 4];
            read(entity, &mut bytes).ok()?;
            if shape != self.shape
                || u32::from_le_bytes(bytes) != self.context.entity
                || !self.current(self.previous)
            {
                return None;
            }
            let before = floats::<3>(vector, 32_000_000.)?;
            let mut state = self.progress.try_lock().ok()?;
            if state.busy
                || state.invalid
                || state.increments.len() >= MAX_CALLS
                || before.map(f32::to_bits) != state.last.map(f32::to_bits)
            {
                state.invalid = true;
                return None;
            }
            state.busy = true;
            Some(Call {
                tracking: self.clone(),
                before,
                finished: false,
            })
        };
        let result = attempt();
        if result.is_none() {
            self.reject();
        }
        result
    }
}
thread_local! {
    static SCOPES:RefCell<Vec<Option<Arc<Tracking>>>>=const{RefCell::new(Vec::new())};
    static SKIPPED:Cell<usize>=const{Cell::new(0)};
}
pub(crate) struct Scope {
    pushed: bool,
    tracking: Option<Arc<Tracking>>,
}
pub(crate) fn begin(level: usize) -> Scope {
    let tracking = std::panic::catch_unwind(|| Tracking::new(level))
        .ok()
        .flatten()
        .map(Arc::new);
    let pushed = SCOPES.with(|scopes| {
        scopes.try_borrow_mut().ok().is_some_and(|mut scopes| {
            if scopes.len() < 8 {
                scopes.push(tracking.clone());
                true
            } else {
                false
            }
        })
    });
    if !pushed {
        SKIPPED.with(|value| value.set(value.get() + 1));
    }
    Scope {
        pushed,
        tracking: if pushed { tracking } else { None },
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        if let Some(tracking) = &self.tracking {
            tracking.active.store(false, Ordering::Release);
        }
        if self.pushed {
            SCOPES.with(|scopes| {
                if let Ok(mut scopes) = scopes.try_borrow_mut() {
                    scopes.pop();
                }
            });
        } else {
            SKIPPED.with(|value| value.set(value.get().saturating_sub(1)));
        }
    }
}
/// Owned math only. No native pointer or entity handle escapes the tick scope.
pub(crate) struct Batch {
    epoch: u64,
    id: String,
    tick: u64,
    start: [f32; 3],
    start_feet: [f64; 3],
    end: [f32; 3],
    increments: Vec<f32>,
}
impl Scope {
    pub(crate) fn finish(&self, tick: u64) -> Option<Batch> {
        let tracking = self.tracking.as_ref()?;
        let attempt = || -> Option<Batch> {
            if tracking.previous.checked_add(1) != Some(tick) || !tracking.current(tick) {
                return None;
            }
            let actor = crate::players::actor(tracking.address)
                .filter(|actor| actor.identity.id == tracking.id)?;
            let state = tracking.progress.try_lock().ok()?;
            if state.invalid
                || state.busy
                || state.increments.is_empty()
                || actor.position.map(f32::to_bits) != state.last.map(f32::to_bits)
                || !tracking.current(tick)
            {
                return None;
            }
            Some(Batch {
                epoch: tracking.session.1,
                id: tracking.id.clone(),
                tick,
                start: tracking.start,
                start_feet: tracking.start_feet,
                end: state.last,
                increments: state.increments.clone(),
            })
        };
        let result = attempt();
        if result.is_none() {
            tracking.reject();
        } else {
            BATCHES.fetch_add(1, Ordering::Relaxed);
        }
        tracking.active.store(false, Ordering::Release);
        result
    }
}
impl Batch {
    /// Only after the batch's identity/tick/endpoints have matched. A previous
    /// feet mismatch cannot authorize changing the filter's movement phase.
    pub(crate) fn cloak_input(&self, previous: [f64; 3]) -> Option<[f64; 3]> {
        (self.start_feet.map(f64::to_bits) == previous.map(f64::to_bits)).then_some(self.start_feet)
    }
    pub(crate) fn increments(
        &self,
        session: (usize, u64),
        actor: &crate::players::Actor,
        tick: u64,
        previous: [f64; 3],
        current: [f64; 3],
    ) -> Option<&[f32]> {
        if self.epoch != session.1
            || self.id != actor.identity.id
            || self.tick != tick
            || self.end.map(f32::to_bits) != actor.position.map(f32::to_bits)
            || [0, 2].into_iter().any(|axis| {
                f64::from(self.start[axis]).to_bits() != previous[axis].to_bits()
                    || f64::from(self.end[axis]).to_bits() != current[axis].to_bits()
            })
        {
            return None;
        }
        Some(&self.increments)
    }
}
pub(crate) fn applied(pre_movement_cloak: bool) {
    APPLIED.fetch_add(1, Ordering::Relaxed);
    if pre_movement_cloak {
        CLOAK_PHASES.fetch_add(1, Ordering::Relaxed);
    }
}
struct Call {
    tracking: Arc<Tracking>,
    before: [f32; 3],
    finished: bool,
}
impl Call {
    fn complete(&mut self) {
        let current = self.tracking.current(self.tracking.previous);
        let after = floats::<3>(self.tracking.vector, 32_000_000.);
        let mut state = match self.tracking.progress.try_lock() {
            Ok(value) => value,
            Err(_) => {
                self.tracking.reject();
                return;
            }
        };
        state.busy = false;
        self.finished = true;
        let Some(after) = after.filter(|_| current && !state.invalid) else {
            state.invalid = true;
            REJECTED.fetch_add(1, Ordering::Relaxed);
            return;
        };
        // Java narrows double endpoint differences before its double square
        // root, then performs one float multiply/add for each movement call.
        let x = (f64::from(after[0]) - f64::from(self.before[0])) as f32;
        let z = (f64::from(after[2]) - f64::from(self.before[2])) as f32;
        let increment = crate::native_capes::horizontal_length(x, z) * 0.6f32;
        if !increment.is_finite() {
            state.invalid = true;
            return;
        }
        state.last = after;
        state.increments.push(increment);
        MATCHED.fetch_add(1, Ordering::Relaxed);
    }
}
impl Drop for Call {
    fn drop(&mut self) {
        if !self.finished {
            self.tracking.reject();
            if let Ok(mut state) = self.tracking.progress.try_lock() {
                state.busy = false;
            }
        }
    }
}
unsafe extern "system" fn finalize(a: usize, b: usize, c: usize, d: usize, e: usize, f: usize) {
    let mut call = std::panic::catch_unwind(|| {
        if !INSTALLED.load(Ordering::Acquire) || SKIPPED.with(Cell::get) != 0 {
            return None;
        }
        let tracking = SCOPES.with(|scopes| scopes.try_borrow().ok()?.last()?.clone())?;
        tracking.before(b, c, f)
    })
    .ok()
    .flatten();
    let original: Finalize = unsafe { std::mem::transmute(ORIGINAL.load(Ordering::Acquire)) };
    unsafe { original(a, b, c, d, e, f) };
    if let Some(call) = call.as_mut() {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| call.complete()));
    }
}
pub(crate) fn install(base: usize) -> Result<(), String> {
    INSTALL
        .get_or_init(|| {
            use hudhook::mh::{MH_CreateHook, MH_EnableHook, MH_STATUS};
            if crate::world_hooks::image_base() != Some(base) {
                return Err("unvalidated cape movement image".into());
            }
            let sites: &[(usize, &[u8])] = &[
                (
                    RVA,
                    &[
                        0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53,
                        0x48, 0x81, 0xec, 0x98,
                    ],
                ),
                (0x63b428b, &[0x48, 0x8d, 0x05, 0xde, 0xf7, 0xff, 0xff]),
                (0x63b42c4, &[0xba, 0xcf, 0xb3, 0xc1, 0xba]),
                (0x63b42d4, &[0xba, 0x88, 0x72, 0x8f, 0x4c]),
                (0x63b42e4, &[0xba, 0x16, 0xc3, 0x21, 0xde]),
                (0x63b42f4, &[0xba, 0x38, 0x52, 0x5d, 0x1b]),
                (
                    0x63b4590,
                    &[0x4c, 0x89, 0x5c, 0x24, 0x28, 0x48, 0x89, 0x7c, 0x24, 0x20],
                ),
                (
                    0x63b3ace,
                    &[
                        0xf3, 0x0f, 0x11, 0x01, 0xf3, 0x0f, 0x11, 0x51, 0x04, 0xf3, 0x0f, 0x11,
                        0x49, 0x08,
                    ],
                ),
            ];
            for &(rva, expected) in sites {
                let mut bytes = vec![0; expected.len()];
                read(
                    base.checked_add(rva)
                        .ok_or("cape movement profile overflow")?,
                    &mut bytes,
                )?;
                if bytes != expected {
                    return Err(format!("cape movement profile differs at {rva:x}"));
                }
            }
            for rva in [0x63b45a7usize, 0x63b476b, 0x63b4941, 0x63b4b17] {
                let mut bytes = [0; 6];
                read(
                    base.checked_add(rva).ok_or("cape movement call overflow")?,
                    &mut bytes,
                )?;
                if bytes[0] != 0xe8
                    || rva as i64 + 5 + i32::from_le_bytes(bytes[1..5].try_into().unwrap()) as i64
                        != RVA as i64
                    || bytes[5] != 0xe9
                {
                    return Err("cape movement call/return differs".into());
                }
            }
            let target = base.checked_add(RVA).ok_or("cape movement hook overflow")?;
            let mut original = std::ptr::null_mut();
            let result =
                unsafe { MH_CreateHook(target as _, finalize as *const () as _, &mut original) };
            if result != MH_STATUS::MH_OK {
                return Err(format!("create cape movement hook: {result:?}"));
            }
            ORIGINAL.store(original as usize, Ordering::Release);
            let result = unsafe { MH_EnableHook(target as _) };
            if result != MH_STATUS::MH_OK {
                return Err(format!("enable cape movement hook: {result:?}"));
            }
            INSTALLED.store(true, Ordering::Release);
            Ok(())
        })
        .clone()
}
pub(crate) fn status() -> Value {
    json!({"installed":INSTALLED.load(Ordering::Acquire),"wrapperRva":RVA,
    "matchedFinalizations":MATCHED.load(Ordering::Relaxed),"completedBatches":BATCHES.load(Ordering::Relaxed),
    "appliedBatches":APPLIED.load(Ordering::Relaxed),"rejected":REJECTED.load(Ordering::Relaxed),"maximumCallsPerTick":MAX_CALLS,
    "preMovementCloakBatches":CLOAK_PHASES.load(Ordering::Relaxed),"preMovementCloakPhaseVerified":false,
    "nativeComponentsModified":false,"allLocalMovementCoverageVerified":false,"javaMovementCallEquivalenceVerified":false,"capePhysicsParityVerified":false})
}
