//! Small headless encoder/pipe diagnostic, including optional probe isolation.
//! Synthetic pixels exercise the encoder only; this is never world/replay proof.
#![cfg_attr(windows, windows_subsystem = "windows")]
#[cfg(windows)]
#[path = "support/debug_notifications.rs"]
mod debug_notifications;

#[cfg(windows)]
fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|v| v == "--debug-private") && (4..=5).contains(&args.len()) {
        let root = std::path::PathBuf::from(&args[2]);
        let result = debug_private(
            &root,
            &args[3],
            args.get(4).is_some_and(|v| v == "--isolate"),
        )
        .unwrap_or_else(|error| serde_json::json!({"error":error}));
        let _ = std::fs::write(root.join("encoder-debug-report.json"), result.to_string());
        return;
    }
    if !(3..=4).contains(&args.len()) || args.get(3).is_some_and(|v| v != "--isolate") {
        return;
    }
    let root = std::path::PathBuf::from(&args[1]);
    let result = run(&root, &std::path::PathBuf::from(&args[2]), args.len() == 4)
        .unwrap_or_else(|error| serde_json::json!({"state":"error","message":error}));
    let report = serde_json::json!({"schema":1,"diagnosticOnly":true,"gameStarted":false,
        "source":"synthetic-encoder-preflight","parityVerified":false,"capture":result,
        "isolation":arctic_bedrock::probe_isolation::status()});
    let _ = std::fs::write(root.join("encoder-report.json"), report.to_string());
}

