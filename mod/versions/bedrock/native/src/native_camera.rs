//! Exact-image camera component ownership and layout observations.
//! The separate opt-in controller scopes writes to an owned native render frame.
use crate::ecs::{
    Context, RegistryKey, actor_context, component_context, hash, owner_context,
    owner_registry_key, weak_context,
};
use crate::native_scene_layout as scene;
use crate::world_hooks::{floats, pointer, read};
use serde::Serialize;
use serde_json::{Value, json};
use std::cell::Cell;
use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};

type Update = unsafe extern "system" fn(usize, usize, usize);
static INSTALLED_BINDING: OnceLock<crate::world_hooks::CameraBinding> = OnceLock::new();
static ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static ITERATE_ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static UPDATE_ENABLED: AtomicBool = AtomicBool::new(false);
static ITERATE_ENABLED: AtomicBool = AtomicBool::new(false);
static CALLS: AtomicU64 = AtomicU64::new(0);
static TARGETS: AtomicU64 = AtomicU64::new(0);
static COMPONENTS: AtomicU64 = AtomicU64::new(0);
static REGISTRY_COMPONENTS: AtomicU64 = AtomicU64::new(0);
static SAMPLES: AtomicU64 = AtomicU64::new(0);
static CONSUMER_STEP: AtomicUsize = AtomicUsize::new(0);
static CONSUMER_MAX_STEP: AtomicUsize = AtomicUsize::new(0);
fn consumer_step(value: usize) {
    CONSUMER_STEP.store(value, Ordering::Relaxed);
    CONSUMER_MAX_STEP.fetch_max(value, Ordering::Relaxed);
}
#[derive(Clone, Copy)]
pub(crate) struct Source {
    owner: usize,
    registry_key: RegistryKey,
    context: Context,
    component: usize,
}
impl Source {
    pub(crate) fn entity(self) -> u32 {
        self.context.entity
    }
    fn render_camera(self) -> Option<Self> {
        self.component()?;
        let entity = crate::ecs::first_entity(
            self.context.registry,
            hash("MinecraftCamera::RenderCameraComponent"),
        )
        .ok()??;
        let context = owner_context(self.owner, entity)?;
        let component =
            component_context(context, hash("MinecraftCamera::CameraComponent"), 0x120).ok()??;
        let result = Self {
            owner: self.owner,
            registry_key: self.registry_key,
            context,
            component,
        };
        result.component()?;
        Some(result)
    }
    #[cfg(test)]
    pub(crate) fn fixture(owner: usize, entity: u32, component: usize) -> Self {
        Self {
            owner,
            registry_key: owner_registry_key(owner, entity).expect("live fixture registry"),
            context: Context {
                registry: owner + scene::entity_registry::REGISTRY,
                entity,
            },
            component,
        }
    }
    pub(crate) fn component(self) -> Option<usize> {
        (owner_registry_key(self.owner, self.context.entity) == Some(self.registry_key)
            && self.registry_key.registry == self.context.registry
            && component_context(
                self.context,
                hash("MinecraftCamera::CameraComponent"),
                0x120,
            )
            .ok()
            .flatten()
                == Some(self.component)
            && owner_registry_key(self.owner, self.context.entity) == Some(self.registry_key))
        .then_some(self.component)
    }
}
#[derive(Clone)]
pub(crate) struct RenderBinding {
    pub session: String,
    pub actor: usize,
    pub source: Source,
}
thread_local! {
    static SOURCE: Cell<Option<Source>> = const {Cell::new(None)};
}
struct SourceScope(Option<Source>);
impl Drop for SourceScope {
    fn drop(&mut self) {
        SOURCE.set(self.0);
    }
}
fn source(closure: usize, entity: usize, component: usize) -> Option<Source> {
    // 0x676c930: closure+8 references the registry owner; the second
    // argument points to the current entity. The third is the actual camera
    // component from the native sparse iterator, independently checked here.
    let capture = pointer(closure.checked_add(8)?)?;
    let owner = pointer(capture)?;
    let mut bytes = [0u8; 4];
    read(entity, &mut bytes).ok()?;
    let context = owner_context(owner, u32::from_le_bytes(bytes))?;
    let registry_key = owner_registry_key(owner, context.entity)?;
    if component_context(context, hash("MinecraftCamera::CameraComponent"), 0x120).ok()??
        != component
    {
        return None;
    }
    let result = Source {
        owner,
        registry_key,
        context,
        component,
    };
    result.component()?;
    Some(result)
}
unsafe extern "system" fn iterate(closure: usize, entity: usize, component: usize) {
    let next = std::panic::catch_unwind(|| source(closure, entity, component))
        .ok()
        .flatten();
    let _scope = SourceScope(SOURCE.replace(next));
    let original: Update = unsafe { std::mem::transmute(ITERATE_ORIGINAL.load(Ordering::Acquire)) };
    unsafe { original(closure, entity, component) };
}
static STATE: Mutex<State> = Mutex::new(State {
    binding: None,
    last: None,
    source: None,
});
#[derive(Clone, Copy)]
struct WeakCamera {
    reference: usize,
    context: Context,
    registry_key: RegistryKey,
    component: Option<usize>,
    kind: Option<u32>,
    preferred: bool,
}
fn context_key(context: Context) -> Option<RegistryKey> {
    owner_registry_key(
        context
            .registry
            .checked_sub(scene::entity_registry::REGISTRY)?,
        context.entity,
    )
}
fn weak_camera(client: usize) -> Option<WeakCamera> {
    let base = crate::world_hooks::image_base()?;
    let table = pointer(client)?;
    if table.checked_sub(base)? != scene::client::VTABLE
        || pointer(table.checked_add(scene::client::CAMERA_GETTER_SLOT * 8)?)?
            != base.checked_add(scene::client::CAMERA_GETTER_RVA)?
    {
        return None;
    }
    for (offset, preferred) in [
        (scene::client::WEAK_CAMERA_REFERENCE, true),
        (scene::client::WEAK_PLAYER_REFERENCE, false),
    ] {
        let reference = client.checked_add(offset)?;
        let Some(context) = weak_context(reference) else {
            continue;
        };
        let registry_key = context_key(context)?;
        // CameraComponent is 0x120 bytes with its HashedString and three cached
        // matrices. The old small camera declaration is insufficient here.
        let mut candidates = Vec::new();
        for kind in [
            hash("MinecraftCamera::CameraComponent"),
            hash("CameraComponent"),
        ] {
            if let Ok(Some(component)) = component_context(context, kind, 0x120) {
                candidates.push((component, kind));
            }
        }
        if weak_context(reference) != Some(context) || context_key(context)? != registry_key {
            return None;
        }
        let (component, kind) = if candidates.len() == 1 {
            (Some(candidates[0].0), Some(candidates[0].1))
        } else {
            (None, None)
        };
        return Some(WeakCamera {
            reference,
            context,
            registry_key,
            component,
            kind,
            preferred,
        });
    }
    None
}
struct Binding {
    game: usize,
    renderer: usize,
    client: usize,
    level: usize,
    actor: usize,
    context: Context,
    registry_key: RegistryKey,
    id: String,
    session: String,
    camera: Option<WeakCamera>,
    at: Instant,
}
impl Binding {
    fn live(&self) -> bool {
        self.at.elapsed() < Duration::from_secs(1)
            && crate::live_client::read_client(self.game, self.renderer).is_some_and(|c| {
                c.instance == self.client
                    && c.level == self.level
                    && c.local_player == Some(self.actor)
            })
            && actor_context(self.actor).ok() == Some(self.context)
            && crate::ecs::registry_key(self.actor) == Some(self.registry_key)
            && self.camera.is_none_or(|c| {
                weak_context(c.reference) == Some(c.context)
                    && context_key(c.context) == Some(c.registry_key)
            })
    }
}
struct State {
    binding: Option<Binding>,
    last: Option<(Instant, Observation)>,
    source: Option<(Instant, Source, std::thread::ThreadId)>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Camera {
    orientation: [f32; 4],
    position: [f32; 3],
    aspect_ratio: f32,
    /// Stored native perspective FOV, in radians rather than command degrees.
    field_of_view: f32,
    near_plane: f32,
    far_plane: f32,
}
fn camera(component: usize) -> Option<Camera> {
    let v = floats::<11>(component.checked_add(0x30)?, 32_000_000.)?;
    let orientation = [v[0], v[1], v[2], v[3]];
    let norm = orientation.iter().map(|v| v * v).sum::<f32>();
    if !(0.95..=1.05).contains(&norm)
        || !(0.1..=10.).contains(&v[7])
        // Native storage is radians. The old degree-shaped upper bound could
        // accept a field beyond the perspective pole; commands use 1..=179
        // degrees, while an observed native lens may be narrower than 1 degree.
        || v[8]<=0. || v[8]>=std::f32::consts::PI
        || !(0.0001..=10.).contains(&v[9])
        || v[10] <= v[9]
    {
        return None;
    }
    Some(Camera {
        orientation,
        position: [v[4], v[5], v[6]],
        aspect_ratio: v[7],
        field_of_view: v[8],
        near_plane: v[9],
        far_plane: v[10],
    })
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Observation {
    session: String,
    player: String,
    entity: u32,
    owned_target: bool,
    weak_entity_component_match: bool,
    camera_registry_component_match: bool,
    camera_entity: Option<u32>,
    look_mode: Option<u32>,
    camera: Camera,
    callers: Vec<usize>,
    observer_only: bool,
}
pub(crate) fn bind(
    game: usize,
    renderer: usize,
    client: &crate::live_client::Client,
    epoch: u64,
    id: &str,
) {
    let Some(actor) = client.local_player else {
        return;
    };
    let Ok(context) = actor_context(actor) else {
        return;
    };
    let Some(registry_key) = crate::ecs::registry_key(actor) else {
        return;
    };
    let session = format!("native_{}_{}", std::process::id(), epoch);
    let camera = weak_camera(client.instance);
    if let Ok(mut state) = STATE.lock() {
        if !state.binding.as_ref().is_some_and(|b| {
            b.session == session
                && b.actor == actor
                && b.context == context
                && b.registry_key == registry_key
        }) {
            state.last = None;
            state.source = None;
        }
        state.binding = Some(Binding {
            game,
            renderer,
            client: client.instance,
            level: client.level,
            actor,
            context,
            registry_key,
            id: id.to_owned(),
            session,
            camera,
            at: Instant::now(),
        });
    }
}
fn observe(component: usize, mode: usize, entity: usize) {
    let Ok(mut state) = STATE.lock() else { return };
    let Some(binding) = state.binding.as_ref().filter(|b| b.live()) else {
        return;
    };
    let target = actor_context(entity).ok();
    let owned_target = target == Some(binding.context);
    let weak_match = binding.camera.is_some_and(|c| {
        c.component == Some(component)
            && component_context(c.context, c.kind.unwrap_or(0), 0x120)
                .ok()
                .flatten()
                == Some(component)
    });
    let registry_source = SOURCE.get().filter(|s| s.component() == Some(component));
    let registry_match = registry_source.is_some();
    if !owned_target && !weak_match {
        return;
    }
    if owned_target {
        TARGETS.fetch_add(1, Ordering::Relaxed);
    }
    if weak_match {
        COMPONENTS.fetch_add(1, Ordering::Relaxed);
    }
    if registry_match {
        REGISTRY_COMPONENTS.fetch_add(1, Ordering::Relaxed);
    }
    let Some(camera) = camera(component) else {
        return;
    };
    let mut bytes = [0u8; 4];
    let look_mode = read(mode, &mut bytes)
        .ok()
        .map(|()| u32::from_le_bytes(bytes))
        .filter(|v| *v < 32);
    let observation = Observation {
        session: binding.session.clone(),
        player: binding.id.clone(),
        entity: target.map_or(binding.context.entity, |c| c.entity),
        owned_target,
        weak_entity_component_match: weak_match,
        camera_registry_component_match: registry_match,
        camera_entity: registry_source.map(|s| s.context.entity),
        look_mode,
        camera,
        callers: crate::world_hooks::game_callers()
            .into_iter()
            .take(16)
            .collect(),
        observer_only: true,
    };
    state.last = Some((Instant::now(), observation));
    if owned_target && let Some(source) = registry_source {
        state.source = Some((Instant::now(), source, std::thread::current().id()));
    }
    SAMPLES.fetch_add(1, Ordering::Relaxed);
}
pub(crate) fn consumer_binding(renderer: usize) -> Option<RenderBinding> {
    consumer_step(0);
    let state = STATE.lock().ok()?;
    let binding = state.binding.as_ref()?;
    consumer_step(1);
    if !binding.live() {
        return None;
    }
    consumer_step(2);
    if binding
        .renderer
        .checked_add(scene::level_renderer::PLAYER_OWNER)
        .and_then(pointer)
        != Some(renderer)
    {
        return None;
    }
    consumer_step(3);
    let (at, source, thread) = state.source?;
    consumer_step(4);
    if at.elapsed() >= Duration::from_millis(500) {
        return None;
    }
    consumer_step(5);
    if thread != std::thread::current().id() {
        return None;
    }
    consumer_step(6);
    let source = source.render_camera()?;
    consumer_step(7);
    Some(RenderBinding {
        session: binding.session.clone(),
        actor: binding.actor,
        source,
    })
}
pub(crate) fn snapshot(epoch: u64) -> Option<Value> {
    let state = STATE.lock().ok()?;
    let (at, sample) = state.last.as_ref()?;
    if at.elapsed() >= Duration::from_millis(500)
        || sample.session != format!("native_{}_{}", std::process::id(), epoch)
    {
        return None;
    }
    let mut value = json!(sample);
    value["ageMs"] = json!(at.elapsed().as_millis());
    Some(value)
}
unsafe extern "system" fn update(component: usize, mode: usize, entity: usize) {
    crate::native_camera_control::before_camera_update(component);
    CALLS.fetch_add(1, Ordering::Relaxed);
    let _ = std::panic::catch_unwind(|| observe(component, mode, entity));
    let original: Update = unsafe { std::mem::transmute(ORIGINAL.load(Ordering::Acquire)) };
    unsafe { original(component, mode, entity) };
}
pub(crate) unsafe fn install() -> Result<(), String> {
    let base = crate::world_hooks::image_base().ok_or("camera image profile unavailable")?;
    let binding = crate::world_hooks::camera_binding()?;
    let target = base
        .checked_add(binding.update_rva)
        .ok_or("camera update address overflow")?;
    let iterator = base
        .checked_add(binding.iterator_rva)
        .ok_or("camera iterator address overflow")?;
    let mut code = [0u8; 19];
    read(target, &mut code)?;
    if code
        != [
            0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x55, 0x53, 0x48, 0x81,
            0xec, 0xd8, 0x00, 0x00, 0x00,
        ]
    {
        return Err("native camera updater prologue differs".into());
    }
    let mut code = [0u8; 16];
    read(iterator, &mut code)?;
    if code
        != [
            0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x83,
            0xec, 0x78,
        ]
    {
        return Err("native camera iterator prologue differs".into());
    }
    // Both sites must pass before either detour is created. Publish both
    // trampolines before enabling callbacks, with the context source first.
    INSTALLED_BINDING
        .set(binding)
        .map_err(|_| "native camera installation already attempted")?;
    use hudhook::mh::{MH_CreateHook, MH_DisableHook, MH_EnableHook, MH_STATUS};
    let mut original = std::ptr::null_mut();
    let status = unsafe { MH_CreateHook(target as _, update as *const () as _, &mut original) };
    if status != MH_STATUS::MH_OK {
        return Err(format!("create camera observer: {status:?}"));
    }
    ORIGINAL.store(original as usize, Ordering::Release);
    let mut original = std::ptr::null_mut();
    let status = unsafe { MH_CreateHook(iterator as _, iterate as *const () as _, &mut original) };
    if status != MH_STATUS::MH_OK {
        return Err(format!("create camera iterator observer: {status:?}"));
    }
    ITERATE_ORIGINAL.store(original as usize, Ordering::Release);
    let status = unsafe { MH_EnableHook(iterator as _) };
    if status != MH_STATUS::MH_OK {
        return Err(format!("enable camera iterator observer: {status:?}"));
    }
    ITERATE_ENABLED.store(true, Ordering::Release);
    let status = unsafe { MH_EnableHook(target as _) };
    if status != MH_STATUS::MH_OK {
        let rollback = unsafe { MH_DisableHook(iterator as _) };
        if matches!(rollback, MH_STATUS::MH_OK | MH_STATUS::MH_ERROR_DISABLED) {
            ITERATE_ENABLED.store(false, Ordering::Release);
        }
        // Retain published originals for callbacks that are already in flight.
        return Err(format!(
            "enable camera observer: {status:?}; disable iterator rollback: {rollback:?}"
        ));
    }
    UPDATE_ENABLED.store(true, Ordering::Release);
    Ok(())
}
pub(crate) fn status() -> Value {
    let state = STATE.lock().ok();
    json!({"installed":UPDATE_ENABLED.load(Ordering::Acquire),"hookRva":INSTALLED_BINDING.get().map(|b|b.update_rva),"calls":CALLS.load(Ordering::Relaxed),
        "ownedTargetCalls":TARGETS.load(Ordering::Relaxed),"weakEntityComponentCalls":COMPONENTS.load(Ordering::Relaxed),"validSamples":SAMPLES.load(Ordering::Relaxed),
        "registryComponentCalls":REGISTRY_COMPONENTS.load(Ordering::Relaxed),"iteratorInstalled":ITERATE_ENABLED.load(Ordering::Acquire),
        "iteratorHookRva":INSTALLED_BINDING.get().map(|b|b.iterator_rva),
        "consumerBindingLastStep":CONSUMER_STEP.load(Ordering::Relaxed),"consumerBindingMaxStep":CONSUMER_MAX_STEP.load(Ordering::Relaxed),
        "binding":state.as_ref().and_then(|s|s.binding.as_ref().map(|b|json!({"session":b.session,"cameraWeakReferenceLive":b.camera.is_some(),
            "cameraComponentFound":b.camera.is_some_and(|c|c.component.is_some()),"preferredWeakReference":b.camera.map(|c|c.preferred)}))),
        "last":state.as_ref().and_then(|s|s.last.as_ref().map(|(_,v)|json!(v))),"nativeCameraControlVerified":false})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn camera_source_requires_exact_sparse_component_and_live_entity_generation() {
        fn point(bytes: &mut [u8], offset: usize, value: usize) {
            bytes[offset..offset + 8].copy_from_slice(&(value as u64).to_le_bytes());
        }
        let fixture =
            crate::ecs::Fixture::new(&[("MinecraftCamera::CameraComponent", &[0u8; 0x120])]);
        let registry = actor_context(fixture.address()).unwrap().registry;
        let mut owner = vec![0u8; 0x164];
        read(registry, &mut owner[0x30..0xb0]).unwrap();
        let mut sparse_page = vec![0xffu8; 2048 * 4];
        sparse_page[28..32].copy_from_slice(&0x40003u32.to_le_bytes());
        let sparse = [sparse_page.as_ptr() as usize];
        point(&mut owner, 0x110, sparse.as_ptr() as usize);
        point(&mut owner, 0x118, sparse.as_ptr() as usize + 8);
        point(&mut owner, 0x150, 4);
        let capture = [owner.as_ptr() as usize];
        let closure = [0usize, capture.as_ptr() as usize];
        let mut entity = 0x40007u32;
        let context = owner_context(owner.as_ptr() as usize, entity).unwrap();
        let component = component_context(context, hash("MinecraftCamera::CameraComponent"), 0x120)
            .unwrap()
            .unwrap();
        let closure_at = closure.as_ptr() as usize;
        let entity_at = &entity as *const u32 as usize;
        assert_eq!(
            source(closure_at, entity_at, component).unwrap().context,
            context
        );
        assert!(source(closure_at, entity_at, component + 0x120).is_none());
        entity = 0x80007;
        std::hint::black_box(&entity);
        assert!(source(closure_at, entity_at, component).is_none());
        entity = 0x40007;
        point(&mut owner, 0x150, 3);
        std::hint::black_box((&entity, &owner));
        assert!(source(closure_at, entity_at, component).is_none());
    }
    #[test]
    fn iterator_forwards_arguments_once_and_restores_the_prior_thread_scope() {
        static ARGS: Mutex<Vec<[usize; 3]>> = Mutex::new(Vec::new());
        #[inline(never)]
        unsafe extern "system" fn host(a: usize, b: usize, c: usize) {
            assert!(SOURCE.get().is_none());
            ARGS.lock().unwrap().push([a, b, c]);
        }
        let prior = Source {
            registry_key: RegistryKey {
                registry: 65,
                id: 0,
            },
            owner: 17,
            context: Context {
                registry: 65,
                entity: 3,
            },
            component: 99,
        };
        SOURCE.set(Some(prior));
        use hudhook::mh::{MH_CreateHook, MH_DisableHook, MH_EnableHook, MH_Initialize, MH_STATUS};
        assert!(matches!(
            unsafe { MH_Initialize() },
            MH_STATUS::MH_OK | MH_STATUS::MH_ERROR_ALREADY_INITIALIZED
        ));
        let target = host as *const () as usize;
        let mut trampoline = std::ptr::null_mut();
        assert_eq!(
            unsafe { MH_CreateHook(target as _, iterate as *const () as _, &mut trampoline) },
            MH_STATUS::MH_OK
        );
        ITERATE_ORIGINAL.store(trampoline as usize, Ordering::Release);
        assert_eq!(unsafe { MH_EnableHook(target as _) }, MH_STATUS::MH_OK);
        let call: Update = std::hint::black_box(host);
        unsafe { call(0x123456789, 0x234567891, 0x345678912) };
        assert_eq!(unsafe { MH_DisableHook(target as _) }, MH_STATUS::MH_OK);
        ITERATE_ORIGINAL.store(target, Ordering::Release);
        assert_eq!(
            *ARGS.lock().unwrap(),
            vec![[0x123456789, 0x234567891, 0x345678912]]
        );
        assert_eq!(SOURCE.get().map(|s| s.component), Some(99));
        SOURCE.set(None);
    }
    #[test]
    fn foreign_camera_values_reject_non_quaternions_and_invalid_clipping_planes() {
        let mut memory = vec![0u8; 0x120];
        // Field of view is stored in radians (70 degrees here).
        let values = [
            0f32,
            0.,
            0.,
            1.,
            10.,
            20.,
            30.,
            1.9,
            70f32.to_radians(),
            0.1,
            500.,
        ];
        for (i, v) in values.into_iter().enumerate() {
            memory[0x30 + i * 4..0x34 + i * 4].copy_from_slice(&v.to_le_bytes());
        }
        let at = memory.as_ptr() as usize;
        assert_eq!(camera(at).unwrap().position, [10., 20., 30.]);
        memory[0x3c..0x40].copy_from_slice(&2f32.to_le_bytes());
        std::hint::black_box(&memory);
        assert!(camera(at).is_none());
        memory[0x3c..0x40].copy_from_slice(&1f32.to_le_bytes());
        memory[0x58..0x5c].copy_from_slice(&0f32.to_le_bytes());
        std::hint::black_box(&memory);
        assert!(camera(at).is_none());
    }
    #[test]
    fn real_camera_detour_forwards_all_three_native_arguments_once() {
        static ARGS: Mutex<Vec<[usize; 3]>> = Mutex::new(Vec::new());
        #[inline(never)]
        unsafe extern "system" fn host(a: usize, b: usize, c: usize) {
            ARGS.lock().unwrap().push([a, b, c]);
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
        unsafe { call(0x123456789, 0x234567891, 0x345678912) };
        assert_eq!(unsafe { MH_DisableHook(target as _) }, MH_STATUS::MH_OK);
        ORIGINAL.store(target, Ordering::Release);
        assert_eq!(
            *ARGS.lock().unwrap(),
            vec![[0x123456789, 0x234567891, 0x345678912]]
        );
    }
}
