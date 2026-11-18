//! The Windows runtime path that recorders use to name their clips.

use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use crate::java::{ensure_directory, file_metadata, is_link, require_directory};

pub(super) const FOLDER: &str = "Arctic Client";
const EXE: &str = "Arctic Client.exe";
const STAGING: &str = ".staging-";

/// Keep the existing copy until a complete replacement has been staged. The caller holds the
/// runtime lock, so the source won't be updated and another launch can't publish at the same time.
pub(super) fn prepare(runtime: &Path, java: &Path) -> io::Result<(PathBuf, bool)> {
    if java.parent() != Some(runtime.join("bin").as_path()) {
        return Err(io::Error::other(
            "Java executable is outside the runtime's bin directory",
        ));
    }
    let runtimes = runtime.parent().unwrap();
    let namespace = runtimes.join(".named");
    let parent = namespace.join(runtime.file_name().unwrap());
    require_directory(runtimes)?;
    require_directory(runtime)?;
    ensure_directory(&namespace)?;
    ensure_directory(&parent)?;

    // A killed launcher or a locked executable may have left an earlier staging directory.
    for entry in fs::read_dir(&parent)?.flatten() {
        if entry.file_name().to_string_lossy().starts_with(STAGING)
            && require_directory(&entry.path()).is_ok()
        {
            let _ = fs::remove_dir_all(entry.path());
        }
    }

    let named = parent.join(FOLDER);
    let branded = Path::new("bin").join(EXE);
    if mirror_matches(runtime, &named, java, &branded).unwrap_or(false) {
        return Ok((named.join(branded), false));
    }
    let staging = tempfile::Builder::new()
        .prefix(STAGING)
        .tempdir_in(&parent)?;
    let replacement = staging.path().join(FOLDER);
    copy_tree(runtime, &replacement)?;
    let source = replacement.join(java.strip_prefix(runtime).unwrap());
    fs::copy(source, replacement.join(&branded))?;
    if !mirror_matches(runtime, &replacement, java, &branded)? {
        return Err(io::Error::other(
            "Java runtime changed while making the named copy",
        ));
    }

    let previous = staging.path().join("previous");
    // Renaming a junction or symlink moves the link itself. Removal of the temporary tree
    // does not follow junctions or symlinks.
    let had_previous = match fs::rename(&named, &previous) {
        Ok(()) => true,
        Err(e) if e.kind() == io::ErrorKind::NotFound => false,
        Err(e) => return Err(e),
    };
    if let Err(e) = fs::rename(&replacement, &named) {
        if had_previous {
            let _ = fs::rename(&previous, &named);
        }
        return Err(e);
    }
    if let Err(e) = staging.close() {
        log::info!("cleaning up the previous named Java runtime: {e}");
    }
    Ok((named.join(branded), true))
}

fn mirror_matches(src: &Path, dst: &Path, java: &Path, branded: &Path) -> io::Result<bool> {
    // Check the tree before accessing the branded executable, including its bin directory.
    Ok(tree_matches(src, dst)? && same_file(java, &dst.join(branded))?)
}

fn tree_matches(src: &Path, dst: &Path) -> io::Result<bool> {
    require_directory(src)?;
    require_directory(dst)?;
    let mut remaining: HashSet<_> = fs::read_dir(dst)?
        .map(|entry| entry.map(|e| e.file_name()))
        .collect::<io::Result<_>>()?;
    if in_bin(dst) {
        remaining.remove(OsStr::new(EXE));
    }
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        if excluded(src, &name) {
            continue;
        }
        if !remaining.remove(&name) {
            return Ok(false);
        }
        let from = entry.path();
        let to = dst.join(name);
        let meta = fs::symlink_metadata(&from)?;
        let matches = !is_link(&meta)
            && if meta.is_dir() {
                tree_matches(&from, &to)?
            } else {
                meta.is_file() && same_file(&from, &to)?
            };
        if !matches {
            return Ok(false);
        }
    }
    Ok(remaining.is_empty())
}

fn copy_tree(src: &Path, dst: &Path) -> io::Result<()> {
    require_directory(src)?;
    fs::create_dir(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        if excluded(src, &name) {
            continue;
        }
        let from = entry.path();
        let to = dst.join(name);
        let meta = fs::symlink_metadata(&from)?;
        if is_link(&meta) {
            return Err(io::Error::other(format!(
                "Java runtime contains a link: {}",
                from.display()
            )));
        }
        if meta.is_dir() {
            copy_tree(&from, &to)?;
        } else if meta.is_file() {
            if fs::hard_link(&from, &to).is_err() {
                fs::copy(&from, &to)?;
            }
        } else {
            return Err(io::Error::other(format!(
                "Unsupported runtime entry: {}",
                from.display()
            )));
        }
    }
    Ok(())
}

