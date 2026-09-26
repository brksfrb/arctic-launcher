//! PvP clients: their own mods for each version (LabyMod, Lunar, Feather)
//! and their HUD setups (LabyMod, Lunar). The clients themselves aren't
//! carried over; the Arctic Client takes their place.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::{app_data, dot_minecraft, found, loader, memory, read_json, subdirs};
use crate::migrate::{Found, Launcher, Scan, hud};

const NO_CLIENT: &str =
    "The client itself and its own add-ons stay behind; the Arctic Client takes their place";

pub fn labymod(scan: &mut Scan) {
    let Some(root) = app_data().map(|d| d.join("LabyMod")) else {
        return;
    };
    if let Some(json) = read_json(&root.join("instances.json")) {
        let list = json
            .get("instances")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for inst in list {
            if let Some(f) = labymod_instance(&inst, &root) {
                scan.instances.push(f);
            }
        }
    }
    if let Some(mc) = dot_minecraft() {
        hud::labymod(&mc.join("labymod-neo").join("configs"), scan);
    }
}

fn labymod_instance(inst: &Value, root: &Path) -> Option<Found> {
    let uid = inst.get("uid").and_then(Value::as_str)?;
    let name = inst
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("LabyMod");
    let components = inst.get("components").and_then(Value::as_array)?;
    let version_of = |id: &str| {
        components
            .iter()
            .find(|c| c.get("uid").and_then(Value::as_str) == Some(id))
            .and_then(|c| c.get("version").and_then(Value::as_str))
    };
    let game = version_of("net.minecraft")?.to_owned();
    let dir = root.join("instances").join(uid);
    let separate = inst.get("separateGameDirectory").and_then(Value::as_bool) == Some(true);
    let game_dir = if separate {
        dir.clone()
    } else {
        dot_minecraft()?
    };
    // LabyMod runs its own Fabric-based loader ("labyModLoader": "fabric")
    // even when no Fabric component is listed.
    let laby_fabric = inst.get("labyModLoader").and_then(Value::as_str) == Some("fabric");
    let fabric = version_of("net.fabricmc.fabric-loader").or(laby_fabric.then_some("latest"));
    let mut notes = vec![NO_CLIENT.to_owned()];
    let found_loader = match fabric {
        Some(_) if old_game(&game) => {
            notes.push(format!(
                "Fabric doesn't run on {game}; this comes over without a loader"
            ));
            None
        }
        Some(v) => loader("fabric", Some(v), &game),
        None => None,
    };
    let mut f = found(Launcher::LabyMod, name, &game, found_loader, game_dir);
    f.shared = !separate;
    let mods = dir.join("mods");
    f.mods_dir = Some(mods);
    f.memory_mb = memory(inst.get("ram").and_then(Value::as_u64));
    if !separate {
        notes.push(
            "Its worlds and packs live in .minecraft; they come with \"Worlds, packs & options\""
                .into(),
        );
    }
    f.notes = notes;
    Some(f)
}

pub fn lunar(scan: &mut Scan) {
    let Some(root) = dirs::home_dir().map(|h| h.join(".lunarclient")) else {
        return;
    };
    if !root.is_dir() {
        return;
    }
    let settings = read_json(&root.join("settings").join("launcher.json"));
    let game_dir = settings
        .as_ref()
        .and_then(|s| s.pointer("/settings/gameDirectory").and_then(Value::as_str))
        .map(PathBuf::from)
        .or_else(dot_minecraft);
    let ram = settings.as_ref().and_then(|s| {
        s.pointer("/settings/allocatedMemory")
            .and_then(Value::as_u64)
    });
    let before = scan.instances.len();
    if let Some(game_dir) = game_dir {
        for major in subdirs(&root.join("profiles").join("lunar")) {
            for mods in subdirs(&major.join("mods")) {
                if let Some(mut f) = client_mods(Launcher::Lunar, &mods, &game_dir) {
                    f.memory_mb = memory(ram);
                    for (from, to) in [
                        ("resourcepacks", "resourcepacks"),
                        ("shaders", "shaderpacks"),
                    ] {
                        let dir = major.join(from);
                        if dir.is_dir() {
                            f.extra_dirs.push((dir, to));
                        }
                    }
                    scan.instances.push(f);
                }
            }
        }
    }
    if scan.instances.len() == before {
        scan.seen.push((
            Launcher::Lunar,
            "No mods of your own (worlds and packs are in .minecraft)".into(),
        ));
    }
    hud::lunar(&root.join("settings").join("game"), scan);
}

pub fn feather(scan: &mut Scan) {
    let Some(root) = app_data().map(|d| d.join(".feather")) else {
        return;
    };
    if !root.is_dir() {
        return;
    }
    let before = scan.instances.len();
    if let Some(game_dir) = dot_minecraft() {
        for mods in subdirs(&root.join("user-mods")) {
            if let Some(f) = client_mods(Launcher::Feather, &mods, &game_dir) {
                scan.instances.push(f);
            }
        }
    }
    let note = if scan.instances.len() == before {
        "No mods of your own (worlds and packs are in .minecraft)"
    } else {
        "Feather's HUD and mod settings use a private format and aren't read"
    };
    scan.seen.push((Launcher::Feather, note.into()));
}

pub fn badlion(scan: &mut Scan) {
    let installed = dot_minecraft().is_some_and(|m| m.join("BLClient-Mod-Profiles").is_dir())
        || app_data().is_some_and(|d| d.join("Badlion Client").is_dir());
    if installed {
        scan.seen.push((
            Launcher::Badlion,
            "Badlion keeps worlds and packs in .minecraft (import Minecraft Launcher's \"Latest release\"); its mod profiles use a private format and aren't read".into(),
        ));
    }
}

/// A client's own-mods folder named `<loader>-<version>` (`fabric-1.21.11`)
/// or `<version>-<loader>` (`1.21-fabric`); empty folders are skipped.
fn client_mods(launcher: Launcher, mods: &Path, game_dir: &Path) -> Option<Found> {
    let folder = mods.file_name()?.to_string_lossy().into_owned();
    let (kind, game) = match folder.split_once('-')? {
        (k @ ("fabric" | "forge" | "quilt" | "neoforge"), v) => (k.to_owned(), v.to_owned()),
        (v, k @ ("fabric" | "forge" | "quilt" | "neoforge")) => (k.to_owned(), v.to_owned()),
        _ => return None,
    };
    if !has_jars(mods) {
        return None;
    }
    let name = format!("{} {game} mods", launcher.name());
    let mut f = found(
        launcher,
        &name,
        &game,
        loader(&kind, None, &game),
        game_dir.to_path_buf(),
    );
    f.shared = true;
    f.mods_dir = Some(mods.to_path_buf());
    f.notes.push(NO_CLIENT.into());
    Some(f)
}

fn has_jars(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .map(|e| {
            e.flatten()
                .any(|e| e.file_name().to_string_lossy().ends_with(".jar"))
        })
        .unwrap_or(false)
}

/// Before 1.14: no Fabric (Legacy Fabric isn't supported).
fn old_game(game: &str) -> bool {
    game.strip_prefix("1.")
        .and_then(|rest| rest.split('.').next())
        .and_then(|minor| minor.parse::<u32>().ok())
        .is_some_and(|minor| minor < 14)
}
