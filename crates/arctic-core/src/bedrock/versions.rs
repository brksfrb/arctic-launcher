//! Bedrock versions that can be installed: every GDK build (1.21.120 and
//! newer) of Release and Preview, with the links to Microsoft's packages,
//! and the older UWP builds before that (known by their Windows Update id;
//! `fe3` turns the id into a link when one is installed). The lists come
//! from community archives that record each build as it's published; the
//! result is kept on disk so it still shows offline.

use std::cmp::Ordering;
use std::path::Path;
use std::sync::atomic::AtomicBool;

use serde::{Deserialize, Serialize};

use super::Edition;
use crate::{Error, Result};

/// The main list: every build with its package links and MD5.
const VERSION_DB: &str = "https://raw.githubusercontent.com/LiteLDev/minecraft-windows-gdk-version-db/refs/heads/main/historical_versions.json";
/// Used when the main list can't be reached: `{ "release": { "1.26.52.3": [urls] }, "preview": … }`.
const FALLBACK_DB: &str = "https://raw.githubusercontent.com/MinecraftBedrockArchiver/GdkLinks/refs/heads/master/urls.json";
/// The older (UWP) builds: `[["1.20.81.01", "<update id>", "0", "x64"], …]` (type 0 Release, 1 Beta, 2 Preview).
const LEGACY_DB: &str = "https://www.raythnetwork.co.uk/versions.php?type=json";
/// Where the last list is kept, in the launcher's cache folder.
const CACHE_FILE: &str = "bedrock-versions.json";

/// A Bedrock build that can be installed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Version {
    pub edition: Edition,
    /// "1.26.52.3".
    pub id: String,
    /// Mirrors of the same package (Microsoft's CDN); none for an older (UWP) build.
    pub urls: Vec<String>,
    /// The Windows Update id of an older (UWP, before 1.21.120) build, whose link `fe3` looks up.
    #[serde(default)]
    pub update_id: Option<String>,
}

impl Version {
    /// The name players know ("1.26.52"; a Preview keeps its build number).
    pub fn display(&self) -> String {
        display(&self.id, self.edition)
    }

    /// An older build (the UWP app type before 1.21.120).
    pub fn is_legacy(&self) -> bool {
        self.update_id.is_some()
    }
}

/// "1.26.52" for a Release ("1.26.52.3"), the full number for a Preview.
pub fn display(id: &str, edition: Edition) -> String {
    match edition {
        Edition::Release => {
            let parts: Vec<&str> = id.split('.').collect();
            if parts.len() == 4 {
                parts[..3].join(".")
            } else {
                id.to_owned()
            }
        }
        Edition::Preview => id.to_owned(),
    }
}

/// Every installable build, newest first.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Catalog {
    pub release: Vec<Version>,
    pub preview: Vec<Version>,
}

impl Catalog {
    pub fn of(&self, edition: Edition) -> &[Version] {
        match edition {
            Edition::Release => &self.release,
            Edition::Preview => &self.preview,
        }
    }

    /// A build by its number ("1.26.52.3", or "1.26.52" for the newest such Release).
    pub fn find(&self, edition: Edition, id: &str) -> Option<&Version> {
        let id = id.trim();
        let list = self.of(edition);
        list.iter().find(|v| v.id == id).or_else(|| {
            let wanted = normalize(id)?;
            list.iter().find(|v| {
                v.id == wanted
                    || (v.id.starts_with(&format!("{wanted}.")) && wanted.matches('.').count() == 2)
            })
        })
    }

    pub fn latest(&self, edition: Edition) -> Option<&Version> {
        self.of(edition).first()
    }

    /// Add the older builds the GDK list doesn't have.
    pub(crate) fn add_legacy(&mut self, legacy: Vec<Version>) {
        for version in legacy {
            let list = match version.edition {
                Edition::Release => &mut self.release,
                Edition::Preview => &mut self.preview,
            };
            if !list.iter().any(|v| v.id == version.id) {
                list.push(version);
            }
        }
        self.release.sort_by(|a, b| compare(&b.id, &a.id));
        self.preview.sort_by(|a, b| compare(&b.id, &a.id));
    }
}

