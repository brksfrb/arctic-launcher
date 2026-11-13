//! Pinned native framebuffer-context correlation, observed inside its actual
//! OMSetRenderTargets callback. No target lease, world scope or draw is invented.
use crate::native_scene_layout::framebuffer as layout;
use crate::{render_bridge::FrameKey, world_hooks::read};
use hudhook::{
    mh::{MH_CreateHook, MH_EnableHook, MH_STATUS},
    windows::{
        Win32::Graphics::Direct3D12::{
            D3D12_CPU_DESCRIPTOR_HANDLE, D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
            D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_BARRIER_FLAG_NONE,
            D3D12_RESOURCE_BARRIER_FLAGS, D3D12_RESOURCE_STATES, ID3D12Device,
            ID3D12GraphicsCommandList, ID3D12Resource,
        },
        core::Interface,
    },
};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

#[path = "native_framebuffer_profile.rs"]
mod profile;
static INSTALL: OnceLock<Result<(), String>> = OnceLock::new();
static INSTALLED: AtomicBool = AtomicBool::new(false);
// The prologue reads both integer registers as DWORDs. Forward all those bits,
// including unspecified high bits from native word/byte callsites. Only the
// requested handle's low word and flag's low byte are interpreted here.
type Set = unsafe extern "system" fn(usize, usize, u32, u32);
static ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static CALLS: AtomicU64 = AtomicU64::new(0);
static OBSERVED: AtomicU64 = AtomicU64::new(0);
static MATCHED: AtomicU64 = AtomicU64::new(0);
static REJECTED: AtomicU64 = AtomicU64::new(0);
static UNBOUND: AtomicU64 = AtomicU64::new(0);
static RESOURCE_MATCHES: AtomicU64 = AtomicU64::new(0);
static BOUND_HANDOFFS: AtomicU64 = AtomicU64::new(0);
static LAST: Mutex<Option<(Instant, Value)>> = Mutex::new(None);
struct Binding {
    header: [u8; layout::HEADER_BYTES],
    list: usize,
    colors: [usize; 8],
    count: u32,
    stride: usize,
    depth: Option<usize>,
    actual_list: ID3D12GraphicsCommandList,
    epoch: crate::native_bound_surfaces::Epoch,
}
struct Request {
    context: usize,
    renderer: usize,
    handle: u16,
    flag: u8,
    frame: FrameKey,
    binding: Option<Binding>,
    invalid: bool,
    resources_checked: bool,
    resources_invalid: bool,
    resources: Option<crate::native_framebuffer_resources::Attachments>,
}
thread_local! {
    static ACTIVE:RefCell<Vec<Option<Request>>>=const {RefCell::new(Vec::new())};
    static SKIPPED:Cell<usize>=const {Cell::new(0)};
}
struct Guard {
    pushed: bool,
    finished: bool,
}
fn word(bytes: &[u8], at: usize) -> Option<usize> {
    usize::try_from(u64::from_le_bytes(
        bytes.get(at..at.checked_add(8)?)?.try_into().ok()?,
    ))
    .ok()
}
fn bytes<const N: usize>(address: usize) -> Option<[u8; N]> {
    let mut value = [0; N];
    read(address, &mut value).ok()?;
    Some(value)
}
fn target_count(renderer: usize, handle: u16) -> Option<u32> {
    if handle == u16::MAX {
        return Some(1);
    }
    let at = renderer
        .checked_add(layout::FRAMEBUFFERS)?
        .checked_add(usize::from(handle).checked_mul(layout::FRAMEBUFFER_STRIDE)?)?
        .checked_add(layout::FRAMEBUFFER_COUNT)?;
    Some(u32::from(bytes::<1>(at)?[0]))
}
fn descriptor_stride(list: usize) -> Option<usize> {
    // The public callback supplies this live command list. Borrow it only for
    // its device query; no COM address is retained as an owning lease.
    let raw = list as *mut core::ffi::c_void;
    let object = unsafe { ID3D12GraphicsCommandList::from_raw_borrowed(&raw) }?;
    let mut device: Option<ID3D12Device> = None;
    unsafe { object.GetDevice(&mut device) }.ok()?;
    let stride = unsafe { device?.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV) }
        as usize;
    (stride != 0).then_some(stride)
}
fn enter(context: usize, renderer: usize, handle: u32, flag: u32) -> Guard {
    let request = if crate::native_body::enabled() || crate::native_clock::probe_enabled() {
        crate::native_batch::framebuffer_frame(renderer).map(|frame| Request {
            context,
            renderer,
            handle: handle as u16,
            flag: flag as u8,
            frame,
            binding: None,
            invalid: false,
            resources_checked: false,
            resources_invalid: false,
            resources: None,
        })
    } else {
        None
    };
    let pushed = ACTIVE.with(|active| {
        let Ok(mut active) = active.try_borrow_mut() else {
            return false;
        };
        if active.len() >= 8 {
            return false;
        }
        active.push(request);
        true
    });
    if !pushed {
        SKIPPED.with(|depth| depth.set(depth.get() + 1));
    }
    Guard {
        pushed,
        finished: false,
    }
}
impl Guard {
    fn pop(&mut self) -> Option<Request> {
        self.finished = true;
        if !self.pushed {
            SKIPPED.with(|depth| depth.set(depth.get().saturating_sub(1)));
            return None;
        }
        ACTIVE
            .try_with(|active| active.try_borrow_mut().ok()?.pop().flatten())
            .ok()
            .flatten()
    }
    fn finish(mut self) {
        let Some(request) = self.pop() else {
            return;
        };
        if request.invalid {
            REJECTED.fetch_add(1, Ordering::Relaxed);
            return;
        }
        let Some(binding) = request.binding else {
            UNBOUND.fetch_add(1, Ordering::Relaxed);
            return;
        };
        let stable = (|| {
            let header = bytes::<{ layout::HEADER_BYTES }>(request.context)?;
            let list = word(
                &bytes::<8>(request.context.checked_add(layout::CONTEXT_LIST)?)?,
                0,
            )?;
            let handle = u16::from_le_bytes(bytes::<2>(
                request.context.checked_add(layout::CONTEXT_HANDLE)?,
            )?);
            let flag = bytes::<1>(request.context.checked_add(layout::CONTEXT_FLAG)?)?[0];
            if header != binding.header
                || list != binding.list
                || handle != request.handle
                || flag != request.flag
                || crate::native_batch::framebuffer_frame(request.renderer) != Some(request.frame)
                || target_count(request.renderer, request.handle) != Some(binding.count)
            {
                return None;
            }
            let mut actual = [0usize; 8];
            if binding.count > 0 {
                let start = word(&bytes::<8>(word(&header, layout::CONTEXT_COLORS)?)?, 0)?;
                for (i, value) in actual.iter_mut().enumerate().take(binding.count as usize) {
                    *value = start.checked_add(i.checked_mul(binding.stride)?)?;
                }
            }
            let depth = if word(&header, layout::CONTEXT_DEPTH)? == 0 {
                None
            } else {
                Some(word(
                    &bytes::<8>(word(&header, layout::CONTEXT_DEPTH)?)?,
                    0,
                )?)
            };
            (actual == binding.colors && depth == binding.depth).then_some(())
        })()
        .is_some();
        if !stable {
            REJECTED.fetch_add(1, Ordering::Relaxed);
            return;
        }
        MATCHED.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut last) = LAST.try_lock() {
            let observed = if request.resources_invalid {
                0
            } else {
                request
                    .resources
                    .as_ref()
                    .map_or(0, |resources| resources.observed())
            };
            let upgrade = observed > 0
                && last.as_ref().is_none_or(|(_, value)| {
                    value["resources"]["observedResources"]
                        .as_u64()
                        .unwrap_or(0)
                        == 0
                });
            if upgrade
                || last
                    .as_ref()
                    .is_none_or(|(at, _)| at.elapsed() >= Duration::from_millis(250))
            {
                let resources = if request.resources_invalid {
                    json!({"registryKnown":false,"recordingStateCohortChanged":true,"observedResources":0,"targetLeaseVerified":false})
                } else {
                    request.resources.as_ref().map_or_else(
                        || {
                            json!({"registryKnown":false,"observedResources":0,
                    "targetLeaseVerified":false})
                        },
                        |resources| resources.summary(),
                    )
                };
                *last = Some((
                    Instant::now(),
                    json!({"backendFrameNumber":request.frame.number,
                    "framebufferHandle":request.handle,"renderTargets":binding.count,"hasDepth":binding.depth.is_some(),
                    "nativeContextAndArgumentsMatched":true,"nativeHeaderStable":true,
                    "resources":resources,"worldScopeAuthorized":false,"targetLeaseVerified":false}),
                ));
            }
        }
        if !request.resources_invalid
            && let Some(resources) = request.resources
            && crate::depth_tracker::framebuffer_binding(
                &binding.actual_list,
                binding.epoch,
                request.frame,
                resources,
            )
        {
            BOUND_HANDOFFS.fetch_add(1, Ordering::Relaxed);
        }
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.pop();
        }
    }
}

