//! An optional "Arctic Launcher" shortcut on the desktop (Settings → "Desktop shortcut"):
//! a `.lnk` on Windows, a `.desktop` launcher on Linux, a link to the app on macOS.

use std::path::{Path, PathBuf};

/// Make the shortcut and say where it went. Making it again just refreshes it.
pub fn create() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let desktop = desktop_folder().ok_or("couldn't find the desktop folder")?;
    if !desktop.is_dir() {
        return Err(format!("{} isn't a folder", desktop.display()));
    }
    write(&exe, &desktop)
}

#[cfg(windows)]
fn desktop_folder() -> Option<PathBuf> {
    // OneDrive moves the desktop when it backs it up.
    let home = std::env::var_os("USERPROFILE").map(PathBuf::from);
    let one_drive = ["OneDrive", "OneDriveConsumer", "OneDriveCommercial"]
        .iter()
        .filter_map(std::env::var_os)
        .map(|p| PathBuf::from(p).join("Desktop"));
    one_drive
        .chain(home.map(|h| h.join("Desktop")))
        .find(|p| p.is_dir())
}

#[cfg(not(windows))]
fn desktop_folder() -> Option<PathBuf> {
    let asked = std::process::Command::new("xdg-user-dir")
        .arg("DESKTOP")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| PathBuf::from(s.trim()))
        .filter(|p| p.is_dir());
    asked.or_else(|| {
        let home = std::env::var_os("HOME").map(PathBuf::from)?;
        Some(home.join("Desktop")).filter(|p| p.is_dir())
    })
}

#[cfg(windows)]
fn write(exe: &Path, desktop: &Path) -> Result<PathBuf, String> {
    let lnk = desktop.join("Arctic Launcher.lnk");
    crate::start_menu::write_link(exe, &lnk).map_err(|e| e.to_string())?;
    Ok(lnk)
}

#[cfg(target_os = "macos")]
fn write(exe: &Path, desktop: &Path) -> Result<PathBuf, String> {
    // Inside "Arctic Launcher.app/Contents/MacOS/": link the .app itself.
    let app = exe
        .ancestors()
        .find(|a| a.extension().is_some_and(|e| e == "app"))
        .ok_or("the launcher isn't running from an app bundle")?;
    let link = desktop.join(app.file_name().ok_or("no app name")?);
    if link.symlink_metadata().is_ok() {
        std::fs::remove_file(&link).map_err(|e| e.to_string())?;
    }
    std::os::unix::fs::symlink(app, &link).map_err(|e| e.to_string())?;
    Ok(link)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn write(exe: &Path, desktop: &Path) -> Result<PathBuf, String> {
    use std::os::unix::fs::PermissionsExt;
    let file = desktop.join("arctic-launcher.desktop");
    // Quoted, with the characters a Desktop Entry Exec line treats specially escaped.
    let quoted = exe
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('$', "\\$")
        .replace('`', "\\`");
    let text = format!(
        "[Desktop Entry]\nType=Application\nName=Arctic Launcher\nComment=A fast, lightweight Minecraft launcher\n\
         Exec=\"{quoted}\"\nTerminal=false\nCategories=Game;\n"
    );
    std::fs::write(&file, text).map_err(|e| e.to_string())?;
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| e.to_string())?;
    Ok(file)
}
