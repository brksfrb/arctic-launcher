//! Own copied cape inputs at actual native previous-position copy events.
//! The component observer is scoped to the primary client tick and remote
//! actors already joined to the level player directory and cape history.
use crate::{
    ecs,
    world_hooks::{floats, pointer, read},
};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
};
const RVA: usize = 0x31cb800;
const COPY: usize = 0x8b30500;
const MAX_ACTORS: usize = 128;
type CopyPrevious = unsafe extern "system" fn(usize, usize, usize);
static ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static BASE: AtomicUsize = AtomicUsize::new(0);
static INSTALLED: AtomicBool = AtomicBool::new(false);
static INSTALL: OnceLock<Result<(), String>> = OnceLock::new();
static COPIES: AtomicU64 = AtomicU64::new(0);
static APPLIED: AtomicU64 = AtomicU64::new(0);
static REJECTED: AtomicU64 = AtomicU64::new(0);
#[derive(Clone, Copy)]
struct Captured {
    position: [f32; 3],
    feet: [f64; 3],
}
struct Target {
    session: (usize, u64),
    previous: u64,
    id: String,
    address: usize,
    context: ecs::Context,
    key: ecs::RegistryKey,
    vector: usize,
    shape: usize,
    seen: AtomicBool,
    invalid: AtomicBool,
    captured: Mutex<Option<Captured>>,
}
impl Target {
    fn new(session: (usize, u64), previous: u64, id: String, address: usize) -> Option<Self> {
        crate::players::identified_actor(address, session, &id)?;
        let context = ecs::actor_context(address).ok()?;
        let key = ecs::registry_key(address)?;
        let vector = ecs::component(address, ecs::hash("StateVectorComponent"), 36).ok()??;
        let shape = ecs::component(address, ecs::hash("AABBShapeComponent"), 32).ok()??;
        let result = Self {
            session,
            previous,
            id,
            address,
            context,
            key,
            vector,
            shape,
            seen: AtomicBool::new(false),
            invalid: AtomicBool::new(false),
            captured: Mutex::new(None),
        };
        result.current(previous).then_some(result)
    }
    fn current(&self, tick: u64) -> bool {
        let attempt = || -> Option<()> {
            if self.invalid.load(Ordering::Acquire)
                || crate::world_hooks::client_completed_tick(self.session.0) != Some(tick)
                || ecs::registry_key(self.address) != Some(self.key)
            {
                return None;
            }
            crate::players::identified_actor(self.address, self.session, &self.id)?;
            let context = ecs::actor_context(self.address).ok()?;
            if context != self.context
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
        self.invalid.store(true, Ordering::Release);
        REJECTED.fetch_add(1, Ordering::Relaxed);
    }
}
struct Frame {
    session: (usize, u64),
    previous: u64,
    targets: HashMap<usize, Arc<Target>>,
    active: AtomicBool,
}
impl Frame {
    fn new(level: usize) -> Option<Self> {
        if !INSTALLED.load(Ordering::Acquire) || !crate::native_body::enabled() {
            return None;
        }
        let session = crate::players::session().filter(|session| session.0 == level)?;
        let previous = crate::world_hooks::client_completed_tick(level)?;
        let mut targets = HashMap::new();
        for (id, address) in crate::native_capes::remote_phase_targets(session, previous)? {
            if targets.len() >= MAX_ACTORS {
                return None;
            }
            if let Some(target) = Target::new(session, previous, id, address)
                && targets.insert(target.vector, Arc::new(target)).is_some()
            {
                return None;
            }
        }
        (!targets.is_empty()).then_some(Self {
            session,
            previous,
            targets,
            active: AtomicBool::new(true),
        })
    }
}
thread_local! {
    static SCOPES:RefCell<Vec<Option<Arc<Frame>>>>=const{RefCell::new(Vec::new())};
    static SKIPPED:Cell<usize>=const{Cell::new(0)};
}
pub(crate) struct Scope {
    pushed: bool,
    frame: Option<Arc<Frame>>,
}
pub(crate) fn begin(level: usize) -> Scope {
    let frame = std::panic::catch_unwind(|| Frame::new(level))
        .ok()
        .flatten()
        .map(Arc::new);
    let pushed = SCOPES.with(|scopes| {
        scopes.try_borrow_mut().ok().is_some_and(|mut scopes| {
            if scopes.len() < 8 {
                scopes.push(frame.clone());
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
        frame: if pushed { frame } else { None },
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        if let Some(frame) = &self.frame {
            frame.active.store(false, Ordering::Release);
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
struct Endpoint {
    input: Captured,
    end: [f32; 3],
}
/// Owned endpoints and directory epoch only; never retains engine addresses.
pub(crate) struct Batch {
    epoch: u64,
    tick: u64,
    actors: HashMap<String, Endpoint>,
}
impl Scope {
    pub(crate) fn finish(&self, tick: u64) -> Option<Batch> {
        let frame = self.frame.as_ref()?;
        let attempt = || -> Option<Batch> {
            if !frame.active.load(Ordering::Acquire)
                || frame.previous.checked_add(1) != Some(tick)
                || crate::players::session() != Some(frame.session)
            {
                return None;
            }
            let mut actors = HashMap::new();
            for target in frame.targets.values() {
                let sample = || -> Option<Endpoint> {
                    if !target.current(tick) {
                        return None;
                    }
                    let input = target.captured.try_lock().ok()?.as_ref().copied()?;
                    let end = floats::<3>(target.vector, 32_000_000.)?;
                    if floats::<3>(target.vector.checked_add(12)?, 32_000_000.)?.map(f32::to_bits)
                        != input.position.map(f32::to_bits)
                        || !target.current(tick)
                    {
                        return None;
                    }
                    Some(Endpoint { input, end })
                };
                if let Some(value) = sample() {
                    actors.insert(target.id.clone(), value);
                }
            }
            (!actors.is_empty()).then_some(Batch {
                epoch: frame.session.1,
                tick,
                actors,
            })
        };
        let result = attempt();
        frame.active.store(false, Ordering::Release);
        result
    }
}
impl Batch {
    pub(crate) fn input(
        &self,
        session: (usize, u64),
        actor: &crate::players::Actor,
        tick: u64,
        previous: [f64; 3],
    ) -> Option<[f64; 3]> {
        if self.epoch != session.1 || self.tick != tick {
            return None;
        }
        let endpoint = self.actors.get(&actor.identity.id)?;
        if endpoint.input.feet.map(f64::to_bits) != previous.map(f64::to_bits)
            || endpoint.input.position.map(f32::to_bits) != actor.old_position.map(f32::to_bits)
            || endpoint.end.map(f32::to_bits) != actor.position.map(f32::to_bits)
        {
            return None;
        }
        Some(endpoint.input.feet)
    }
}
struct Call {
    frame: Arc<Frame>,
    target: Arc<Target>,
    position: [f32; 3],
    bounds: [f32; 6],
    finished: bool,
}
impl Call {
    fn before(frame: Arc<Frame>, target: Arc<Target>, entity: usize) -> Option<Self> {
        if target.seen.swap(true, Ordering::AcqRel) {
            target.reject();
            return None;
        }
        let attempt = || -> Option<()> {
            let mut bytes = [0; 4];
            read(entity, &mut bytes).ok()?;
            if !frame.active.load(Ordering::Acquire)
                || u32::from_le_bytes(bytes) != target.context.entity
                || !target.current(target.previous)
            {
                return None;
            }
            Some(())
        };
        if attempt().is_none() {
            target.reject();
            return None;
        }
        let sample = || {
            Some((
                floats::<3>(target.vector, 32_000_000.)?,
                floats::<6>(target.shape, 32_000_000.)?,
            ))
        };
        let Some((position, bounds)) = sample() else {
            target.reject();
            return None;
        };
        if (0..3)
            .any(|axis| bounds[axis + 3] < bounds[axis] || bounds[axis + 3] - bounds[axis] > 64.)
        {
            target.reject();
            return None;
        }
        Some(Self {
            frame,
            target,
            position,
            bounds,
            finished: false,
        })
    }
    fn complete(&mut self) {
        let attempt = || -> Option<()> {
            if !self.frame.active.load(Ordering::Acquire)
                || !self.target.current(self.target.previous)
                || floats::<3>(self.target.vector, 32_000_000.)?.map(f32::to_bits)
                    != self.position.map(f32::to_bits)
                || floats::<3>(self.target.vector.checked_add(12)?, 32_000_000.)?.map(f32::to_bits)
                    != self.position.map(f32::to_bits)
                || floats::<6>(self.target.shape, 32_000_000.)?.map(f32::to_bits)
                    != self.bounds.map(f32::to_bits)
            {
                return None;
            }
            let feet = [
                f64::from(self.position[0]),
                f64::from(self.bounds[1]),
                f64::from(self.position[2]),
            ];
            *self.target.captured.try_lock().ok()? = Some(Captured {
                position: self.position,
                feet,
            });
            Some(())
        };
        let captured = attempt().is_some();
        self.finished = true;
        if captured {
            COPIES.fetch_add(1, Ordering::Relaxed);
        } else {
            self.target.reject();
        }
    }
}
impl Drop for Call {
    fn drop(&mut self) {
        if !self.finished {
            self.target.reject();
        }
    }
}
unsafe extern "system" fn copy_previous(a: usize, b: usize, c: usize) {
    let mut call = std::panic::catch_unwind(|| {
        if !INSTALLED.load(Ordering::Acquire) || SKIPPED.with(Cell::get) != 0 {
            return None;
        }
        let frame = SCOPES.with(|scopes| scopes.try_borrow().ok()?.last()?.clone())?;
        let target = frame.targets.get(&c)?.clone();
        // This wrapper is shared by other ECS queries. Admit only the pinned
        // actual previous-position copy target, not arbitrary callbacks.
        if pointer(pointer(a.checked_add(0x10)?)?)?
            != BASE.load(Ordering::Acquire).checked_add(COPY)?
        {
            return None;
        }
        Call::before(frame, target, b)
    })
    .ok()
    .flatten();
    let original: CopyPrevious = unsafe { std::mem::transmute(ORIGINAL.load(Ordering::Acquire)) };
    unsafe { original(a, b, c) };
    if let Some(call) = call.as_mut() {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| call.complete()));
    }
}
pub(crate) fn applied() {
    APPLIED.fetch_add(1, Ordering::Relaxed);
}
pub(crate) fn install(base: usize) -> Result<(), String> {
    INSTALL
        .get_or_init(|| {
            use hudhook::mh::{MH_CreateHook, MH_EnableHook, MH_STATUS};
            if crate::world_hooks::image_base() != Some(base) {
                return Err("unvalidated cape phase image".into());
            }
            let sites: &[(usize, &[u8])] = &[
                (
                    RVA,
                    &[
                        0x55, 0x56, 0x57, 0x53, 0x48, 0x83, 0xec, 0x58, 0x48, 0x8d, 0x6c, 0x24,
                        0x50,
                    ],
                ),
                (0x31cb866, &[0x48, 0x8b, 0x4f, 0x10]),
                (0x31cb876, &[0x48, 0x8b, 0x01]),
                (
                    COPY,
                    &[
                        0x8b, 0x42, 0x08, 0x89, 0x42, 0x14, 0x48, 0x8b, 0x02, 0x48, 0x89, 0x42,
                        0x0c, 0xc3,
                    ],
                ),
                (0x90351c6, &[0x48, 0x8d, 0x05, 0x33, 0xb3, 0xaf, 0xff]),
                (0x8b5adfe, &[0x48, 0x8d, 0x05, 0xfb, 0x56, 0xfd, 0xff]),
                (0x28b9f0d, &[0xba, 0x38, 0x52, 0x5d, 0x1b]),
                (0x28b9f1d, &[0xba, 0xce, 0x4c, 0x15, 0x94]),
            ];
            for &(rva, expected) in sites {
                let mut bytes = vec![0; expected.len()];
                read(
                    base.checked_add(rva).ok_or("cape phase profile overflow")?,
                    &mut bytes,
                )?;
                if bytes != expected {
                    return Err(format!("cape phase profile differs at {rva:x}"));
                }
            }
            let target = base.checked_add(RVA).ok_or("cape phase hook overflow")?;
            let mut original = std::ptr::null_mut();
            let result = unsafe {
                MH_CreateHook(target as _, copy_previous as *const () as _, &mut original)
            };
            if result != MH_STATUS::MH_OK {
                return Err(format!("create cape phase hook: {result:?}"));
            }
            ORIGINAL.store(original as usize, Ordering::Release);
            BASE.store(base, Ordering::Release);
            let result = unsafe { MH_EnableHook(target as _) };
            if result != MH_STATUS::MH_OK {
                return Err(format!("enable cape phase hook: {result:?}"));
            }
            INSTALLED.store(true, Ordering::Release);
            Ok(())
        })
        .clone()
}
pub(crate) fn status() -> Value {
    json!({"installed":INSTALLED.load(Ordering::Acquire),"wrapperRva":RVA,"copyCallbackRva":COPY,
    "capturedCopies":COPIES.load(Ordering::Relaxed),"appliedRemoteInputs":APPLIED.load(Ordering::Relaxed),"rejected":REJECTED.load(Ordering::Relaxed),
    "maximumActorsPerTick":MAX_ACTORS,"nativeComponentsModified":false,"allRemoteTickPhasesVerified":false,"capePhysicsParityVerified":false})
}
