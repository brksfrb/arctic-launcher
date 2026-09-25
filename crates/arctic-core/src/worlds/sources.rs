//! Places to import worlds from: the official launcher, other launchers
//! found on this PC, and this profile's other instances.

use std::path::{Path, PathBuf};

use crate::instances;
use crate::storage::DataDirs;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    /// "Minecraft Launcher", "Prism Launcher: Fabric 1.21", "Instance: Modded"…
    pub label: String,
    pub saves: PathBuf,
}

/// Every `saves` folder with at least one world, except `exclude` (the
/// instance being imported into).
pub fn sources(dirs: &DataDirs, exclude: &Path) -> Vec<Source> {
    let mut out = Vec::new();
    let mut add = |label: String, saves: PathBuf| {
        if saves != exclude && !super::list(&saves).is_empty() {
            out.push(Source { label, saves });
        }
    };
    if let Some(dot) = official_dir() {
        add("Minecraft Launcher".into(), dot.join("saves"));
    }
    for (launcher, root) in prism_like_roots() {
        for (name, game_dir) in prism_instances(&root) {
            add(format!("{launcher}: {name}"), game_dir.join("saves"));
        }
    }
    let mut all = instances::list_custom(dirs).unwrap_or_default();
    if let Ok(default) = instances::load_default(dirs) {
        all.insert(0, default);
    }
    for instance in all {
        add(
            format!("Instance: {}", instance.name),
            instance.game_dir(dirs).join("saves"),
        );
    }
    out
}

/// The official launcher's `.minecraft`.
fn official_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        dirs::config_dir().map(|d| d.join(".minecraft"))
    } else {
        dirs::home_dir().map(|h| h.join(".minecraft"))
    }
}

/// (launcher name, data folder) for launchers using the MultiMC layout.
fn prism_like_roots() -> Vec<(&'static str, PathBuf)> {
    let mut roots = Vec::new();
    let data = if cfg!(windows) {
        dirs::config_dir()
    } else {
        dirs::data_dir()
    };
    if let Some(base) = data {
        roots.push(("Prism Launcher", base.join("PrismLauncher")));
        roots.push(("PolyMC", base.join("PolyMC")));
    }
    roots
}

/// (instance name, game folder) for each instance of a MultiMC-style launcher.
fn prism_instances(root: &Path) -> Vec<(String, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(root.join("instances")) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|e| {
            let dir = e.path();
            let game = [".minecraft", "minecraft"]
                .iter()
                .map(|n| dir.join(n))
                .find(|p| p.is_dir())?;
            let name = instance_cfg_name(&dir)
                .unwrap_or_else(|| e.file_name().to_string_lossy().into_owned());
            Some((name, game))
        })
        .collect()
}

/// `name=` from a MultiMC/Prism `instance.cfg`.
fn instance_cfg_name(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join("instance.cfg")).ok()?;
    text.lines()
        .find_map(|l| l.strip_prefix("name="))
        .map(|n| n.trim().to_owned())
        .filter(|n| !n.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_prism_instances() {
        let root = tempfile::tempdir().unwrap();
        let inst = root.path().join("instances").join("abc");
        std::fs::create_dir_all(inst.join(".minecraft")).unwrap();
        std::fs::write(
            inst.join("instance.cfg"),
            "InstanceType=OneSix\nname=Fabric 1.21\n",
        )
        .unwrap();
        let found = prism_instances(root.path());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, "Fabric 1.21");
        assert!(found[0].1.ends_with(".minecraft"));
    }
}
