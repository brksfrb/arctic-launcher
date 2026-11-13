//! Observe D3D12 depth commands and apply their states at queue submission,
//! never at command recording time. Ambiguous or incomplete traces are rejected.
use std::cell::Cell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use hudhook::hooks::dx12::{CommandQueueObserver, set_queue_observer};
use hudhook::mh::{MH_CreateHook, MH_EnableHook, MH_STATUS};
use hudhook::windows::Win32::Foundation::{E_NOINTERFACE, RECT};
use hudhook::windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL;
use hudhook::windows::Win32::Graphics::Direct3D12::*;
use hudhook::windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use hudhook::windows::core::{BOOL, GUID, HRESULT, IUnknown, Interface, s, w};

#[path = "depth_queue.rs"]
mod native_queue;
#[path = "depth_snapshot.rs"]
mod snapshot;

pub(crate) fn preflight_native_queue(base: usize) -> Result<(), String> {
    native_queue::preflight(base)
}
pub(crate) unsafe fn observe_native_queue(base: usize) -> Result<(), String> {
    unsafe { native_queue::install(base) }
}

const MAX_LISTS: usize = 512;
const MAX_EVENTS: usize = 8192;
const MAX_RESOURCES: usize = 32;
const MAX_RESOURCE_BYTES: u64 = 256 * 1024 * 1024;
static NEXT_ID: AtomicU64 = AtomicU64::new(1);
static SUBMIT: Mutex<()> = Mutex::new(());
static TRACKER: OnceLock<Mutex<Tracker>> = OnceLock::new();
static INSTALL: OnceLock<Result<(), String>> = OnceLock::new();
// Published under the tracker lock only after every public observer is enabled
// and all history collected with incomplete coverage has been invalidated.
static GRAPHICS_READY: AtomicBool = AtomicBool::new(false);
pub(crate) fn native_asset_hooks_available() -> bool {
    INSTALL.get().is_some_and(Result::is_ok)
        && GRAPHICS_READY.load(Ordering::Acquire)
        && crate::native_graphics_observer::enabled()
        && !SUBMIT.is_poisoned()
        && !OWN_COPY.try_with(Cell::get).unwrap_or(true)
}
static DEVICE_HOOK: OnceLock<Result<(), String>> = OnceLock::new();
static DEVICE_CREATIONS: AtomicU64 = AtomicU64::new(0);
static RESET_TARGET: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static RELEASE_TARGET: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
type CreateDevice = unsafe extern "system" fn(
    *mut c_void,
    D3D_FEATURE_LEVEL,
    *const GUID,
    *mut *mut c_void,
) -> HRESULT;
static CREATE_DEVICE: OnceLock<CreateDevice> = OnceLock::new();

unsafe extern "system" fn created_device(
    adapter: *mut c_void,
    level: D3D_FEATURE_LEVEL,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    let result = unsafe {
        (CREATE_DEVICE.get().expect("published device trampoline"))(adapter, level, iid, out)
    };
    if result.is_ok() && !out.is_null() {
        let _ = std::panic::catch_unwind(|| unsafe {
            let raw = *out;
            if let Some(object) = IUnknown::from_raw_borrowed(&raw)
                && let Ok(device) = object.cast::<ID3D12Device>()
            {
                DEVICE_CREATIONS.fetch_add(1, Ordering::Relaxed);
                let _ = initialize(&device);
            }
        });
    }
    result
}

/// Install after hudhook's own device discovery, before the game's render device
/// is created. This observes the actual returned device and catches PSOs/DSVs
/// allocated before the first swap-chain frame. Present-time discovery remains
/// a fallback for ordinary late attachment, with its documented trace limits.
pub fn observe_device_creation() -> Result<(), String> {
    DEVICE_HOOK
        .get_or_init(|| unsafe {
            // Keep this module reference for the lifetime of the trampoline.
            let module = LoadLibraryW(w!("d3d12.dll")).map_err(|e| e.to_string())?;
            let target = GetProcAddress(module, s!("D3D12CreateDevice"))
                .ok_or("D3D12CreateDevice unavailable")?;
            let mut original = std::ptr::null_mut();
            let status = MH_CreateHook(
                target as *const () as _,
                created_device as *const () as _,
                &mut original,
            );
            if status != MH_STATUS::MH_OK {
                return Err(format!("create device observer: {status:?}"));
            }
            CREATE_DEVICE
                .set(std::mem::transmute::<*mut c_void, CreateDevice>(original))
                .map_err(|_| "device observer already published")?;
            let status = MH_EnableHook(target as *const () as _);
            if status != MH_STATUS::MH_OK {
                return Err(format!("enable device observer: {status:?}"));
            }
            Ok(())
        })
        .clone()
}
thread_local! { static OWN_COPY:Cell<bool> = const{Cell::new(false)}; }
pub(crate) struct OwnCopy {
    previous: bool,
}
/// Balanced Arctic-owned recording/submission must not become evidence of
/// native world production. Preserve an enclosing readback/private scope too.
pub(crate) fn private_commands() -> OwnCopy {
    OwnCopy {
        previous: OWN_COPY.with(|copy| copy.replace(true)),
    }
}
impl Drop for OwnCopy {
    fn drop(&mut self) {
        let _ = OWN_COPY.try_with(|copy| copy.set(self.previous));
    }
}
fn native_edit_gate() -> Option<std::sync::MutexGuard<'static, ()>> {
    if OWN_COPY.try_with(Cell::get).unwrap_or(false) {
        return None;
    }
    match SUBMIT.lock() {
        Ok(gate) => Some(gate),
        Err(_) => {
            snapshot::fault_all();
            crate::native_draw_assets::fault_all();
            None
        }
    }
}

struct Resource {
    id: u64,
    resource: ID3D12Resource,
    width: u32,
    height: u32,
    estimated_bytes: u64,
}
#[derive(Clone)]
enum Op {
    Transition(
        Arc<Resource>,
        D3D12_RESOURCE_STATES,
        D3D12_RESOURCE_STATES,
        D3D12_RESOURCE_BARRIER_FLAGS,
    ),
    Clear(Arc<Resource>, Option<u64>, Option<bool>, bool, bool),
    Draw(
        Arc<Resource>,
        Option<u64>,
        bool,
        Option<bool>,
        Option<crate::native_gpu_constants::DrawConstants>,
    ),
    Snapshot(Arc<Resource>, snapshot::Ticket, D3D12_RESOURCE_STATES),
    Discard(Arc<Resource>),
    ColorClear(u64),
    Unknown,
}
#[derive(Default)]
struct List {
    graphics: crate::native_graphics_state::State,
    surfaces: crate::native_bound_surfaces::State,
    events: Vec<Op>,
    bound: Option<Arc<Resource>>,
    viewport: Option<D3D12_VIEWPORT>,
    depth_write: Option<bool>,
    read_only_depth: bool,
    render_pass: Option<(Arc<Resource>, bool)>,
    unknown: bool,
    closed: bool,
    frame: u64,
    root_signature: Option<usize>,
    cbv: Option<u64>,
    cbf: Option<u64>,
    shaders: Option<crate::native_shader_layout::ShaderPair>,
    root_binding: Option<crate::native_shader_layout::RootBinding>,
    color_targets: [u64; 8],
}
struct State {
    object: Arc<Resource>,
    state: Option<D3D12_RESOURCE_STATES>,
    split: bool,
    queue: usize,
    scope: Option<u64>,
    reverse_z: Option<bool>,
    full_viewport: bool,
    frame: u64,
    cleared_frame: Option<u64>,
}
#[derive(Default)]
struct Tracker {
    views: HashMap<usize, Arc<Resource>>,
    resources: HashMap<usize, State>,
    lists: HashMap<usize, List>,
    frame: u64,
    submissions: u64,
    unknown_submissions: u64,
    last_capture: Option<serde_json::Value>,
    scoped_draws: u64,
    unscoped_draws: u64,
    backend_draws: u64,
    cleared_scoped_depth: u64,
    overwritten_scoped_depth: u64,
    last_scoped_clear: Option<serde_json::Value>,
    unscoped_callers: Vec<Vec<usize>>,
    caller_samples: u8,
    closed_list_draws: u64,
    closed_list_samples: Vec<serde_json::Value>,
    missing_lists: u64,
    unclosed_lists: u64,
    invalid_lists: u64,
    list_evictions: u64,
    unknown_bindings: u64,
    enhanced_calls: u64,
    indirect_calls: u64,
    bundle_calls: u64,
    created_lists: u64,
    reset_lists: u64,
    closed_lists: u64,
    created_views: u64,
    rejected_views: Vec<serde_json::Value>,
}
fn tracker() -> &'static Mutex<Tracker> {
    TRACKER.get_or_init(Mutex::default)
}

impl Tracker {
    fn invalidate(&mut self) {
        crate::native_scene_target::invalidate();
        snapshot::invalidate_all();
        for state in self.resources.values_mut() {
            state.state = None;
            state.scope = None;
            state.split = false;
            state.cleared_frame = None;
            state.reverse_z = None;
        }
    }
    fn resource(&mut self, resource: &ID3D12Resource) -> Option<Arc<Resource>> {
        let key = resource.as_raw() as usize;
        if let Some(state) = self.resources.get_mut(&key) {
            state.frame = self.frame;
            return Some(state.object.clone());
        }
        // SAFETY: callers borrow a live COM argument during its API invocation.
        let d = unsafe { resource.GetDesc() };
        if d.Flags & D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL == D3D12_RESOURCE_FLAGS(0)
            || crate::depth::encoding(d.Format).is_none()
            || d.Dimension != D3D12_RESOURCE_DIMENSION_TEXTURE2D
            || d.DepthOrArraySize != 1
            || d.MipLevels != 1
            || d.SampleDesc.Count != 1
            || !(1..=8192).contains(&d.Width)
            || !(1..=8192).contains(&d.Height)
        {
            return None;
        }
        let estimated_bytes = d.Width * d.Height as u64 * 8;
        if estimated_bytes > MAX_RESOURCE_BYTES {
            return None;
        }
        while self.resources.len() >= MAX_RESOURCES
            || self
                .resources
                .values()
                .map(|state| state.object.estimated_bytes)
                .sum::<u64>()
                + estimated_bytes
                > MAX_RESOURCE_BYTES
        {
            let victim = self
                .resources
                .iter()
                .min_by_key(|(_, state)| state.frame)
                .map(|(key, state)| (*key, state.object.id))?;
            self.resources.remove(&victim.0);
            self.views.retain(|_, resource| resource.id != victim.1);
            for list in self.lists.values_mut() {
                let previous = list.events.len();
                list.events.retain(|op| match op {
                    Op::Transition(r, ..)
                    | Op::Clear(r, ..)
                    | Op::Draw(r, ..)
                    | Op::Snapshot(r, ..)
                    | Op::Discard(r) => r.id != victim.1,
                    Op::Unknown | Op::ColorClear(_) => true,
                });
                if list.events.len() != previous {
                    list.unknown = true;
                }
                if list.bound.as_ref().is_some_and(|r| r.id == victim.1) {
                    list.bound = None;
                    list.unknown = true;
                }
                if list
                    .render_pass
                    .as_ref()
                    .is_some_and(|(r, _)| r.id == victim.1)
                {
                    list.render_pass = None;
                    list.unknown = true;
                }
            }
        }
        let object = Arc::new(Resource {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            resource: resource.clone(),
            width: d.Width as u32,
            height: d.Height,
            estimated_bytes,
        });
        self.resources.insert(
            key,
            State {
                object: object.clone(),
                state: None,
                split: false,
                queue: 0,
                scope: None,
                reverse_z: None,
                full_viewport: false,
                frame: self.frame,
                cleared_frame: None,
            },
        );
        Some(object)
    }
    fn list(&mut self, key: usize) -> &mut List {
        if self.lists.len() >= MAX_LISTS
            && !self.lists.contains_key(&key)
            && let Some(oldest) = self
                .lists
                .iter()
                .min_by_key(|(_, list)| list.frame)
                .map(|(key, _)| *key)
        {
            self.lists.remove(&oldest);
            self.list_evictions += 1;
        }
        let list = self.lists.entry(key).or_default();
        list.frame = self.frame;
        list
    }
    fn push(&mut self, key: usize, op: Op) {
        let list = self.list(key);
        if list.events.len() < MAX_EVENTS {
            list.events.push(op)
        } else {
            list.events.clear();
            list.unknown = true;
        }
    }
    fn draw(&mut self, key: usize) {
        if self.lists.get(&key).is_some_and(|list| list.closed) {
            self.closed_list_draws += 1;
            if self.closed_list_draws <= 8 {
                // This is the live public graphics-list interface from Draw.
                let table = unsafe { *(key as *const *const usize) };
                let reset = unsafe { *table.add(10) };
                let release = unsafe { *table.add(2) };
                let sample = serde_json::json!({"resetTargetMatches":reset==RESET_TARGET.load(Ordering::Acquire),"releaseTargetMatches":release==RELEASE_TARGET.load(Ordering::Acquire)});
                if !self.closed_list_samples.contains(&sample) {
                    self.closed_list_samples.push(sample);
                }
            }
        }
        let list = self.list(key);
        let Some(resource) = list.bound.clone() else {
            return;
        };
        let full = full_viewport(list.viewport.as_ref(), &resource);
        let writes = if list.read_only_depth {
            Some(false)
        } else {
            list.depth_write
        };
        let scope = crate::world_hooks::current_pass()
            .or_else(|| crate::native_batch::draw_scope(key, list.cbv, list.cbf));
        let constants = list.cbv.zip(list.cbf).map(|(vertex, fragment)| {
            crate::native_gpu_constants::DrawConstants {
                vertex,
                fragment,
                shaders: list.shaders,
                root: list.root_binding,
                targets: list.color_targets,
            }
        });
        if scope.is_none() && writes != Some(false) {
            self.preserve(key, &resource);
        }
        if crate::render_bridge::current_submission().is_some() {
            self.backend_draws += 1;
        }
        if scope.is_some() {
            self.scoped_draws += 1;
        } else {
            self.unscoped_draws += 1;
            // A recent world pass only gates diagnostic sampling. It never
            // labels this deferred draw or relaxes depth selection.
            if full
                && writes == Some(true)
                && self.caller_samples < 8
                && crate::world_hooks::latest_scope().is_some()
            {
                self.caller_samples += 1;
                let callers = crate::world_hooks::game_callers();
                if !callers.is_empty() && !self.unscoped_callers.contains(&callers) {
                    self.unscoped_callers.push(callers);
                }
            }
        }
        self.push(key, Op::Draw(resource, scope, full, writes, constants));
    }
    fn preserve(&mut self, key: usize, resource: &Arc<Resource>) {
        let Some(before) = self
            .lists
            .get(&key)
            .and_then(|list| snapshot_candidate(list, resource.id))
        else {
            return;
        };
        // The key is borrowed from the live native method currently intercepted.
        // No list is reset, closed or submitted by this balanced plane-zero copy.
        match unsafe { snapshot::record(key, resource, before) } {
            Ok(Some(ticket)) => self.push(key, Op::Snapshot(resource.clone(), ticket, before)),
            Ok(None) => {}
            Err(error) => self.last_capture = Some(serde_json::json!({"snapshotError":error})),
        }
    }
    fn after_draw(&mut self, key: usize) {
        let Some(state) = self.lists.get_mut(&key) else {
            return;
        };
        if state.closed {
            state.surfaces.invalidate();
            return;
        }
        let raw = key as *mut c_void;
        // This key is the actual live public Draw callback's this pointer.
        if let Some(list) = unsafe { ID3D12GraphicsCommandList::from_raw_borrowed(&raw) } {
            state.surfaces.after_draw(list, &state.graphics);
        }
    }
    fn apply(&mut self, queue: usize, lists: &[usize]) {
        self.submissions += 1;
        for key in lists {
            let Some(list) = self.lists.get_mut(key) else {
                self.missing_lists += 1;
                self.unknown_submissions += 1;
                self.invalidate();
                continue;
            };
            list.frame = self.frame;
            let unknown = list.unknown || !list.closed;
            if list.unknown {
                self.invalid_lists += 1;
            }
            if !list.closed {
                self.unclosed_lists += 1;
            }
            let events = list.events.clone();
            if unknown {
                self.unknown_submissions += 1;
                self.invalidate();
                continue;
            }
            for op in events {
                let object = match &op {
                    Op::Transition(object, ..)
                    | Op::Clear(object, ..)
                    | Op::Draw(object, ..)
                    | Op::Snapshot(object, ..)
                    | Op::Discard(object) => object,
                    Op::Unknown => {
                        self.invalidate();
                        continue;
                    }
                    Op::ColorClear(id) => {
                        crate::native_scene_target::clear(*id);
                        continue;
                    }
                };
                let Some(state) = self.resources.get_mut(&(object.resource.as_raw() as usize))
                else {
                    continue;
                };
                if state.object.id != object.id {
                    continue;
                }
                state.frame = self.frame;
                state.queue = queue;
                match op {
                    Op::Transition(_, before, after, flags) => {
                        if flags == D3D12_RESOURCE_BARRIER_FLAG_BEGIN_ONLY {
                            state.split = true;
                            state.state = None;
                        } else if flags == D3D12_RESOURCE_BARRIER_FLAG_END_ONLY {
                            if state.split {
                                state.state = Some(after);
                                state.split = false;
                            } else {
                                state.state = None;
                                state.scope = None;
                            }
                        } else if flags == D3D12_RESOURCE_BARRIER_FLAG_NONE {
                            if state.state.is_some_and(|actual| actual != before) {
                                state.scope = None;
                                state.cleared_frame = None;
                            }
                            state.state = Some(after);
                            state.split = false;
                        } else {
                            state.state = None;
                            state.scope = None;
                        }
                    }
                    Op::Clear(_, _scope, reverse, _full, full_clear) => {
                        if let Some(scope) = state.scope {
                            self.cleared_scoped_depth += 1;
                            self.last_scoped_clear = Some(
                                serde_json::json!({"scope":scope,"frame":self.frame,"width":state.object.width,"height":state.object.height,"fullClear":full_clear,"fullViewport":state.full_viewport}),
                            );
                        }
                        state.state = Some(D3D12_RESOURCE_STATE_DEPTH_WRITE);
                        state.split = false;
                        state.scope = None;
                        state.full_viewport = false;
                        if full_clear {
                            state.reverse_z = reverse;
                            state.cleared_frame = Some(self.frame);
                        }
                    }
                    Op::Draw(_, scope, full, writes, constants) => {
                        if full && let (Some(scope), Some(constants)) = (scope, constants) {
                            crate::native_gpu_constants::observe_draw(queue, scope, constants);
                            crate::native_scene_target::draw(
                                queue,
                                constants.targets,
                                scope,
                                state.object.width,
                                state.object.height,
                            );
                        }
                        if scope.is_some() && writes != Some(false) {
                            snapshot::invalidate_source(state.object.id);
                        }
                        if writes.is_none() {
                            state.scope = None;
                            state.full_viewport = false;
                            state.cleared_frame = None;
                        } else if writes == Some(true)
                            && state.state.is_some_and(|s| {
                                s & D3D12_RESOURCE_STATE_DEPTH_WRITE != D3D12_RESOURCE_STATES(0)
                            })
                        {
                            if state.scope.is_some() && scope.is_none() {
                                self.overwritten_scoped_depth += 1;
                            }
                            state.scope = scope;
                            state.full_viewport = full;
                        }
                    }
                    Op::Discard(_) => {
                        state.scope = None;
                        state.cleared_frame = None;
                        state.reverse_z = None;
                    }
                    Op::Snapshot(_, ticket, before) => {
                        snapshot::apply(ticket, state, queue, self.frame, before)
                    }
                    Op::Unknown | Op::ColorClear(_) => unreachable!(),
                }
            }
        }
    }
    fn retire(&mut self) {
        self.frame += 1;
        let frame = self.frame;
        self.lists
            .retain(|_, list| frame.saturating_sub(list.frame) <= 120);
        self.resources
            .retain(|_, state| frame.saturating_sub(state.frame) <= 120);
        self.views.retain(|_, object| {
            self.resources
                .get(&(object.resource.as_raw() as usize))
                .is_some_and(|state| state.object.id == object.id)
        });
    }
}
fn snapshot_candidate(list: &List, source: u64) -> Option<D3D12_RESOURCE_STATES> {
    if list.unknown || list.closed || list.render_pass.is_some() {
        return None;
    }
    let mut state = None;
    for op in list.events.iter().rev() {
        match op {
            Op::Unknown => return None,
            Op::Transition(r, _, after, flags) if r.id == source => {
                if *flags != D3D12_RESOURCE_BARRIER_FLAG_NONE {
                    return None;
                }
                if state.is_none() {
                    state = Some(*after);
                }
            }
            Op::Draw(r, scope, full, writes, _) if r.id == source => {
                if *writes == Some(false) {
                    continue;
                }
                // A known writable DSV draw requires the exclusive DEPTH_WRITE
                // state. Explicit later transitions supersede that requirement.
                return (scope.is_some() && *full && *writes == Some(true))
                    .then_some(state.unwrap_or(D3D12_RESOURCE_STATE_DEPTH_WRITE));
            }
            Op::Clear(r, ..) | Op::Discard(r) | Op::Snapshot(r, ..) if r.id == source => {
                return None;
            }
            _ => {}
        }
    }
    None
}
fn full_viewport(viewport: Option<&D3D12_VIEWPORT>, object: &Resource) -> bool {
    viewport.is_some_and(|v| {
        v.TopLeftX == 0.
            && v.TopLeftY == 0.
            && v.Width == object.width as f32
            && v.Height == object.height as f32
            && v.MinDepth == 0.
            && v.MaxDepth == 1.
    })
}
fn observe(f: impl FnOnce(&mut Tracker)) {
    if OWN_COPY.try_with(Cell::get).unwrap_or(false) {
        return;
    }
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if let Ok(mut tracker) = tracker().lock() {
            f(&mut tracker)
        }
    }));
}

