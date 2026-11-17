//! Starred worlds, kept per instance (by folder name) in a small JSON file.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::World;
use crate::Result;
use crate::storage::{load_json, save_json};

/// The starred world folders of one instance.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Favorites {
    #[serde(default)]
    folders: BTreeSet<String>,
}

impl Favorites {
    /// Read `file`; missing or unreadable means none starred (the file is left alone).
    pub fn load(file: &Path) -> Self {
        match load_json::<Self>(file) {
            Ok(found) => found.unwrap_or_default(),
            Err(e) => {
                log::warn!("world favorites {}: {e}", file.display());
                Self::default()
            }
        }
    }

    pub fn contains(&self, folder: &str) -> bool {
        self.folders.contains(folder)
    }

    pub fn len(&self) -> usize {
        self.folders.len()
    }

    pub fn is_empty(&self) -> bool {
        self.folders.is_empty()
    }

    /// Flip the star of `folder` and save; returns whether it is starred now. When saving fails
    /// the change is not kept in memory either.
    pub fn toggle(&mut self, folder: &str, file: &Path) -> Result<bool> {
        let before = self.folders.clone();
        let starred = if self.folders.remove(folder) {
            false
        } else {
            self.folders.insert(folder.to_owned());
            true
        };
        if let Err(e) = save_json(file, self) {
            self.folders = before;
            return Err(e);
        }
        Ok(starred)
    }

    /// Move a star to another folder name (a restored world that got a new name).
    pub fn rename(&mut self, from: &str, to: &str, file: &Path) -> Result<()> {
        if from == to || !self.folders.contains(from) {
            return Ok(());
        }
        let before = self.folders.clone();
        self.folders.remove(from);
        self.folders.insert(to.to_owned());
        if let Err(e) = save_json(file, self) {
            self.folders = before;
            return Err(e);
        }
        Ok(())
    }
}

/// Starred worlds first; each group keeps its order (most recently played first).
pub fn sort_favorites_first(worlds: &mut [World], favorites: &Favorites) {
    worlds.sort_by_key(|w| !favorites.contains(&w.folder));
}

/// Where an instance keeps its favourites, given its own folder.
pub fn favorites_file(instance_dir: &Path) -> PathBuf {
    instance_dir.join("world-favorites.json")
}
