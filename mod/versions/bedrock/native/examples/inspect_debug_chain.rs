//! Headless launch-chain inspection. Default mode stops at CREATE_PROCESS and
//! kills the owned target. Its game entry point is never executed, and no
//! account/world test is claimed.
//! Optional --inject-bundle advances the system loader to an entry-point trap,
//! then checks Arctic initialization with the game entry point still suspended.
//! This is not gameplay, and no world/account behavior is inferred from it.
//! --isolated-boot additionally resumes the owned game only after its fresh
//! profile, network denial and GDK-user denial guards are initialized.
#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
#[path = "../src/loader.rs"]
mod loader;

#[cfg(windows)]
#[path = "support/debug_notifications.rs"]
mod debug_notifications;
#[cfg(windows)]
#[path = "support/desktop.rs"]
mod desktop;

#[cfg(windows)]
mod windows_probe {
    use crate::desktop::startup_desktop;
    use hudhook::windows::Win32::Foundation::{
        CloseHandle, DBG_CONTINUE, DBG_EXCEPTION_NOT_HANDLED, HANDLE, HWND, LPARAM, RECT, WPARAM,
    };
    use hudhook::windows::Win32::System::Diagnostics::Debug::*;
    use hudhook::windows::Win32::System::Memory::{
        PAGE_EXECUTE_READWRITE, PAGE_PROTECTION_FLAGS, VirtualProtectEx,
    };
    use hudhook::windows::Win32::System::Threading::*;
    use hudhook::windows::Win32::UI::Input::KeyboardAndMouse::{MAPVK_VK_TO_VSC, MapVirtualKeyW};
    use hudhook::windows::Win32::UI::WindowsAndMessaging::{
        GetClientRect, GetWindowThreadProcessId, IsWindowVisible, PostMessageW, WM_KEYDOWN,
        WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE,
    };
    use hudhook::windows::core::{HSTRING, PCWSTR, PWSTR};
    use serde_json::{Value, json};
    use std::collections::{HashMap, HashSet};
    use std::ffi::c_void;
    use std::time::{Duration, Instant};
    pub static ENTRY_RESUMED: std::sync::atomic::AtomicBool =
        std::sync::atomic::AtomicBool::new(false);

