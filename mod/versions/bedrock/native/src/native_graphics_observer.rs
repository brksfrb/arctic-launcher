//! Additional public setters needed for the cosmetics recorder's restore set.
use hudhook::{
    mh::{MH_CreateHook, MH_EnableHook, MH_STATUS},
    windows::{
        Win32::{
            Foundation::RECT,
            Graphics::{Direct3D::D3D_PRIMITIVE_TOPOLOGY, Direct3D12::*},
        },
        core::Interface,
    },
};
use std::{
    ffi::c_void,
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};
type Heaps = unsafe extern "system" fn(*mut c_void, u32, *const *mut c_void);
type Topology = unsafe extern "system" fn(*mut c_void, D3D_PRIMITIVE_TOPOLOGY);
type Vertex = unsafe extern "system" fn(*mut c_void, u32, u32, *const D3D12_VERTEX_BUFFER_VIEW);
type Index = unsafe extern "system" fn(*mut c_void, *const D3D12_INDEX_BUFFER_VIEW);
type Table = unsafe extern "system" fn(*mut c_void, u32, D3D12_GPU_DESCRIPTOR_HANDLE);
type Signature = unsafe extern "system" fn(*mut c_void, *mut c_void);
type Constant = unsafe extern "system" fn(*mut c_void, u32, u32, u32);
type Constants = unsafe extern "system" fn(*mut c_void, u32, u32, *const c_void, u32);
type Address = unsafe extern "system" fn(*mut c_void, u32, u64);
type Scissors = unsafe extern "system" fn(*mut c_void, u32, *const RECT);
struct Originals {
    heaps: Heaps,
    topology: Topology,
    vertex: Vertex,
    index: Index,
    table: Table,
    compute_table: Table,
    compute_signature: Signature,
    constant: Constant,
    constants: Constants,
    srv: Address,
    uav: Address,
    scissors: Scissors,
    state_object: Option<Signature>,
}
static ORIGINALS: OnceLock<Originals> = OnceLock::new();
static INSTALL: OnceLock<Result<(), String>> = OnceLock::new();
static ENABLED: AtomicBool = AtomicBool::new(false);
pub(crate) fn enabled() -> bool {
    ENABLED.load(Ordering::Acquire)
}
unsafe extern "system" fn heaps(this: *mut c_void, count: u32, values: *const *mut c_void) {
    crate::depth_tracker::graphics_observe(this as usize, |state| unsafe {
        state.heaps(count, values)
    });
    unsafe {
        (ORIGINALS.get().unwrap().heaps)(this, count, values);
    }
}
unsafe extern "system" fn topology(this: *mut c_void, value: D3D_PRIMITIVE_TOPOLOGY) {
    crate::depth_tracker::graphics_observe(this as usize, |state| state.topology(value));
    unsafe {
        (ORIGINALS.get().unwrap().topology)(this, value);
    }
}
unsafe extern "system" fn vertex(
    this: *mut c_void,
    start: u32,
    count: u32,
    values: *const D3D12_VERTEX_BUFFER_VIEW,
) {
    crate::depth_tracker::graphics_observe(this as usize, |state| unsafe {
        state.vertex(start, count, values)
    });
    unsafe {
        (ORIGINALS.get().unwrap().vertex)(this, start, count, values);
    }
}
unsafe extern "system" fn index(this: *mut c_void, value: *const D3D12_INDEX_BUFFER_VIEW) {
    crate::depth_tracker::graphics_observe(this as usize, |state| unsafe { state.index(value) });
    unsafe {
        (ORIGINALS.get().unwrap().index)(this, value);
    }
}
unsafe extern "system" fn table(this: *mut c_void, index: u32, value: D3D12_GPU_DESCRIPTOR_HANDLE) {
    crate::depth_tracker::graphics_observe(this as usize, |state| state.table(false, index, value));
    unsafe {
        (ORIGINALS.get().unwrap().table)(this, index, value);
    }
}
unsafe extern "system" fn compute_table(
    this: *mut c_void,
    index: u32,
    value: D3D12_GPU_DESCRIPTOR_HANDLE,
) {
    crate::depth_tracker::graphics_observe(this as usize, |state| state.table(true, index, value));
    unsafe {
        (ORIGINALS.get().unwrap().compute_table)(this, index, value);
    }
}
unsafe extern "system" fn compute_signature(this: *mut c_void, value: *mut c_void) {
    crate::depth_tracker::graphics_observe(this as usize, |state| unsafe {
        state.signature(true, value)
    });
    unsafe {
        (ORIGINALS.get().unwrap().compute_signature)(this, value);
    }
}
unsafe extern "system" fn constant(this: *mut c_void, index: u32, value: u32, offset: u32) {
    crate::depth_tracker::graphics_observe(this as usize, |state| unsafe {
        state.constants(index, 1, &value as *const u32 as _, offset)
    });
    unsafe {
        (ORIGINALS.get().unwrap().constant)(this, index, value, offset);
    }
}
unsafe extern "system" fn constants(
    this: *mut c_void,
    index: u32,
    count: u32,
    values: *const c_void,
    offset: u32,
) {
    crate::depth_tracker::graphics_observe(this as usize, |state| unsafe {
        state.constants(index, count, values, offset)
    });
    unsafe {
        (ORIGINALS.get().unwrap().constants)(this, index, count, values, offset);
    }
}
unsafe extern "system" fn srv(this: *mut c_void, index: u32, value: u64) {
    crate::depth_tracker::graphics_observe(this as usize, |state| {
        state.address(index, D3D12_ROOT_PARAMETER_TYPE_SRV, value)
    });
    unsafe {
        (ORIGINALS.get().unwrap().srv)(this, index, value);
    }
}
unsafe extern "system" fn uav(this: *mut c_void, index: u32, value: u64) {
    crate::depth_tracker::graphics_observe(this as usize, |state| {
        state.address(index, D3D12_ROOT_PARAMETER_TYPE_UAV, value)
    });
    unsafe {
        (ORIGINALS.get().unwrap().uav)(this, index, value);
    }
}
unsafe extern "system" fn state_object(this: *mut c_void, value: *mut c_void) {
    crate::depth_tracker::graphics_observe(this as usize, |state| state.invalidate());
    unsafe {
        (ORIGINALS
            .get()
            .unwrap()
            .state_object
            .expect("installed state-object hook"))(this, value);
    }
}
unsafe extern "system" fn scissors(this: *mut c_void, count: u32, values: *const RECT) {
    crate::depth_tracker::graphics_observe(this as usize, |state| unsafe {
        state.scissors(count, values)
    });
    unsafe {
        (ORIGINALS.get().unwrap().scissors)(this, count, values);
    }
}
pub(crate) unsafe fn install(list: &ID3D12GraphicsCommandList) -> Result<(), String> {
    INSTALL
        .get_or_init(|| unsafe { install_targets(list) })
        .clone()
}
unsafe fn install_targets(list: &ID3D12GraphicsCommandList) -> Result<(), String> {
    let v = list.vtable();
    let mut targets = vec![
        v.SetDescriptorHeaps as usize,
        v.IASetPrimitiveTopology as usize,
        v.IASetVertexBuffers as usize,
        v.IASetIndexBuffer as usize,
        v.SetGraphicsRootDescriptorTable as usize,
        v.SetComputeRootDescriptorTable as usize,
        v.SetComputeRootSignature as usize,
        v.SetGraphicsRoot32BitConstant as usize,
        v.SetGraphicsRoot32BitConstants as usize,
        v.SetGraphicsRootShaderResourceView as usize,
        v.SetGraphicsRootUnorderedAccessView as usize,
        v.RSSetScissorRects as usize,
    ];
    let mut callbacks = vec![
        heaps as *const () as usize,
        topology as *const () as usize,
        vertex as *const () as usize,
        index as *const () as usize,
        table as *const () as usize,
        compute_table as *const () as usize,
        compute_signature as *const () as usize,
        constant as *const () as usize,
        constants as *const () as usize,
        srv as *const () as usize,
        uav as *const () as usize,
        scissors as *const () as usize,
    ];
    if let Ok(list) = list.cast::<ID3D12GraphicsCommandList4>() {
        targets.push(list.vtable().SetPipelineState1 as usize);
        callbacks.push(state_object as *const () as usize);
    }
    let mut originals = Vec::new();
    for (&target, callback) in targets.iter().zip(callbacks) {
        let mut original = std::ptr::null_mut();
        let result = unsafe { MH_CreateHook(target as _, callback as _, &mut original) };
        if result != MH_STATUS::MH_OK {
            return Err(format!("create native graphics-state observer: {result:?}"));
        }
        originals.push(original as usize);
    }
    let originals = unsafe {
        Originals {
            heaps: std::mem::transmute::<usize, Heaps>(originals[0]),
            topology: std::mem::transmute::<usize, Topology>(originals[1]),
            vertex: std::mem::transmute::<usize, Vertex>(originals[2]),
            index: std::mem::transmute::<usize, Index>(originals[3]),
            table: std::mem::transmute::<usize, Table>(originals[4]),
            compute_table: std::mem::transmute::<usize, Table>(originals[5]),
            compute_signature: std::mem::transmute::<usize, Signature>(originals[6]),
            constant: std::mem::transmute::<usize, Constant>(originals[7]),
            constants: std::mem::transmute::<usize, Constants>(originals[8]),
            srv: std::mem::transmute::<usize, Address>(originals[9]),
            uav: std::mem::transmute::<usize, Address>(originals[10]),
            scissors: std::mem::transmute::<usize, Scissors>(originals[11]),
            state_object: originals
                .get(12)
                .map(|&value| std::mem::transmute::<usize, Signature>(value)),
        }
    };
    ORIGINALS
        .set(originals)
        .map_err(|_| "native graphics-state observers already registered")?;
    for target in targets {
        let result = unsafe { MH_EnableHook(target as _) };
        if result != MH_STATUS::MH_OK {
            return Err(format!("enable native graphics-state observer: {result:?}"));
        }
    }
    ENABLED.store(true, Ordering::Release);
    Ok(())
}
