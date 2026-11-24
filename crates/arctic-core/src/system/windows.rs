use std::ffi::OsString;
use std::fs;
use std::io;
use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};

use windows_sys::Wdk::System::SystemServices::RtlGetVersion;
use windows_sys::Win32::System::Registry::{
    HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RRF_SUBKEY_WOW6464KEY, RegGetValueW,
};
use windows_sys::Win32::System::SystemInformation::{GetSystemDirectoryW, OSVERSIONINFOW};
use windows_sys::core::w;

pub(crate) const UCRT: &str = "ucrtbase.dll";
pub(crate) const VCRUNTIME: &str = "vcruntime140.dll";

fn version() -> Option<OSVERSIONINFOW> {
    let mut info = OSVERSIONINFOW {
        dwOSVersionInfoSize: std::mem::size_of::<OSVERSIONINFOW>() as u32,
        ..Default::default()
    };
    // SAFETY: the writable structure starts with its size as required by RtlGetVersion.
    (unsafe { RtlGetVersion(&mut info) } == 0).then_some(info)
}

fn product_name() -> Option<String> {
    let mut name = [0u16; 256];
    let mut bytes = std::mem::size_of_val(&name) as u32;
    // SAFETY: the key and value names are NUL-terminated and the buffer holds `bytes` bytes.
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            w!("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion"),
            w!("ProductName"),
            RRF_RT_REG_SZ | RRF_SUBKEY_WOW6464KEY,
            std::ptr::null_mut(),
            name.as_mut_ptr().cast(),
            &mut bytes,
        )
    };
    if status != 0 {
        return None;
    }
    let chars = (bytes as usize / 2).saturating_sub(1).min(name.len());
    Some(String::from_utf16_lossy(&name[..chars]))
}

pub(crate) fn description() -> String {
    let product = product_name().unwrap_or_else(|| "product name unavailable".into());
    let Some(info) = version() else {
        return format!("{product} (version and build unavailable)");
    };
    // Windows 11 can still have a Windows 10 ProductName in this registry key.
    let product = if info.dwBuildNumber >= 22000 {
        match product.strip_prefix("Windows 10") {
            Some(edition) => format!("Windows 11{edition}"),
            None => product,
        }
    } else {
        product
    };
    format!(
        "{product} (version {}.{}, build {})",
        info.dwMajorVersion, info.dwMinorVersion, info.dwBuildNumber
    )
}

pub(crate) fn system_directory() -> io::Result<PathBuf> {
    let mut path = [0u16; 260];
    // SAFETY: the buffer holds the stated number of UTF-16 code units.
    let len = unsafe { GetSystemDirectoryW(path.as_mut_ptr(), path.len() as u32) } as usize;
    match len {
        0 => Err(io::Error::last_os_error()),
        n if n >= path.len() => Err(io::Error::other("Windows system directory path too long")),
        n => Ok(PathBuf::from(OsString::from_wide(&path[..n]))),
    }
}

pub(crate) fn has_dll(directory: &Path, name: &str) -> io::Result<bool> {
    match fs::metadata(directory.join(name)) {
        Ok(meta) => Ok(meta.is_file()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

pub(crate) fn can_use_system_ucrt() -> bool {
    // Windows 10 and later prefer the OS UCRT even when a bundled copy is present.
    version().is_some_and(|info| info.dwMajorVersion >= 10)
        && system_directory()
            .and_then(|dir| has_dll(&dir, UCRT))
            .is_ok_and(|present| present)
}
