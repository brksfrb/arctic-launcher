//! Read actual GPU depth into the software cosmetics renderer's conventional
//! near=0/far=1 depth convention. No all-far substitute is produced on failure.
use hudhook::windows::Win32::Graphics::Direct3D12::*;
use hudhook::windows::Win32::Graphics::Dxgi::Common::*;

#[derive(Clone, Copy, Debug)]
pub enum Encoding {
    Float32,
    Unorm24,
    Unorm16,
}

pub fn encoding(format: DXGI_FORMAT) -> Option<Encoding> {
    match format {
        DXGI_FORMAT_D32_FLOAT
        | DXGI_FORMAT_R32_TYPELESS
        | DXGI_FORMAT_D32_FLOAT_S8X24_UINT
        | DXGI_FORMAT_R32G8X24_TYPELESS => Some(Encoding::Float32),
        DXGI_FORMAT_D24_UNORM_S8_UINT | DXGI_FORMAT_R24G8_TYPELESS => Some(Encoding::Unorm24),
        DXGI_FORMAT_D16_UNORM | DXGI_FORMAT_R16_TYPELESS => Some(Encoding::Unorm16),
        _ => None,
    }
}

pub struct Depth {
    pub reverse_z: bool,
    pub width: u32,
    pub height: u32,
    pub values: Vec<f32>,
}

#[cfg(test)]
pub(crate) static GPU_TEST: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn decode(
    raw: crate::readback::RawTexture,
    encoding: Encoding,
    reverse_z: bool,
) -> Result<Depth, String> {
    if raw.width == 0
        || raw.height == 0
        || !raw.row_bytes.is_multiple_of(raw.width as usize)
        || raw.bytes.len() != raw.row_bytes * raw.height as usize
    {
        return Err("invalid depth row layout".into());
    }
    let stride = raw.row_bytes / raw.width as usize;
    let required = match encoding {
        Encoding::Unorm16 => 2,
        _ => 4,
    };
    if stride < required || stride > 8 {
        return Err("unsupported depth plane pixel size".into());
    }
    let mut values = Vec::with_capacity(raw.width as usize * raw.height as usize);
    for row in raw.bytes.chunks_exact(raw.row_bytes) {
        for pixel in row.chunks_exact(stride) {
            let d = match encoding {
                Encoding::Float32 => f32::from_le_bytes(pixel[..4].try_into().unwrap()),
                Encoding::Unorm24 => {
                    (u32::from_le_bytes(pixel[..4].try_into().unwrap()) & 0xffffff) as f32
                        / 0xffffff as f32
                }
                Encoding::Unorm16 => {
                    u16::from_le_bytes(pixel[..2].try_into().unwrap()) as f32 / 65535.
                }
            };
            if !d.is_finite() || !(0.0..=1.0).contains(&d) {
                return Err("GPU depth contains invalid values".into());
            }
            values.push(if reverse_z { 1. - d } else { d });
        }
    }
    Ok(Depth {
        reverse_z,
        width: raw.width,
        height: raw.height,
        values,
    })
}

impl Depth {
    /// Cover the complete source footprint of each target pixel. Minimum depth
    /// preserves thin walls when reducing the surface, including fractional
    /// boundaries. The same footprint rule also supports a native depth plane
    /// smaller than the presentation surface; no invented far-depth pixels.
    pub fn downsample(&self, width: u32, height: u32) -> Result<Vec<f32>, String> {
        if width == 0
            || height == 0
            || width > 3840
            || height > 2160
            || self.width == 0
            || self.height == 0
            || self.values.len() != self.width as usize * self.height as usize
        {
            return Err("invalid depth downsample dimensions".into());
        }
        if width == self.width && height == self.height {
            if self
                .values
                .iter()
                .any(|d| !d.is_finite() || !(0.0..=1.0).contains(d))
            {
                return Err("invalid source depth".into());
            }
            return Ok(self.values.clone());
        }
        let mut output = vec![1.; width as usize * height as usize];
        for y in 0..height {
            let top = y as u64 * self.height as u64 / height as u64;
            let bottom = ((y + 1) as u64 * self.height as u64).div_ceil(height as u64);
            for x in 0..width {
                let left = x as u64 * self.width as u64 / width as u64;
                let right = ((x + 1) as u64 * self.width as u64).div_ceil(width as u64);
                let mut nearest = 1f32;
                for sy in top..bottom {
                    for sx in left..right {
                        let d = self.values[sy as usize * self.width as usize + sx as usize];
                        if !d.is_finite() || !(0.0..=1.0).contains(&d) {
                            return Err("invalid source depth".into());
                        }
                        nearest = nearest.min(d);
                    }
                }
                output[y as usize * width as usize + x as usize] = nearest;
            }
        }
        Ok(output)
    }
}

