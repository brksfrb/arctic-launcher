//! One pattern for destructive clicks: do it at once, say so, and offer Undo on the
//! toast for a few seconds. What was removed waits in a trash folder (mods, packs) or
//! in memory (a skin's PNG) until the toast is gone.

use std::path::PathBuf;

use arctic_core::mods::packs::{self, PackKind, TrashedPack};
use arctic_core::mods::{self, TrashedMod};
use arctic_core::skins::Variant;
use arctic_core::worlds::TrashedWorld;

use crate::app::ArcticApp;
use crate::toasts::Kind;
use crate::{t, tf};
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
    Skin {
        name: String,
        variant: Variant,
        png: Vec<u8>,
    },
    World {
        instance_id: String,
        saves: PathBuf,
        trashed: TrashedWorld,
    },
    /// A server taken off the game's multiplayer list: name, address and the address of
    /// the one that followed it (to put it back in the same place).
    Server {
        game_dir: PathBuf,
        name: String,
        address: String,
        before: Option<String>,
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
                    Ok(()) => self
                        .toasts
                        .push(Kind::Success, t!("tasks.mod_put_back"), ""),
                    Err(e) => self.toasts.push(
                        Kind::Error,
                        t!("tasks.couldnt_put_the_mod_back"),
                        e.to_string(),
                    ),
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
                    Ok(()) => self
                        .toasts
                        .push(Kind::Success, t!("tasks.pack_put_back"), ""),
                    Err(e) => self.toasts.push(
                        Kind::Error,
                        t!("tasks.couldnt_put_the_pack_back"),
                        e.to_string(),
                    ),
                }
                if let Some(instance) = self.instance_by_id(&instance_id).cloned() {
                    self.refresh_packs(&instance);
                }
            }
            Undo::Skin { name, variant, png } => {
                let dir = self.skins_dir();
                match self.skins.library.add(&dir, &name, &png, Some(variant)) {
                    Ok(_) => self
                        .toasts
                        .push(Kind::Success, t!("tasks.skin_put_back"), ""),
                    Err(e) => self.toasts.push(
                        Kind::Error,
                        t!("tasks.couldnt_put_the_skin_back"),
                        e.to_string(),
                    ),
                }
            }
            Undo::World {
                instance_id,
                saves,
                trashed,
            } => self.restore_world(&instance_id, &saves, &trashed),
            Undo::Server {
                game_dir,
                name,
                address,
                before,
            } => {
                let result = arctic_core::servers::add(&game_dir, &name, &address).and_then(|_| {
                    arctic_core::servers::move_before(&game_dir, &address, before.as_deref())
                });
                match result {
                    Ok(_) => self
                        .toasts
                        .push(Kind::Success, tf!("tasks.is_back", name = name), ""),
                    Err(e) => self.toasts.push(
                        Kind::Error,
                        t!("tasks.couldnt_put_the_server_back"),
                        e.to_string(),
                    ),
                }
                self.servers_changed();
            }
        }
    }
}
