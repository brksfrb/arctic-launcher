//! Cosmetics tab: your Arctic look (skin, cape and 3D cosmetics, seen by
//! every Arctic player, on any account), your Minecraft skin and capes
//! (Microsoft accounts), a 3D preview and a local library of skins.
//!
//! Clicking something wears it at once: the preview shows it right away
//! (`trying`) while the look is saved in the background; clicks made while
//! a save is still going are queued, and only the newest one is sent.

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

/// The part of the tab on the right.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Section {
    #[default]
    Skins,
    Capes,
    Cosmetics,
}

/// A change shown in the preview before the server has confirmed it.
/// `Some(None)` means "taken off".
#[derive(Debug, Clone, Default)]
pub struct Trying {
    /// Texture key and arm model.
    pub skin: Option<Option<(String, Variant)>>,
    /// Texture key.
    pub cape: Option<Option<String>>,
    pub cosmetics: Option<Vec<String>>,
}

/// Which skins the Skins section lists.
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
/// `acape:<hash>` Arctic cape, `acos:<hash>` Arctic cosmetic texture.
#[derive(Default)]
pub struct SkinsUi {
    loaded_for: Option<PathBuf>,
    pub library: Library,
    textures: HashMap<String, SkinTexture>,
    /// Keys whose PNG couldn't be read or decoded (not retried every frame).
    failed: HashSet<String>,
    pub section: Section,
    pub trying: Trying,
    /// The newest look asked for (sent or queued): later clicks build on it.
    intended: Option<NewLook>,
    /// Asked for while a save was still going; sent when it finishes.
    queued: Option<NewLook>,
    /// When the last save finished (for the "Saved" note).
    pub saved_at: Option<f64>,
    /// A skin the game picked before the look had loaded (worn once it has).
    from_game: Option<Option<String>>,
    /// Library skin id -> the hash the server would store it under.
    lib_hashes: HashMap<String, String>,
    /// The frame this tab was last drawn in (to notice it being reopened).
    last_pass: u64,
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
        // Debug builds: ARCTIC_DEVSHOT_SECTION picks the section (screenshots).
        let section = match std::env::var("ARCTIC_DEVSHOT_SECTION")
            .ok()
            .filter(|_| cfg!(debug_assertions))
            .as_deref()
        {
            Some("capes") => Section::Capes,
            Some("cosmetics") => Section::Cosmetics,
            _ => Section::default(),
        };
        self.skins = SkinsUi {
            view,
            yaw,
            pitch: 0.12,
            section,
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
        // Opened again: start from what you're wearing, not half-done edits.
        let pass = ui.ctx().cumulative_pass_nr();
        if pass > self.skins.last_pass + 1 {
            self.skins.renaming = None;
        }
        self.skins.last_pass = pass;
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
            self.tasks.arctic_look(account.clone(), None, None);
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

    /// Wear a change right away (`show` puts it in the preview) and save it:
    /// now, or after the save in progress (only the newest change is sent).
    fn change_look(&mut self, edit: impl FnOnce(&mut NewLook), show: impl FnOnce(&mut Trying)) {
        let (Some(account), Some(state)) = (
            self.accounts.active().cloned(),
            self.arctic_state().cloned(),
        ) else {
            return;
        };
        let mut look = self
            .skins
            .intended
            .clone()
            .unwrap_or_else(|| state.current());
        edit(&mut look);
        show(&mut self.skins.trying);
        self.skins.intended = Some(look.clone());
        self.skins.saved_at = None;
        if self.skins.arctic_busy {
            self.skins.queued = Some(look);
        } else {
            self.skins.arctic_busy = true;
            self.tasks.arctic_look(account, Some(look), Some(state));
        }
    }

    fn wear_skin(&mut self, entry: &SkinEntry) {
        match Library::read_png(&self.skins_dir(), &entry.id) {
            Ok(png) => {
                let variant = entry.variant;
                let key = format!("lib:{}", entry.id);
                self.change_look(
                    |l| l.skin = Some((Texture::Png(png), variant)),
                    |t| t.skin = Some(Some((key, variant))),
                );
            }
            Err(e) => self
                .toasts
                .push(Kind::Error, "Could not read skin", e.to_string()),
        }
    }

    /// The game's skin picker chose a library skin (`None`: the Minecraft skin).
    pub(crate) fn wear_skin_from_game(&mut self, id: Option<String>) {
        let Some(id) = id else {
            if self.arctic_state().is_none() {
                self.skins.from_game = Some(None);
            }
            self.stop_wearing_skin();
            return;
        };
        self.ensure_skin_library();
        if self.arctic_state().is_none() {
            // The look isn't loaded yet (the Cosmetics tab never opened): load
            // it, then wear this.
            if let Some(account) = self.accounts.active().cloned()
                && !self.skins.arctic_busy
            {
                self.skins.arctic_busy = true;
                self.tasks.arctic_look(account, None, None);
            }
            self.skins.from_game = Some(Some(id));
            return;
        }
        let entry = self
            .skins
            .library
            .skins
            .iter()
            .find(|s| s.id == id)
            .cloned();
        match entry {
            Some(entry) => self.wear_skin(&entry),
            None => log::warn!("the game picked skin {id}, which isn't in the library"),
        }
    }

    /// Back to the Minecraft skin (for Arctic players too).
    fn stop_wearing_skin(&mut self) {
        self.change_look(|l| l.skin = None, |t| t.skin = Some(None));
    }

    /// `key`: the cape's texture key for the preview (`None`: no cape).
    fn set_arctic_cape(&mut self, cape: Option<CapeChoice>, key: Option<String>) {
        self.change_look(|l| l.cape = cape, |t| t.cape = Some(key));
    }

    fn set_cosmetics(&mut self, ids: Vec<String>) {
        let shown = ids.clone();
        self.change_look(|l| l.cosmetics = Some(ids), |t| t.cosmetics = Some(shown));
    }

    /// The server's hash for a library skin (worked out once).
    fn library_hash(&mut self, id: &str) -> Option<String> {
        if !self.skins.lib_hashes.contains_key(id) {
            let png = Library::read_png(&self.skins_dir(), id).ok()?;
            self.skins
                .lib_hashes
                .insert(id.to_owned(), arctic_core::cosmetics::texture_hash(&png));
        }
        self.skins.lib_hashes.get(id).cloned()
    }

    /// Texture for a key (see `SkinsUi`), uploading on first use.
    pub(crate) fn skin_texture(&mut self, ctx: &egui::Context, key: &str) -> Option<&SkinTexture> {
        if key == MANNEQUIN && !self.skins.textures.contains_key(key) {
            self.skins.textures.insert(key.to_owned(), mannequin(ctx));
        }
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
            .or_else(|| key.strip_prefix("acos:"))
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

    /// Add a skin to the library (or find it there); its entry.
    fn add_skin(&mut self, name: &str, png: &[u8], variant: Option<Variant>) -> Option<SkinEntry> {
        let dir = self.skins_dir();
        if let Some(existing) = self.skins.library.find_same(&dir, png) {
            let existing = existing.clone();
            self.toasts
                .push(Kind::Info, "Already in your library", existing.name.clone());
            return Some(existing);
        }
        match self.skins.library.add(&dir, name, png, variant) {
            Ok(entry) => {
                self.toasts
                    .push(Kind::Success, format!("Added {}", entry.name), "");
                Some(entry)
            }
            Err(e) => {
                self.toasts
                    .push(Kind::Error, "Could not add skin", e.to_string());
                None
            }
        }
    }

    fn remove_skin(&mut self, id: &str) {
        let dir = self.skins_dir();
        if let Err(e) = self.skins.library.remove(&dir, id) {
            self.toasts
                .push(Kind::Error, "Could not delete skin", e.to_string());
        }
        self.skins.textures.remove(&format!("lib:{id}"));
        self.skins.lib_hashes.remove(id);
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
            let _ = self.add_skin(&name, &bytes, None);
        }
    }

    /// A load or save came back: show the server's look, or send the change
    /// that was queued meanwhile.
    fn on_look_saved(&mut self, account_id: String, result: Result<ArcticState, String>) {
        self.skins.arctic_busy = false;
        self.skins.failed.clear();
        let had_look = self
            .skins
            .arctic
            .get(&account_id)
            .is_some_and(Result::is_ok);
        let queued = self.skins.queued.take();
        match result {
            Err(e) if had_look => {
                // A save failed: back to the look the server has.
                self.skins.intended = None;
                self.skins.trying = Trying::default();
                self.toasts.push(Kind::Error, "Couldn't save your look", e);
            }
            result => {
                let ok = result.is_ok();
                self.skins.arctic.insert(account_id.clone(), result);
                let active = self.accounts.active().cloned();
                let state = self.arctic_state().cloned();
                match (queued, active, state) {
                    (Some(next), Some(account), Some(state)) if ok && account.id == account_id => {
                        self.skins.arctic_busy = true;
                        self.tasks.arctic_look(account, Some(next), Some(state));
                    }
                    _ => {
                        self.skins.intended = None;
                        self.skins.trying = Trying::default();
                        if ok && had_look {
                            self.skins.saved_at = Some(now_secs_f64());
                        }
                        if ok && let Some(picked) = self.skins.from_game.take() {
                            self.wear_skin_from_game(picked);
                        }
                    }
                }
            }
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
            Event::ArcticLook(account_id, result) => self.on_look_saved(account_id, result),
            Event::PlayerSkin(name, result) => {
                self.skins.looking_up = false;
                match result {
                    Ok((png, variant)) => {
                        let _ = self.add_skin(&name, &png, Some(variant));
                        self.skins.player_name.clear();
                    }
                    Err(e) => self.toasts.push(Kind::Error, "Could not get skin", e),
                }
            }
            Event::SkinFile(result) => match result {
                Ok(Some((name, bytes))) => {
                    let _ = self.add_skin(&name, &bytes, None);
                }
                Ok(None) => {}
                Err(e) => self.toasts.push(Kind::Error, "Could not read file", e),
            },
            e @ (Event::Gallery(..) | Event::GalleryTaken(..) | Event::GalleryDone(..)) => {
                self.on_gallery_event(e)
            }
            Event::CapeFile(result) => match result {
                Ok(Some(bytes)) => {
                    // Shown by its hash once saved; until then the old cape stays.
                    self.set_arctic_cape(Some(CapeChoice::Custom(Texture::Png(bytes))), None);
                    self.skins.trying.cape = None;
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
    if key.starts_with("acos:") {
        return upload_plain(ctx, key, png);
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

/// Texture key of the plain figure shown when there's no skin yet.
pub(crate) const MANNEQUIN: &str = "mannequin";

/// A plain, featureless figure in the skin layout (so cosmetics can be
/// previewed before any skin is chosen). Drawn in code: no bundled art.
fn mannequin(ctx: &egui::Context) -> SkinTexture {
    let mut rgba = vec![0u8; 64 * 64 * 4];
    // (x, y, w, h, shade) of each base-layer region in the 64×64 layout.
    let regions: [(usize, usize, usize, usize, u8); 6] = [
        (0, 0, 32, 16, 196),   // head
        (16, 16, 24, 16, 176), // body
        (40, 16, 16, 16, 186), // right arm
        (32, 48, 16, 16, 186), // left arm
        (0, 16, 16, 16, 160),  // right leg
        (16, 48, 16, 16, 160), // left leg
    ];
    for (x0, y0, w, h, shade) in regions {
        for y in y0..y0 + h {
            for x in x0..x0 + w {
                let i = (y * 64 + x) * 4;
                // A faint checker keeps the shape readable when turning.
                let tint = if (x + y) % 2 == 0 { 0 } else { 8 };
                rgba[i..i + 4].copy_from_slice(&[
                    shade - tint,
                    shade - tint + 6,
                    shade - tint + 14,
                    255,
                ]);
            }
        }
    }
    let color = ColorImage::from_rgba_unmultiplied([64, 64], &rgba);
    SkinTexture {
        handle: ctx.load_texture(MANNEQUIN, color, TextureOptions::NEAREST),
        overlay: false,
        guessed: Variant::Classic,
        frames: Vec::new(),
    }
}

/// Largest cosmetic texture side (as the server and game allow).
const MAX_COSMETIC_TEXTURE: u32 = 512;

/// A cosmetic texture: any power-of-two PNG up to 512 on a side.
fn upload_plain(ctx: &egui::Context, key: &str, png: &[u8]) -> Option<SkinTexture> {
    let (w, h) = arctic_core::cosmetics::png_size(png)?;
    let side = |s: u32| (1..=MAX_COSMETIC_TEXTURE).contains(&s) && s.is_power_of_two();
    if !side(w) || !side(h) {
        return None;
    }
    let image = image::load_from_memory(png).ok()?.to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    let color = ColorImage::from_rgba_unmultiplied(size, image.as_raw());
    let handle = ctx.load_texture(key, color, TextureOptions::NEAREST);
    Some(SkinTexture {
        handle,
        overlay: false,
        guessed: Variant::Classic,
        frames: Vec::new(),
    })
}

/// A cape texture; animated capes become one texture per frame.
fn upload_cape(ctx: &egui::Context, key: &str, png: &[u8]) -> Option<SkinTexture> {
    // Only cape-sized images get decoded (a bad file could claim any size).
    if !arctic_core::cosmetics::is_cape_png(png) {
        return None;
    }
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

/// Seconds since the Unix epoch, for "saved just now" notes.
fn now_secs_f64() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64())
}
