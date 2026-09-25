//! Profiles: fully separate launcher "identities" on one machine.
//!
//! Each profile has its own accounts, settings, instances (and therefore
//! worlds, mods and configs) and game logs under `profiles/<id>/`. Game
//! files (versions, libraries, assets, Java runtimes) are content-addressed
//! and shared, so switching profiles never re-downloads anything.
//!
//! Data from before profiles existed is moved into the "Default" profile on
//! first start (a rename, nothing is copied or deleted).

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::auth::now_secs;
use crate::error::IoContext;
use crate::storage::{DataDirs, load_json, save_json};
use crate::{Error, Result};

pub const DEFAULT_PROFILE_ID: &str = "default";
const MAX_NAME_LEN: usize = 32;
/// Removed profiles are moved here instead of being deleted.
const TRASH_DIR: &str = ".trash";
/// Items from the pre-profiles layout that belong to a profile.
const LEGACY_ITEMS: [&str; 4] = ["settings.json", "accounts.json", "instances", "logs"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    /// Accent color shown next to the profile name.
    pub color: [u8; 3],
    /// Unix seconds.
    pub created: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileStore {
    pub profiles: Vec<Profile>,
    pub active: String,
}

/// Colors handed out to new profiles, in order.
const PALETTE: [[u8; 3]; 6] = [
    [0x7d, 0xd3, 0xfc],
    [0x34, 0xd3, 0x99],
    [0xa7, 0x8b, 0xfa],
    [0xf4, 0x72, 0xb6],
    [0xfb, 0xbf, 0x24],
    [0x5e, 0xea, 0xd4],
];

impl ProfileStore {
    /// Load `profiles.json`, creating the Default profile (and migrating
    /// pre-profile data into it) on first run.
    pub fn load_or_init(dirs: &DataDirs) -> Result<Self> {
        if let Some(store) = load_json::<Self>(&dirs.profiles_file())? {
            return Ok(store.repaired());
        }
        let store = Self {
            profiles: vec![Profile {
                id: DEFAULT_PROFILE_ID.into(),
                name: "Default".into(),
                color: PALETTE[0],
                created: now_secs(),
            }],
            active: DEFAULT_PROFILE_ID.into(),
        };
        migrate_legacy(dirs, DEFAULT_PROFILE_ID)?;
        store.save(dirs)?;
        Ok(store)
    }

    pub fn save(&self, dirs: &DataDirs) -> Result<()> {
        save_json(&dirs.profiles_file(), self)
    }

    /// Make sure `active` points at an existing profile.
    fn repaired(mut self) -> Self {
        if self.profiles.is_empty() {
            self.profiles.push(Profile {
                id: DEFAULT_PROFILE_ID.into(),
                name: "Default".into(),
                color: PALETTE[0],
                created: now_secs(),
            });
        }
        if !self.profiles.iter().any(|p| p.id == self.active) {
            self.active = self.profiles[0].id.clone();
        }
        self
    }

    pub fn active(&self) -> &Profile {
        self.profiles
            .iter()
            .find(|p| p.id == self.active)
            .unwrap_or(&self.profiles[0])
    }

    /// Find by id or name (case-insensitive).
    pub fn find(&self, query: &str) -> Option<&Profile> {
        let q = query.trim();
        self.profiles.iter().find(|p| p.id == q).or_else(|| {
            self.profiles
                .iter()
                .find(|p| p.name.eq_ignore_ascii_case(q))
        })
    }

    /// `DataDirs` scoped to the active profile.
    pub fn scoped(&self, dirs: &DataDirs) -> DataDirs {
        dirs.with_profile(&self.active().id)
    }

    /// Add a profile (not activated). Returns its id.
    pub fn create(&mut self, dirs: &DataDirs, name: &str) -> Result<String> {
        let name = validate_name(name)?;
        if self
            .profiles
            .iter()
            .any(|p| p.name.eq_ignore_ascii_case(&name))
        {
            return Err(Error::Other(format!(
                "a profile named '{name}' already exists"
            )));
        }
        let id = self.unique_id(&name);
        let color = PALETTE[self.profiles.len() % PALETTE.len()];
        dirs.with_profile(&id).ensure()?;
        self.profiles.push(Profile {
            id: id.clone(),
            name,
            color,
            created: now_secs(),
        });
        Ok(id)
    }

    pub fn rename(&mut self, id: &str, name: &str) -> Result<()> {
        let name = validate_name(name)?;
        let profile = self
            .profiles
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| Error::Other(format!("no profile '{id}'")))?;
        profile.name = name;
        Ok(())
    }

    pub fn set_color(&mut self, id: &str, color: [u8; 3]) {
        if let Some(p) = self.profiles.iter_mut().find(|p| p.id == id) {
            p.color = color;
        }
    }

    pub fn set_active(&mut self, id: &str) -> bool {
        let exists = self.profiles.iter().any(|p| p.id == id);
        if exists {
            self.active = id.to_owned();
        }
        exists
    }

    /// Remove a profile. Its folder (worlds included) is moved to
    /// `profiles/.trash/`, never deleted. The last profile and the active
    /// one cannot be removed.
    pub fn remove(&mut self, dirs: &DataDirs, id: &str) -> Result<()> {
        if self.profiles.len() <= 1 {
            return Err(Error::Other("the last profile can't be removed".into()));
        }
        if self.active == id {
            return Err(Error::Other(
                "switch to another profile before removing this one".into(),
            ));
        }
        let index = self
            .profiles
            .iter()
            .position(|p| p.id == id)
            .ok_or_else(|| Error::Other(format!("no profile '{id}'")))?;
        let folder = dirs.profiles_dir().join(id);
        if folder.exists() {
            let trash = dirs.profiles_dir().join(TRASH_DIR);
            fs::create_dir_all(&trash).at(&trash)?;
            let dest = trash.join(format!("{id}-{}", now_secs()));
            fs::rename(&folder, &dest).at(&dest)?;
        }
        self.profiles.remove(index);
        Ok(())
    }

    fn unique_id(&self, name: &str) -> String {
        let base: String = name
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect::<String>()
            .split('-')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-");
        let base = if base.is_empty() || base == TRASH_DIR.trim_start_matches('.') {
            "profile".to_owned()
        } else {
            base
        };
        let taken = |id: &str| self.profiles.iter().any(|p| p.id == id);
        if !taken(&base) {
            return base;
        }
        (2..)
            .map(|n| format!("{base}-{n}"))
            .find(|id| !taken(id))
            .unwrap_or(base)
    }
}

