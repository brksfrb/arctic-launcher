//! Laptops with two graphics cards (a built-in one and a gaming one) often
//! run Java on the built-in one, which is many times slower. Windows keeps
//! a per-program choice (Settings → System → Display → Graphics); the game
//! is set to the high-performance card there, unless the player already
//! made a choice for it. On Linux the game is started with the variables
//! that send it to the other card (PRIME render offload: NVIDIA's driver or
//! Mesa's `DRI_PRIME`). macOS already runs OpenGL games on the gaming card.

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
                log::info!(
                    "graphics card preference for {}: error {code}",
                    path.display()
                );
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (exe, prefer);
    }
}

/// PCI vendor id of NVIDIA graphics cards.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
const NVIDIA: u16 = 0x10de;

/// Environment for the game on the gaming card (`prefer`): set on Linux laptops with two
/// cards, unless the player set these variables themselves.
pub fn env(prefer: bool) -> Vec<(&'static str, &'static str)> {
    #[cfg(target_os = "linux")]
    {
        if prefer {
            return offload_env(&linux_gpu_vendors(), |name| {
                std::env::var_os(name).is_some()
            });
        }
    }
    let _ = prefer;
    Vec::new()
}

/// The offload variables for a computer with these graphics cards: NVIDIA's driver with
/// another card next to it, or two cards of other makers (Mesa). One card: nothing to choose.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn offload_env(
    vendors: &[u16],
    already_set: impl Fn(&str) -> bool,
) -> Vec<(&'static str, &'static str)> {
    if vendors.len() < 2 {
        return Vec::new();
    }
    let hybrid_nvidia = vendors.contains(&NVIDIA) && vendors.iter().any(|v| *v != NVIDIA);
    let wanted: &[(&'static str, &'static str)] = if hybrid_nvidia {
        &[
            ("__NV_PRIME_RENDER_OFFLOAD", "1"),
            ("__GLX_VENDOR_LIBRARY_NAME", "nvidia"),
            ("__VK_LAYER_NV_optimus", "NVIDIA_only"),
        ]
    } else {
        &[("DRI_PRIME", "1")]
    };
    if wanted.iter().any(|(name, _)| already_set(name)) {
        return Vec::new();
    }
    wanted.to_vec()
}

/// The vendor of each graphics card the kernel knows (`/sys/class/drm/cardN`).
#[cfg(target_os = "linux")]
fn linux_gpu_vendors() -> Vec<u16> {
    let Ok(entries) = std::fs::read_dir("/sys/class/drm") else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            name.strip_prefix("card")
                .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
        })
        .filter_map(|e| std::fs::read_to_string(e.path().join("device/vendor")).ok())
        .filter_map(|v| u16::from_str_radix(v.trim().trim_start_matches("0x"), 16).ok())
        .collect()
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
        HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
        RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegQueryValueExW, RegSetValueExW,
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
                if status == ERROR_SUCCESS {
                    Ok(())
                } else {
                    Err(status)
                }
            }
            _ => Ok(()),
        }
    }

    fn read(key: &Key, name: &[u16]) -> Result<Option<String>, u32> {
        let mut bytes: u32 = 0;
        let status = unsafe {
            RegQueryValueExW(
                key.0,
                name.as_ptr(),
                null(),
                null_mut(),
                null_mut(),
                &mut bytes,
            )
        };
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        if status != ERROR_SUCCESS {
            return Err(status);
        }
        let mut buffer = vec![0u16; (bytes as usize).div_ceil(2)];
        let status = unsafe {
            RegQueryValueExW(
                key.0,
                name.as_ptr(),
                null(),
                null_mut(),
                buffer.as_mut_ptr().cast(),
                &mut bytes,
            )
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
            RegSetValueExW(
                key.0,
                name.as_ptr(),
                0,
                REG_SZ,
                data.as_ptr().cast(),
                (data.len() * 2) as u32,
            )
        };
        if status == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(status)
        }
    }
}

#[cfg(test)]
mod offload_tests {
    use super::*;

    const INTEL: u16 = 0x8086;
    const AMD: u16 = 0x1002;

    #[test]
    fn nvidia_laptops_use_the_nvidia_offload() {
        let env = offload_env(&[INTEL, NVIDIA], |_| false);
        assert!(env.contains(&("__NV_PRIME_RENDER_OFFLOAD", "1")));
    }

    #[test]
    fn other_pairs_use_mesa_and_one_card_needs_nothing() {
        assert_eq!(
            offload_env(&[INTEL, AMD], |_| false),
            vec![("DRI_PRIME", "1")]
        );
        assert!(offload_env(&[NVIDIA], |_| false).is_empty());
    }

    #[test]
    fn the_players_own_variables_win() {
        assert!(offload_env(&[INTEL, AMD], |n| n == "DRI_PRIME").is_empty());
    }
}
