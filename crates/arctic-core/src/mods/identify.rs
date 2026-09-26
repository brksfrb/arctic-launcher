//! Recognize mod files (from another launcher) on Modrinth by their SHA-1,
//! so they're tracked like mods installed here: shown with names, and
//! shareable and updatable. Unknown files stay as they are.

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use serde_json::json;
use sha1::{Digest, Sha1};

use super::index::ModIndex;
use super::modrinth::{self, Version};
use super::{InstalledMod, with_project_info};
use crate::Result;
use crate::net::agent;

/// Hashes per request.
const BATCH: usize = 25;
/// Modrinth has off moments; try a few times before giving up.
const TRIES: u32 = 3;
const RETRY_WAIT: std::time::Duration = std::time::Duration::from_secs(2);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Recognized {
    /// Files now tracked in the index.
    pub tracked: usize,
    /// Files Modrinth doesn't know (still installed, just not tracked).
    pub unknown: usize,
    /// Files Modrinth says are for another Minecraft version or loader.
    pub mismatched: Vec<String>,
}

/// Look up every jar in `mods_dir` and record the known ones in `index`.
pub fn recognize(
    mods_dir: &Path,
    index: &Path,
    game_version: &str,
    loader: crate::loaders::LoaderKind,
) -> Result<Recognized> {
    let mut hashes: HashMap<String, String> = HashMap::new();
    for entry in std::fs::read_dir(mods_dir)
        .map_err(|e| crate::Error::io(mods_dir, e))?
        .flatten()
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !(name.ends_with(".jar") || name.ends_with(".jar.disabled")) {
            continue;
        }
        if let Ok(sha) = sha1_of(&entry.path()) {
            hashes.insert(sha, name);
        }
    }
    let mut found: HashMap<String, Version> = HashMap::new();
    let keys: Vec<String> = hashes.keys().cloned().collect();
    for chunk in keys.chunks(BATCH) {
        found.extend(lookup_retrying(chunk)?);
    }
    let loaders = modrinth::compatible_loaders(loader);
    let mut report = Recognized::default();
    let mut entries = Vec::new();
    for (sha, file) in &hashes {
        let Some(version) = found.get(sha) else {
            report.unknown += 1;
            continue;
        };
        if !version.is_compatible(&loaders, game_version) {
            report.mismatched.push(file.clone());
        }
        let base = file.strip_suffix(".disabled").unwrap_or(file);
        entries.push(InstalledMod {
            project_id: version.project_id.clone(),
            version_id: version.id.clone(),
            title: version.project_id.clone(),
            version_number: version.version_number.clone(),
            file_name: base.to_owned(),
            icon_url: None,
            dependency: false,
        });
    }
    report.tracked = entries.len();
    let mut updated = ModIndex::load(index)?;
    for m in with_project_info(entries) {
        updated = updated.with(m);
    }
    updated.save(index)?;
    report.mismatched.sort();
    Ok(report)
}

fn lookup_retrying(hashes: &[String]) -> Result<HashMap<String, Version>> {
    let mut attempt = 1;
    loop {
        match lookup(hashes) {
            Ok(found) => return Ok(found),
            Err(_) if attempt < TRIES => {
                std::thread::sleep(RETRY_WAIT * attempt);
                attempt += 1;
            }
            Err(e) => return Err(e),
        }
    }
}

/// `POST /version_files`: hash → version, for the hashes Modrinth knows.
fn lookup(hashes: &[String]) -> Result<HashMap<String, Version>> {
    let url = format!("{}/version_files", modrinth::API);
    let mut resp = agent()
        .post(&url)
        .header("User-Agent", modrinth::user_agent())
        .config()
        .http_status_as_error(false)
        .timeout_recv_response(Some(modrinth::SLOW_API))
        .timeout_recv_body(Some(modrinth::SLOW_API))
        .build()
        .send_json(json!({ "hashes": hashes, "algorithm": "sha1" }))?;
    if let Some(e) = modrinth::status_error(resp.status().as_u16(), "mod lookup") {
        return Err(e);
    }
    Ok(resp
        .body_mut()
        .with_config()
        .limit(32 * 1024 * 1024)
        .read_json()?)
}

fn sha1_of(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha1::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}
