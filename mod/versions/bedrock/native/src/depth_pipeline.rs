//! Depth-write facts attached to observed PSO objects. COM private data follows
//! the object's lifetime, avoiding address reuse bugs and retained PSO caches.
use hudhook::windows::Win32::Graphics::Direct3D12::*;
use hudhook::windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT, DXGI_SAMPLE_DESC};
use hudhook::windows::core::{BOOL, GUID, IUnknown, Interface};
use std::ffi::c_void;
use std::mem::{align_of, size_of};

const TAG: GUID = GUID::from_u128(0xc3815470_2147_42db_8b23_85f04a762678);

pub fn writes(enabled: BOOL, mask: D3D12_DEPTH_WRITE_MASK) -> bool {
    enabled.as_bool() && mask == D3D12_DEPTH_WRITE_MASK_ALL
}

pub unsafe fn tag(raw: *mut c_void, value: Option<bool>) {
    if let Some(value) = value
        && let Some(object) = unsafe { IUnknown::from_raw_borrowed(&raw) }
        && let Ok(pso) = object.cast::<ID3D12PipelineState>()
    {
        let byte = u8::from(value);
        let _ = unsafe { pso.SetPrivateData(&TAG, 1, Some(&byte as *const u8 as _)) };
    }
}

pub unsafe fn mode(raw: *mut c_void) -> Option<bool> {
    let pso = unsafe { ID3D12PipelineState::from_raw_borrowed(&raw) }?;
    let mut size = 1;
    let mut byte = 255u8;
    unsafe { pso.GetPrivateData(&TAG, &mut size, Some(&mut byte as *mut u8 as _)) }.ok()?;
    (size == 1 && byte <= 1).then_some(byte == 1)
}

fn layout(kind: D3D12_PIPELINE_STATE_SUBOBJECT_TYPE) -> Option<(usize, usize)> {
    fn of<T>() -> (usize, usize) {
        (size_of::<T>(), align_of::<T>())
    }
    Some(match kind {
        D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_ROOT_SIGNATURE => of::<usize>(),
        D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_VS
        | D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_PS
        | D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_DS
        | D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_HS
        | D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_GS
        | D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_CS
        | D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_AS
        | D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_MS => of::<D3D12_SHADER_BYTECODE>(),
        D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_STREAM_OUTPUT => of::<D3D12_STREAM_OUTPUT_DESC>(),
        D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_BLEND => of::<D3D12_BLEND_DESC>(),
        D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_SAMPLE_MASK
        | D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_NODE_MASK => of::<u32>(),
        D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_RASTERIZER => of::<D3D12_RASTERIZER_DESC>(),
        D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_RASTERIZER1 => of::<D3D12_RASTERIZER_DESC1>(),
        D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_RASTERIZER2 => of::<D3D12_RASTERIZER_DESC2>(),
        D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_DEPTH_STENCIL => of::<D3D12_DEPTH_STENCIL_DESC>(),
        D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_DEPTH_STENCIL1 => of::<D3D12_DEPTH_STENCIL_DESC1>(),
        D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_DEPTH_STENCIL2 => of::<D3D12_DEPTH_STENCIL_DESC2>(),
        D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_INPUT_LAYOUT => of::<D3D12_INPUT_LAYOUT_DESC>(),
        D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_IB_STRIP_CUT_VALUE => {
            of::<D3D12_INDEX_BUFFER_STRIP_CUT_VALUE>()
        }
        D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_PRIMITIVE_TOPOLOGY => {
            of::<D3D12_PRIMITIVE_TOPOLOGY_TYPE>()
        }
        D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_RENDER_TARGET_FORMATS => of::<D3D12_RT_FORMAT_ARRAY>(),
        D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_DEPTH_STENCIL_FORMAT => of::<DXGI_FORMAT>(),
        D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_SAMPLE_DESC => of::<DXGI_SAMPLE_DESC>(),
        D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_CACHED_PSO => of::<D3D12_CACHED_PIPELINE_STATE>(),
        D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_FLAGS => of::<D3D12_PIPELINE_STATE_FLAGS>(),
        D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_VIEW_INSTANCING => of::<D3D12_VIEW_INSTANCING_DESC>(),
        _ => return None,
    })
}

/// Parse public x64 PSO stream layout, never the pointers inside its subobjects.
/// D3DX12 wraps each type/payload pair in alignas(void*); the payload itself may
/// have four-byte alignment. Assuming every payload begins at +8 is incorrect.
pub(crate) fn subobjects(
    bytes: &[u8],
) -> Option<Vec<(D3D12_PIPELINE_STATE_SUBOBJECT_TYPE, &[u8])>> {
    let mut offset = 0usize;
    let mut objects = Vec::new();
    let mut seen = std::collections::HashSet::new();
    while offset < bytes.len() {
        let kind = i32::from_le_bytes(bytes.get(offset..offset + 4)?.try_into().ok()?);
        if !seen.insert(kind) {
            return None;
        }
        let kind = D3D12_PIPELINE_STATE_SUBOBJECT_TYPE(kind);
        let (size, align) = layout(kind)?;
        let start = offset.checked_add(4)?.next_multiple_of(align);
        let payload = bytes.get(start..start.checked_add(size)?)?;
        objects.push((kind, payload));
        offset = start.checked_add(size)?.next_multiple_of(8);
        if offset > bytes.len() {
            return None;
        }
    }
    Some(objects)
}
pub fn stream(bytes: &[u8]) -> Option<bool> {
    let mut depth = None;
    for (kind, payload) in subobjects(bytes)? {
        if matches!(
            kind,
            D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_DEPTH_STENCIL
                | D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_DEPTH_STENCIL1
                | D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_DEPTH_STENCIL2
        ) {
            if depth.is_some() {
                return None;
            }
            let enabled = i32::from_le_bytes(payload.get(..4)?.try_into().ok()?);
            let mask = i32::from_le_bytes(payload.get(4..8)?.try_into().ok()?);
            depth = Some(writes(BOOL(enabled), D3D12_DEPTH_WRITE_MASK(mask)));
        }
    }
    // No explicit depth subobject: do not infer an SDK helper's defaults.
    depth
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pso_stream_respects_payload_alignment_and_rejects_truncation_duplicates_and_unknowns() {
        let mut bytes = vec![0u8; 24]; // root signature (16), sample mask (8)
        bytes[16..20].copy_from_slice(
            &D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_SAMPLE_MASK
                .0
                .to_le_bytes(),
        );
        let start = bytes.len();
        bytes.resize(
            start + (4 + size_of::<D3D12_DEPTH_STENCIL_DESC1>()).next_multiple_of(8),
            0,
        );
        bytes[start..start + 4].copy_from_slice(
            &D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_DEPTH_STENCIL1
                .0
                .to_le_bytes(),
        );
        bytes[start + 4..start + 8].copy_from_slice(&1i32.to_le_bytes());
        bytes[start + 8..start + 12].copy_from_slice(&D3D12_DEPTH_WRITE_MASK_ALL.0.to_le_bytes());
        assert_eq!(stream(&bytes), Some(true));
        bytes[start + 8..start + 12].copy_from_slice(&0i32.to_le_bytes());
        assert_eq!(stream(&bytes), Some(false));
        assert_eq!(stream(&bytes[..bytes.len() - 1]), None);
        let duplicate = bytes[start..].to_vec();
        bytes.extend(duplicate);
        assert_eq!(stream(&bytes), None);
        assert_eq!(stream(&[255; 8]), None);
        assert_eq!(stream(&[]), None);
    }
}
