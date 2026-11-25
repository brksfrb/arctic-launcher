//! Forge/NeoForge installer jars: download (cached) and read files from them.

use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use zip::ZipArchive;

use super::forge_meta::InstallerSource;
use crate::error::IoContext;
use crate::net::{
    DownloadJob, Policy, download_single, fetch_text, forge_mirror_url, is_not_found,
};
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
    let policy = Policy::for_url(&source.url);
    download_verified(
        &source.url,
        &dest,
        meta_url(&source.url).as_deref(),
        &policy,
        forge_mirror_url(&source.url),
        progress,
    )?;
    Installer::open(&dest)
}

/// A checksum for the installer that did not come from the mirror.
#[derive(Debug, PartialEq, Eq)]
enum Checksum {
    Sha1(String),
    Md5(String),
}

/// Download `url` to `dest`. The Forge mirror is used only when a hash from the primary side
/// (the primary's `.sha1`, else Forge's meta.json at `meta`) can verify its bytes; without one
/// only the primary is tried.
fn download_verified(
    url: &str,
    dest: &Path,
    meta: Option<&str>,
    policy: &Policy,
    mirror: Option<String>,
    progress: Progress,
) -> Result<()> {
    // Fetched on its own, so a jar download that fails never skips the checksum.
    let checksum = resolve_checksum(url, meta, policy);
    let mut job = DownloadJob {
        url: url.to_string(),
        sha1: None,
        md5: None,
        dest: dest.to_path_buf(),
        size: None,
        lzma: None,
    };
    let mirror_blocked = checksum.is_none() && mirror.is_some();
    match checksum {
        Some(Checksum::Sha1(h)) => job.sha1 = Some(h),
        Some(Checksum::Md5(h)) => job.md5 = Some(h),
        None if !mirror_blocked => log::warn!("no checksum for {url}"),
        None => log::warn!("no checksum for {url}: the mirror will not be used"),
    }
    let mirror = mirror.filter(|_| !mirror_blocked);
    match download_single(&job, mirror, policy, progress) {
        Err(e)
            if mirror_blocked
                && !is_not_found(&e)
                && !matches!(
                    e,
                    Error::DownloadInterrupted(_) | Error::DownloadConnectionDropping
                ) =>
        {
            Err(Error::Other(format!(
                "no checksum is available to verify a mirror copy ({e})"
            )))
        }
        done => done,
    }
}

/// The checksum for `url`: its `.sha1` on the same server, else the MD5 in Forge's meta.json.
fn resolve_checksum(url: &str, meta: Option<&str>, policy: &Policy) -> Option<Checksum> {
    match remote_sha1(url, policy) {
        Ok(Some(sha)) => return Some(Checksum::Sha1(sha)),
        Ok(None) => log::info!("{url}.sha1 is missing or malformed"),
        Err(e) => log::warn!("{url}.sha1 unavailable: {e}"),
    }
    let meta = meta?;
    match meta_md5(meta, url, policy) {
        Ok(md5) => md5.map(Checksum::Md5),
        Err(e) => {
            log::warn!("{meta} unavailable: {e}");
            None
        }
    }
}

/// The `.sha1` Maven publishes next to a file; `None` when absent or not a SHA-1.
fn remote_sha1(url: &str, policy: &Policy) -> Result<Option<String>> {
    let Some(text) = fetch_text(&format!("{url}.sha1"), MAX_SHA1_BYTES, policy)? else {
        return Ok(None);
    };
    let sha = text
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    Ok((sha.len() == 40 && sha.bytes().all(|b| b.is_ascii_hexdigit())).then_some(sha))
}

// What files.minecraftforge.net publishes (checked 2026-11-16 against 1.20.1-47.3.0 and
// 1.8.9-11.15.1.2318-1.8.9):
// - `forge/<version>/meta.json` is `{"classifiers": {"installer": {"jar": "<md5>"}, ...}}`: one
//   32-hex-digit **MD5** per artifact (installer, universal, userdev, mdk, changelog, ...). No
//   SHA-1 or SHA-256 appears anywhere in it. The installer's value equals the md5 of the jar on
//   maven.minecraftforge.net (and its `.md5`), so it is a valid second source.
// - `forge/maven-metadata.json` and `promotions_slim.json` only list versions; no hashes.
// MD5 is weaker than the `.sha1`, so it is used only when the primary has no usable `.sha1`.
const MAX_META_BYTES: u64 = 64 * 1024;

/// Forge's meta.json for the installer at `url` (a file on maven.minecraftforge.net).
fn meta_url(url: &str) -> Option<String> {
    let rest = url.strip_prefix(FORGE_MAVEN_FORGE)?;
    let version = rest.split('/').next().filter(|v| !v.is_empty())?;
    Some(format!("{FORGE_FILES}{version}/meta.json"))
}

