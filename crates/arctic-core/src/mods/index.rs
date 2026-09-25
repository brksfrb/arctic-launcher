//! `mods.json`: which files in the mods folder came from Modrinth.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::InstalledMod;
use crate::Result;
use crate::storage::{load_json, save_json};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct ModIndex {
    #[serde(default)]
    pub mods: Vec<InstalledMod>,
}

impl ModIndex {
    /// Load the index; a missing file is an empty index.
    pub fn load(path: &Path) -> Result<Self> {
        Ok(load_json(path)?.unwrap_or_default())
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        save_json(path, self)
    }

    pub fn by_file(&self, file_name: &str) -> Option<&InstalledMod> {
        self.mods.iter().find(|m| m.file_name == file_name)
    }

    pub fn by_project(&self, project_id: &str) -> Option<&InstalledMod> {
        self.mods.iter().find(|m| m.project_id == project_id)
    }

    /// A copy with `entry` replacing any entry for the same project or file.
    pub fn with(&self, entry: InstalledMod) -> Self {
        let mut mods: Vec<InstalledMod> = self
            .mods
            .iter()
            .filter(|m| m.project_id != entry.project_id && m.file_name != entry.file_name)
            .cloned()
            .collect();
        mods.push(entry);
        Self { mods }
    }

    /// A copy without the entry for `file_name`.
    pub fn without_file(&self, file_name: &str) -> Self {
        Self {
            mods: self
                .mods
                .iter()
                .filter(|m| m.file_name != file_name)
                .cloned()
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(project: &str, file: &str) -> InstalledMod {
        InstalledMod {
            project_id: project.into(),
            version_id: format!("{project}-v"),
            title: project.to_uppercase(),
            version_number: "1.0".into(),
            file_name: file.into(),
            icon_url: None,
            dependency: false,
        }
    }

    #[test]
    fn roundtrip_and_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mods.json");
        assert_eq!(ModIndex::load(&path).unwrap(), ModIndex::default());
        let index = ModIndex::default()
            .with(entry("a", "a.jar"))
            .with(entry("b", "b.jar"));
        index.save(&path).unwrap();
        assert_eq!(ModIndex::load(&path).unwrap(), index);
    }

    #[test]
    fn tolerates_missing_optional_fields() {
        let json = r#"{"mods":[{"project_id":"p","version_id":"v","title":"T",
            "version_number":"1","file_name":"p.jar","icon_url":null}]}"#;
        let index: ModIndex = serde_json::from_str(json).unwrap();
        assert!(!index.mods[0].dependency);
        let empty: ModIndex = serde_json::from_str("{}").unwrap();
        assert!(empty.mods.is_empty());
    }

    #[test]
    fn with_replaces_same_project_and_without_drops_file() {
        let index = ModIndex::default().with(entry("a", "a-1.jar"));
        let updated = index.with(entry("a", "a-2.jar"));
        assert_eq!(updated.mods.len(), 1);
        assert_eq!(updated.by_project("a").unwrap().file_name, "a-2.jar");
        assert!(index.by_file("a-1.jar").is_some(), "original untouched");
        assert!(updated.without_file("a-2.jar").mods.is_empty());
    }
}
