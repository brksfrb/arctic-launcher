//! The official launcher's profiles, and TLauncher's versions (both live
//! in `.minecraft`).

use std::path::Path;

use serde_json::Value;

use super::{app_data, dot_minecraft, found, read_json, subdirs};
use crate::loaders::LoaderKind;
use crate::migrate::detect::{VersionInfo, vanilla_by_client, version_info, xmx_mb};
use crate::migrate::{Found, Launcher, Scan};

/// Folders that mean a version keeps its own game folder (TLauncher modpacks).
const OWN_GAME_DIR: [&str; 5] = ["mods", "saves", "config", "options.txt", "resourcepacks"];

pub fn official(scan: &mut Scan) {
    let Some(root) = dot_minecraft().filter(|r| r.is_dir()) else {
        return;
    };
    let Some(json) = read_json(&root.join("launcher_profiles.json")) else {
        scan.seen.push((
            Launcher::Official,
            "No launcher profiles in .minecraft".into(),
        ));
        return;
    };
    let versions = root.join("versions");
    let vanilla = vanilla_by_client(&versions);
    let profiles = json
        .get("profiles")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let before = scan.instances.len();
    for profile in profiles.values() {
        let text = |k: &str| profile.get(k).and_then(Value::as_str).unwrap_or("");
        let kind = text("type");
        let version = text("lastVersionId");
        let game_dir = Some(text("gameDir"))
            .filter(|d| !d.is_empty())
            .map(Into::into)
            .unwrap_or_else(|| root.clone());
        let latest = matches!(kind, "latest-release" | "latest-snapshot")
            || matches!(version, "latest-release" | "latest-snapshot");
        let name = match (text("name"), kind) {
            ("", "latest-snapshot") => "Latest snapshot".to_owned(),
            ("", _) if latest => "Latest release".to_owned(),
            ("", _) => version.to_owned(),
            (n, _) => n.to_owned(),
        };
        let mut entry = if latest {
            let mut f = found(Launcher::Official, &name, "latest", None, game_dir.clone());
            f.into_vanilla = true;
            f
        } else {
            let Some(info) = version_info(&versions, version, &vanilla) else {
                scan.seen.push((
                    Launcher::Official,
                    format!(
                        "\"{name}\": its version {version} isn't installed, so it can't be read"
                    ),
                ));
                continue;
            };
            from_version(Launcher::Official, &name, info, game_dir.clone())
        };
        entry.shared = same_dir(&game_dir, &root);
        entry.memory_mb = xmx_mb(text("javaArgs"));
        scan.instances.push(entry);
    }
    if scan.instances.len() == before {
        scan.seen.push((Launcher::Official, "No profiles".into()));
    }
}

pub fn tlauncher(scan: &mut Scan) {
    let Some(root) = dot_minecraft().filter(|r| r.is_dir()) else {
        return;
    };
    let config = app_data().map(|d| d.join(".tlauncher"));
    let present = root.join("TlauncherProfiles.json").is_file()
        || config.as_ref().is_some_and(|c| c.is_dir());
    if !present {
        return;
    }
    let memory = config
        .map(|c| c.join("tlauncher-2.0.properties"))
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|text| {
            text.lines()
                .filter_map(|l| l.strip_prefix("minecraft.memory."))
                .filter_map(|l| {
                    l.split_once('=')
                        .and_then(|(_, v)| v.trim().parse::<u64>().ok())
                })
                .next_back()
        });
    let versions = root.join("versions");
    let vanilla = vanilla_by_client(&versions);
    let before = scan.instances.len();
    for dir in subdirs(&versions) {
        let extra = dir.join("TLauncherAdditional.json");
        if !extra.is_file() {
            continue;
        }
        let id = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let Some(info) = version_info(&versions, &id, &vanilla) else {
            scan.seen.push((
                Launcher::TLauncher,
                format!("\"{id}\": couldn't tell what it runs"),
            ));
            continue;
        };
        // Plain versions share .minecraft; the Minecraft Launcher entry covers them.
        if info.loader.is_none() && info.optifine.is_none() && info.game_version == id {
            continue;
        }
        let own = OWN_GAME_DIR.iter().any(|n| dir.join(n).exists());
        let game_dir = if own { dir.clone() } else { root.clone() };
        let mut entry = from_version(Launcher::TLauncher, &id, info, game_dir);
        entry.shared = !own;
        let pack = read_json(&extra).and_then(|j| j.get("modpack").cloned());
        let pack_memory = pack
            .filter(|p| p.get("modpackMemory").and_then(Value::as_bool) == Some(true))
            .and_then(|p| p.get("memory").and_then(Value::as_u64));
        entry.memory_mb = super::memory(pack_memory.or(memory));
        scan.instances.push(entry);
    }
    if scan.instances.len() == before {
        scan.seen.push((
            Launcher::TLauncher,
            "No modded versions (plain ones come in with Minecraft Launcher)".into(),
        ));
    }
}

fn from_version(
    launcher: Launcher,
    name: &str,
    info: VersionInfo,
    game_dir: std::path::PathBuf,
) -> Found {
    let mut entry = found(
        launcher,
        name,
        &info.game_version,
        info.loader.clone(),
        game_dir,
    );
    entry.notes = info.notes;
    let forge = info
        .loader
        .as_ref()
        .is_some_and(|l| l.kind == LoaderKind::Forge);
    if let (Some(jar), true) = (info.optifine, forge) {
        entry.extra_jars.push(jar);
    }
    entry
}

fn same_dir(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}
