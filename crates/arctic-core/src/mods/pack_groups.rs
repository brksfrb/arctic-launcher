//! Groups for resource packs and shaders: a name a player puts on a pack so a long list stays
//! organised ("PvP", "Seasonal"). Kept in the game folder (`config/arctic-pack-groups.json`),
//! so the launcher and the game show the same groups; the pack files never move.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::packs::PackKind;
use crate::Result;
use crate::storage::{load_json, save_json};

const FILE: &str = "config/arctic-pack-groups.json";
/// Longest group name kept.
pub const MAX_NAME: usize = 32;

/// Pack file name to group name, for each kind of pack.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PackGroups {
    resourcepacks: BTreeMap<String, String>,
    shaderpacks: BTreeMap<String, String>,
}

fn path(game_dir: &Path) -> PathBuf {
    game_dir.join(FILE)
}

impl PackGroups {
    /// The groups of a game folder (none when it has no file yet or it's unreadable).
    pub fn load(game_dir: &Path) -> Self {
        load_json::<Self>(&path(game_dir))
            .ok()
            .flatten()
            .unwrap_or_default()
    }

    pub fn save(&self, game_dir: &Path) -> Result<()> {
        save_json(&path(game_dir), self)
    }

    fn map(&self, kind: PackKind) -> &BTreeMap<String, String> {
        match kind {
            PackKind::Resource => &self.resourcepacks,
            PackKind::Shader => &self.shaderpacks,
        }
    }

    fn map_mut(&mut self, kind: PackKind) -> &mut BTreeMap<String, String> {
        match kind {
            PackKind::Resource => &mut self.resourcepacks,
            PackKind::Shader => &mut self.shaderpacks,
        }
    }

    /// The group a pack file is in.
    pub fn group_of(&self, kind: PackKind, file_name: &str) -> Option<&str> {
        self.map(kind).get(file_name).map(String::as_str)
    }

    /// Put a pack in a group (`None`, or an empty name: out of any group).
    pub fn set(&mut self, kind: PackKind, file_name: &str, group: Option<&str>) {
        let name = group.map(clean).filter(|g| !g.is_empty());
        match name {
            Some(name) => {
                self.map_mut(kind).insert(file_name.to_owned(), name);
            }
            None => {
                self.map_mut(kind).remove(file_name);
            }
        }
    }

    /// The group names in use, in alphabetical order.
    pub fn names(&self, kind: PackKind) -> Vec<String> {
        let mut names: Vec<String> = self.map(kind).values().cloned().collect();
        names.sort_by_key(|n| n.to_lowercase());
        names.dedup();
        names
    }
}

/// A group name as it's kept: trimmed and not too long.
fn clean(name: &str) -> String {
    name.trim().chars().take(MAX_NAME).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_are_kept_per_kind_and_survive_a_save() {
        let dir = tempfile::tempdir().unwrap();
        let mut groups = PackGroups::load(dir.path());
        assert!(groups.names(PackKind::Resource).is_empty());
        groups.set(PackKind::Resource, "a.zip", Some("  PvP "));
        groups.set(PackKind::Resource, "b.zip", Some("Cozy"));
        groups.set(PackKind::Shader, "s.zip", Some("PvP"));
        groups.save(dir.path()).unwrap();
        let back = PackGroups::load(dir.path());
        assert_eq!(back.group_of(PackKind::Resource, "a.zip"), Some("PvP"));
        assert_eq!(back.names(PackKind::Resource), ["Cozy", "PvP"]);
        assert_eq!(back.group_of(PackKind::Shader, "a.zip"), None);
        let mut back = back;
        back.set(PackKind::Resource, "a.zip", Some("   "));
        assert_eq!(back.group_of(PackKind::Resource, "a.zip"), None);
    }
}
