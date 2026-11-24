//! The Windows runtime path that recorders use to name their clips.

use std::collections::{HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use crate::java::{
    ensure_directory, file_metadata, is_link, is_runtime_control_file, require_directory,
};

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
    remove_control_stamps(&named);
    match check_mirror(runtime, &named, java, &branded) {
        Ok(()) => return Ok((named.join(branded), false)),
        Err(e) => log::info!("rebuilding named Java runtime {}: {e}", named.display()),
    }
    let staging = tempfile::Builder::new()
        .prefix(STAGING)
        .tempdir_in(&parent)?;
    let replacement = staging.path().join(FOLDER);
    copy_tree(runtime, &replacement, true)?;
    let source = replacement.join(java.strip_prefix(runtime).unwrap());
    copy_with_mtime(&source, &replacement.join(&branded))?;
    check_mirror(runtime, &replacement, java, &branded)?;

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

/// Copies made by launchers before the runtime's own stamps were left out of the mirror still
/// hold them. They are no part of Java: taking them out gives the tree a rebuild would, without
/// the seconds a rebuild takes (it used to cost one whole rebuild per runtime after updating).
fn remove_control_stamps(named: &Path) {
    // Never through a link: the mirror is rebuilt then.
    if require_directory(named).is_err() {
        return;
    }
    let Ok(entries) = fs::read_dir(named) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if is_runtime_control_file(&entry.file_name())
            && matches!(file_metadata(&path), Ok(Some(_)))
            && fs::remove_file(&path).is_ok()
        {
            log::info!("removed the old runtime stamp {}", path.display());
        }
    }
}

fn check_mirror(src: &Path, dst: &Path, java: &Path, branded: &Path) -> io::Result<()> {
    // Validate the bin directory before accessing the branded executable.
    check_tree(src, dst, true)?;
    let exe = dst.join(branded);
    if let Some(source) = file_metadata(java)?
        && same_file(java, &source, &exe)?
    {
        return Ok(());
    }
    Err(mismatch(
        &exe,
        "branded executable missing or size, mtime or contents changed",
    ))
}

fn mismatch(path: &Path, reason: &str) -> io::Error {
    io::Error::other(format!("{reason}: {}", path.display()))
}

fn check_tree(src: &Path, dst: &Path, root: bool) -> io::Result<()> {
    require_directory(src)?;
    require_directory(dst).map_err(|e| mismatch(dst, &e.to_string()))?;
    let mut remaining: HashSet<_> = fs::read_dir(dst)?
        .map(|entry| entry.map(|e| e.file_name()))
        .collect::<io::Result<_>>()?;
    if in_bin(dst) {
        remaining.remove(OsStr::new(EXE));
    }
    // Nearly every file of the mirror is a hard link to the runtime's: the two folders' own
    // listings prove that without opening the files.
    let linked = hard_links(src, dst);
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        if excluded(src, &name, root) {
            continue;
        }
        let to = dst.join(&name);
        if !remaining.remove(&name) {
            return Err(mismatch(&to, "missing named entry"));
        }
        let from = entry.path();
        if linked.contains(&name) {
            continue;
        }
        let meta = fs::symlink_metadata(&from)?;
        if is_link(&meta) {
            return Err(mismatch(&from, "source entry is a link"));
        }
        if meta.is_dir() {
            check_tree(&from, &to, false)?;
        } else if !meta.is_file() || !same_file(&from, &meta, &to)? {
            return Err(mismatch(&to, "file size, mtime or contents changed"));
        }
    }
    if let Some(extra) = remaining.into_iter().next() {
        return Err(mismatch(&dst.join(extra), "unexpected named entry"));
    }
    Ok(())
}

/// One regular file of a folder as the folder's listing gives it: which file it is.
#[derive(PartialEq, Eq)]
struct Listed {
    volume: u64,
    file: [u8; 16],
}

/// Names that are the same regular file (one file with two names) in both folders. A name
/// left out is not known to differ: it is compared the long way. Empty where the listing can't
/// tell (other systems than Windows, file systems without file IDs).
fn hard_links(src: &Path, dst: &Path) -> HashSet<OsString> {
    let (Some(src), Some(dst)) = (list_files(src), list_files(dst)) else {
        return HashSet::new();
    };
    src.into_iter()
        .filter(|(name, file)| dst.get(name) == Some(file))
        .map(|(name, _)| name)
        .collect()
}

