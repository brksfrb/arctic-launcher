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
    AddPackageOptions, DeploymentProgress, DeploymentResult, PackageManager,
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

/// Does a process run from inside `install_dir`?
pub(super) fn running(install_dir: &Path) -> bool {
    process_id(install_dir).is_some()
}

/// The id of the game's process running from inside `install_dir`.
pub(super) fn process_id(install_dir: &Path) -> Option<u32> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
        QueryFullProcessImageNameW,
    };
    let dir = install_dir.to_string_lossy().to_lowercase();
    // SAFETY: plain Win32 calls with buffers sized as declared; every handle is closed.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return None;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut found = None;
        let mut more = Process32FirstW(snapshot, &mut entry) != 0;
        while more && found.is_none() {
            let len = entry
                .szExeFile
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
            if name.eq_ignore_ascii_case("Minecraft.Windows.exe") {
                let process =
                    OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, entry.th32ProcessID);
                if !process.is_null() {
                    let mut buf = [0u16; 1024];
                    let mut size = buf.len() as u32;
                    if QueryFullProcessImageNameW(
                        process,
                        PROCESS_NAME_WIN32,
                        buf.as_mut_ptr(),
                        &mut size,
                    ) != 0
                    {
                        let path = String::from_utf16_lossy(&buf[..size as usize]).to_lowercase();
                        if path.starts_with(&dir) {
                            found = Some(entry.th32ProcessID);
                        }
                    }
                    CloseHandle(process);
                }
            }
            more = Process32NextW(snapshot, &mut entry) != 0;
        }
        CloseHandle(snapshot);
        found
    }
}