pub(crate) fn graphics_observe(
    list: usize,
    run: impl FnOnce(&mut crate::native_graphics_state::State),
) {
    observe(|tracker| {
        let state = &mut tracker.list(list).graphics;
        if !GRAPHICS_READY.load(Ordering::Acquire) {
            state.invalidate();
            return;
        }
        run(state);
    });
}
pub(crate) fn framebuffer_binding(
    list: &ID3D12GraphicsCommandList,
    epoch: crate::native_bound_surfaces::Epoch,
    frame: crate::render_bridge::FrameKey,
    attachments: crate::native_framebuffer_resources::Attachments,
) -> bool {
    let mut attached = false;
    observe(|tracker| {
        let key = list.as_raw() as usize;
        let Some(state) = tracker
            .lists
            .get(&key)
            .filter(|state| !state.closed && state.surfaces.accepts(epoch))
        else {
            return;
        };
        let old_bytes = state.surfaces.bytes();
        let old_count = usize::from(state.surfaces.bound());
        let Some(bound) = crate::native_bound_surfaces::Bound::copy(
            list,
            &state.graphics,
            epoch,
            frame,
            attachments,
        ) else {
            return;
        };
        let count = tracker
            .lists
            .values()
            .filter(|state| state.surfaces.bound())
            .count();
        let bytes = tracker.lists.values().fold(0u64, |total, state| {
            total.saturating_add(state.surfaces.bytes())
        });
        if count.saturating_sub(old_count) >= crate::native_bound_surfaces::MAX_BINDINGS
            || bytes
                .checked_sub(old_bytes)
                .and_then(|bytes| bytes.checked_add(bound.allocation_bytes()))
                .is_none_or(|bytes| bytes > crate::native_bound_surfaces::MAX_ALLOCATION_BYTES)
        {
            return;
        }
        if let Some(state) = tracker.lists.get_mut(&key) {
            attached = state.surfaces.attach(epoch, bound);
        }
    });
    attached
}

pub(crate) fn graphics_snapshot(
    list: &ID3D12GraphicsCommandList,
) -> Option<crate::native_graphics_state::Snapshot> {
    if !native_asset_hooks_available() {
        return None;
    }
    let tracker = tracker().try_lock().ok()?;
    let state = tracker.lists.get(&(list.as_raw() as usize))?;
    if state.closed {
        return None;
    }
    state.graphics.snapshot(list)
}

/// # Safety
/// Called by the owning adapter at its proven native feature phase on this
/// live recording list. Cohort native ownership/material/group contracts are
/// those of native_avatar_record::record. This is not a presentation fallback.
pub(crate) unsafe fn record_avatar(
    list: &ID3D12GraphicsCommandList,
    geometry: &Arc<crate::native_avatar_record::Geometry>,
    cohort: &crate::native_avatar_record::Cohort,
) -> Result<Option<crate::native_avatar_record::Recorded>, String> {
    if OWN_COPY.try_with(Cell::get).unwrap_or(true) {
        return Err("native avatar insertion is already inside private commands".into());
    }
    if !native_asset_hooks_available() {
        return Err("native graphics observer family is unavailable".into());
    }
    let tracker = tracker()
        .lock()
        .map_err(|_| "native avatar tracker lock is poisoned")?;
    let state = tracker
        .lists
        .get(&(list.as_raw() as usize))
        .filter(|state| !state.closed)
        .ok_or("native avatar recording list is unknown or closed")?;
    unsafe {
        crate::native_avatar_record::record(
            list,
            &state.graphics,
            &state.surfaces,
            geometry,
            cohort,
        )
    }
}

/// # Safety
/// The owning adapter is at this actual source/world phase and fulfills all
/// native_avatar_record::record_phase ownership and recording contracts.
pub(crate) unsafe fn record_avatar_phase(
    list: &ID3D12GraphicsCommandList,
    geometry: &Arc<crate::native_avatar_record::Geometry>,
    cohort: &crate::native_avatar_record::Cohort,
    phase: crate::avatar_draw_order::PhaseKey,
) -> Result<Option<crate::native_avatar_record::Recorded>, String> {
    if OWN_COPY.try_with(Cell::get).unwrap_or(true) {
        return Err("native avatar insertion is already inside private commands".into());
    }
    if !native_asset_hooks_available() {
        return Err("native graphics observer family is unavailable".into());
    }
    let tracker = tracker()
        .lock()
        .map_err(|_| "native avatar tracker lock is poisoned")?;
    let state = tracker
        .lists
        .get(&(list.as_raw() as usize))
        .filter(|state| !state.closed)
        .ok_or("native avatar recording list is unknown or closed")?;
    unsafe {
        crate::native_avatar_record::record_phase(
            list,
            &state.graphics,
            &state.surfaces,
            geometry,
            cohort,
            phase,
        )
    }
}

/// # Safety
/// The owning adapter supplies the actual wing glint model phase, native
/// ownership, preceding base depth and a valid uninterrupted model delivery.
/// Joint foreign-model sorting/interleaving remains an owning producer input.
pub(crate) unsafe fn record_avatar_glint(
    list: &ID3D12GraphicsCommandList,
    glint: &Arc<crate::native_avatar_record::GlintGeometry>,
    cohort: &crate::native_avatar_record::Cohort,
    phase: crate::avatar_draw_order::PhaseKey,
) -> Result<Option<crate::native_avatar_record::Recorded>, String> {
    if OWN_COPY.try_with(Cell::get).unwrap_or(true) {
        return Err("native glint insertion is already inside private commands".into());
    }
    if !native_asset_hooks_available() {
        return Err("native graphics observer family is unavailable".into());
    }
    let tracker = tracker()
        .lock()
        .map_err(|_| "native avatar tracker lock is poisoned")?;
    let state = tracker
        .lists
        .get(&(list.as_raw() as usize))
        .filter(|state| !state.closed)
        .ok_or("native avatar recording list is unknown or closed")?;
    unsafe {
        crate::native_avatar_record::record_glint(
            list,
            &state.graphics,
            &state.surfaces,
            glint,
            cohort,
            phase,
        )
    }
}

/// # Safety
/// The owner drives the actual complete model phase and fulfils the recording,
/// base-depth and native/resource contracts of record_glint_step.
pub(crate) unsafe fn record_avatar_glint_step(
    list: &ID3D12GraphicsCommandList,
    cursor: &mut crate::native_avatar_record::GlintCursor,
    cohort: &crate::native_avatar_record::Cohort,
    phase: crate::avatar_draw_order::PhaseKey,
) -> Result<Option<crate::native_avatar_record::Recorded>, String> {
    if OWN_COPY.try_with(Cell::get).unwrap_or(true) {
        return Err("native glint insertion is already inside private commands".into());
    }
    if !native_asset_hooks_available() {
        return Err("native graphics observer family is unavailable".into());
    }
    let tracker = tracker()
        .lock()
        .map_err(|_| "native avatar tracker lock is poisoned")?;
    let state = tracker
        .lists
        .get(&(list.as_raw() as usize))
        .filter(|state| !state.closed)
        .ok_or("native avatar recording list is unknown or closed")?;
    unsafe {
        crate::native_avatar_record::record_glint_step(
            list,
            &state.graphics,
            &state.surfaces,
            cursor,
            cohort,
            phase,
        )
    }
}

/// # Safety
/// The native owner actually delivered this exact foreign step outside the
/// tracker/private-command scope, then supplies its matching opaque token.
/// Acknowledgment does not execute/prove that draw or GPU completion.
pub(crate) unsafe fn acknowledge_avatar_glint_foreign(
    list: &ID3D12GraphicsCommandList,
    cursor: &mut crate::native_avatar_record::GlintCursor,
    cohort: &crate::native_avatar_record::Cohort,
    phase: crate::avatar_draw_order::PhaseKey,
    token: u64,
) -> Result<(), String> {
    if OWN_COPY.try_with(Cell::get).unwrap_or(true) {
        return Err("foreign delivery cannot be acknowledged inside private commands".into());
    }
    if !native_asset_hooks_available() {
        return Err("native graphics observer family is unavailable".into());
    }
    let tracker = tracker()
        .lock()
        .map_err(|_| "native avatar tracker lock is poisoned")?;
    let state = tracker
        .lists
        .get(&(list.as_raw() as usize))
        .filter(|state| !state.closed)
        .ok_or("native avatar recording list is unknown or closed")?;
    unsafe {
        crate::native_avatar_record::acknowledge_glint_foreign(
            list,
            &state.graphics,
            cursor,
            cohort,
            phase,
            token,
        )
    }
}

pub(crate) fn graphics_state_status() -> serde_json::Value {
    let Ok(tracker) = tracker().try_lock() else {
        return serde_json::Value::Null;
    };
    let installed = INSTALL.get().is_some_and(Result::is_ok)
        && GRAPHICS_READY.load(Ordering::Acquire)
        && crate::native_graphics_observer::enabled();
    serde_json::json!({"installed":installed,"publicSettersEnabled":crate::native_graphics_observer::enabled(),
        "completeObserverHistoryRequired":true,
        "trackedLists":tracker.lists.len(),"restorableCandidates":tracker.lists.values()
            .filter(|list|installed&&!list.closed&&list.graphics.restorable()).count(),
        "snapshotRequiresLiveListIdentity":true,"nativeInsertionImplemented":false,
        "boundSurfaceCandidates":tracker.lists.values().filter(|list|list.surfaces.bound()).count(),
        "scopedSurfaceCandidates":tracker.lists.values().filter(|list|list.surfaces.scoped()).count(),
        "retainedSurfaceAllocationBytes":tracker.lists.values().fold(0u64,|total,list|total.saturating_add(list.surfaces.bytes())),
        "stateRestorationVerified":false,"gpuRenderingVerified":false})
}

