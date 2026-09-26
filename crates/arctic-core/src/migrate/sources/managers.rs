//! Instance managers that keep one folder per instance: Prism / PolyMC /
//! MultiMC, CurseForge, ATLauncher and GDLauncher (new and old).

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::{app_data, found, loader, memory, read_json, subdirs};
use crate::migrate::{Found, Launcher, Scan};

pub fn prism_family(scan: &mut Scan) {
    let Some(base) = app_data() else {
        return;
    };
    for (launcher, dir, cfg) in [
        (Launcher::Prism, "PrismLauncher", "prismlauncher.cfg"),
        (Launcher::PolyMc, "PolyMC", "polymc.cfg"),
    ] {
        let root = base.join(dir);
        if root.is_dir() {
            let instances = instance_dir(&root, cfg);
            add_all(scan, launcher, &instances, |d| prism(d, launcher));
        }
    }
}

/// `InstanceDir=` from the launcher's config (relative to its folder).
fn instance_dir(root: &Path, cfg: &str) -> PathBuf {
    let custom = std::fs::read_to_string(root.join(cfg))
        .ok()
        .and_then(|text| {
            text.lines()
                .find_map(|l| l.strip_prefix("InstanceDir="))
                .map(|v| v.trim().to_owned())
                .filter(|v| !v.is_empty())
        });
    match custom {
        Some(dir) => root.join(dir),
        None => root.join("instances"),
    }
}

/// A MultiMC-format instance (`instance.cfg` + `mmc-pack.json`).
fn prism(dir: &Path, launcher: Launcher) -> Option<Found> {
    let cfg = std::fs::read_to_string(dir.join("instance.cfg")).ok()?;
    let key = |k: &str| {
        cfg.lines()
            .find_map(|l| l.strip_prefix(&format!("{k}=")))
            .map(|v| v.trim().to_owned())
    };
    let pack = read_json(&dir.join("mmc-pack.json"))?;
    let components = pack.get("components")?.as_array()?;
    let version_of = |uid: &str| {
        components
            .iter()
            .find(|c| c.get("uid").and_then(Value::as_str) == Some(uid))
            .and_then(|c| c.get("version").and_then(Value::as_str))
            .map(str::to_owned)
    };
    let game = version_of("net.minecraft")?;
    let found_loader = [
        ("net.neoforged", "neoforge"),
        ("net.minecraftforge", "forge"),
        ("org.quiltmc.quilt-loader", "quilt"),
        ("net.fabricmc.fabric-loader", "fabric"),
    ]
    .iter()
    .find_map(|(uid, kind)| version_of(uid).and_then(|v| loader(kind, Some(&v), &game)));
    let game_dir = [".minecraft", "minecraft"]
        .iter()
        .map(|n| dir.join(n))
        .find(|p| p.is_dir())
        .unwrap_or_else(|| dir.join(".minecraft"));
    let name = key("name").unwrap_or_else(|| folder_name(dir));
    let mut entry = found(launcher, &name, &game, found_loader, game_dir);
    if key("OverrideMemory").as_deref() == Some("true") {
        entry.memory_mb = memory(key("MaxMemAlloc").and_then(|v| v.parse().ok()));
    }
    if components.iter().any(|c| {
        c.get("uid")
            .and_then(Value::as_str)
            .is_some_and(|u| u.contains("liteloader") || u.contains("optifine"))
    }) {
        entry
            .notes
            .push("LiteLoader and OptiFine components aren't supported and are left out".into());
    }
    Some(entry)
}

pub fn curseforge(scan: &mut Scan) {
    let roots: Vec<PathBuf> = [dirs::home_dir(), dirs::document_dir()]
        .into_iter()
        .flatten()
        .map(|d| d.join("curseforge").join("minecraft").join("Instances"))
        .filter(|d| d.is_dir())
        .collect();
    for root in roots {
        add_all(scan, Launcher::CurseForge, &root, curseforge_instance);
    }
}

fn curseforge_instance(dir: &Path) -> Option<Found> {
    let json = read_json(&dir.join("minecraftinstance.json"))?;
    let text = |k: &str| json.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
    let game = text("gameVersion");
    if game.is_empty() {
        return None;
    }
    let base = json.get("baseModLoader").cloned().unwrap_or(Value::Null);
    let full = base.get("name").and_then(Value::as_str).unwrap_or("");
    let found_loader = full
        .split_once('-')
        .and_then(|(kind, version)| loader(kind, Some(version), &game));
    let name = Some(text("name"))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| folder_name(dir));
    let mut entry = found(
        Launcher::CurseForge,
        &name,
        &game,
        found_loader,
        dir.to_path_buf(),
    );
    if json.get("isMemoryOverride").and_then(Value::as_bool) == Some(true) {
        entry.memory_mb = memory(json.get("allocatedMemory").and_then(Value::as_u64));
    }
    Some(entry)
}