    fn probe_cosmetics_url(url: &Value, service: Option<&Value>, automatic: bool) -> bool {
        let Some(url) = url.as_str() else {
            return false;
        };
        if url == "http://127.0.0.1:9" {
            return true;
        }
        if !service.is_some_and(|v| {
            automatic && v["schema"] == 1 && v["item"] == "arctic_probe_hat" && v["url"] == url
        }) {
            return false;
        }
        url.strip_prefix("http://127.0.0.1:").is_some_and(|port| {
            !port.is_empty()
                && port.bytes().all(|b| b.is_ascii_digit())
                && port.parse::<u16>().is_ok_and(|port| port != 0)
        })
    }
    #[test]
    fn cosmetic_service_remains_exact_loopback_and_opt_in() {
        let service = json!({"schema":1,"item":"arctic_probe_hat","url":"http://127.0.0.1:52341"});
        assert!(probe_cosmetics_url(&service["url"], Some(&service), true));
        assert!(!probe_cosmetics_url(&service["url"], Some(&service), false));
        assert!(!probe_cosmetics_url(&service["url"], None, true));
        for url in [
            "http://localhost:52341",
            "http://127.0.0.1:0",
            "http://127.0.0.1:65536",
            "http://127.0.0.1:52341/path",
            "http://127.0.0.1:52341@other.test",
            "https://other.test",
        ] {
            let mut other = service.clone();
            other["url"] = json!(url);
            assert!(!probe_cosmetics_url(&json!(url), Some(&other), true));
        }
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        fn EnumDesktopWindows(
            desktop: *mut c_void,
            callback: unsafe extern "system" fn(HWND, LPARAM) -> i32,
            param: LPARAM,
        ) -> i32;
        fn CreateDesktopW(
            name: *const u16,
            device: *const u16,
            mode: *const c_void,
            flags: u32,
            access: u32,
            security: *const c_void,
        ) -> *mut c_void;
        fn CloseDesktop(desktop: *mut c_void) -> i32;
        fn GetThreadDesktop(thread: u32) -> *mut c_void;
        fn SetThreadDesktop(desktop: *mut c_void) -> i32;
        fn AttachThreadInput(from: u32, to: u32, attach: i32) -> i32;
        fn SetActiveWindow(hwnd: HWND) -> HWND;
        fn GetActiveWindow() -> HWND;
        fn SetFocus(hwnd: HWND) -> HWND;
        fn GetFocus() -> HWND;
        fn GetUserObjectInformationW(
            object: *mut c_void,
            index: i32,
            information: *mut c_void,
            length: u32,
            needed: *mut u32,
        ) -> i32;
    }
    unsafe fn focus_private(desktop: *mut c_void, hwnd: HWND, pid: u32) -> Result<Value, String> {
        struct Restore(*mut c_void);
        impl Drop for Restore {
            fn drop(&mut self) {
                unsafe {
                    SetThreadDesktop(self.0);
                }
            }
        }
        struct Detach(u32, u32);
        impl Drop for Detach {
            fn drop(&mut self) {
                unsafe {
                    AttachThreadInput(self.0, self.1, 0);
                }
            }
        }
        unsafe {
            let thread = GetCurrentThreadId();
            let previous = GetThreadDesktop(thread);
            if previous.is_null() || SetThreadDesktop(desktop) == 0 {
                return Err("cannot attach the inspector to its private desktop".into());
            }
            let _restore = Restore(previous);
            let mut window_pid = 0;
            let target = GetWindowThreadProcessId(hwnd, Some(&mut window_pid));
            if window_pid != pid || target == 0 || AttachThreadInput(thread, target, 1) == 0 {
                return Err("cannot attach owned private input queue".into());
            }
            let _detach = Detach(thread, target);
            SetActiveWindow(hwnd);
            SetFocus(hwnd);
            if GetActiveWindow() != hwnd || GetFocus() != hwnd {
                return Err("private focus was not established".into());
            }
            use hudhook::windows::Win32::UI::WindowsAndMessaging::{
                WM_ACTIVATE, WM_ACTIVATEAPP, WM_SETFOCUS,
            };
            for message in [WM_ACTIVATEAPP, WM_ACTIVATE, WM_SETFOCUS] {
                PostMessageW(
                    Some(hwnd),
                    message,
                    WPARAM(usize::from(message != WM_SETFOCUS)),
                    LPARAM(0),
                )
                .map_err(|e| e.to_string())?;
            }
            Ok(
                json!({"privateFocusVerified":true,"privateActiveWindowVerified":true,"consumptionVerified":false}),
            )
        }
    }
    struct WindowQuery {
        pid: u32,
        windows: Vec<(HWND, i32, i32)>,
    }
    unsafe extern "system" fn find_window(hwnd: HWND, param: LPARAM) -> i32 {
        let query = unsafe { &mut *(param.0 as *mut WindowQuery) };
        let mut pid = 0;
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
        }
        let mut rect = RECT::default();
        if pid == query.pid
            && unsafe { IsWindowVisible(hwnd) }.as_bool()
            && unsafe { GetClientRect(hwnd, &mut rect) }.is_ok()
            && rect.right >= 320
            && rect.bottom >= 200
        {
            query.windows.push((hwnd, rect.right, rect.bottom));
        }
        1
    }
    /// Queue messages only to an owned window enumerated on our private test
    /// desktop. This does not move the system cursor, switch desktops, activate
    /// an interactive window, or claim that GameInput consumed the messages.
    unsafe fn input(desktop: *mut c_void, pid: u32, request: &Value) -> Result<Value, String> {
        let mut query = WindowQuery {
            pid,
            windows: Vec::new(),
        };
        unsafe {
            EnumDesktopWindows(
                desktop,
                find_window,
                LPARAM((&mut query as *mut WindowQuery) as isize),
            );
        }
        if query.windows.len() != 1 {
            return Err("expected one owned private-desktop game window".into());
        }
        let (hwnd, width, height) = query.windows[0];
        unsafe {
            match request["action"].as_str() {
                Some("focus") => return focus_private(desktop, hwnd, pid),
                Some("click") => {
                    let x = request["x"]
                        .as_i64()
                        .filter(|x| *x >= 0 && *x < width as i64)
                        .ok_or("click x outside client")?;
                    let y = request["y"]
                        .as_i64()
                        .filter(|y| *y >= 0 && *y < height as i64)
                        .ok_or("click y outside client")?;
                    let point = LPARAM((x | (y << 16)) as isize);
                    PostMessageW(Some(hwnd), WM_MOUSEMOVE, WPARAM(0), point)
                        .map_err(|e| e.to_string())?;
                    PostMessageW(Some(hwnd), WM_LBUTTONDOWN, WPARAM(1), point)
                        .map_err(|e| e.to_string())?;
                    PostMessageW(Some(hwnd), WM_LBUTTONUP, WPARAM(0), point)
                        .map_err(|e| e.to_string())?;
                }
                Some("key") => {
                    let key = request["key"]
                        .as_u64()
                        .filter(|k| {
                            [9, 13, 16, 27, 32, 37, 38, 39, 40, 65, 68, 83, 87, 112, 116]
                                .contains(k)
                        })
                        .ok_or("unsupported probe key")?;
                    let hold = request
                        .get("holdMs")
                        .map(|v| {
                            v.as_u64()
                                .filter(|ms| (20..=2000).contains(ms))
                                .ok_or("holdMs must be 20–2000 milliseconds")
                        })
                        .transpose()?;
                    if hold.is_some() && ![16, 32, 65, 68, 83, 87].contains(&key) {
                        return Err("held input is restricted to world movement keys".into());
                    }
                    let scan = MapVirtualKeyW(key as u32, MAPVK_VK_TO_VSC) & 0xff;
                    let bits =
                        1 | (scan << 16) | if (37..=40).contains(&key) { 1 << 24 } else { 0 };
                    PostMessageW(
                        Some(hwnd),
                        WM_KEYDOWN,
                        WPARAM(key as usize),
                        LPARAM(bits as isize),
                    )
                    .map_err(|e| e.to_string())?;
                    if let Some(milliseconds) = hold {
                        // This is the owned private window's message queue;
                        // no physical keyboard state or global input is used.
                        std::thread::sleep(Duration::from_millis(milliseconds));
                    }
                    PostMessageW(
                        Some(hwnd),
                        WM_KEYUP,
                        WPARAM(key as usize),
                        LPARAM((bits | 0xc0000000) as isize),
                    )
                    .map_err(|e| e.to_string())?;
                }
                _ => return Err("unsupported private-desktop probe action".into()),
            }
        }
        Ok(json!({"queued":true,"consumptionVerified":false,"width":width,"height":height}))
    }
    struct Session {
        desktop: *mut c_void,
        root: PROCESS_INFORMATION,
        live: HashMap<u32, HANDLE>,
        pending: Option<(u32, u32)>,
    }
    impl Drop for Session {
        fn drop(&mut self) {
            unsafe {
                // Kill only children whose creation we actually observed. Do not
                // enumerate or terminate any pre-existing Minecraft/user processes.
                if !self.root.hProcess.is_invalid() {
                    let _ = TerminateProcess(self.root.hProcess, 0);
                }
                for handle in self.live.values() {
                    let _ = TerminateProcess(*handle, 0);
                }
                if let Some((pid, tid)) = self.pending.take() {
                    let _ = ContinueDebugEvent(pid, tid, DBG_CONTINUE);
                }
                let deadline = Instant::now() + Duration::from_secs(3);
                while !self.live.is_empty() && Instant::now() < deadline {
                    let mut event = DEBUG_EVENT::default();
                    if WaitForDebugEvent(&mut event, 50).is_err() {
                        continue;
                    }
                    if event.dwDebugEventCode == CREATE_PROCESS_DEBUG_EVENT {
                        let info = event.u.CreateProcessInfo;
                        self.live.insert(event.dwProcessId, info.hProcess);
                        let _ = TerminateProcess(info.hProcess, 0);
                        if !info.hFile.is_invalid() {
                            let _ = CloseHandle(info.hFile);
                        }
                    } else if event.dwDebugEventCode == LOAD_DLL_DEBUG_EVENT {
                        let file = event.u.LoadDll.hFile;
                        if !file.is_invalid() {
                            let _ = CloseHandle(file);
                        }
                    } else if event.dwDebugEventCode == EXIT_PROCESS_DEBUG_EVENT {
                        self.live.remove(&event.dwProcessId);
                    }
                    let _ = ContinueDebugEvent(event.dwProcessId, event.dwThreadId, DBG_CONTINUE);
                }
                // Windows' default debugger exit policy also kills any remaining
                // debuggee. The probe never disables that policy or detaches live.
                let _ = CloseHandle(self.root.hThread);
                let _ = CloseHandle(self.root.hProcess);
                if !self.desktop.is_null() {
                    let _ = CloseDesktop(self.desktop);
                }
            }
        }
    }
    unsafe fn path(handle: HANDLE) -> Option<String> {
        let mut bytes = vec![0u16; 32768];
        let mut length = bytes.len() as u32;
        unsafe {
            QueryFullProcessImageNameW(
                handle,
                PROCESS_NAME_WIN32,
                PWSTR(bytes.as_mut_ptr()),
                &mut length,
            )
        }
        .ok()?;
        Some(String::from_utf16_lossy(&bytes[..length as usize]))
    }
    unsafe fn desktop_name(thread: u32) -> Option<String> {
        let desktop = unsafe { GetThreadDesktop(thread) };
        if desktop.is_null() {
            return None;
        }
        let mut buffer = [0u16; 256];
        let mut needed = 0;
        if unsafe {
            GetUserObjectInformationW(desktop, 2, buffer.as_mut_ptr().cast(), 512, &mut needed)
        } == 0
            || needed > 512
        {
            return None;
        }
        Some(String::from_utf16_lossy(
            &buffer[..buffer.iter().position(|v| *v == 0)?],
        ))
    }
    struct EntryTrap {
        process: HANDLE,
        thread: HANDLE,
        pid: u32,
        tid: u32,
        address: usize,
        original: u8,
        armed: bool,
        hit: bool,
    }
    impl EntryTrap {
        unsafe fn write(&self, byte: u8) -> Result<(), Box<dyn std::error::Error>> {
            let mut old = PAGE_PROTECTION_FLAGS(0);
            unsafe {
                VirtualProtectEx(
                    self.process,
                    self.address as *const c_void,
                    1,
                    PAGE_EXECUTE_READWRITE,
                    &mut old,
                )
            }
            .map_err(|e| format!("protect entry trap: {e}"))?;
            let mut count = 0;
            let result = unsafe {
                WriteProcessMemory(
                    self.process,
                    self.address as *const c_void,
                    &byte as *const u8 as _,
                    1,
                    Some(&mut count),
                )
            };
            let mut ignored = PAGE_PROTECTION_FLAGS(0);
            let restore = unsafe {
                VirtualProtectEx(
                    self.process,
                    self.address as *const c_void,
                    1,
                    old,
                    &mut ignored,
                )
            };
            result.map_err(|e| format!("write entry trap: {e}"))?;
            restore.map_err(|e| format!("restore entry protection: {e}"))?;
            if count != 1 {
                return Err("entry trap write was incomplete".into());
            }
            unsafe { FlushInstructionCache(self.process, Some(self.address as *const c_void), 1) }?;
            Ok(())
        }
        unsafe fn set(
            info: CREATE_PROCESS_DEBUG_INFO,
            pid: u32,
            tid: u32,
            header: &[u8],
        ) -> Result<Self, Box<dyn std::error::Error>> {
            let pe = u32::from_le_bytes(
                header
                    .get(0x3c..0x40)
                    .ok_or("short DOS header")?
                    .try_into()?,
            ) as usize;
            if header.get(pe..pe + 4) != Some(b"PE\0\0")
                || header.get(pe + 24..pe + 26) != Some(&[0x0b, 0x02])
            {
                return Err("entry trap requires a checked x64 PE image".into());
            }
            let entry = u32::from_le_bytes(
                header
                    .get(pe + 40..pe + 44)
                    .ok_or("missing entry RVA")?
                    .try_into()?,
            ) as usize;
            let size = u32::from_le_bytes(
                header
                    .get(pe + 80..pe + 84)
                    .ok_or("missing image size")?
                    .try_into()?,
            ) as usize;
            if entry == 0 || entry >= size {
                return Err("entry point is outside the image".into());
            }
            let address = (info.lpBaseOfImage as usize)
                .checked_add(entry)
                .ok_or("entry address overflow")?;
            let trap = Self {
                process: info.hProcess,
                thread: info.hThread,
                pid,
                tid,
                address,
                original: 0,
                armed: false,
                hit: false,
            };
            Ok(trap)
        }
        unsafe fn arm(&mut self) -> Result<(), Box<dyn std::error::Error>> {
            let mut byte = [0u8; 1];
            let mut copied = 0;
            unsafe {
                ReadProcessMemory(
                    self.process,
                    self.address as *const c_void,
                    byte.as_mut_ptr().cast(),
                    1,
                    Some(&mut copied),
                )
            }
            .map_err(|e| format!("read entry after system loader: {e}"))?;
            if copied != 1 || byte[0] == 0xcc {
                return Err("entry is unreadable or already trapped".into());
            }
            self.original = byte[0];
            unsafe { self.write(0xcc) }?;
            self.armed = true;
            Ok(())
        }
        unsafe fn hold(&mut self) -> Result<(), Box<dyn std::error::Error>> {
            // Win32 requires 16-byte alignment. The generated windows crate's
            // CONTEXT has only natural 8-byte alignment on this toolchain.
            #[repr(C, align(16))]
            struct AlignedContext(CONTEXT);
            let mut context = AlignedContext(CONTEXT {
                ContextFlags: CONTEXT_CONTROL_AMD64,
                ..Default::default()
            });
            unsafe { GetThreadContext(self.thread, &mut context.0) }
                .map_err(|e| format!("read entry thread context: {e}"))?;
            if context.0.Rip as usize != self.address + 1 {
                return Err("entry trap instruction pointer did not match".into());
            }
            unsafe { self.write(self.original) }?;
            context.0.Rip = self.address as u64;
            if unsafe { SuspendThread(self.thread) } == u32::MAX {
                return Err("could not hold entry-point thread".into());
            }
            unsafe { SetThreadContext(self.thread, &context.0) }
                .map_err(|e| format!("restore entry thread context: {e}"))?;
            self.hit = true;
            Ok(())
        }
        unsafe fn resume(&mut self) -> Result<(), Box<dyn std::error::Error>> {
            if !self.hit {
                return Err("entry thread was not held".into());
            }
            let previous = unsafe { ResumeThread(self.thread) };
            if previous != u32::MAX {
                ENTRY_RESUMED.store(true, std::sync::atomic::Ordering::Release);
            }
            if previous != 1 {
                return Err(format!("unexpected entry suspension count: {previous}").into());
            }
            Ok(())
        }
    }
    pub fn run(
        game: &std::ffi::OsStr,
        bundle: Option<&std::path::Path>,
        isolated_boot: bool,
        boot_seconds: u64,
    ) -> Result<Value, Box<dyn std::error::Error>> {
        // An explicit export probe identifies its encoder before any child runs.
        // Only this exact executable may use the debugger thread-name protocol.
        let encoder =
            if let Some(dir) = bundle.filter(|dir| dir.join("probe-encoder.json").exists()) {
                #[derive(serde::Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Encoder {
                    schema: u32,
                    ffmpeg: std::path::PathBuf,
                }
                let bytes = std::fs::read(dir.join("probe-encoder.json"))?;
                if bytes.len() > 4096 {
                    return Err("encoder probe configuration exceeds limit".into());
                }
                let config: Encoder = serde_json::from_slice(&bytes)?;
                if config.schema != 1
                    || !config.ffmpeg.is_absolute()
                    || !config.ffmpeg.is_file()
                    || !config
                        .ffmpeg
                        .extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
                {
                    return Err("encoder probe requires an existing absolute executable".into());
                }
                Some(config.ffmpeg.canonicalize()?)
            } else {
                None
            };
        let automatic_world = isolated_boot
            && bundle.is_some_and(|dir| {
                std::fs::read(dir.join("arctic-offline-probe.json"))
                    .ok()
                    .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
                    .is_some_and(|v| v["auto_world"] == true)
            });
        if let Some(dir) = bundle {
            let config = arctic_bedrock::bundle::Bundle::read(dir)?;
            let session: Value = serde_json::from_slice(&std::fs::read(
                config.config_dir.join("arctic-session.json"),
            )?)?;
            let service_path = dir.join("cosmetics-service.json");
            let service = service_path
                .metadata()
                .ok()
                .filter(|m| m.len() <= 4096)
                .and_then(|_| std::fs::read(service_path).ok())
                .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
            if config.offline_name != "ArcticTest"
                || !probe_cosmetics_url(&session["url"], service.as_ref(), automatic_world)
                || session.get("token").is_some()
                || session.get("bridge").is_some()
            {
                return Err("early initialization probe requires ArcticTest and a credential-free loopback session".into());
            }
            if isolated_boot && !dir.join("arctic-offline-probe.json").is_file() {
                return Err("isolated boot requires an explicit fresh-profile sidecar".into());
            }
        }
        unsafe {
            let name = format!("ArcticInspect_{}", std::process::id());
            let desktop = HSTRING::from(name.as_str());
            let handle = CreateDesktopW(
                desktop.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                0,
                0x1ff,
                std::ptr::null(),
            );
            if handle.is_null() {
                return Err(format!(
                    "create private desktop: {}",
                    std::io::Error::last_os_error()
                )
                .into());
            }
            // Never call SwitchDesktop. The game/helper cannot display on the
            // interactive desktop through this inherited launch desktop.
            let application = HSTRING::from(game);
            let startup = STARTUPINFOW {
                cb: size_of::<STARTUPINFOW>() as u32,
                lpDesktop: PWSTR(desktop.as_ptr() as *mut u16),
                ..Default::default()
            };
            let mut session = Session {
                desktop: handle,
                root: PROCESS_INFORMATION::default(),
                live: HashMap::new(),
                pending: None,
            };
            CreateProcessW(
                PCWSTR(application.as_ptr()),
                None,
                None,
                None,
                false,
                DEBUG_PROCESS | CREATE_NO_WINDOW,
                None,
                None,
                &startup,
                &mut session.root,
            )?;
            DebugSetProcessKillOnExit(true)?;
            let deadline = Instant::now()
                + Duration::from_secs(if isolated_boot {
                    boot_seconds + 30
                } else if bundle.is_some() {
                    25
                } else {
                    15
                });
            let mut processes = Vec::new();
            let mut encoder_pids = HashSet::new();
            let mut encoder_notifications = 0u64;
            let mut target: Option<Value> = None;
            let mut injection: Option<std::sync::mpsc::Receiver<Result<(), String>>> = None;
            let mut injected = false;
            let mut entry_trap: Option<EntryTrap> = None;
            let mut initial_breaks = HashSet::new();
            let mut boot_at: Option<Instant> = None;
            let mut exceptions = Vec::new();
            let mut input_sequence = 0u64;
            let mut pending_input: Option<(u64, std::sync::mpsc::Receiver<Result<Value, String>>)> =
                None;
            while Instant::now() < deadline {
                // A failed DLL bootstrap may never produce an engine report.
                // Retire the owned debuggee even while entry is still held.
                if boot_at.is_none()
                    && automatic_world
                    && let Some(dir) = bundle
                    && let Ok(bytes) = std::fs::read(dir.join("auto-world.json"))
                    && bytes.len() <= 64 * 1024
                    && let Ok(auto) = serde_json::from_slice::<Value>(&bytes)
                    && auto["state"] == "error"
                {
                    if let Some(target) = &mut target {
                        target["automaticWorldTest"] = auto;
                    }
                    break;
                }
                if boot_at.is_some()
                    && let (Some(dir), Some(target)) = (bundle, &target)
                {
                    // Input holds run outside the debugger's event pump. Sleeping
                    // here would suspend the entire game at its next debug event.
                    if let Some((sequence, receiver)) = &pending_input {
                        let result = match receiver.try_recv() {
                            Ok(result) => Some(result),
                            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                Some(Err("private input worker disconnected".into()))
                            }
                            Err(std::sync::mpsc::TryRecvError::Empty) => None,
                        };
                        if let Some(result) = result {
                            let response = match result {
                                Ok(mut v) => {
                                    v["sequence"] = json!(sequence);
                                    v
                                }
                                Err(error) => json!({"sequence":sequence,"error":error}),
                            };
                            let _ = std::fs::write(
                                dir.join("probe-input-result.json"),
                                response.to_string(),
                            );
                            pending_input = None;
                        }
                    }
                    let request = dir.join("probe-input.json");
                    if pending_input.is_none()
                        && let Ok(bytes) = std::fs::read(request)
                        && bytes.len() <= 4096
                        && let Ok(value) = serde_json::from_slice::<Value>(&bytes)
                        && let Some(sequence) = value["sequence"]
                            .as_u64()
                            .filter(|s| *s > input_sequence && *s <= 1024)
                    {
                        input_sequence = sequence;
                        let desktop = session.desktop as usize;
                        let pid = target["pid"].as_u64().ok_or("missing owned game PID")? as u32;
                        let (sender, receiver) = std::sync::mpsc::channel();
                        std::thread::spawn(move || {
                            let result = input(desktop as *mut c_void, pid, &value);
                            let _ = sender.send(result);
                        });
                        pending_input = Some((sequence, receiver));
                    }
                }
                if let Some(receiver) = &injection {
                    match receiver.try_recv() {
                        Ok(Ok(())) => {
                            injected = true;
                            injection = None;
                            if let Some(target) = &mut target {
                                target["dllLoaded"] = json!(true);
                            }
                        }
                        Ok(Err(error)) => {
                            if let Some(target) = &mut target {
                                target["injectionError"] = json!(error);
                            }
                            break;
                        }
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                            if let Some(target) = &mut target {
                                target["injectionError"] = json!("injection worker disconnected");
                            }
                            break;
                        }
                        Err(std::sync::mpsc::TryRecvError::Empty) => {}
                    }
                }
                if injected && let (Some(dir), Some(target)) = (bundle, &mut target) {
                    let report = dir.join(format!(
                        "arctic-bedrock-{}.engine.json",
                        target["pid"].as_u64().ok_or("missing probe PID")?
                    ));
                    if let Ok(bytes) = std::fs::read(report)
                        && bytes.len() <= 1024 * 1024
                        && let Ok(engine) = serde_json::from_slice::<Value>(&bytes)
                    {
                        target["engineObserverReport"] = engine;
                        if isolated_boot {
                            if boot_at.is_none() {
                                let engine = &target["engineObserverReport"];
                                if engine["activeHookMask"] != 7
                                    || engine["state"] != "observing"
                                    || !matches!(
                                        engine["probeIsolation"]["networkMode"].as_str(),
                                        Some("deny-all" | "loopback-only")
                                    )
                                    || !matches!(
                                        engine["probeIsolation"]["userMode"].as_str(),
                                        Some("deny-provider" | "empty-provider")
                                    )
                                {
                                    return Err("refusing entry resume without initialized engine observers and isolation guards".into());
                                }
                                entry_trap.as_mut().ok_or("missing entry trap")?.resume()?;
                                target["entryPointResumed"] = json!(true);
                                boot_at = Some(Instant::now());
                                std::fs::write(dir.join("probe-live.json"), json!({"schema":1,"pid":target["pid"],"inspector":GetCurrentProcessId(),"privateDesktop":true,"isolatedBoot":true}).to_string())?;
                            }
                            if automatic_world
                                && let Ok(bytes) = std::fs::read(dir.join("auto-world.json"))
                                && bytes.len() <= 64 * 1024
                                && let Ok(auto) = serde_json::from_slice::<Value>(&bytes)
                            {
                                let engine = &target["engineObserverReport"];
                                let observed = engine["worldPasses"].as_u64().unwrap_or(0) >= 30
                                    && engine["actorCalls"].as_u64().unwrap_or(0) > 0
                                    && engine["camera"]["clientTicks"].as_u64().unwrap_or(0) >= 2
                                    && engine["playerDirectory"]["levelObserved"] == true;
                                let depth_observed = auto["worldDepthObserved"] == true
                                    && auto["finalDelivered"]
                                        .as_u64()
                                        .zip(auto["initialDelivered"].as_u64())
                                        .and_then(|(last, first)| last.checked_sub(first))
                                        == auto["worldFramesDelivered"].as_u64()
                                    && auto["worldFramesDelivered"].as_u64().unwrap_or(0) >= 5
                                    && auto["lastDeliveryGapMs"]
                                        .as_u64()
                                        .is_some_and(|v| v <= 2000)
                                    && auto["initialExhausted"].as_u64().is_some_and(|initial| {
                                        Some(initial) == auto["finalExhausted"].as_u64()
                                    })
                                    && auto["finalDelivered"].as_u64().is_some_and(|last| {
                                        engine["worldProducer"]["delivered"].as_u64().unwrap_or(0)
                                            >= last
                                    });
                                if auto["state"] == "error"
                                    || (auto["state"] == "complete"
                                        && auto["worldHooksObserved"] == true
                                        && depth_observed
                                        && observed)
                                {
                                    target["automaticWorldTest"] = auto;
                                    break;
                                }
                            }
                            if boot_at
                                .is_some_and(|at| at.elapsed() >= Duration::from_secs(boot_seconds))
                            {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                }
                let mut event = DEBUG_EVENT::default();
                if WaitForDebugEvent(&mut event, 100).is_err() {
                    continue;
                }
                session.pending = Some((event.dwProcessId, event.dwThreadId));
                let mut disposition = DBG_CONTINUE;
                match event.dwDebugEventCode {
                    CREATE_PROCESS_DEBUG_EVENT => {
                        let info = event.u.CreateProcessInfo;
                        session.live.insert(event.dwProcessId, info.hProcess);
                        if !info.hFile.is_invalid() {
                            let _ = CloseHandle(info.hFile);
                        }
                        let path =
                            path(info.hProcess).ok_or("cannot identify launch-chain process")?;
                        if encoder.as_ref().is_some_and(|encoder| {
                            std::path::Path::new(&path)
                                .canonicalize()
                                .is_ok_and(|path| {
                                    path.to_string_lossy()
                                        .eq_ignore_ascii_case(&encoder.to_string_lossy())
                                })
                        }) {
                            encoder_pids.insert(event.dwProcessId);
                        }
                        let observed_desktop = desktop_name(event.dwThreadId);
                        let startup_desktop = startup_desktop(info.hProcess);
                        let private = observed_desktop
                            .as_ref()
                            .or(startup_desktop.as_ref())
                            .map(|d| d.rsplit('\\').next() == Some(name.as_str()));
                        processes.push(
                            json!({"pid":event.dwProcessId,"path":path,"desktop":observed_desktop,"startupDesktop":startup_desktop}),
                        );
                        let game = std::path::Path::new(&path)
                            .file_name()
                            .is_some_and(|n| n.eq_ignore_ascii_case("Minecraft.Windows.exe"));
                        if game {
                            let mut bytes = [0u8; 4096];
                            let mut copied = 0;
                            let memory = ReadProcessMemory(
                                info.hProcess,
                                info.lpBaseOfImage,
                                bytes.as_mut_ptr().cast(),
                                bytes.len(),
                                Some(&mut copied),
                            );
                            target = Some(
                                json!({"pid":event.dwProcessId,"entryPointResumed":false,"requestedDesktop":name,"observedDesktop":observed_desktop,"startupDesktop":startup_desktop,"privateDesktop":private,"imageHeaderReadable":memory.is_ok()&&copied==4096&&bytes[..2]==*b"MZ"}),
                            );
                            let Some(_dir) = bundle else {
                                break;
                            };
                            if private != Some(true) {
                                return Err(
                                    "early injection requires a verified private startup desktop"
                                        .into(),
                                );
                            }
                            memory?;
                            entry_trap = Some(EntryTrap::set(
                                info,
                                event.dwProcessId,
                                event.dwThreadId,
                                &bytes,
                            )?);
                            if let Some(target) = &mut target {
                                target["systemLoaderAdvanced"] = json!(true);
                            }
                        }
                        // Fail closed if a redirected helper explicitly selected a
                        // different desktop. Do not resume it to create a window.
                        if private != Some(true) {
                            return Err("launch helper did not inherit the private desktop".into());
                        }
                    }
                    LOAD_DLL_DEBUG_EVENT => {
                        let file = event.u.LoadDll.hFile;
                        if !file.is_invalid() {
                            let _ = CloseHandle(file);
                        }
                    }
                    EXIT_PROCESS_DEBUG_EVENT => {
                        session.live.remove(&event.dwProcessId);
                        encoder_pids.remove(&event.dwProcessId);
                        if let Some(process) = processes
                            .iter_mut()
                            .find(|p| p["pid"].as_u64() == Some(u64::from(event.dwProcessId)))
                        {
                            process["exitCode"] = json!(event.u.ExitProcess.dwExitCode);
                        }
                        if let Some(target) = &mut target
                            && target["pid"].as_u64() == Some(event.dwProcessId as u64)
                        {
                            target["exitCode"] = json!(event.u.ExitProcess.dwExitCode);
                            break;
                        }
                    }
                    EXCEPTION_DEBUG_EVENT => {
                        let exception = event.u.Exception.ExceptionRecord;
                        if crate::debug_notifications::owned_encoder_thread_name(
                            &event.u.Exception,
                            encoder_pids.contains(&event.dwProcessId),
                        ) {
                            encoder_notifications += 1;
                            ContinueDebugEvent(event.dwProcessId, event.dwThreadId, DBG_CONTINUE)?;
                            session.pending = None;
                            continue;
                        }
                        if exception.ExceptionCode.0 as u32 != 0x80000003 && exceptions.len() < 32 {
                            exceptions.push(json!({"pid":event.dwProcessId,"code":format!("{:08x}",exception.ExceptionCode.0 as u32),"firstChance":event.u.Exception.dwFirstChance!=0}));
                        }
                        if exception.ExceptionCode.0 as u32 == 0x80000003
                            && let Some(trap) = &mut entry_trap
                            && trap.armed
                            && !trap.hit
                            && trap.pid == event.dwProcessId
                            && trap.tid == event.dwThreadId
                            && exception.ExceptionAddress as usize == trap.address
                        {
                            trap.hold()?;
                            if let Some(target) = &mut target {
                                target["stoppedAtEntryPoint"] = json!(true);
                            }
                            let (sender, receiver) =
                                std::sync::mpsc::channel::<Result<(), String>>();
                            injection = Some(receiver);
                            let pid = event.dwProcessId;
                            let dll = bundle
                                .ok_or("missing injection bundle")?
                                .join("arctic_bedrock.dll")
                                .canonicalize()?;
                            std::thread::spawn(move || {
                                let result =
                                    crate::loader::inject(pid, &dll).map_err(|e| e.to_string());
                                let _ = sender.send(result);
                            });
                        } else if exception.ExceptionCode.0 as u32 == 0x80000003
                            && let Some(trap) = &mut entry_trap
                            && !trap.armed
                            && trap.pid == event.dwProcessId
                            && trap.tid == event.dwThreadId
                        {
                            // Initial system-loader breakpoint precedes EXE
                            // entry. Some packaged images are unreadable until
                            // this point; refuse to continue unless armed.
                            trap.arm()?;
                            initial_breaks.insert(event.dwProcessId);
                            if let Some(target) = &mut target {
                                target["entryTrapArmed"] = json!(true);
                            }
                        } else if exception.ExceptionCode.0 as u32 != 0x80000003
                            || !initial_breaks.insert(event.dwProcessId)
                        {
                            // Consume only each debuggee's initial loader break
                            // and our own entry trap. Other exceptions belong
                            // to the target's handlers, including assertions.
                            disposition = DBG_EXCEPTION_NOT_HANDLED;
                        }
                    }
                    _ => {}
                }
                ContinueDebugEvent(event.dwProcessId, event.dwThreadId, disposition)?;
                session.pending = None;
                if session.live.is_empty() {
                    break;
                }
            }
            if let Some(target) = &mut target
                && bundle.is_some()
            {
                target["stoppedAtEntryPoint"] =
                    json!(entry_trap.as_ref().is_some_and(|trap| trap.hit));
                target["dllLoaded"] = json!(injected);
                target["bootObservedMs"] = json!(boot_at.map(|at| at.elapsed().as_millis() as u64));
            }
            Ok(
                json!({"schema":1,"processes":processes,"target":target,"exceptions":exceptions,"encoderThreadNameNotifications":encoder_notifications,"gameEntryPointResumed":ENTRY_RESUMED.load(std::sync::atomic::Ordering::Acquire),"worldTested":false}),
            )
        }
    }
}

#[cfg(windows)]
fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() != 3
        && !(args.len() == 5 && args[3] == "--inject-bundle")
        && !(args.len() == 6 && args[3] == "--isolated-boot")
    {
        return;
    }
    let bundle = if args.len() >= 5 {
        Some(std::path::Path::new(&args[4]))
    } else {
        None
    };
    let boot_seconds = if args.len() == 6 {
        args[5]
            .to_str()
            .and_then(|s| s.parse::<u64>().ok())
            .filter(|s| (15..=3600).contains(s))
    } else {
        Some(15)
    };
    let Some(boot_seconds) = boot_seconds else {
        return;
    };
    let report = match windows_probe::run(
        &args[1],
        bundle,
        args.len() == 6 && args[3] == "--isolated-boot",
        boot_seconds,
    ) {
        Ok(report) => report,
        Err(error) => {
            serde_json::json!({"schema":1,"error":error.to_string(),"gameEntryPointResumed":windows_probe::ENTRY_RESUMED.load(std::sync::atomic::Ordering::Acquire),"worldTested":false})
        }
    };
    let _ = std::fs::write(&args[2], report.to_string());
}
#[cfg(not(windows))]
fn main() {
    eprintln!("Windows launch-chain inspection requires Windows");
}