fn excluded(dir: &Path, name: &OsStr) -> bool {
    // The named copy gets a fresh branded executable. Windows supplies its own C runtime.
    in_bin(dir) && (name.eq_ignore_ascii_case(EXE) || is_system_dll(name))
}

fn in_bin(dir: &Path) -> bool {
    dir.file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("bin"))
}

pub(super) fn is_system_dll(name: &OsStr) -> bool {
    let name = name.to_string_lossy().to_ascii_lowercase();
    name == "ucrtbase.dll" || (name.starts_with("api-ms-win-") && name.ends_with(".dll"))
}

fn same_file(a: &Path, b: &Path) -> io::Result<bool> {
    let (Some(a_meta), Some(b_meta)) = (file_metadata(a)?, file_metadata(b)?) else {
        return Ok(false);
    };
    if a_meta.len() != b_meta.len() {
        return Ok(false);
    }
    let (mut a, mut b) = (File::open(a)?, File::open(b)?);
    #[cfg(windows)]
    if file_id(&a).zip(file_id(&b)).is_some_and(|(a, b)| a == b) {
        return Ok(true);
    }
    // Compare ordinary copies, including on filesystems that don't report file IDs.
    let (mut left, mut right) = ([0; 64 * 1024], [0; 64 * 1024]);
    let mut remaining = a_meta.len();
    while remaining > 0 {
        let chunk = remaining.min(left.len() as u64) as usize;
        a.read_exact(&mut left[..chunk])?;
        b.read_exact(&mut right[..chunk])?;
        if left[..chunk] != right[..chunk] {
            return Ok(false);
        }
        remaining -= chunk as u64;
    }
    Ok(true)
}

