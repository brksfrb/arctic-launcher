//! A start whose performance-mod work found nothing to change, remembered so the next one
//! with the same inputs can skip that work.
//!
//! The work (the Modrinth set and its dependencies, jars for another game version, Polarium,
//! the cut-down Fabric API) decides everything from: the mods folder and the folder the whole
//! Fabric API waits in, its own records, the instance's switches, the game version, and the
//! launcher itself (its mod list and the Polarium jars it carries). With all of those the same
//! as when a run changed nothing, running it again changes nothing either. The one thing that
//! changes on its own is the weekly look at Modrinth, so a record ends when that is due.
//!
//! Jars count as the same by name, size and modification time (to the nanosecond), which is
//! what the launcher already goes by for the Fabric API choice; the small records, and
//! Polarium's jar (which every start compares with the launcher's own), by content.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

use super::{RECHECK_SECS, State, index_path, pending_path, state_path};
use crate::instances::Instance;
use crate::storage::{load_json, save_json};

/// Bump when the record or what goes into it changes.
const FORMAT: u32 = 1;

#[derive(Serialize, Deserialize, PartialEq, Eq)]
struct Record {
    format: u32,
    inputs: String,
    /// Unix seconds from which Modrinth is asked again (`None`: nothing installed that is).
    recheck_at: Option<u64>,
}

/// What a run saw before it started.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Inputs {
    digest: String,
    recheck_at: Option<u64>,
}

pub(super) struct Settled<'a> {
    game_dir: &'a Path,
    game_version: &'a str,
    instance: &'a Instance,
    launcher_owned_vanilla: bool,
}

impl<'a> Settled<'a> {
    pub(super) fn new(
        game_dir: &'a Path,
        game_version: &'a str,
        instance: &'a Instance,
        launcher_owned_vanilla: bool,
    ) -> Self {
        Self {
            game_dir,
            game_version,
            instance,
            launcher_owned_vanilla,
        }
    }

    fn path(&self) -> PathBuf {
        self.game_dir
            .join(".arctic")
            .join("performance-settled.json")
    }

    /// Everything the work reads, or `None` when some of it can't be told (it then runs).
    pub(super) fn inputs(&self) -> Option<Inputs> {
        let mods_dir = self.game_dir.join("mods");
        let mut text = format!(
            "{FORMAT}\n{}\n{}\n{:?}\n{}\n{}\n{}\n{:?}\n{}\n",
            launcher()?,
            self.game_version,
            self.instance.loader,
            self.instance.performance,
            self.instance.shaders,
            self.launcher_owned_vanilla,
            self.instance.extras,
            self.instance.arctic_mod,
        );
        listing(&mods_dir, &mut text)?;
        listing(&super::super::slim_api::stash_dir(&mods_dir), &mut text)?;
        let state = state_path(self.game_dir);
        // Polarium's jar is compared with the launcher's own byte for byte at every start.
        let polarium = mods_dir.join(super::super::polarium::FILE_NAME);
        for record in [&index_path(self.game_dir), &state, &polarium] {
            content(record, &mut text)?;
        }
        text.push_str(&format!("{}\n", pending_path(self.game_dir).exists()));
        let recheck_at = match load_json::<State>(&state).ok()? {
            // A newer build is waiting: the next start installs it.
            Some(state) if state.update_waiting => return None,
            Some(state) => Some(state.checked.saturating_add(RECHECK_SECS)),
            None => None,
        };
        Some(Inputs {
            digest: hex::encode(Sha1::digest(text.as_bytes())),
            recheck_at,
        })
    }

    /// Whether a run that started from exactly `inputs` changed nothing, and Modrinth is not
    /// due to be asked again.
    pub(super) fn holds(&self, inputs: Option<&Inputs>) -> bool {
        let Some(inputs) = inputs else {
            return false;
        };
        let Ok(Some(record)) = load_json::<Record>(&self.path()) else {
            return false;
        };
        record.format == FORMAT
            && record.inputs == inputs.digest
            && record.recheck_at == inputs.recheck_at
            && inputs
                .recheck_at
                .is_none_or(|at| crate::auth::now_secs() < at)
    }

    /// After a run without errors that started from `before`: remember it if it left
    /// everything as it was. A run that changed something is not remembered; the one after
    /// it, which then finds nothing to do, is.
    pub(super) fn remember(&self, before: Option<Inputs>) {
        let path = self.path();
        let unchanged = before.filter(|before| self.inputs().as_ref() == Some(before));
        let Some(inputs) = unchanged else {
            let _ = fs::remove_file(&path);
            return;
        };
        let record = Record {
            format: FORMAT,
            inputs: inputs.digest,
            recheck_at: inputs.recheck_at,
        };
        if let Err(e) = save_json(&path, &record) {
            log::debug!("performance mods: couldn't note that nothing changed: {e}");
        }
    }
}

/// This launcher build: another one may choose other mods or carry other Polarium jars.
fn launcher() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let meta = fs::metadata(&exe).ok()?;
    Some(format!(
        "{} {} {}",
        crate::APP_VERSION,
        meta.len(),
        nanos(&meta)?
    ))
}

fn nanos(meta: &fs::Metadata) -> Option<u128> {
    let modified = meta.modified().ok()?;
    Some(
        modified
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_nanos(),
    )
}

/// Every entry of a folder (a missing folder is an empty one), sorted: name, kind, size, time.
/// Files are looked at themselves (through a link, as the work reads them; and not as the
/// folder's listing has them, which on Windows can be behind for a file with another name).
fn listing(dir: &Path, out: &mut String) -> Option<()> {
    let mut lines = Vec::new();
    match fs::read_dir(dir) {
        Ok(entries) => {
            for entry in entries {
                let entry = entry.ok()?;
                // A link to nowhere, a device: can't be told, so the work runs.
                let meta = fs::metadata(entry.path()).ok()?;
                let name = entry.file_name();
                let name = name.to_str()?;
                lines.push(if meta.is_file() {
                    format!("{name}\0{}\0{}", meta.len(), nanos(&meta)?)
                } else if meta.is_dir() {
                    // The work reads no folder inside these.
                    format!("{name}\0folder")
                } else {
                    return None;
                });
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return None,
    }
    lines.sort_unstable();
    out.push_str(&format!("{}\n{}\n", lines.len(), lines.join("\n")));
    Some(())
}

/// A file's content (a missing one is told apart from an empty one).
fn content(file: &Path, out: &mut String) -> Option<()> {
    match fs::read(file) {
        Ok(bytes) => out.push_str(&format!("{}\n", hex::encode(Sha1::digest(&bytes)))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => out.push_str("none\n"),
        Err(_) => return None,
    }
    Some(())
}
