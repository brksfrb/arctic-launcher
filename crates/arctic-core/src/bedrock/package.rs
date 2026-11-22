//! The game's Windows package, through Windows' own package manager: find
//! it, start it, and install another version over it (the Store license on
//! this computer unlocks the encrypted package).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

use windows::ApplicationModel::Package;
use windows::Foundation::Uri;
use windows::Management::Deployment::{
    AddPackageOptions, DeploymentProgress, DeploymentResult, PackageManager, RemovalOptions,
};
use windows::core::HSTRING;

use crate::{Error, ProgressInfo, Result};

fn package(family: &str) -> windows::core::Result<Option<Package>> {
    let manager = PackageManager::new()?;
    let found =
        manager.FindPackagesByUserSecurityIdPackageFamilyName(&HSTRING::new(), &family.into())?;
    let it = found.First()?;
    if it.HasCurrent()? {
        Ok(Some(it.Current()?))
    } else {
        Ok(None)
    }
}

/// (package version "1.26.5203.0", install folder) of the package family.
pub(super) fn find(family: &str) -> Option<(String, PathBuf)> {
    let package = package(family).ok().flatten()?;
    let v = package.Id().ok()?.Version().ok()?;
    let path = package.InstalledPath().ok()?;
    Some((
        format!("{}.{}.{}.{}", v.Major, v.Minor, v.Build, v.Revision),
        PathBuf::from(path.to_string_lossy()),
    ))
}

/// Start the package's app.
pub(super) fn launch(family: &str) -> Result<()> {
    let run = || -> windows::core::Result<bool> {
        let Some(package) = package(family)? else {
            return Ok(false);
        };
        let entries = package.GetAppListEntries()?;
        if entries.Size()? == 0 {
            return Ok(false);
        }
        entries.GetAt(0)?.LaunchAsync()?.join()
    };
    match run() {
        Ok(true) => Ok(()),
        Ok(false) => Err(Error::Other("Windows didn't start it".into())),
        Err(e) => Err(Error::Other(e.message())),
    }
}

/// Take the installed package off the computer, keeping its data (an older install keeps its worlds
/// and settings inside the package's data folder).
pub(super) fn remove(family: &str) -> Result<()> {
    let run = || -> windows::core::Result<()> {
        let Some(package) = package(family)? else {
            return Ok(());
        };
        let name = package.Id()?.FullName()?;
        PackageManager::new()?
            .RemovePackageWithOptionsAsync(&name, RemovalOptions::PreserveApplicationData)?
            .join()?;
        Ok(())
    };
    run().map_err(|e| {
        Error::Other(format!(
            "couldn't remove the installed game: {}",
            install_error(&e)
        ))
    })
}

/// Install the package at `url` over whatever version is there (newer or
/// older), closing the game if it's running. Windows downloads it itself.
pub(super) fn install(
    url: &str,
    stage: &str,
    cancel: &AtomicBool,
    progress: crate::Progress<'_>,
) -> Result<()> {
    let percent = Arc::new(AtomicU32::new(0));
    let operation = (|| {
        let manager = PackageManager::new()?;
        let options = AddPackageOptions::new()?;
        options.SetForceUpdateFromAnyVersion(true)?;
        options.SetForceAppShutdown(true)?;
        let operation = manager.AddPackageByUriAsync(&Uri::CreateUri(&url.into())?, &options)?;
        let seen = percent.clone();
        operation.SetProgress(&windows_future::AsyncOperationProgressHandler::<
            DeploymentResult,
            DeploymentProgress,
        >::new(move |_, p| {
            if let Some(p) = p.as_ref() {
                seen.store(p.percentage, Ordering::Relaxed);
            }
            Ok(())
        }))?;
        windows::core::Result::Ok(operation)
    })()
    .map_err(|e| Error::Other(install_error(&e)))?;
    let finished = AtomicBool::new(false);
    let result = std::thread::scope(|s| {
        s.spawn(|| {
            while !finished.load(Ordering::Relaxed) {
                if cancel.load(Ordering::Relaxed) {
                    let _ = operation.Cancel();
                }
                progress(ProgressInfo {
                    stage,
                    done: u64::from(percent.load(Ordering::Relaxed)),
                    total: 100,
                    ..ProgressInfo::default()
                });
                std::thread::sleep(Duration::from_millis(200));
            }
        });
        let result = operation.join();
        finished.store(true, Ordering::Relaxed);
        result
    });
    match result {
        Ok(_) => Ok(()),
        Err(_) if cancel.load(Ordering::Relaxed) => Err(Error::Other("cancelled".into())),
        Err(e) => {
            // The deployment's own text says more than the bare HRESULT.
            let text = operation
                .GetResults()
                .ok()
                .and_then(|r| r.ErrorText().ok())
                .map(|t| t.to_string_lossy())
                .filter(|t| !t.trim().is_empty());
            Err(Error::Other(match text {
                Some(text) => format!("{} ({})", install_error(&e), text.trim()),
                None => install_error(&e),
            }))
        }
    }
}