struct Observer;
impl CommandQueueObserver for Observer {
    fn execute(
        &self,
        queue: &ID3D12CommandQueue,
        lists: &[ID3D12CommandList],
        submit: &mut dyn FnMut(),
    ) {
        if OWN_COPY.try_with(Cell::get).unwrap_or(false) {
            submit();
            return;
        }
        // One gate establishes the same ordering for metadata and the actual
        // driver calls, including concurrent submissions from different threads.
        let Ok(_gate) = SUBMIT.lock() else {
            snapshot::fault_all();
            crate::native_draw_assets::fault_all();
            submit();
            return;
        };
        let keys = lists
            .iter()
            .map(|list| list.as_raw() as usize)
            .collect::<Vec<_>>();
        if lists.len() > 4096 {
            observe(Tracker::invalidate);
        } else {
            observe(|tracker| tracker.apply(queue.as_raw() as usize, &keys));
        }
        crate::native_gpu_constants::before_submit(queue.as_raw() as usize);
        let assets = crate::native_draw_assets::before_submit(queue, lists);
        submit();
        crate::native_draw_assets::after_submit(queue, assets);
        snapshot::submitted(queue, &keys);
        snapshot::retire_released();
    }
}

type Reset = unsafe extern "system" fn(*mut c_void, *mut c_void, *mut c_void) -> HRESULT;
type Close = unsafe extern "system" fn(*mut c_void) -> HRESULT;
type CreateList = unsafe extern "system" fn(
    *mut c_void,
    u32,
    D3D12_COMMAND_LIST_TYPE,
    *mut c_void,
    *mut c_void,
    *const GUID,
    *mut *mut c_void,
) -> HRESULT;
type CreateView = unsafe extern "system" fn(
    *mut c_void,
    *mut c_void,
    *const D3D12_DEPTH_STENCIL_VIEW_DESC,
    D3D12_CPU_DESCRIPTOR_HANDLE,
);
type Barrier = unsafe extern "system" fn(*mut c_void, u32, *const D3D12_RESOURCE_BARRIER);
type Bind = unsafe extern "system" fn(
    *mut c_void,
    u32,
    *const D3D12_CPU_DESCRIPTOR_HANDLE,
    BOOL,
    *const D3D12_CPU_DESCRIPTOR_HANDLE,
);
type Viewport = unsafe extern "system" fn(*mut c_void, u32, *const D3D12_VIEWPORT);
type Clear = unsafe extern "system" fn(
    *mut c_void,
    D3D12_CPU_DESCRIPTOR_HANDLE,
    D3D12_CLEAR_FLAGS,
    f32,
    u8,
    u32,
    *const RECT,
);
type Draw = unsafe extern "system" fn(*mut c_void, u32, u32, u32, u32);
type Indexed = unsafe extern "system" fn(*mut c_void, u32, u32, u32, i32, u32);
type Enhanced = unsafe extern "system" fn(*mut c_void, u32, *const D3D12_BARRIER_GROUP);
type CreatePso = unsafe extern "system" fn(
    *mut c_void,
    *const D3D12_GRAPHICS_PIPELINE_STATE_DESC,
    *const GUID,
    *mut *mut c_void,
) -> HRESULT;
type CreateStreamPso = unsafe extern "system" fn(
    *mut c_void,
    *const D3D12_PIPELINE_STATE_STREAM_DESC,
    *const GUID,
    *mut *mut c_void,
) -> HRESULT;
type SetPso = unsafe extern "system" fn(*mut c_void, *mut c_void);
type RootCbv = unsafe extern "system" fn(*mut c_void, u32, u64);
type RootSignature = unsafe extern "system" fn(*mut c_void, *mut c_void);
type CreateRootSignature = unsafe extern "system" fn(
    *mut c_void,
    u32,
    *const c_void,
    usize,
    *const GUID,
    *mut *mut c_void,
) -> HRESULT;
type CreateColorView = unsafe extern "system" fn(
    *mut c_void,
    *mut c_void,
    *const D3D12_RENDER_TARGET_VIEW_DESC,
    D3D12_CPU_DESCRIPTOR_HANDLE,
);
type ClearColor = unsafe extern "system" fn(
    *mut c_void,
    D3D12_CPU_DESCRIPTOR_HANDLE,
    *const f32,
    u32,
    *const RECT,
);
type Release = unsafe extern "system" fn(*mut c_void) -> u32;
type CopyViews = unsafe extern "system" fn(
    *mut c_void,
    u32,
    *const D3D12_CPU_DESCRIPTOR_HANDLE,
    *const u32,
    u32,
    *const D3D12_CPU_DESCRIPTOR_HANDLE,
    *const u32,
    D3D12_DESCRIPTOR_HEAP_TYPE,
);
type CopyViewsSimple = unsafe extern "system" fn(
    *mut c_void,
    u32,
    D3D12_CPU_DESCRIPTOR_HANDLE,
    D3D12_CPU_DESCRIPTOR_HANDLE,
    D3D12_DESCRIPTOR_HEAP_TYPE,
);
type BeginPass = unsafe extern "system" fn(
    *mut c_void,
    u32,
    *const D3D12_RENDER_PASS_RENDER_TARGET_DESC,
    *const D3D12_RENDER_PASS_DEPTH_STENCIL_DESC,
    D3D12_RENDER_PASS_FLAGS,
);
type EndPass = unsafe extern "system" fn(*mut c_void);
type Bundle = unsafe extern "system" fn(*mut c_void, *mut c_void);
type Indirect =
    unsafe extern "system" fn(*mut c_void, *mut c_void, u32, *mut c_void, u64, *mut c_void, u64);
type CreateCommittedResource = unsafe extern "system" fn(
    *mut c_void,
    *const D3D12_HEAP_PROPERTIES,
    D3D12_HEAP_FLAGS,
    *const D3D12_RESOURCE_DESC,
    D3D12_RESOURCE_STATES,
    *const D3D12_CLEAR_VALUE,
    *const GUID,
    *mut *mut c_void,
) -> HRESULT;
type CreateDescriptorHeap = unsafe extern "system" fn(
    *mut c_void,
    *const D3D12_DESCRIPTOR_HEAP_DESC,
    *const GUID,
    *mut *mut c_void,
) -> HRESULT;
type CreateCbv = unsafe extern "system" fn(
    *mut c_void,
    *const D3D12_CONSTANT_BUFFER_VIEW_DESC,
    D3D12_CPU_DESCRIPTOR_HANDLE,
);
type CreateSrv = unsafe extern "system" fn(
    *mut c_void,
    *mut c_void,
    *const D3D12_SHADER_RESOURCE_VIEW_DESC,
    D3D12_CPU_DESCRIPTOR_HANDLE,
);
type CreateUav = unsafe extern "system" fn(
    *mut c_void,
    *mut c_void,
    *mut c_void,
    *const D3D12_UNORDERED_ACCESS_VIEW_DESC,
    D3D12_CPU_DESCRIPTOR_HANDLE,
);
type CreateSampler =
    unsafe extern "system" fn(*mut c_void, *const D3D12_SAMPLER_DESC, D3D12_CPU_DESCRIPTOR_HANDLE);
type CreateSampler2 =
    unsafe extern "system" fn(*mut c_void, *const D3D12_SAMPLER_DESC2, D3D12_CPU_DESCRIPTOR_HANDLE);
type CreateFeedback =
    unsafe extern "system" fn(*mut c_void, *mut c_void, *mut c_void, D3D12_CPU_DESCRIPTOR_HANDLE);
struct Originals {
    reset: Reset,
    close: Close,
    create_list: CreateList,
    create_view: CreateView,
    barrier: Barrier,
    bind: Bind,
    viewport: Viewport,
    clear: Clear,
    draw: Draw,
    indexed: Indexed,
    enhanced: Option<Enhanced>,
    create_pso: CreatePso,
    set_pso: SetPso,
    create_stream_pso: Option<CreateStreamPso>,
    copy_views: CopyViews,
    copy_views_simple: CopyViewsSimple,
    begin_pass: Option<BeginPass>,
    end_pass: Option<EndPass>,
    bundle: Bundle,
    indirect: Indirect,
    root_cbv: RootCbv,
    root_signature: RootSignature,
    create_root_signature: CreateRootSignature,
    create_color_view: CreateColorView,
    clear_color: ClearColor,
    create_committed_resource: CreateCommittedResource,
    create_descriptor_heap: CreateDescriptorHeap,
    create_cbv: CreateCbv,
    create_srv: CreateSrv,
    create_uav: CreateUav,
    create_sampler: CreateSampler,
    create_sampler2: Option<CreateSampler2>,
    create_feedback: Option<CreateFeedback>,
    release: Release,
}
static ORIGINALS: OnceLock<Originals> = OnceLock::new();

fn descriptor_before(
    run: impl FnOnce() -> Option<crate::native_descriptor_resources::Write>,
) -> Option<crate::native_descriptor_resources::Write> {
    if OWN_COPY.try_with(Cell::get).unwrap_or(true) {
        return None;
    }
    if !crate::native_descriptor_resources::observing() {
        let _ = std::panic::catch_unwind(|| {
            let _private = private_commands();
            crate::native_descriptor_resources::collect();
        });
        return None;
    }
    let write = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _private = private_commands();
        run()
    }))
    .ok()
    .flatten();
    write.or_else(|| {
        std::panic::catch_unwind(|| {
            let _private = private_commands();
            crate::native_descriptor_resources::uncertain_write()
        })
        .ok()
        .flatten()
    })
}
fn descriptor_after(write: Option<crate::native_descriptor_resources::Write>) {
    if let Some(write) = write {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _private = private_commands();
            write.finish();
        }));
    }
}
unsafe extern "system" fn create_descriptor_heap(
    this: *mut c_void,
    desc: *const D3D12_DESCRIPTOR_HEAP_DESC,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    let external = !OWN_COPY.try_with(Cell::get).unwrap_or(true);
    let result = unsafe { (ORIGINALS.get().unwrap().create_descriptor_heap)(this, desc, iid, out) };
    if external && result.is_ok() && !out.is_null() {
        let _ = std::panic::catch_unwind(|| {
            let _private = private_commands();
            unsafe { crate::native_descriptor_resources::created_heap(this, *out) };
        });
    }
    result
}
unsafe extern "system" fn create_cbv(
    this: *mut c_void,
    desc: *const D3D12_CONSTANT_BUFFER_VIEW_DESC,
    handle: D3D12_CPU_DESCRIPTOR_HANDLE,
) {
    let write = descriptor_before(|| unsafe {
        crate::native_descriptor_resources::cbv(this, desc, handle.ptr)
    });
    unsafe { (ORIGINALS.get().unwrap().create_cbv)(this, desc, handle) };
    descriptor_after(write);
}
unsafe extern "system" fn create_srv(
    this: *mut c_void,
    resource: *mut c_void,
    desc: *const D3D12_SHADER_RESOURCE_VIEW_DESC,
    handle: D3D12_CPU_DESCRIPTOR_HANDLE,
) {
    let write = descriptor_before(|| unsafe {
        crate::native_descriptor_resources::srv(this, resource, desc, handle.ptr)
    });
    unsafe { (ORIGINALS.get().unwrap().create_srv)(this, resource, desc, handle) };
    descriptor_after(write);
}
unsafe extern "system" fn create_uav(
    this: *mut c_void,
    resource: *mut c_void,
    counter: *mut c_void,
    desc: *const D3D12_UNORDERED_ACCESS_VIEW_DESC,
    handle: D3D12_CPU_DESCRIPTOR_HANDLE,
) {
    let write = descriptor_before(|| unsafe {
        crate::native_descriptor_resources::uav(this, resource, counter, desc, handle.ptr)
    });
    unsafe { (ORIGINALS.get().unwrap().create_uav)(this, resource, counter, desc, handle) };
    descriptor_after(write);
}
unsafe extern "system" fn create_sampler(
    this: *mut c_void,
    desc: *const D3D12_SAMPLER_DESC,
    handle: D3D12_CPU_DESCRIPTOR_HANDLE,
) {
    let write = descriptor_before(|| unsafe {
        crate::native_descriptor_resources::sampler(this, !desc.is_null(), handle.ptr)
    });
    unsafe { (ORIGINALS.get().unwrap().create_sampler)(this, desc, handle) };
    descriptor_after(write);
}
unsafe extern "system" fn create_sampler2(
    this: *mut c_void,
    desc: *const D3D12_SAMPLER_DESC2,
    handle: D3D12_CPU_DESCRIPTOR_HANDLE,
) {
    let write = descriptor_before(|| unsafe {
        crate::native_descriptor_resources::sampler(this, !desc.is_null(), handle.ptr)
    });
    unsafe {
        (ORIGINALS
            .get()
            .unwrap()
            .create_sampler2
            .expect("published sampler2 observer"))(this, desc, handle)
    };
    descriptor_after(write);
}
unsafe extern "system" fn create_feedback(
    this: *mut c_void,
    target: *mut c_void,
    feedback: *mut c_void,
    handle: D3D12_CPU_DESCRIPTOR_HANDLE,
) {
    let write = descriptor_before(|| unsafe {
        crate::native_descriptor_resources::feedback(this, target, feedback, handle.ptr)
    });
    unsafe {
        (ORIGINALS
            .get()
            .unwrap()
            .create_feedback
            .expect("published feedback observer"))(this, target, feedback, handle)
    };
    descriptor_after(write);
}

unsafe extern "system" fn create_committed_resource(
    this: *mut c_void,
    properties: *const D3D12_HEAP_PROPERTIES,
    flags: D3D12_HEAP_FLAGS,
    desc: *const D3D12_RESOURCE_DESC,
    initial: D3D12_RESOURCE_STATES,
    clear: *const D3D12_CLEAR_VALUE,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    let external = !OWN_COPY.try_with(Cell::get).unwrap_or(true);
    let result = unsafe {
        (ORIGINALS.get().unwrap().create_committed_resource)(
            this, properties, flags, desc, initial, clear, iid, out,
        )
    };
    if external && result.is_ok() && !out.is_null() {
        // Forward exactly once, then borrow only the successful API result.
        // Do not hold the tracker/submission lock while acquiring/dropping owners.
        let _ = std::panic::catch_unwind(|| {
            let _private = private_commands();
            unsafe { crate::native_committed_resources::created(this, *out) };
        });
    }
    result
}

unsafe extern "system" fn release(this: *mut c_void) -> u32 {
    let candidates = snapshot::release_candidates(this as usize);
    let asset_candidates = crate::native_draw_assets::release_candidates(this as usize);
    let count = unsafe { (ORIGINALS.get().unwrap().release)(this) };
    if count == 0 {
        snapshot::released(candidates);
        crate::native_draw_assets::released(asset_candidates);
    }
    count
}

unsafe extern "system" fn root_cbv(this: *mut c_void, index: u32, address: u64) {
    observe(|t| {
        let list = t.list(this as usize);
        list.graphics
            .address(index, D3D12_ROOT_PARAMETER_TYPE_CBV, address);
        match index {
            2 => list.cbv = Some(address),
            3 => list.cbf = Some(address),
            _ => {}
        }
    });
    unsafe { (ORIGINALS.get().unwrap().root_cbv)(this, index, address) };
}
unsafe extern "system" fn root_signature(this: *mut c_void, signature: *mut c_void) {
    observe(|t| {
        let list = t.list(this as usize);
        unsafe {
            list.graphics.signature(false, signature);
        }
        if list.root_signature != Some(signature as usize) {
            list.cbv = None;
            list.cbf = None;
        }
        list.root_signature = Some(signature as usize);
        list.root_binding = unsafe { crate::native_shader_layout::root(signature) };
    });
    unsafe { (ORIGINALS.get().unwrap().root_signature)(this, signature) };
}

