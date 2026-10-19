//! One-click fixes for the crashes the launcher can explain (`crash::Fix`): switching off the
//! mods a crash names, and moving an instance's Fabric Loader to the newest stable build.

use std::path::Path;

use crate::crash::Fix;
use crate::instances::{Instance, Loader};
use crate::loaders::LoaderKind;
use crate::storage::DataDirs;
use crate::{Error, Result};

/// Do `fix` for `instance`; says in words what happened (for a pop-up). A loader update is saved
/// on the instance here.
pub fn apply(dirs: &DataDirs, instance: &mut Instance, fix: &Fix) -> Result<String> {
    match fix {
        Fix::DisableMods(names) => {
            let mods_dir = instance.game_dir(dirs).join("mods");
            let off = disable_named(&mods_dir, names)?;
            Ok(match off.len() {
                0 => "None of those mods are switched on any more.".to_owned(),
                n => format!(
                    "Switched off {n} mod{}: {}. You can switch them back on in the Mods page.",
                    if n == 1 { "" } else { "s" },
                    off.join(", ")
                ),
            })
        }
        Fix::UpdateLoader => update_loader(dirs, instance),
    }
}

/// Rename the enabled mods whose name or id is one of `names` to `.jar.disabled`; returns the
/// names of those switched off.
fn disable_named(mods_dir: &Path, names: &[String]) -> Result<Vec<String>> {
    let wanted: Vec<String> = names.iter().map(|n| n.trim().to_lowercase()).collect();
    let mut off = Vec::new();
    let Ok(entries) = std::fs::read_dir(mods_dir) else {
        return Ok(off);
    };
    for entry in entries.flatten() {
        let file = entry.file_name().to_string_lossy().into_owned();
        // The launcher's own files are never switched off from here.
        if !file.to_lowercase().ends_with(".jar")
            || file == crate::arctic_mod::FILE_NAME
            || file == crate::mods::polarium::FILE_NAME
        {
            continue;
        }
        let Some((id, name)) = identity(&entry.path()) else {
            continue;
        };
        if wanted.contains(&id.to_lowercase()) || wanted.contains(&name.to_lowercase()) {
            crate::mods::set_enabled(mods_dir, &file, false)?;
            off.push(name);
        }
    }
    Ok(off)
}

/// A Fabric mod's id and display name, from its `fabric.mod.json`.
fn identity(jar: &Path) -> Option<(String, String)> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(jar).ok()?).ok()?;
    let mut text = String::new();
    std::io::Read::read_to_string(&mut zip.by_name("fabric.mod.json").ok()?, &mut text).ok()?;
    // Some mods put raw newlines inside strings; lenient enough for id and name.
    let json: serde_json::Value =
        serde_json::from_str(&text.replace(['\n', '\r', '\t'], " ")).ok()?;
    let id = json.get("id")?.as_str()?.to_owned();
    let name = json
        .get("name")
        .and_then(|n| n.as_str())
        .unwrap_or(&id)
        .to_owned();
    Some((id, name))
}

fn update_loader(dirs: &DataDirs, instance: &mut Instance) -> Result<String> {
    let kind = match instance.loader.kind() {
        Some(kind @ (LoaderKind::Fabric | LoaderKind::Quilt)) => kind,
        Some(_) => {
            return Err(Error::Other(
                "Only Fabric and Quilt loaders can be updated from here.".into(),
            ));
        }
        None => {
            return Err(Error::Other(
                "This instance uses the game's own loader, which the launcher keeps up to date itself."
                    .into(),
            ));
        }
    };
    let game = instance
        .version
        .clone()
        .ok_or_else(|| Error::Other("This instance has no Minecraft version of its own.".into()))?;
    let newest = crate::loaders::default_loader_version(kind, &game)?;
    instance.loader = Loader::new(Some(kind), newest.clone());
    instance.save(dirs)?;
    Ok(format!("{} updated to {newest}.", kind.label()))
}
