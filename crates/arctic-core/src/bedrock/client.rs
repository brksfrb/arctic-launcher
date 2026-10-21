//! Third-party clients for Bedrock Edition (Latite, Flarial).
//!
//! Bedrock has no mod loader, so every Bedrock client is a DLL that gets loaded into the running game
//! (Windows only), which is also how other launchers offer them. Arctic doesn't ship or rebuild these:
//! the player picks one, the launcher downloads the DLL from the client's own releases (checking the
//! SHA-256 GitHub lists for it) and loads it into Bedrock once the game is up.
//!
//! Servers can ban players for modified clients, and the clients are not made by Arctic; the Bedrock tab
//! says both before a client is switched on.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::storage::DataDirs;
use crate::{Error, Result};

/// How long the game gets to load the client (milliseconds).
#[cfg(windows)]
const LOAD_TIMEOUT_MS: u32 = 30_000;

/// The most a client DLL may be (bytes): they are a few MB.
const MAX_DLL_BYTES: u64 = 128 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Client {
    Latite,
    Flarial,
}

impl Client {
    pub const ALL: [Client; 2] = [Client::Latite, Client::Flarial];

    pub fn name(self) -> &'static str {
        match self {
            Client::Latite => "Latite Client",
            Client::Flarial => "Flarial Client",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Client::Latite => "latite",
            Client::Flarial => "flarial",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "latite" | "latite-client" => Some(Client::Latite),
            "flarial" | "flarial-client" => Some(Client::Flarial),
            _ => None,
        }
    }

    /// The GitHub repository whose releases carry the DLL.
    fn repo(self) -> &'static str {
        match self {
            Client::Latite => "Imrglop/Latite-Releases",
            Client::Flarial => "flarialmc/dll-oss",
        }
    }

    /// The release file to load into the game.
    fn asset(self) -> &'static str {
        match self {
            Client::Latite => "Latite.dll",
            Client::Flarial => "Flarial.dll",
        }
    }

    /// Flarial publishes its builds as pre-releases.
    fn takes_prereleases(self) -> bool {
        matches!(self, Client::Flarial)
    }

    pub fn site(self) -> &'static str {
        match self {
            Client::Latite => "https://latite.net",
            Client::Flarial => "https://flarial.xyz",
        }
    }

    /// What the client is, for the Bedrock tab.
    pub fn about(self) -> &'static str {
        match self {
            Client::Latite => {
                "FPS, CPS, keystrokes, zoom and more on a draggable HUD, made for Minecraft Windows."
            }
            Client::Flarial => {
                "30+ modules (FPS, CPS, keystrokes, zoom, motion blur and more) with its own menu."
            }
        }
    }
}

/// A release of a client, as GitHub lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub client: Client,
    pub tag: String,
    /// When it came out (`2026-03-04`).
    pub published: String,
    pub url: String,
    /// The SHA-256 GitHub lists for the file, when it does.
    pub sha256: Option<String>,
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    published_at: String,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
    /// `sha256:<hex>`.
    #[serde(default)]
    digest: Option<String>,
}

/// The newest usable release in a GitHub listing (newest first).
fn pick(client: Client, releases: &[GhRelease]) -> Option<Release> {
    releases
        .iter()
        .filter(|r| !r.draft && (client.takes_prereleases() || !r.prerelease))
        .find_map(|r| {
            let asset = r.assets.iter().find(|a| a.name == client.asset())?;
            let sha256 = asset
                .digest
                .as_deref()
                .and_then(|d| d.strip_prefix("sha256:"))
                .map(str::to_ascii_lowercase);
            Some(Release {
                client,
                tag: r.tag_name.clone(),
                published: r.published_at.chars().take(10).collect(),
                url: asset.browser_download_url.clone(),
                sha256,
            })
        })
}

/// The newest release of `client`.
pub fn latest(client: Client) -> Result<Release> {
    let releases: Vec<GhRelease> = crate::net::get_json(&format!(
        "https://api.github.com/repos/{}/releases?per_page=15",
        client.repo()
    ))?;
    pick(client, &releases)
        .ok_or_else(|| Error::Other(format!("{} has no release to download", client.name())))
}

fn client_dir(dirs: &DataDirs, client: Client) -> PathBuf {
    dirs.cache().join("bedrock-clients").join(client.id())
}

/// A tag as a folder name (letters, digits, dots and dashes only).
fn tag_folder(tag: &str) -> String {
    let name: String = tag
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    // Only dots would be a folder step (`..`), not a name.
    if name.chars().all(|c| c == '.') {
        "_".to_owned()
    } else {
        name
    }
}

