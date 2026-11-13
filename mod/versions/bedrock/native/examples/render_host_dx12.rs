#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use hudhook::windows::Win32::Foundation::*;
    use hudhook::windows::Win32::Graphics::Direct3D::*;
    use hudhook::windows::Win32::Graphics::Direct3D12::*;
    use hudhook::windows::Win32::Graphics::Dxgi::Common::*;
    use hudhook::windows::Win32::Graphics::Dxgi::*;
    use hudhook::windows::Win32::System::LibraryLoader::*;
    use hudhook::windows::Win32::UI::WindowsAndMessaging::*;
    use hudhook::windows::core::{Interface, s, w};
    use std::mem::ManuallyDrop;
    use std::time::{Duration, Instant};

    unsafe extern "system" fn procedure(hwnd: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        unsafe { DefWindowProcW(hwnd, message, w, l) }
    }
    unsafe fn transition(
        list: &ID3D12GraphicsCommandList,
        resource: &ID3D12Resource,
        before: D3D12_RESOURCE_STATES,
        after: D3D12_RESOURCE_STATES,
    ) {
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
        unsafe {
            list.ResourceBarrier(std::slice::from_ref(&barrier));
            ManuallyDrop::drop(&mut (*barrier.Anonymous.Transition).pResource);
        }
    }
    let bundle = std::path::PathBuf::from(std::env::args().nth(1).ok_or("bundle required")?)
        .canonicalize()?;
    let ffmpeg = std::env::args().nth(2).ok_or("FFmpeg required")?;
    let output = std::env::args().nth(3).ok_or("MP4 output required")?;
    // This HWND never receives WS_VISIBLE or ShowWindow. It uses an isolated
    // offline ArcticTest bundle; it neither starts Minecraft nor loads accounts.
    unsafe {
        let module = GetModuleHandleW(None)?;
        let class = WNDCLASSW {
            lpfnWndProc: Some(procedure),
            hInstance: module.into(),
            lpszClassName: w!("ArcticCaptureDx12Test"),
            ..Default::default()
        };
        if RegisterClassW(&class) == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let window = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class.lpszClassName,
            w!("Arctic headless capture"),
            WS_OVERLAPPEDWINDOW,
            0,
            0,
            318,
            178,
            None,
            None,
            Some(module.into()),
            None,
        )?;
        let factory: IDXGIFactory4 = CreateDXGIFactory1()?;
        let mut device: Option<ID3D12Device> = None;
        if D3D12CreateDevice(None, D3D_FEATURE_LEVEL_11_0, &mut device).is_err() {
            let warp: IDXGIAdapter = factory.EnumWarpAdapter()?;
            D3D12CreateDevice(&warp, D3D_FEATURE_LEVEL_11_0, &mut device)?;
        }
        let device = device.ok_or("DX12 device missing")?;
        let queue: ID3D12CommandQueue = device.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC {
            Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
            ..Default::default()
        })?;
        let mut swap: IDXGISwapChain3 = factory
            .CreateSwapChainForHwnd(
                &queue,
                window,
                &DXGI_SWAP_CHAIN_DESC1 {
                    Width: 318,
                    Height: 178,
                    Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                    BufferCount: 2,
                    SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
                    ..Default::default()
                },
                None,
                None,
            )?
            .cast()?;
        let heap: ID3D12DescriptorHeap =
            device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
                NumDescriptors: 3,
                ..Default::default()
            })?;
        let base = heap.GetCPUDescriptorHandleForHeapStart();
        let increment =
            device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV) as usize;
        let mut buffers = Vec::new();
        for index in 0..2 {
            let resource: ID3D12Resource = swap.GetBuffer(index)?;
            device.CreateRenderTargetView(
                &resource,
                None,
                D3D12_CPU_DESCRIPTOR_HANDLE {
                    ptr: base.ptr + index as usize * increment,
                },
            );
            buffers.push(resource);
        }
        let allocator: ID3D12CommandAllocator =
            device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        let list: ID3D12GraphicsCommandList =
            device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &allocator, None)?;
        list.Close()?;
        let fence: ID3D12Fence = device.CreateFence(0, D3D12_FENCE_FLAG_NONE)?;
        let loaded = std::process::Command::new(bundle.join("arctic-bedrock-loader.exe"))
            .arg(std::process::id().to_string())
            .arg(&bundle)
            .output()?;
        if !loaded.status.success() {
            return Err(String::from_utf8_lossy(&loaded.stderr).into_owned().into());
        }
        let dll = GetModuleHandleW(w!("arctic_bedrock.dll"))?;
        type Status = unsafe extern "C" fn() -> u64;
        let status: Status = std::mem::transmute(
            GetProcAddress(dll, s!("arctic_bedrock_capture_status_v1"))
                .ok_or("capture status export missing")?,
        );
        let started = Instant::now();
        let rendered: Status = std::mem::transmute(
            GetProcAddress(dll, s!("arctic_bedrock_renderer_frames_v1"))
                .ok_or("renderer frame counter missing")?,
        );
        let mut recording = false;
        let mut rejected_request_sent = false;
        let rejected_output = format!("{output}.rejected.mp4");
        let mut phase = 0;
        let mut current_output = output.clone();
        let mut current_rejected = rejected_output.clone();
        let mut dimensions = (318, 178);
        let mut value = 0;
        loop {
            let mut message = MSG::default();
            while PeekMessageW(&mut message, Some(window), 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            allocator.Reset()?;
            list.Reset(&allocator, None)?;
            let index = swap.GetCurrentBackBufferIndex() as usize;
            let resource = &buffers[index];
            transition(
                &list,
                resource,
                D3D12_RESOURCE_STATE_PRESENT,
                D3D12_RESOURCE_STATE_RENDER_TARGET,
            );
            let handle = D3D12_CPU_DESCRIPTOR_HANDLE {
                ptr: base.ptr + index * increment,
            };
            list.ClearRenderTargetView(handle, &[0.8, 0.1, 0.2, 1.0], None);
            list.ClearRenderTargetView(
                handle,
                &[0.1, 0.8, 0.2, 1.0],
                Some(&[RECT {
                    left: 0,
                    top: 0,
                    right: dimensions.0 / 2,
                    bottom: dimensions.1,
                }]),
            );
            transition(
                &list,
                resource,
                D3D12_RESOURCE_STATE_RENDER_TARGET,
                D3D12_RESOURCE_STATE_PRESENT,
            );
            list.Close()?;
            queue.ExecuteCommandLists(&[Some(list.cast()?)]);
            let before = rendered();
            swap.Present(0, DXGI_PRESENT_TEST).ok()?;
            swap.Present1(0, DXGI_PRESENT_TEST, &DXGI_PRESENT_PARAMETERS::default())
                .ok()?;
            if rendered() != before {
                return Err("TEST presents rendered or captured a frame".into());
            }
            swap.Present(0, DXGI_PRESENT(0)).ok()?;
            value += 1;
            queue.Signal(&fence, value)?;
            let wait = Instant::now();
            while fence.GetCompletedValue() < value {
                if wait.elapsed() > Duration::from_secs(5) {
                    return Err("DX12 host GPU timeout".into());
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            if !recording
                && bundle
                    .join(format!("arctic-bedrock-{}.ready", std::process::id()))
                    .is_file()
            {
                let command=serde_json::json!({"op":"start","ffmpeg":ffmpeg,"output":current_output,"fps":10,"frames":8}).to_string();
                let request = bundle.join(format!(
                    "arctic-bedrock-{}.capture-request.json",
                    std::process::id()
                ));
                let temporary = request.with_extension("json.tmp");
                std::fs::write(&temporary, command)?;
                std::fs::rename(temporary, request)?;
                recording = true;
            }
            let state = status();
            if recording && state as u32 == 2 && !rejected_request_sent {
                let command=serde_json::json!({"op":"start","ffmpeg":ffmpeg,"output":current_rejected,"fps":10,"frames":8}).to_string();
                let request = bundle.join(format!(
                    "arctic-bedrock-{}.capture-request.json",
                    std::process::id()
                ));
                let temporary = request.with_extension("json.tmp");
                std::fs::write(&temporary, command)?;
                std::fs::rename(temporary, request)?;
                rejected_request_sent = true;
            }
            if recording && state as u32 == 5 {
                return Err(format!(
                    "DX12 capture failed: {}",
                    std::fs::read_to_string(bundle.join(format!(
                        "arctic-bedrock-{}.capture.json",
                        std::process::id()
                    )))
                    .unwrap_or_default()
                )
                .into());
            }
            if recording && state as u32 == 4 {
                if state >> 32 != 8 {
                    return Err("DX12 capture lost frames".into());
                }
                let report = bundle.join(format!(
                    "arctic-bedrock-{}.capture.json",
                    std::process::id()
                ));
                let report = std::fs::read(&report)
                    .ok()
                    .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
                if !report
                    .as_ref()
                    .is_some_and(|report| report["state"] == "complete")
                {
                    continue;
                }
                let report = report.unwrap();
                if report["output"] != current_output {
                    continue;
                }
                if !rejected_request_sent
                    || report["requestOutput"] != current_rejected
                    || report["requestError"].as_str().is_none()
                    || std::path::Path::new(&current_rejected).exists()
                {
                    return Err(
                        "a rejected concurrent capture interfered with the active recording".into(),
                    );
                }
                println!("Native D3D12 capture phase {phase} passed: 8 actual swap-chain frames");
                if phase == 3 {
                    let log = std::fs::read_to_string(
                        bundle.join(format!("arctic-bedrock-{}.log", std::process::id())),
                    )?;
                    if log.matches("swap-chain renderer initialized").count() < 4 {
                        return Err("renderer did not reinitialize after all transitions".into());
                    }
                    std::process::exit(0);
                }
                buffers.clear();
                if phase == 0 {
                    // Change format and buffer count while keeping the HWND and
                    // swap-chain object. The overlay must rebuild and reupload
                    // the shared Java texture rather than retaining an old ID.
                    swap.ResizeBuffers(
                        3,
                        320,
                        180,
                        DXGI_FORMAT_R8G8B8A8_UNORM,
                        DXGI_SWAP_CHAIN_FLAG(0),
                    )?;
                    dimensions = (320, 180);
                } else if phase == 1 {
                    // Release the old flip chain before creating another on the
                    // same hidden HWND. A different identity exercises factory
                    // replacement, including its context-lock lifetime.
                    drop(swap);
                    swap = factory
                        .CreateSwapChainForHwnd(
                            &queue,
                            window,
                            &DXGI_SWAP_CHAIN_DESC1 {
                                Width: 318,
                                Height: 178,
                                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                                SampleDesc: DXGI_SAMPLE_DESC {
                                    Count: 1,
                                    Quality: 0,
                                },
                                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                                BufferCount: 2,
                                SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
                                ..Default::default()
                            },
                            None,
                            None,
                        )?
                        .cast()?;
                    dimensions = (318, 178);
                } else {
                    // A size-only change must also recreate the Java overlay
                    // texture, even though format and buffer count stay equal.
                    swap.ResizeBuffers(
                        2,
                        334,
                        188,
                        DXGI_FORMAT_B8G8R8A8_UNORM,
                        DXGI_SWAP_CHAIN_FLAG(0),
                    )?;
                    dimensions = (334, 188);
                }
                phase += 1;
                for index in 0..if phase == 1 { 3 } else { 2 } {
                    let resource: ID3D12Resource = swap.GetBuffer(index)?;
                    device.CreateRenderTargetView(
                        &resource,
                        None,
                        D3D12_CPU_DESCRIPTOR_HANDLE {
                            ptr: base.ptr + index as usize * increment,
                        },
                    );
                    buffers.push(resource);
                }
                current_output = format!("{output}.phase{phase}.mp4");
                current_rejected = format!("{current_output}.rejected.mp4");
                recording = false;
                rejected_request_sent = false;
            }
            if started.elapsed() > Duration::from_secs(50) {
                return Err("DX12 headless capture timed out".into());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
#[cfg(not(windows))]
fn main() {}