/// The regular files of `dir` (no folders, no links of any kind) with their file IDs, read
/// from the folder in one go: no file is opened, which with a few hundred files, each looked
/// at by the antivirus when opened, was most of the seconds before Java started.
#[cfg(windows)]
fn list_files(dir: &Path) -> Option<HashMap<OsString, Listed>> {
    use std::os::windows::ffi::OsStringExt;
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS,
        FILE_FLAG_OPEN_REPARSE_POINT, FILE_ID_EXTD_DIR_INFO, FileIdExtdDirectoryInfo,
        GetFileInformationByHandleEx,
    };

    // The folder itself, never what a link in its place points to.
    let handle = File::options()
        .read(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(dir)
        .ok()?;
    let (volume, _) = file_id(&handle)?;
    let mut files = HashMap::new();
    // u64s: the records need 8-byte alignment.
    let mut buffer = vec![0u64; 8 * 1024];
    let bytes = buffer.len() * std::mem::size_of::<u64>();
    let name_at = std::mem::offset_of!(FILE_ID_EXTD_DIR_INFO, FileName);
    loop {
        // SAFETY: the handle stays open, and the buffer is writable for `bytes` bytes.
        let filled = unsafe {
            GetFileInformationByHandleEx(
                handle.as_raw_handle(),
                FileIdExtdDirectoryInfo,
                buffer.as_mut_ptr().cast(),
                bytes as u32,
            )
        };
        if filled == 0 {
            // The end of the folder; anything else means the listing can't be relied on.
            const ERROR_NO_MORE_FILES: i32 = 18;
            let done = io::Error::last_os_error().raw_os_error() == Some(ERROR_NO_MORE_FILES);
            return done.then_some(files);
        }
        let base = buffer.as_ptr().cast::<u8>();
        let mut at = 0usize;
        loop {
            // A record's fixed part, then its name: both inside the buffer before they are read.
            if at.checked_add(name_at)? > bytes {
                return None;
            }
            // SAFETY: `at + offset + 4 <= at + name_at <= bytes` for these fields (checked above).
            let number = |offset: usize| unsafe {
                std::ptr::read_unaligned(base.add(at + offset).cast::<u32>())
            };
            let next = number(std::mem::offset_of!(FILE_ID_EXTD_DIR_INFO, NextEntryOffset));
            let attributes = number(std::mem::offset_of!(FILE_ID_EXTD_DIR_INFO, FileAttributes));
            let name_bytes =
                number(std::mem::offset_of!(FILE_ID_EXTD_DIR_INFO, FileNameLength)) as usize;
            // SAFETY: the 16 bytes of the id end before the name starts, inside the buffer.
            let file = unsafe {
                std::ptr::read_unaligned(
                    base.add(at + std::mem::offset_of!(FILE_ID_EXTD_DIR_INFO, FileId))
                        .cast::<[u8; 16]>(),
                )
            };
            if (at + name_at).checked_add(name_bytes)? > bytes {
                return None;
            }
            let name: Vec<u16> = (0..name_bytes / 2)
                // SAFETY: inside the buffer (checked above); read unaligned, two bytes at a time.
                .map(|i| unsafe {
                    std::ptr::read_unaligned(base.add(at + name_at + 2 * i).cast::<u16>())
                })
                .collect();
            let plain = attributes & (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT) == 0;
            if plain && file != [0; 16] {
                files.insert(OsString::from_wide(&name), Listed { volume, file });
            }
            if next == 0 {
                break;
            }
            at = at.checked_add(next as usize)?;
        }
    }
}

#[cfg(not(windows))]
fn list_files(_dir: &Path) -> Option<HashMap<OsString, Listed>> {
    None
}

