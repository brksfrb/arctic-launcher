//! The built-in Arctic instance, and moving older data over to it.
//!
//! Every profile has exactly one built-in instance, marked `"builtin": true` in its
//! `instance.json`: "Arctic", which always runs the Arctic Client (and Polarium, with the
//! Performance mods) on Fabric and follows the version picked on the Play tab. It can't be
//! removed or copied.
//!
//! Launchers before it had a built-in default in `instances/vanilla`, named "Vanilla", whose
//! "Arctic Client" switch (on unless the player turned it off) quietly ran it on Fabric with the
//! client. [`ensure_default`] moves a profile over once, in place, without touching game files:
//!
//! - old default with the client on: it *is* the Arctic instance now (same folder and id, so its
//!   worlds, settings, playtime, servers and shortcuts stay put); renamed to "Arctic" only if it
//!   still had the stock name;
//! - old default with the client off: it stays as an ordinary Vanilla instance (pinned to the
//!   version last picked on the Play tab, still the selected instance) and a fresh Arctic
//!   instance is added next to it;
//! - nothing yet: a fresh Arctic instance.
//!
//! Each step is a single atomic `instance.json` write and the steps are ordered so that an error
//! part way leaves a state the next start finishes from; once a built-in instance exists nothing
//! runs again.

use super::{ARCTIC_NAME, INSTANCE_FILE, Instance, read};
use crate::settings::Settings;
use crate::storage::{DataDirs, load_json};
use crate::{Error, Result};

/// Folder id of a newly made Arctic instance.
pub const ARCTIC_ID: &str = "arctic";
/// Folder id of the old built-in default ("Vanilla"); a converted Arctic instance keeps it.
pub const LEGACY_DEFAULT_ID: &str = "vanilla";
/// The old default's stock name.
const LEGACY_NAME: &str = "Vanilla";
/// Copy of the old default's `instance.json`, made before it is changed.
const BACKUP_FILE: &str = "instance.pre-arctic.json";

/// What [`ensure_default`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuiltinMigration {
    /// No built-in instance (and no old default): a new Arctic instance was made.
    Created { id: String },
    /// The old default had the Arctic Client on and is the Arctic instance now.
    Converted { id: String, renamed: bool },
    /// The old default was pure vanilla: it stays a Vanilla instance (`vanilla`) and a new
    /// Arctic instance (`id`) was added.
    Added { id: String, vanilla: String },
}

/// The built-in Arctic instance, moving the profile over to it first if needed (once; later
/// calls only read it). Returns what was done, if anything.
pub fn ensure_default(dirs: &DataDirs) -> Result<(Instance, Option<BuiltinMigration>)> {
    if let Some(found) = find_builtin(dirs)? {
        return Ok((found, None));
    }
    let legacy_dir = dirs.instance_dir(LEGACY_DEFAULT_ID);
    // As written: normalizing would already switch the client off on a Vanilla instance.
    let legacy = match load_json::<Instance>(&legacy_dir.join(INSTANCE_FILE))? {
        Some(old) => Some(old),
        // Its game folder without an instance.json: earlier launchers made a stock default
        // for it, so do the same rather than leave its worlds behind.
        None if legacy_dir.join(super::GAME_DIR).is_dir() => Some(Instance {
            id: LEGACY_DEFAULT_ID.into(),
            name: LEGACY_NAME.into(),
            builtin: false,
            ..Instance::arctic_default()
        }),
        None => None,
    };
    let (instance, done) = match legacy {
        Some(old) if old.arctic_mod && old.version.is_none() => convert(dirs, old)?,
        Some(old) => keep_vanilla_and_add(dirs, old)?,
        None => {
            let arctic = create_arctic(dirs)?;
            let id = arctic.id.clone();
            log::info!("instances: made the Arctic instance ({id})");
            (arctic, BuiltinMigration::Created { id })
        }
    };
    Ok((instance, Some(done)))
}

/// The built-in instance as it is on disk, without moving anything over: for reading another
/// profile, say. Before the move, the old default (if any) stands in for it.
pub fn peek_default(dirs: &DataDirs) -> Option<Instance> {
    match find_builtin(dirs) {
        Ok(Some(found)) => Some(found),
        Ok(None) => read(dirs, LEGACY_DEFAULT_ID).ok().flatten(),
        Err(e) => {
            log::warn!("instances: couldn't read the Arctic instance: {e}");
            None
        }
    }
}