/// Why an install failed, with the usual cause spelled out.
fn install_error(e: &windows::core::Error) -> String {
    let code = e.code().0 as u32;
    let hint = match code {
        // Not owned, or the license couldn't be checked.
        0x8007_0005 | 0x8007_32B8 | 0x8007_3CF0 | 0x8007_3D25 | 0x8007_3CFB => {
            " Make sure you own Minecraft on the Microsoft account signed in to the Microsoft Store, and that Gaming Services is installed."
        }
        // Developer mode / sideloading off for a package that needs it.
        0x8007_3CFF => {
            " Windows refused the package (check that app installs are allowed in Settings)."
        }
        // Not enough space.
        0x8007_0070 => " There isn't enough free space on the drive.",
        _ => "",
    };
    format!("{} [0x{code:08X}]{hint}", e.message().trim())
}

/// Does the game run? (See [`process_id`].)
pub(super) fn running(install_dir: &Path, family: &str) -> bool {
    process_id(install_dir, family).is_some()
}

/// Find the game by executable name, with package identity or path to distinguish editions.
pub(super) fn process_id(install_dir: &Path, family: &str) -> Option<u32> {
    find_process(install_dir, family, false)
}

/// GDK may launch an executable whose package identity and image path cannot
/// confirm its edition. Permit an unambiguous name match when only one edition
/// is installed.
pub(super) fn find_process(
    install_dir: &Path,
    family: &str,
    allow_name_match: bool,
) -> Option<u32> {
    use windows_sys::Win32::System::RemoteDesktop::ProcessIdToSessionId;

    let mut current_session = 0;
    // SAFETY: the output points to a writable session ID.
    if unsafe { ProcessIdToSessionId(std::process::id(), &mut current_session) } == 0 {
        log::debug!(
            "Bedrock process poll: launcher session ID lookup failed: {}",
            std::io::Error::last_os_error()
        );
        return None;
    }
    let dir = install_dir.to_string_lossy().to_lowercase();
    let dir = dir.trim_start_matches(r"\\?\").trim_end_matches('\\');
    let package_folder = format!(
        r"\{}_",
        family.split('_').next().unwrap_or(family).to_lowercase()
    );
    let mut candidates = 0;
    let mut unverified = Vec::new();
    let found = visit_game_processes(|pid| {
        candidates += 1;
        let mut session = 0;
        // SAFETY: the output points to a writable session ID.
        if unsafe { ProcessIdToSessionId(pid, &mut session) } == 0 {
            log::debug!(
                "Bedrock process poll: PID {pid} session lookup failed: {}",
                std::io::Error::last_os_error()
            );
            return false;
        }
        if session != current_session {
            log::debug!(
                "Bedrock process poll: PID {pid} belongs to session {session}, expected {current_session}"
            );
            return false;
        }
        match process_identity(pid, family, dir, &package_folder) {
            Some(matches) => matches,
            None => {
                unverified.push(pid);
                false
            }
        }
    });
    match found {
        Err(e) => {
            log::debug!("Bedrock process poll: {e}");
            None
        }
        Ok(Some(pid)) => Some(pid),
        Ok(None) if candidates == 0 => {
            log::debug!("Bedrock process poll: snapshot contains no Minecraft.Windows.exe");
            None
        }
        Ok(None) => match unverified.as_slice() {
            [pid] if allow_name_match => {
                log::debug!(
                    "Bedrock process poll: using PID {pid}, the sole unverified Minecraft.Windows.exe in this session"
                );
                Some(*pid)
            }
            _ => {
                log::debug!(
                    "Bedrock process poll: no confirmed match for {family}; unverified PIDs: {unverified:?}, name fallback enabled: {allow_name_match}"
                );
                None
            }
        },
    }
}

/// A known package identity takes precedence. Unpackaged processes can still
/// be identified by their executable path.
fn process_identity(pid: u32, family: &str, dir: &str, package_folder: &str) -> Option<bool> {
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::Storage::Packaging::Appx::{
        GetPackageFamilyName, PACKAGE_FAMILY_NAME_MAX_LENGTH,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
        QueryFullProcessImageNameW,
    };

    // SAFETY: the returned handle is checked and owned below.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        log::debug!(
            "Bedrock process poll: OpenProcess for PID {pid} failed: {}",
            std::io::Error::last_os_error()
        );
        return None;
    }
    // SAFETY: OpenProcess returned a valid handle owned by this call.
    let process = unsafe { OwnedHandle::from_raw_handle(process) };
    let mut name = [0u16; PACKAGE_FAMILY_NAME_MAX_LENGTH as usize + 1];
    let mut size = name.len() as u32;
    // SAFETY: both output parameters are writable and size describes the buffer.
    let status =
        unsafe { GetPackageFamilyName(process.as_raw_handle(), &mut size, name.as_mut_ptr()) };
    if status == ERROR_SUCCESS {
        let actual = String::from_utf16_lossy(&name[..size.saturating_sub(1) as usize]);
        let matches = actual.eq_ignore_ascii_case(family);
        if !matches {
            log::debug!(
                "Bedrock process poll: PID {pid} package family is {actual}, expected {family}"
            );
        }
        return Some(matches);
    }
    log::debug!("Bedrock process poll: GetPackageFamilyName for PID {pid} returned {status}");

    let mut path = [0u16; 1024];
    let mut size = path.len() as u32;
    // SAFETY: both output parameters are writable and size describes the buffer.
    if unsafe {
        QueryFullProcessImageNameW(
            process.as_raw_handle(),
            PROCESS_NAME_WIN32,
            path.as_mut_ptr(),
            &mut size,
        )
    } == 0
    {
        log::debug!(
            "Bedrock process poll: QueryFullProcessImageNameW for PID {pid} failed: {}",
            std::io::Error::last_os_error()
        );
        return None;
    }
    let path = String::from_utf16_lossy(&path[..size as usize]).to_lowercase();
    let path = path.trim_start_matches(r"\\?\");
    if path
        .strip_prefix(dir)
        .is_some_and(|rest| rest.starts_with('\\'))
        || path.contains(package_folder)
    {
        return Some(true);
    }
    log::debug!(
        "Bedrock process poll: PID {pid} image path {path} matches neither {dir} nor {package_folder}"
    );
    None
}