/// Correlate resource pointers only from a real public transition callback in
/// this native routine. Native texture pointers themselves are never invoked.
pub(crate) fn unknown_resource_operation(list: usize) {
    if SKIPPED.try_with(Cell::get).unwrap_or(1) != 0 {
        return;
    }
    let _ = ACTIVE.try_with(|active| {
        let Ok(mut active) = active.try_borrow_mut() else {
            return;
        };
        let Some(Some(request)) = active.last_mut() else {
            return;
        };
        let actual_list = request
            .context
            .checked_add(layout::CONTEXT_LIST)
            .and_then(bytes::<8>)
            .and_then(|value| word(&value, 0));
        if actual_list == Some(list) {
            request.resources_invalid = true;
        }
    });
}
/// Only whole, unsplit legacy transitions can provide this scoped witness.
pub(crate) fn transition(
    list: usize,
    resource: &ID3D12Resource,
    before: D3D12_RESOURCE_STATES,
    after: D3D12_RESOURCE_STATES,
    subresource: u32,
    flags: D3D12_RESOURCE_BARRIER_FLAGS,
) {
    if SKIPPED.try_with(Cell::get).unwrap_or(1) != 0 {
        return;
    }
    let _ = ACTIVE.try_with(|active| {
        let Ok(mut active) = active.try_borrow_mut() else {
            return;
        };
        let Some(Some(request)) = active.last_mut() else {
            return;
        };
        let actual_list = request
            .context
            .checked_add(layout::CONTEXT_LIST)
            .and_then(bytes::<8>)
            .and_then(|value| word(&value, 0));
        if request.invalid
            || request.resources_invalid
            || actual_list != Some(list)
            || crate::native_batch::framebuffer_frame(request.renderer) != Some(request.frame)
        {
            return;
        }
        if request.binding.is_some()
            || flags != D3D12_RESOURCE_BARRIER_FLAG_NONE
            || subresource != D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES
        {
            request.resources_invalid = true;
            return;
        }
        if !request.resources_checked {
            request.resources_checked = true;
            request.resources = crate::native_framebuffer_resources::Attachments::copy(
                request.context,
                request.renderer,
                request.handle,
            );
        }
        if let Some(resources) = request.resources.as_mut() {
            RESOURCE_MATCHES.fetch_add(
                resources.transition(resource, before, after) as u64,
                Ordering::Relaxed,
            );
        }
    });
}

