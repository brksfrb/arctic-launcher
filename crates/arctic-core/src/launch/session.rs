//! Reserve a game directory for each watched process. The first copy uses the
//! instance's folder. Additional copies have private mods, configs, logs and
//! saves, so they can run other versions without touching files in use.

use std::fs::{self, File, TryLockError};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::Result;
use crate::error::IoContext;
use crate::instances::Instance;
use crate::shared::{self, SharedSettings};
use crate::storage::DataDirs;

const LOCK: &str = ".arctic-launch.lock";
const SNAPSHOT: &[&str] = &[
    "mods",
    "config",
    "resourcepacks",
    "shaderpacks",
    "texturepacks",
    "servers.dat",
    "options.txt",
    "optionsof.txt",
    "optionsshaders.txt",
    "arctic-performance.json",
    "arctic-performance-state.json",
    "arctic-performance-forge.json",
    ".arctic-shared-options-projection.json",
];

pub(super) struct GameSession {
    dirs: DataDirs,
    primary: PathBuf,
    game_dir: PathBuf,
    shared: SharedSettings,
    reservation: File,
}

impl GameSession {
    pub(super) fn reserve(
        dirs: &DataDirs,
        instance: &Instance,
        shared: SharedSettings,
        preferred_copy: u32,
    ) -> Result<(Self, u32)> {
        let _sync = shared::lock(dirs)?;
        let primary = instance.game_dir(dirs);
        let copies = dirs.instance_dir(&instance.id).join("copies");
        for copy in preferred_copy.. {
            let game_dir = if copy == 0 {
                primary.clone()
            } else {
                copies.join(copy.to_string())
            };
            if crate::sharing::client::in_use(&game_dir) {
                continue;
            }
            let Some(reservation) = try_reserve_dir(&game_dir)? else {
                continue;
            };
            if copy != 0 {
                snapshot(&primary, &game_dir)?;
            }
            let shared = if instance.own_settings {
                SharedSettings {
                    game: false,
                    client: false,
                    servers: false,
                }
            } else {
                shared
            };
            return Ok((
                Self {
                    dirs: dirs.clone(),
                    primary,
                    game_dir,
                    shared,
                    reservation,
                },
                copy,
            ));
        }
        unreachable!("the number of live game copies cannot exhaust u32")
    }

    pub(super) fn game_dir(&self) -> &Path {
        &self.game_dir
    }

    /// A running primary will save its own options later. Once it has exited,
    /// each finishing copy can safely publish its settings in close order.
    pub(super) fn finish(self) -> Result<()> {
        let _sync = shared::lock(&self.dirs)?;
        if self.game_dir != self.primary
            && let Some(_primary_reservation) = try_reserve_dir(&self.primary)?
            && !crate::sharing::client::in_use(&self.primary)
        {
            shared::save_copy(&self.game_dir, &self.primary, self.shared)?;
        }
        drop(self.reservation);
        shared::sync_locked(&self.dirs, self.shared, None)
    }
}

pub(crate) fn try_reserve_dir(game_dir: &Path) -> Result<Option<File>> {
    let path = game_dir.join(LOCK);
    fs::create_dir_all(game_dir).at(game_dir)?;
    let reservation = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .at(&path)?;
    match reservation.try_lock() {
        Ok(()) => Ok(Some(reservation)),
        Err(TryLockError::WouldBlock) => Ok(None),
        Err(TryLockError::Error(e)) => Err(crate::Error::io(&path, e)),
    }
}

pub(crate) fn in_use(game_dir: &Path) -> bool {
    let path = game_dir.join(LOCK);
    match File::options().read(true).write(true).open(path) {
        Ok(file) => file.try_lock().is_err(),
        Err(e) => e.kind() != ErrorKind::NotFound,
    }
}

fn snapshot(primary: &Path, game_dir: &Path) -> Result<()> {
    let stage = tempfile::tempdir_in(game_dir).at(game_dir)?;
    for name in SNAPSHOT {
        let from = primary.join(name);
        let to = stage.path().join(name);
        if !from.exists() {
            continue;
        }
        if *name == "config" {
            snapshot_config(&from, &to)?;
        } else {
            crate::instances::copy_entry(&from, &to)?;
        }
    }
    for name in SNAPSHOT {
        let to = game_dir.join(name);
        match fs::symlink_metadata(&to) {
            Ok(meta) if meta.is_dir() => fs::remove_dir_all(&to).at(&to)?,
            Ok(_) => fs::remove_file(&to).at(&to)?,
            Err(e) if e.kind() == ErrorKind::NotFound => {}
            Err(e) => return Err(crate::Error::io(&to, e)),
        }
        let staged = stage.path().join(name);
        if staged.exists() {
            fs::rename(staged, &to).at(&to)?;
        }
    }
    Ok(())
}

