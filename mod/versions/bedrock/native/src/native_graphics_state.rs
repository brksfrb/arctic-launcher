//! State changed by avatar_material::record_ordered. This is not a target/world
//! lease. Missing history and opaque bundle/indirect/state-object changes reject
//! snapshots; no guessed default root arguments are restored.
use hudhook::windows::{
    Win32::{
        Foundation::RECT,
        Graphics::{
            Direct3D::{D3D_PRIMITIVE_TOPOLOGY, D3D_PRIMITIVE_TOPOLOGY_UNDEFINED},
            Direct3D12::*,
        },
    },
    core::{GUID, IUnknown, Interface},
};
use std::{
    ffi::c_void,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

const ROOT: GUID = GUID::from_u128(0xa09ccf24_1724_4a71_85a8_12d7391be1c2);
const LIST: GUID = GUID::from_u128(0x475571ee_dcfe_4b70_8467_8e9f08cef964);
const MAGIC: &[u8; 8] = b"ARCRS002";
const LEGACY_MAGIC: &[u8; 8] = b"ARCRS001";
const MAX_ROOT_RANGES: usize = 256;
const MAX_ROOT_BYTES: usize = 16 + 64 * 12 + MAX_ROOT_RANGES * 12;
const MAX_TABLE_DESCRIPTORS: u64 = 256;
static NEXT: AtomicU64 = AtomicU64::new(1);
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct ListToken(u64);
fn enabled() -> bool {
    crate::native_body::enabled() || crate::native_clock::probe_enabled()
}
fn list_identity(list: &ID3D12GraphicsCommandList) -> Option<u64> {
    let mut bytes = [0u8; 8];
    let mut size = 8;
    unsafe { list.GetPrivateData(&LIST, &mut size, Some(bytes.as_mut_ptr() as _)) }.ok()?;
    let identity = u64::from_le_bytes(bytes);
    (size == 8 && identity != 0).then_some(identity)
}
#[derive(Clone, Copy)]
struct TableRange {
    kind: D3D12_DESCRIPTOR_RANGE_TYPE,
    count: u32,
    offset: u32,
}
#[derive(Clone)]
struct Parameter {
    kind: D3D12_ROOT_PARAMETER_TYPE,
    words: u32,
    ranges: Option<Vec<TableRange>>,
}
struct RootLayout {
    parameters: Vec<Parameter>,
    flags: Option<u32>,
}
fn root_cost(parameters: &[Parameter]) -> Option<u32> {
    let mut cost = 0u32;
    for parameter in parameters {
        let words = match parameter.kind {
            D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE => 1,
            D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS if (1..=64).contains(&parameter.words) => {
                parameter.words
            }
            D3D12_ROOT_PARAMETER_TYPE_CBV
            | D3D12_ROOT_PARAMETER_TYPE_SRV
            | D3D12_ROOT_PARAMETER_TYPE_UAV => 2,
            _ => return None,
        };
        if parameter.kind != D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS && parameter.words != 0 {
            return None;
        }
        cost = cost.checked_add(words)?;
        if cost > 64 {
            return None;
        }
    }
    Some(cost)
}
/// Actual returned root object and actual serialized API argument, never an
/// inferred native pointer. Store a bounded scalar layout on that COM object.
pub(crate) unsafe fn tag_root(root: *mut c_void, blob: *const c_void, len: usize) {
    if !enabled() || blob.is_null() || !(1..=65536).contains(&len) {
        return;
    }
    let layout = (|| {
        let mut raw = std::ptr::null_mut();
        unsafe {
            D3D12CreateVersionedRootSignatureDeserializer(
                blob,
                len,
                &ID3D12VersionedRootSignatureDeserializer::IID,
                &mut raw,
            )
        }
        .ok()?;
        if raw.is_null() {
            return None;
        }
        let owner = unsafe { ID3D12VersionedRootSignatureDeserializer::from_raw(raw) };
        let desc =
            unsafe { owner.GetRootSignatureDescAtVersion(D3D_ROOT_SIGNATURE_VERSION_1_0) }.ok()?;
        let desc = unsafe { desc.as_ref() }?;
        if desc.Version != D3D_ROOT_SIGNATURE_VERSION_1_0 {
            return None;
        }
        let desc = unsafe { desc.Anonymous.Desc_1_0 };
        if desc.NumParameters > 64
            || desc.NumParameters > 0 && desc.pParameters.is_null()
            || desc.Flags.0 & D3D12_ROOT_SIGNATURE_FLAG_LOCAL_ROOT_SIGNATURE.0 != 0
        {
            return None;
        }
        let mut parameters = Vec::with_capacity(desc.NumParameters as usize);
        let mut remaining = MAX_ROOT_RANGES;
        for index in 0..desc.NumParameters as usize {
            let parameter = unsafe { &*desc.pParameters.add(index) };
            let ranges = if parameter.ParameterType == D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE {
                let table = unsafe { parameter.Anonymous.DescriptorTable };
                let count = table.NumDescriptorRanges as usize;
                if count > remaining || count > 0 && table.pDescriptorRanges.is_null() {
                    None
                } else {
                    remaining -= count;
                    Some(
                        (0..count)
                            .map(|index| {
                                let range = unsafe { &*table.pDescriptorRanges.add(index) };
                                TableRange {
                                    kind: range.RangeType,
                                    count: range.NumDescriptors,
                                    offset: range.OffsetInDescriptorsFromTableStart,
                                }
                            })
                            .collect(),
                    )
                }
            } else {
                None
            };
            parameters.push(Parameter {
                kind: parameter.ParameterType,
                words: if parameter.ParameterType == D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS {
                    unsafe { parameter.Anonymous.Constants.Num32BitValues }
                } else {
                    0
                },
                ranges,
            });
        }
        root_cost(&parameters)?;
        Some(RootLayout {
            parameters,
            flags: Some(desc.Flags.0 as u32),
        })
    })();
    let Some(object) = (unsafe { IUnknown::from_raw_borrowed(&root) }) else {
        return;
    };
    let Ok(root) = object.cast::<ID3D12RootSignature>() else {
        return;
    };
    // An unknown layout never inherits a previously attached known layout.
    let mut bytes = Vec::new();
    if let Some(RootLayout {
        parameters,
        flags: Some(flags),
    }) = layout
    {
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&(parameters.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&flags.to_le_bytes());
        for parameter in parameters {
            bytes.extend_from_slice(&parameter.kind.0.to_le_bytes());
            bytes.extend_from_slice(&parameter.words.to_le_bytes());
            bytes.extend_from_slice(
                &parameter
                    .ranges
                    .as_ref()
                    .map_or(u32::MAX, |ranges| ranges.len() as u32)
                    .to_le_bytes(),
            );
            if let Some(ranges) = parameter.ranges {
                for range in ranges {
                    bytes.extend_from_slice(&range.kind.0.to_le_bytes());
                    bytes.extend_from_slice(&range.count.to_le_bytes());
                    bytes.extend_from_slice(&range.offset.to_le_bytes());
                }
            }
        }
    }
    let _ = unsafe {
        root.SetPrivateData(
            &ROOT,
            bytes.len() as u32,
            if bytes.is_empty() {
                None
            } else {
                Some(bytes.as_ptr() as _)
            },
        )
    };
}
fn root_word(bytes: &[u8], at: &mut usize) -> Option<u32> {
    let end = (*at).checked_add(4)?;
    let value = u32::from_le_bytes(bytes.get(*at..end)?.try_into().ok()?);
    *at = end;
    Some(value)
}
fn layout(root: &ID3D12RootSignature) -> Option<RootLayout> {
    let mut bytes = [0u8; MAX_ROOT_BYTES];
    let mut size = MAX_ROOT_BYTES as u32;
    unsafe { root.GetPrivateData(&ROOT, &mut size, Some(bytes.as_mut_ptr() as _)) }.ok()?;
    if size < 12 || size as usize > MAX_ROOT_BYTES {
        return None;
    }
    let legacy = &bytes[..8] == LEGACY_MAGIC;
    if !legacy && &bytes[..8] != MAGIC {
        return None;
    }
    let bytes = &bytes[..size as usize];
    let mut at = 8;
    let count = root_word(bytes, &mut at)? as usize;
    if count > 64 {
        return None;
    }
    let flags = if legacy {
        None
    } else {
        Some(root_word(bytes, &mut at)?)
    };
    let mut parameters = Vec::with_capacity(count);
    let mut remaining = MAX_ROOT_RANGES;
    for _ in 0..count {
        let kind = D3D12_ROOT_PARAMETER_TYPE(root_word(bytes, &mut at)? as i32);
        let words = root_word(bytes, &mut at)?;
        let ranges = if legacy {
            None
        } else {
            let count = root_word(bytes, &mut at)?;
            if count == u32::MAX {
                None
            } else {
                let count = count as usize;
                if kind != D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE || count > remaining {
                    return None;
                }
                remaining -= count;
                let mut ranges = Vec::with_capacity(count);
                for _ in 0..count {
                    ranges.push(TableRange {
                        kind: D3D12_DESCRIPTOR_RANGE_TYPE(root_word(bytes, &mut at)? as i32),
                        count: root_word(bytes, &mut at)?,
                        offset: root_word(bytes, &mut at)?,
                    });
                }
                Some(ranges)
            }
        };
        parameters.push(Parameter {
            kind,
            words,
            ranges,
        });
    }
    if at != bytes.len() {
        return None;
    }
    root_cost(&parameters)?;
    Some(RootLayout { parameters, flags })
}
#[derive(Clone)]
enum Argument {
    Table(Option<D3D12_GPU_DESCRIPTOR_HANDLE>),
    Address(D3D12_ROOT_PARAMETER_TYPE, Option<u64>),
    Constants(Vec<Option<u32>>),
}
#[derive(Clone, Default)]
struct Stage {
    root: Option<ID3D12RootSignature>,
    known: bool,
    args: Vec<Argument>,
    parameters: Vec<Parameter>,
    flags: Option<u32>,
}

/// Declared descriptor slots resolved against an actual bound public heap.
/// Heap ownership does not freeze descriptor contents or own their resources.
#[derive(Clone)]
pub(crate) struct DescriptorSpan {
    pub heap: Arc<ID3D12DescriptorHeap>,
    pub kind: D3D12_DESCRIPTOR_RANGE_TYPE,
    pub first: u32,
    pub count: u32,
    pub increment: u32,
    pub cpu: usize,
    pub gpu: u64,
}
struct TableHeap {
    heap: Arc<ID3D12DescriptorHeap>,
    kind: D3D12_DESCRIPTOR_HEAP_TYPE,
    count: u32,
    increment: u32,
    cpu: usize,
    gpu: u64,
}
fn range_heap(kind: D3D12_DESCRIPTOR_RANGE_TYPE) -> Option<D3D12_DESCRIPTOR_HEAP_TYPE> {
    match kind {
        D3D12_DESCRIPTOR_RANGE_TYPE_CBV
        | D3D12_DESCRIPTOR_RANGE_TYPE_SRV
        | D3D12_DESCRIPTOR_RANGE_TYPE_UAV => Some(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV),
        D3D12_DESCRIPTOR_RANGE_TYPE_SAMPLER => Some(D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER),
        _ => None,
    }
}
fn table_heaps(
    list: &ID3D12GraphicsCommandList,
    heaps: &[ID3D12DescriptorHeap],
) -> Option<Vec<TableHeap>> {
    if heaps.len() > 2 {
        return None;
    }
    let mut device: Option<ID3D12Device> = None;
    unsafe { list.GetDevice(&mut device) }.ok()?;
    let device = device?;
    let identity = device.cast::<IUnknown>().ok()?;
    let mut copied: Vec<TableHeap> = Vec::new();
    for heap in heaps {
        let desc = unsafe { heap.GetDesc() };
        if !matches!(
            desc.Type,
            D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV | D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER
        ) || desc.Flags.0 & D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE.0 == 0
            || desc.NumDescriptors == 0
            || copied.iter().any(|old| old.kind == desc.Type)
        {
            return None;
        }
        let mut owner: Option<ID3D12Device> = None;
        unsafe { heap.GetDevice(&mut owner) }.ok()?;
        let owner = owner?.cast::<IUnknown>().ok()?;
        if owner.as_raw() != identity.as_raw() {
            return None;
        }
        let increment = unsafe { device.GetDescriptorHandleIncrementSize(desc.Type) };
        let cpu = unsafe { heap.GetCPUDescriptorHandleForHeapStart() }.ptr;
        let gpu = unsafe { heap.GetGPUDescriptorHandleForHeapStart() }.ptr;
        if increment == 0 || cpu == 0 || gpu == 0 {
            return None;
        }
        let bytes = u64::from(desc.NumDescriptors).checked_mul(u64::from(increment))?;
        cpu.checked_add(usize::try_from(bytes).ok()?)?;
        gpu.checked_add(bytes)?;
        copied.push(TableHeap {
            heap: Arc::new(heap.clone()),
            kind: desc.Type,
            count: desc.NumDescriptors,
            increment,
            cpu,
            gpu,
        });
    }
    Some(copied)
}
impl Stage {
    unsafe fn signature(&mut self, raw: *mut c_void) {
        let next = unsafe { ID3D12RootSignature::from_raw_borrowed(&raw) };
        if self.root.as_ref() == next {
            return;
        }
        self.root = next.cloned();
        self.args.clear();
        self.parameters.clear();
        self.flags = None;
        let layout = self.root.as_ref().and_then(layout);
        self.known = layout.is_some();
        if let Some(layout) = layout {
            self.flags = layout.flags;
            self.parameters = layout.parameters;
            self.args = self
                .parameters
                .iter()
                .map(|parameter| match parameter.kind {
                    D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE => Argument::Table(None),
                    D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS => {
                        Argument::Constants(vec![None; parameter.words as usize])
                    }
                    kind => Argument::Address(kind, None),
                })
                .collect();
        }
    }
    fn invalidate_tables(&mut self) {
        for arg in &mut self.args {
            if let Argument::Table(value) = arg {
                *value = None;
            }
        }
    }
    fn graphics_ready(&self) -> bool {
        self.root.is_some()
            && self.known
            && self.args.iter().all(|arg| match arg {
                Argument::Table(value) => value.is_some(),
                Argument::Address(_, value) => value.is_some(),
                Argument::Constants(values) => values.iter().all(Option::is_some),
            })
    }
    fn tables_ready(&self) -> bool {
        self.root.is_none()
            || self.known
                && self.args.iter().all(|arg| match arg {
                    Argument::Table(value) => value.is_some(),
                    _ => true,
                })
    }
    fn descriptor_spans(
        &self,
        heaps: &[TableHeap],
        spans: &mut Vec<DescriptorSpan>,
        total: &mut u64,
    ) -> Option<()> {
        if self.root.is_none() {
            return Some(());
        }
        if !self.known || self.parameters.len() != self.args.len() {
            return None;
        }
        let direct = (D3D12_ROOT_SIGNATURE_FLAG_CBV_SRV_UAV_HEAP_DIRECTLY_INDEXED.0
            | D3D12_ROOT_SIGNATURE_FLAG_SAMPLER_HEAP_DIRECTLY_INDEXED.0)
            as u32;
        // Legacy tags omit flags; direct indexing has no declared table extent.
        if self.flags? & direct != 0 {
            return None;
        }
        for (parameter, argument) in self.parameters.iter().zip(&self.args) {
            if parameter.kind != D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE {
                continue;
            }
            let ranges = parameter.ranges.as_ref()?;
            let Argument::Table(Some(handle)) = argument else {
                return None;
            };
            let Some(first_range) = ranges.first() else {
                continue;
            };
            let kind = range_heap(first_range.kind)?;
            if ranges
                .iter()
                .any(|range| range_heap(range.kind) != Some(kind))
            {
                return None;
            }
            let heap = heaps.iter().find(|heap| heap.kind == kind)?;
            let displacement = handle.ptr.checked_sub(heap.gpu)?;
            let increment = u64::from(heap.increment);
            if !displacement.is_multiple_of(increment) {
                return None;
            }
            let base = displacement / increment;
            if base >= u64::from(heap.count) {
                return None;
            }
            let mut cursor = 0;
            for range in ranges {
                let offset = if range.offset == D3D12_DESCRIPTOR_RANGE_OFFSET_APPEND {
                    cursor
                } else {
                    u64::from(range.offset)
                };
                let first = base.checked_add(offset)?;
                if first > u64::from(heap.count) {
                    return None;
                }
                let count = if range.count == u32::MAX {
                    u64::from(heap.count) - first
                } else {
                    u64::from(range.count)
                };
                if first.checked_add(count)? > u64::from(heap.count) {
                    return None;
                }
                cursor = offset.checked_add(count)?;
                *total = (*total).checked_add(count)?;
                if *total > MAX_TABLE_DESCRIPTORS {
                    return None;
                }
                if count == 0 {
                    continue;
                }
                let byte_offset = first.checked_mul(increment)?;
                spans.push(DescriptorSpan {
                    heap: heap.heap.clone(),
                    kind: range.kind,
                    first: u32::try_from(first).ok()?,
                    count: u32::try_from(count).ok()?,
                    increment: heap.increment,
                    cpu: heap.cpu.checked_add(usize::try_from(byte_offset).ok()?)?,
                    gpu: heap.gpu.checked_add(byte_offset)?,
                });
            }
        }
        Some(())
    }
}
#[derive(Default)]
pub(crate) struct State {
    stamp: u64,
    identity: u64,
    initialized: bool,
    invalid: bool,
    pipeline: Option<ID3D12PipelineState>,
    heaps: Option<Vec<ID3D12DescriptorHeap>>,
    graphics: Stage,
    compute: Stage,
    topology: Option<D3D_PRIMITIVE_TOPOLOGY>,
    vertex: Option<D3D12_VERTEX_BUFFER_VIEW>,
    index: Option<Option<D3D12_INDEX_BUFFER_VIEW>>,
    viewport: Option<D3D12_VIEWPORT>,
    scissor: Option<RECT>,
}
impl State {
    fn recording(&mut self) -> bool {
        if !self.initialized || self.invalid {
            return false;
        }
        if !enabled() {
            self.invalid = true;
            return false;
        }
        true
    }
    fn bump(&mut self) {
        match NEXT.try_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        }) {
            Ok(stamp) => self.stamp = stamp,
            Err(_) => self.invalid = true,
        }
    }
    pub(crate) unsafe fn reset(list: *mut c_void, pipeline: *mut c_void) -> Self {
        if !enabled() {
            return Self::default();
        }
        let mut state = Self {
            initialized: true,
            ..Self::default()
        };
        unsafe {
            state.pipeline(pipeline);
        }
        state.identity = state.stamp;
        let tagged = (|| {
            let object = unsafe { IUnknown::from_raw_borrowed(&list) }?
                .cast::<ID3D12Object>()
                .ok()?;
            let bytes = state.identity.to_le_bytes();
            unsafe { object.SetPrivateData(&LIST, 8, Some(bytes.as_ptr() as _)) }.ok()
        })()
        .is_some();
        if !tagged {
            state.invalid = true;
        }
        state
    }
    pub(crate) fn invalidate(&mut self) {
        if self.initialized {
            self.invalid = true;
            self.bump();
        }
    }
    pub(crate) unsafe fn pipeline(&mut self, raw: *mut c_void) {
        if !self.recording() {
            return;
        }
        self.pipeline = unsafe { ID3D12PipelineState::from_raw_borrowed(&raw) }.cloned();
        self.bump();
    }
    pub(crate) unsafe fn signature(&mut self, compute: bool, raw: *mut c_void) {
        if !self.recording() {
            return;
        }
        unsafe {
            if compute {
                self.compute.signature(raw)
            } else {
                self.graphics.signature(raw)
            }
        }
        self.bump();
    }
    pub(crate) fn table(&mut self, compute: bool, index: u32, value: D3D12_GPU_DESCRIPTOR_HANDLE) {
        if !self.recording() {
            return;
        }
        let stage = if compute {
            &mut self.compute
        } else {
            &mut self.graphics
        };
        if stage.known {
            match stage.args.get_mut(index as usize) {
                Some(Argument::Table(slot)) => *slot = Some(value),
                _ => {
                    self.invalidate();
                    return;
                }
            }
        }
        self.bump();
    }
    pub(crate) fn address(&mut self, index: u32, kind: D3D12_ROOT_PARAMETER_TYPE, value: u64) {
        if !self.recording() {
            return;
        }
        if self.graphics.known {
            match self.graphics.args.get_mut(index as usize) {
                Some(Argument::Address(actual, slot)) if *actual == kind => *slot = Some(value),
                _ => {
                    self.invalidate();
                    return;
                }
            }
        }
        self.bump();
    }
    pub(crate) unsafe fn constants(
        &mut self,
        index: u32,
        count: u32,
        values: *const c_void,
        offset: u32,
    ) {
        if !self.recording() {
            return;
        }
        if count > 64 || count > 0 && values.is_null() {
            self.invalidate();
            return;
        }
        if self.graphics.known {
            let Some(Argument::Constants(slots)) = self.graphics.args.get_mut(index as usize)
            else {
                self.invalidate();
                return;
            };
            let Some(end) = (offset as usize)
                .checked_add(count as usize)
                .filter(|end| *end <= slots.len())
            else {
                self.invalidate();
                return;
            };
            // Preserve exact 32-bit values, including NaN/sign bits. The actual
            // API owns this input span for the duration of its callback only.
            for (index, slot) in slots[offset as usize..end].iter_mut().enumerate() {
                *slot = Some(unsafe { std::ptr::read_unaligned(values.cast::<u32>().add(index)) });
            }
        }
        self.bump();
    }
    pub(crate) unsafe fn heaps(&mut self, count: u32, raw: *const *mut c_void) {
        if !self.recording() {
            return;
        }
        if count > 2 || count > 0 && raw.is_null() {
            self.invalidate();
            return;
        }
        let mut heaps = Vec::new();
        let mut types = Vec::new();
        for index in 0..count as usize {
            let raw = unsafe { *raw.add(index) };
            let Some(heap) = (unsafe { ID3D12DescriptorHeap::from_raw_borrowed(&raw) }) else {
                self.invalidate();
                return;
            };
            let desc = unsafe { heap.GetDesc() };
            if !matches!(
                desc.Type,
                D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV | D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER
            ) || desc.Flags.0 & D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE.0 == 0
                || types.contains(&desc.Type)
            {
                self.invalidate();
                return;
            }
            types.push(desc.Type);
            heaps.push(heap.clone());
        }
        // Heap order does not select table types. Normalize to avoid treating a
        // reordered binding of the identical set as a different heap set.
        heaps.sort_by_key(|heap| unsafe { heap.GetDesc() }.Type.0);
        if self.heaps.as_ref() != Some(&heaps) {
            self.graphics.invalidate_tables();
            self.compute.invalidate_tables();
        }
        self.heaps = Some(heaps);
        self.bump();
    }
    pub(crate) fn topology(&mut self, value: D3D_PRIMITIVE_TOPOLOGY) {
        if self.recording() {
            self.topology = Some(value);
            self.bump();
        }
    }
    pub(crate) unsafe fn vertex(
        &mut self,
        start: u32,
        count: u32,
        views: *const D3D12_VERTEX_BUFFER_VIEW,
    ) {
        if !self.recording() {
            return;
        }
        if start > 32 || count > 32 || start.checked_add(count).is_none_or(|end| end > 32) {
            self.invalidate();
            return;
        }
        if start == 0 && count > 0 {
            self.vertex = if views.is_null() {
                None
            } else {
                Some(unsafe { *views })
            };
            self.bump();
        }
    }
    pub(crate) unsafe fn index(&mut self, view: *const D3D12_INDEX_BUFFER_VIEW) {
        if !self.recording() {
            return;
        }
        self.index = Some(if view.is_null() {
            None
        } else {
            Some(unsafe { *view })
        });
        self.bump();
    }
    pub(crate) unsafe fn viewport(&mut self, count: u32, values: *const D3D12_VIEWPORT) {
        if !self.recording() {
            return;
        }
        self.viewport = if count == 1 && !values.is_null() {
            Some(unsafe { *values })
        } else {
            None
        };
        self.bump();
    }
    pub(crate) unsafe fn scissors(&mut self, count: u32, values: *const RECT) {
        if !self.recording() {
            return;
        }
        self.scissor = if count == 1 && !values.is_null() {
            Some(unsafe { *values })
        } else {
            None
        };
        self.bump();
    }
    /// Observed full-surface raster bounds. The cosmetics recorder leaves these
    /// bindings untouched, so they are checked without being restored.
    pub(crate) fn full_surface(&self, width: u32, height: u32) -> bool {
        self.viewport
            .as_ref()
            .zip(self.scissor.as_ref())
            .is_some_and(|(view, rect)| {
                view.TopLeftX == 0.
                    && view.TopLeftY == 0.
                    && view.Width == width as f32
                    && view.Height == height as f32
                    && view.MinDepth == 0.
                    && view.MaxDepth == 1.
                    && rect.left == 0
                    && rect.top == 0
                    && i32::try_from(width).ok() == Some(rect.right)
                    && i32::try_from(height).ok() == Some(rect.bottom)
            })
    }
    pub(crate) fn restorable(&self) -> bool {
        enabled()
            && self.initialized
            && !self.invalid
            && self.pipeline.is_some()
            && self.heaps.is_some()
            && self.graphics.graphics_ready()
            && self.compute.tables_ready()
            && self
                .topology
                .is_some_and(|value| value != D3D_PRIMITIVE_TOPOLOGY_UNDEFINED)
            && self.vertex.is_some()
            && self.index.is_some()
    }
    /// Exact public CBV bindings 2/3 joined to the enclosing native batch's
    /// copied packet map. A recent world scope is never used as a fallback.
    pub(crate) fn native_world_scope(&self, list: &ID3D12GraphicsCommandList) -> Option<u64> {
        if !self.restorable() || !self.same_list(list) {
            return None;
        }
        let address = |index: usize| match self.graphics.args.get(index)? {
            Argument::Address(D3D12_ROOT_PARAMETER_TYPE_CBV, Some(address)) => Some(*address),
            _ => None,
        };
        crate::native_batch::binding_scope(list.as_raw() as usize, address(2), address(3))
    }
    fn same_list(&self, list: &ID3D12GraphicsCommandList) -> bool {
        self.identity != 0 && list_identity(list) == Some(self.identity)
    }
    pub(crate) fn list_token(&self, list: &ID3D12GraphicsCommandList) -> Option<ListToken> {
        (enabled() && self.initialized && !self.invalid && self.same_list(list))
            .then_some(ListToken(self.identity))
    }
    pub(crate) fn snapshot(&self, list: &ID3D12GraphicsCommandList) -> Option<Snapshot> {
        if !self.restorable() || !self.same_list(list) {
            return None;
        }
        let arguments: Option<Vec<RestoreArgument>> = self
            .graphics
            .args
            .iter()
            .map(|arg| {
                Some(match arg {
                    Argument::Table(value) => RestoreArgument::Table((*value)?),
                    Argument::Address(kind, value) => {
                        let value = (*value)?;
                        match *kind {
                            D3D12_ROOT_PARAMETER_TYPE_CBV => RestoreArgument::Cbv(value),
                            D3D12_ROOT_PARAMETER_TYPE_SRV => RestoreArgument::Srv(value),
                            D3D12_ROOT_PARAMETER_TYPE_UAV => RestoreArgument::Uav(value),
                            _ => return None,
                        }
                    }
                    Argument::Constants(values) => RestoreArgument::Constants(
                        values.iter().copied().collect::<Option<Vec<u32>>>()?,
                    ),
                })
            })
            .collect();
        let arguments = arguments?;
        let vertex = self.vertex?;
        let index = self.index?;
        let buffer_owners = committed_buffer_owners(list, &arguments, &vertex, index.as_ref());
        let descriptor_spans = (|| {
            let heaps = table_heaps(list, self.heaps.as_ref()?)?;
            let mut spans = Vec::new();
            let mut total = 0;
            self.graphics
                .descriptor_spans(&heaps, &mut spans, &mut total)?;
            self.compute
                .descriptor_spans(&heaps, &mut spans, &mut total)?;
            Some(spans)
        })();
        let descriptor_resources = descriptor_spans
            .as_deref()
            .and_then(crate::native_descriptor_resources::capture);
        Some(Snapshot {
            stamp: self.stamp,
            identity: self.identity,
            pipeline: self.pipeline.clone()?,
            heaps: self.heaps.as_ref()?.iter().cloned().map(Some).collect(),
            root: self.graphics.root.clone()?,
            arguments,
            _compute_root: self.compute.root.clone(),
            compute_tables: self
                .compute
                .args
                .iter()
                .enumerate()
                .filter_map(|(index, arg)| match arg {
                    Argument::Table(Some(handle)) => Some((index as u32, *handle)),
                    _ => None,
                })
                .collect(),
            topology: self.topology?,
            vertex,
            index,
            buffer_owners,
            descriptor_spans,
            descriptor_resources,
        })
    }
}
#[derive(Clone)]
enum RestoreArgument {
    Table(D3D12_GPU_DESCRIPTOR_HANDLE),
    Cbv(u64),
    Srv(u64),
    Uav(u64),
    Constants(Vec<u32>),
}
fn committed_buffer_owners(
    list: &ID3D12GraphicsCommandList,
    arguments: &[RestoreArgument],
    vertex: &D3D12_VERTEX_BUFFER_VIEW,
    index: Option<&D3D12_INDEX_BUFFER_VIEW>,
) -> Option<Vec<Arc<crate::native_committed_resources::Owner>>> {
    use crate::native_committed_resources::{self, BufferSpan};
    let mut spans = Vec::new();
    for argument in arguments {
        let address = match argument {
            RestoreArgument::Cbv(address)
            | RestoreArgument::Srv(address)
            | RestoreArgument::Uav(address) => *address,
            _ => continue,
        };
        if address != 0 {
            // Root descriptors have no byte-size field. Pin the whole owning
            // committed buffer for this point; do not invent shader read bounds.
            spans.push(BufferSpan { address, bytes: 1 });
        }
    }
    if vertex.SizeInBytes != 0 {
        spans.push(BufferSpan {
            address: vertex.BufferLocation,
            bytes: u64::from(vertex.SizeInBytes),
        });
    }
    if let Some(index) = index.filter(|index| index.SizeInBytes != 0) {
        spans.push(BufferSpan {
            address: index.BufferLocation,
            bytes: u64::from(index.SizeInBytes),
        });
    }
    let mut device: Option<ID3D12Device> = None;
    unsafe { list.GetDevice(&mut device) }.ok()?;
    native_committed_resources::buffers(&device?, &spans)
}
#[derive(Clone)]
pub(crate) struct Snapshot {
    stamp: u64,
    identity: u64,
    pipeline: ID3D12PipelineState,
    heaps: Vec<Option<ID3D12DescriptorHeap>>,
    root: ID3D12RootSignature,
    arguments: Vec<RestoreArgument>,
    _compute_root: Option<ID3D12RootSignature>,
    compute_tables: Vec<(u32, D3D12_GPU_DESCRIPTOR_HANDLE)>,
    topology: D3D_PRIMITIVE_TOPOLOGY,
    vertex: D3D12_VERTEX_BUFFER_VIEW,
    index: Option<D3D12_INDEX_BUFFER_VIEW>,
    buffer_owners: Option<Vec<Arc<crate::native_committed_resources::Owner>>>,
    descriptor_spans: Option<Vec<DescriptorSpan>>,
    descriptor_resources: Option<crate::native_descriptor_resources::Snapshot>,
}
impl Snapshot {
    /// Declared graphics/compute table coverage only, not a descriptor-content
    /// snapshot or a resource lifetime owner. Unknown/oversized coverage is None.
    #[allow(dead_code)]
    pub(crate) fn descriptor_spans(&self) -> Option<&[DescriptorSpan]> {
        self.descriptor_spans.as_deref()
    }
    /// Storage completeness is separate from owning objects or freezing slots.
    #[allow(dead_code)]
    pub(crate) fn descriptor_allocations_known(&self) -> bool {
        self.descriptor_resources
            .as_ref()
            .is_some_and(crate::native_descriptor_resources::Snapshot::allocations_known)
    }
    /// Optional automatic storage owners for every restored non-null graphics
    /// root address and nonempty vertex/index view. Unknown/placed allocations
    /// remain the caller's responsibility, as do contents/descriptors/phase.
    /// Automatic address resolution needs a single-node device until an actual
    /// recording-node observation is carried by this list shadow.
    #[allow(dead_code)]
    pub(crate) fn committed_buffers(
        &self,
    ) -> Option<&[Arc<crate::native_committed_resources::Owner>]> {
        self.buffer_owners.as_deref()
    }
    pub(crate) fn matches(&self, state: &State, list: &ID3D12GraphicsCommandList) -> bool {
        state.restorable()
            && state.stamp == self.stamp
            && state.identity == self.identity
            && state.same_list(list)
            && self
                .descriptor_resources
                .as_ref()
                .is_none_or(|resources| resources.stable())
    }
    /// # Safety
    /// Same live recording list/cohort as capture, after only record_ordered's
    /// documented state changes. Its compute root signature, targets, viewport,
    /// scissor and other dynamic state must be unchanged. Caller pins this
    /// snapshot and all native assets through the native list's lifetime. Known
    /// committed buffer owners are retained by the snapshot; that does not pin
    /// unknown allocations or descriptor contents. The inserted recorder must
    /// also run inside private_commands so its temporary
    /// state cannot replace the captured native shadow state.
    pub(crate) unsafe fn restore(&self, list: &ID3D12GraphicsCommandList) -> Result<(), String> {
        if list_identity(list) != Some(self.identity) {
            return Err("native graphics snapshot belongs to another list lifetime".into());
        }
        let _private = crate::depth_tracker::private_commands();
        unsafe {
            list.SetDescriptorHeaps(&self.heaps);
            list.SetPipelineState(&self.pipeline);
            list.SetGraphicsRootSignature(&self.root);
            for (index, arg) in self.arguments.iter().enumerate() {
                let index = index as u32;
                match arg {
                    RestoreArgument::Table(handle) => {
                        list.SetGraphicsRootDescriptorTable(index, *handle)
                    }
                    RestoreArgument::Cbv(address) => {
                        list.SetGraphicsRootConstantBufferView(index, *address)
                    }
                    RestoreArgument::Srv(address) => {
                        list.SetGraphicsRootShaderResourceView(index, *address)
                    }
                    RestoreArgument::Uav(address) => {
                        list.SetGraphicsRootUnorderedAccessView(index, *address)
                    }
                    RestoreArgument::Constants(values) => {
                        list.SetGraphicsRoot32BitConstants(
                            index,
                            values.len() as u32,
                            values.as_ptr() as _,
                            0,
                        );
                    }
                }
            }
            // The cosmetics recorder does not change the compute signature or
            // its non-table arguments. Restore only heap-invalidated tables.
            for &(index, handle) in &self.compute_tables {
                list.SetComputeRootDescriptorTable(index, handle);
            }
            list.IASetPrimitiveTopology(self.topology);
            list.IASetVertexBuffers(0, Some(&[self.vertex]));
            list.IASetIndexBuffer(self.index.as_ref().map(|value| value as *const _));
        }
        Ok(())
    }
}