/// Called by the native API observer, before forwarding its actual binding.
pub(crate) fn binding(
    list: usize,
    count: u32,
    rtv: *const D3D12_CPU_DESCRIPTOR_HANDLE,
    single: bool,
    dsv: *const D3D12_CPU_DESCRIPTOR_HANDLE,
    epoch: Option<crate::native_bound_surfaces::Epoch>,
) {
    if SKIPPED.try_with(Cell::get).unwrap_or(1) != 0 {
        return;
    }
    let _ = ACTIVE.try_with(|active| {
        let Ok(mut active) = active.try_borrow_mut() else {
            return;
        };
        let Some(Some(request)) = active.last_mut() else {
            return;
        };
        OBSERVED.fetch_add(1, Ordering::Relaxed);
        if request.binding.is_some() || request.invalid {
            request.invalid = true;
            return;
        }
        let copied = (|| {
            let epoch = epoch?;
            if count > 8
                || !single
                || crate::native_batch::framebuffer_frame(request.renderer) != Some(request.frame)
            {
                return None;
            }
            let header = bytes::<{ layout::HEADER_BYTES }>(request.context)?;
            if word(
                &bytes::<8>(request.context.checked_add(layout::CONTEXT_LIST)?)?,
                0,
            )? != list
                || word(&header, layout::CONTEXT_COLORS)? != rtv as usize
                || word(&header, layout::CONTEXT_DEPTH)? != dsv as usize
            {
                return None;
            }
            let expected = target_count(request.renderer, request.handle)?;
            if expected != count {
                return None;
            }
            let mut colors = [0usize; 8];
            let stride = if count > 0 {
                descriptor_stride(list)?
            } else {
                0
            };
            if count > 0 {
                // single=true means one base handle, even with multiple RTVs.
                let start = word(&bytes::<8>(rtv as usize)?, 0)?;
                if start == 0 {
                    return None;
                }
                for (i, value) in colors.iter_mut().enumerate().take(count as usize) {
                    *value = start.checked_add(i.checked_mul(stride)?)?;
                }
            }
            let depth = if dsv.is_null() {
                None
            } else {
                let value = word(&bytes::<8>(dsv as usize)?, 0)?;
                if value == 0 {
                    return None;
                }
                Some(value)
            };
            let raw = list as *mut core::ffi::c_void;
            let actual_list =
                unsafe { ID3D12GraphicsCommandList::from_raw_borrowed(&raw) }?.clone();
            Some(Binding {
                header,
                list,
                colors,
                count,
                stride,
                depth,
                actual_list,
                epoch,
            })
        })();
        match copied {
            Some(binding) => {
                if !request.resources_invalid
                    && let Some(resources) = request.resources.as_mut()
                {
                    resources.binding(
                        list,
                        &binding.colors[..binding.count as usize],
                        binding.depth,
                    );
                }
                request.binding = Some(binding);
            }
            None => request.invalid = true,
        }
    });
}
unsafe extern "system" fn set(context: usize, renderer: usize, handle: u32, flag: u32) {
    CALLS.fetch_add(1, Ordering::Relaxed);
    if !INSTALLED.load(Ordering::Acquire) {
        let original: Set = unsafe { std::mem::transmute(ORIGINAL.load(Ordering::Acquire)) };
        unsafe { original(context, renderer, handle, flag) };
        return;
    }
    let guard = std::panic::catch_unwind(|| enter(context, renderer, handle, flag)).ok();
    let original: Set = unsafe { std::mem::transmute(ORIGINAL.load(Ordering::Acquire)) };
    unsafe {
        original(context, renderer, handle, flag);
    }
    if let Some(guard) = guard {
        let _ = std::panic::catch_unwind(|| guard.finish());
    }
}
/// Admit all immutable framebuffer/texture reader bytes before any detour.
pub(crate) fn preflight(base: usize) -> Result<(), String> {
    profile::binding(base).map(|_| ())
}
pub(crate) fn reader_ready() -> bool {
    profile::current().is_some()
}
pub(crate) unsafe fn install(base: usize) -> Result<(), String> {
    let binding = profile::binding(base)?;
    INSTALL
        .get_or_init(|| {
            let target = base
                .checked_add(binding.rva)
                .ok_or("framebuffer address overflow")?;
            let mut original = std::ptr::null_mut();
            let result =
                unsafe { MH_CreateHook(target as _, set as *const () as _, &mut original) };
            if result != MH_STATUS::MH_OK {
                return Err(format!("create native framebuffer observer: {result:?}"));
            }
            ORIGINAL.store(original as usize, Ordering::Release);
            let result = unsafe { MH_EnableHook(target as _) };
            if result != MH_STATUS::MH_OK {
                return Err(format!("enable native framebuffer observer: {result:?}"));
            }
            INSTALLED.store(true, Ordering::Release);
            Ok(())
        })
        .clone()
}
pub(crate) fn status() -> Value {
    let sample = LAST
        .try_lock()
        .ok()
        .and_then(|last| last.as_ref().map(|(_, value)| value.clone()));
    json!({"installed":INSTALLED.load(Ordering::Acquire),"hookRva":profile::current().map(|binding|binding.rva),
        "readerAdmitted":reader_ready(),"exactFamilyFramebufferSourceImplemented":true,
        "calls":CALLS.load(Ordering::Relaxed),"nativeBindings":OBSERVED.load(Ordering::Relaxed),
        "matchedContexts":MATCHED.load(Ordering::Relaxed),"rejected":REJECTED.load(Ordering::Relaxed),
        "matchedTextureResourceJoins":RESOURCE_MATCHES.load(Ordering::Relaxed),
        "validatedBindingHandoffs":BOUND_HANDOFFS.load(Ordering::Relaxed),
        "callsWithoutBinding":UNBOUND.load(Ordering::Relaxed),"sample":sample,"observerOnly":true,
        "targetLeaseVerified":false,"gpuRenderingVerified":false})
}