unsafe extern "system" fn reset(
    this: *mut c_void,
    allocator: *mut c_void,
    pipeline: *mut c_void,
) -> HRESULT {
    // Reset may follow ExecuteCommandLists on another CPU thread before the
    // GPU finishes. Keep ownership until the observer queues its fence signal.
    let _gate = native_edit_gate();
    snapshot::retire_released();
    let result = unsafe { (ORIGINALS.get().unwrap().reset)(this, allocator, pipeline) };
    if result.is_ok() {
        observe(|t| {
            unsafe {
                crate::native_draw_assets::reset(this);
            }
            snapshot::reset(this as usize);
            t.reset_lists += 1;
            *t.list(this as usize) = List {
                graphics: unsafe { crate::native_graphics_state::State::reset(this, pipeline) },
                depth_write: unsafe { crate::depth_pipeline::mode(pipeline) },
                shaders: unsafe { crate::native_shader_layout::pipeline(pipeline) },
                ..Default::default()
            }
        })
    }
    result
}
unsafe extern "system" fn close(this: *mut c_void) -> HRESULT {
    observe(|t| {
        let resources = t
            .lists
            .get(&(this as usize))
            .map(|list| {
                list.events
                    .iter()
                    .filter_map(|op| {
                        if let Op::Draw(r, Some(_), true, Some(true), _) = op {
                            Some((r.id, r.clone()))
                        } else {
                            None
                        }
                    })
                    .collect::<HashMap<_, _>>()
            })
            .unwrap_or_default();
        for r in resources.into_values() {
            t.preserve(this as usize, &r);
        }
    });
    let result = unsafe { (ORIGINALS.get().unwrap().close)(this) };
    if result.is_ok() {
        observe(|t| {
            t.closed_lists += 1;
            t.list(this as usize).closed = true;
            t.list(this as usize).graphics = Default::default();
            t.list(this as usize).surfaces = Default::default();
        })
    }
    result
}
unsafe extern "system" fn create_list(
    this: *mut c_void,
    node: u32,
    kind: D3D12_COMMAND_LIST_TYPE,
    allocator: *mut c_void,
    pipeline: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    let _gate = native_edit_gate();
    snapshot::retire_released();
    let result = unsafe {
        (ORIGINALS.get().unwrap().create_list)(this, node, kind, allocator, pipeline, iid, out)
    };
    if result.is_ok() && !out.is_null() {
        observe(|t| {
            t.created_lists += 1;
            let raw = unsafe { *out };
            if !raw.is_null() {
                snapshot::reset(raw as usize);
                *t.list(raw as usize) = List {
                    graphics: unsafe { crate::native_graphics_state::State::reset(raw, pipeline) },
                    depth_write: unsafe { crate::depth_pipeline::mode(pipeline) },
                    shaders: unsafe { crate::native_shader_layout::pipeline(pipeline) },
                    ..Default::default()
                }
            }
        })
    }
    result
}
unsafe extern "system" fn create_view(
    this: *mut c_void,
    resource: *mut c_void,
    desc: *const D3D12_DEPTH_STENCIL_VIEW_DESC,
    handle: D3D12_CPU_DESCRIPTOR_HANDLE,
) {
    let mut surface_write = None;
    observe(|_| {
        surface_write =
            unsafe { crate::native_surface_views::depth(this, resource, desc, handle.ptr) };
    });
    unsafe { (ORIGINALS.get().unwrap().create_view)(this, resource, desc, handle) };
    observe(|_| {
        if let Some(write) = surface_write {
            write.finish();
        }
    });
    observe(|t| {
        t.views.remove(&handle.ptr);
        t.created_views += 1;
        if !desc.is_null() {
            let d = unsafe { &*desc };
            if d.ViewDimension != D3D12_DSV_DIMENSION_TEXTURE2D
                || unsafe { d.Anonymous.Texture2D.MipSlice } != 0
            {
                if t.rejected_views.len() < 8 {
                    t.rejected_views.push(serde_json::json!({"reason":"view-dimension-or-mip","dimension":d.ViewDimension.0,"flags":d.Flags.0}));
                }
                return;
            }
        }
        if let Some(resource) = unsafe { ID3D12Resource::from_raw_borrowed(&resource) } {
            let Some(object) = t.resource(resource) else {
                if t.rejected_views.len() < 8 {
                    let d = unsafe { resource.GetDesc() };
                    t.rejected_views.push(serde_json::json!({"reason":"resource-layout","width":d.Width,"height":d.Height,"samples":d.SampleDesc.Count,"layers":d.DepthOrArraySize,"mips":d.MipLevels,"format":d.Format.0,"flags":d.Flags.0}));
                }
                return;
            };
            if t.views.len() >= 256 {
                t.views.clear();
                t.invalidate();
            }
            t.views.insert(handle.ptr, object);
        }
    });
}
unsafe extern "system" fn create_pso(
    this: *mut c_void,
    desc: *const D3D12_GRAPHICS_PIPELINE_STATE_DESC,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    let result = unsafe { (ORIGINALS.get().unwrap().create_pso)(this, desc, iid, out) };
    if result.is_ok() && !desc.is_null() && !out.is_null() {
        observe(|_| unsafe {
            let depth = &(*desc).DepthStencilState;
            crate::depth_pipeline::tag(
                *out,
                Some(crate::depth_pipeline::writes(
                    depth.DepthEnable,
                    depth.DepthWriteMask,
                )),
            );
            crate::native_shader_layout::tag_pipeline(*out, (*desc).VS, (*desc).PS);
        });
    }
    result
}
unsafe extern "system" fn create_stream_pso(
    this: *mut c_void,
    desc: *const D3D12_PIPELINE_STATE_STREAM_DESC,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    let result =
        unsafe { (ORIGINALS.get().unwrap().create_stream_pso.unwrap())(this, desc, iid, out) };
    if result.is_ok() && !desc.is_null() && !out.is_null() {
        observe(|_| unsafe {
            let desc = &*desc;
            if desc.SizeInBytes == 0
                || desc.SizeInBytes > 65536
                || desc.pPipelineStateSubobjectStream.is_null()
            {
                return;
            }
            let bytes = std::slice::from_raw_parts(
                desc.pPipelineStateSubobjectStream as *const u8,
                desc.SizeInBytes,
            );
            crate::depth_pipeline::tag(*out, crate::depth_pipeline::stream(bytes));
            crate::native_shader_layout::tag_stream(*out, bytes);
        });
    }
    result
}
unsafe extern "system" fn set_pso(this: *mut c_void, pipeline: *mut c_void) {
    observe(|t| {
        let list = t.list(this as usize);
        unsafe {
            list.graphics.pipeline(pipeline);
        }
        list.depth_write = unsafe { crate::depth_pipeline::mode(pipeline) };
        list.shaders = unsafe { crate::native_shader_layout::pipeline(pipeline) };
    });
    unsafe { (ORIGINALS.get().unwrap().set_pso)(this, pipeline) };
}
unsafe extern "system" fn create_root_signature(
    this: *mut c_void,
    node: u32,
    blob: *const c_void,
    len: usize,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    let result = unsafe {
        (ORIGINALS.get().unwrap().create_root_signature)(this, node, blob, len, iid, out)
    };
    if result.is_ok() && !out.is_null() {
        observe(|_| unsafe { crate::native_graphics_state::tag_root(*out, blob, len) });
        observe(|_| unsafe { crate::native_shader_layout::tag_root(*out, blob, len) });
    }
    result
}
unsafe extern "system" fn create_color_view(
    this: *mut c_void,
    resource: *mut c_void,
    desc: *const D3D12_RENDER_TARGET_VIEW_DESC,
    handle: D3D12_CPU_DESCRIPTOR_HANDLE,
) {
    let mut surface_write = None;
    observe(|_| {
        surface_write =
            unsafe { crate::native_surface_views::color(this, resource, desc, handle.ptr) };
    });
    unsafe { (ORIGINALS.get().unwrap().create_color_view)(this, resource, desc, handle) };
    observe(|_| {
        if let Some(write) = surface_write {
            write.finish();
        }
    });
    observe(|_| unsafe { crate::native_scene_target::view(this, resource, handle.ptr) });
}
unsafe extern "system" fn clear_color(
    this: *mut c_void,
    handle: D3D12_CPU_DESCRIPTOR_HANDLE,
    color: *const f32,
    count: u32,
    rects: *const RECT,
) {
    observe(|t| {
        t.list(this as usize)
            .surfaces
            .clear(crate::native_surface_views::Kind::Color, handle.ptr);
        let id = crate::native_scene_target::target(handle.ptr);
        if id != 0 {
            t.push(this as usize, Op::ColorClear(id));
        }
    });
    unsafe { (ORIGINALS.get().unwrap().clear_color)(this, handle, color, count, rects) };
}
unsafe extern "system" fn barrier(
    this: *mut c_void,
    count: u32,
    barriers: *const D3D12_RESOURCE_BARRIER,
) {
    // Private readbacks restore the copied plane before submission. Their
    // resources and temporary states must not enter the native-world cache.
    if OWN_COPY.try_with(Cell::get).unwrap_or(false) {
        unsafe { (ORIGINALS.get().unwrap().barrier)(this, count, barriers) };
        return;
    }
    observe(|t| {
        if count > 4096 || count > 0 && barriers.is_null() {
            t.list(this as usize).surfaces.invalidate();
            crate::native_framebuffer::unknown_resource_operation(this as usize);
            t.push(this as usize, Op::Unknown);
            return;
        }
        for i in 0..count as usize {
            let b = unsafe { &*barriers.add(i) };
            if b.Type == D3D12_RESOURCE_BARRIER_TYPE_TRANSITION {
                let tr = unsafe { &b.Anonymous.Transition };
                t.list(this as usize)
                    .surfaces
                    .transition(tr.pResource.as_ref());
                if let Some(resource) = tr.pResource.as_ref() {
                    crate::native_framebuffer::transition(
                        this as usize,
                        resource,
                        tr.StateBefore,
                        tr.StateAfter,
                        tr.Subresource,
                        b.Flags,
                    );
                }
                if tr.Subresource != 0 && tr.Subresource != D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES
                {
                    continue;
                }
                if let Some(resource) = tr.pResource.as_ref()
                    && let Some(object) = t.resource(resource)
                {
                    t.push(
                        this as usize,
                        Op::Transition(object, tr.StateBefore, tr.StateAfter, b.Flags),
                    );
                }
            } else if b.Type == D3D12_RESOURCE_BARRIER_TYPE_ALIASING {
                t.list(this as usize).surfaces.invalidate();
                crate::native_framebuffer::unknown_resource_operation(this as usize);
                t.push(this as usize, Op::Unknown)
            } else if b.Type == D3D12_RESOURCE_BARRIER_TYPE_UAV {
                let uav = unsafe { &b.Anonymous.UAV };
                t.list(this as usize)
                    .surfaces
                    .transition(uav.pResource.as_ref());
            }
        }
    });
    unsafe { (ORIGINALS.get().unwrap().barrier)(this, count, barriers) };
}
unsafe extern "system" fn bind(
    this: *mut c_void,
    count: u32,
    rtv: *const D3D12_CPU_DESCRIPTOR_HANDLE,
    single: BOOL,
    dsv: *const D3D12_CPU_DESCRIPTOR_HANDLE,
) {
    observe(|t| {
        let epoch = t.list(this as usize).surfaces.begin_binding();
        crate::native_framebuffer::binding(this as usize, count, rtv, single.as_bool(), dsv, epoch);
        let object = if dsv.is_null() {
            None
        } else {
            t.views.get(&unsafe { *dsv }.ptr).cloned()
        };
        let unknown = !dsv.is_null() && object.is_none();
        if unknown {
            t.unknown_bindings += 1;
        }
        let list = t.list(this as usize);
        list.bound = object;
        list.color_targets =
            unsafe { crate::native_scene_target::bindings(count, rtv, single.as_bool()) };
        list.read_only_depth = false;
        list.render_pass = None;
        if unknown {
            list.unknown = true;
        }
    });
    unsafe { (ORIGINALS.get().unwrap().bind)(this, count, rtv, single, dsv) };
}
unsafe extern "system" fn viewport(
    this: *mut c_void,
    count: u32,
    viewports: *const D3D12_VIEWPORT,
) {
    observe(|t| {
        unsafe {
            t.list(this as usize).graphics.viewport(count, viewports);
        }
        t.list(this as usize).viewport = if count == 1 && !viewports.is_null() {
            Some(unsafe { *viewports })
        } else {
            None
        }
    });
    unsafe { (ORIGINALS.get().unwrap().viewport)(this, count, viewports) };
}
unsafe extern "system" fn clear(
    this: *mut c_void,
    handle: D3D12_CPU_DESCRIPTOR_HANDLE,
    flags: D3D12_CLEAR_FLAGS,
    depth: f32,
    stencil: u8,
    count: u32,
    rects: *const RECT,
) {
    observe(|t| {
        t.list(this as usize)
            .surfaces
            .clear(crate::native_surface_views::Kind::Depth, handle.ptr);
        if flags & D3D12_CLEAR_FLAG_DEPTH == D3D12_CLEAR_FLAGS(0) {
            return;
        }
        let Some(object) = t.views.get(&handle.ptr).cloned() else {
            t.list(this as usize).unknown = true;
            return;
        };
        let reverse = if count == 0 {
            if depth == 0. {
                Some(true)
            } else if depth == 1. {
                Some(false)
            } else {
                None
            }
        } else {
            None
        };
        let full = full_viewport(t.list(this as usize).viewport.as_ref(), &object);
        t.preserve(this as usize, &object);
        t.push(
            this as usize,
            Op::Clear(
                object,
                crate::world_hooks::current_pass(),
                reverse,
                full,
                count == 0,
            ),
        );
    });
    unsafe { (ORIGINALS.get().unwrap().clear)(this, handle, flags, depth, stencil, count, rects) };
}
unsafe extern "system" fn draw(this: *mut c_void, a: u32, b: u32, c: u32, d: u32) {
    if a > 0 && b > 0 {
        observe(|t| t.draw(this as usize))
    }
    unsafe { (ORIGINALS.get().unwrap().draw)(this, a, b, c, d) };
    if a > 0 && b > 0 {
        observe(|t| t.after_draw(this as usize));
    }
}
unsafe extern "system" fn indexed(this: *mut c_void, a: u32, b: u32, c: u32, d: i32, e: u32) {
    if a > 0 && b > 0 {
        observe(|t| t.draw(this as usize))
    }
    unsafe { (ORIGINALS.get().unwrap().indexed)(this, a, b, c, d, e) };
    if a > 0 && b > 0 {
        observe(|t| t.after_draw(this as usize));
    }
}
unsafe extern "system" fn enhanced(
    this: *mut c_void,
    count: u32,
    groups: *const D3D12_BARRIER_GROUP,
) {
    // Enhanced layouts must not be mistaken for legacy resource states.
    observe(|t| {
        crate::native_framebuffer::unknown_resource_operation(this as usize);
        t.list(this as usize).surfaces.invalidate();
        t.enhanced_calls += 1;
        t.push(this as usize, Op::Unknown);
    });
    unsafe { (ORIGINALS.get().unwrap().enhanced.unwrap())(this, count, groups) };
}

