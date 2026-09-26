//! Skins tab: your Arctic look (skin + cape, seen by every Arctic player,
//! on any account), your Minecraft skin and capes (Microsoft accounts), a
//! 3D preview and a local library of skins.

mod gallery;
mod model;
mod panels;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use arctic_core::cosmetics::{CapeChoice, NewLook, Texture};
use arctic_core::skins::{self, Library, SkinEntry, Variant};
use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};

use crate::app::ArcticApp;
use crate::skin_tasks::{AccountSkin, ArcticState, SkinChange};
use crate::tasks::Event;
use crate::toasts::Kind;

/// What the preview shows.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Selection {
    /// How other players see you right now.
    #[default]
    Current,
    Library(String),
    Gallery(arctic_core::cosmetics::GalleryItem),
}

/// Which list the right side shows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SkinsView {
    #[default]
    Library,
    Gallery,
}

/// A skin or cape uploaded to the GPU.
pub struct SkinTexture {
    pub handle: TextureHandle,
    /// Has the second layer on body and limbs (not a legacy 64×32 skin).
    pub overlay: bool,
    pub guessed: Variant,
    /// Animated capes: one texture per frame (`handle` is the first).
    frames: Vec<TextureHandle>,
}

impl SkinTexture {
    /// The texture to draw at `time` (seconds): animated capes cycle.
    pub fn id_at(&self, time: f64) -> egui::TextureId {
        if self.frames.is_empty() {
            return self.handle.id();
        }
        let frame = (time / arctic_core::cosmetics::CAPE_FRAME_SECS) as usize % self.frames.len();
        self.frames[frame].id()
    }

    pub fn animated(&self) -> bool {
        !self.frames.is_empty()
    }
}

/// Texture keys: `lib:<id>` library skin, `mc` Minecraft skin,
/// `mccape:<id>` Minecraft cape, `askin:<hash>` Arctic skin,
/// `acape:<hash>` Arctic cape.
#[derive(Default)]
pub struct SkinsUi {
    loaded_for: Option<PathBuf>,
    pub library: Library,
    textures: HashMap<String, SkinTexture>,
    /// Keys whose PNG couldn't be read or decoded (not retried every frame).
    failed: HashSet<String>,
    pub selection: Selection,
    /// Account id → Minecraft skin state (Microsoft accounts).
    pub account: HashMap<String, Result<AccountSkin, String>>,
    pub busy: bool,
    /// Account id → Arctic look.
    pub arctic: HashMap<String, Result<ArcticState, String>>,
    pub arctic_busy: bool,
    pub player_name: String,
    pub looking_up: bool,
    pub yaw: f32,
    pub pitch: f32,
    pub renaming: Option<(String, String)>,
    pub view: SkinsView,
    pub gallery: gallery::GalleryUi,
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
        // Debug builds: ARCTIC_DEVSHOT_YAW turns the preview (screenshots).
        let yaw = std::env::var("ARCTIC_DEVSHOT_YAW")
            .ok()
            .filter(|_| cfg!(debug_assertions))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.5);
        let view = if cfg!(debug_assertions)
            && std::env::var("ARCTIC_DEVSHOT_PAGE").as_deref() == Ok("gallery")
        {
            SkinsView::Gallery
        } else {
            SkinsView::Library
        };
        self.skins = SkinsUi {
            view,
            yaw,
            pitch: 0.12,
            ..SkinsUi::default()
        };
        match Library::load(&dir) {
            Ok(mut lib) => {
                if let Err(e) = lib.dedupe(&dir) {
                    log::warn!("skin library cleanup: {e}");
                }
                self.skins.library = lib;
            }
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
        self.request_skin_state(false);
        self.skins_page(ui);
        self.accept_dropped_skins(ui.ctx());
    }

    /// Fetch the active account's Arctic look (and Minecraft skin) once, or
    /// again with `force`.
    fn request_skin_state(&mut self, force: bool) {
        let Some(account) = self.accounts.active().cloned() else {
            return;
        };
        if (force || !self.skins.arctic.contains_key(&account.id)) && !self.skins.arctic_busy {
            self.skins.arctic_busy = true;
            self.tasks.arctic_look(account.clone(), None);
        }
        if account.is_microsoft()
            && (force || !self.skins.account.contains_key(&account.id))
            && !self.skins.busy
        {
            self.skins.busy = true;
            self.tasks.skin_account(account, SkinChange::None);
        }
    }

    fn change_minecraft_skin(&mut self, change: SkinChange) {
        let Some(account) = self.accounts.active().cloned() else {
            return;
        };
        self.skins.busy = true;
        self.tasks.skin_account(account, change);
    }

    pub(crate) fn arctic_state(&self) -> Option<&ArcticState> {
        let account = self.accounts.active()?;
        self.skins.arctic.get(&account.id)?.as_ref().ok()
    }

    /// Publish a new Arctic look built from the current one.
    fn change_look(&mut self, edit: impl FnOnce(&mut NewLook)) {
        let (Some(account), Some(state)) = (self.accounts.active().cloned(), self.arctic_state())
        else {
            return;
        };
        let mut look = state.current();
        edit(&mut look);
        self.skins.arctic_busy = true;
        self.tasks.arctic_look(account, Some(look));
    }

    fn wear_skin(&mut self, entry: &SkinEntry) {
        match Library::read_png(&self.skins_dir(), &entry.id) {
            Ok(png) => {
                let variant = entry.variant;
                self.change_look(|l| l.skin = Some((Texture::Png(png), variant)));
                self.skins.selection = Selection::Current;
            }
            Err(e) => self
                .toasts
                .push(Kind::Error, "Could not read skin", e.to_string()),
        }
    }

    fn set_arctic_cape(&mut self, cape: Option<CapeChoice>) {
        self.change_look(|l| l.cape = cape);
    }

    /// Texture for a key (see `SkinsUi`), uploading on first use.
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
        if let Some(hash) = key.strip_prefix("gal:") {
            return self.skins.gallery.pngs.get(hash).cloned();
        }
        if let Some(hash) = key
            .strip_prefix("askin:")
            .or_else(|| key.strip_prefix("acape:"))
        {
            return self.arctic_state()?.png(hash).map(<[u8]>::to_vec);
        }
        let account = self.accounts.active()?;
        let state = self.skins.account.get(&account.id)?.as_ref().ok()?;
        match key.strip_prefix("mccape:") {
            Some(cape) => state
                .capes
                .iter()
                .find(|(id, _)| id == cape)
                .map(|(_, png)| png.clone()),
            None if key == "mc" => state.skin_png.clone(),
            None => None,
        }
    }

    fn add_skin(&mut self, name: &str, png: &[u8], variant: Option<Variant>) {
        let dir = self.skins_dir();
        if let Some(existing) = self.skins.library.find_same(&dir, png) {
            let id = existing.id.clone();
            self.toasts
                .push(Kind::Info, "Already in your library", existing.name.clone());
            self.skins.selection = Selection::Library(id);
            return;
        }
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

    fn set_minecraft_skin(&mut self, entry: &SkinEntry) {
        match Library::read_png(&self.skins_dir(), &entry.id) {
            Ok(png) => self.change_minecraft_skin(SkinChange::Upload(entry.variant, png)),
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
                self.skins.textures.retain(|k, _| !k.starts_with("mc"));
                self.skins.failed.clear();
                if let Err(e) = &result {
                    self.toasts
                        .push(Kind::Error, "Minecraft skin change failed", e.clone());
                }
                self.skins.account.insert(account_id, result);
            }
            Event::ArcticLook(account_id, result) => {
                self.skins.arctic_busy = false;
                self.skins.failed.clear();
                if let (Ok(_), Some(Ok(_))) = (&result, self.skins.arctic.get(&account_id)) {
                    self.toasts.push(
                        Kind::Success,
                        "Look updated",
                        "Every Arctic player sees it now.",
                    );
                }
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
            e @ (Event::Gallery(..) | Event::GalleryTaken(..) | Event::GalleryDone(..)) => {
                self.on_gallery_event(e)
            }
            Event::CapeFile(result) => match result {
                Ok(Some(bytes)) => {
                    self.set_arctic_cape(Some(CapeChoice::Custom(Texture::Png(bytes))))
                }
                Ok(None) => {}
                Err(e) => self.toasts.push(Kind::Error, "Could not read file", e),
            },
            _ => {}
        }
    }
}

