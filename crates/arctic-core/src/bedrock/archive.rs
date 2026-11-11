//! Zipping and unzipping for Bedrock's archives: `.mcworld` (a world's
//! folder contents at the root) and `.mcpack` / `.mcaddon` (packs).

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::error::IoContext;
use crate::{Error, Result};

/// Refuse to unpack archives larger than this (a zip bomb guard).
const MAX_UNPACKED_BYTES: u64 = 16 * 1024 * 1024 * 1024;

pub(crate) type Zip = zip::ZipArchive<fs::File>;

pub(crate) fn open(path: &Path) -> Result<Zip> {
    let file = fs::File::open(path).at(path)?;
    zip::ZipArchive::new(file).map_err(|e| Error::Other(format!("not a zip file: {e}")))
}

/// Folders inside the archive (`""` for the root) that hold `file`, at most
/// `max_depth` folders down, shallowest first.
pub(crate) fn dirs_with(zip: &Zip, file: &str, max_depth: usize) -> Vec<String> {
    let mut found: Vec<String> = zip
        .file_names()
        .filter_map(|name| {
            let dir = name.strip_suffix(file)?;
            (dir.is_empty() || dir.ends_with('/')).then(|| dir.to_owned())
        })
        .filter(|dir| dir.matches('/').count() <= max_depth)
        .collect();
    found.sort_by_key(|d| (d.matches('/').count(), d.clone()));
    found.dedup();
    found
}

/// Read one small file from the archive.
pub(crate) fn read_file(zip: &mut Zip, name: &str) -> Option<Vec<u8>> {
    let mut entry = zip.by_name(name).ok()?;
    if entry.size() > 4 * 1024 * 1024 {
        return None;
    }
    let mut out = Vec::new();
    entry.read_to_end(&mut out).ok()?;
    Some(out)
}

/// Unpack what's under `prefix` in the archive into `dest` (made fresh),
/// skipping `skip` (names of files to leave out anywhere).
pub(crate) fn unpack(zip: &mut Zip, prefix: &str, dest: &Path, skip: &[&str]) -> Result<()> {
    let _ = fs::remove_dir_all(dest);
    fs::create_dir_all(dest).at(dest)?;
    let mut total = 0u64;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| Error::Other(e.to_string()))?;
        // `enclosed_name` rejects absolute paths and `..` (zip slip).
        let Some(inner) = entry.enclosed_name() else {
            continue;
        };
        let Ok(relative) = inner.strip_prefix(prefix) else {
            continue;
        };
        if relative.as_os_str().is_empty()
            || relative
                .file_name()
                .is_some_and(|n| skip.iter().any(|s| n == *s))
        {
            continue;
        }
        let out = dest.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&out).at(&out)?;
            continue;
        }
        total += entry.size();
        if total > MAX_UNPACKED_BYTES {
            let _ = fs::remove_dir_all(dest);
            return Err(Error::Other("the archive is too large".into()));
        }
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).at(parent)?;
        }
        let mut file = fs::File::create(&out).at(&out)?;
        std::io::copy(&mut entry, &mut file).at(&out)?;
    }
    Ok(())
}

/// Zip the contents of `dir` (not the folder itself) into `out`.
pub(crate) fn zip_dir(dir: &Path, out: &Path) -> Result<()> {
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent).at(parent)?;
    }
    let partial = out.with_extension("arctic-partial");
    let file = fs::File::create(&partial).at(&partial)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let result = add_dir(&mut zip, dir, "", options).and_then(|()| {
        zip.finish()
            .map(|_| ())
            .map_err(|e| Error::Other(e.to_string()))
    });
    if let Err(e) = result {
        let _ = fs::remove_file(&partial);
        return Err(e);
    }
    fs::rename(&partial, out).at(out)
}

fn add_dir(
    zip: &mut zip::ZipWriter<fs::File>,
    dir: &Path,
    prefix: &str,
    options: zip::write::SimpleFileOptions,
) -> Result<()> {
    for entry in fs::read_dir(dir).at(dir)?.flatten() {
        let path = entry.path();
        let name = format!("{prefix}{}", entry.file_name().to_string_lossy());
        if path.is_dir() {
            add_dir(zip, &path, &format!("{name}/"), options)?;
        } else {
            zip.start_file(name, options)
                .map_err(|e| Error::Other(e.to_string()))?;
            let mut buf = Vec::new();
            fs::File::open(&path)
                .at(&path)?
                .read_to_end(&mut buf)
                .at(&path)?;
            zip.write_all(&buf).at(&path)?;
        }
    }
    Ok(())
}

/// Copy a folder (and everything in it).
pub(crate) fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to).at(to)?;
    for entry in fs::read_dir(from).at(from)?.flatten() {
        let src = entry.path();
        let dst = to.join(entry.file_name());
        match entry.file_type() {
            Ok(t) if t.is_dir() => copy_dir(&src, &dst)?,
            Ok(t) if t.is_file() => {
                fs::copy(&src, &dst).at(&src)?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// Total size of the files in a folder.
pub(crate) fn dir_size(path: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => dir_size(&e.path()),
            Ok(_) => e.metadata().map_or(0, |m| m.len()),
            Err(_) => 0,
        })
        .sum()
}

/// A file or folder name made safe for Windows.
pub(crate) fn safe_name(wanted: &str, fallback: &str) -> String {
    let clean: String = wanted
        .chars()
        .map(|c| {
            if "<>:\"/\\|?*".contains(c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let clean = clean.trim().trim_matches('.').to_owned();
    if clean.is_empty() {
        fallback.to_owned()
    } else {
        clean.chars().take(80).collect()
    }
}

/// `dir/name`, or `dir/name (2)`, … whichever doesn't exist yet.
pub(crate) fn free_path(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    (2..)
        .map(|n| dir.join(format!("{name} ({n})")))
        .find(|p| !p.exists())
        .unwrap_or(first)
}

/// Move `path` into `trash_dir` (kept, not deleted), under a unique name.
pub(crate) fn trash(path: &Path, trash_dir: &Path) -> Result<()> {
    fs::create_dir_all(trash_dir).at(trash_dir)?;
    let name = path
        .file_name()
        .map_or("item".into(), |n| n.to_string_lossy().into_owned());
    let to = trash_dir.join(format!("{name}-{}", crate::auth::now_secs()));
    fs::rename(path, &to).at(path)
}
