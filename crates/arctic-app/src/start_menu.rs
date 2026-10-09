//! Windows: an "Arctic Launcher" shortcut in the Start menu, so Windows search finds the launcher
//! (it's a single exe with no installer). Made on start, and again whenever the exe has moved.

use std::path::{Path, PathBuf};

use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoUninitialize, IPersistFile,
};
use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
use windows::core::{HSTRING, Interface};

const NAME: &str = "Arctic Launcher";
/// Which exe the shortcut was last made for (in the launcher's data folder).
const MARKER: &str = "start-menu-shortcut.txt";

/// Make or update the shortcut, on its own thread (COM and a file write; nothing waits on it).
pub fn ensure(data_root: &Path) {
    let data_root = data_root.to_path_buf();
    let spawned = std::thread::Builder::new()
        .name("start-menu".into())
        .spawn(move || {
            if let Err(e) = make(&data_root) {
                log::warn!("start menu shortcut: {e}");
            }
        });
    if let Err(e) = spawned {
        log::warn!("start menu shortcut: {e}");
    }
}

fn shortcut_path() -> Option<PathBuf> {
    let appdata = std::env::var_os("APPDATA")?;
    Some(
        PathBuf::from(appdata)
            .join(r"Microsoft\Windows\Start Menu\Programs")
            .join(format!("{NAME}.lnk")),
    )
}

fn make(data_root: &Path) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let lnk = shortcut_path().ok_or("no APPDATA folder")?;
    let marker = data_root.join(MARKER);
    let exe_text = exe.to_string_lossy().into_owned();
    // A build run straight from the source tree (tests, screenshots) isn't the installed launcher.
    if exe.components().any(|c| c.as_os_str() == "target") {
        return Ok(());
    }
    if lnk.is_file() && std::fs::read_to_string(&marker).is_ok_and(|m| m == exe_text) {
        return Ok(());
    }
    write_link(&exe, &lnk).map_err(|e| e.to_string())?;
    std::fs::write(&marker, exe_text).map_err(|e| e.to_string())?;
    log::info!("start menu shortcut made for {}", exe.display());
    Ok(())
}

fn write_link(exe: &Path, lnk: &Path) -> windows::core::Result<()> {
    // SAFETY: COM is set up for this thread only and torn down below; the shell link object
    // and its IPersistFile view are used on this thread and dropped before CoUninitialize.
    unsafe {
        let init = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let result = (|| {
            let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
            let exe_h = HSTRING::from(exe.as_os_str());
            link.SetPath(&exe_h)?;
            if let Some(dir) = exe.parent() {
                link.SetWorkingDirectory(&HSTRING::from(dir.as_os_str()))?;
            }
            link.SetDescription(&HSTRING::from(NAME))?;
            link.SetIconLocation(&exe_h, 0)?;
            let file: IPersistFile = link.cast()?;
            file.Save(&HSTRING::from(lnk.as_os_str()), true)
        })();
        if init.is_ok() {
            CoUninitialize();
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_a_shortcut_to_the_exe() {
        let dir = tempfile::tempdir().unwrap();
        let lnk = dir.path().join("Arctic Launcher.lnk");
        let exe = std::env::current_exe().unwrap();
        write_link(&exe, &lnk).unwrap();
        let bytes = std::fs::read(&lnk).unwrap();
        // A shell link starts with its header size (0x4C) and holds the target's path.
        assert_eq!(bytes[0], 0x4C);
        let name = exe.file_name().unwrap().to_string_lossy().into_owned();
        let wide: Vec<u8> = name.encode_utf16().flat_map(u16::to_le_bytes).collect();
        let found = bytes.windows(name.len()).any(|w| w == name.as_bytes())
            || bytes.windows(wide.len()).any(|w| w == wide.as_slice());
        assert!(found, "the shortcut doesn't name {name}");
    }
}
