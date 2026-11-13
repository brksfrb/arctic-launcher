//! Bounded stack diagnostics for an explicitly identified, isolated test game.
//! No heap dumps, symbol downloads, account reads, or interactive UI operations.
#![cfg_attr(windows, windows_subsystem = "windows")]
#[cfg(windows)]
#[path = "support/desktop.rs"]
mod desktop;
#[cfg(windows)]
fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    if !(4..=5).contains(&args.len())
        || args
            .get(4)
            .is_some_and(|v| v != "--stop" && v != "--wake" && v != "--focus-private")
    {
        return;
    }
    let root = std::path::PathBuf::from(&args[1]);
    let result = run(
        &root,
        &args[2].to_string_lossy(),
        &args[3].to_string_lossy(),
        args.get(4).is_some_and(|v| v == "--stop"),
        args.get(4).is_some_and(|v| v == "--wake"),
        args.get(4).is_some_and(|v| v == "--focus-private"),
    )
    .unwrap_or_else(|e| serde_json::json!({"error":e}));
    let _ = std::fs::write(root.join("probe-stacks.json"), result.to_string());
}
#[cfg(not(windows))]
fn main() {}

#[cfg(windows)]
#[derive(serde::Serialize)]
struct StartupCheck {
    target_rva: u64,
    pending: bool,
    provider_targets: Vec<u64>,
    provider_signing_in: Vec<Option<bool>>,
    provider_tables: Vec<u64>,
    local_player_getter_rva: Option<u64>,
}
/// Resolve the observed startup check's code pointers and pending-operation flag.
/// No names, identifiers, handles or credentials are read.
#[cfg(windows)]
fn startup_virtual_target(
    process: hudhook::windows::Win32::Foundation::HANDLE,
    base: u64,
    size: u64,
) -> Option<StartupCheck> {
    use hudhook::windows::Win32::System::Diagnostics::Debug::ReadProcessMemory;
    fn bytes<const N: usize>(
        process: hudhook::windows::Win32::Foundation::HANDLE,
        at: u64,
    ) -> Option<[u8; N]> {
        let mut value = [0u8; N];
        let mut copied = 0;
        unsafe {
            ReadProcessMemory(
                process,
                at as *const _,
                value.as_mut_ptr().cast(),
                N,
                Some(&mut copied),
            )
        }
        .ok()?;
        (copied == N).then_some(value)
    }
    let pointer = |at| -> Option<u64> {
        let value = u64::from_le_bytes(bytes::<8>(process, at)?);
        (0x10000..0x0000800000000000)
            .contains(&value)
            .then_some(value)
    };
    let win_main = pointer(base.checked_add(0x11d1bf80)?)?;
    let platform = pointer(win_main.checked_add(8)?)?;
    let holder = pointer(platform.checked_add(0x118)?)?;
    let game = pointer(holder.checked_add(0x48)?)?;
    let head = pointer(game.checked_add(0x970)?)?;
    let count = u64::from_le_bytes(bytes::<8>(process, game.checked_add(0x978)?)?);
    if count == 0 || count > 16 {
        return None;
    }
    let mut node = pointer(head.checked_add(8)?)?;
    for _ in 0..=count {
        if bytes::<1>(process, node.checked_add(25)?)?[0] != 0 {
            return None;
        }
        if bytes::<1>(process, node.checked_add(32)?)?[0] == 0 {
            let client = pointer(node.checked_add(48)?)?;
            if pointer(client.checked_add(0x1a8)?)? != game {
                return None;
            }
            let table = pointer(client)?;
            if pointer(table.checked_add(0x598)?)? != base.checked_add(0x5dc29e0)? {
                return None;
            }
            let local_player_getter_rva = pointer(table.checked_add(0xf8)?)
                .and_then(|target| target.checked_sub(base))
                .filter(|rva| (0x1000..0xe5a7000).contains(rva));
            // This exact getter returns the shared pointer at client + 0xE00.
            let object = pointer(client.checked_add(0xe00)?)?;
            let table = pointer(object)?;
            let target = pointer(table.checked_add(0x48)?)?;
            let rva = target.checked_sub(base)?;
            if rva != 0xe53f30 || rva >= size {
                return None;
            }
            let pending = bytes::<1>(process, object.checked_add(0x228)?)?[0];
            if pending > 1 {
                return None;
            }
            let container = pointer(object.checked_add(0x218)?)?;
            let head = pointer(container.checked_add(8)?)?;
            let mut node = pointer(head)?;
            let mut provider_targets = Vec::new();
            let mut provider_signing_in = Vec::new();
            let mut provider_tables = Vec::new();
            for _ in 0..16 {
                if node == head {
                    return Some(StartupCheck {
                        target_rva: rva,
                        pending: pending != 0,
                        provider_targets,
                        provider_signing_in,
                        provider_tables,
                        local_player_getter_rva,
                    });
                }
                let provider = pointer(node.checked_add(0x18)?)?;
                let table = pointer(provider)?;
                provider_tables.push(table.checked_sub(base)?);
                let target = pointer(table.checked_add(0x50)?)?.checked_sub(base)?;
                if !(0x1000..0xe5a7000).contains(&target) || target >= size {
                    return None;
                }
                provider_targets.push(target);
                // These two exact-image accessors only read operation state.
                // Do not evaluate arbitrary code or inspect identity fields.
                provider_signing_in.push(match target {
                    0x4288a90 => Some(
                        u32::from_le_bytes(bytes::<4>(process, provider.checked_add(0x70)?)?) == 1,
                    ),
                    0x13c8a50 => {
                        Some(bytes::<1>(process, provider.checked_add(0x1f9)?)?[0] & 1 != 0)
                    }
                    _ => None,
                });
                node = pointer(node)?;
            }
            return None;
        }
        node = pointer(node)?;
    }
    None
}

