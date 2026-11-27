//! Desktop shortcuts for the launcher itself and for playing a specific instance.

use crate::{t, tf};
use arctic_core::instances::Instance;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const WINDOWS_LAUNCHER_SHORTCUT: &str = "Arctic Launcher.lnk"; // i18n-ignore
const LINUX_LAUNCHER_SHORTCUT: &str = "arctic-launcher.desktop";

/// Make the shortcut and say where it went. Making it again just refreshes it.
pub fn create() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    write(&exe, &desktop()?)
}

pub fn create_instance(instance: &Instance) -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    write_instance(&exe, &desktop()?, &instance.name, &instance.id)
}

fn desktop() -> Result<PathBuf, String> {
    let desktop = desktop_folder().ok_or(t!("nav.couldnt_find_the_desktop_folder"))?;
    if !desktop.is_dir() {
        return Err(tf!("nav.isnt_a_folder", desktop = desktop.display()));
    }
    Ok(desktop)
}

#[cfg(windows)]
fn desktop_folder() -> Option<PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{FOLDERID_Desktop, KF_FLAG_DEFAULT, SHGetKnownFolderPath};

    // SAFETY: the Shell returns a NUL-terminated UTF-16 path allocated with CoTaskMemAlloc.
    if let Ok(path) = unsafe { SHGetKnownFolderPath(&FOLDERID_Desktop, KF_FLAG_DEFAULT, None) } {
        let desktop = unsafe {
            let desktop = std::ffi::OsString::from_wide(path.as_wide());
            CoTaskMemFree(Some(path.as_ptr().cast()));
            PathBuf::from(desktop)
        };
        if desktop.is_dir() {
            return Some(desktop);
        }
    }

    let home = std::env::var_os("USERPROFILE").map(PathBuf::from); // i18n-ignore
    let one_drive = ["OneDrive", "OneDriveConsumer", "OneDriveCommercial"] // i18n-ignore
        .iter()
        .filter_map(std::env::var_os)
        .map(|p| PathBuf::from(p).join("Desktop"));
    one_drive
        .chain(home.map(|h| h.join("Desktop")))
        .find(|p| p.is_dir())
}

#[cfg(target_os = "macos")]
fn desktop_folder() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("HOME")?).join("Desktop"))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn desktop_folder() -> Option<PathBuf> {
    let asked = std::process::Command::new("xdg-user-dir")
        .arg("DESKTOP")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| PathBuf::from(s.trim()))
        .filter(|p| p.is_absolute() && p.is_dir());
    let home = std::env::var_os("HOME").map(PathBuf::from); // i18n-ignore
    let configured = std::env::var_os("XDG_DESKTOP_DIR").map(PathBuf::from);
    asked
        .or_else(|| {
            let path = configured?;
            let path = match path.strip_prefix("$HOME") {
                Ok(relative) => home.as_ref()?.join(relative),
                Err(_) => path,
            };
            (path.is_absolute() && path.is_dir()).then_some(path)
        })
        .or_else(|| home.map(|home| home.join("Desktop")))
}

#[cfg(windows)]
fn write(exe: &Path, desktop: &Path) -> Result<PathBuf, String> {
    let lnk = desktop.join(WINDOWS_LAUNCHER_SHORTCUT);
    crate::start_menu::write_link(exe, &lnk).map_err(|e| e.to_string())?;
    Ok(lnk)
}

#[cfg(target_os = "macos")]
fn write(exe: &Path, desktop: &Path) -> Result<PathBuf, String> {
    // Inside "Arctic Launcher.app/Contents/MacOS/": link the .app itself.
    let app = exe
        .ancestors()
        .find(|a| a.extension().is_some_and(|e| e == "app"))
        .ok_or(t!("nav.the_launcher_isnt_running_from_an_app_bundle"))?;
    let link = desktop.join(app.file_name().ok_or("no app name")?);
    if link.symlink_metadata().is_ok() {
        std::fs::remove_file(&link).map_err(|e| e.to_string())?;
    }
    std::os::unix::fs::symlink(app, &link).map_err(|e| e.to_string())?;
    Ok(link)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn write(exe: &Path, desktop: &Path) -> Result<PathBuf, String> {
    let file = desktop.join(LINUX_LAUNCHER_SHORTCUT);
    let quoted = desktop_exec_target(exe);
    let text = format!(
        "[Desktop Entry]\nType=Application\nName=Arctic Launcher\nComment=A fast, lightweight Minecraft launcher\n\
         Exec={quoted}\nTerminal=false\nCategories=Game;\n"
    );
    write_executable(&file, &text).map_err(|e| e.to_string())?;
    Ok(file)
}

/// Find the existing shortcut by its launch arguments or reserve an unused title.
fn instance_file(
    desktop: &Path,
    name: &str,
    extension: &str,
    owns: impl Fn(&Path) -> bool,
    write: impl FnOnce(&Path) -> io::Result<()>,
) -> io::Result<PathBuf> {
    for entry in fs::read_dir(desktop)? {
        let entry = entry?;
        let path = entry.path();
        if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case(extension))
            && entry.file_type()?.is_file()
            && owns(&path)
        {
            write(&path)?;
            return Ok(path);
        }
    }
    let name = shortcut_title(name);
    let titles = std::iter::once(name.clone()).chain((2..).map(|n| format!("{name} ({n})")));
    for stem in titles {
        let filename = format!("{stem}.{extension}");
        if [WINDOWS_LAUNCHER_SHORTCUT, LINUX_LAUNCHER_SHORTCUT]
            .iter()
            .any(|launcher| filename.eq_ignore_ascii_case(launcher))
        {
            continue;
        }
        let path = desktop.join(filename);
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => drop(file),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
        if let Err(e) = write(&path) {
            let _ = fs::remove_file(&path);
            return Err(e);
        }
        return Ok(path);
    }
    unreachable!()
}

