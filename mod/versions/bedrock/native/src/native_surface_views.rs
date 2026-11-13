//! Bounded RTV/DSV argument metadata. Entries own no GPU objects or raw object
//! pointers. Pending writes hide old metadata until the public API returns.
use hudhook::windows::{
    Win32::Graphics::{Direct3D12::*, Dxgi::Common::*},
    core::{GUID, Interface},
};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    ffi::c_void,
    sync::{Mutex, OnceLock},
};

const TAG: GUID = GUID::from_u128(0xe2876329_830a_45a2_8ba7_116a51199ae6);
const MAX_VIEWS: usize = 4096;
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Kind {
    Color,
    Depth,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Key {
    device: u64,
    kind: Kind,
    handle: usize,
}
#[derive(Clone, Copy, PartialEq)]
struct View {
    resource: u64,
    desc: D3D12_RESOURCE_DESC,
    format: i32,
    mip: u32,
    plane: u32,
    multisampled: bool,
    flags: i32,
    explicit: bool,
}
#[derive(Clone, Copy)]
struct Entry {
    revision: u64,
    value: Option<View>,
}
#[derive(Default)]
struct State {
    serial: u64,
    exhausted: bool,
    views: HashMap<Key, Entry>,
}
static STATE: OnceLock<Mutex<State>> = OnceLock::new();
fn state() -> &'static Mutex<State> {
    STATE.get_or_init(Mutex::default)
}
impl State {
    fn next(&mut self) -> Option<u64> {
        if self.exhausted {
            return None;
        }
        match self.serial.checked_add(1) {
            Some(value) => {
                self.serial = value;
                Some(value)
            }
            None => {
                self.exhausted = true;
                self.views.clear();
                None
            }
        }
    }
    fn identity(&mut self, object: &ID3D12Object, create: bool) -> Option<u64> {
        let mut bytes = [0u8; 8];
        let mut size = 8;
        if unsafe { object.GetPrivateData(&TAG, &mut size, Some(bytes.as_mut_ptr() as _)) }.is_ok()
            && size == 8
            && u64::from_le_bytes(bytes) != 0
        {
            return Some(u64::from_le_bytes(bytes));
        }
        if !create {
            return None;
        }
        let id = self.next()?;
        let bytes = id.to_le_bytes();
        unsafe { object.SetPrivateData(&TAG, 8, Some(bytes.as_ptr() as _)) }.ok()?;
        Some(id)
    }
    fn pending(&mut self, device: u64, kind: Kind, handles: &[usize]) -> Option<Vec<(Key, u64)>> {
        if handles.len() > 256 || handles.contains(&0) {
            self.views.clear();
            return None;
        }
        // Duplicate destinations are ambiguous; do not publish a guessed winner.
        let unique: HashSet<_> = handles.iter().copied().collect();
        if unique.len() != handles.len() {
            self.views.clear();
            return None;
        }
        if self.views.len().checked_add(handles.len())? > MAX_VIEWS {
            self.views.clear();
        }
        let mut writes = Vec::with_capacity(handles.len());
        for &handle in handles {
            let key = Key {
                device,
                kind,
                handle,
            };
            let revision = self.next()?;
            self.views.insert(
                key,
                Entry {
                    revision,
                    value: None,
                },
            );
            writes.push((key, revision));
        }
        Some(writes)
    }
}
pub(crate) struct Write {
    writes: Vec<(Key, u64)>,
    values: Vec<Option<View>>,
    sources: Vec<(Key, u64)>,
}
impl Write {
    /// Publish only if neither destination nor any copied source was rewritten
    /// during the real driver call. Unknown/pending sources stay unknown.
    pub(crate) fn finish(self) {
        let Ok(mut state) = state().lock() else {
            return;
        };
        if !self.sources.iter().all(|(key, revision)| {
            state
                .views
                .get(key)
                .is_some_and(|entry| entry.revision == *revision)
        }) {
            return;
        }
        for ((key, revision), value) in self.writes.into_iter().zip(self.values) {
            if let Some(entry) = state
                .views
                .get_mut(&key)
                .filter(|entry| entry.revision == revision)
            {
                entry.value = value;
            }
        }
    }
}
fn enabled() -> bool {
    crate::native_body::enabled() || crate::native_clock::probe_enabled()
}
pub(crate) fn invalidate() {
    if let Ok(mut state) = state().lock() {
        state.views.clear();
    }
}
pub(crate) fn device_identity(device: &ID3D12Device) -> Option<u64> {
    let object = device.cast::<ID3D12Object>().ok()?;
    state().lock().ok()?.identity(&object, false)
}
fn supported(desc: &D3D12_RESOURCE_DESC, kind: Kind) -> bool {
    let required = match kind {
        Kind::Color => D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET.0,
        Kind::Depth => D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL.0,
    };
    desc.Dimension == D3D12_RESOURCE_DIMENSION_TEXTURE2D
        && desc.DepthOrArraySize == 1
        && desc.Width > 0
        && desc.Width <= 16384
        && desc.Height > 0
        && desc.Height <= 16384
        && desc.MipLevels > 0
        && desc.SampleDesc.Count > 0
        && (desc.SampleDesc.Count == 1 || desc.MipLevels == 1)
        && desc.Flags.0 & required != 0
}
fn default_format(format: DXGI_FORMAT, kind: Kind) -> bool {
    // A null descriptor cannot supply a typed view for a typeless resource.
    match kind {
        Kind::Color => matches!(
            format,
            DXGI_FORMAT_R8G8B8A8_UNORM
                | DXGI_FORMAT_R8G8B8A8_UNORM_SRGB
                | DXGI_FORMAT_B8G8R8A8_UNORM
                | DXGI_FORMAT_B8G8R8A8_UNORM_SRGB
                | DXGI_FORMAT_R16G16B16A16_FLOAT
                | DXGI_FORMAT_R10G10B10A2_UNORM
        ),
        Kind::Depth => matches!(
            format,
            DXGI_FORMAT_D32_FLOAT
                | DXGI_FORMAT_D16_UNORM
                | DXGI_FORMAT_D24_UNORM_S8_UINT
                | DXGI_FORMAT_D32_FLOAT_S8X24_UINT
        ),
    }
}
unsafe fn begin_create(
    device: *mut c_void,
    resource: *mut c_void,
    handle: usize,
    kind: Kind,
    color: *const D3D12_RENDER_TARGET_VIEW_DESC,
    depth: *const D3D12_DEPTH_STENCIL_VIEW_DESC,
) -> Option<Write> {
    if !enabled() {
        invalidate();
        return None;
    }
    let mut state = state().lock().ok()?;
    let Some(device) = (unsafe { ID3D12Device::from_raw_borrowed(&device) }) else {
        state.views.clear();
        return None;
    };
    let device_id = match state.identity(&device.cast::<ID3D12Object>().ok()?, true) {
        Some(value) => value,
        None => {
            state.views.clear();
            return None;
        }
    };
    // Invalidate the old handle even for null or unsupported resource/view args.
    let writes = state.pending(device_id, kind, &[handle])?;
    let value = (|| {
        let resource = unsafe { ID3D12Resource::from_raw_borrowed(&resource) }?;
        let mut owner: Option<ID3D12Device> = None;
        unsafe { resource.GetDevice(&mut owner) }.ok()?;
        if owner.as_ref() != Some(device) {
            return None;
        }
        let desc = unsafe { resource.GetDesc() };
        if !supported(&desc, kind) {
            return None;
        }
        let resource_id = state.identity(&resource.cast::<ID3D12Object>().ok()?, true)?;
        let mut view = View {
            resource: resource_id,
            desc,
            format: desc.Format.0,
            mip: 0,
            plane: 0,
            multisampled: desc.SampleDesc.Count > 1,
            flags: 0,
            explicit: false,
        };
        match kind {
            Kind::Color if !color.is_null() => {
                let arg = unsafe { &*color };
                view.format = arg.Format.0;
                view.explicit = true;
                if arg.ViewDimension == D3D12_RTV_DIMENSION_TEXTURE2D && desc.SampleDesc.Count == 1
                {
                    let arg = unsafe { arg.Anonymous.Texture2D };
                    view.mip = arg.MipSlice;
                    view.plane = arg.PlaneSlice;
                } else if arg.ViewDimension != D3D12_RTV_DIMENSION_TEXTURE2DMS
                    || desc.SampleDesc.Count == 1
                {
                    return None;
                }
            }
            Kind::Depth if !depth.is_null() => {
                let arg = unsafe { &*depth };
                view.format = arg.Format.0;
                view.flags = arg.Flags.0;
                view.explicit = true;
                if arg.ViewDimension == D3D12_DSV_DIMENSION_TEXTURE2D && desc.SampleDesc.Count == 1
                {
                    view.mip = unsafe { arg.Anonymous.Texture2D.MipSlice };
                } else if arg.ViewDimension != D3D12_DSV_DIMENSION_TEXTURE2DMS
                    || desc.SampleDesc.Count == 1
                {
                    return None;
                }
            }
            _ => {
                if !default_format(desc.Format, kind) {
                    return None;
                }
            }
        }
        // Retain supplied typed-format facts, without claiming driver validation.
        if view.format == DXGI_FORMAT_UNKNOWN.0
            || view.mip >= u32::from(desc.MipLevels)
            || view.plane > 1
        {
            return None;
        }
        Some(view)
    })();
    Some(Write {
        writes,
        values: vec![value],
        sources: Vec::new(),
    })
}
pub(crate) unsafe fn color(
    device: *mut c_void,
    resource: *mut c_void,
    desc: *const D3D12_RENDER_TARGET_VIEW_DESC,
    handle: usize,
) -> Option<Write> {
    unsafe {
        begin_create(
            device,
            resource,
            handle,
            Kind::Color,
            desc,
            std::ptr::null(),
        )
    }
}
pub(crate) unsafe fn depth(
    device: *mut c_void,
    resource: *mut c_void,
    desc: *const D3D12_DEPTH_STENCIL_VIEW_DESC,
    handle: usize,
) -> Option<Write> {
    unsafe {
        begin_create(
            device,
            resource,
            handle,
            Kind::Depth,
            std::ptr::null(),
            desc,
        )
    }
}
pub(crate) fn copy(
    device: &ID3D12Device,
    kind: Kind,
    dst: Option<Vec<usize>>,
    src: Option<Vec<usize>>,
) -> Option<Write> {
    if !enabled() {
        invalidate();
        return None;
    }
    let mut state = state().lock().ok()?;
    let Some((dst, src)) = dst.zip(src).filter(|(dst, src)| {
        dst.len() == src.len()
            && dst.len() <= 256
            && dst.iter().chain(src).all(|handle| *handle != 0)
    }) else {
        state.views.clear();
        return None;
    };
    let Some(device) = state.identity(&device.cast::<ID3D12Object>().ok()?, true) else {
        state.views.clear();
        return None;
    };
    let mut sources = Vec::new();
    let values = src
        .iter()
        .map(|&handle| {
            let key = Key {
                device,
                kind,
                handle,
            };
            let entry = state.views.get(&key)?;
            sources.push((key, entry.revision));
            entry.value
        })
        .collect();
    // D3D12 source/destination ranges may not overlap. Reject rather than
    // publishing metadata for invalid overlapping or duplicate destinations.
    let source_handles: HashSet<_> = src.iter().copied().collect();
    if dst.iter().any(|handle| source_handles.contains(handle)) {
        state.views.clear();
        return None;
    }
    let writes = state.pending(device, kind, &dst)?;
    Some(Write {
        writes,
        values,
        sources,
    })
}
pub(crate) struct Snapshot {
    key: Key,
    revision: u64,
    view: View,
}
#[derive(Clone, Copy)]
pub(crate) struct Surface {
    pub kind: Kind,
    pub format: DXGI_FORMAT,
    pub mip: u32,
    pub plane: u32,
    pub flags: D3D12_DSV_FLAGS,
}
/// Check the whole descriptor cohort under one metadata lock. Individual
/// successful lookups from different instants cannot authorize the aggregate.
pub(crate) fn cohort_stable<'a>(views: impl Iterator<Item = &'a Snapshot>) -> bool {
    let Ok(state) = state().lock() else {
        return false;
    };
    views.into_iter().all(|view| view.unchanged(&state))
}
impl Snapshot {
    pub(crate) fn surface(&self) -> Option<Surface> {
        self.stable().then_some(Surface {
            kind: self.key.kind,
            format: DXGI_FORMAT(self.view.format),
            mip: self.view.mip,
            plane: self.view.plane,
            flags: D3D12_DSV_FLAGS(self.view.flags),
        })
    }
    /// Classify a clear descriptor on the same actual device without invoking
    /// any guessed resource pointer. Unknown/overwritten metadata stays unknown.
    pub(crate) fn aliases(&self, kind: Kind, handle: usize) -> Option<bool> {
        let state = state().lock().ok()?;
        let key = Key {
            device: self.key.device,
            kind,
            handle,
        };
        Some(state.views.get(&key)?.value?.resource == self.view.resource)
    }
    fn unchanged(&self, state: &State) -> bool {
        state
            .views
            .get(&self.key)
            .is_some_and(|entry| entry.revision == self.revision && entry.value == Some(self.view))
    }
    pub(crate) fn matches(
        device: u64,
        kind: Kind,
        handle: usize,
        resource: &ID3D12Resource,
    ) -> Option<Self> {
        let mut state = state().lock().ok()?;
        let key = Key {
            device,
            kind,
            handle,
        };
        let entry = *state.views.get(&key)?;
        let view = entry.value?;
        let actual = state.identity(&resource.cast::<ID3D12Object>().ok()?, false)?;
        if actual != view.resource || unsafe { resource.GetDesc() } != view.desc {
            return None;
        }
        Some(Self {
            key,
            revision: entry.revision,
            view,
        })
    }
    pub(crate) fn stable(&self) -> bool {
        state()
            .lock()
            .ok()
            .is_some_and(|state| self.unchanged(&state))
    }
    pub(crate) fn summary(&self) -> Value {
        let mip = self.view.mip;
        json!({"kind":match self.key.kind {Kind::Color=>"color",Kind::Depth=>"depth"},
            "format":self.view.format,"mip":mip,"plane":self.view.plane,"multisampled":self.view.multisampled,
            "width":self.view.desc.Width.checked_shr(mip).unwrap_or(0).max(1),
            "height":self.view.desc.Height.checked_shr(mip).unwrap_or(0).max(1),
            "depthStencilFlags":self.view.flags,"explicitDescriptor":self.view.explicit,
            "apiDescriptorResourceMatched":true,"descriptorWriteStable":self.stable(),"driverValidationVerified":false})
    }
}
