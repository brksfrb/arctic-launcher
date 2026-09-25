//! Skins tab: a 3D preview, the account's current skin and capes, and a
//! local library of skins that can be applied with one click.

mod model;
mod panels;

use std::collections::HashMap;
use std::path::PathBuf;

use arctic_core::skins::{self, Library, SkinEntry, Variant};
use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};

use crate::app::ArcticApp;
use crate::skin_tasks::{AccountSkin, ArcticCapes, SkinChange};
use crate::tasks::Event;
use crate::toasts::Kind;

/// What the preview shows.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Selection {
    /// The account's active skin.
    #[default]
    Current,
    Library(String),
}

/// A skin uploaded to the GPU.
pub struct SkinTexture {
    pub handle: TextureHandle,
    /// Has the second layer on body and limbs (not a legacy 64×32 skin).
    pub overlay: bool,
    pub guessed: Variant,
}

#[derive(Default)]
pub struct SkinsUi {
    loaded_for: Option<PathBuf>,
    pub library: Library,
    textures: HashMap<String, SkinTexture>,
    /// Keys whose PNG couldn't be read or decoded (not retried every frame).
    failed: std::collections::HashSet<String>,
    pub selection: Selection,
    /// Account id → its skin state (fetched when the tab opens).
    pub account: HashMap<String, Result<AccountSkin, String>>,
    pub busy: bool,
    pub player_name: String,
    pub looking_up: bool,
    pub yaw: f32,
    pub pitch: f32,
    pub renaming: Option<(String, String)>,
    /// Account id → Arctic capes (fetched with the account's skin).
    pub arctic: HashMap<String, Result<ArcticCapes, String>>,
    pub arctic_busy: bool,
}

impl ArcticApp {
    fn skins_dir(&self) -> PathBuf {
        Library::dir(self.dirs.profile_root())
    }

    /// Load the library the first time the tab opens (per profile).
    fn ensure_skin_library(&mut self) {
        let dir = self.skins_dir();
        if self.skins.loaded_for.as_ref() == Some(&dir) {
            return;
        }
        self.skins = SkinsUi {
            yaw: 0.5,
            pitch: 0.12,
            ..SkinsUi::default()
        };
        match Library::load(&dir) {
            Ok(lib) => self.skins.library = lib,
            Err(e) => self.toasts.push(
                Kind::Error,
                "Could not read your skin library",
                e.to_string(),
            ),
        }
        self.skins.loaded_for = Some(dir);
    }

    pub(crate) fn skins_tab(&mut self, ui: &mut egui::Ui) {
        self.ensure_skin_library();
        self.request_account_skin(false);
        self.skins_page(ui);
        self.accept_dropped_skins(ui.ctx());
    }

    /// Fetch the active account's skin once (or again with `force`).
    fn request_account_skin(&mut self, force: bool) {
        let Some(account) = self.accounts.active().cloned() else {
            return;
        };
        if !account.is_microsoft() || self.skins.busy {
            return;
        }
        if !force && self.skins.account.contains_key(&account.id) {
            return;
        }
        self.skins.busy = true;
        self.tasks.skin_account(account.clone(), SkinChange::None);
        if !self.skins.arctic.contains_key(&account.id) && !self.skins.arctic_busy {
            self.skins.arctic_busy = true;
            self.tasks.arctic_capes(account, None, None);
        }
    }

    /// Equip an Arctic cape (or none) for the active account.
    fn equip_arctic_cape(&mut self, cape: Option<String>) {
        let Some(account) = self.accounts.active().cloned() else {
            return;
        };
        let token = match self.skins.arctic.get(&account.id) {
            Some(Ok(state)) => Some(state.token.clone()),
            _ => None,
        };
        self.skins.arctic_busy = true;
        self.tasks.arctic_capes(account, token, Some(cape));
    }

    fn change_account_skin(&mut self, change: SkinChange) {
        let Some(account) = self.accounts.active().cloned() else {
            return;
        };
        self.skins.busy = true;
        self.tasks.skin_account(account, change);
    }

    /// Texture for a library entry or the current skin, uploading on first use.
    pub(crate) fn skin_texture(&mut self, ctx: &egui::Context, key: &str) -> Option<&SkinTexture> {
        if !self.skins.textures.contains_key(key) {
            if self.skins.failed.contains(key) {
                return None;
            }
            let png = self.skin_png(key)?;
            let Some(texture) = upload(ctx, key, &png) else {
                self.skins.failed.insert(key.to_owned());
                return None;
            };
            self.skins.textures.insert(key.to_owned(), texture);
        }
        self.skins.textures.get(key)
    }