/// The list of builds: fetched, or the copy from last time when offline.
pub fn catalog(cache_dir: &Path) -> Result<Catalog> {
    let cached = cache_dir.join(CACHE_FILE);
    let fetched = fetch_main().or_else(|e| {
        log::warn!("the Bedrock version list couldn't be loaded ({e}), trying the other one");
        fetch_fallback()
    });
    match fetched {
        Ok(mut catalog) => {
            // The older builds are a bonus: without them the rest still works.
            let legacy = fetch_legacy().unwrap_or_else(|e| {
                log::warn!("the older Bedrock builds couldn't be listed ({e}), using the last list");
                kept_legacy(&cached)
            });
            catalog.add_legacy(legacy);
            if let Err(e) = crate::storage::save_json(&cached, &catalog) {
                log::warn!("couldn't keep the Bedrock version list: {e}");
            }
            Ok(catalog)
        }
        Err(e) => match crate::storage::load_json::<Catalog>(&cached) {
            Ok(Some(catalog)) => Ok(catalog),
            _ => Err(e),
        },
    }
}

/// The older builds of the list kept from last time.
fn kept_legacy(cached: &Path) -> Vec<Version> {
    let Ok(Some(old)) = crate::storage::load_json::<Catalog>(cached) else {
        return Vec::new();
    };
    old.release
        .into_iter()
        .chain(old.preview)
        .filter(Version::is_legacy)
        .collect()
}

fn fetch_legacy() -> Result<Vec<Version>> {
    parse_legacy(&get_text(LEGACY_DB)?)
}