/// Expand public descriptor ranges before copying any metadata. D3D12 uses one
/// descriptor per range when the optional sizes pointer is null. Keep bounded
/// even when observing an invalid application's input; the driver still gets
/// exactly its original arguments.
unsafe fn view_ranges(
    count: u32,
    starts: *const D3D12_CPU_DESCRIPTOR_HANDLE,
    sizes: *const u32,
    increment: usize,
) -> Option<Vec<usize>> {
    if count > 256 || (count > 0 && starts.is_null()) || increment == 0 {
        return None;
    }
    let mut handles = Vec::new();
    for i in 0..count as usize {
        let size = if sizes.is_null() {
            1
        } else {
            unsafe { *sizes.add(i) }
        } as usize;
        if size > 256 || handles.len().checked_add(size)? > 256 {
            return None;
        }
        let start = unsafe { (*starts.add(i)).ptr };
        for n in 0..size {
            handles.push(start.checked_add(n.checked_mul(increment)?)?);
        }
    }
    Some(handles)
}
fn copy_view_metadata(t: &mut Tracker, dst: Option<Vec<usize>>, src: Option<Vec<usize>>) {
    let Some((dst, src)) = dst.zip(src).filter(|(d, s)| d.len() == s.len()) else {
        t.views.clear();
        t.invalidate();
        return;
    };
    let sources: Vec<_> = src.iter().map(|h| t.views.get(h).cloned()).collect();
    for (handle, object) in dst.into_iter().zip(sources) {
        t.views.remove(&handle);
        if let Some(object) = object {
            t.views.insert(handle, object);
        }
    }
    if t.views.len() > 256 {
        t.views.clear();
        t.invalidate();
    }
}
unsafe extern "system" fn copy_views(
    this: *mut c_void,
    dst_count: u32,
    dst: *const D3D12_CPU_DESCRIPTOR_HANDLE,
    dst_sizes: *const u32,
    src_count: u32,
    src: *const D3D12_CPU_DESCRIPTOR_HANDLE,
    src_sizes: *const u32,
    kind: D3D12_DESCRIPTOR_HEAP_TYPE,
) {
    let descriptor_write = if matches!(
        kind,
        D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV | D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER
    ) {
        descriptor_before(|| unsafe {
            let device = ID3D12Device::from_raw_borrowed(&this)?;
            let increment = device.GetDescriptorHandleIncrementSize(kind) as usize;
            crate::native_descriptor_resources::copy(
                device,
                kind,
                view_ranges(dst_count, dst, dst_sizes, increment),
                view_ranges(src_count, src, src_sizes, increment),
            )
        })
    } else {
        None
    };
    let mut surface_write = None;
    if kind == D3D12_DESCRIPTOR_HEAP_TYPE_RTV || kind == D3D12_DESCRIPTOR_HEAP_TYPE_DSV {
        observe(|_| unsafe {
            if let Some(device) = ID3D12Device::from_raw_borrowed(&this) {
                let increment = device.GetDescriptorHandleIncrementSize(kind) as usize;
                let surface_kind = if kind == D3D12_DESCRIPTOR_HEAP_TYPE_RTV {
                    crate::native_surface_views::Kind::Color
                } else {
                    crate::native_surface_views::Kind::Depth
                };
                surface_write = crate::native_surface_views::copy(
                    device,
                    surface_kind,
                    view_ranges(dst_count, dst, dst_sizes, increment),
                    view_ranges(src_count, src, src_sizes, increment),
                );
            } else {
                crate::native_surface_views::invalidate();
            }
        });
    }
    if kind == D3D12_DESCRIPTOR_HEAP_TYPE_RTV {
        observe(|_| unsafe {
            if let Some(device) = ID3D12Device::from_raw_borrowed(&this) {
                let increment = device.GetDescriptorHandleIncrementSize(kind) as usize;
                crate::native_scene_target::copy_views(
                    view_ranges(dst_count, dst, dst_sizes, increment),
                    view_ranges(src_count, src, src_sizes, increment),
                );
            } else {
                crate::native_scene_target::invalidate_views();
            }
        });
    }
    if kind == D3D12_DESCRIPTOR_HEAP_TYPE_DSV {
        observe(|t| unsafe {
            if let Some(device) = ID3D12Device::from_raw_borrowed(&this) {
                let increment = device.GetDescriptorHandleIncrementSize(kind) as usize;
                copy_view_metadata(
                    t,
                    view_ranges(dst_count, dst, dst_sizes, increment),
                    view_ranges(src_count, src, src_sizes, increment),
                );
            }
        });
    }
    unsafe {
        (ORIGINALS.get().unwrap().copy_views)(
            this, dst_count, dst, dst_sizes, src_count, src, src_sizes, kind,
        )
    };
    observe(|_| {
        if let Some(write) = surface_write {
            write.finish();
        }
    });
    descriptor_after(descriptor_write);
}
unsafe extern "system" fn copy_views_simple(
    this: *mut c_void,
    count: u32,
    dst: D3D12_CPU_DESCRIPTOR_HANDLE,
    src: D3D12_CPU_DESCRIPTOR_HANDLE,
    kind: D3D12_DESCRIPTOR_HEAP_TYPE,
) {
    let descriptor_write = if matches!(
        kind,
        D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV | D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER
    ) {
        descriptor_before(|| unsafe {
            let device = ID3D12Device::from_raw_borrowed(&this)?;
            let increment = device.GetDescriptorHandleIncrementSize(kind) as usize;
            crate::native_descriptor_resources::copy(
                device,
                kind,
                view_ranges(1, &dst, &count, increment),
                view_ranges(1, &src, &count, increment),
            )
        })
    } else {
        None
    };
    let mut surface_write = None;
    if kind == D3D12_DESCRIPTOR_HEAP_TYPE_RTV || kind == D3D12_DESCRIPTOR_HEAP_TYPE_DSV {
        observe(|_| unsafe {
            if let Some(device) = ID3D12Device::from_raw_borrowed(&this) {
                let increment = device.GetDescriptorHandleIncrementSize(kind) as usize;
                let surface_kind = if kind == D3D12_DESCRIPTOR_HEAP_TYPE_RTV {
                    crate::native_surface_views::Kind::Color
                } else {
                    crate::native_surface_views::Kind::Depth
                };
                surface_write = crate::native_surface_views::copy(
                    device,
                    surface_kind,
                    view_ranges(1, &dst, &count, increment),
                    view_ranges(1, &src, &count, increment),
                );
            } else {
                crate::native_surface_views::invalidate();
            }
        });
    }
    if kind == D3D12_DESCRIPTOR_HEAP_TYPE_RTV {
        observe(|_| unsafe {
            if let Some(device) = ID3D12Device::from_raw_borrowed(&this) {
                let increment = device.GetDescriptorHandleIncrementSize(kind) as usize;
                crate::native_scene_target::copy_views(
                    view_ranges(1, &dst, &count, increment),
                    view_ranges(1, &src, &count, increment),
                );
            } else {
                crate::native_scene_target::invalidate_views();
            }
        });
    }
    if kind == D3D12_DESCRIPTOR_HEAP_TYPE_DSV {
        observe(|t| unsafe {
            if let Some(device) = ID3D12Device::from_raw_borrowed(&this) {
                let increment = device.GetDescriptorHandleIncrementSize(kind) as usize;
                copy_view_metadata(
                    t,
                    view_ranges(1, &dst, &count, increment),
                    view_ranges(1, &src, &count, increment),
                );
            }
        });
    }
    unsafe { (ORIGINALS.get().unwrap().copy_views_simple)(this, count, dst, src, kind) };
    observe(|_| {
        if let Some(write) = surface_write {
            write.finish();
        }
    });
    descriptor_after(descriptor_write);
}
unsafe extern "system" fn begin_pass(
    this: *mut c_void,
    count: u32,
    targets: *const D3D12_RENDER_PASS_RENDER_TARGET_DESC,
    depth: *const D3D12_RENDER_PASS_DEPTH_STENCIL_DESC,
    flags: D3D12_RENDER_PASS_FLAGS,
) {
    observe(|t| {
        let key = this as usize;
        t.list(key).surfaces.invalidate();
        if t.list(key).render_pass.is_some() {
            t.list(key).unknown = true;
        }
        t.list(key).bound = None;
        t.list(key).read_only_depth =
            flags & D3D12_RENDER_PASS_FLAG_BIND_READ_ONLY_DEPTH != D3D12_RENDER_PASS_FLAGS(0);
        t.list(key).render_pass = None;
        t.list(key).color_targets = [0; 8];
        if count > 0 && count <= 8 && !targets.is_null() {
            let colors = unsafe { std::slice::from_raw_parts(targets, count as usize) };
            let handles: Vec<_> = colors.iter().map(|c| c.cpuDescriptor).collect();
            let bindings =
                unsafe { crate::native_scene_target::bindings(count, handles.as_ptr(), false) };
            t.list(key).color_targets = bindings;
            for (color, id) in colors.iter().zip(bindings) {
                if id != 0
                    && matches!(
                        color.BeginningAccess.Type,
                        D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_CLEAR
                            | D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_DISCARD
                    )
                {
                    t.push(key, Op::ColorClear(id));
                }
            }
        }
        if depth.is_null() {
            return;
        }
        let d = unsafe { &*depth };
        let Some(object) = t.views.get(&d.cpuDescriptor.ptr).cloned() else {
            t.list(key).unknown = true;
            return;
        };
        // Split passes and local-only preservation need an explicit continuation
        // trace. Refuse them instead of treating tile-local depth as readable.
        if flags & (D3D12_RENDER_PASS_FLAG_SUSPENDING_PASS | D3D12_RENDER_PASS_FLAG_RESUMING_PASS)
            != D3D12_RENDER_PASS_FLAGS(0)
        {
            t.list(key).unknown = true;
        }
        let begin = d.DepthBeginningAccess.Type;
        if begin == D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_CLEAR {
            let value = unsafe {
                d.DepthBeginningAccess
                    .Anonymous
                    .Clear
                    .ClearValue
                    .Anonymous
                    .DepthStencil
                    .Depth
            };
            let reverse = if value == 0. {
                Some(true)
            } else if value == 1. {
                Some(false)
            } else {
                None
            };
            t.push(
                key,
                Op::Clear(
                    object.clone(),
                    crate::world_hooks::current_pass(),
                    reverse,
                    false,
                    true,
                ),
            );
        } else if begin == D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_DISCARD {
            t.push(key, Op::Discard(object.clone()));
        } else if begin == D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_NO_ACCESS {
            return;
        } else if begin != D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_PRESERVE {
            t.list(key).unknown = true;
        }
        let preserve = d.DepthEndingAccess.Type == D3D12_RENDER_PASS_ENDING_ACCESS_TYPE_PRESERVE;
        t.list(key).render_pass = Some((object.clone(), preserve));
        t.list(key).bound = Some(object);
    });
    unsafe { (ORIGINALS.get().unwrap().begin_pass.unwrap())(this, count, targets, depth, flags) };
}
unsafe extern "system" fn end_pass(this: *mut c_void) {
    observe(|t| t.list(this as usize).surfaces.invalidate());
    observe(|t| {
        let key = this as usize;
        if let Some((object, false)) = t.list(key).render_pass.take() {
            t.push(key, Op::Discard(object));
        }
        t.list(key).bound = None;
        t.list(key).read_only_depth = false;
    });
    unsafe { (ORIGINALS.get().unwrap().end_pass.unwrap())(this) };
}
unsafe extern "system" fn bundle(this: *mut c_void, child: *mut c_void) {
    // Bundles can change inherited pipeline/bindings; an unexpanded bundle must
    // invalidate the whole trace, including later apparent direct draws.
    observe(|t| {
        t.bundle_calls += 1;
        t.list(this as usize).surfaces.invalidate();
        t.list(this as usize).graphics.invalidate();
        t.list(this as usize).unknown = true;
    });
    unsafe { (ORIGINALS.get().unwrap().bundle)(this, child) };
}
unsafe extern "system" fn indirect(
    this: *mut c_void,
    signature: *mut c_void,
    count: u32,
    args: *mut c_void,
    offset: u64,
    counts: *mut c_void,
    count_offset: u64,
) {
    observe(|t| {
        // The command signature can reset bindings after ExecuteIndirect; do
        // not assume a zero maximum count preserves an uncaptured signature.
        t.list(this as usize).graphics.invalidate();
        t.list(this as usize).surfaces.invalidate();
        if count > 0 {
            t.indirect_calls += 1;
            t.list(this as usize).unknown = true;
        }
    });
    unsafe {
        (ORIGINALS.get().unwrap().indirect)(
            this,
            signature,
            count,
            args,
            offset,
            counts,
            count_offset,
        )
    };
}