/// The newest DLL already downloaded, if any (for when GitHub can't be reached).
pub fn cached(dirs: &DataDirs, client: Client) -> Option<PathBuf> {
    let mut found: Vec<(std::time::SystemTime, PathBuf)> =
        std::fs::read_dir(client_dir(dirs, client))
            .ok()?
            .flatten()
            .filter_map(|e| {
                let dll = e.path().join(client.asset());
                let modified = std::fs::metadata(&dll).and_then(|m| m.modified()).ok()?;
                Some((modified, dll))
            })
            .collect();
    found.sort_by_key(|found| std::cmp::Reverse(found.0));
    found.into_iter().next().map(|(_, dll)| dll)
}

/// Download `release` (once; it's kept) and return the DLL's path. Older downloads are removed.
pub fn download(dirs: &DataDirs, release: &Release) -> Result<PathBuf> {
    let folder = client_dir(dirs, release.client).join(tag_folder(&release.tag));
    let dll = folder.join(release.client.asset());
    if dll.is_file() {
        return Ok(dll);
    }
    let mut resp = crate::net::agent().get(&release.url).call()?;
    let bytes = resp
        .body_mut()
        .with_config()
        .limit(MAX_DLL_BYTES)
        .read_to_vec()?;
    if let Some(expected) = &release.sha256 {
        let actual = hex::encode(Sha256::digest(&bytes));
        if !actual.eq_ignore_ascii_case(expected) {
            return Err(Error::Other(format!(
                "the download of {} doesn't match the checksum GitHub lists for it; not using it",
                release.client.name()
            )));
        }
    }
    if bytes.len() < 1024 || &bytes[..2] != b"MZ" {
        return Err(Error::Other(format!(
            "what was downloaded for {} isn't a program file",
            release.client.name()
        )));
    }
    std::fs::create_dir_all(&folder).map_err(|e| Error::io(&folder, e))?;
    let tmp = folder.join("download.tmp");
    std::fs::write(&tmp, &bytes).map_err(|e| Error::io(&tmp, e))?;
    std::fs::rename(&tmp, &dll).map_err(|e| Error::io(&dll, e))?;
    // Keep only this release.
    if let Ok(entries) = std::fs::read_dir(client_dir(dirs, release.client)) {
        for entry in entries.flatten() {
            if entry.path() != folder {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }
    Ok(dll)
}

/// The client's DLL ready to load: the newest release, or the last download when offline.
pub fn prepare(dirs: &DataDirs, client: Client) -> Result<PathBuf> {
    match latest(client).and_then(|release| download(dirs, &release)) {
        Ok(dll) => Ok(dll),
        Err(e) => cached(dirs, client).ok_or(e),
    }
}

/// Load `dll` into the running process `pid` (Windows): the same call every other Bedrock client
/// launcher makes, `LoadLibraryW` run on a thread started inside the game.
#[cfg(windows)]
pub fn inject(pid: u32, dll: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Diagnostics::Debug::WriteProcessMemory;
    use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
    use windows_sys::Win32::System::Memory::{
        MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_READWRITE, VirtualAllocEx, VirtualFreeEx,
    };
    use windows_sys::Win32::System::Threading::{
        CreateRemoteThread, GetExitCodeThread, OpenProcess, PROCESS_CREATE_THREAD,
        PROCESS_QUERY_INFORMATION, PROCESS_VM_OPERATION, PROCESS_VM_READ, PROCESS_VM_WRITE,
        WaitForSingleObject,
    };

    let path: Vec<u16> = dll
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let bytes = path.len() * 2;
    let kernel32: Vec<u16> = "kernel32.dll\0".encode_utf16().collect();
    // SAFETY: plain Win32 calls; the buffer written into the game holds exactly `bytes` bytes, every
    // handle is closed and the allocation is freed.
    unsafe {
        let process = OpenProcess(
            PROCESS_CREATE_THREAD
                | PROCESS_QUERY_INFORMATION
                | PROCESS_VM_OPERATION
                | PROCESS_VM_WRITE
                | PROCESS_VM_READ,
            0,
            pid,
        );
        if process.is_null() {
            return Err(Error::Other(
                "couldn't open the game's process (is it running as another user?)".into(),
            ));
        }
        let result = (|| -> Result<()> {
            let remote = VirtualAllocEx(
                process,
                std::ptr::null(),
                bytes,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_READWRITE,
            );
            if remote.is_null() {
                return Err(Error::Other("couldn't reserve memory in the game".into()));
            }
            let mut written = 0usize;
            if WriteProcessMemory(process, remote, path.as_ptr().cast(), bytes, &mut written) == 0
                || written != bytes
            {
                VirtualFreeEx(process, remote, 0, MEM_RELEASE);
                return Err(Error::Other("couldn't write to the game's memory".into()));
            }
            let load = GetProcAddress(
                GetModuleHandleW(kernel32.as_ptr()),
                c"LoadLibraryW".as_ptr().cast(),
            );
            let Some(load) = load else {
                VirtualFreeEx(process, remote, 0, MEM_RELEASE);
                return Err(Error::Other("couldn't find LoadLibraryW".into()));
            };
            let thread = CreateRemoteThread(
                process,
                std::ptr::null(),
                0,
                Some(std::mem::transmute::<
                    unsafe extern "system" fn() -> isize,
                    unsafe extern "system" fn(*mut std::ffi::c_void) -> u32,
                >(load)),
                remote,
                0,
                std::ptr::null_mut(),
            );
            if thread.is_null() {
                VirtualFreeEx(process, remote, 0, MEM_RELEASE);
                return Err(Error::Other(
                    "couldn't start the loader inside the game".into(),
                ));
            }
            // A DLL that hangs while loading must not hang the launcher too.
            let finished = WaitForSingleObject(thread, LOAD_TIMEOUT_MS) == 0;
            let mut loaded = 0u32;
            GetExitCodeThread(thread, &mut loaded);
            CloseHandle(thread);
            if finished {
                VirtualFreeEx(process, remote, 0, MEM_RELEASE);
            }
            if !finished {
                return Err(Error::Other(
                    "the game didn't finish loading the client in time".into(),
                ));
            }
            if loaded == 0 {
                return Err(Error::Other(
                    "the game refused to load the client (it may not support this game version yet)".into(),
                ));
            }
            Ok(())
        })();
        CloseHandle(process);
        result
    }
}

#[cfg(not(windows))]
pub fn inject(_pid: u32, _dll: &Path) -> Result<()> {
    Err(Error::Other(
        "Bedrock clients can only be loaded on Windows".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listing() -> Vec<GhRelease> {
        serde_json::from_str(
            r#"[
              {"tag_name":"v2.7.0-rc1","published_at":"2026-05-01T10:00:00Z","prerelease":true,"draft":false,
               "assets":[{"name":"Latite.dll","browser_download_url":"https://example.invalid/rc.dll"}]},
              {"tag_name":"v2.6.2","published_at":"2026-03-04T12:30:00Z","prerelease":false,"draft":false,
               "assets":[{"name":"readme.txt","browser_download_url":"https://example.invalid/readme.txt"},
                         {"name":"Latite.dll","browser_download_url":"https://example.invalid/Latite.dll",
                          "digest":"sha256:ABCDEF"}]},
              {"tag_name":"v2.6.1","published_at":"2025-06-26T09:00:00Z","prerelease":false,"draft":false,
               "assets":[{"name":"Latite.dll","browser_download_url":"https://example.invalid/old.dll"}]}
            ]"#,
        )
        .unwrap()
    }

    #[test]
    fn the_newest_stable_release_with_the_dll_is_picked() {
        let release = pick(Client::Latite, &listing()).unwrap();
        assert_eq!(release.tag, "v2.6.2");
        assert_eq!(release.published, "2026-03-04");
        assert_eq!(release.url, "https://example.invalid/Latite.dll");
        assert_eq!(release.sha256.as_deref(), Some("abcdef"));
    }

    #[test]
    fn flarial_takes_its_pre_releases() {
        let releases: Vec<GhRelease> = serde_json::from_str(
            r#"[{"tag_name":"beta-5a80f3b","published_at":"2025-11-03T00:00:00Z","prerelease":true,"draft":false,
                 "assets":[{"name":"Flarial.dll","browser_download_url":"https://example.invalid/f.dll"}]}]"#,
        )
        .unwrap();
        assert_eq!(
            pick(Client::Flarial, &releases).unwrap().tag,
            "beta-5a80f3b"
        );
        assert!(pick(Client::Latite, &releases).is_none());
    }

    #[test]
    fn a_release_without_the_file_is_skipped() {
        let releases: Vec<GhRelease> = serde_json::from_str(
            r#"[{"tag_name":"v1","published_at":"","prerelease":false,"draft":false,"assets":[]}]"#,
        )
        .unwrap();
        assert!(pick(Client::Latite, &releases).is_none());
    }

    #[test]
    fn clients_parse_by_name() {
        assert_eq!(Client::parse(" Latite "), Some(Client::Latite));
        assert_eq!(Client::parse("flarial-client"), Some(Client::Flarial));
        assert_eq!(Client::parse("lunar"), None);
    }

    #[test]
    fn tags_become_safe_folder_names() {
        assert_eq!(tag_folder("v2.6.2"), "v2.6.2");
        assert_eq!(tag_folder("../beta/x"), ".._beta_x");
        assert_eq!(tag_folder(".."), "_");
    }
}