fn upload(ctx: &egui::Context, key: &str, png: &[u8]) -> Option<SkinTexture> {
    // Capes are 64×32 (or larger multiples, or stacked animation frames);
    // skins go through the skin decoder.
    if key.starts_with("mccape:") || key.starts_with("acape:") {
        return upload_cape(ctx, key, png);
    }
    let image = skins::decode(png).ok()?;
    let color = ColorImage::from_rgba_unmultiplied([64, 64], &image.rgba);
    let handle = ctx.load_texture(key, color, TextureOptions::NEAREST);
    Some(SkinTexture {
        handle,
        overlay: !image.legacy,
        guessed: image.guess_variant(),
        frames: Vec::new(),
    })
}

/// A cape texture; animated capes become one texture per frame.
fn upload_cape(ctx: &egui::Context, key: &str, png: &[u8]) -> Option<SkinTexture> {
    let image = image::load_from_memory(png).ok()?.to_rgba8();
    let (w, h) = image.dimensions();
    let count = arctic_core::cosmetics::cape_frames(w, h).unwrap_or(1);
    let frame_h = h / count;
    let frames: Vec<TextureHandle> = (0..count)
        .map(|i| {
            let frame = image::imageops::crop_imm(&image, 0, i * frame_h, w, frame_h).to_image();
            let size = [w as usize, frame_h as usize];
            let color = ColorImage::from_rgba_unmultiplied(size, frame.as_raw());
            ctx.load_texture(format!("{key}#{i}"), color, TextureOptions::NEAREST)
        })
        .collect();
    let handle = frames.first()?.clone();
    Some(SkinTexture {
        handle,
        overlay: false,
        guessed: Variant::Classic,
        frames: if count > 1 { frames } else { Vec::new() },
    })
}