/// Release and Preview builds for 64-bit Windows (Beta needs a Microsoft account to download).
pub(crate) fn parse_legacy(text: &str) -> Result<Vec<Version>> {
    let rows: Vec<Vec<String>> = serde_json::from_str(text)?;
    let list: Vec<Version> = rows
        .into_iter()
        .filter_map(|row| {
            let [number, update_id, kind, arch] = <[String; 4]>::try_from(row).ok()?;
            let edition = match kind.as_str() {
                "0" => Edition::Release,
                "2" => Edition::Preview,
                _ => return None,
            };
            if arch != "x64" || !super::fe3::is_update_id(&update_id) {
                return None;
            }
            Some(Version {
                edition,
                id: normalize(&number)?,
                urls: Vec::new(),
                update_id: Some(update_id),
            })
        })
        .collect();
    if list.is_empty() {
        return Err(Error::Other("the older Bedrock builds list is empty".into()));
    }
    Ok(list)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MainDb {
    #[serde(default)]
    release_versions: Vec<MainEntry>,
    #[serde(default)]
    preview_versions: Vec<MainEntry>,
}

#[derive(Deserialize)]
struct MainEntry {
    /// "Release 1.26.52.03".
    version: String,
    urls: Vec<String>,
}

fn fetch_main() -> Result<Catalog> {
    let text = get_text(VERSION_DB)?;
    parse_main(&text)
}

fn fetch_fallback() -> Result<Catalog> {
    let text = get_text(FALLBACK_DB)?;
    parse_fallback(&text)
}

fn get_text(url: &str) -> Result<String> {
    let mut resp = crate::net::agent().get(url).call()?;
    Ok(resp
        .body_mut()
        .with_config()
        .limit(16 * 1024 * 1024)
        .read_to_string()?)
}

pub(crate) fn parse_main(text: &str) -> Result<Catalog> {
    let db: MainDb = serde_json::from_str(text)?;
    let convert = |entries: Vec<MainEntry>, edition: Edition| -> Vec<Version> {
        let list = entries
            .into_iter()
            .filter_map(|e| {
                let number = e.version.rsplit(' ').next()?;
                Some(Version {
                    edition,
                    id: normalize(number)?,
                    urls: clean_urls(e.urls),
                    update_id: None,
                })
            })
            .collect();
        sorted(list)
    };
    let catalog = Catalog {
        release: convert(db.release_versions, Edition::Release),
        preview: convert(db.preview_versions, Edition::Preview),
    };
    if catalog.release.is_empty() && catalog.preview.is_empty() {
        return Err(Error::Other("the Bedrock version list is empty".into()));
    }
    Ok(catalog)
}

pub(crate) fn parse_fallback(text: &str) -> Result<Catalog> {
    use std::collections::BTreeMap;
    let db: BTreeMap<String, BTreeMap<String, Vec<String>>> = serde_json::from_str(text)?;
    let convert = |key: &str, edition: Edition| -> Vec<Version> {
        let list = db
            .get(key)
            .into_iter()
            .flatten()
            .filter_map(|(id, urls)| {
                Some(Version {
                    edition,
                    id: normalize(id)?,
                    urls: clean_urls(urls.clone()),
                    update_id: None,
                })
            })
            .collect();
        sorted(list)
    };
    Ok(Catalog {
        release: convert("release", Edition::Release),
        preview: convert("preview", Edition::Preview),
    })
}

/// Only Microsoft's package links (the list is someone else's data).
fn clean_urls(urls: Vec<String>) -> Vec<String> {
    let mut urls: Vec<String> = urls
        .into_iter()
        .filter(|u| {
            let Some(rest) = u
                .strip_prefix("http://")
                .or_else(|| u.strip_prefix("https://"))
            else {
                return false;
            };
            let host = rest.split('/').next().unwrap_or("").to_ascii_lowercase();
            (host.ends_with(".xboxlive.com") || host.ends_with(".xboxlive.cn"))
                && u.to_ascii_lowercase().ends_with(".msixvc")
        })
        .collect();
    // The global CDN first, China's mirrors after.
    urls.sort_by_key(|u| u.contains(".xboxlive.cn/"));
    urls
}

fn sorted(mut list: Vec<Version>) -> Vec<Version> {
    list.retain(|v| !v.urls.is_empty() || v.is_legacy());
    list.sort_by(|a, b| compare(&b.id, &a.id));
    list.dedup_by(|a, b| a.id == b.id);
    list
}

/// "1.26.52.03" → "1.26.52.3"; `None` when it isn't a version number.
pub fn normalize(text: &str) -> Option<String> {
    let parts: Option<Vec<u64>> = text.trim().split('.').map(|p| p.parse().ok()).collect();
    let parts = parts.filter(|p| (2..=4).contains(&p.len()))?;
    Some(
        parts
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join("."),
    )
}

/// Compare version numbers part by part.
pub fn compare(a: &str, b: &str) -> Ordering {
    let parse = |s: &str| -> Vec<u64> { s.split('.').map(|p| p.parse().unwrap_or(0)).collect() };
    parse(a).cmp(&parse(b))
}

/// The game version of a package version: the third part packs the patch
/// and the build (1.26.5203.0 → 1.26.52.3).
pub fn from_package_version(package: &str) -> String {
    let parts: Vec<u64> = package.split('.').filter_map(|p| p.parse().ok()).collect();
    match parts.as_slice() {
        [major, minor, build, ..] if *build >= 100 => {
            format!("{major}.{minor}.{}.{}", build / 100, build % 100)
        }
        _ => package.to_owned(),
    }
}

/// Install `version` over the installed game of its edition (newer or
/// older). Windows downloads the package and unlocks it with the Store
/// license, so the game must be owned on this computer's Microsoft account.
/// Worlds and settings are kept: they live outside the package.
pub fn install(
    version: &Version,
    cancel: &AtomicBool,
    progress: crate::Progress<'_>,
) -> Result<()> {
    if !super::supported() {
        return Err(Error::Other(
            "Bedrock Edition can only be installed on Windows".into(),
        ));
    }
    if !super::gaming_services() {
        return Err(Error::Other(
            "Gaming Services isn't installed: Windows needs it to install Bedrock. \
             Get it from the Microsoft Store, then try again."
                .into(),
        ));
    }
    #[cfg(windows)]
    {
        if version.is_legacy() {
            return install_legacy(version, cancel, progress);
        }
        let stage = format!(
            "Installing {} {}",
            version.edition.name(),
            version.display()
        );
        make_room(version)?;
        let mut last = Error::Other("no download link for this version".into());
        for url in &version.urls {
            match super::package::install(url, &stage, cancel, progress) {
                Ok(()) => return Ok(()),
                Err(e) if cancel.load(std::sync::atomic::Ordering::Relaxed) => return Err(e),
                Err(e) => {
                    log::warn!("installing Bedrock from {url} failed: {e}");
                    // Only a link that couldn't be reached is worth another
                    // mirror; anything else (a license, the disk) fails the same.
                    let unreachable = ["[0x80072E", "[0x80072F", "[0x8019"]
                        .iter()
                        .any(|code| e.to_string().contains(code));
                    last = e;
                    if !unreachable {
                        break;
                    }
                }
            }
        }
        Err(last)
    }
    #[cfg(not(windows))]
    {
        let _ = (version, cancel, progress);
        unreachable!()
    }
}

/// A GDK game and an older (UWP) one can't replace each other in place: take the installed one off
/// first (its worlds and settings are kept, they live outside the package).
#[cfg(windows)]
fn make_room(version: &Version) -> Result<()> {
    match super::find(version.edition) {
        Some(installed) if installed.gdk == version.is_legacy() => {
            super::package::remove(version.edition.package_family())
        }
        _ => Ok(()),
    }
}

/// An older build: Microsoft's update service gives the link, the package is fetched here (it is
/// signed by Microsoft and a plain zip), and Windows installs it.
#[cfg(windows)]
fn install_legacy(
    version: &Version,
    cancel: &AtomicBool,
    progress: crate::Progress<'_>,
) -> Result<()> {
    use crate::error::IoContext;
    use crate::net::{DownloadJob, download_all};
    let update_id = version.update_id.as_deref().unwrap_or_default();
    let name = format!("{} {}", version.edition.name(), version.display());
    let url = super::fe3::resolve(update_id)?;
    let dir = std::env::temp_dir().join("arctic-bedrock-install");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).at(&dir)?;
    let result = (|| {
        let file = dir.join("package.download");
        download_all(
            &format!("Downloading {name}"),
            vec![DownloadJob {
                url,
                dest: file.clone(),
                sha1: None,
                size: None,
                lzma: None,
            }],
            progress,
        )?;
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(Error::Other("cancelled".into()));
        }
        let package = with_package_extension(&file)?;
        make_room(version)?;
        super::package::install(&file_uri(&package), &format!("Installing {name}"), cancel, progress)
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// Windows tells a package from a bundle by its extension: look inside and name the file for what it is.
#[cfg(windows)]
fn with_package_extension(file: &Path) -> Result<std::path::PathBuf> {
    use crate::error::IoContext;
    let zip = std::fs::File::open(file).at(file)?;
    let archive = zip::ZipArchive::new(zip)
        .map_err(|_| Error::Other("what Microsoft sent isn't a game package".into()))?;
    let bundle = archive
        .file_names()
        .any(|name| name.ends_with("AppxBundleManifest.xml"));
    drop(archive);
    let named = file.with_extension(if bundle { "appxbundle" } else { "appx" });
    std::fs::rename(file, &named).at(file)?;
    Ok(named)
}

/// `file:///C:/…` for a local path.
#[cfg(windows)]
fn file_uri(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    let text = text
        .replace('%', "%25")
        .replace(' ', "%20")
        .replace('#', "%23");
    format!("file:///{text}")
}
