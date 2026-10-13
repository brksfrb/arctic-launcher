//! Capes you made yourself, kept so you can switch between them: each image is
//! saved once in the profile's `capes` folder, named by the hash the cosmetics
//! server knows it by (`texture_hash`).

use std::path::{Path, PathBuf};

use crate::{Error, Result};

const DIR: &str = "capes";

/// The folder of a profile's own capes.
pub fn dir(profile_root: &Path) -> PathBuf {
    profile_root.join(DIR)
}

/// Keep `png`; returns its hash. Saving the same image again changes nothing.
pub fn save(dir: &Path, png: &[u8]) -> Result<String> {
    let hash = crate::cosmetics::texture_hash(png);
    let path = dir.join(format!("{hash}.png"));
    if !path.is_file() {
        std::fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
        std::fs::write(&path, png).map_err(|e| Error::io(&path, e))?;
    }
    Ok(hash)
}

/// The capes kept, newest first.
pub fn list(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<(std::time::SystemTime, String)> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let hash = name.strip_suffix(".png")?;
            let is_hash = hash.len() == 40 && hash.bytes().all(|b| b.is_ascii_hexdigit());
            let modified = e.metadata().and_then(|m| m.modified()).ok()?;
            is_hash.then(|| (modified, hash.to_owned()))
        })
        .collect();
    found.sort_by_key(|a| std::cmp::Reverse(a.0));
    found.into_iter().map(|(_, hash)| hash).collect()
}

/// One kept cape's image.
pub fn read(dir: &Path, hash: &str) -> Result<Vec<u8>> {
    let path = dir.join(format!("{hash}.png"));
    std::fs::read(&path).map_err(|e| Error::io(&path, e))
}

/// Forget one (what's worn stays worn).
pub fn remove(dir: &Path, hash: &str) -> Result<()> {
    let path = dir.join(format!("{hash}.png"));
    match std::fs::remove_file(&path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(Error::io(&path, e)),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capes_are_kept_once_newest_first_and_removed() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = dir(tmp.path());
        assert!(list(&dir).is_empty());
        let first = save(&dir, b"first cape").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        let second = save(&dir, b"second cape").unwrap();
        assert_eq!(save(&dir, b"first cape").unwrap(), first);
        assert_eq!(list(&dir), vec![second.clone(), first.clone()]);
        assert_eq!(read(&dir, &first).unwrap(), b"first cape");
        // Other files in the folder are no capes.
        std::fs::write(dir.join("notes.txt"), "x").unwrap();
        remove(&dir, &second).unwrap();
        remove(&dir, &second).unwrap();
        assert_eq!(list(&dir), vec![first]);
    }
}