/// What to show as the Arctic instance when [`ensure_default`] failed (an error writing, say),
/// without writing anything. Before the move, it stands on the old default's folder, so the
/// player sees (and plays) their own worlds, and saving it later does what the move would have.
/// A pure-vanilla old default isn't taken over: a stand-in in `arctic` is used, which the move
/// adopts when it next runs.
pub fn fallback_default(dirs: &DataDirs) -> Instance {
    if let Ok(Some(found)) = find_builtin(dirs) {
        return found;
    }
    let legacy_dir = dirs.instance_dir(LEGACY_DEFAULT_ID);
    let as_arctic = |old: Instance| {
        Instance {
            id: LEGACY_DEFAULT_ID.into(),
            name: if old.name == LEGACY_NAME {
                ARCTIC_NAME.into()
            } else {
                old.name.clone()
            },
            builtin: true,
            ..old
        }
        .normalized()
    };
    match load_json::<Instance>(&legacy_dir.join(INSTANCE_FILE)) {
        Ok(Some(old)) if old.arctic_mod && old.version.is_none() => as_arctic(old),
        Ok(Some(_)) => Instance::arctic_default(),
        // Unreadable, or only its game folder: it was the default, keep showing its worlds.
        Err(_) => as_arctic(Instance {
            id: LEGACY_DEFAULT_ID.into(),
            name: LEGACY_NAME.into(),
            ..Instance::arctic_default()
        }),
        Ok(None) if legacy_dir.join(super::GAME_DIR).is_dir() => as_arctic(Instance {
            id: LEGACY_DEFAULT_ID.into(),
            name: LEGACY_NAME.into(),
            ..Instance::arctic_default()
        }),
        Ok(None) => Instance::arctic_default(),
    }
}

/// The instance marked built-in. An unreadable `arctic` or `vanilla` is an error (it may be
/// the built-in one, and making another would leave two); other broken folders are skipped.
fn find_builtin(dirs: &DataDirs) -> Result<Option<Instance>> {
    for id in [ARCTIC_ID, LEGACY_DEFAULT_ID] {
        if let Some(found) = read(dirs, id)?
            && found.builtin
        {
            return Ok(Some(Instance {
                id: id.to_owned(),
                ..found
            }));
        }
    }
    // Only when `arctic` was already a player's own instance when the Arctic one was made.
    let Ok(entries) = std::fs::read_dir(dirs.instances()) else {
        return Ok(None);
    };
    let mut ids: Vec<String> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|id| !id.starts_with('.') && id != ARCTIC_ID && id != LEGACY_DEFAULT_ID)
        .collect();
    ids.sort();
    for id in ids {
        match read(dirs, &id) {
            Ok(Some(found)) if found.builtin => return Ok(Some(Instance { id, ..found })),
            Ok(_) => {}
            Err(e) => log::warn!("instances: skipping broken instance {id}: {e}"),
        }
    }
    Ok(None)
}

/// Case 1: the old default ran the Arctic Client, so it becomes the Arctic instance in place.
fn convert(dirs: &DataDirs, old: Instance) -> Result<(Instance, BuiltinMigration)> {
    backup(dirs, LEGACY_DEFAULT_ID);
    // A name the player chose stays.
    let renamed = old.name == LEGACY_NAME;
    let arctic = Instance {
        id: LEGACY_DEFAULT_ID.into(),
        name: if renamed {
            ARCTIC_NAME.into()
        } else {
            old.name.clone()
        },
        builtin: true,
        ..old
    }
    .normalized();
    checkpoint("convert")?;
    arctic.save(dirs)?;
    log::info!(
        "instances: the default instance ({LEGACY_DEFAULT_ID}, \"{}\") had the Arctic Client on and is the Arctic instance now{}",
        arctic.name,
        if renamed {
            ", renamed from \"Vanilla\""
        } else {
            ""
        }
    );
    Ok((
        arctic,
        BuiltinMigration::Converted {
            id: LEGACY_DEFAULT_ID.into(),
            renamed,
        },
    ))
}

