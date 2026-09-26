//! Self-update from GitHub Releases.
//!
//! * **Channels**: `Stable` only considers full releases; `Beta` also
//!   considers pre-releases.
//! * **Optional vs required**: a release is *required* when its notes contain
//!   `<!-- arctic:required -->`, or when the running version is older than a
//!   `<!-- arctic:min-supported=X.Y.Z -->` marker. Required updates block
//!   launching until installed; optional ones show a dismissible banner.
//! * **Integrity**: the Windows asset must carry a GitHub `sha256:` digest,
//!   which is verified before the running executable is replaced.
//!
//! The whole launcher is one small executable, so an update is a single
//! download + in-place swap + restart.

use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::Command;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::IoContext;
use crate::net::{agent, body_timeout, get_json};
use crate::storage::DataDirs;
use crate::{APP_VERSION, Error, Progress, ProgressInfo, Result};

pub const GITHUB_REPO: &str = "brksfrb/arctic-launcher";
/// Release asset of the launcher for this platform.
#[cfg(windows)]
pub const PLATFORM_ASSET: &str = "arctic-launcher-windows-x64.exe";
#[cfg(not(windows))]
pub const PLATFORM_ASSET: &str = "arctic-launcher-linux-x64";
const REQUIRED_MARKER: &str = "<!-- arctic:required -->";
const MIN_SUPPORTED_PREFIX: &str = "<!-- arctic:min-supported=";
const RELEASES_PER_PAGE: u32 = 20;
const CHUNK: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateChannel {
    #[default]
    Stable,
    Beta,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UpdateInfo {
    pub version: semver::Version,
    pub required: bool,
    pub notes: String,
    pub page_url: String,
    pub asset_url: String,
    pub asset_size: u64,
    /// Lowercase hex SHA-256 of the asset.
    pub sha256: String,
}

#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    #[serde(default)]
    body: Option<String>,
    prerelease: bool,
    draft: bool,
    html_url: String,
    assets: Vec<Asset>,
}

#[derive(Debug, Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    size: u64,
    digest: Option<String>,
}

/// Ask GitHub for the newest applicable release. `Ok(None)` = up to date.
pub fn check(channel: UpdateChannel) -> Result<Option<UpdateInfo>> {
    let url =
        format!("https://api.github.com/repos/{GITHUB_REPO}/releases?per_page={RELEASES_PER_PAGE}");
    let releases: Vec<Release> = get_json(&url)?;
    let current = semver::Version::parse(APP_VERSION).map_err(|e| Error::Other(e.to_string()))?;
    Ok(pick_update(&releases, channel, &current))
}

fn pick_update(
    releases: &[Release],
    channel: UpdateChannel,
    current: &semver::Version,
) -> Option<UpdateInfo> {
    let eligible = releases
        .iter()
        .filter(|r| !r.draft && (channel == UpdateChannel::Beta || !r.prerelease))
        .filter_map(|r| parse_tag(&r.tag_name).map(|v| (v, r)))
        .filter(|(v, _)| v > current);
    let (version, release) = eligible.max_by(|a, b| a.0.cmp(&b.0))?;
    let asset = release.assets.iter().find(|a| a.name == PLATFORM_ASSET)?;
    let sha256 = asset
        .digest
        .as_deref()?
        .strip_prefix("sha256:")?
        .to_lowercase();
    let notes = release.body.clone().unwrap_or_default();
    Some(UpdateInfo {
        required: is_required(&notes, current),
        version,
        notes,
        page_url: release.html_url.clone(),
        asset_url: asset.browser_download_url.clone(),
        asset_size: asset.size,
        sha256,
    })
}

fn parse_tag(tag: &str) -> Option<semver::Version> {
    semver::Version::parse(tag.trim_start_matches('v')).ok()
}

fn is_required(notes: &str, current: &semver::Version) -> bool {
    let min_supported = notes
        .split(MIN_SUPPORTED_PREFIX)
        .nth(1)
        .and_then(|rest| rest.split("-->").next())
        .and_then(|v| parse_tag(v.trim()));
    notes.contains(REQUIRED_MARKER) || min_supported.is_some_and(|min| current < &min)
}