/// Post lifecycle messages only after run() verifies this probe's process,
/// injected DLL and private desktop. This never switches or activates desktops.
#[cfg(windows)]
fn wake_window(pid: u32, owner: u32, focus: bool) -> Result<serde_json::Value, String> {
    use hudhook::windows::Win32::{
        Foundation::{HWND, LPARAM, RECT, WPARAM},
        UI::WindowsAndMessaging::*,
    };
    use std::ffi::c_void;
    #[link(name = "user32")]
    unsafe extern "system" {
        fn OpenDesktopW(name: *const u16, flags: u32, inherit: i32, access: u32) -> *mut c_void;
        fn CloseDesktop(desktop: *mut c_void) -> i32;
        fn GetThreadDesktop(thread: u32) -> *mut c_void;
        fn SetThreadDesktop(desktop: *mut c_void) -> i32;
        fn AttachThreadInput(from: u32, to: u32, attach: i32) -> i32;
        fn SetActiveWindow(hwnd: HWND) -> HWND;
        fn GetActiveWindow() -> HWND;
        fn SetFocus(hwnd: HWND) -> HWND;
        fn GetFocus() -> HWND;
        fn EnumDesktopWindows(
            desktop: *mut c_void,
            callback: unsafe extern "system" fn(HWND, LPARAM) -> i32,
            param: LPARAM,
        ) -> i32;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentThreadId() -> u32;
    }
    struct RestoreDesktop(*mut c_void);
    impl Drop for RestoreDesktop {
        fn drop(&mut self) {
            unsafe {
                SetThreadDesktop(self.0);
            }
        }
    }
    struct Attached(u32, u32);
    impl Drop for Attached {
        fn drop(&mut self) {
            unsafe {
                AttachThreadInput(self.0, self.1, 0);
            }
        }
    }
    struct Desktop(*mut c_void);
    impl Drop for Desktop {
        fn drop(&mut self) {
            unsafe {
                CloseDesktop(self.0);
            }
        }
    }
    struct Query {
        pid: u32,
        windows: Vec<HWND>,
    }
    unsafe extern "system" fn collect(hwnd: HWND, param: LPARAM) -> i32 {
        let query = unsafe { &mut *(param.0 as *mut Query) };
        let mut pid = 0;
        let mut rect = RECT::default();
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == query.pid
                && IsWindowVisible(hwnd).as_bool()
                && GetClientRect(hwnd, &mut rect).is_ok()
                && rect.right >= 320
                && rect.bottom >= 200
            {
                query.windows.push(hwnd);
            }
        }
        if query.windows.len() > 8 { 0 } else { 1 }
    }
    unsafe {
        let name: Vec<u16> = format!("ArcticInspect_{owner}")
            .encode_utf16()
            .chain(Some(0))
            .collect();
        // No DESKTOP_SWITCHDESKTOP access is requested. The interactive input
        // desktop and the user's foreground app are never selected or changed.
        let desktop = Desktop(OpenDesktopW(
            name.as_ptr(),
            0,
            0,
            if focus { 0xc1 } else { 1 },
        ));
        if desktop.0.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let mut query = Query {
            pid,
            windows: Vec::new(),
        };
        let result = EnumDesktopWindows(
            desktop.0,
            collect,
            LPARAM((&mut query as *mut Query) as isize),
        );
        if result == 0 || query.windows.len() != 1 {
            return Err("owned private game window is not unique".into());
        }
        let hwnd = query.windows[0];
        let (active, focused) = if focus {
            let own_thread = GetCurrentThreadId();
            let old = GetThreadDesktop(own_thread);
            if old.is_null() || SetThreadDesktop(desktop.0) == 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            let _restore = RestoreDesktop(old);
            let mut window_pid = 0;
            let target_thread = GetWindowThreadProcessId(hwnd, Some(&mut window_pid));
            if window_pid != pid
                || target_thread == 0
                || AttachThreadInput(own_thread, target_thread, 1) == 0
            {
                return Err("private input queue attachment failed".into());
            }
            let _attached = Attached(own_thread, target_thread);
            SetActiveWindow(hwnd);
            SetFocus(hwnd);
            (GetActiveWindow() == hwnd, GetFocus() == hwnd)
        } else {
            (false, false)
        };
        PostMessageW(Some(hwnd), WM_ACTIVATEAPP, WPARAM(1), LPARAM(0))
            .map_err(|e| e.to_string())?;
        PostMessageW(Some(hwnd), WM_ACTIVATE, WPARAM(1), LPARAM(0)).map_err(|e| e.to_string())?;
        PostMessageW(Some(hwnd), WM_SETFOCUS, WPARAM(0), LPARAM(0)).map_err(|e| e.to_string())?;
        Ok(
            serde_json::json!({"pid":pid,"inspector":owner,"privateDesktop":true,"lifecycleMessagesQueued":true,"consumptionVerified":false,"privateActiveWindowVerified":active,"privateFocusVerified":focused}),
        )
    }
}