fn copy_tree(src: &Path, dst: &Path, root: bool) -> io::Result<()> {
    require_directory(src)?;
    fs::create_dir(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        if excluded(src, &name, root) {
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
            copy_tree(&from, &to, false)?;
        } else if meta.is_file() {
            if fs::hard_link(&from, &to).is_err() {
                copy_with_mtime(&from, &to)?;
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

fn copy_with_mtime(from: &Path, to: &Path) -> io::Result<()> {
    let modified = fs::metadata(from)?.modified()?;
    fs::copy(from, to)?;
    if fs::metadata(to)?.modified()? != modified {
        File::options()
            .write(true)
            .open(to)?
            .set_modified(modified)?;
    }
    Ok(())
}

fn excluded(dir: &Path, name: &OsStr, root: bool) -> bool {
    // The named copy gets a fresh branded executable. Windows supplies its own C runtime.
    (root && is_runtime_control_file(name))
        || (in_bin(dir) && (name.eq_ignore_ascii_case(EXE) || is_system_dll(name)))
}

fn in_bin(dir: &Path) -> bool {
    dir.file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("bin"))
}

pub(super) fn is_system_dll(name: &OsStr) -> bool {
    let name = name.to_string_lossy().to_ascii_lowercase();
    name == "ucrtbase.dll" || (name.starts_with("api-ms-win-") && name.ends_with(".dll"))
}

fn matching_metadata(a: &fs::Metadata, b: &fs::Metadata) -> io::Result<bool> {
    Ok(a.len() == b.len() && a.modified()? == b.modified()?)
}

fn same_file(a: &Path, a_meta: &fs::Metadata, b: &Path) -> io::Result<bool> {
    let Some(b_meta) = file_metadata(b)? else {
        return Ok(false);
    };
    if !matching_metadata(a_meta, &b_meta)? {
        return Ok(false);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if a_meta.dev() == b_meta.dev() && a_meta.ino() == b_meta.ino() {
            return Ok(true);
        }
    }
    #[cfg(test)]
    OPENED.with(|opened| opened.set(opened.get() + 1));
    let (mut a, mut b) = (File::open(a)?, File::open(b)?);
    #[cfg(windows)]
    if file_id(&a).zip(file_id(&b)).is_some_and(|(a, b)| a == b) {
        return Ok(true);
    }
    // Ordinary copies and filesystems without IDs still need content comparison.
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
thread_local! {
    /// Pairs of files opened to compare them (this thread's).
    static OPENED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Checking an unchanged mirror opens no file but the branded executable (a copy, so
    /// compared by content): hard links are told by the folders' listings on Windows and by
    /// their inode elsewhere.
    #[test]
    fn an_unchanged_mirror_is_checked_without_opening_its_files() {
        let temp = tempfile::tempdir().unwrap();
        let (runtime, java, _) = runtime(temp.path());
        fs::create_dir_all(runtime.join("lib/security")).unwrap();
        for i in 0..40 {
            fs::write(runtime.join(format!("lib/file{i}.jar")), b"lib").unwrap();
        }
        fs::write(runtime.join("lib/security/cacerts"), b"certs").unwrap();
        assert!(prepare(&runtime, &java).unwrap().1);
        OPENED.with(|opened| opened.set(0));
        assert!(!prepare(&runtime, &java).unwrap().1);
        assert_eq!(OPENED.with(|opened| opened.get()), 1);
    }

    #[cfg(windows)]
    #[test]
    fn folder_listings_tell_hard_links_from_copies() {
        let temp = tempfile::tempdir().unwrap();
        let (a, b) = (temp.path().join("a"), temp.path().join("b"));
        fs::create_dir_all(a.join("folder")).unwrap();
        fs::create_dir_all(b.join("folder")).unwrap();
        fs::write(a.join("linked.dll"), b"same").unwrap();
        fs::hard_link(a.join("linked.dll"), b.join("linked.dll")).unwrap();
        fs::write(a.join("copied.dll"), b"same").unwrap();
        fs::copy(a.join("copied.dll"), b.join("copied.dll")).unwrap();
        fs::write(a.join("only-here.dll"), b"same").unwrap();
        // Enough names for more than one buffer of the listing.
        for i in 0..3000 {
            let name = format!("a-rather-long-file-name-to-fill-the-listing-buffer-{i}.bin");
            fs::write(a.join(&name), b"x").unwrap();
            fs::hard_link(a.join(&name), b.join(&name)).unwrap();
        }
        let listed = list_files(&a).unwrap();
        assert_eq!(listed.len(), 3003);
        assert!(!listed.contains_key(OsStr::new("folder")));
        let linked = hard_links(&a, &b);
        assert_eq!(linked.len(), 3001);
        assert!(linked.contains(OsStr::new("linked.dll")));
        assert!(!linked.contains(OsStr::new("copied.dll")));
        // A junction in place of the folder is not listed through.
        let outside = temp.path().join("outside");
        fs::create_dir(&outside).unwrap();
        fs::hard_link(a.join("linked.dll"), outside.join("linked.dll")).unwrap();
        junction::create(&outside, temp.path().join("link")).unwrap();
        assert!(list_files(&temp.path().join("link")).is_none_or(|files| files.is_empty()));
    }

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
    fn removes_legacy_control_stamps_from_the_mirror_and_reuses_it() {
        let temp = tempfile::tempdir().unwrap();
        let (runtime, java, named) = runtime(temp.path());
        let stamps = [
            ".arctic-runtime",
            ".arctic-runtime-repair",
            ".arctic-runtime-repair-ack",
        ];
        assert!(prepare(&runtime, &java).unwrap().1);
        for stamp in stamps {
            fs::write(runtime.join(stamp), "old").unwrap();
            fs::hard_link(runtime.join(stamp), named.join(stamp)).unwrap();
        }

        // The stamps go without rebuilding the mirror, and the runtime keeps its own.
        assert!(!prepare(&runtime, &java).unwrap().1);
        for stamp in stamps {
            assert!(!named.join(stamp).exists());
            assert_eq!(fs::read_to_string(runtime.join(stamp)).unwrap(), "old");
            fs::write(runtime.join(stamp), "updated").unwrap();
        }
        assert!(!prepare(&runtime, &java).unwrap().1);
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