const FORGE_MAVEN_FORGE: &str = "https://maven.minecraftforge.net/net/minecraftforge/forge/";
const FORGE_FILES: &str = "https://files.minecraftforge.net/net/minecraftforge/forge/";

/// The installer jar's MD5 from a meta.json document; `Ok(None)` when it doesn't cover the file.
fn meta_md5(meta: &str, url: &str, policy: &Policy) -> Result<Option<String>> {
    let Some(text) = fetch_text(meta, MAX_META_BYTES, policy)? else {
        return Ok(None);
    };
    let class = if url.ends_with("-installer.jar") {
        "installer"
    } else {
        return Ok(None);
    };
    let doc: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| Error::Other(format!("{meta} could not be read: {e}")))?;
    let md5 = doc["classifiers"][class]["jar"]
        .as_str()
        .map(str::to_ascii_lowercase)
        .filter(|h| h.len() == 32 && h.bytes().all(|b| b.is_ascii_hexdigit()));
    Ok(md5)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use md5::Md5;
    use sha1::{Digest, Sha1};

    const JAR: &[u8] = b"installer-bytes";

    /// Local HTTP server routing by path: each path has a script of statuses (the last repeats;
    /// `200` serves the path's body). Unknown paths are 404. Returns the base URL and the
    /// request log.
    fn serve(routes: Vec<(&'static str, Vec<u16>, Vec<u8>)>) -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let log = Arc::new(Mutex::new(Vec::<String>::new()));
        let seen = log.clone();
        std::thread::spawn(move || {
            for mut stream in listener.incoming().flatten() {
                let mut buf = [0u8; 4096];
                let n = io::Read::read(&mut stream, &mut buf).unwrap_or(0);
                let head = String::from_utf8_lossy(&buf[..n]).to_string();
                let path = head.split_whitespace().nth(1).unwrap_or("").to_string();
                let hits = {
                    let mut seen = seen.lock().unwrap();
                    seen.push(path.clone());
                    seen.iter().filter(|p| **p == path).count()
                };
                let (status, body) = match routes.iter().find(|r| r.0 == path) {
                    Some((_, script, body)) => {
                        let status = script[(hits - 1).min(script.len() - 1)];
                        (status, if status == 200 { body.clone() } else { vec![] })
                    }
                    None => (404, vec![]),
                };
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(&body);
            }
        });
        (base, log)
    }

    fn sha1_hex(b: &[u8]) -> String {
        hex::encode(Sha1::digest(b))
    }

    fn md5_hex(b: &[u8]) -> String {
        hex::encode(Md5::digest(b))
    }

    fn policy() -> Policy {
        Policy {
            attempts: 3,
            delays: vec![Duration::from_millis(5)],
        }
    }

    fn meta_doc(md5: &str) -> Vec<u8> {
        format!(r#"{{"classifiers":{{"installer":{{"jar":"{md5}"}}}}}}"#).into_bytes()
    }

    /// Run `download_verified` for `/p/forge-1-installer.jar` on `primary`, with `mirror` as the
    /// mirror base and `primary`'s `/meta.json` as Forge's metadata.
    fn run(primary: &str, mirror: &str) -> (Result<()>, PathBuf) {
        let dest = tempfile::tempdir().unwrap().keep().join("i.jar");
        let meta = format!("{primary}/meta.json");
        let result = download_verified(
            &format!("{primary}/p/forge-1-installer.jar"),
            &dest,
            Some(&meta),
            &policy(),
            Some(format!("{mirror}/p/forge-1-installer.jar")),
            &|_| {},
        );
        (result, dest)
    }

    const INSTALLER: &str = "/p/forge-1-installer.jar";

    #[test]
    fn mirror_installer_accepted_with_matching_primary_sha1() {
        let (primary, _) = serve(vec![
            (INSTALLER, vec![503], vec![]),
            (
                "/p/forge-1-installer.jar.sha1",
                vec![200],
                sha1_hex(JAR).into_bytes(),
            ),
        ]);
        let (mirror, mirror_log) = serve(vec![(INSTALLER, vec![200], JAR.to_vec())]);
        let (res, dest) = run(&primary, &mirror);
        res.unwrap();
        assert_eq!(fs::read(dest).unwrap(), JAR);
        assert_eq!(mirror_log.lock().unwrap().as_slice(), [INSTALLER]);
    }

    #[test]
    fn mirror_installer_with_wrong_hash_is_rejected() {
        let (primary, _) = serve(vec![
            (INSTALLER, vec![503], vec![]),
            (
                "/p/forge-1-installer.jar.sha1",
                vec![200],
                sha1_hex(b"other").into_bytes(),
            ),
        ]);
        let (mirror, _) = serve(vec![(INSTALLER, vec![200], JAR.to_vec())]);
        let (res, dest) = run(&primary, &mirror);
        assert!(res.is_err());
        assert!(!dest.exists());
    }

    #[test]
    fn no_hash_anywhere_rejects_the_mirror_copy() {
        // The primary is down and publishes no .sha1 or meta.json entry.
        let (primary, _) = serve(vec![(INSTALLER, vec![503], vec![])]);
        let (mirror, mirror_log) = serve(vec![(INSTALLER, vec![200], JAR.to_vec())]);
        let (res, dest) = run(&primary, &mirror);
        assert!(
            res.unwrap_err()
                .to_string()
                .contains("verify a mirror copy")
        );
        assert!(mirror_log.lock().unwrap().is_empty());
        assert!(!dest.exists());
    }

    #[test]
    fn neoforge_without_a_checksum_reports_the_primary_http_error() {
        const NEOFORGE: &str =
            "/releases/net/neoforged/neoforge/21.1.251/neoforge-21.1.251-installer.jar";
        let (primary, log) = serve(vec![(NEOFORGE, vec![503], vec![])]);
        let dir = tempfile::tempdir().unwrap();
        let url = format!("{primary}{NEOFORGE}");
        let error = download_verified(
            &url,
            &dir.path().join("installer.jar"),
            None,
            &policy(),
            forge_mirror_url(&url),
            &|_| {},
        )
        .unwrap_err();
        assert!(matches!(error, Error::Http(reason) if reason.ends_with("http status: 503")));
        assert_eq!(
            *log.lock().unwrap(),
            [
                format!("{NEOFORGE}.sha1"),
                NEOFORGE.into(),
                NEOFORGE.into(),
                NEOFORGE.into()
            ]
        );
    }

    #[test]
    fn hash_served_only_by_the_mirror_is_not_trusted() {
        let (primary, _) = serve(vec![(INSTALLER, vec![503], vec![])]);
        let (mirror, mirror_log) = serve(vec![
            (INSTALLER, vec![200], JAR.to_vec()),
            (
                "/p/forge-1-installer.jar.sha1",
                vec![200],
                sha1_hex(JAR).into_bytes(),
            ),
            (
                "/p/forge-1-installer.jar.md5",
                vec![200],
                md5_hex(JAR).into_bytes(),
            ),
        ]);
        let (res, dest) = run(&primary, &mirror);
        assert!(res.is_err());
        assert!(!dest.exists());
        assert!(mirror_log.lock().unwrap().is_empty());
    }

    #[test]
    fn primary_sha1_that_succeeds_on_retry_is_used() {
        let (primary, log) = serve(vec![
            (INSTALLER, vec![503], vec![]),
            (
                "/p/forge-1-installer.jar.sha1",
                vec![502, 503, 200],
                sha1_hex(JAR).into_bytes(),
            ),
        ]);
        let (mirror, _) = serve(vec![(INSTALLER, vec![200], JAR.to_vec())]);
        let (res, dest) = run(&primary, &mirror);
        res.unwrap();
        assert_eq!(fs::read(dest).unwrap(), JAR);
        let sha_hits = log
            .lock()
            .unwrap()
            .iter()
            .filter(|p| p.ends_with(".sha1"))
            .count();
        assert_eq!(sha_hits, 3);
    }

    #[test]
    fn meta_json_md5_verifies_the_mirror_copy() {
        let (primary, _) = serve(vec![
            (INSTALLER, vec![503], vec![]),
            ("/meta.json", vec![200], meta_doc(&md5_hex(JAR))),
        ]);
        let (mirror, _) = serve(vec![(INSTALLER, vec![200], JAR.to_vec())]);
        let (res, dest) = run(&primary, &mirror);
        res.unwrap();
        assert_eq!(fs::read(dest).unwrap(), JAR);
    }

    #[test]
    fn meta_json_md5_mismatch_is_rejected() {
        let (primary, _) = serve(vec![
            (INSTALLER, vec![503], vec![]),
            ("/meta.json", vec![200], meta_doc(&md5_hex(b"other"))),
        ]);
        let (mirror, _) = serve(vec![(INSTALLER, vec![200], JAR.to_vec())]);
        let (res, dest) = run(&primary, &mirror);
        assert!(res.is_err());
        assert!(!dest.exists());
    }

    #[test]
    fn missing_everywhere_is_a_real_not_found() {
        let (primary, _) = serve(vec![]);
        let (mirror, _) = serve(vec![]);
        let (res, _) = run(&primary, &mirror);
        assert!(is_not_found(&res.unwrap_err()));
    }

    #[test]
    fn meta_url_for_forge_installers_only() {
        assert_eq!(
            meta_url("https://maven.minecraftforge.net/net/minecraftforge/forge/1.20.1-47.3.0/forge-1.20.1-47.3.0-installer.jar")
                .as_deref(),
            Some("https://files.minecraftforge.net/net/minecraftforge/forge/1.20.1-47.3.0/meta.json")
        );
        assert_eq!(
            meta_url("https://maven.neoforged.net/releases/x/y.jar"),
            None
        );
    }

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