fn snapshot_config(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to).at(to)?;
    for entry in fs::read_dir(from).at(from)? {
        let entry = entry.at(from)?;
        let name = entry.file_name();
        let name_text = name.to_string_lossy();
        if name_text == "arctic.lock" || name_text.starts_with("arctic-session.json") {
            continue;
        }
        match crate::instances::copy_entry(&entry.path(), &to.join(name)) {
            Err(crate::Error::Io { source, .. })
                if source.kind() == ErrorKind::NotFound
                    || (cfg!(windows) && matches!(source.raw_os_error(), Some(32 | 33))) => {}
            result => result?,
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_isolate_logs_and_mods_and_the_last_close_keeps_its_options() {
        let temp = tempfile::tempdir().unwrap();
        let dirs = DataDirs::new(temp.path());
        let instance = crate::instances::load_default(&dirs).unwrap();
        let shared = SharedSettings::default();
        let reserve = || GameSession::reserve(&dirs, &instance, shared, 0).unwrap();
        let primary_dir = instance.game_dir(&dirs);
        let write_options = |dir: &Path, volume: &str| {
            fs::write(
                dir.join("options.txt"),
                format!("soundCategory_master:{volume}\n"),
            )
            .unwrap();
        };
        let read_options = || fs::read_to_string(primary_dir.join("options.txt")).unwrap();

        let (primary, slot) = reserve();
        assert_eq!(slot, 0);
        fs::create_dir_all(primary_dir.join("mods")).unwrap();
        fs::create_dir_all(primary_dir.join("logs")).unwrap();
        fs::create_dir_all(primary_dir.join("saves")).unwrap();
        fs::write(primary_dir.join("mods/performance.jar"), b"modern").unwrap();
        fs::write(primary_dir.join("logs/latest.log"), b"primary only").unwrap();
        write_options(&primary_dir, "0.5");

        let (copy, slot) = reserve();
        assert_eq!(slot, 1);
        assert!(!copy.game_dir().join("logs/latest.log").exists());
        assert!(!copy.game_dir().join("saves").exists());
        let copy_mod = copy.game_dir().join("mods/performance.jar");
        assert_eq!(fs::read(&copy_mod).unwrap(), b"modern");
        fs::write(&copy_mod, b"legacy").unwrap();
        assert_eq!(
            fs::read(primary_dir.join("mods/performance.jar")).unwrap(),
            b"modern"
        );
        write_options(copy.game_dir(), "0.2");
        write_options(&primary_dir, "0.8");
        primary.finish().unwrap();
        assert_eq!(read_options(), "soundCategory_master:0.8\n");
        copy.finish().unwrap();
        assert_eq!(read_options(), "soundCategory_master:0.2\n");

        let (primary, _) = reserve();
        let (copy, _) = reserve();
        write_options(copy.game_dir(), "0.1");
        copy.finish().unwrap();
        assert_eq!(read_options(), "soundCategory_master:0.2\n");
        write_options(&primary_dir, "0.7");
        primary.finish().unwrap();
        assert_eq!(read_options(), "soundCategory_master:0.7\n");
    }

    #[test]
    fn truncated_options_from_a_finished_copy_do_not_replace_the_instance_file() {
        let temp = tempfile::tempdir().unwrap();
        let dirs = DataDirs::new(temp.path());
        let instance = crate::instances::load_default(&dirs).unwrap();
        let shared = SharedSettings::default();
        let (primary, _) = GameSession::reserve(&dirs, &instance, shared, 0).unwrap();
        let options = primary.game_dir().join("options.txt");
        let original = b"soundCategory_master:0.5\n";
        fs::write(&options, original).unwrap();
        let (copy, _) = GameSession::reserve(&dirs, &instance, shared, 1).unwrap();
        fs::write(
            copy.game_dir().join("options.txt"),
            b"soundCategory_master:",
        )
        .unwrap();
        primary.finish().unwrap();
        copy.finish().unwrap();
        assert_eq!(fs::read(&options).unwrap(), original);
    }

    #[test]
    fn finished_copy_keeps_the_primary_pack_selection() {
        let temp = tempfile::tempdir().unwrap();
        let dirs = DataDirs::new(temp.path());
        let instance = crate::instances::load_default(&dirs).unwrap();
        let shared = SharedSettings::default();
        let (primary, _) = GameSession::reserve(&dirs, &instance, shared, 0).unwrap();
        let (copy, _) = GameSession::reserve(&dirs, &instance, shared, 1).unwrap();
        let options = primary.game_dir().join("options.txt");
        fs::write(
            &options,
            "resourcePacks:[\"vanilla\",\"file/primary.zip\"]\nincompatibleResourcePacks:[\"file/primary.zip\"]\nsoundCategory_master:0.5\n",
        )
        .unwrap();
        fs::write(
            copy.game_dir().join("options.txt"),
            "resourcePacks:[\"only-in-copy.zip\"]\nincompatibleResourcePacks:[]\nsoundCategory_master:0.2\n",
        )
        .unwrap();
        primary.finish().unwrap();
        copy.finish().unwrap();
        assert_eq!(
            fs::read_to_string(options).unwrap(),
            "soundCategory_master:0.2\nresourcePacks:[\"vanilla\",\"file/primary.zip\"]\nincompatibleResourcePacks:[\"file/primary.zip\"]\n"
        );
    }
}
