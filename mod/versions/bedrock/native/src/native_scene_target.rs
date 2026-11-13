//! Diagnostic association between submitted world draws and a real presented RTV.
//! This observes issued draws, not all subsequent compositing or shader execution.
use hudhook::windows::Win32::Graphics::Direct3D12::*;
use hudhook::windows::core::{GUID, IUnknown, Interface};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicU64, AtomicUsize, Ordering},
};
use std::time::Instant;

const MAX_TARGETS: usize = 128;
const TAG: GUID = GUID::from_u128(0x35cbb27f_a6ed_46fd_96a4_65a951ad9e1d);
static NEXT: AtomicU64 = AtomicU64::new(1);
static STRIDE: AtomicUsize = AtomicUsize::new(0);
static STATE: OnceLock<Mutex<State>> = OnceLock::new();
fn state() -> &'static Mutex<State> {
    STATE.get_or_init(Mutex::default)
}
struct Target {
    width: u32,
    height: u32,
    presented: bool,
    last: Option<(usize, u64, Instant)>,
}
#[derive(Default)]
struct State {
    targets: HashMap<u64, Target>,
    views: HashMap<usize, u64>,
    recorded: u64,
    matched: u64,
    omitted: u64,
    descriptor_invalidations: u64,
    descriptor_copies: u64,
}
fn identity(resource: &ID3D12Resource) -> Option<u64> {
    let mut bytes = [0; 8];
    let mut size = 8;
    unsafe { resource.GetPrivateData(&TAG, &mut size, Some(bytes.as_mut_ptr() as _)) }.ok()?;
    let id = u64::from_le_bytes(bytes);
    (size == 8 && id != 0).then_some(id)
}
impl State {
    fn invalidate_views(&mut self) {
        self.views.clear();
        self.descriptor_invalidations += 1;
        for target in self.targets.values_mut() {
            target.last = None;
        }
    }
    fn copy_views(&mut self, dst: Option<Vec<usize>>, src: Option<Vec<usize>>) {
        let Some((dst, src)) = dst.zip(src).filter(|(dst, src)| {
            dst.len() == src.len()
                && dst.len() <= 256
                && dst.iter().chain(src).all(|handle| *handle != 0)
        }) else {
            self.invalidate_views();
            return;
        };
        // Snapshot all sources before touching destinations, as for DSV metadata.
        // Unknown aliases cannot be attributed to a native world target.
        let known: Option<Vec<u64>> = src
            .iter()
            .map(|handle| {
                self.views
                    .get(handle)
                    .copied()
                    .filter(|id| self.targets.contains_key(id))
            })
            .collect();
        let Some(known) = known else {
            self.invalidate_views();
            return;
        };
        for (handle, id) in dst.into_iter().zip(known) {
            self.views.insert(handle, id);
            self.descriptor_copies += 1;
        }
        if self.views.len() > 4096 {
            self.invalidate_views();
        }
    }
    unsafe fn view(&mut self, raw: *mut core::ffi::c_void, handle: usize) {
        self.views.remove(&handle);
        if self.views.len() >= 4096 {
            self.views.clear();
        }
        let Some(object) = (unsafe { IUnknown::from_raw_borrowed(&raw) }) else {
            return;
        };
        let Ok(resource) = object.cast::<ID3D12Resource>() else {
            return;
        };
        if let Some(id) = identity(&resource)
            && self.targets.contains_key(&id)
        {
            self.views.insert(handle, id);
            return;
        }
        let desc = unsafe { resource.GetDesc() };
        if desc.Dimension != D3D12_RESOURCE_DIMENSION_TEXTURE2D
            || desc.DepthOrArraySize != 1
            || desc.MipLevels != 1
            || desc.SampleDesc.Count != 1
            || desc.Width == 0
            || desc.Width > 16384
            || desc.Height == 0
            || desc.Height > 16384
        {
            return;
        }
        if self.targets.len() >= MAX_TARGETS {
            let Some(id) = self
                .targets
                .iter()
                .min_by_key(|(id, t)| (t.presented, **id))
                .map(|(id, _)| *id)
            else {
                self.omitted += 1;
                return;
            };
            self.targets.remove(&id);
            self.views.retain(|_, v| *v != id);
        }
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let bytes = id.to_le_bytes();
        if unsafe { resource.SetPrivateData(&TAG, 8, Some(bytes.as_ptr() as _)) }.is_err() {
            self.omitted += 1;
            return;
        }
        self.targets.insert(
            id,
            Target {
                width: desc.Width as u32,
                height: desc.Height,
                presented: false,
                last: None,
            },
        );
        self.views.insert(handle, id);
    }
    fn draw(&mut self, queue: usize, targets: [u64; 8], scope: u64, width: u32, height: u32) {
        for id in targets {
            if let Some(target) = self.targets.get_mut(&id)
                && target.width == width
                && target.height == height
            {
                target.last = Some((queue, scope, Instant::now()));
                self.recorded += 1;
            }
        }
    }
    fn take(&mut self, id: u64, queue: usize) -> Option<(u64, u64)> {
        let target = self.targets.get_mut(&id)?;
        target.presented = true;
        let (actual, scope, at) = target.last.take()?;
        if actual != queue {
            return None;
        }
        self.matched += 1;
        Some((scope, at.elapsed().as_millis().min(u64::MAX as u128) as u64))
    }
}
pub(crate) fn initialize(device: &ID3D12Device) {
    STRIDE.store(
        unsafe { device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV) } as usize,
        Ordering::Release,
    );
}
pub(crate) unsafe fn view(
    _device: *mut core::ffi::c_void,
    resource: *mut core::ffi::c_void,
    handle: usize,
) {
    if !crate::native_clock::probe_enabled() {
        return;
    }
    if let Ok(mut state) = state().lock() {
        unsafe { state.view(resource, handle) };
    }
}
pub(crate) unsafe fn bindings(
    count: u32,
    handles: *const D3D12_CPU_DESCRIPTOR_HANDLE,
    single: bool,
) -> [u64; 8] {
    let mut result = [0; 8];
    if count == 0 || count > 8 || handles.is_null() || !crate::native_clock::probe_enabled() {
        return result;
    }
    let stride = STRIDE.load(Ordering::Acquire);
    if stride == 0 {
        return result;
    }
    let Ok(state) = state().lock() else {
        return result;
    };
    for (i, result) in result.iter_mut().enumerate().take(count as usize) {
        let handle = if single {
            unsafe { (*handles).ptr }.checked_add(i * stride)
        } else {
            Some(unsafe { (*handles.add(i)).ptr })
        };
        *result = handle
            .and_then(|h| state.views.get(&h).copied())
            .unwrap_or(0);
    }
    result
}
pub(crate) fn target(handle: usize) -> u64 {
    state()
        .lock()
        .ok()
        .and_then(|s| s.views.get(&handle).copied())
        .unwrap_or(0)
}
pub(crate) fn clear(id: u64) {
    if let Ok(mut state) = state().lock()
        && let Some(target) = state.targets.get_mut(&id)
    {
        target.last = None;
    }
}
pub(crate) fn invalidate() {
    if let Ok(mut state) = state().lock() {
        for t in state.targets.values_mut() {
            t.last = None;
        }
    }
}
pub(crate) fn invalidate_views() {
    if let Ok(mut state) = state().lock() {
        state.invalidate_views();
    }
}
pub(crate) fn copy_views(dst: Option<Vec<usize>>, src: Option<Vec<usize>>) {
    if let Ok(mut state) = state().lock() {
        state.copy_views(dst, src);
    }
}
pub(crate) fn draw(queue: usize, targets: [u64; 8], scope: u64, width: u32, height: u32) {
    if !targets.iter().any(|id| *id != 0) {
        return;
    }
    if let Ok(mut state) = state().lock() {
        state.draw(queue, targets, scope, width, height);
    }
}
/// Called once per real present, even when no capture is requested. Consuming the
/// witness prevents a recycled back buffer from borrowing a previous frame's scope.
pub(crate) fn presented(buffer: &ID3D12Resource, queue: usize, wanted: bool) -> Option<Value> {
    if !crate::native_clock::probe_enabled() {
        return None;
    }
    let (scope, age) = state().lock().ok()?.take(identity(buffer)?, queue)?;
    if !wanted {
        return None;
    }
    let mut sample = crate::native_gpu_constants::sample(scope)?;
    sample["presentTarget"] = json!({"sameResourceIdentity":true,"sameQueue":true,"consumedOnce":true,
        "submissionAgeMs":age,"sceneCompletenessVerified":false});
    Some(sample)
}
pub(crate) fn status() -> Value {
    let Ok(s) = state().try_lock() else {
        return Value::Null;
    };
    json!({"targets":s.targets.len(),"retainedGpuResources":0,
        "views":s.views.len(),"worldDraws":s.recorded,"matchedPresents":s.matched,"omittedTargets":s.omitted,
        "descriptorInvalidations":s.descriptor_invalidations,"copiedViewDescriptors":s.descriptor_copies,
        "observerOnly":true,"sceneCompletenessVerified":false})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn presented_target_uses_canonical_resource_identity_exact_queue_and_one_fresh_draw()
    -> Result<(), Box<dyn std::error::Error>> {
        use hudhook::windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0;
        use hudhook::windows::Win32::Graphics::Dxgi::Common::{
            DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SAMPLE_DESC,
        };
        use hudhook::windows::Win32::Graphics::Dxgi::{
            CreateDXGIFactory1, IDXGIAdapter, IDXGIFactory4,
        };
        let _gpu = crate::depth::GPU_TEST.lock().unwrap();
        unsafe {
            let factory: IDXGIFactory4 = CreateDXGIFactory1()?;
            let adapter: IDXGIAdapter = factory.EnumWarpAdapter()?;
            let mut device = None;
            D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_11_0, &mut device)?;
            let device: ID3D12Device = device.ok_or("missing WARP device")?;
            let mut buffer: Option<ID3D12Resource> = None;
            device.CreateCommittedResource(
                &D3D12_HEAP_PROPERTIES {
                    Type: D3D12_HEAP_TYPE_DEFAULT,
                    ..Default::default()
                },
                D3D12_HEAP_FLAG_NONE,
                &D3D12_RESOURCE_DESC {
                    Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
                    Width: 64,
                    Height: 64,
                    DepthOrArraySize: 1,
                    MipLevels: 1,
                    Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    Flags: D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET,
                    ..Default::default()
                },
                D3D12_RESOURCE_STATE_RENDER_TARGET,
                None,
                &mut buffer,
            )?;
            let buffer = buffer.ok_or("missing target")?;
            let mut state = State::default();
            state.view(buffer.as_raw(), 100);
            let identity = identity(&buffer).ok_or("resource lifetime tag missing")?;
            let alias = buffer.cast::<ID3D12Resource1>()?.cast::<ID3D12Resource>()?;
            assert_eq!(super::identity(&alias), Some(identity));
            let id = state.views[&100];
            let mut targets = [0; 8];
            targets[0] = id;
            state.draw(7, targets, 40, 64, 64);
            assert_eq!(state.take(identity + 1, 7), None);
            assert_eq!(state.take(identity, 8), None);
            assert_eq!(state.take(identity, 7), None);
            state.draw(7, targets, 41, 63, 64);
            assert_eq!(state.take(identity, 7), None);
            state.draw(7, targets, 42, 64, 64);
            assert_eq!(state.take(identity, 7).map(|x| x.0), Some(42));
            assert_eq!(state.take(identity, 7), None);
        }
        Ok(())
    }
}
