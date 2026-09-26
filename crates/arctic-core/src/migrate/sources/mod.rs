//! Finding other launchers' instances and clients' settings on this PC.

mod clients;
mod managers;
mod minecraft;
mod modrinth;

use std::path::{Path, PathBuf};

use super::{Found, FoundLoader, Launcher, Scan};
use crate::loaders::LoaderKind;

/// Everything importable on this PC. Never fails: a launcher whose files
/// can't be read is listed in `seen` with the reason.
pub fn scan() -> Scan {
    let mut scan = Scan::default();
    minecraft::official(&mut scan);
    minecraft::tlauncher(&mut scan);
    managers::prism_family(&mut scan);
    modrinth::app(&mut scan);
    managers::curseforge(&mut scan);
    managers::atlauncher(&mut scan);
    managers::gdlauncher(&mut scan);
    clients::labymod(&mut scan);
    clients::lunar(&mut scan);
    clients::feather(&mut scan);
    clients::badlion(&mut scan);
    tidy(&mut scan);
    scan
}

/// One entry per `.minecraft` for its worlds and packs, no duplicates
/// (TLauncher also writes the official launcher's profiles), and no
/// entries for shared-folder versions that would bring nothing.
fn tidy(scan: &mut Scan) {
    let mut vanilla_dirs: Vec<PathBuf> = Vec::new();
    let mut seen: Vec<(PathBuf, String, Option<FoundLoader>)> = Vec::new();
    let mut empty = 0;
    scan.instances.retain_mut(|f| {
        if f.into_vanilla {
            if vanilla_dirs.contains(&f.game_dir) {
                return false;
            }
            vanilla_dirs.push(f.game_dir.clone());
            f.name = "Worlds, packs & options".into();
            return true;
        }
        // Versions installed into .minecraft (official launcher, TLauncher)
        // with nothing of their own are noise, and TLauncher also writes the
        // official launcher's profiles; a client's instances are things the
        // player made, so they always show.
        if !matches!(f.launcher, Launcher::Official | Launcher::TLauncher) {
            return true;
        }
        if f.shared && f.extra_jars.is_empty() && !super::copy::has_mods(f) {
            empty += 1;
            return false;
        }
        let key = (f.game_dir.clone(), f.game_version.clone(), f.loader.clone());
        if seen.contains(&key) {
            return false;
        }
        seen.push(key);
        true
    });
    if empty > 0 {
        scan.seen.push((
            Launcher::Folder,
            format!("{empty} versions in shared folders have no mods of their own, so there's nothing extra to bring (their worlds and packs come with \"Worlds, packs & options\")"),
        ));
    }
}

/// Instances in a folder the player picked: an instance folder of any
/// supported launcher, a folder of them, or a launcher's data folder
/// (portable MultiMC, a moved CurseForge folder…).
pub fn scan_folder(path: &Path) -> Scan {
    let mut scan = Scan::default();
    managers::folder(path, &mut scan);
    if scan.instances.is_empty() {
        scan.seen.push((
            Launcher::Folder,
            "No instances there that Arctic can read".into(),
        ));
    }
    scan
}

/// Roaming app data on Windows, the data folder elsewhere.
pub(super) fn app_data() -> Option<PathBuf> {
    if cfg!(windows) {
        dirs::config_dir()
    } else {
        dirs::data_dir()
    }
}

/// The official `.minecraft` folder.
pub(super) fn dot_minecraft() -> Option<PathBuf> {
    if cfg!(windows) {
        dirs::config_dir().map(|d| d.join(".minecraft"))
    } else if cfg!(target_os = "macos") {
        dirs::data_dir().map(|d| d.join("minecraft"))
    } else {
        dirs::home_dir().map(|h| h.join(".minecraft"))
    }
}

/// A new entry with nothing optional set.
pub(super) fn found(
    launcher: Launcher,
    name: &str,
    game_version: &str,
    loader: Option<FoundLoader>,
    game_dir: PathBuf,
) -> Found {
    Found {
        launcher,
        name: name.trim().to_owned(),
        game_version: game_version.trim().to_owned(),
        loader,
        game_dir,
        mods_dir: None,
        shared: false,
        memory_mb: None,
        extra_jars: Vec::new(),
        disabled_mods_dir: None,
        extra_dirs: Vec::new(),
        into_vanilla: false,
        notes: Vec::new(),
    }
}

/// A loader from a launcher's own words (`fabric`, `NeoForge`, `forge`…),
/// with the version cleaned (`1.20.1-47.2.0` → `47.2.0`).
pub(super) fn loader(kind: &str, version: Option<&str>, game: &str) -> Option<FoundLoader> {
    let kind = match kind
        .to_ascii_lowercase()
        .replace(['-', '_', ' '], "")
        .as_str()
    {
        "fabric" => LoaderKind::Fabric,
        "quilt" => LoaderKind::Quilt,
        "forge" => LoaderKind::Forge,
        "neoforge" | "neo" => LoaderKind::NeoForge,
        _ => return None,
    };
    let version = version
        .map(str::trim)
        .filter(|v| {
            !v.is_empty() && !v.eq_ignore_ascii_case("latest") && !v.eq_ignore_ascii_case("stable")
        })
        .map(|v| {
            let v = v.strip_prefix(&format!("{game}-")).unwrap_or(v);
            v.strip_suffix(&format!("-{game}")).unwrap_or(v).to_owned()
        });
    Some(FoundLoader { kind, version })
}

pub(super) fn read_json(path: &Path) -> Option<serde_json::Value> {
    super::detect::read_json(path)
}

/// Sub-folders of `dir` (sorted by name).
pub(super) fn subdirs(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|e| {
            e.flatten()
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

/// Memory settings outside a sane range are ignored.
pub(super) fn memory(mb: Option<u64>) -> Option<u32> {
    mb.filter(|m| (512..=65536).contains(m)).map(|m| m as u32)
}