pub fn atlauncher(scan: &mut Scan) {
    let Some(base) = app_data() else {
        return;
    };
    let root = base.join("ATLauncher").join("instances");
    if root.is_dir() {
        add_all(scan, Launcher::AtLauncher, &root, atlauncher_instance);
    }
}

fn atlauncher_instance(dir: &Path) -> Option<Found> {
    let json = read_json(&dir.join("instance.json"))?;
    let launcher = json.get("launcher")?;
    let game = json.get("id").and_then(Value::as_str)?.to_owned();
    let name = launcher
        .get("name")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| folder_name(dir));
    let lv = launcher.get("loaderVersion");
    let found_loader = lv.and_then(|lv| {
        let kind = lv.get("type").and_then(Value::as_str)?;
        loader(kind, lv.get("version").and_then(Value::as_str), &game)
    });
    let mut entry = found(
        Launcher::AtLauncher,
        &name,
        &game,
        found_loader,
        dir.to_path_buf(),
    );
    entry.memory_mb = memory(launcher.get("maximumMemory").and_then(Value::as_u64));
    let disabled = dir.join("disabledmods");
    entry.disabled_mods_dir = disabled.is_dir().then_some(disabled);
    if lv.and_then(|l| l.get("type")).and_then(Value::as_str) == Some("LegacyFabric") {
        entry
            .notes
            .push("Legacy Fabric isn't supported; this comes over without a loader".into());
    }
    Some(entry)
}

pub fn gdlauncher(scan: &mut Scan) {
    let Some(base) = app_data() else {
        return;
    };
    for root in [
        base.join("gdlauncher_carbon")
            .join("data")
            .join("instances"),
        base.join("gdlauncher_carbon").join("instances"),
    ] {
        if root.is_dir() {
            add_all(scan, Launcher::GdLauncher, &root, gdlauncher_instance);
        }
    }
    let legacy = base.join("gdlauncher_next").join("instances");
    if legacy.is_dir() {
        add_all(scan, Launcher::GdLauncherLegacy, &legacy, gdlauncher_legacy);
    }
}

fn gdlauncher_instance(dir: &Path) -> Option<Found> {
    let json = read_json(&dir.join("instance.json"))?;
    let config = json.get("game_configuration")?;
    let version = config.get("version")?;
    let game = version.get("release").and_then(Value::as_str)?.to_owned();
    let found_loader = version
        .get("modloaders")
        .and_then(Value::as_array)
        .and_then(|l| l.first())
        .and_then(|m| {
            let kind = m.get("type").and_then(Value::as_str)?;
            loader(kind, m.get("version").and_then(Value::as_str), &game)
        });
    let name = json
        .get("name")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| folder_name(dir));
    let mut entry = found(
        Launcher::GdLauncher,
        &name,
        &game,
        found_loader,
        dir.join("instance"),
    );
    entry.memory_mb = memory(config.pointer("/memory/max_mb").and_then(Value::as_u64));
    Some(entry)
}

fn gdlauncher_legacy(dir: &Path) -> Option<Found> {
    let json = read_json(&dir.join("config.json"))?;
    let l = json.get("loader")?;
    let game = l.get("mcVersion").and_then(Value::as_str)?.to_owned();
    let found_loader = l
        .get("loaderType")
        .and_then(Value::as_str)
        .and_then(|kind| loader(kind, l.get("loaderVersion").and_then(Value::as_str), &game));
    Some(found(
        Launcher::GdLauncherLegacy,
        &folder_name(dir),
        &game,
        found_loader,
        dir.to_path_buf(),
    ))
}

/// Every instance under `root` that `read` understands.
fn add_all(
    scan: &mut Scan,
    launcher: Launcher,
    root: &Path,
    read: impl Fn(&Path) -> Option<Found>,
) {
    let before = scan.instances.len();
    for dir in subdirs(root) {
        if let Some(entry) = read(&dir) {
            scan.instances.push(entry);
        }
    }
    if scan.instances.len() == before {
        scan.seen
            .push((launcher, format!("No instances in {}", root.display())));
    }
}

/// A picked folder: an instance of any kind, or a folder holding them.
pub fn folder(path: &Path, scan: &mut Scan) {
    if let Some(entry) = any_instance(path) {
        scan.instances.push(entry);
        return;
    }
    for dir in [
        path.to_path_buf(),
        path.join("instances"),
        path.join("Instances"),
    ] {
        for sub in subdirs(&dir) {
            if let Some(entry) = any_instance(&sub) {
                scan.instances.push(entry);
            }
        }
    }
    scan.instances.dedup_by(|a, b| a.game_dir == b.game_dir);
}

fn any_instance(dir: &Path) -> Option<Found> {
    prism(dir, Launcher::MultiMc)
        .or_else(|| curseforge_instance(dir))
        .or_else(|| atlauncher_instance(dir))
        .or_else(|| gdlauncher_instance(dir))
        .or_else(|| gdlauncher_legacy(dir))
        .or_else(|| super::modrinth::legacy_profile(dir))
}

fn folder_name(dir: &Path) -> String {
    dir.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Imported".into())
}