#[cfg(windows)]
fn file_id(file: &File) -> Option<(u64, [u8; 16])> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_ID_INFO, FileIdInfo, GetFileInformationByHandleEx,
    };

    let mut id = FILE_ID_INFO::default();
    // SAFETY: The file handle stays open and id has the documented size for FileIdInfo.
    let found = unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            FileIdInfo,
            (&raw mut id).cast(),
            std::mem::size_of::<FILE_ID_INFO>() as u32,
        )
    };
    (found != 0 && id.FileId.Identifier != [0; 16])
        .then_some((id.VolumeSerialNumber, id.FileId.Identifier))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runtime(root: &Path) -> (PathBuf, PathBuf, PathBuf) {
        let runtime = root.join("java-runtime-epsilon");
        let bin = runtime.join("bin");
        fs::create_dir_all(bin.join("server")).unwrap();
        let java = bin.join("javaw.exe");
        fs::write(&java, b"java").unwrap();
        fs::write(bin.join("server/jvm.dll"), b"old jvm").unwrap();
        fs::write(bin.join("management.dll"), b"old management").unwrap();
        fs::write(runtime.join("release"), "JAVA_VERSION=\"25.0.1\"\n").unwrap();
        let named = root.join(".named/java-runtime-epsilon").join(FOLDER);
        (runtime, java, named)
    }

    #[test]
    fn replaces_stale_tree_and_same_size_files_with_the_same_timestamp() {
        let temp = tempfile::tempdir().unwrap();
        let (runtime, java, named) = runtime(temp.path());
        assert_eq!(
            prepare(&runtime, &java).unwrap(),
            (named.join("bin").join(EXE), true)
        );
        assert!(!prepare(&runtime, &java).unwrap().1);

        let dll = runtime.join("bin/server/jvm.dll");
        let old = fs::metadata(&dll).unwrap().modified().unwrap();
        let new = runtime.join("jvm-replacement");
        fs::write(&new, b"new jvm").unwrap();
        File::options()
            .write(true)
            .open(&new)
            .unwrap()
            .set_modified(old)
            .unwrap();
        fs::rename(new, &dll).unwrap();
        assert_eq!(fs::metadata(&dll).unwrap().modified().unwrap(), old);
        assert_eq!(
            fs::read(named.join("bin/server/jvm.dll")).unwrap(),
            b"old jvm"
        );
        assert!(prepare(&runtime, &java).unwrap().1);
        assert_eq!(
            fs::read(named.join("bin/server/jvm.dll")).unwrap(),
            b"new jvm"
        );

        fs::remove_file(runtime.join("bin/management.dll")).unwrap();
        fs::create_dir(named.join("stale")).unwrap();
        fs::write(named.join("stale/old.dll"), b"old").unwrap();
        assert!(prepare(&runtime, &java).unwrap().1);
        assert!(!named.join("bin/management.dll").exists());
        assert!(!named.join("stale").exists());
        assert!(!prepare(&runtime, &java).unwrap().1);

        let new = runtime.join("release.new");
        fs::write(&new, "JAVA_VERSION=\"25.0.2\"\n").unwrap();
        fs::rename(new, runtime.join("release")).unwrap();
        assert!(prepare(&runtime, &java).unwrap().1);
        assert_eq!(
            fs::read(named.join("release")).unwrap(),
            b"JAVA_VERSION=\"25.0.2\"\n"
        );
    }

    #[test]
    fn repairs_the_branded_executable_and_leaves_bundled_system_names_in_source() {
        let temp = tempfile::tempdir().unwrap();
        let (runtime, java, named) = runtime(temp.path());
        let bin = runtime.join("bin");
        fs::write(bin.join("UCRTBASE.dll"), b"source").unwrap();
        fs::write(bin.join("api-ms-win-core-file-l1-1-0.dll"), b"source").unwrap();
        fs::write(bin.join(EXE), b"old branded").unwrap();
        let (exe, _) = prepare(&runtime, &java).unwrap();
        fs::write(&exe, b"copy").unwrap();
        assert!(prepare(&runtime, &java).unwrap().1);
        assert_eq!(fs::read(&exe).unwrap(), b"java");
        fs::hard_link(bin.join("UCRTBASE.dll"), named.join("bin/UCRTBASE.dll")).unwrap();
        fs::write(named.join("bin/API-MS-WIN-core-x.DLL"), b"old").unwrap();
        assert!(prepare(&runtime, &java).unwrap().1);
        assert_eq!(fs::read(bin.join("UCRTBASE.dll")).unwrap(), b"source");
        assert_eq!(fs::read(bin.join(EXE)).unwrap(), b"old branded");
        assert!(!named.join("bin/UCRTBASE.dll").exists());
        assert!(!named.join("bin/API-MS-WIN-core-x.DLL").exists());
        assert!(!named.join("bin/api-ms-win-core-file-l1-1-0.dll").exists());
    }

    #[cfg(windows)]
    use junction::create as directory_link;
    #[cfg(unix)]
    use std::os::unix::fs::symlink as directory_link;

    #[test]
    fn failed_staging_preserves_the_previous_tree() {
        let temp = tempfile::tempdir().unwrap();
        let (runtime, java, named) = runtime(temp.path());
        let (exe, _) = prepare(&runtime, &java).unwrap();
        let outside = temp.path().join("outside");
        fs::create_dir(&outside).unwrap();
        directory_link(&outside, runtime.join("link")).unwrap();
        fs::write(runtime.join("bin/new.dll"), b"new").unwrap();
        assert!(prepare(&runtime, &java).is_err());
        assert!(!named.join("bin/new.dll").exists());
        assert_eq!(fs::read(&exe).unwrap(), b"java");
        assert_eq!(fs::read_dir(named.parent().unwrap()).unwrap().count(), 1);
    }

    #[test]
    fn never_uses_or_cleans_through_a_linked_named_directory() {
        let temp = tempfile::tempdir().unwrap();
        let (runtime, java, named) = runtime(temp.path());
        let outside = temp.path().join("outside");
        fs::create_dir_all(outside.join("bin")).unwrap();
        fs::write(outside.join("bin/UCRTBASE.dll"), b"untouched").unwrap();
        fs::create_dir_all(named.parent().unwrap()).unwrap();
        directory_link(&outside, &named).unwrap();
        assert!(prepare(&runtime, &java).unwrap().1);
        assert_eq!(
            fs::read(outside.join("bin/UCRTBASE.dll")).unwrap(),
            b"untouched"
        );

        fs::rename(temp.path().join(".named"), temp.path().join("old-named")).unwrap();
        directory_link(&outside, temp.path().join(".named")).unwrap();
        assert!(prepare(&runtime, &java).is_err());
        assert_eq!(fs::read_dir(outside).unwrap().count(), 1);
    }
}
