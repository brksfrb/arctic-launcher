//! One pattern for destructive clicks: do it at once, say so, and offer Undo on the
//! toast for a few seconds. What was removed waits in a trash folder (mods, packs) or
//! in memory (a skin's PNG) until the toast is gone.

use std::path::PathBuf;

use arctic_core::mods::packs::{self, PackKind, TrashedPack};
use arctic_core::mods::{self, TrashedMod};

use crate::app::ArcticApp;
use crate::toasts::Kind;

/// What "Undo" on a toast puts back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Undo {
    Mod {
        instance_id: String,
        index: PathBuf,
        trashed: TrashedMod,
    },
    Pack {
        instance_id: String,
        kind: PackKind,
        game_dir: PathBuf,
        trashed: TrashedPack,
    },
}

impl ArcticApp {
    /// The player clicked Undo on a toast.
    pub(crate) fn undo(&mut self, undo: Undo) {
        match undo {
            Undo::Mod {
                instance_id,
                index,
                trashed,
            } => {
                match mods::restore(&index, &trashed) {
                    Ok(()) => self.toasts.push(Kind::Success, "Mod put back", ""),
                    Err(e) => {
                        self.toasts
                            .push(Kind::Error, "Couldn't put the mod back", e.to_string())
                    }
                }
                self.refresh_mods(&instance_id);
            }
            Undo::Pack {
                instance_id,
                kind,
                game_dir,
                trashed,
            } => {
                match packs::restore(kind, &game_dir, &trashed) {
                    Ok(()) => self.toasts.push(Kind::Success, "Pack put back", ""),
                    Err(e) => {
                        self.toasts
                            .push(Kind::Error, "Couldn't put the pack back", e.to_string())
                    }
                }
                if let Some(instance) = self.instance_by_id(&instance_id).cloned() {
                    self.refresh_packs(&instance);
                }
            }
        }
    }
}