unsafe fn install(device: &ID3D12Device) -> Result<(), String> {
    crate::native_gpu_constants::initialize(device)?;
    crate::native_scene_target::initialize(device);
    // Use a command list from the actual swap-chain device, not a guessed WARP
    // or different adapter vtable. No game resource is changed during discovery.
    let allocator: ID3D12CommandAllocator =
        unsafe { device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT) }
            .map_err(|e| e.to_string())?;
    let list: ID3D12GraphicsCommandList =
        unsafe { device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &allocator, None) }
            .map_err(|e| e.to_string())?;
    unsafe { list.Close() }.map_err(|e| e.to_string())?;
    let d = device.vtable();
    let l = list.vtable();
    RESET_TARGET.store(l.Reset as usize, Ordering::Release);
    RELEASE_TARGET.store(
        unsafe { *(*(list.as_raw() as *const *const usize)).add(2) },
        Ordering::Release,
    );
    let e = list.cast::<ID3D12GraphicsCommandList7>().ok();
    let device2 = device.cast::<ID3D12Device2>().ok();
    let pass = list.cast::<ID3D12GraphicsCommandList4>().ok();
    let mut targets = vec![
        d.CreateCommandList as usize,
        d.CreateDepthStencilView as usize,
        l.Reset as usize,
        l.Close as usize,
        l.ResourceBarrier as usize,
        l.OMSetRenderTargets as usize,
        l.RSSetViewports as usize,
        l.ClearDepthStencilView as usize,
        l.DrawInstanced as usize,
        l.DrawIndexedInstanced as usize,
        d.CreateGraphicsPipelineState as usize,
        l.SetPipelineState as usize,
        d.CopyDescriptors as usize,
        d.CopyDescriptorsSimple as usize,
        l.ExecuteBundle as usize,
        l.ExecuteIndirect as usize,
        l.SetGraphicsRootConstantBufferView as usize,
        l.SetGraphicsRootSignature as usize,
        // Hook the actual graphics-list interface's IUnknown prefix. A QI for
        // canonical IUnknown can return a different tear-off implementation.
        unsafe { *(*(list.as_raw() as *const *const usize)).add(2) },
        d.CreateRootSignature as usize,
        d.CreateRenderTargetView as usize,
        l.ClearRenderTargetView as usize,
        d.CreateCommittedResource as usize,
        d.CreateDescriptorHeap as usize,
        d.CreateConstantBufferView as usize,
        d.CreateShaderResourceView as usize,
        d.CreateUnorderedAccessView as usize,
        d.CreateSampler as usize,
    ];
    let mut callbacks = vec![
        create_list as *const () as usize,
        create_view as *const () as usize,
        reset as *const () as usize,
        close as *const () as usize,
        barrier as *const () as usize,
        bind as *const () as usize,
        viewport as *const () as usize,
        clear as *const () as usize,
        draw as *const () as usize,
        indexed as *const () as usize,
        create_pso as *const () as usize,
        set_pso as *const () as usize,
        copy_views as *const () as usize,
        copy_views_simple as *const () as usize,
        bundle as *const () as usize,
        indirect as *const () as usize,
        root_cbv as *const () as usize,
        root_signature as *const () as usize,
        release as *const () as usize,
        create_root_signature as *const () as usize,
        create_color_view as *const () as usize,
        clear_color as *const () as usize,
        create_committed_resource as *const () as usize,
        create_descriptor_heap as *const () as usize,
        create_cbv as *const () as usize,
        create_srv as *const () as usize,
        create_uav as *const () as usize,
        create_sampler as *const () as usize,
    ];
    let device11 = match device.cast::<ID3D12Device11>() {
        Ok(value) => Some(value),
        Err(error) if error.code() == E_NOINTERFACE => None,
        Err(error) => return Err(format!("query descriptor sampler interface: {error}")),
    };
    let sampler2_index = if let Some(device) = device11.as_ref() {
        let target = device.vtable().CreateSampler2 as usize;
        if target == d.CreateSampler as usize {
            // Same ABI and only pointer presence is inspected by our sampler observer.
            None
        } else {
            if targets.contains(&target) {
                return Err("sampler2 observer aliases another public callback".into());
            }
            let index = targets.len();
            targets.push(target);
            callbacks.push(create_sampler2 as *const () as usize);
            Some(index)
        }
    } else {
        None
    };
    let device8 = match device.cast::<ID3D12Device8>() {
        Ok(value) => Some(value),
        Err(error) if error.code() == E_NOINTERFACE => None,
        Err(error) => return Err(format!("query descriptor feedback interface: {error}")),
    };
    let feedback_index = if let Some(device) = device8.as_ref() {
        let target = device.vtable().CreateSamplerFeedbackUnorderedAccessView as usize;
        if targets.contains(&target) {
            return Err("feedback observer aliases another public callback".into());
        }
        let index = targets.len();
        targets.push(target);
        callbacks.push(create_feedback as *const () as usize);
        Some(index)
    } else {
        None
    };
    crate::native_descriptor_resources::prepare(device)?;
    let stream_index = device2.as_ref().map(|d| {
        let index = targets.len();
        targets.push(d.vtable().CreatePipelineState as usize);
        callbacks.push(create_stream_pso as *const () as usize);
        index
    });
    let enhanced_index = e.as_ref().map(|_| targets.len());
    if let Some(e) = &e {
        targets.push(e.vtable().Barrier as usize);
        callbacks.push(enhanced as *const () as usize);
    }
    let pass_index = pass.as_ref().map(|p| {
        let index = targets.len();
        targets.extend([
            p.vtable().BeginRenderPass as usize,
            p.vtable().EndRenderPass as usize,
        ]);
        callbacks.extend([
            begin_pass as *const () as usize,
            end_pass as *const () as usize,
        ]);
        index
    });
    let mut originals = Vec::new();
    for (target, callback) in targets.iter().zip(callbacks) {
        let mut original = std::ptr::null_mut();
        let status = unsafe { MH_CreateHook(*target as _, callback as _, &mut original) };
        if status != MH_STATUS::MH_OK {
            return Err(format!("create depth observer: {status:?}"));
        }
        originals.push(original as usize);
    }
    // SAFETY: all signatures above are the corresponding public D3D12 vtable
    // signatures. Store every trampoline before enabling any callback.
    let tr = unsafe {
        Originals {
            create_list: std::mem::transmute::<usize, CreateList>(originals[0]),
            create_view: std::mem::transmute::<usize, CreateView>(originals[1]),
            reset: std::mem::transmute::<usize, Reset>(originals[2]),
            close: std::mem::transmute::<usize, Close>(originals[3]),
            barrier: std::mem::transmute::<usize, Barrier>(originals[4]),
            bind: std::mem::transmute::<usize, Bind>(originals[5]),
            viewport: std::mem::transmute::<usize, Viewport>(originals[6]),
            clear: std::mem::transmute::<usize, Clear>(originals[7]),
            draw: std::mem::transmute::<usize, Draw>(originals[8]),
            indexed: std::mem::transmute::<usize, Indexed>(originals[9]),
            create_pso: std::mem::transmute::<usize, CreatePso>(originals[10]),
            set_pso: std::mem::transmute::<usize, SetPso>(originals[11]),
            copy_views: std::mem::transmute::<usize, CopyViews>(originals[12]),
            copy_views_simple: std::mem::transmute::<usize, CopyViewsSimple>(originals[13]),
            bundle: std::mem::transmute::<usize, Bundle>(originals[14]),
            indirect: std::mem::transmute::<usize, Indirect>(originals[15]),
            root_cbv: std::mem::transmute::<usize, RootCbv>(originals[16]),
            root_signature: std::mem::transmute::<usize, RootSignature>(originals[17]),
            release: std::mem::transmute::<usize, Release>(originals[18]),
            create_root_signature: std::mem::transmute::<usize, CreateRootSignature>(originals[19]),
            create_color_view: std::mem::transmute::<usize, CreateColorView>(originals[20]),
            clear_color: std::mem::transmute::<usize, ClearColor>(originals[21]),
            create_committed_resource: std::mem::transmute::<usize, CreateCommittedResource>(
                originals[22],
            ),
            create_descriptor_heap: std::mem::transmute::<usize, CreateDescriptorHeap>(
                originals[23],
            ),
            create_cbv: std::mem::transmute::<usize, CreateCbv>(originals[24]),
            create_srv: std::mem::transmute::<usize, CreateSrv>(originals[25]),
            create_uav: std::mem::transmute::<usize, CreateUav>(originals[26]),
            create_sampler: std::mem::transmute::<usize, CreateSampler>(originals[27]),
            create_sampler2: sampler2_index
                .map(|index| std::mem::transmute::<usize, CreateSampler2>(originals[index])),
            create_feedback: feedback_index
                .map(|index| std::mem::transmute::<usize, CreateFeedback>(originals[index])),
            begin_pass: pass_index.map(|i| std::mem::transmute::<usize, BeginPass>(originals[i])),
            end_pass: pass_index.map(|i| std::mem::transmute::<usize, EndPass>(originals[i + 1])),
            create_stream_pso: stream_index
                .map(|i| std::mem::transmute::<usize, CreateStreamPso>(originals[i])),
            enhanced: enhanced_index.map(|i| std::mem::transmute::<usize, Enhanced>(originals[i])),
        }
    };
    ORIGINALS
        .set(tr)
        .map_err(|_| "depth observers already registered")?;
    set_queue_observer(Box::new(Observer)).map_err(|_| "queue observer already registered")?;
    unsafe {
        crate::native_graphics_observer::install(&list)?;
    }
    for target in targets {
        let status = unsafe { MH_EnableHook(target as _) };
        if status != MH_STATUS::MH_OK {
            return Err(format!("enable depth observer: {status:?}"));
        }
    }
    crate::native_descriptor_resources::activate();
    // Public setters are an API/vtable dependency, not a private game RVA.
    // A list reset while hooks were being enabled may have missed setters.
    // Never promote that partial history into a restorable graphics snapshot.
    let mut tracker = tracker()
        .lock()
        .map_err(|_| "graphics observer admission lock is poisoned")?;
    for state in tracker.lists.values_mut() {
        state.graphics.invalidate();
    }
    GRAPHICS_READY.store(true, Ordering::Release);
    Ok(())
}

pub fn initialize(device: &ID3D12Device) -> Result<(), String> {
    INSTALL
        .get_or_init(|| {
            // SAFETY: hudhook initialized MinHook and the DLL remains loaded.
            unsafe { install(device) }
        })
        .clone()
}

fn eligible(state: &State, queue: usize, frame: u64, width: u32, height: u32) -> bool {
    state.queue == queue
        && state.state.is_some()
        && !state.split
        && state.full_viewport
        && state.reverse_z.is_some()
        && state.cleared_frame == Some(frame)
        && state.object.width == width
        && state.object.height == height
        && state.frame == frame
}

/// Select copied geometry input using actual submitted surface facts. No depth
/// readback, GPU wait, world-recency guess or resource ownership is involved.
pub(crate) fn geometry_world(
    queue: &ID3D12CommandQueue,
    width: u32,
    height: u32,
) -> Result<Option<crate::world_hooks::GeometryInput>, String> {
    if !INSTALL.get().is_some_and(Result::is_ok)
        || !(1..=3840).contains(&width)
        || !(1..=2160).contains(&height)
    {
        return Ok(None);
    }
    let surface = {
        let _gate = SUBMIT
            .lock()
            .map_err(|_| "submission observer lock failed")?;
        let t = tracker().lock().map_err(|_| "depth tracker lock failed")?;
        let queue = queue.as_raw() as usize;
        if let Some(surface) = snapshot::geometry_surface(
            queue,
            t.frame,
            width,
            height,
            crate::world_hooks::has_snapshot,
        )? {
            Some(surface)
        } else {
            let mut candidates = t.resources.values().filter(|state| {
                eligible(state, queue, t.frame, width, height)
                    && state.scope.is_some_and(crate::world_hooks::has_snapshot)
            });
            let surface = candidates
                .next()
                .and_then(|state| Some((state.scope?, state.reverse_z?)));
            if candidates.next().is_some() {
                return Err("multiple submitted geometry world surfaces".into());
            }
            surface
        }
    };
    Ok(surface.and_then(|(scope, reverse)| {
        crate::world_hooks::geometry_input(scope, width, height, reverse)
    }))
}

/// Select by actual submitted depth scope, including a buffered CPU frame.
/// Camera recency alone never provides a scope or authorizes a depth resource.
pub(crate) fn capture_world(
    queue: &ID3D12CommandQueue,
    width: u32,
    height: u32,
) -> Result<Option<(u64, crate::depth::Depth)>, String> {
    let scope = {
        let _gate = SUBMIT
            .lock()
            .map_err(|_| "submission observer lock failed")?;
        let frame = tracker()
            .lock()
            .map_err(|_| "depth tracker lock failed")?
            .frame;
        let _own_copy = private_commands();
        if let Some(depth) = snapshot::capture(
            queue,
            frame,
            width,
            height,
            crate::world_hooks::has_snapshot,
        )? {
            return Ok(Some(depth));
        }
        let mut t = tracker().lock().map_err(|_| "depth tracker lock failed")?;
        let mut candidates = t.resources.values().filter(|state| {
            eligible(state, queue.as_raw() as usize, t.frame, width, height)
                && state.scope.is_some_and(crate::world_hooks::has_snapshot)
        });
        let scope = candidates.next().and_then(|state| state.scope);
        if candidates.next().is_some() {
            return Err("multiple scoped depth buffers match buffered worlds".into());
        }
        if scope.is_none() {
            t.last_capture = Some(
                serde_json::json!({"scope":null,"frame":t.frame,"width":width,"height":height,
                "resources":t.resources.values().take(MAX_RESOURCES).map(|state|serde_json::json!({"width":state.object.width,"height":state.object.height,"scope":state.scope,
                    "frame":state.frame,"clearedFrame":state.cleared_frame,"queueMatches":state.queue==queue.as_raw() as usize,"fullViewport":state.full_viewport,"reverseKnown":state.reverse_z.is_some(),"split":state.split,"stateKnown":state.state.is_some()})).collect::<Vec<_>>()}),
            );
        }
        scope
    };
    // capture reacquires the submission gate and revalidates every resource
    // condition, so a queue change between discovery and copying fails closed.
    let Some(scope) = scope else {
        return Ok(None);
    };
    Ok(capture(queue, scope, width, height)?.map(|depth| (scope, depth)))
}

pub fn capture(
    queue: &ID3D12CommandQueue,
    scope: u64,
    width: u32,
    height: u32,
) -> Result<Option<crate::depth::Depth>, String> {
    if !INSTALL.get().is_some_and(Result::is_ok) {
        return Ok(None);
    }
    let _gate = SUBMIT
        .lock()
        .map_err(|_| "submission observer lock failed")?;
    let candidate = {
        let mut t = tracker().lock().map_err(|_| "depth tracker lock failed")?;
        // Bounded selection evidence, without COM addresses or changing which
        // resource is accepted. This distinguishes absent cross-thread scope
        // propagation from stale state, wrong dimensions or missing clears.
        t.last_capture = Some(
            serde_json::json!({"scope":scope,"frame":t.frame,"width":width,"height":height,
            "resources":t.resources.values().take(MAX_RESOURCES).map(|state|serde_json::json!({
                "width":state.object.width,"height":state.object.height,"scope":state.scope,
                "frame":state.frame,"clearedFrame":state.cleared_frame,"queueMatches":state.queue==queue.as_raw() as usize,
                "fullViewport":state.full_viewport,"reverseKnown":state.reverse_z.is_some(),"split":state.split,"stateKnown":state.state.is_some()
            })).collect::<Vec<_>>()}),
        );
        let mut candidates = t.resources.values().filter(|state| {
            state.scope == Some(scope)
                && eligible(state, queue.as_raw() as usize, t.frame, width, height)
        });
        let candidate = candidates.next().map(|state| {
            (
                state.object.resource.clone(),
                state.state.unwrap(),
                state.reverse_z.unwrap(),
            )
        });
        if candidates.next().is_some() {
            return Err(
                "multiple depth buffers match this world pass; refusing ambiguous terrain depth"
                    .into(),
            );
        }
        candidate
    };
    // Keep the gate through readback. Other threads must not change the GPU
    // resource state after selection but before this copy enters the queue.
    let _own_copy = private_commands();
    candidate
        .map(|(resource, state, reverse)| crate::depth::dx12(resource, queue, state, reverse))
        .transpose()
}

pub fn end_frame() {
    observe(Tracker::retire)
}
pub fn status() -> serde_json::Value {
    let Ok(t) = tracker().try_lock() else {
        return serde_json::Value::Null;
    };
    serde_json::json!({"installed":INSTALL.get().is_some_and(Result::is_ok),"deviceCreationObserver":DEVICE_HOOK.get().is_some_and(Result::is_ok),"observedDeviceCreations":DEVICE_CREATIONS.load(Ordering::Relaxed),"resources":t.resources.len(),"views":t.views.len(),"lists":t.lists.len(),"submissions":t.submissions,"unknownSubmissions":t.unknown_submissions,"lastCapture":t.last_capture,"preservedDepth":snapshot::status(),"nativeQueue":native_queue::status(),"snapshotOwners":snapshot::owner_trace(&t),"scopedDepthLoss":{"clears":t.cleared_scoped_depth,"writes":t.overwritten_scoped_depth,"lastClear":t.last_scoped_clear},"drawScopes":{"scoped":t.scoped_draws,"unscoped":t.unscoped_draws,"backend":t.backend_draws,"callerSamples":t.caller_samples,"gameCallerRvas":t.unscoped_callers},"trace":{"missingLists":t.missing_lists,"unclosedLists":t.unclosed_lists,"invalidLists":t.invalid_lists,"listEvictions":t.list_evictions,"unknownBindings":t.unknown_bindings,"enhancedCalls":t.enhanced_calls,"indirectCalls":t.indirect_calls,"bundleCalls":t.bundle_calls,"createdLists":t.created_lists,"resetLists":t.reset_lists,"closedLists":t.closed_lists,"closedListDraws":t.closed_list_draws,"closedListSamples":t.closed_list_samples,"createdViews":t.created_views,"rejectedViews":t.rejected_views}})
}

