//! Forge/NeoForge installer jars: download (cached) and read files from them.

use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use zip::ZipArchive;

use super::forge_meta::InstallerSource;
use crate::error::IoContext;
use crate::net::{DownloadJob, agent, download_all};
use crate::storage::DataDirs;
use crate::{Error, Progress, Result};

/// Checksum files are tiny; anything bigger is not a checksum.
const MAX_SHA1_BYTES: u64 = 128;

/// An opened installer jar.
pub struct Installer {
    pub path: PathBuf,
    zip: ZipArchive<File>,
}

impl Installer {
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::open(path).at(path)?;
        let zip = ZipArchive::new(file).map_err(|e| {
            Error::Other(format!(
                "installer {} is not a valid jar: {e}",
                path.display()
            ))
        })?;
        Ok(Self {
            path: path.to_path_buf(),
            zip,
        })
    }

    /// Parse a JSON file inside the jar.
    pub fn read_json<T: DeserializeOwned>(&mut self, name: &str) -> Result<T> {
        let bytes = self.read(name)?;
        serde_json::from_slice(&bytes)
            .map_err(|e| Error::Other(format!("installer file {name} could not be read: {e}")))
    }

    fn read(&mut self, name: &str) -> Result<Vec<u8>> {
        let mut entry = self
            .zip
            .by_name(name.trim_start_matches('/'))
            .map_err(|e| Error::Other(format!("installer has no {name}: {e}")))?;
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).at(&self.path)?;
        Ok(bytes)
    }

    /// Copy `name` out of the jar to `dest` (atomically). Returns `false`
    /// when the jar has no such file.
    pub fn extract(&mut self, name: &str, dest: &Path) -> Result<bool> {
        let Ok(mut entry) = self.zip.by_name(name.trim_start_matches('/')) else {
            return Ok(false);
        };
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).at(parent)?;
        }
        let tmp = dest.with_extension("extract.part");
        let mut out = File::create(&tmp).at(&tmp)?;
        io::copy(&mut entry, &mut out).at(&tmp)?;
        drop(out);
        fs::rename(&tmp, dest).at(dest)?;
        Ok(true)
    }
}

/// Where installers are cached.
pub fn cache_dir(dirs: &DataDirs) -> PathBuf {
    dirs.cache().join("installers")
}

/// Download `source` into the installer cache (reused when it opens as a
/// valid jar) and open it.
pub fn fetch(dirs: &DataDirs, source: &InstallerSource, progress: Progress) -> Result<Installer> {
    let dest = cache_dir(dirs).join(&source.file_name);
    if dest.is_file() {
        match Installer::open(&dest) {
            Ok(installer) => return Ok(installer),
            Err(e) => {
                log::warn!("re-downloading broken installer: {e}");
                fs::remove_file(&dest).at(&dest)?;
            }
        }
    }
    let job = DownloadJob {
        url: source.url.clone(),
        sha1: remote_sha1(&source.url),
        dest: dest.clone(),
        size: None,
        lzma: None,
    };
    download_all("Downloading installer", vec![job], progress)?;
    Installer::open(&dest)
}

/// The `.sha1` Maven publishes next to a file, when available.
fn remote_sha1(url: &str) -> Option<String> {
    let mut resp = agent().get(&format!("{url}.sha1")).call().ok()?;
    let text = resp
        .body_mut()
        .with_config()
        .limit(MAX_SHA1_BYTES)
        .read_to_string()
        .ok()?;
    let sha = text.split_whitespace().next()?.to_ascii_lowercase();
    (sha.len() == 40 && sha.bytes().all(|b| b.is_ascii_hexdigit())).then_some(sha)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn reads_and_extracts_entries() {
        let dir = tempfile::tempdir().unwrap();
        let jar = dir.path().join("i.jar");
        let mut zip = zip::ZipWriter::new(File::create(&jar).unwrap());
        let opts = zip::write::SimpleFileOptions::default();
        zip.start_file("install_profile.json", opts).unwrap();
        zip.write_all(br#"{"spec":1}"#).unwrap();
        zip.start_file("data/client.lzma", opts).unwrap();
        zip.write_all(b"patch").unwrap();
        zip.finish().unwrap();

        let mut inst = Installer::open(&jar).unwrap();
        let v: serde_json::Value = inst.read_json("install_profile.json").unwrap();
        assert_eq!(v["spec"], 1);
        let out = dir.path().join("x/client.lzma");
        assert!(inst.extract("/data/client.lzma", &out).unwrap());
        assert_eq!(fs::read(&out).unwrap(), b"patch");
        assert!(!inst.extract("/missing", &out).unwrap());
        assert!(inst.read_json::<serde_json::Value>("nope.json").is_err());
    }
}