#[cfg(windows)]
fn debug_private(
    root: &std::path::Path,
    ffmpeg: &std::ffi::OsStr,
    isolated: bool,
) -> Result<serde_json::Value, String> {
    use hudhook::windows::Win32::Foundation::{
        CloseHandle, DBG_CONTINUE, DBG_EXCEPTION_NOT_HANDLED, HANDLE,
    };
    use hudhook::windows::Win32::System::{Diagnostics::Debug::*, Threading::*};
    use hudhook::windows::core::{HSTRING, PCWSTR, PWSTR};
    use std::collections::{HashMap, HashSet};
    use std::os::windows::ffi::OsStrExt;
    use std::time::{Duration, Instant};
    #[link(name = "user32")]
    unsafe extern "system" {
        fn CreateDesktopW(
            name: *const u16,
            device: *const u16,
            mode: *const std::ffi::c_void,
            flags: u32,
            access: u32,
            security: *const std::ffi::c_void,
        ) -> *mut std::ffi::c_void;
        fn CloseDesktop(desktop: *mut std::ffi::c_void) -> i32;
    }
    struct Desktop(*mut std::ffi::c_void);
    impl Drop for Desktop {
        fn drop(&mut self) {
            unsafe {
                CloseDesktop(self.0);
            }
        }
    }
    let name = HSTRING::from(format!("ArcticEncoder_{}", std::process::id()));
    let desktop = Desktop(unsafe {
        CreateDesktopW(
            name.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            0,
            0x1ff,
            std::ptr::null(),
        )
    });
    if desktop.0.is_null() {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let command = format!(
        "\"{}\" \"{}\" \"{}\"{}",
        executable.display(),
        root.display(),
        ffmpeg.to_string_lossy(),
        if isolated { " --isolate" } else { "" }
    );
    let mut command: Vec<u16> = std::ffi::OsStr::new(&command)
        .encode_wide()
        .chain(Some(0))
        .collect();
    let application = HSTRING::from(executable.as_os_str());
    let startup = STARTUPINFOW {
        cb: size_of::<STARTUPINFOW>() as u32,
        lpDesktop: PWSTR(name.as_ptr() as *mut u16),
        ..Default::default()
    };
    let mut process = PROCESS_INFORMATION::default();
    unsafe {
        CreateProcessW(
            PCWSTR(application.as_ptr()),
            Some(PWSTR(command.as_mut_ptr())),
            None,
            None,
            false,
            DEBUG_PROCESS | CREATE_NO_WINDOW,
            None,
            None,
            &startup,
            &mut process,
        )
        .map_err(|e| e.to_string())?;
        DebugSetProcessKillOnExit(true).map_err(|e| e.to_string())?;
        let _ = CloseHandle(process.hThread);
        let _ = CloseHandle(process.hProcess);
    }
    let encoder = std::path::Path::new(ffmpeg)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let mut encoders = HashSet::new();
    let mut notifications = 0;
    let mut live = HashMap::<u32, HANDLE>::new();
    let mut initial = HashSet::new();
    let mut exits = Vec::new();
    let mut exceptions = Vec::new();
    let until = Instant::now() + Duration::from_secs(30);
    let mut root_exited = false;
    while Instant::now() < until && !root_exited {
        let mut event = DEBUG_EVENT::default();
        if unsafe { WaitForDebugEvent(&mut event, 100) }.is_err() {
            continue;
        }
        let mut disposition = DBG_CONTINUE;
        unsafe {
            match event.dwDebugEventCode {
                CREATE_PROCESS_DEBUG_EVENT => {
                    let info = event.u.CreateProcessInfo;
                    live.insert(event.dwProcessId, info.hProcess);
                    let mut image = [0u16; 32768];
                    let mut length = image.len() as u32;
                    if QueryFullProcessImageNameW(
                        info.hProcess,
                        PROCESS_NAME_WIN32,
                        PWSTR(image.as_mut_ptr()),
                        &mut length,
                    )
                    .is_ok()
                        && let Ok(path) = std::path::PathBuf::from(String::from_utf16_lossy(
                            &image[..length as usize],
                        ))
                        .canonicalize()
                        && path
                            .to_string_lossy()
                            .eq_ignore_ascii_case(&encoder.to_string_lossy())
                    {
                        encoders.insert(event.dwProcessId);
                    }
                    let _ = CloseHandle(info.hThread);
                    if !info.hFile.is_invalid() {
                        let _ = CloseHandle(info.hFile);
                    }
                }
                LOAD_DLL_DEBUG_EVENT => {
                    let file = event.u.LoadDll.hFile;
                    if !file.is_invalid() {
                        let _ = CloseHandle(file);
                    }
                }
                EXIT_PROCESS_DEBUG_EVENT => {
                    exits.push(serde_json::json!({"pid":event.dwProcessId,"exitCode":event.u.ExitProcess.dwExitCode}));
                    if let Some(handle) = live.remove(&event.dwProcessId) {
                        let _ = CloseHandle(handle);
                    }
                    root_exited = event.dwProcessId == process.dwProcessId;
                }
                EXCEPTION_DEBUG_EVENT => {
                    let code = event.u.Exception.ExceptionRecord.ExceptionCode.0 as u32;
                    if debug_notifications::owned_encoder_thread_name(
                        &event.u.Exception,
                        encoders.contains(&event.dwProcessId),
                    ) {
                        notifications += 1;
                    } else if code != 0x80000003 || !initial.insert(event.dwProcessId) {
                        let record = event.u.Exception.ExceptionRecord;
                        let count = (record.NumberParameters as usize)
                            .min(record.ExceptionInformation.len());
                        exceptions.push(serde_json::json!({"pid":event.dwProcessId,"code":format!("{code:08x}"),"firstChance":event.u.Exception.dwFirstChance!=0,
                            "flags":record.ExceptionFlags,"parameters":record.ExceptionInformation[..count]}));
                        disposition = DBG_EXCEPTION_NOT_HANDLED;
                    }
                }
                _ => {}
            }
            ContinueDebugEvent(event.dwProcessId, event.dwThreadId, disposition)
                .map_err(|e| e.to_string())?;
        }
    }
    for handle in live.into_values() {
        unsafe {
            let _ = TerminateProcess(handle, 1);
            let _ = CloseHandle(handle);
        }
    }
    Ok(
        serde_json::json!({"schema":1,"gameStarted":false,"diagnosticOnly":true,"privateDesktop":true,
        "rootExited":root_exited,"exits":exits,"exceptions":exceptions,"encoderThreadNameNotifications":notifications}),
    )
}

#[cfg(not(windows))]
fn main() {}

#[cfg(windows)]
fn run(
    root: &std::path::Path,
    ffmpeg: &std::path::Path,
    isolated: bool,
) -> Result<serde_json::Value, String> {
    use arctic_bedrock::capture;
    use std::time::{Duration, Instant};
    if !root.is_absolute() || !root.is_dir() {
        return Err("expected a fresh absolute diagnostic directory".into());
    }
    if isolated {
        let source = root.join("source");
        let profile = root.join("profile");
        std::fs::create_dir(&source).map_err(|e| e.to_string())?;
        std::fs::create_dir(&profile).map_err(|e| e.to_string())?;
        arctic_bedrock::probe_isolation::install_loopback(&profile, &source)?;
    }
    capture::start_control(root.to_owned());
    capture::command(
        serde_json::json!({"op":"start","ffmpeg":ffmpeg,"output":root.join("encoder.mp4"),
        "fps":24,"frames":2})
        .to_string()
        .as_bytes(),
    )?;
    let pixels = [20, 80, 160, 255].repeat(32 * 32);
    capture::frame(32, 32, pixels.clone(), None);
    std::thread::sleep(Duration::from_millis(50));
    capture::frame(32, 32, pixels, None);
    let until = Instant::now() + Duration::from_secs(20);
    while Instant::now() < until {
        let status = capture::status();
        if matches!(status["state"].as_str(), Some("complete" | "error")) {
            return Ok(status);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Err("encoder preflight exceeded its completion deadline".into())
}
