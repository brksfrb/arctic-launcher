//! Swap-chain readback on the presenting thread, after Arctic's overlay.
use hudhook::windows::Win32::Graphics::Direct3D11::*;
use hudhook::windows::Win32::Graphics::Direct3D12::*;
use hudhook::windows::Win32::Graphics::Dxgi::Common::*;
use hudhook::windows::Win32::Graphics::Dxgi::*;
use hudhook::windows::core::Interface;
use std::mem::ManuallyDrop;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

fn format(format: DXGI_FORMAT) -> Result<bool, String> {
    match format {
        DXGI_FORMAT_R8G8B8A8_UNORM | DXGI_FORMAT_R8G8B8A8_UNORM_SRGB => Ok(false),
        DXGI_FORMAT_B8G8R8A8_UNORM | DXGI_FORMAT_B8G8R8A8_UNORM_SRGB => Ok(true),
        _ => Err(format!(
            "MP4 capture does not support HDR/packed swap-chain format {}",
            format.0
        )),
    }
}
fn size(width: u64, height: u32) -> Result<(u32, usize), String> {
    if !(2..=3840).contains(&width) || !(2..=2160).contains(&height) {
        return Err("capture supports dimensions 2×2 through 3840×2160".into());
    }
    Ok((width as u32, width as usize * height as usize * 4))
}
unsafe fn pixels(
    data: *const u8,
    width: u32,
    height: u32,
    pitch: usize,
    bgra: bool,
) -> Result<Vec<u8>, String> {
    let (_, length) = size(width as u64, height)?;
    if data.is_null() || pitch < width as usize * 4 {
        return Err("invalid GPU readback pitch/pointer".into());
    }
    let mut pixels = vec![0; length];
    for y in 0..height as usize {
        // SAFETY: the mapped GPU allocation has height rows with the reported pitch.
        let row = unsafe { std::slice::from_raw_parts(data.add(y * pitch), width as usize * 4) };
        pixels[y * width as usize * 4..(y + 1) * width as usize * 4].copy_from_slice(row);
    }
    for pixel in pixels.as_chunks_mut::<4>().0 {
        if bgra {
            pixel.swap(0, 2);
        }
        pixel[3] = 255;
    }
    Ok(pixels)
}

pub fn dx11(swap: &IDXGISwapChain) -> Result<(u32, u32, Vec<u8>), String> {
    // SAFETY: hudhook supplies a live swap chain on its presenting thread. Map
    // stays valid until Unmap; the back buffer is not retained across frames.
    unsafe {
        let device: ID3D11Device = swap.GetDevice().map_err(|e| e.to_string())?;
        let context = device.GetImmediateContext().map_err(|e| e.to_string())?;
        let back: ID3D11Texture2D = swap.GetBuffer(0).map_err(|e| e.to_string())?;
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        back.GetDesc(&mut desc);
        size(desc.Width as u64, desc.Height)?;
        let bgra = format(desc.Format)?;
        if desc.SampleDesc.Count != 1 {
            return Err("multisampled swap-chain capture is unsupported".into());
        }
        desc.Usage = D3D11_USAGE_STAGING;
        desc.BindFlags = 0;
        desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
        desc.MiscFlags = 0;
        let mut staging = None;
        device
            .CreateTexture2D(&desc, None, Some(&mut staging))
            .map_err(|e| e.to_string())?;
        let staging = staging.ok_or("missing DX11 readback resource")?;
        context.CopyResource(&staging, &back);
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        context
            .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
            .map_err(|e| e.to_string())?;
        let rgba = pixels(
            mapped.pData.cast(),
            desc.Width,
            desc.Height,
            mapped.RowPitch as usize,
            bgra,
        );
        context.Unmap(&staging, 0);
        Ok((desc.Width, desc.Height, rgba?))
    }
}

struct Barrier(D3D12_RESOURCE_BARRIER);
impl Barrier {
    fn subresource(
        resource: &ID3D12Resource,
        before: D3D12_RESOURCE_STATES,
        after: D3D12_RESOURCE_STATES,
        subresource: u32,
    ) -> Self {
        Self(D3D12_RESOURCE_BARRIER {
            Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
            Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
            Anonymous: D3D12_RESOURCE_BARRIER_0 {
                Transition: ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                    pResource: ManuallyDrop::new(Some(resource.clone())),
                    Subresource: subresource,
                    StateBefore: before,
                    StateAfter: after,
                }),
            },
        })
    }
}
impl Drop for Barrier {
    fn drop(&mut self) {
        unsafe {
            ManuallyDrop::drop(&mut (*self.0.Anonymous.Transition).pResource);
        }
    }
}
struct Location(D3D12_TEXTURE_COPY_LOCATION);
impl Drop for Location {
    fn drop(&mut self) {
        unsafe {
            ManuallyDrop::drop(&mut self.0.pResource);
        }
    }
}
static DX12_FAILED: AtomicBool = AtomicBool::new(false);