/// The caller must supply an actually observed state and the resource's owning
/// direct queue. This function copies depth plane zero and restores its state.
pub fn dx12(
    resource: ID3D12Resource,
    queue: &ID3D12CommandQueue,
    state: D3D12_RESOURCE_STATES,
    reverse_z: bool,
) -> Result<Depth, String> {
    // SAFETY: the owned COM reference keeps the description valid.
    let desc = unsafe { resource.GetDesc() };
    if desc.Flags & D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL == D3D12_RESOURCE_FLAGS(0) {
        return Err("resource is not a depth buffer".into());
    }
    let encoding = encoding(desc.Format).ok_or("unsupported GPU depth format")?;
    decode(
        crate::readback::texture_dx12(resource, queue, state)?,
        encoding,
        reverse_z,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::readback::RawTexture;
    #[test]
    fn decodes_depth_planes_reverse_z_and_preserves_thin_occluders() {
        let raw = RawTexture {
            width: 3,
            height: 1,
            row_bytes: 12,
            bytes: [0.8f32, 0.1, 1.]
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect(),
        };
        let depth = decode(raw, Encoding::Float32, false).unwrap();
        assert_eq!(depth.downsample(2, 1).unwrap(), vec![0.1, 0.1]);
        let raw = RawTexture {
            width: 1,
            height: 1,
            row_bytes: 4,
            bytes: 0.25f32.to_le_bytes().to_vec(),
        };
        assert_eq!(
            decode(raw, Encoding::Float32, true).unwrap().values,
            vec![0.75]
        );
        let raw = RawTexture {
            width: 1,
            height: 1,
            row_bytes: 4,
            bytes: 0xffffffffu32.to_le_bytes().to_vec(),
        };
        assert_eq!(
            decode(raw, Encoding::Unorm24, false).unwrap().values,
            vec![1.]
        );
        let raw = RawTexture {
            width: 1,
            height: 1,
            row_bytes: 4,
            bytes: f32::NAN.to_le_bytes().to_vec(),
        };
        assert!(decode(raw, Encoding::Float32, false).is_err());
    }
    #[test]
    fn real_gpu_depth_readback_handles_all_depth_families_and_padded_rows()
    -> Result<(), Box<dyn std::error::Error>> {
        let _gpu = GPU_TEST.lock().unwrap();
        use hudhook::windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0;
        use hudhook::windows::Win32::Graphics::Dxgi::{
            CreateDXGIFactory1, IDXGIAdapter, IDXGIFactory4,
        };
        use hudhook::windows::core::Interface;
        // WARP makes this test independent of a visible desktop or physical GPU.
        unsafe {
            let factory: IDXGIFactory4 = CreateDXGIFactory1()?;
            let adapter: IDXGIAdapter = factory.EnumWarpAdapter()?;
            let mut device: Option<ID3D12Device> = None;
            D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_11_0, &mut device)?;
            let device = device.ok_or("no WARP device")?;
            let queue: ID3D12CommandQueue =
                device.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC {
                    Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
                    ..Default::default()
                })?;
            let heap: ID3D12DescriptorHeap =
                device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                    Type: D3D12_DESCRIPTOR_HEAP_TYPE_DSV,
                    NumDescriptors: 1,
                    ..Default::default()
                })?;
            let handle = heap.GetCPUDescriptorHandleForHeapStart();
            for format in [
                DXGI_FORMAT_D16_UNORM,
                DXGI_FORMAT_D24_UNORM_S8_UINT,
                DXGI_FORMAT_D32_FLOAT,
                DXGI_FORMAT_D32_FLOAT_S8X24_UINT,
            ] {
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
                        Format: format,
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
                let resource = resource.ok_or("depth resource missing")?;
                device.CreateDepthStencilView(&resource, None, handle);
                let allocator: ID3D12CommandAllocator =
                    device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
                let list: ID3D12GraphicsCommandList = device.CreateCommandList(
                    0,
                    D3D12_COMMAND_LIST_TYPE_DIRECT,
                    &allocator,
                    None,
                )?;
                list.ClearDepthStencilView(handle, D3D12_CLEAR_FLAG_DEPTH, 0.75, 0, None);
                list.ClearDepthStencilView(
                    handle,
                    D3D12_CLEAR_FLAG_DEPTH,
                    0.25,
                    0,
                    Some(&[hudhook::windows::Win32::Foundation::RECT {
                        left: 0,
                        top: 0,
                        right: 159,
                        bottom: 178,
                    }]),
                );
                list.Close()?;
                queue.ExecuteCommandLists(&[Some(list.cast()?)]);
                // Readback executes behind the actual depth clears and waits for
                // its GPU fence, so the test needs no guessed sleep/frame delay.
                let depth = dx12(
                    resource.clone(),
                    &queue,
                    D3D12_RESOURCE_STATE_DEPTH_WRITE,
                    false,
                )?;
                assert_eq!((depth.width, depth.height), (318, 178));
                assert!(
                    (depth.values[10] - 0.25).abs() < 0.0001,
                    "format {}",
                    format.0
                );
                assert!(
                    (depth.values[300] - 0.75).abs() < 0.0001,
                    "format {}",
                    format.0
                );
                let reverse = dx12(resource, &queue, D3D12_RESOURCE_STATE_DEPTH_WRITE, true)?;
                assert!((reverse.values[300] - 0.25).abs() < 0.0001);
            }
        }
        Ok(())
    }
}