    fn skin_png(&self, key: &str) -> Option<Vec<u8>> {
        if let Some(id) = key.strip_prefix("lib:") {
            return Library::read_png(&self.skins_dir(), id).ok();
        }
        let account = self.accounts.active()?;
        if let Some(id) = key.strip_prefix("arctic:") {
            return match self.skins.arctic.get(&account.id)? {
                Ok(state) => state
                    .textures
                    .iter()
                    .find(|(c, _)| c == id)
                    .map(|(_, png)| png.clone()),
                Err(_) => None,
            };
        }
        match self.skins.account.get(&account.id)? {
            Ok(state) if key == "current" => state.skin_png.clone(),
            Ok(state) => {
                let cape = key.strip_prefix("cape:")?;
                state
                    .capes
                    .iter()
                    .find(|(id, _)| id == cape)
                    .map(|(_, png)| png.clone())
            }
            Err(_) => None,
        }
    }

    fn add_skin(&mut self, name: &str, png: &[u8], variant: Option<Variant>) {
        let dir = self.skins_dir();
        match self.skins.library.add(&dir, name, png, variant) {
            Ok(entry) => {
                self.toasts
                    .push(Kind::Success, format!("Added {}", entry.name), "");
                self.skins.selection = Selection::Library(entry.id);
            }
            Err(e) => self
                .toasts
                .push(Kind::Error, "Could not add skin", e.to_string()),
        }
    }

    fn remove_skin(&mut self, id: &str) {
        let dir = self.skins_dir();
        if let Err(e) = self.skins.library.remove(&dir, id) {
            self.toasts
                .push(Kind::Error, "Could not delete skin", e.to_string());
        }
        self.skins.textures.remove(&format!("lib:{id}"));
        self.skins.selection = Selection::Current;
    }

    fn update_skin(&mut self, id: &str, change: impl FnOnce(&mut SkinEntry)) {
        let dir = self.skins_dir();
        if let Err(e) = self.skins.library.update(&dir, id, change) {
            self.toasts
                .push(Kind::Error, "Could not save skin", e.to_string());
        }
    }

    fn apply_library_skin(&mut self, entry: &SkinEntry) {
        match Library::read_png(&self.skins_dir(), &entry.id) {
            Ok(png) => self.change_account_skin(SkinChange::Upload(entry.variant, png)),
            Err(e) => self
                .toasts
                .push(Kind::Error, "Could not read skin", e.to_string()),
        }
    }

    /// PNG files dropped onto the window while the tab is open.
    fn accept_dropped_skins(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        for file in dropped {
            let bytes = match file.bytes() {
                Ok(b) => b,
                Err(e) => {
                    self.toasts.push(Kind::Error, "Could not read file", e);
                    continue;
                }
            };
            let name = crate::skin_tasks::file_stem(file.path());
            self.add_skin(&name, &bytes, None);
        }
    }

    pub(crate) fn on_skins_event(&mut self, event: Event) {
        match event {
            Event::SkinAccount(account_id, result) => {
                self.skins.busy = false;
                // New textures for the current skin and capes.
                self.skins.textures.retain(|k, _| k.starts_with("lib:"));
                self.skins.failed.clear();
                if let Err(e) = &result {
                    self.toasts
                        .push(Kind::Error, "Skin change failed", e.clone());
                }
                self.skins.account.insert(account_id, result);
            }
            Event::ArcticCapes(account_id, result) => {
                self.skins.arctic_busy = false;
                self.skins.textures.retain(|k, _| !k.starts_with("arctic:"));
                self.skins.arctic.insert(account_id, result);
            }
            Event::PlayerSkin(name, result) => {
                self.skins.looking_up = false;
                match result {
                    Ok((png, variant)) => {
                        self.add_skin(&name, &png, Some(variant));
                        self.skins.player_name.clear();
                    }
                    Err(e) => self.toasts.push(Kind::Error, "Could not get skin", e),
                }
            }
            Event::SkinFile(result) => match result {
                Ok(Some((name, bytes))) => self.add_skin(&name, &bytes, None),
                Ok(None) => {}
                Err(e) => self.toasts.push(Kind::Error, "Could not read file", e),
            },
            _ => {}
        }
    }
}

fn upload(ctx: &egui::Context, key: &str, png: &[u8]) -> Option<SkinTexture> {
    // Capes are 64×32 (or larger multiples); skins go through the skin decoder.
    if key.starts_with("cape:") || key.starts_with("arctic:") {
        let image = image::load_from_memory(png).ok()?.to_rgba8();
        let size = [image.width() as usize, image.height() as usize];
        let color = ColorImage::from_rgba_unmultiplied(size, image.as_raw());
        let handle = ctx.load_texture(key, color, TextureOptions::NEAREST);
        return Some(SkinTexture {
            handle,
            overlay: false,
            guessed: Variant::Classic,
        });
    }
    let image = skins::decode(png).ok()?;
    let color = ColorImage::from_rgba_unmultiplied([64, 64], &image.rgba);
    let handle = ctx.load_texture(key, color, TextureOptions::NEAREST);
    Some(SkinTexture {
        handle,
        overlay: !image.legacy,
        guessed: image.guess_variant(),
    })
}
