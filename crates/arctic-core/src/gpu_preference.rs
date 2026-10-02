//! Laptops with two graphics cards (a built-in one and a gaming one) often
//! run Java on the built-in one, which is many times slower. Windows keeps
//! a per-program choice (Settings → System → Display → Graphics); the game
//! is set to the high-performance card there, unless the player already
//! made a choice for it.

use std::path::Path;

/// What Windows' Graphics settings write for "High performance".
#[cfg_attr(not(windows), allow(dead_code))]
const HIGH_PERFORMANCE: &str = "GpuPreference=2;";

/// Ask Windows to run `exe` on the high-performance graphics card (`prefer`),
/// or take back that request (`!prefer`). A choice the player made for it in
/// Windows' settings is left as it is. Best effort: logs and carries on.
pub fn apply(exe: &Path, prefer: bool) {
    #[cfg(windows)]
    {
        // The game may be started through a junction; Windows can know it by either path.
        let mut paths = vec![exe.to_path_buf()];
        if let Ok(real) = std::fs::canonicalize(exe) {
            let real = strip_verbatim(&real);
            if real != exe {
                paths.push(real);
            }
        }
        for path in paths {
            if let Err(code) = windows::apply(&path, prefer) {
                log::info!("graphics card preference for {}: error {code}", path.display());
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (exe, prefer);
    }
}

/// `\\?\C:\…` (from canonicalize) as `C:\…`, the form Windows' settings use.
#[cfg(windows)]
fn strip_verbatim(path: &Path) -> std::path::PathBuf {
    let text = path.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC\\") => std::path::PathBuf::from(rest),
        _ => path.to_path_buf(),
    }
}

#[cfg(windows)]
mod windows {
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;
    use std::ptr::{null, null_mut};

    use windows_sys::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey,
        RegCreateKeyExW, RegDeleteValueW, RegQueryValueExW, RegSetValueExW,
    };

    use super::HIGH_PERFORMANCE;

    const KEY: &str = r"Software\Microsoft\DirectX\UserGpuPreferences";
    const ERROR_SUCCESS: u32 = 0;
    const ERROR_FILE_NOT_FOUND: u32 = 2;

    fn wide(text: &std::ffi::OsStr) -> Vec<u16> {
        text.encode_wide().chain(Some(0)).collect()
    }

    struct Key(HKEY);

    impl Drop for Key {
        fn drop(&mut self) {
            unsafe {
                RegCloseKey(self.0);
            }
        }
    }

    pub(super) fn apply(exe: &Path, prefer: bool) -> Result<(), u32> {
        let key_path = wide(std::ffi::OsStr::new(KEY));
        let name = wide(exe.as_os_str());
        let mut handle: HKEY = null_mut();
        let status = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                key_path.as_ptr(),
                0,
                null(),
                REG_OPTION_NON_VOLATILE,
                KEY_QUERY_VALUE | KEY_SET_VALUE,
                null(),
                &mut handle,
                null_mut(),
            )
        };
        if status != ERROR_SUCCESS {
            return Err(status);
        }
        let key = Key(handle);
        let current = read(&key, &name)?;
        match (prefer, current.as_deref()) {
            // Not chosen yet: the high-performance card.
            (true, None) => write(&key, &name),
            // Taken back only if it's what we'd have set (not another choice the player made).
            (false, Some(HIGH_PERFORMANCE)) => {
                let status = unsafe { RegDeleteValueW(key.0, name.as_ptr()) };
                if status == ERROR_SUCCESS { Ok(()) } else { Err(status) }
            }
            _ => Ok(()),
        }
    }

    fn read(key: &Key, name: &[u16]) -> Result<Option<String>, u32> {
        let mut bytes: u32 = 0;
        let status = unsafe { RegQueryValueExW(key.0, name.as_ptr(), null(), null_mut(), null_mut(), &mut bytes) };
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        if status != ERROR_SUCCESS {
            return Err(status);
        }
        let mut buffer = vec![0u16; (bytes as usize).div_ceil(2)];
        let status = unsafe {
            RegQueryValueExW(key.0, name.as_ptr(), null(), null_mut(), buffer.as_mut_ptr().cast(), &mut bytes)
        };
        if status != ERROR_SUCCESS {
            return Err(status);
        }
        let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
        Ok(Some(String::from_utf16_lossy(&buffer[..end])))
    }

    fn write(key: &Key, name: &[u16]) -> Result<(), u32> {
        let data = wide(std::ffi::OsStr::new(HIGH_PERFORMANCE));
        let status = unsafe {
            RegSetValueExW(key.0, name.as_ptr(), 0, REG_SZ, data.as_ptr().cast(), (data.len() * 2) as u32)
        };
        if status == ERROR_SUCCESS { Ok(()) } else { Err(status) }
    }
}