fn shortcut_title(name: &str) -> String {
    let mut title = String::new();
    for c in name.trim().chars() {
        if title.len() + c.len_utf8() > 200 {
            break;
        }
        title.push(if c.is_control() || "<>:\"/\\|?*".contains(c) {
            '_'
        } else {
            c
        });
    }
    let title = title.trim_matches([' ', '.']);
    if title.is_empty() {
        return "Minecraft".to_owned();
    }
    // Windows reserves these device names even with an extension.
    let stem = title
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end()
        .to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"))
            .is_some_and(|n| {
                matches!(
                    n,
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            });
    if reserved {
        format!("_{title}")
    } else {
        title.to_owned()
    }
}

#[cfg(windows)]
fn write_instance(exe: &Path, desktop: &Path, name: &str, id: &str) -> Result<PathBuf, String> {
    let args = windows_instance_args(id);
    instance_file(
        desktop,
        name,
        "lnk",
        |path| crate::start_menu::is_instance_link(path, &args),
        |path| {
            crate::start_menu::write_instance_link(exe, path, &args, &shortcut_title(name))
                .map_err(io::Error::other)
        },
    )
    .map_err(|e| e.to_string())
}

#[cfg(any(windows, test))]
fn windows_instance_args(id: &str) -> String {
    // CommandLineToArgvW doubles backslashes before a quote or the end of a quoted argument.
    let mut args = String::from("--instance \"");
    let mut slashes = 0;
    for c in id.chars() {
        if c == '\\' {
            slashes += 1;
            continue;
        }
        let escaped = if c == '"' { slashes * 2 + 1 } else { slashes };
        args.extend(std::iter::repeat_n('\\', escaped));
        args.push(c);
        slashes = 0;
    }
    args.extend(std::iter::repeat_n('\\', slashes * 2));
    args.push_str("\" --no-intro");
    args
}

#[cfg(all(unix, not(target_os = "macos")))]
fn write_instance(exe: &Path, desktop: &Path, name: &str, id: &str) -> Result<PathBuf, String> {
    let path = write_desktop_entry(exe, desktop, name, id).map_err(|e| e.to_string())?;
    // GNOME and other GIO-based desktops may require this in addition to the executable bit.
    let _ = std::process::Command::new("gio")
        .arg("set")
        .arg(&path)
        .args(["metadata::trusted", "true"])
        .output();
    Ok(path)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn write_desktop_entry(exe: &Path, desktop: &Path, name: &str, id: &str) -> io::Result<PathBuf> {
    let name_value = name
        .trim()
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t");
    let exe = desktop_exec_target(exe);
    let arg = desktop_exec_arg(id);
    let marker = instance_marker("X-Arctic-Instance=", id); // i18n-ignore
    let text = format!(
        "[Desktop Entry]\nType=Application\nName={name_value}\nExec={exe} --instance {arg} --no-intro\n\
         Terminal=false\nCategories=Game;\n{marker}\n"
    );
    instance_file(
        desktop,
        name,
        "desktop",
        |path| has_marker(path, &marker),
        |path| write_executable(path, &text),
    )
}

#[cfg(all(unix, not(target_os = "macos")))]
fn desktop_exec_target(exe: &Path) -> String {
    let exe = exe.to_string_lossy();
    let quoted = desktop_exec_arg(&exe);
    if exe.contains(['%', '=']) {
        // GIO checks the first token before expanding %% and the spec excludes '=' there.
        // Pass the path as argv so the shell receives it without interpolating it.
        let script = desktop_exec_arg(r#"exec "$@""#);
        format!("/bin/sh -c {script} arctic-launcher {quoted}")
    } else {
        quoted
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn desktop_exec_arg(arg: &str) -> String {
    let quoted = arg
        .replace('\\', "\\\\\\\\")
        .replace('"', "\\\\\"")
        .replace('$', "\\\\$")
        .replace('`', "\\\\`")
        .replace('%', "%%")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t");
    format!("\"{quoted}\"")
}

#[cfg(target_os = "macos")]
fn write_instance(exe: &Path, desktop: &Path, name: &str, id: &str) -> Result<PathBuf, String> {
    write_command(exe, desktop, name, id).map_err(|e| e.to_string())
}

#[cfg(any(target_os = "macos", all(unix, test)))]
fn write_command(exe: &Path, desktop: &Path, name: &str, id: &str) -> io::Result<PathBuf> {
    let marker = instance_marker("# Arctic instance: ", id); // i18n-ignore
    let shell_arg = |arg: &str| format!("'{}'", arg.replace('\'', "'\\''"));
    let exe = shell_arg(&exe.to_string_lossy());
    let arg = shell_arg(id);
    let script = format!(
        "#!/bin/sh\n{marker}\n/usr/bin/nohup {exe} --instance {arg} --no-intro </dev/null >/dev/null 2>&1 &\n"
    );
    instance_file(
        desktop,
        name,
        "command",
        |path| has_marker(path, &marker),
        |path| write_executable(path, &script),
    )
}

#[cfg(unix)]
fn instance_marker(prefix: &str, id: &str) -> String {
    format!("{prefix}{}", serde_json::json!(id))
}

#[cfg(unix)]
fn has_marker(path: &Path, marker: &str) -> bool {
    fs::read_to_string(path).is_ok_and(|text| text.lines().any(|line| line == marker))
}

#[cfg(unix)]
fn write_executable(path: &Path, text: &str) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::write(path, text)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_shortcut_titles() {
        assert_eq!(
            shortcut_title(r#"  ...My:Pack/\<>|?*".  "#),
            "My_Pack________"
        );
        assert_eq!(shortcut_title("..."), "Minecraft");
        assert_eq!(shortcut_title("COM¹.example"), "_COM¹.example");
        assert_eq!(shortcut_title("雪".repeat(100).as_str()).len(), 198);
    }

    #[test]
    fn colliding_titles_get_distinct_paths_and_refresh_by_id() {
        let desktop = tempfile::tempdir().unwrap();
        let create = |name, id| {
            instance_file(
                desktop.path(),
                name,
                "test",
                |path| fs::read_to_string(path).is_ok_and(|text| text == id),
                |path| fs::write(path, id),
            )
            .unwrap()
        };
        let existing = desktop.path().join("Pack_.test");
        fs::write(&existing, "user file").unwrap();
        let first = create("Pack?", "first");
        let second = create("Pack*", "second");
        assert_eq!(first, desktop.path().join("Pack_ (2).test"));
        assert_eq!(second, desktop.path().join("Pack_ (3).test"));
        assert_eq!(create("Renamed", "second"), second);
        assert_eq!(fs::read_to_string(existing).unwrap(), "user file");
        assert_eq!(fs::read_to_string(first).unwrap(), "first");
        assert_eq!(fs::read_to_string(second).unwrap(), "second");
    }

    #[test]
    fn reserves_the_launcher_shortcut_names() {
        let desktop = tempfile::tempdir().unwrap();
        for (name, extension) in [("Arctic Launcher", "lnk"), ("arctic-launcher", "desktop")] {
            let shortcut =
                instance_file(desktop.path(), name, extension, |_| false, |_| Ok(())).unwrap();
            assert_eq!(
                shortcut,
                desktop.path().join(format!("{name} (2).{extension}"))
            );
        }
    }

    #[test]
    fn windows_args_quote_the_instance_id() {
        assert_eq!(
            windows_instance_args("vanilla"),
            r#"--instance "vanilla" --no-intro"#
        );
        assert_eq!(
            windows_instance_args("my \"pack\\"),
            r#"--instance "my \"pack\\" --no-intro"#
        );
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn desktop_entry_writes_escaped_name_and_exec() {
        let desktop = tempfile::tempdir().unwrap();
        let exe = Path::new("/test/Arctic \\ \"$HOME").join("100% \x60launcher\x60");
        let path = write_desktop_entry(&exe, desktop.path(), "My Pack\\\nNext", "pack %f").unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.lines().any(|line| line == r"Name=My Pack\\\nNext"));
        let exec = concat!(
            r#"Exec=/bin/sh -c "exec \\"\\$@\\"" arctic-launcher "/test/Arctic \\\\ \\"\\"#,
            "$HOME/100%% \\\\\x60launcher\\\\\x60\" --instance \"pack %%f\" --no-intro"
        );
        assert!(text.lines().any(|line| line == exec), "{text}");
        assert_executable(&path);
    }

    #[cfg(unix)]
    #[test]
    fn command_launches_the_gui_with_a_shell_quoted_id() {
        let desktop = tempfile::tempdir().unwrap();
        let exe = Path::new("/Applications/Arctic's Launcher.app/Contents/MacOS/Arctic");
        let path = write_command(exe, desktop.path(), "My Pack", "pack's $id").unwrap();
        assert_eq!(path, desktop.path().join("My Pack.command"));
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("#!/bin/sh\n"));
        assert!(text.contains(concat!(
            r"/usr/bin/nohup '/Applications/Arctic'\''s Launcher.app/Contents/MacOS/Arctic'",
            r" --instance 'pack'\''s $id' --no-intro </dev/null >/dev/null 2>&1 &"
        )));
        assert_executable(&path);
    }

    #[cfg(unix)]
    fn assert_executable(path: &Path) {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o755
        );
    }
}
