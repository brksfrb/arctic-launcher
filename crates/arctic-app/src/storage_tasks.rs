//! Background work for choosing folders: where the launcher keeps its files (Settings → Storage)
//! and where an instance keeps its game (instance settings).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use arctic_core::instances::{self, FolderChange, Instance};
use arctic_core::storage::location::{self, Target};

use crate::tasks::{Event, Tasks};

/// How often a copy reports progress.
const PROGRESS_EVERY: Duration = Duration::from_millis(100);

pub enum StorageEvent {
    /// A folder was picked for the launcher's files and looked at.
    Checked(Checked),
    /// Copying: bytes done, bytes in all.
    Progress(u64, u64),
    /// The launcher's files were moved (restart to use them) or not.
    Moved(Result<(), String>),
    /// A folder was picked for an instance's game.
    InstancePicked(String, PathBuf, bool),
    /// An instance now uses its new game folder, or couldn't.
    InstanceMoved(String, Result<Instance, String>),
}

/// A folder offered for the launcher's files.
pub struct Checked {
    pub to: PathBuf,
    pub target: Target,
    /// Why it can't be used, if it can't.
    pub problem: Option<String>,
    /// Bytes a move copies.
    pub size: u64,
}

impl Tasks {
    /// Ask for a folder for the launcher's files, then look at it.
    pub fn storage_pick(&self, from: PathBuf) {
        self.run(move |t| {
            let picked = rfd::FileDialog::new()
                .set_title("Where should Arctic keep its files?")
                .pick_folder();
            t.note_if_picker_missing(&picked);
            if let Some(to) = picked {
                t.storage_check(from, to);
            }
        });
    }

    /// Look at `to` as the new home of the files now in `from`.
    pub fn storage_check(&self, from: PathBuf, to: PathBuf) {
        self.run(move |t| {
            let checked = Checked {
                problem: location::check_move(&from, &to),
                target: location::inspect(&to),
                size: location::size_to_move(&from),
                to,
            };
            t.send(Event::Storage(StorageEvent::Checked(checked)));
        });
    }

    /// Copy the launcher's files to `to` and make it the data folder.
    pub fn storage_move(&self, default: PathBuf, from: PathBuf, to: PathBuf) {
        self.run(move |t| {
            let last = std::sync::Mutex::new(None::<Instant>);
            let progress = |done: u64, total: u64| {
                let mut last = last.lock().unwrap_or_else(|e| e.into_inner());
                if last.is_none_or(|at| at.elapsed() >= PROGRESS_EVERY) || done == total {
                    *last = Some(Instant::now());
                    t.send(Event::Storage(StorageEvent::Progress(done, total)));
                }
            };
            let result =
                location::move_data(&default, &from, &to, &progress).map_err(|e| e.to_string());
            t.send(Event::Storage(StorageEvent::Moved(result)));
        });
    }

    /// Ask for a folder for an instance's game.
    pub fn instance_folder_pick(&self, id: String) {
        self.run(move |t| {
            let picked = rfd::FileDialog::new()
                .set_title("Game folder for this instance")
                .pick_folder();
            t.note_if_picker_missing(&picked);
            if let Some(folder) = picked {
                let empty = instances::folder_is_empty(&folder);
                t.send(Event::Storage(StorageEvent::InstancePicked(
                    id, folder, empty,
                )));
            }
        });
    }

    /// Switch an instance to `folder` (`None`: its own folder again).
    pub fn instance_folder_set(
        &self,
        instance: Instance,
        folder: Option<PathBuf>,
        change: FolderChange,
    ) {
        self.run(move |t| {
            let id = instance.id.clone();
            let result = instances::set_game_folder(
                t.dirs(),
                &instance,
                folder.as_deref(),
                change,
                &|_, _| {},
            )
            .map_err(|e| e.to_string());
            t.send(Event::Storage(StorageEvent::InstanceMoved(id, result)));
        });
    }
}