/// Case 2: the old default was pure vanilla. It stays (an ordinary Vanilla instance, still the
/// one selected) and a fresh Arctic instance is added. Safe to run again after any failure.
fn keep_vanilla_and_add(dirs: &DataDirs, old: Instance) -> Result<(Instance, BuiltinMigration)> {
    // The Play tab keeps showing Vanilla ("no instance picked" would now mean Arctic).
    let settings = match Settings::load(dirs) {
        Ok(mut settings) => {
            if settings.last_instance.is_none() {
                settings.last_instance = Some(LEGACY_DEFAULT_ID.into());
                checkpoint("settings")?;
                settings.save(dirs)?;
                log::info!(
                    "instances: the Vanilla instance stays the selected one on the Play tab"
                );
            }
            Some(settings)
        }
        Err(e) => {
            log::warn!("instances: settings unreadable, the selected instance is left alone: {e}");
            None
        }
    };
    // It no longer follows the Play tab: pin the version it was playing.
    if old.version.is_none() {
        backup(dirs, LEGACY_DEFAULT_ID);
        let pinned = Instance {
            id: LEGACY_DEFAULT_ID.into(),
            version: settings.and_then(|s| s.last_version),
            builtin: false,
            ..old
        }
        .normalized();
        checkpoint("pin")?;
        pinned.save(dirs)?;
        log::info!(
            "instances: the default instance had the Arctic Client off; kept as the Vanilla instance \"{}\" on {}",
            pinned.name,
            pinned
                .version
                .as_deref()
                .unwrap_or("the Play tab's version")
        );
    }
    let arctic = create_arctic(dirs)?;
    log::info!(
        "instances: added the Arctic instance ({}) next to Vanilla",
        arctic.id
    );
    let id = arctic.id.clone();
    Ok((
        arctic,
        BuiltinMigration::Added {
            id,
            vanilla: LEGACY_DEFAULT_ID.into(),
        },
    ))
}

/// A new Arctic instance in a folder no instance uses (`arctic` unless a player's instance has
/// it). A folder without an `instance.json` is taken as it is (games played on the stand-in
/// Arctic instance while a move failed went there). A folder it made is removed again if
/// saving fails; one that was there is left alone.
fn create_arctic(dirs: &DataDirs) -> Result<Instance> {
    let id = std::iter::once(ARCTIC_ID.to_owned())
        .chain((2..).map(|n| format!("{ARCTIC_ID}-{n}")))
        .find(|id| !dirs.instance_dir(id).join(INSTANCE_FILE).exists())
        .unwrap_or_else(|| ARCTIC_ID.to_owned());
    let existed = dirs.instance_dir(&id).exists();
    let arctic = Instance {
        id,
        ..Instance::arctic_default()
    };
    let dir = dirs.instance_dir(&arctic.id);
    let made = checkpoint("create")
        .and_then(|()| arctic.save(dirs))
        .and_then(|()| {
            let game = arctic.default_game_dir(dirs);
            std::fs::create_dir_all(&game).map_err(|e| Error::io(&game, e))
        });
    if let Err(e) = made {
        if !existed
            && dir.exists()
            && let Err(cleanup) = std::fs::remove_dir_all(&dir)
        {
            log::warn!("instances: couldn't tidy up {}: {cleanup}", dir.display());
        }
        return Err(e);
    }
    Ok(arctic)
}

/// Keep the old default's `instance.json` as it was before the move (first time only).
fn backup(dirs: &DataDirs, id: &str) {
    let dir = dirs.instance_dir(id);
    let (from, to) = (dir.join(INSTANCE_FILE), dir.join(BACKUP_FILE));
    if from.is_file()
        && !to.exists()
        && let Err(e) = std::fs::copy(&from, &to)
    {
        log::warn!("instances: couldn't back up {}: {e}", from.display());
    }
}

#[cfg(test)]
thread_local! {
    /// Test hook: the migration step that fails.
    static FAIL_AT: std::cell::Cell<Option<&'static str>> = const { std::cell::Cell::new(None) };
}

fn checkpoint(step: &'static str) -> Result<()> {
    #[cfg(test)]
    if FAIL_AT.with(|f| f.get()) == Some(step) {
        return Err(Error::Other(format!("injected failure at {step}")));
    }
    let _ = step;
    Ok(())
}

#[cfg(test)]
mod tests;
