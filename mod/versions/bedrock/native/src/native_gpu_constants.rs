//! Queue-time diagnostic prefixes and named vectors of observed upload buffers.
//! These are not shader reflection or proof of GPU execution. No resource is edited.
use hudhook::mh::{MH_CreateHook, MH_EnableHook, MH_STATUS};
use hudhook::windows::Win32::Graphics::Direct3D12::*;
use hudhook::windows::Win32::Graphics::Dxgi::Common::DXGI_SAMPLE_DESC;
use hudhook::windows::core::{HRESULT, Interface};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};
use std::ffi::c_void;
use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicU64, Ordering},
};

const PREFIX: usize = 512;
const MAX_RESOURCES: usize = 64;
const MAX_BYTES: u64 = 256 * 1024 * 1024;
const MAX_SCOPES: usize = 8;
const MAX_DRAWS: usize = 16;
type Map =
    unsafe extern "system" fn(*mut c_void, u32, *const D3D12_RANGE, *mut *mut c_void) -> HRESULT;
type Unmap = unsafe extern "system" fn(*mut c_void, u32, *const D3D12_RANGE);
static ORIGINALS: OnceLock<(Map, Unmap)> = OnceLock::new();
static INSTALL: OnceLock<Result<(), String>> = OnceLock::new();
static MAP_CALLS: AtomicU64 = AtomicU64::new(0);
// Independently bounded immutable copies survive scope/resource retirement until
// their native Java frame is accepted (or superseded) by the capture pipeline.
static FRAME_SAMPLES: Mutex<VecDeque<(String, u64, Value)>> = Mutex::new(VecDeque::new());
static STATE: Mutex<State> = Mutex::new(State {
    resources: Vec::new(),
    scopes: VecDeque::new(),
    rejected_maps: 0,
    omitted_maps: 0,
    heap_profiles: Vec::new(),
});
struct Mapping {
    // Retain the COM object so a destroyed/reallocated resource cannot alias this VA.
    resource: ID3D12Resource,
    gpu: u64,
    cpu: usize,
    width: u64,
}
struct Scope {
    id: u64,
    queue: usize,
    draws: u64,
    omitted: u64,
    samples: Vec<DrawSample>,
    // Bound repeated-draw accounting separately from the retained byte prefixes.
    seen: HashMap<DrawConstants, u64>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct DrawConstants {
    pub vertex: u64,
    pub fragment: u64,
    pub shaders: Option<crate::native_shader_layout::ShaderPair>,
    pub root: Option<crate::native_shader_layout::RootBinding>,
    pub targets: [u64; 8],
}
struct DrawSample {
    draw: u64,
    input: DrawConstants,
    captured: Option<Value>,
}
struct State {
    resources: Vec<Mapping>,
    scopes: VecDeque<Scope>,
    rejected_maps: u64,
    omitted_maps: u64,
    heap_profiles: Vec<Value>,
}
fn cpu_upload(heap: &D3D12_HEAP_PROPERTIES) -> bool {
    matches!(
        heap.Type,
        D3D12_HEAP_TYPE_UPLOAD | D3D12_HEAP_TYPE_GPU_UPLOAD
    ) || (heap.Type == D3D12_HEAP_TYPE_CUSTOM
        && matches!(
            heap.CPUPageProperty,
            D3D12_CPU_PAGE_PROPERTY_WRITE_COMBINE | D3D12_CPU_PAGE_PROPERTY_WRITE_BACK
        ))
}
#[cfg(test)]
fn offset(gpu: u64, width: u64, address: u64) -> Option<usize> {
    offset_for_bytes(gpu, width, address, PREFIX)
}
fn offset_for_bytes(gpu: u64, width: u64, address: u64, bytes: usize) -> Option<usize> {
    gpu.checked_add(width)?;
    if address == 0 || !address.is_multiple_of(256) {
        return None;
    }
    let at = address.checked_sub(gpu)?;
    (bytes > 0 && at.checked_add(u64::try_from(bytes).ok()?)? <= width)
        .then(|| usize::try_from(at).ok())
        .flatten()
}
fn mapped_span(resources: &[Mapping], address: u64, bytes: usize) -> Option<(&Mapping, usize)> {
    if bytes == 0 {
        return None;
    }
    let end = address.checked_add(u64::try_from(bytes).ok()?)?;
    let mut selected = None;
    for resource in resources {
        let resource_end = resource.gpu.checked_add(resource.width)?;
        if resource.gpu < end && address < resource_end {
            // Even a partial alias makes the GPU-address owner ambiguous.
            if selected.is_some() {
                return None;
            }
            selected = Some(resource);
        }
    }
    let resource = selected?;
    Some((
        resource,
        offset_for_bytes(resource.gpu, resource.width, address, bytes)?,
    ))
}
fn named_vectors(
    resources: &[Mapping],
    address: u64,
    shader: [u8; 32],
    register: u32,
    space: u32,
) -> Option<Value> {
    let layout = crate::native_shader_asset::vectors(shader, register, space)?;
    let (resource, at) = mapped_span(resources, address, layout.buffer_bytes)?;
    layout.capture(|offset, bytes| {
        if offset.checked_add(bytes.len())? > layout.buffer_bytes {
            return None;
        }
        crate::world_hooks::read(resource.cpu.checked_add(at)?.checked_add(offset)?, bytes).ok()
    })
}
fn prefix(resources: &[Mapping], address: u64) -> Option<Value> {
    let (resource, at) = mapped_span(resources, address, PREFIX)?;
    let mut bytes = [0; PREFIX];
    crate::world_hooks::read(resource.cpu.checked_add(at)?, &mut bytes).ok()?;
    Some(json!({"sha256":format!("{:x}",Sha256::digest(bytes)),
        "hex":bytes.iter().map(|b|format!("{b:02x}")).collect::<String>(),
        "resourceOffset":at,"resourceWidth":resource.width,"bytes":PREFIX}))
}
unsafe fn track(this: *mut c_void, subresource: u32, out: *mut *mut c_void) {
    if subresource != 0 || out.is_null() {
        return;
    }
    let raw = this;
    let Some(resource) = (unsafe { ID3D12Resource::from_raw_borrowed(&raw) }) else {
        return;
    };
    let desc = unsafe { resource.GetDesc() };
    if desc.Dimension != D3D12_RESOURCE_DIMENSION_BUFFER {
        return;
    }
    let mut heap = D3D12_HEAP_PROPERTIES::default();
    if unsafe { resource.GetHeapProperties(Some(&mut heap), None) }.is_err() {
        return;
    }
    let gpu = unsafe { resource.GetGPUVirtualAddress() };
    let cpu = unsafe { *out } as usize;
    let Ok(mut state) = STATE.lock() else { return };
    let profile = json!({"heapType":heap.Type.0,"cpuPageProperty":heap.CPUPageProperty.0,
        "memoryPool":heap.MemoryPoolPreference.0});
    if state.heap_profiles.len() < 8 && !state.heap_profiles.contains(&profile) {
        state.heap_profiles.push(profile);
    }
    if !cpu_upload(&heap) {
        return;
    }
    // A repeated Map replaces the observed mapping; Unmap conservatively removes
    // it even if native code nested Maps. Missing evidence is preferable to stale VA.
    state.resources.retain(|r| r.resource.as_raw() != this);
    if cpu == 0 || gpu == 0 || desc.Width < PREFIX as u64 || gpu.checked_add(desc.Width).is_none() {
        state.rejected_maps += 1;
        return;
    }
    let total: u64 = state.resources.iter().map(|r| r.width).sum();
    if state.resources.len() >= MAX_RESOURCES || desc.Width > MAX_BYTES.saturating_sub(total) {
        state.omitted_maps += 1;
        return;
    }
    state.resources.push(Mapping {
        resource: resource.clone(),
        gpu,
        cpu,
        width: desc.Width,
    });
}
unsafe extern "system" fn mapped(
    this: *mut c_void,
    sub: u32,
    range: *const D3D12_RANGE,
    out: *mut *mut c_void,
) -> HRESULT {
    MAP_CALLS.fetch_add(1, Ordering::Relaxed);
    let result =
        unsafe { (ORIGINALS.get().expect("published Map trampoline").0)(this, sub, range, out) };
    if result.is_ok() {
        let _ = std::panic::catch_unwind(|| unsafe { track(this, sub, out) });
    }
    result
}
unsafe extern "system" fn unmapped(this: *mut c_void, sub: u32, range: *const D3D12_RANGE) {
    let _ = std::panic::catch_unwind(|| {
        if sub == 0
            && let Ok(mut state) = STATE.lock()
        {
            // All observers hold this same lock, so none can read across Unmap.
            state.resources.retain(|r| r.resource.as_raw() != this);
        }
    });
    unsafe { (ORIGINALS.get().expect("published Unmap trampoline").1)(this, sub, range) };
}
pub(crate) fn initialize(device: &ID3D12Device) -> Result<(), String> {
    if !crate::native_clock::probe_enabled() {
        return Ok(());
    }
    install(device)
}
fn install(device: &ID3D12Device) -> Result<(), String> {
    INSTALL
        .get_or_init(|| unsafe {
            let mut resource: Option<ID3D12Resource> = None;
            device
                .CreateCommittedResource(
                    &D3D12_HEAP_PROPERTIES {
                        Type: D3D12_HEAP_TYPE_UPLOAD,
                        CreationNodeMask: 1,
                        VisibleNodeMask: 1,
                        ..Default::default()
                    },
                    D3D12_HEAP_FLAG_NONE,
                    &D3D12_RESOURCE_DESC {
                        Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
                        Width: 256,
                        Height: 1,
                        DepthOrArraySize: 1,
                        MipLevels: 1,
                        SampleDesc: DXGI_SAMPLE_DESC {
                            Count: 1,
                            Quality: 0,
                        },
                        Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
                        ..Default::default()
                    },
                    D3D12_RESOURCE_STATE_GENERIC_READ,
                    None,
                    &mut resource,
                )
                .map_err(|e| e.to_string())?;
            let resource = resource.ok_or("constant observer discovery resource missing")?;
            let targets = [
                resource.vtable().Map as usize,
                resource.vtable().Unmap as usize,
            ];
            let callbacks = [mapped as *const () as usize, unmapped as *const () as usize];
            let mut originals = [0usize; 2];
            for i in 0..2 {
                let mut original = std::ptr::null_mut();
                let result = MH_CreateHook(targets[i] as _, callbacks[i] as _, &mut original);
                if result != MH_STATUS::MH_OK {
                    return Err(format!("create GPU constant observer: {result:?}"));
                }
                originals[i] = original as usize;
            }
            ORIGINALS
                .set((
                    std::mem::transmute::<usize, Map>(originals[0]),
                    std::mem::transmute::<usize, Unmap>(originals[1]),
                ))
                .map_err(|_| "GPU constant observer already registered")?;
            for target in targets {
                let result = MH_EnableHook(target as _);
                if result != MH_STATUS::MH_OK {
                    return Err(format!("enable GPU constant observer: {result:?}"));
                }
            }
            Ok(())
        })
        .clone()
}
/// Called only for a closed, valid list being submitted, with its already-proven
/// native world scope and the root CBVs captured at this exact draw command.
pub(crate) fn observe_draw(queue: usize, scope: u64, input: DrawConstants) {
    if !crate::native_clock::frozen_for_probe() || ORIGINALS.get().is_none() {
        return;
    }
    let Ok(mut state) = STATE.lock() else { return };
    if !state
        .scopes
        .iter()
        .any(|s| s.id == scope && s.queue == queue)
    {
        if state.scopes.len() >= MAX_SCOPES {
            state.scopes.pop_front();
        }
        state.scopes.push_back(Scope {
            id: scope,
            queue,
            draws: 0,
            omitted: 0,
            samples: Vec::new(),
            seen: HashMap::new(),
        });
    }
    let index = state
        .scopes
        .iter()
        .position(|s| s.id == scope && s.queue == queue)
        .unwrap();
    let s = &mut state.scopes[index];
    s.draws += 1;
    if let Some(count) = s.seen.get_mut(&input) {
        *count += 1;
        return;
    }
    if s.seen.len() >= 4096 {
        s.omitted += 1;
        return;
    }
    s.seen.insert(input, 1);
    let draw = s.draws;
    if s.samples.len() == MAX_DRAWS {
        // Keep both initial and final packets, including late transparent draws.
        s.samples.remove(MAX_DRAWS / 2);
        s.omitted += 1;
    }
    s.samples.push(DrawSample {
        draw,
        input,
        captured: None,
    });
}
/// Copy only retained prefixes after recording the whole submission. Sampling
/// every packet and then discarding most of its bytes needlessly stalls rendering.
/// Named vectors have a separate fixed byte budget and declared-buffer bounds.
pub(crate) fn before_submit(queue: usize) {
    if ORIGINALS.get().is_none() {
        return;
    }
    let Ok(mut state) = STATE.lock() else { return };
    let State {
        resources, scopes, ..
    } = &mut *state;
    for scope in scopes.iter_mut().filter(|s| s.queue == queue) {
        for sample in scope.samples.iter_mut().filter(|s| s.captured.is_none()) {
            let input = sample.input;
            let vertex = prefix(resources, input.vertex);
            let fragment = prefix(resources, input.fragment);
            let vertex_named = input.shaders.zip(input.root).and_then(|(shaders, root)| {
                named_vectors(resources, input.vertex, shaders.0, root[0], root[1])
            });
            let fragment_named = input.shaders.zip(input.root).and_then(|(shaders, root)| {
                named_vectors(resources, input.fragment, shaders.1, root[2], root[3])
            });
            sample.captured = Some(
                json!({"draw":sample.draw,"vertexAddress":format!("{:x}",input.vertex),
                "fragmentAddress":format!("{:x}",input.fragment),"vertex":vertex,"fragment":fragment,
                "vertexNamed":vertex_named,"fragmentNamed":fragment_named,
                "vertexShader":input.shaders.and_then(|p|crate::native_shader_layout::key(p.0)),
                "fragmentShader":input.shaders.and_then(|p|crate::native_shader_layout::key(p.1)),
                "rootBinding":input.root.map(|r|json!({"vertexRegister":r[0],"vertexSpace":r[1],
                    "fragmentRegister":r[2],"fragmentSpace":r[3],"mappingVerified":true}))}),
            );
        }
    }
}
pub(crate) fn sample(scope: u64) -> Option<Value> {
    let state = STATE.try_lock().ok()?;
    let mut scopes = state.scopes.iter().filter(|s| s.id == scope);
    let s = scopes.next()?;
    if scopes.next().is_some() {
        return None;
    }
    let mut layouts = serde_json::Map::new();
    for hash in s
        .samples
        .iter()
        .filter_map(|s| s.input.shaders)
        .flat_map(|(v, p)| [v, p])
    {
        if let Some(key) = crate::native_shader_layout::key(hash)
            && !layouts.contains_key(&key)
            && let Some(layout) = crate::native_shader_layout::metadata(hash)
        {
            layouts.insert(key, layout);
        }
    }
    let named_vectors_observed =
        s.samples
            .iter()
            .filter_map(|s| s.captured.as_ref())
            .any(|sample| {
                ["vertexNamed", "fragmentNamed"].iter().any(|key| {
                    sample[*key]["copiedBytes"]
                        .as_u64()
                        .is_some_and(|bytes| bytes > 0)
                })
            });
    Some(
        json!({"scope":scope,"draws":s.draws,"uniqueBufferPairs":s.seen.len(),"omitted":s.omitted,
        "samples":s.samples.iter().filter_map(|s|s.captured.as_ref()).collect::<Vec<_>>(),
        "shaderLayouts":layouts,
        "prefixOnly":!named_vectors_observed,"rawPrefixBytes":PREFIX,
        "namedVectorByteLimitPerStage":crate::native_shader_asset::MAX_VECTOR_BYTES,
        "coherentSnapshotVerified":false,"completeInputCoverageVerified":false,
        "firstSubmissionPerBufferPair":true,"queueSubmissionObserved":true,
        "shaderLayoutVerified":false,"gpuExecutionVerified":false}),
    )
}
/// Only native live_frame publication binds a GPU scope to a session/sequence.
/// A fixture or an arbitrary capture tag cannot create this association.
pub(crate) fn frame_sample(scope: u64, session: &str, sequence: u64) -> Option<Value> {
    if let Some(value) = captured_frame(session, sequence) {
        return Some(value);
    }
    let value = sample(scope)?;
    let mut frames = FRAME_SAMPLES.lock().ok()?;
    if frames.len() >= 32 {
        frames.pop_front();
    }
    frames.push_back((session.to_owned(), sequence, value.clone()));
    Some(value)
}
pub(crate) fn captured_frame(session: &str, sequence: u64) -> Option<Value> {
    FRAME_SAMPLES
        .try_lock()
        .ok()?
        .iter()
        .find(|(s, n, _)| s == session && *n == sequence)
        .map(|(_, _, v)| v.clone())
}
pub(crate) fn status() -> Value {
    let Ok(state) = STATE.try_lock() else {
        return Value::Null;
    };
    json!({"installed":INSTALL.get().is_some_and(Result::is_ok),"mappedCpuVisibleBuffers":state.resources.len(),
        "mapCalls":MAP_CALLS.load(Ordering::Relaxed),"heapProfiles":state.heap_profiles,
        "retainedBytes":state.resources.iter().map(|r|r.width).sum::<u64>(),"rejectedMaps":state.rejected_maps,
        "omittedMaps":state.omitted_maps,"scopes":state.scopes.iter().map(|s|json!({"scope":s.id,"draws":s.draws,"omitted":s.omitted})).collect::<Vec<_>>(),
        "prefixBytes":PREFIX,"namedVectorByteLimitPerStage":crate::native_shader_asset::MAX_VECTOR_BYTES,
        "observerOnly":true,"deterministicVideoVerified":false})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prefixes_require_aligned_complete_non_overflowing_ranges() {
        assert_eq!(offset(0x1000, 1024, 0x1200), Some(512));
        assert_eq!(offset(0x1000, 1023, 0x1200), None);
        assert_eq!(offset(0x1000, 512, 0x1001), None);
        assert_eq!(offset(0x1000, 512, 0xf00), None);
        assert_eq!(offset(0x1000, u64::MAX, u64::MAX - 255), None);
    }
    #[test]
    fn mapped_native_upload_prefix_reads_current_bytes_and_rejects_unmapped_resource()
    -> Result<(), Box<dyn std::error::Error>> {
        use hudhook::windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0;
        use hudhook::windows::Win32::Graphics::Dxgi::{
            CreateDXGIFactory1, IDXGIAdapter, IDXGIFactory4,
        };
        let _gpu = crate::depth::GPU_TEST.lock().unwrap();
        unsafe {
            let _ = hudhook::mh::MH_Initialize();
            let factory: IDXGIFactory4 = CreateDXGIFactory1()?;
            let adapter: IDXGIAdapter = factory.EnumWarpAdapter()?;
            let mut device: Option<ID3D12Device> = None;
            D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_11_0, &mut device)?;
            let device = device.ok_or("missing WARP device")?;
            install(&device)?;
            for heap in [
                D3D12_HEAP_PROPERTIES {
                    Type: D3D12_HEAP_TYPE_UPLOAD,
                    CreationNodeMask: 1,
                    VisibleNodeMask: 1,
                    ..Default::default()
                },
                device.GetCustomHeapProperties(1, D3D12_HEAP_TYPE_UPLOAD),
            ] {
                assert!(cpu_upload(&heap));
                let mut buffer: Option<ID3D12Resource> = None;
                device.CreateCommittedResource(
                    &heap,
                    D3D12_HEAP_FLAG_NONE,
                    &D3D12_RESOURCE_DESC {
                        Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
                        Width: 1024,
                        Height: 1,
                        DepthOrArraySize: 1,
                        MipLevels: 1,
                        SampleDesc: DXGI_SAMPLE_DESC {
                            Count: 1,
                            Quality: 0,
                        },
                        Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
                        ..Default::default()
                    },
                    D3D12_RESOURCE_STATE_GENERIC_READ,
                    None,
                    &mut buffer,
                )?;
                let buffer = buffer.ok_or("missing upload buffer")?;
                let mut cpu = std::ptr::null_mut();
                buffer.Map(0, Some(&D3D12_RANGE { Begin: 0, End: 0 }), Some(&mut cpu))?;
                std::ptr::write_bytes(cpu, 0x31, 1024);
                let gpu = buffer.GetGPUVirtualAddress();
                let first = prefix(&STATE.lock().unwrap().resources, gpu)
                    .ok_or("upload Map was not observed")?;
                std::ptr::write_bytes(cpu, 0x62, PREFIX);
                let second = prefix(&STATE.lock().unwrap().resources, gpu)
                    .ok_or("updated upload prefix missing")?;
                assert_ne!(first["sha256"], second["sha256"]);
                assert_eq!(second["hex"], "62".repeat(PREFIX));
                // An owned test submission keeps an immutable copy at its
                // submission boundary, not at Map or at a later stats query.
                STATE.lock().unwrap().scopes.push_back(Scope {
                    id: u64::MAX,
                    queue: 123,
                    draws: 1,
                    omitted: 0,
                    seen: HashMap::from([(
                        DrawConstants {
                            vertex: gpu,
                            fragment: gpu,
                            shaders: None,
                            root: None,
                            targets: [0; 8],
                        },
                        1,
                    )]),
                    samples: vec![DrawSample {
                        draw: 1,
                        input: DrawConstants {
                            vertex: gpu,
                            fragment: gpu,
                            shaders: None,
                            root: None,
                            targets: [0; 8],
                        },
                        captured: None,
                    }],
                });
                before_submit(456);
                assert!(
                    sample(u64::MAX).unwrap()["samples"]
                        .as_array()
                        .unwrap()
                        .is_empty()
                );
                before_submit(123);
                let cached = frame_sample(u64::MAX, "native_test_1", 17).unwrap();
                assert!(captured_frame("native_test_2", 17).is_none());
                assert!(captured_frame("native_test_1", 18).is_none());
                std::ptr::write_bytes(cpu, 0x73, PREFIX);
                assert_eq!(
                    sample(u64::MAX).unwrap()["samples"][0]["vertex"]["hex"],
                    "62".repeat(PREFIX)
                );
                STATE.lock().unwrap().scopes.retain(|s| s.id != u64::MAX);
                let aliases = [
                    Mapping {
                        resource: buffer.clone(),
                        gpu,
                        cpu: cpu as usize,
                        width: 1024,
                    },
                    Mapping {
                        resource: buffer.clone(),
                        gpu,
                        cpu: cpu as usize,
                        width: 1024,
                    },
                ];
                assert!(prefix(&aliases, gpu).is_none());
                assert!(prefix(&STATE.lock().unwrap().resources, gpu + 1024).is_none());
                buffer.Unmap(0, None);
                assert!(prefix(&STATE.lock().unwrap().resources, gpu).is_none());
                assert_eq!(captured_frame("native_test_1", 17), Some(cached));
                FRAME_SAMPLES.lock().unwrap().clear();
            }
        }
        Ok(())
    }
}