#[cfg(test)]
mod tests {
    use super::*;
    unsafe fn depth_pipelines(
        device: &ID3D12Device,
    ) -> Result<
        (
            ID3D12RootSignature,
            ID3D12PipelineState,
            ID3D12PipelineState,
            ID3D12PipelineState,
        ),
        Box<dyn std::error::Error>,
    > {
        use hudhook::windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
        use hudhook::windows::Win32::Graphics::Direct3D::{
            D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST, ID3DBlob, ID3DInclude,
        };
        use hudhook::windows::Win32::Graphics::Dxgi::Common::{
            DXGI_FORMAT_D32_FLOAT, DXGI_SAMPLE_DESC,
        };
        use hudhook::windows::core::s;
        use std::mem::ManuallyDrop;
        let _ = D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST;
        let source = b"float4 main(uint i:SV_VertexID):SV_POSITION { float2 p[6]={float2(-1,-1),float2(-1,1),float2(0,1),float2(-1,-1),float2(0,1),float2(0,-1)};return float4(p[i],0.25,1);}";
        let mut shader: Option<ID3DBlob> = None;
        unsafe {
            D3DCompile(
                source.as_ptr() as _,
                source.len(),
                None,
                None,
                None::<&ID3DInclude>,
                s!("main"),
                s!("vs_5_0"),
                0,
                0,
                &mut shader,
                None,
            )
        }?;
        let shader = shader.ok_or("no test vertex shader")?;
        let mut signature: Option<ID3DBlob> = None;
        unsafe {
            D3D12SerializeRootSignature(
                &D3D12_ROOT_SIGNATURE_DESC::default(),
                D3D_ROOT_SIGNATURE_VERSION_1,
                &mut signature,
                None,
            )
        }?;
        let signature = signature.ok_or("no test root signature")?;
        let root: ID3D12RootSignature = unsafe {
            device.CreateRootSignature(
                0,
                std::slice::from_raw_parts(
                    signature.GetBufferPointer() as *const u8,
                    signature.GetBufferSize(),
                ),
            )
        }?;
        let mut desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            pRootSignature: ManuallyDrop::new(Some(root.clone())),
            VS: D3D12_SHADER_BYTECODE {
                pShaderBytecode: unsafe { shader.GetBufferPointer() },
                BytecodeLength: unsafe { shader.GetBufferSize() },
            },
            RasterizerState: D3D12_RASTERIZER_DESC {
                FillMode: D3D12_FILL_MODE_SOLID,
                CullMode: D3D12_CULL_MODE_NONE,
                DepthClipEnable: true.into(),
                ..Default::default()
            },
            DepthStencilState: D3D12_DEPTH_STENCIL_DESC {
                DepthEnable: true.into(),
                DepthWriteMask: D3D12_DEPTH_WRITE_MASK_ALL,
                DepthFunc: D3D12_COMPARISON_FUNC_LESS_EQUAL,
                StencilReadMask: 255,
                StencilWriteMask: 255,
                ..Default::default()
            },
            PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
            DSVFormat: DXGI_FORMAT_D32_FLOAT,
            SampleMask: u32::MAX,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            ..Default::default()
        };
        let writer: ID3D12PipelineState = unsafe { device.CreateGraphicsPipelineState(&desc) }?;
        desc.DepthStencilState.DepthWriteMask = D3D12_DEPTH_WRITE_MASK_ZERO;
        let reader: ID3D12PipelineState = unsafe { device.CreateGraphicsPipelineState(&desc) }?;
        desc.DepthStencilState.DepthWriteMask = D3D12_DEPTH_WRITE_MASK_ALL;
        desc.DepthStencilState.DepthFunc = D3D12_COMPARISON_FUNC_ALWAYS;
        let overwriter: ID3D12PipelineState = unsafe { device.CreateGraphicsPipelineState(&desc) }?;
        unsafe { ManuallyDrop::drop(&mut desc.pRootSignature) };
        assert_eq!(
            unsafe { crate::depth_pipeline::mode(writer.as_raw()) },
            Some(true)
        );
        assert_eq!(
            unsafe { crate::depth_pipeline::mode(reader.as_raw()) },
            Some(false)
        );
        Ok((root, writer, reader, overwriter))
    }
    #[test]
    fn real_d3d12_hooks_track_execution_order_and_copy_only_the_matching_world_pass()
    -> Result<(), Box<dyn std::error::Error>> {
        use hudhook::mh::MH_Initialize;
        use hudhook::windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0;
        use hudhook::windows::Win32::Graphics::Dxgi::Common::{
            DXGI_FORMAT_D32_FLOAT, DXGI_SAMPLE_DESC,
        };
        use hudhook::windows::Win32::Graphics::Dxgi::{
            CreateDXGIFactory1, IDXGIAdapter, IDXGIFactory4,
        };
        use std::mem::ManuallyDrop;
        let _gpu = crate::depth::GPU_TEST.lock().unwrap();
        unsafe {
            let initialized = MH_Initialize();
            assert!(matches!(
                initialized,
                MH_STATUS::MH_OK | MH_STATUS::MH_ERROR_ALREADY_INITIALIZED
            ));
            observe_device_creation()?;
            let factory: IDXGIFactory4 = CreateDXGIFactory1()?;
            let adapter: IDXGIAdapter = factory.EnumWarpAdapter()?;
            let mut device: Option<ID3D12Device> = None;
            D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_11_0, &mut device)?;
            let device = device.ok_or("WARP device missing")?;
            let queue: ID3D12CommandQueue =
                device.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC {
                    Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
                    ..Default::default()
                })?;
            assert!(
                INSTALL.get().is_some_and(Result::is_ok),
                "actual device creation must initialize tracking before any PSO/DSV or swap chain"
            );
            assert!(DEVICE_CREATIONS.load(Ordering::Relaxed) > 0);
            let (root, writer, reader, overwriter) = depth_pipelines(&device)?;
            let mut resource: Option<ID3D12Resource> = None;
            device.CreateCommittedResource(
                &D3D12_HEAP_PROPERTIES {
                    Type: D3D12_HEAP_TYPE_DEFAULT,
                    CreationNodeMask: 1,
                    VisibleNodeMask: 1,
                    ..Default::default()
                },
                D3D12_HEAP_FLAG_NONE,
                &D3D12_RESOURCE_DESC {
                    Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
                    Width: 318,
                    Height: 178,
                    DepthOrArraySize: 1,
                    MipLevels: 1,
                    Format: DXGI_FORMAT_D32_FLOAT,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    Flags: D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL,
                    ..Default::default()
                },
                D3D12_RESOURCE_STATE_DEPTH_WRITE,
                None,
                &mut resource,
            )?;
            let resource = resource.ok_or("no depth resource")?;
            let heap: ID3D12DescriptorHeap =
                device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                    Type: D3D12_DESCRIPTOR_HEAP_TYPE_DSV,
                    NumDescriptors: 6,
                    ..Default::default()
                })?;
            let handle = heap.GetCPUDescriptorHandleForHeapStart();
            device.CreateDepthStencilView(&resource, None, handle);
            let allocator_a: ID3D12CommandAllocator =
                device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
            let allocator_b: ID3D12CommandAllocator =
                device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
            let a: ID3D12GraphicsCommandList =
                device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &allocator_a, None)?;
            let b: ID3D12GraphicsCommandList =
                device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &allocator_b, None)?;
            let transition = |list: &ID3D12GraphicsCommandList, before, after| {
                let mut barrier = D3D12_RESOURCE_BARRIER {
                    Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
                    Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
                    Anonymous: D3D12_RESOURCE_BARRIER_0 {
                        Transition: ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                            pResource: ManuallyDrop::new(Some(resource.clone())),
                            Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                            StateBefore: before,
                            StateAfter: after,
                        }),
                    },
                };
                list.ResourceBarrier(std::slice::from_ref(&barrier));
                ManuallyDrop::drop(&mut (*barrier.Anonymous.Transition).pResource);
            };
            // Record B first, but submit A then B. Recording order must not
            // become the assumed GPU state used by the subsequent readback.
            transition(
                &b,
                D3D12_RESOURCE_STATE_DEPTH_READ,
                D3D12_RESOURCE_STATE_DEPTH_WRITE,
            );
            b.Close()?;
            crate::world_hooks::with_test_scope(77, || {
                a.RSSetViewports(&[D3D12_VIEWPORT {
                    Width: 318.,
                    Height: 178.,
                    MinDepth: 0.,
                    MaxDepth: 1.,
                    ..Default::default()
                }]);
                a.OMSetRenderTargets(0, None, false, Some(&handle));
                a.ClearDepthStencilView(handle, D3D12_CLEAR_FLAG_DEPTH, 1., 0, None);
                a.RSSetScissorRects(&[RECT {
                    left: 0,
                    top: 0,
                    right: 318,
                    bottom: 178,
                }]);
                a.SetGraphicsRootSignature(&root);
                a.SetPipelineState(&writer);
                a.IASetPrimitiveTopology(hudhook::windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
                a.DrawInstanced(6, 1, 0, 0);
                transition(
                    &a,
                    D3D12_RESOURCE_STATE_DEPTH_WRITE,
                    D3D12_RESOURCE_STATE_DEPTH_READ,
                );
            });
            a.Close()?;
            assert!(
                tracker().lock().unwrap().resources[&(resource.as_raw() as usize)]
                    .state
                    .is_none()
            );
            let lists = [a.cast::<ID3D12CommandList>()?, b.cast()?];
            let mut submit =
                || queue.ExecuteCommandLists(&[Some(lists[0].clone()), Some(lists[1].clone())]);
            // The DLL uses hudhook's real ExecuteCommandLists detour. This unit
            // process drives the same observer around the real GPU submission.
            Observer.execute(&queue, &lists, &mut submit);
            assert_eq!(
                tracker().lock().unwrap().resources[&(resource.as_raw() as usize)].state,
                Some(D3D12_RESOURCE_STATE_DEPTH_WRITE)
            );
            assert!(capture(&queue, 78, 318, 178)?.is_none());
            assert!(capture(&queue, 77, 640, 480)?.is_none());
            let depth = capture(&queue, 77, 318, 178)?.ok_or("matching depth not observed")?;
            assert!((depth.values[10] - 0.25).abs() < 0.0001);
            assert_eq!(depth.values[300], 1.);
            let again =
                capture(&queue, 77, 318, 178)?.ok_or("readback failed to preserve state")?;
            assert_eq!(again.values, depth.values);
            // A real draw with depth writes disabled cannot claim a new world
            // pass, even while its resource is in DEPTH_WRITE state.
            allocator_a.Reset()?;
            a.Reset(&allocator_a, &reader)?;
            crate::world_hooks::with_test_scope(79, || {
                a.RSSetViewports(&[D3D12_VIEWPORT {
                    Width: 318.,
                    Height: 178.,
                    MinDepth: 0.,
                    MaxDepth: 1.,
                    ..Default::default()
                }]);
                a.RSSetScissorRects(&[RECT {
                    left: 0,
                    top: 0,
                    right: 318,
                    bottom: 178,
                }]);
                a.OMSetRenderTargets(0, None, false, Some(&handle));
                a.ClearDepthStencilView(handle, D3D12_CLEAR_FLAG_DEPTH, 1., 0, None);
                a.SetGraphicsRootSignature(&root);
                a.IASetPrimitiveTopology(hudhook::windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
                a.DrawInstanced(6, 1, 0, 0);
            });
            a.Close()?;
            let read_list = a.cast::<ID3D12CommandList>()?;
            Observer.execute(&queue, std::slice::from_ref(&read_list), &mut || {
                queue.ExecuteCommandLists(&[Some(read_list.clone())])
            });
            assert!(capture(&queue, 79, 318, 178)?.is_none());
            // Wait for this submission before releasing the allocator/resources.
            crate::depth::dx12(
                resource.clone(),
                &queue,
                D3D12_RESOURCE_STATE_DEPTH_WRITE,
                false,
            )?;
            // Follow actual descriptor copies, including range sizes omitted
            // (one descriptor per range), before binding through BeginRenderPass.
            let increment =
                device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_DSV) as usize;
            let at = |n| D3D12_CPU_DESCRIPTOR_HANDLE {
                ptr: handle.ptr + n * increment,
            };
            device.CopyDescriptorsSimple(1, at(1), handle, D3D12_DESCRIPTOR_HEAP_TYPE_DSV);
            device.CopyDescriptors(
                1,
                &at(2),
                None,
                1,
                &at(1),
                None,
                D3D12_DESCRIPTOR_HEAP_TYPE_DSV,
            );
            let sizes = [2u32];
            device.CopyDescriptors(
                1,
                &at(3),
                Some(sizes.as_ptr()),
                1,
                &handle,
                Some(sizes.as_ptr()),
                D3D12_DESCRIPTOR_HEAP_TYPE_DSV,
            );
            for n in 0..5 {
                assert_eq!(
                    tracker().lock().unwrap().views[&at(n).ptr]
                        .resource
                        .as_raw(),
                    resource.as_raw()
                );
            }
            // Copying an untracked descriptor must erase a previously known
            // destination, rather than reusing a stale depth resource.
            device.CreateDepthStencilView(
                None,
                Some(&D3D12_DEPTH_STENCIL_VIEW_DESC {
                    Format: DXGI_FORMAT_D32_FLOAT,
                    ViewDimension: D3D12_DSV_DIMENSION_TEXTURE2D,
                    ..Default::default()
                }),
                at(5),
            );
            device.CopyDescriptorsSimple(1, at(4), at(5), D3D12_DESCRIPTOR_HEAP_TYPE_DSV);
            assert!(!tracker().lock().unwrap().views.contains_key(&at(4).ptr));
            let pass = a.cast::<ID3D12GraphicsCommandList4>()?;
            let mut desc = D3D12_RENDER_PASS_DEPTH_STENCIL_DESC {
                cpuDescriptor: at(3),
                DepthBeginningAccess: D3D12_RENDER_PASS_BEGINNING_ACCESS {
                    Type: D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_CLEAR,
                    Anonymous: D3D12_RENDER_PASS_BEGINNING_ACCESS_0 {
                        Clear: D3D12_RENDER_PASS_BEGINNING_ACCESS_CLEAR_PARAMETERS {
                            ClearValue: D3D12_CLEAR_VALUE {
                                Format: DXGI_FORMAT_D32_FLOAT,
                                Anonymous: D3D12_CLEAR_VALUE_0 {
                                    DepthStencil: D3D12_DEPTH_STENCIL_VALUE {
                                        Depth: 1.,
                                        Stencil: 0,
                                    },
                                },
                            },
                        },
                    },
                },
                StencilBeginningAccess: D3D12_RENDER_PASS_BEGINNING_ACCESS {
                    Type: D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_NO_ACCESS,
                    ..Default::default()
                },
                DepthEndingAccess: D3D12_RENDER_PASS_ENDING_ACCESS {
                    Type: D3D12_RENDER_PASS_ENDING_ACCESS_TYPE_PRESERVE,
                    ..Default::default()
                },
                StencilEndingAccess: D3D12_RENDER_PASS_ENDING_ACCESS {
                    Type: D3D12_RENDER_PASS_ENDING_ACCESS_TYPE_NO_ACCESS,
                    ..Default::default()
                },
            };
            for (scope, discard) in [(80, false), (81, true)] {
                allocator_a.Reset()?;
                a.Reset(&allocator_a, &writer)?;
                if discard {
                    desc.DepthEndingAccess.Type = D3D12_RENDER_PASS_ENDING_ACCESS_TYPE_DISCARD;
                }
                crate::world_hooks::with_test_scope(scope, || {
                    a.RSSetViewports(&[D3D12_VIEWPORT {
                        Width: 318.,
                        Height: 178.,
                        MinDepth: 0.,
                        MaxDepth: 1.,
                        ..Default::default()
                    }]);
                    a.RSSetScissorRects(&[RECT {
                        left: 0,
                        top: 0,
                        right: 318,
                        bottom: 178,
                    }]);
                    pass.BeginRenderPass(None, Some(&desc), D3D12_RENDER_PASS_FLAG_NONE);
                    a.SetGraphicsRootSignature(&root);
                    a.IASetPrimitiveTopology(hudhook::windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
                    a.DrawInstanced(6, 1, 0, 0);
                    pass.EndRenderPass();
                });
                a.Close()?;
                let list = a.cast::<ID3D12CommandList>()?;
                Observer.execute(&queue, std::slice::from_ref(&list), &mut || {
                    queue.ExecuteCommandLists(&[Some(list.clone())])
                });
                let depth = capture(&queue, scope, 318, 178)?;
                if discard {
                    assert!(
                        depth.is_none(),
                        "discarded render-pass depth must not be read back"
                    );
                } else {
                    let depth = depth
                        .ok_or("render-pass clear/draw through a copied DSV was not observed")?;
                    assert!((depth.values[10] - 0.25).abs() < 0.0001);
                    assert_eq!(depth.values[300], 1.);
                }
                // Fence the real submission before reusing this allocator.
                crate::depth::dx12(
                    resource.clone(),
                    &queue,
                    D3D12_RESOURCE_STATE_DEPTH_WRITE,
                    false,
                )?;
            }
            // Preserve real geometry before a later native clear destroys it.
            // The copy is recorded in A, not a separate submission after clear.
            allocator_a.Reset()?;
            a.Reset(&allocator_a, &writer)?;
            crate::world_hooks::with_test_scope(82, || {
                a.RSSetViewports(&[D3D12_VIEWPORT {
                    Width: 318.,
                    Height: 178.,
                    MinDepth: 0.,
                    MaxDepth: 1.,
                    ..Default::default()
                }]);
                a.RSSetScissorRects(&[RECT {
                    left: 0,
                    top: 0,
                    right: 318,
                    bottom: 178,
                }]);
                a.OMSetRenderTargets(0, None, false, Some(&handle));
                a.ClearDepthStencilView(handle, D3D12_CLEAR_FLAG_DEPTH, 1., 0, None);
                a.SetGraphicsRootSignature(&root);
                a.IASetPrimitiveTopology(hudhook::windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
                a.DrawInstanced(6, 1, 0, 0);
            });
            a.ClearDepthStencilView(handle, D3D12_CLEAR_FLAG_DEPTH, 1., 0, None);
            a.Close()?;
            let list = a.cast::<ID3D12CommandList>()?;
            let native_resource_count = tracker().lock().unwrap().resources.len();
            for _ in 0..2 {
                // Closed native lists can legally execute again. The copy's
                // destination must begin/end in the same state on each run.
                Observer.execute(&queue, std::slice::from_ref(&list), &mut || {
                    queue.ExecuteCommandLists(&[Some(list.clone())])
                });
                let _gate = SUBMIT.lock().unwrap();
                let _own_copy = private_commands();
                let frame = tracker().lock().unwrap().frame;
                let (scope, saved) = snapshot::capture(&queue, frame, 318, 178, |s| s == 82)?
                    .ok_or("pre-clear world depth snapshot missing")?;
                assert_eq!(scope, 82);
                assert!((saved.values[10] - 0.25).abs() < 0.0001);
                assert_eq!(saved.values[300], 1.);
                let erased = crate::depth::dx12(
                    resource.clone(),
                    &queue,
                    D3D12_RESOURCE_STATE_DEPTH_WRITE,
                    false,
                )?;
                assert!(erased.values.iter().all(|d| *d == 1.));
            }
            assert_eq!(
                tracker().lock().unwrap().resources.len(),
                native_resource_count,
                "private snapshot readbacks must not consume native depth-cache capacity"
            );
            // Reset is legal after ExecuteCommandLists returns while GPU work
            // remains pending. A queued GPU wait makes that race reproducible.
            let hold: ID3D12Fence = device.CreateFence(0, D3D12_FENCE_FLAG_NONE)?;
            queue.Wait(&hold, 1)?;
            Observer.execute(&queue, std::slice::from_ref(&list), &mut || {
                queue.ExecuteCommandLists(&[Some(list.clone())])
            });
            let pending_check = (|| -> Result<(u64, u64), Box<dyn std::error::Error>> {
                allocator_b.Reset()?;
                a.Reset(&allocator_b, &writer)?;
                let before = snapshot::status()["retiredPending"].as_u64().unwrap();
                crate::world_hooks::with_test_scope(83, || {
                    a.RSSetViewports(&[D3D12_VIEWPORT {
                        Width: 318.,
                        Height: 178.,
                        MinDepth: 0.,
                        MaxDepth: 1.,
                        ..Default::default()
                    }]);
                    a.RSSetScissorRects(&[RECT {
                        left: 0,
                        top: 0,
                        right: 318,
                        bottom: 178,
                    }]);
                    a.OMSetRenderTargets(0, None, false, Some(&handle));
                    a.ClearDepthStencilView(handle, D3D12_CLEAR_FLAG_DEPTH, 1., 0, None);
                    a.SetGraphicsRootSignature(&root);
                    a.IASetPrimitiveTopology(hudhook::windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
                    a.DrawInstanced(6, 1, 0, 0);
                });
                a.ClearDepthStencilView(handle, D3D12_CLEAR_FLAG_DEPTH, 1., 0, None);
                a.Close()?;
                Ok((
                    before,
                    snapshot::status()["retiredPending"].as_u64().unwrap(),
                ))
            })();
            // Always release the GPU wait, including a failed recording path.
            hold.Signal(1)?;
            crate::depth::dx12(
                resource.clone(),
                &queue,
                D3D12_RESOURCE_STATE_DEPTH_WRITE,
                false,
            )?;
            let (pending_before, pending_after) = pending_check?;
            assert!(pending_before > 0, "reset must retain the in-flight copy");
            assert_eq!(
                pending_before, pending_after,
                "a new recording must not reuse a GPU-pending slot"
            );
            a.Reset(&allocator_b, &writer)?;
            a.Close()?;
            // An unscoped draw can also destroy depth without clearing it.
            // Its ALWAYS PSO and collapsed depth viewport overwrite the actual
            // geometry with 0.75 after the scoped 0.25 draw.
            allocator_b.Reset()?;
            a.Reset(&allocator_b, &writer)?;
            crate::world_hooks::with_test_scope(84, || {
                a.RSSetViewports(&[D3D12_VIEWPORT {
                    Width: 318.,
                    Height: 178.,
                    MinDepth: 0.,
                    MaxDepth: 1.,
                    ..Default::default()
                }]);
                a.RSSetScissorRects(&[RECT {
                    left: 0,
                    top: 0,
                    right: 318,
                    bottom: 178,
                }]);
                a.OMSetRenderTargets(0, None, false, Some(&handle));
                a.ClearDepthStencilView(handle, D3D12_CLEAR_FLAG_DEPTH, 1., 0, None);
                a.SetGraphicsRootSignature(&root);
                a.IASetPrimitiveTopology(hudhook::windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
                a.DrawInstanced(6, 1, 0, 0);
            });
            a.SetPipelineState(&overwriter);
            a.RSSetViewports(&[D3D12_VIEWPORT {
                Width: 318.,
                Height: 178.,
                MinDepth: 0.75,
                MaxDepth: 0.75,
                ..Default::default()
            }]);
            a.DrawInstanced(6, 1, 0, 0);
            // A later world view in the same list narrows the occluder. Its
            // second copy must reuse the destination and supersede the first.
            a.ClearDepthStencilView(handle, D3D12_CLEAR_FLAG_DEPTH, 1., 0, None);
            a.SetPipelineState(&writer);
            a.RSSetViewports(&[D3D12_VIEWPORT {
                Width: 318.,
                Height: 178.,
                MinDepth: 0.,
                MaxDepth: 1.,
                ..Default::default()
            }]);
            a.RSSetScissorRects(&[RECT {
                left: 80,
                top: 0,
                right: 318,
                bottom: 178,
            }]);
            crate::world_hooks::with_test_scope(85, || a.DrawInstanced(6, 1, 0, 0));
            a.SetPipelineState(&overwriter);
            a.RSSetScissorRects(&[RECT {
                left: 0,
                top: 0,
                right: 318,
                bottom: 178,
            }]);
            a.RSSetViewports(&[D3D12_VIEWPORT {
                Width: 318.,
                Height: 178.,
                MinDepth: 0.9,
                MaxDepth: 0.9,
                ..Default::default()
            }]);
            a.DrawInstanced(6, 1, 0, 0);
            assert_eq!(
                snapshot::status()["owned"].as_u64(),
                Some(1),
                "multiple boundaries on one native list must share one texture"
            );
            a.Close()?;
            let list = a.cast::<ID3D12CommandList>()?;
            Observer.execute(&queue, std::slice::from_ref(&list), &mut || {
                queue.ExecuteCommandLists(&[Some(list.clone())])
            });
            {
                let _gate = SUBMIT.lock().unwrap();
                let _own_copy = private_commands();
                let frame = tracker().lock().unwrap().frame;
                assert!(snapshot::capture(&queue, frame, 318, 178, |s| s == 84)?.is_none());
                let (_, saved) = snapshot::capture(&queue, frame, 318, 178, |s| s == 85)?
                    .ok_or("pre-draw depth snapshot missing")?;
                assert_eq!(saved.values[10], 1.);
                assert!((saved.values[100] - 0.25).abs() < 0.0001);
                let overwritten = crate::depth::dx12(
                    resource.clone(),
                    &queue,
                    D3D12_RESOURCE_STATE_DEPTH_WRITE,
                    false,
                )?;
                assert!((overwritten.values[10] - 0.9).abs() < 0.0001);
            }
            // Model the pinned queue's used->free handoff using this real GPU
            // submission. Merely completing a fence without moving the context
            // must not retire an executable native list.
            let handoff_fence: ID3D12Fence = device.CreateFence(0, D3D12_FENCE_FLAG_NONE)?;
            queue.Signal(&handoff_fence, 9)?;
            crate::depth::dx12(
                resource.clone(),
                &queue,
                D3D12_RESOURCE_STATE_DEPTH_WRITE,
                false,
            )?;
            use crate::native_scene_layout::native_queue as queue_layout;
            let mut queue_model = vec![0u8; queue_layout::FENCE_CONTEXT + 8];
            let mut context_model =
                vec![0u8; queue_layout::CONTEXT_DEFERRED + queue_layout::DEFERRED_BYTES];
            let mut fence_model = vec![0u8; queue_layout::FENCE_COMPLETED + 8];
            let put = |bytes: &mut [u8], at: usize, value: u64| {
                bytes[at..at + 8].copy_from_slice(&value.to_le_bytes())
            };
            put(
                &mut context_model,
                queue_layout::CONTEXT_LIST,
                a.as_raw() as u64,
            );
            put(&mut context_model, queue_layout::CONTEXT_FENCE, 9);
            context_model[queue_layout::CONTEXT_SUBMITTED] = 1;
            put(
                &mut fence_model,
                queue_layout::FENCE_OBJECT,
                handoff_fence.as_raw() as u64,
            );
            put(
                &mut fence_model,
                queue_layout::FENCE_COMPLETED,
                handoff_fence.GetCompletedValue(),
            );
            put(
                &mut queue_model,
                queue_layout::QUEUE_OBJECT,
                queue.as_raw() as u64,
            );
            put(
                &mut queue_model,
                queue_layout::FENCE_CONTEXT,
                fence_model.as_ptr() as u64,
            );
            put(
                &mut queue_model,
                queue_layout::USED,
                context_model.as_ptr() as u64,
            );
            put(
                &mut queue_model,
                queue_layout::FREE,
                context_model.as_ptr() as u64,
            );
            put(
                &mut queue_model,
                queue_layout::FREE_READ,
                queue_layout::CAPACITY as u64,
            );
            put(
                &mut queue_model,
                queue_layout::FREE_WRITE,
                queue_layout::CAPACITY as u64,
            );
            let model = queue_model.as_ptr() as usize;
            let owned = snapshot::status()["owned"].as_u64().unwrap();
            let stale_ticket =
                snapshot::native_candidates(queue.as_raw() as usize, a.as_raw() as usize);
            let pending =
                native_queue::before(model, 9).ok_or("native handoff candidate missing")?;
            native_queue::after(model, pending).ok_or("used-only queue check failed")?;
            assert_eq!(snapshot::status()["owned"].as_u64(), Some(owned));
            let pending =
                native_queue::before(model, 9).ok_or("native handoff candidate disappeared")?;
            put(
                &mut queue_model,
                queue_layout::USED_READ,
                queue_layout::CAPACITY as u64,
            );
            put(
                &mut queue_model,
                queue_layout::USED_WRITE,
                queue_layout::CAPACITY as u64,
            );
            put(&mut queue_model, queue_layout::FREE_READ, 0);
            put(&mut queue_model, queue_layout::FREE_WRITE, 0);
            native_queue::after(model, pending).ok_or("native free-ring handoff rejected")?;
            assert_eq!(snapshot::status()["owned"].as_u64(), Some(owned - 1));
            // A list destroyed after submission will never Reset. Observe its
            // real final COM Release, retire ownership, and retain GPU contents.
            let scratch_allocator: ID3D12CommandAllocator =
                device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
            let scratch: ID3D12GraphicsCommandList = device.CreateCommandList(
                0,
                D3D12_COMMAND_LIST_TYPE_DIRECT,
                &scratch_allocator,
                &writer,
            )?;
            crate::world_hooks::with_test_scope(86, || {
                scratch.RSSetViewports(&[D3D12_VIEWPORT {
                    Width: 318.,
                    Height: 178.,
                    MinDepth: 0.,
                    MaxDepth: 1.,
                    ..Default::default()
                }]);
                scratch.RSSetScissorRects(&[RECT {
                    left: 0,
                    top: 0,
                    right: 318,
                    bottom: 178,
                }]);
                scratch.OMSetRenderTargets(0, None, false, Some(&handle));
                scratch.ClearDepthStencilView(handle, D3D12_CLEAR_FLAG_DEPTH, 1., 0, None);
                scratch.SetGraphicsRootSignature(&root);
                scratch.IASetPrimitiveTopology(hudhook::windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
                scratch.DrawInstanced(6, 1, 0, 0);
            });
            scratch.Close()?;
            {
                let _gate = SUBMIT.lock().unwrap();
                let tickets =
                    snapshot::native_candidates(queue.as_raw() as usize, scratch.as_raw() as usize);
                assert!(
                    !tickets.is_empty(),
                    "a queued context can be captured before public submission"
                );
                assert_eq!(
                    snapshot::native_retired(&tickets),
                    0,
                    "an unsubmitted generation cannot be retired"
                );
            }
            let scratch_list = scratch.cast::<ID3D12CommandList>()?;
            Observer.execute(&queue, std::slice::from_ref(&scratch_list), &mut || {
                queue.ExecuteCommandLists(&[Some(scratch_list.clone())])
            });
            {
                let _gate = SUBMIT.lock().unwrap();
                assert_eq!(
                    snapshot::native_retired(&stale_ticket),
                    0,
                    "old handoff must not retire a new reservation"
                );
            }
            let owned_before = snapshot::status()["owned"].as_u64().unwrap();
            drop(scratch_list);
            drop(scratch);
            {
                let _gate = SUBMIT.lock().unwrap();
                snapshot::retire_released();
                assert_eq!(snapshot::status()["owned"].as_u64(), Some(owned_before - 1));
                let _own_copy = private_commands();
                let frame = tracker().lock().unwrap().frame;
                let (_, saved) = snapshot::capture(&queue, frame, 318, 178, |s| s == 86)?
                    .ok_or("destroyed list's depth was released prematurely")?;
                assert!((saved.values[10] - 0.25).abs() < 0.0001);
            }
            // Unknown/split state must never become an all-far fallback.
            observe(|t| t.invalidate());
            assert!(capture(&queue, 77, 318, 178)?.is_none());
            end_frame();
            *tracker().lock().unwrap() = Tracker::default();
        }
        Ok(())
    }
}