#[cfg(windows)]
fn run(
    root: &std::path::Path,
    pid: &str,
    owner: &str,
    stop: bool,
    wake: bool,
    focus: bool,
) -> Result<serde_json::Value, String> {
    use hudhook::windows::Win32::Foundation::{CloseHandle, FILETIME, HANDLE};
    use hudhook::windows::Win32::System::Diagnostics::{Debug::*, ToolHelp::*};
    use hudhook::windows::Win32::System::SystemServices::PROCESS_MITIGATION_CHILD_PROCESS_POLICY;
    use hudhook::windows::Win32::System::Threading::*;
    use hudhook::windows::core::PCWSTR;
    use serde_json::json;
    unsafe extern "system" fn function_table(
        process: HANDLE,
        address: u64,
    ) -> *mut std::ffi::c_void {
        unsafe { SymFunctionTableAccess64(process, address) }
    }
    unsafe extern "system" fn module_base(process: HANDLE, address: u64) -> u64 {
        unsafe { SymGetModuleBase64(process, address) }
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetThreadDesktop(thread: u32) -> *mut std::ffi::c_void;
        fn GetUserObjectInformationW(
            object: *mut std::ffi::c_void,
            index: i32,
            information: *mut std::ffi::c_void,
            length: u32,
            needed: *mut u32,
        ) -> i32;
    }
    struct Handle(HANDLE);
    impl Drop for Handle {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
    struct Suspended(HANDLE);
    impl Drop for Suspended {
        fn drop(&mut self) {
            unsafe {
                ResumeThread(self.0);
            }
        }
    }
    let pid: u32 = pid.parse().map_err(|_| "invalid target PID")?;
    let owner: u32 = owner.parse().map_err(|_| "invalid inspector PID")?;
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let bytes = std::fs::read(root.join(format!("arctic-bedrock-{pid}.engine.json")))
        .map_err(|e| e.to_string())?;
    if bytes.len() > 1024 * 1024 {
        return Err("engine report exceeds limit".into());
    }
    let report: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let profile = report["probeIsolation"]["profile"]
        .as_str()
        .ok_or("missing test profile")?;
    if std::path::Path::new(profile)
        .canonicalize()
        .map_err(|e| e.to_string())?
        != root
            .join("game-profile")
            .canonicalize()
            .map_err(|e| e.to_string())?
        || report["probeIsolation"]["licenseChanged"] != false
        || !matches!(
            report["probeIsolation"]["networkMode"].as_str(),
            Some("deny-all" | "loopback-only")
        )
    {
        return Err("target is not the requested isolated probe".into());
    }
    unsafe {
        let owner_handle = Handle(
            OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, owner)
                .map_err(|e| e.to_string())?,
        );
        let mut exit = 0;
        GetExitCodeProcess(owner_handle.0, &mut exit).map_err(|e| e.to_string())?;
        if exit != 259 {
            return Err("owning inspector is not live".into());
        }
        let process = Handle(
            OpenProcess(
                PROCESS_QUERY_INFORMATION
                    | PROCESS_VM_READ
                    | if stop {
                        PROCESS_TERMINATE
                    } else {
                        PROCESS_ACCESS_RIGHTS(0)
                    },
                false,
                pid,
            )
            .map_err(|e| e.to_string())?,
        );
        let mut child_policy = PROCESS_MITIGATION_CHILD_PROCESS_POLICY::default();
        let child_policy_result = GetProcessMitigationPolicy(
            process.0,
            ProcessChildProcessPolicy,
            (&mut child_policy as *mut PROCESS_MITIGATION_CHILD_PROCESS_POLICY).cast(),
            size_of::<PROCESS_MITIGATION_CHILD_PROCESS_POLICY>(),
        );
        let modules_snapshot = Handle(
            CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid)
                .map_err(|e| e.to_string())?,
        );
        let mut entry = MODULEENTRY32W {
            dwSize: size_of::<MODULEENTRY32W>() as u32,
            ..Default::default()
        };
        let mut modules = Vec::new();
        let mut matched = false;
        let mut game = false;
        if Module32FirstW(modules_snapshot.0, &mut entry).is_ok() {
            loop {
                let name = String::from_utf16_lossy(
                    &entry.szModule[..entry
                        .szModule
                        .iter()
                        .position(|v| *v == 0)
                        .unwrap_or(entry.szModule.len())],
                );
                let path = String::from_utf16_lossy(
                    &entry.szExePath[..entry
                        .szExePath
                        .iter()
                        .position(|v| *v == 0)
                        .unwrap_or(entry.szExePath.len())],
                );
                if name.eq_ignore_ascii_case("arctic_bedrock.dll") {
                    matched = std::path::Path::new(&path).canonicalize().ok()
                        == root.join("arctic_bedrock.dll").canonicalize().ok();
                }
                game |= name.eq_ignore_ascii_case("Minecraft.Windows.exe");
                modules.push((entry.modBaseAddr as u64, entry.modBaseSize as u64, name));
                if modules.len() >= 1024 || Module32NextW(modules_snapshot.0, &mut entry).is_err() {
                    break;
                }
            }
        }
        if !matched || !game {
            return Err("owned game/DLL identity did not match".into());
        }
        let threads_snapshot =
            Handle(CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0).map_err(|e| e.to_string())?);
        let mut thread = THREADENTRY32 {
            dwSize: size_of::<THREADENTRY32>() as u32,
            ..Default::default()
        };
        let mut ids = Vec::new();
        let mut private = desktop::startup_desktop(process.0).is_some_and(|s| {
            s.rsplit('\\').next() == Some(format!("ArcticInspect_{owner}").as_str())
        });
        if Thread32First(threads_snapshot.0, &mut thread).is_ok() {
            loop {
                if thread.th32OwnerProcessID == pid {
                    ids.push(thread.th32ThreadID);
                    let desktop = GetThreadDesktop(thread.th32ThreadID);
                    let mut text = [0u16; 256];
                    let mut needed = 0;
                    if !desktop.is_null()
                        && GetUserObjectInformationW(
                            desktop,
                            2,
                            text.as_mut_ptr().cast(),
                            512,
                            &mut needed,
                        ) != 0
                        && needed <= 512
                    {
                        let name = String::from_utf16_lossy(
                            &text[..text.iter().position(|v| *v == 0).unwrap_or(text.len())],
                        );
                        private |= name == format!("ArcticInspect_{owner}");
                    }
                }
                if ids.len() >= 256 || Thread32Next(threads_snapshot.0, &mut thread).is_err() {
                    break;
                }
            }
        }
        if !private {
            return Err("private desktop ownership not verified".into());
        }
        if wake || focus {
            return wake_window(pid, owner, focus);
        }
        if stop {
            // Stop only this exact verified test-process handle. Its owning
            // debugger records the exit and retires its own children.
            TerminateProcess(process.0, 0x41524354).map_err(|e| e.to_string())?;
            return Ok(
                json!({"pid":pid,"inspector":owner,"privateDesktop":true,"stoppedForRebuild":true}),
            );
        }
        // Disable network symbol paths and prompts. Only module unwind metadata
        // is needed; no PDB or full symbol names are requested.
        SymSetOptions(
            SYMOPT_DEFERRED_LOADS
                | SYMOPT_FAIL_CRITICAL_ERRORS
                | SYMOPT_NO_PROMPTS
                | SYMOPT_IGNORE_NT_SYMPATH,
        );
        let empty = [0u16];
        SymInitializeW(process.0, PCWSTR(empty.as_ptr()), true).map_err(|e| e.to_string())?;
        let mut functions = Vec::new();
        let mut startup_target = None;
        if let Some((base, size, _)) = modules
            .iter()
            .find(|(_, _, name)| name.eq_ignore_ascii_case("Minecraft.Windows.exe"))
        {
            let diagnostics = &report["probeIsolation"]["connectionErrors"];
            if diagnostics["startupGate"]["targetRva"] == 0x5dc29e0u64 {
                startup_target = startup_virtual_target(process.0, *base, *size);
            }
            let mut addresses = diagnostics["progressCallers"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            addresses.push(diagnostics["startCallbackRva"].clone());
            if let Some(check) = &startup_target {
                addresses.push(json!(check.target_rva));
                addresses.extend(check.provider_targets.iter().map(|target| json!(target)));
            }
            for rva in addresses.iter().take(50).filter_map(|value| value.as_u64()) {
                if !(0x1000..0xe5a7000).contains(&rva) || rva >= *size {
                    continue;
                }
                let entry = SymFunctionTableAccess64(process.0, base + rva);
                if !entry.is_null() {
                    // DbgHelp returns a local RUNTIME_FUNCTION for AMD64. Only
                    // module-relative unwind ranges are copied, never symbols.
                    let entry = &*(entry as *const [u32; 3]);
                    let (begin, end) = (u64::from(entry[0]), u64::from(entry[1]));
                    if begin <= rva && rva < end && end <= *size {
                        functions.push(json!({"rva":rva,"begin":begin,"end":end}));
                    }
                }
            }
        }
        let mut output = Vec::new();
        for id in ids {
            let Ok(handle) = OpenThread(
                THREAD_SUSPEND_RESUME | THREAD_GET_CONTEXT | THREAD_QUERY_INFORMATION,
                false,
                id,
            ) else {
                continue;
            };
            let handle = Handle(handle);
            let mut created = FILETIME::default();
            let mut exited = FILETIME::default();
            let mut kernel = FILETIME::default();
            let mut user = FILETIME::default();
            let times = GetThreadTimes(handle.0, &mut created, &mut exited, &mut kernel, &mut user);
            let cpu = times.is_ok().then(|| {
                ((kernel.dwHighDateTime as u64) << 32 | kernel.dwLowDateTime as u64)
                    .saturating_add((user.dwHighDateTime as u64) << 32 | user.dwLowDateTime as u64)
            });
            if SuspendThread(handle.0) == u32::MAX {
                continue;
            }
            let suspended = Suspended(handle.0);
            #[repr(align(16))]
            struct Aligned(CONTEXT);
            let mut context = Aligned(CONTEXT {
                ContextFlags: CONTEXT_FULL_AMD64,
                ..Default::default()
            });
            if GetThreadContext(handle.0, &mut context.0).is_err() {
                continue;
            }
            let mut frame = STACKFRAME64::default();
            frame.AddrPC.Offset = context.0.Rip;
            frame.AddrPC.Mode = AddrModeFlat;
            frame.AddrStack.Offset = context.0.Rsp;
            frame.AddrStack.Mode = AddrModeFlat;
            frame.AddrFrame.Offset = context.0.Rbp;
            frame.AddrFrame.Mode = AddrModeFlat;
            let mut frames = Vec::new();
            let at = std::time::Instant::now();
            for _ in 0..32 {
                let pc = frame.AddrPC.Offset;
                if let Some((base, _, name)) = modules
                    .iter()
                    .find(|(base, size, _)| pc >= *base && pc - base < *size)
                {
                    frames.push(json!({"module":name,"rva":pc-base}));
                } else {
                    frames.push(json!({"module":"unknown"}));
                }
                if at.elapsed() > std::time::Duration::from_millis(50)
                    || !StackWalk64(
                        0x8664,
                        process.0,
                        handle.0,
                        &mut frame,
                        (&mut context.0 as *mut CONTEXT).cast(),
                        None,
                        Some(function_table),
                        Some(module_base),
                        None,
                    )
                    .as_bool()
                {
                    break;
                }
            }
            drop(suspended);
            output.push(json!({"thread":id,"cpu100ns":cpu,"frames":frames}));
        }
        let _ = SymCleanup(process.0);
        Ok(json!({"pid":pid,"inspector":owner,"privateDesktop":true,
            "childProcessPolicy":child_policy_result.ok().map(|_|child_policy.Anonymous.Flags),"startupCheck":startup_target,"functions":functions,"threads":output}))
    }
}
