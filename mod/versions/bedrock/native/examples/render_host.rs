#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use hudhook::windows::Win32::Foundation::*;
    use hudhook::windows::Win32::Graphics::Direct3D::*;
    use hudhook::windows::Win32::Graphics::Direct3D11::*;
    use hudhook::windows::Win32::Graphics::Dxgi::Common::*;
    use hudhook::windows::Win32::Graphics::Dxgi::*;
    use hudhook::windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
    use hudhook::windows::Win32::UI::WindowsAndMessaging::*;
    use hudhook::windows::core::{s, w};

    unsafe extern "system" fn procedure(hwnd: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        if message == WM_DESTROY {
            unsafe { PostQuitMessage(0) };
            return LRESULT(0);
        }
        unsafe { DefWindowProcW(hwnd, message, w, l) }
    }

    let bundle = std::path::PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("usage: render_host <bundle directory>")?,
    )
    .canonicalize()?;
    // A hidden, disposable D3D11 process exercises the real injected DLL without
    // attaching experimental hooks to a user's world.
    unsafe {
        let module = GetModuleHandleW(None)?;
        let class = WNDCLASSW {
            lpfnWndProc: Some(procedure),
            hInstance: module.into(),
            lpszClassName: w!("ArcticBedrockRenderTest"),
            ..Default::default()
        };
        if RegisterClassW(&class) == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let window = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class.lpszClassName,
            w!("Arctic native renderer test"),
            WS_OVERLAPPEDWINDOW,
            0,
            0,
            1600,
            900,
            None,
            None,
            Some(module.into()),
            None,
        )?;
        let mut swap: Option<IDXGISwapChain> = None;
        let mut device: Option<ID3D11Device> = None;
        let mut context: Option<ID3D11DeviceContext> = None;
        let mut create_device = |driver| {
            D3D11CreateDeviceAndSwapChain(
                None,
                driver,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_FLAG(0),
                Some(&[D3D_FEATURE_LEVEL_11_0]),
                D3D11_SDK_VERSION,
                Some(&DXGI_SWAP_CHAIN_DESC {
                    BufferDesc: DXGI_MODE_DESC {
                        Width: 1600,
                        Height: 900,
                        Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                        ..Default::default()
                    },
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                    BufferCount: 2,
                    OutputWindow: window,
                    Windowed: true.into(),
                    SwapEffect: DXGI_SWAP_EFFECT_DISCARD,
                    ..Default::default()
                }),
                Some(&mut swap),
                Some(&mut device),
                None,
                Some(&mut context),
            )
        };
        if let Err(hardware) = create_device(D3D_DRIVER_TYPE_HARDWARE) {
            // CI/headless machines may have no hardware graphics adapter.
            create_device(D3D_DRIVER_TYPE_WARP)
                .map_err(|warp| format!("D3D11 hardware: {hardware}; WARP: {warp}"))?;
        }
        let swap = swap.ok_or("missing swap chain")?;
        let device = device.ok_or("missing device")?;
        let context = context.ok_or("missing context")?;
        let back: ID3D11Texture2D = swap.GetBuffer(0)?;
        let mut target = None;
        device.CreateRenderTargetView(&back, None, Some(&mut target))?;
        let target = target.ok_or("missing render target")?;
        println!("renderer test PID: {}", std::process::id());
        let loader = bundle.join("arctic-bedrock-loader.exe");
        let result = std::process::Command::new(loader)
            .arg(std::process::id().to_string())
            .arg(&bundle)
            .output()?;
        if !result.status.success() {
            return Err(String::from_utf8_lossy(&result.stderr).into_owned().into());
        }
        let duplicate = std::process::Command::new(bundle.join("arctic-bedrock-loader.exe"))
            .arg(std::process::id().to_string())
            .arg(&bundle)
            .output()?;
        if duplicate.status.success()
            || !String::from_utf8_lossy(&duplicate.stderr).contains("already loaded")
        {
            return Err(format!(
                "loader did not reject a duplicate native client: {}; stderr: {}",
                duplicate.status,
                String::from_utf8_lossy(&duplicate.stderr)
            )
            .into());
        }
        let ready = bundle.join(format!("arctic-bedrock-{}.ready", std::process::id()));
        let dll = GetModuleHandleW(w!("arctic_bedrock.dll"))?;
        type Surface = unsafe extern "C" fn() -> u64;
        type World = unsafe extern "C" fn(*const u8, usize, *const f32, usize) -> i32;
        type Texture = unsafe extern "C" fn(*const u8, usize, u32, u32, *const u8, usize) -> i32;
        type Clear = unsafe extern "C" fn();
        type Capture = unsafe extern "C" fn(*const u8, usize) -> i32;
        type CaptureStatus = unsafe extern "C" fn() -> u64;
        let capture: Capture = std::mem::transmute(
            GetProcAddress(dll, s!("arctic_bedrock_capture_v1")).ok_or("capture ABI missing")?,
        );
        let capture_status: CaptureStatus = std::mem::transmute(
            GetProcAddress(dll, s!("arctic_bedrock_capture_status_v1"))
                .ok_or("capture status ABI missing")?,
        );
        let capture_options = std::env::args().nth(2).map(|ffmpeg| {
            serde_json::json!({"op":"start","ffmpeg":ffmpeg,
            "output":std::env::args().nth(3).expect("capture output required"),"fps":10,"frames":8})
            .to_string()
        });
        let mut capture_started = false;
        let surface: Surface = std::mem::transmute(
            GetProcAddress(dll, s!("arctic_bedrock_surface_v1")).ok_or("surface ABI missing")?,
        );
        let world: World = std::mem::transmute(
            GetProcAddress(dll, s!("arctic_bedrock_world_v1")).ok_or("world ABI missing")?,
        );
        let texture: Texture = std::mem::transmute(
            GetProcAddress(dll, s!("arctic_bedrock_texture_v1")).ok_or("texture ABI missing")?,
        );
        let clear: Clear = std::mem::transmute(
            GetProcAddress(dll, s!("arctic_bedrock_clear_world_v1")).ok_or("clear ABI missing")?,
        );
        let skin: Vec<u8> = [40, 80, 240, 255].repeat(64 * 64);
        let key = b"engine:fixture";
        if texture(key.as_ptr(), key.len(), 64, 64, skin.as_ptr(), skin.len()) != 0 {
            return Err("engine fixture texture rejected".into());
        }
        let cape: Vec<u8> = [240, 40, 60, 255].repeat(64 * 32);
        let cape_key = b"engine:cape";
        if texture(
            cape_key.as_ptr(),
            cape_key.len(),
            64,
            32,
            cape.as_ptr(),
            cape.len(),
        ) != 0
        {
            return Err("engine fixture cape rejected".into());
        }
        let started = std::time::Instant::now();
        let mut menu_at = None;
        let mut world_at = None;
        let mut cleared_at = None;
        let mut sequence = 0u64;
        loop {
            let mut message = MSG::default();
            while PeekMessageW(&mut message, Some(window), 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            if ready.is_file() && cleared_at.is_none() {
                let size = surface();
                let width = (size >> 32) as usize;
                let height = size as u32 as usize;
                if width > 0 && height > 0 {
                    sequence += 1;
                    let json=serde_json::json!({
                        "schema":1,"width":width,"height":height,"sequence":sequence,"clockMs":125,
                        "session":"offline-fixture","server":"headless-offline","dimension":"overworld",
                        "localPlayer":"b50ad385-829d-3141-a216-7e7d7539ba7f",
                        "camera":[0,0,0,0,0,70],
                        "viewProjection":[1,0,0,0,0,-1,0,0,0,0,0.01,0,0,0.5,0.5,1],
                        "players":[{"id":"b50ad385-829d-3141-a216-7e7d7539ba7f","name":"Notch",
                            "skin":"engine:fixture","position":[0,0,0,0,0],
                            "cape":"engine:cape","capeMotion":{"delta":[0,0.4,-0.5],"bodyYaw":0,"walkDistance":0.2,"bob":0.1,"fallFlyingScale":0},
                            "model":[-1,0,0,0,0,1,0,0,0,0,-1,0],
                            "replaceBody":true,"replaceCape":true,"visible":true,"slim":false}]
                    }).to_string();
                    let mut depth = vec![1f32; width * height];
                    for row in depth.chunks_mut(width) {
                        row[..width / 2].fill(0f32);
                    }
                    if world(json.as_ptr(), json.len(), depth.as_ptr(), depth.len()) != 0 {
                        return Err("engine fixture frame rejected".into());
                    }
                    world_at.get_or_insert_with(std::time::Instant::now);
                }
            }
            context.OMSetRenderTargets(Some(&[Some(target.clone())]), None);
            context.ClearRenderTargetView(&target, &[0.025, 0.04, 0.06, 1.0]);
            swap.Present(1, DXGI_PRESENT(0)).ok()?;
            let inspect_world = cleared_at.is_none()
                && world_at.is_some_and(|at: std::time::Instant| at.elapsed().as_secs() >= 2);
            let inspect_clear = menu_at.is_none()
                && cleared_at.is_some_and(|at: std::time::Instant| at.elapsed().as_secs() >= 1);
            let inspect_menu =
                menu_at.is_some_and(|at: std::time::Instant| at.elapsed().as_secs() >= 2);
            if inspect_world || inspect_clear || inspect_menu {
                // Inspect actual GPU output, not just a successful LoadLibrary call.
                let mut desc = D3D11_TEXTURE2D_DESC::default();
                back.GetDesc(&mut desc);
                desc.Usage = D3D11_USAGE_STAGING;
                desc.BindFlags = 0;
                desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
                desc.MiscFlags = 0;
                let mut staging = None;
                device.CreateTexture2D(&desc, None, Some(&mut staging))?;
                let staging = staging.ok_or("missing staging texture")?;
                context.CopyResource(&staging, &back);
                let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
                context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;
                let mut changed = 0;
                let mut blue_left = 0;
                let mut blue_right = 0;
                let mut red_left = 0;
                let mut red_right = 0;
                for y in 0..desc.Height as usize {
                    let row = std::slice::from_raw_parts(
                        (mapped.pData as *const u8).add(y * mapped.RowPitch as usize),
                        desc.Width as usize * 4,
                    );
                    changed += row
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .filter(|rgba| rgba[0] > 32 || rgba[1] > 32 || rgba[2] > 32)
                        .count();
                    // Exclude the HUD badge at the bottom. Fixture blue is
                    // distinguishable from the clear color and UI text.
                    if y < desc.Height as usize - 80 {
                        for (x, rgba) in row.as_chunks::<4>().0.iter().enumerate() {
                            if rgba[2] > 80 && u16::from(rgba[2]) > u16::from(rgba[0]) * 2 {
                                if x < desc.Width as usize / 2 {
                                    blue_left += 1;
                                } else {
                                    blue_right += 1;
                                }
                            }
                            if rgba[0] > 80 && u16::from(rgba[0]) > u16::from(rgba[2]) * 2 {
                                if x < desc.Width as usize / 2 {
                                    red_left += 1;
                                } else {
                                    red_right += 1;
                                }
                            }
                        }
                    }
                }
                context.Unmap(&staging, 0);
                if inspect_world {
                    if blue_left != 0 || blue_right < 5000 || red_left != 0 || red_right < 5000 {
                        return Err(format!("world/depth GPU check failed: skin left {blue_left}, right {blue_right}; cape left {red_left}, right {red_right}").into());
                    }
                    if let Some(options) = &capture_options {
                        if !capture_started {
                            if capture(options.as_ptr(), options.len()) != 0 {
                                return Err("capture start rejected".into());
                            }
                            capture_started = true;
                        }
                        let state = capture_status();
                        if state as u32 == 5 {
                            return Err(format!(
                                "native capture failed: {}",
                                std::fs::read_to_string(bundle.join(format!(
                                    "arctic-bedrock-{}.capture.json",
                                    std::process::id()
                                )))
                                .unwrap_or_default()
                            )
                            .into());
                        }
                        if state as u32 != 4 {
                            continue;
                        }
                        if state >> 32 != 8 {
                            return Err("capture lost frames".into());
                        }
                        println!("Native D3D11/MP4 capture completed: 8 actual swap-chain frames");
                    }
                    println!(
                        "Native engine ABI/skin/terrain-depth smoke passed: {blue_right} visible, {blue_left} occluded-side pixels"
                    );
                    println!(
                        "Native cape physics/depth smoke passed: {red_right} visible, {red_left} occluded-side pixels"
                    );
                    clear();
                    cleared_at = Some(std::time::Instant::now());
                    continue;
                }
                if inspect_clear {
                    if blue_left + blue_right + red_left + red_right != 0 {
                        return Err("world pixels survived clearing the engine snapshot".into());
                    }
                    println!("Native disconnect clearing smoke passed");
                    PostMessageW(Some(window), WM_KEYDOWN, WPARAM(0xA1), LPARAM(0))?;
                    menu_at = Some(std::time::Instant::now());
                    continue;
                }
                if changed < 1000 {
                    return Err(format!(
                        "native client rendered no visible menu ({changed} changed pixels)"
                    )
                    .into());
                }
                println!("Native injection/JVM/D3D11/menu smoke passed: {changed} changed pixels");
                DestroyWindow(window)?;
                // The embedded JVM has service threads. Exit with the host, as the
                // game would; unloading a live JVM and its callbacks isn't supported.
                std::process::exit(0);
            }
            if started.elapsed().as_secs() > 45 {
                return Err(format!(
                    "native client didn't render; inspect {}",
                    bundle
                        .join(format!("arctic-bedrock-{}.log", std::process::id()))
                        .display()
                )
                .into());
            }
        }
    }
}

#[cfg(not(windows))]
fn main() {}