pub fn dx12(
    swap: &IDXGISwapChain3,
    queue: &ID3D12CommandQueue,
) -> Result<(u32, u32, Vec<u8>), String> {
    // SAFETY: hudhook supplies the current back buffer and its direct queue.
    let back: ID3D12Resource =
        unsafe { swap.GetBuffer(swap.GetCurrentBackBufferIndex()) }.map_err(|e| e.to_string())?;
    let desc = unsafe { back.GetDesc() };
    size(desc.Width, desc.Height)?;
    let bgra = format(desc.Format)?;
    let mut raw = texture_dx12(back, queue, D3D12_RESOURCE_STATE_PRESENT)?;
    if raw.row_bytes != raw.width as usize * 4 {
        return Err("unexpected color readback row size".into());
    }
    for pixel in raw.bytes.as_chunks_mut::<4>().0 {
        if bgra {
            pixel.swap(0, 2);
        }
        pixel[3] = 255;
    }
    Ok((raw.width, raw.height, raw.bytes))
}

pub(crate) struct RawTexture {
    pub width: u32,
    pub height: u32,
    pub row_bytes: usize,
    pub bytes: Vec<u8>,
}

/// Copy plane zero into owned, tightly packed rows. The caller must know the
/// resource's actual state and synchronize ownership with the submitting queue.
/// Only the copied plane is transitioned; a stencil plane is left untouched.
pub(crate) fn texture_dx12(
    back: ID3D12Resource,
    queue: &ID3D12CommandQueue,
    state: D3D12_RESOURCE_STATES,
) -> Result<RawTexture, String> {
    if DX12_FAILED.load(Ordering::Acquire) {
        return Err("DX12 capture was disabled after a GPU timeout; restart the process".into());
    }
    // SAFETY: caller supplies a live resource, its actual state and direct queue.
    // The same resource plane is restored before the command list is submitted.
    unsafe {
        let mut device: Option<ID3D12Device> = None;
        back.GetDevice(&mut device).map_err(|e| e.to_string())?;
        let device = device.ok_or("resource device missing")?;
        let mut queue_device: Option<ID3D12Device> = None;
        queue
            .GetDevice(&mut queue_device)
            .map_err(|e| e.to_string())?;
        let queue_device = queue_device.ok_or("queue device missing")?;
        if device != queue_device || queue.GetDesc().Type != D3D12_COMMAND_LIST_TYPE_DIRECT {
            return Err("readback queue and resource must belong to the same direct device".into());
        }
        let desc = back.GetDesc();
        if !(1..=8192).contains(&desc.Width)
            || !(1..=8192).contains(&desc.Height)
            || desc.Dimension != D3D12_RESOURCE_DIMENSION_TEXTURE2D
            || desc.SampleDesc.Count != 1
            || desc.DepthOrArraySize != 1
            || desc.MipLevels != 1
        {
            return Err("unsupported DX12 capture resource".into());
        }
        let width = desc.Width as u32;
        let mut footprint = D3D12_PLACED_SUBRESOURCE_FOOTPRINT::default();
        let mut total = 0;
        let mut row_bytes = 0;
        let mut rows = 0;
        device.GetCopyableFootprints(
            &desc,
            0,
            1,
            0,
            Some(&mut footprint),
            Some(&mut rows),
            Some(&mut row_bytes),
            Some(&mut total),
        );
        let pitch = footprint.Footprint.RowPitch as u64;
        if total > 64 * 1024 * 1024
            || rows != desc.Height
            || row_bytes == 0
            || row_bytes > desc.Width * 16
            || pitch < row_bytes
            || footprint.Offset + pitch * (rows as u64 - 1) + row_bytes > total
        {
            return Err("invalid DX12 readback allocation size".into());
        }
        let mut readback: Option<ID3D12Resource> = None;
        device
            .CreateCommittedResource(
                &D3D12_HEAP_PROPERTIES {
                    Type: D3D12_HEAP_TYPE_READBACK,
                    CreationNodeMask: 1,
                    VisibleNodeMask: 1,
                    ..Default::default()
                },
                D3D12_HEAP_FLAG_NONE,
                &D3D12_RESOURCE_DESC {
                    Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
                    Width: total,
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
                D3D12_RESOURCE_STATE_COPY_DEST,
                None,
                &mut readback,
            )
            .map_err(|e| e.to_string())?;
        let readback = readback.ok_or("missing DX12 readback resource")?;
        let allocator: ID3D12CommandAllocator = device
            .CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)
            .map_err(|e| e.to_string())?;
        let list: ID3D12GraphicsCommandList = device
            .CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &allocator, None)
            .map_err(|e| e.to_string())?;
        let before = Barrier::subresource(&back, state, D3D12_RESOURCE_STATE_COPY_SOURCE, 0);
        let after = Barrier::subresource(&back, D3D12_RESOURCE_STATE_COPY_SOURCE, state, 0);
        if state != D3D12_RESOURCE_STATE_COPY_SOURCE {
            list.ResourceBarrier(std::slice::from_ref(&before.0));
        }
        let source = Location(D3D12_TEXTURE_COPY_LOCATION {
            pResource: ManuallyDrop::new(Some(back.clone())),
            Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
            Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                SubresourceIndex: 0,
            },
        });
        let destination = Location(D3D12_TEXTURE_COPY_LOCATION {
            pResource: ManuallyDrop::new(Some(readback.clone())),
            Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
            Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                PlacedFootprint: footprint,
            },
        });
        list.CopyTextureRegion(&destination.0, 0, 0, 0, &source.0, None);
        if state != D3D12_RESOURCE_STATE_COPY_SOURCE {
            list.ResourceBarrier(std::slice::from_ref(&after.0));
        }
        list.Close().map_err(|e| e.to_string())?;
        let fence: ID3D12Fence = device
            .CreateFence(0, D3D12_FENCE_FLAG_NONE)
            .map_err(|e| e.to_string())?;
        queue.ExecuteCommandLists(&[Some(list.cast().map_err(|e| e.to_string())?)]);
        let signaled = queue.Signal(&fence, 1);
        let at = Instant::now();
        while fence.GetCompletedValue() < 1
            && signaled.is_ok()
            && at.elapsed() < Duration::from_secs(2)
        {
            std::thread::sleep(Duration::from_millis(1));
        }
        if signaled.is_err() || fence.GetCompletedValue() < 1 {
            DX12_FAILED.store(true, Ordering::Release);
            let retained_queue = queue.clone();
            // Keep resources alive until the submitted GPU copy retires. Never
            // release in-flight resources on a timeout or block the render loop forever.
            std::thread::spawn(move || {
                while fence.GetCompletedValue() < 1 && device.GetDeviceRemovedReason().is_ok() {
                    std::thread::sleep(Duration::from_millis(100));
                }
                drop((readback, back, allocator, list, retained_queue));
            });
            return Err("DX12 capture copy did not retire within two seconds".into());
        }
        let mut data = std::ptr::null_mut();
        readback
            .Map(
                0,
                Some(&D3D12_RANGE {
                    Begin: 0,
                    End: total as usize,
                }),
                Some(&mut data),
            )
            .map_err(|e| e.to_string())?;
        if data.is_null() {
            readback.Unmap(0, Some(&D3D12_RANGE { Begin: 0, End: 0 }));
            return Err("GPU readback returned a null mapping".into());
        }
        let mut bytes = vec![0u8; row_bytes as usize * rows as usize];
        for y in 0..rows as usize {
            let row = std::slice::from_raw_parts(
                (data as *const u8).add(footprint.Offset as usize + y * pitch as usize),
                row_bytes as usize,
            );
            bytes[y * row_bytes as usize..(y + 1) * row_bytes as usize].copy_from_slice(row);
        }
        readback.Unmap(0, Some(&D3D12_RANGE { Begin: 0, End: 0 }));
        Ok(RawTexture {
            width,
            height: desc.Height,
            row_bytes: row_bytes as usize,
            bytes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strips_gpu_row_padding_converts_bgra_and_sets_video_alpha() {
        let data = [
            30, 20, 10, 9, 60, 50, 40, 9, 99, 99, 99, 99, 99, 99, 99, 99, 90, 80, 70, 9, 120, 110,
            100, 9, 99, 99, 99, 99, 99, 99, 99, 99,
        ];
        let actual = unsafe { pixels(data.as_ptr(), 2, 2, 16, true) }.unwrap();
        assert_eq!(
            actual,
            vec![
                10, 20, 30, 255, 40, 50, 60, 255, 70, 80, 90, 255, 100, 110, 120, 255
            ]
        );
        assert!(unsafe { pixels(data.as_ptr(), 2, 2, 4, true) }.is_err());
        assert!(format(DXGI_FORMAT_R10G10B10A2_UNORM).is_err());
    }
}
