//! What taking a manifest would change in an instance: mods added, removed
//! and updated, config files added, changed and removed, and a change of
//! Minecraft version or loader. Every item is accepted (or not) by the player
//! on the diff screen; nothing is applied without that.

use std::collections::HashMap;
use std::path::Path;

use super::manifest::{self, ConfigFile, LinkedMod, Manifest};
use super::paths;
use crate::instances::Loader;
use crate::mods;
use crate::{Error, Result};

/// A mod as it is in the instance now (from `mods.json`, file present).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalMod {
    pub project: String,
    pub version: String,
    pub version_number: String,
    pub title: String,
    pub file: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModChange {
    Add(LinkedMod),
    /// `from_link`: it came from the link earlier (else the player added it themselves).
    Remove {
        local: LocalMod,
        from_link: bool,
    },
    Update {
        local: LocalMod,
        to: LinkedMod,
    },
}

impl ModChange {
    pub fn title(&self) -> &str {
        match self {
            ModChange::Add(m) => &m.title,
            ModChange::Remove { local, .. } | ModChange::Update { local, .. } => &local.title,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigChange {
    Add(ConfigFile),
    Change(ConfigFile),
    /// A file the link brought earlier that the owner no longer has. `edited`: changed here since.
    Remove {
        path: String,
        edited: bool,
    },
}

impl ConfigChange {
    pub fn path(&self) -> &str {
        match self {
            ConfigChange::Add(c) | ConfigChange::Change(c) => &c.path,
            ConfigChange::Remove { path, .. } => path,
        }
    }
}

/// The owner moved the instance to another Minecraft version or loader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetChange {
    pub from_version: Option<String>,
    pub to_version: String,
    pub from_loader: Loader,
    pub to_loader: Loader,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item<T> {
    pub change: T,
    /// Ticked on the diff screen. Starts as what's usually wanted.
    pub accept: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diff {
    /// The manifest revision this diff takes the instance to (0: not from a link).
    pub revision: u64,
    pub target: Option<Item<TargetChange>>,
    pub mods: Vec<Item<ModChange>>,
    pub configs: Vec<Item<ConfigChange>>,
}

impl Diff {
    pub fn is_empty(&self) -> bool {
        self.target.is_none() && self.mods.is_empty() && self.configs.is_empty()
    }

    pub fn accepted_count(&self) -> usize {
        usize::from(self.target.as_ref().is_some_and(|t| t.accept))
            + self.mods.iter().filter(|m| m.accept).count()
            + self.configs.iter().filter(|c| c.accept).count()
    }

    /// Mods can only be added or updated on the version and loader they were picked for: with
    /// a version change waiting and not accepted, those items can't be applied.
    pub fn mods_blocked(&self) -> bool {
        self.target.as_ref().is_some_and(|t| !t.accept)
    }

    /// Whether this item can be applied as things are ticked now.
    pub fn mod_allowed(&self, change: &ModChange) -> bool {
        !self.mods_blocked() || matches!(change, ModChange::Remove { .. })
    }

    /// Accepted items that can be applied (see [`Diff::mods_blocked`]), as a diff.
    pub fn selected(&self) -> Diff {
        Diff {
            revision: self.revision,
            target: self.target.clone().filter(|t| t.accept),
            mods: self
                .mods
                .iter()
                .filter(|m| m.accept && self.mod_allowed(&m.change))
                .cloned()
                .collect(),
            configs: self.configs.iter().filter(|c| c.accept).cloned().collect(),
        }
    }
}

/// The mods of an instance as the diff sees them: Modrinth mods whose file is there,
/// launcher-managed files left aside.
pub fn local_mods(game_dir: &Path, index: &Path) -> Result<Vec<LocalMod>> {
    let mods_dir = game_dir.join("mods");
    let managed = manifest::managed_files(game_dir);
    Ok(mods::list(&mods_dir, index)?
        .into_iter()
        .filter(|f| !managed.contains(&f.file_name))
        .filter_map(|f| {
            let t = f.tracked?;
            Some(LocalMod {
                project: t.project_id,
                version: t.version_id,
                version_number: t.version_number,
                title: t.title,
                file: f.file_name,
                enabled: f.enabled,
            })
        })
        .collect())
}

/// What taking `wanted` would change. `previous` is the manifest taken last time (what came
/// from the link), so removals of the owner's mods are ticked and the player's own are not.
pub fn compute(
    wanted: &Manifest,
    revision: u64,
    previous: Option<&Manifest>,
    version: Option<&str>,
    loader: &Loader,
    game_dir: &Path,
    index: &Path,
) -> Result<Diff> {
    let local = local_mods(game_dir, index)?;
    let target =
        (version != Some(wanted.game_version.as_str()) || *loader != wanted.loader).then(|| Item {
            change: TargetChange {
                from_version: version.map(str::to_owned),
                to_version: wanted.game_version.clone(),
                from_loader: loader.clone(),
                to_loader: wanted.loader.clone(),
            },
            accept: true,
        });
    let mut mods = Vec::new();
    let have: HashMap<&str, &LocalMod> = local.iter().map(|m| (m.project.as_str(), m)).collect();
    for m in &wanted.mods {
        match have.get(m.project.as_str()) {
            None => mods.push(Item {
                change: ModChange::Add(m.clone()),
                accept: true,
            }),
            Some(l) if l.version != m.version => mods.push(Item {
                change: ModChange::Update {
                    local: (*l).clone(),
                    to: m.clone(),
                },
                accept: true,
            }),
            Some(_) => {}
        }
    }
    for l in &local {
        if wanted.mods.iter().any(|m| m.project == l.project) {
            continue;
        }
        let from_link = previous.is_some_and(|p| p.mods.iter().any(|m| m.project == l.project));
        mods.push(Item {
            change: ModChange::Remove {
                local: l.clone(),
                from_link,
            },
            accept: from_link,
        });
    }
    let mut configs = Vec::new();
    for c in &wanted.configs {
        let path = paths::resolve(game_dir, &c.path)?;
        match std::fs::read(&path) {
            Ok(bytes) if manifest::sha256_hex(&bytes) == c.sha256 => {}
            Ok(_) => configs.push(Item {
                change: ConfigChange::Change(c.clone()),
                accept: true,
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => configs.push(Item {
                change: ConfigChange::Add(c.clone()),
                accept: true,
            }),
            Err(e) => return Err(Error::io(path, e)),
        }
    }
    if let Some(previous) = previous {
        for old in &previous.configs {
            if wanted.configs.iter().any(|c| c.path == old.path) {
                continue;
            }
            let Ok(path) = paths::resolve(game_dir, &old.path) else {
                continue;
            };
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            let edited = manifest::sha256_hex(&bytes) != old.sha256;
            configs.push(Item {
                change: ConfigChange::Remove {
                    path: old.path.clone(),
                    edited,
                },
                accept: !edited,
            });
        }
    }
    mods.sort_by_key(|m| m.change.title().to_lowercase());
    configs.sort_by(|a, b| a.change.path().cmp(b.change.path()));
    Ok(Diff {
        revision,
        target,
        mods,
        configs,
    })
}

/// A diff of mods to add only (the missing-mods flow: nothing removed or replaced).
pub fn additions(mods: Vec<LinkedMod>) -> Diff {
    Diff {
        revision: 0,
        target: None,
        mods: mods
            .into_iter()
            .map(|m| Item {
                change: ModChange::Add(m),
                accept: true,
            })
            .collect(),
        configs: Vec::new(),
    }
}