/// Download, verify and swap in the new executable. Call `restart` afterwards.
pub fn download_and_apply(dirs: &DataDirs, info: &UpdateInfo, progress: Progress) -> Result<()> {
    let dir = dirs.cache().join("updates");
    fs::create_dir_all(&dir).at(&dir)?;
    let path = dir.join(format!("arctic-launcher-{}.exe", info.version));

    let mut resp = agent()
        .get(&info.asset_url)
        .config()
        .timeout_recv_body(Some(body_timeout(Some(info.asset_size))))
        .build()
        .call()?;
    let mut reader = resp.body_mut().as_reader();
    let mut file = fs::File::create(&path).at(&path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; CHUNK];
    let mut done = 0u64;
    loop {
        let n = reader.read(&mut buf).at(&path)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        file.write_all(&buf[..n]).at(&path)?;
        done += n as u64;
        progress(ProgressInfo {
            stage: "Downloading update",
            done: 0,
            total: 1,
            bytes_done: done,
            bytes_total: info.asset_size,
        });
    }
    drop(file);

    if hex::encode(hasher.finalize()) != info.sha256 {
        let _ = fs::remove_file(&path);
        return Err(Error::Checksum(PLATFORM_ASSET.into()));
    }
    self_replace::self_replace(&path)
        .map_err(|e| Error::Other(format!("could not install update: {e}")))?;
    let _ = fs::remove_file(&path);
    Ok(())
}

/// Start the (new) executable and let the caller exit. `--replace` tells it
/// to take over instead of handing off to this still-running process.
pub fn restart() -> Result<()> {
    let exe: PathBuf = std::env::current_exe().map_err(|e| Error::Other(e.to_string()))?;
    Command::new(&exe).arg("--replace").spawn().at(&exe)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str, prerelease: bool, body: &str, digest: Option<&str>) -> Release {
        Release {
            tag_name: tag.into(),
            body: Some(body.into()),
            prerelease,
            draft: false,
            html_url: format!("https://example/{tag}"),
            assets: vec![Asset {
                name: PLATFORM_ASSET.into(),
                browser_download_url: "u".into(),
                size: 10,
                digest: digest.map(str::to_owned),
            }],
        }
    }

    fn v(s: &str) -> semver::Version {
        semver::Version::parse(s).unwrap()
    }

    #[test]
    fn stable_skips_prereleases() {
        let rs = [
            release("v0.3.0-beta.1", true, "", Some("sha256:AB")),
            release("v0.2.0", false, "", Some("sha256:AB")),
        ];
        let stable = pick_update(&rs, UpdateChannel::Stable, &v("0.1.0")).unwrap();
        assert_eq!(stable.version, v("0.2.0"));
        assert_eq!(stable.sha256, "ab");
        let beta = pick_update(&rs, UpdateChannel::Beta, &v("0.1.0")).unwrap();
        assert_eq!(beta.version, v("0.3.0-beta.1"));
    }

    #[test]
    fn up_to_date_or_missing_digest_yields_none() {
        let rs = [release("v0.1.0", false, "", Some("sha256:ab"))];
        assert!(pick_update(&rs, UpdateChannel::Stable, &v("0.1.0")).is_none());
        let rs = [release("v0.2.0", false, "", None)];
        assert!(pick_update(&rs, UpdateChannel::Stable, &v("0.1.0")).is_none());
    }

    #[test]
    fn required_markers() {
        assert!(is_required("fix <!-- arctic:required -->", &v("0.1.0")));
        assert!(is_required(
            "<!-- arctic:min-supported=0.2.0 -->",
            &v("0.1.5")
        ));
        assert!(!is_required(
            "<!-- arctic:min-supported=0.2.0 -->",
            &v("0.2.0")
        ));
        assert!(!is_required("just notes", &v("0.1.0")));
    }
}