fn validate_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > MAX_NAME_LEN {
        return Err(Error::Other(format!(
            "profile names must be 1-{MAX_NAME_LEN} characters"
        )));
    }
    Ok(name.to_owned())
}

/// Move pre-profile data (`settings.json`, `accounts.json`, `instances/`,
/// `logs/` game logs) from the root into `profiles/<id>/`.
fn migrate_legacy(dirs: &DataDirs, id: &str) -> Result<()> {
    let target = dirs.with_profile(id);
    let dest_root = target.profile_root().to_path_buf();
    fs::create_dir_all(&dest_root).at(&dest_root)?;
    for item in LEGACY_ITEMS {
        let from = dirs.root().join(item);
        if !from.exists() {
            continue;
        }
        if item == "logs" {
            move_game_logs(&from, &dest_root.join("logs"))?;
            continue;
        }
        let to = dest_root.join(item);
        if !to.exists() {
            log::info!("migrating {} into profile '{id}'", from.display());
            fs::rename(&from, &to).at(&to)?;
        }
    }
    Ok(())
}

/// Game logs move with the profile; `launcher.log` stays launcher-wide.
fn move_game_logs(from: &Path, to: &Path) -> Result<()> {
    let Ok(entries) = fs::read_dir(from) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        if name.to_string_lossy().starts_with("game-") {
            fs::create_dir_all(to).at(to)?;
            let dest = to.join(&name);
            fs::rename(entry.path(), &dest).at(&dest)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dirs() -> (tempfile::TempDir, DataDirs) {
        let dir = tempfile::tempdir().unwrap();
        let dirs = DataDirs::new(dir.path());
        (dir, dirs)
    }

    #[test]
    fn first_run_creates_default_and_migrates_legacy_data() {
        let (_t, dirs) = dirs();
        fs::write(dirs.root().join("accounts.json"), "{}").unwrap();
        fs::create_dir_all(dirs.root().join("instances/vanilla/minecraft/saves/World")).unwrap();
        fs::create_dir_all(dirs.root().join("logs")).unwrap();
        fs::write(dirs.root().join("logs/game-vanilla.log"), "x").unwrap();
        fs::write(dirs.root().join("logs/launcher.log"), "y").unwrap();

        let store = ProfileStore::load_or_init(&dirs).unwrap();
        assert_eq!(store.active().id, DEFAULT_PROFILE_ID);
        let scoped = store.scoped(&dirs);
        assert!(scoped.accounts_file().is_file());
        assert!(
            scoped
                .instance_dir("vanilla")
                .join("minecraft/saves/World")
                .is_dir()
        );
        assert!(scoped.logs().join("game-vanilla.log").is_file());
        assert!(dirs.launcher_logs().join("launcher.log").is_file());
        assert!(!dirs.root().join("accounts.json").exists());
        // Second load reads the saved file and doesn't migrate again.
        assert_eq!(ProfileStore::load_or_init(&dirs).unwrap(), store);
    }

    #[test]
    fn create_rename_switch_remove() {
        let (_t, dirs) = dirs();
        let mut store = ProfileStore::load_or_init(&dirs).unwrap();
        let id = store.create(&dirs, "Speedrun Practice!").unwrap();
        assert_eq!(id, "speedrun-practice");
        assert!(store.create(&dirs, "speedrun practice!").is_err());
        assert_eq!(
            store.create(&dirs, "Speedrun  Practice").unwrap(),
            "speedrun-practice-2"
        );
        store.rename(&id, "Speedruns").unwrap();
        assert_eq!(store.find("speedruns").unwrap().id, id);

        assert!(store.set_active(&id));
        assert!(
            store.remove(&dirs, &id).is_err(),
            "active profile can't be removed"
        );
        store.set_active(DEFAULT_PROFILE_ID);
        fs::write(
            dirs.with_profile(&id).profile_root().join("keep.txt"),
            "world",
        )
        .unwrap();
        store.remove(&dirs, &id).unwrap();
        let trash = fs::read_dir(dirs.profiles_dir().join(TRASH_DIR))
            .unwrap()
            .count();
        assert_eq!(trash, 1);
        assert!(store.find(&id).is_none());
    }

    #[test]
    fn last_profile_and_bad_names_are_rejected() {
        let (_t, dirs) = dirs();
        let mut store = ProfileStore::load_or_init(&dirs).unwrap();
        assert!(store.remove(&dirs, DEFAULT_PROFILE_ID).is_err());
        assert!(store.create(&dirs, "   ").is_err());
        assert!(store.create(&dirs, &"x".repeat(40)).is_err());
        assert_eq!(store.create(&dirs, "!!!").unwrap(), "profile");
    }

    #[test]
    fn dangling_active_is_repaired() {
        let store = ProfileStore {
            profiles: vec![],
            active: "gone".into(),
        }
        .repaired();
        assert_eq!(store.active().id, DEFAULT_PROFILE_ID);
    }
}