/// Check the chosen PID without redoing edition detection on later polls.
pub(super) fn game_process_exists(pid: u32) -> bool {
    match visit_game_processes(|candidate| candidate == pid) {
        Ok(Some(_)) => true,
        Ok(None) => {
            log::debug!("Bedrock process poll: Minecraft.Windows.exe PID {pid} is absent");
            false
        }
        Err(e) => {
            log::debug!("Bedrock process poll: couldn't recheck PID {pid}: {e}");
            true
        }
    }
}

fn visit_game_processes(
    mut matches: impl FnMut(u32) -> bool,
) -> std::result::Result<Option<u32>, String> {
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use windows_sys::Win32::Foundation::{ERROR_NO_MORE_FILES, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };

    // SAFETY: the process snapshot takes no pointers and returns an owned handle on success.
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Err(format!(
            "CreateToolhelp32Snapshot failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    // SAFETY: the snapshot is valid and owned by this call.
    let snapshot = unsafe { OwnedHandle::from_raw_handle(snapshot) };
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut next: unsafe extern "system" fn(_, _) -> _ = Process32FirstW;
    let mut operation = "Process32FirstW";
    loop {
        // SAFETY: the handle is live and entry is sized for Process32FirstW/Process32NextW.
        if unsafe { next(snapshot.as_raw_handle(), &mut entry) } == 0 {
            let error = std::io::Error::last_os_error();
            return if error.raw_os_error() == Some(ERROR_NO_MORE_FILES as i32) {
                Ok(None)
            } else {
                Err(format!("{operation} failed: {error}"))
            };
        }
        let len = entry
            .szExeFile
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(entry.szExeFile.len());
        if String::from_utf16_lossy(&entry.szExeFile[..len])
            .eq_ignore_ascii_case("Minecraft.Windows.exe")
            && matches(entry.th32ProcessID)
        {
            return Ok(Some(entry.th32ProcessID));
        }
        next = Process32NextW;
        operation = "Process32NextW";
    }
}

/// Does the process `pid` own a visible window yet? The game's graphics come up with it, which is when
/// a client can be loaded.
pub(super) fn has_window(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowThreadProcessId, IsWindowVisible,
    };

    struct Search {
        pid: u32,
        found: bool,
    }
    unsafe extern "system" fn visit(window: HWND, param: LPARAM) -> i32 {
        // SAFETY: `param` is the `Search` passed to `EnumWindows` below, alive for the whole call.
        let search = unsafe { &mut *(param as *mut Search) };
        let mut owner = 0u32;
        // SAFETY: plain Win32 calls on the window handle Windows gave us.
        unsafe {
            GetWindowThreadProcessId(window, &mut owner);
            if owner == search.pid && IsWindowVisible(window) != 0 {
                search.found = true;
                return 0;
            }
        }
        1
    }
    let mut search = Search { pid, found: false };
    // SAFETY: the callback only touches `search`, which outlives the call.
    unsafe {
        EnumWindows(Some(visit), &mut search as *mut Search as LPARAM);
    }
    search.found
}
