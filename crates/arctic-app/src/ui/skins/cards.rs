//! The lists of the Cosmetics tab: skins (your library and the gallery),
//! capes and each cosmetic slot, as cards. Pointing at a card tries it on
//! in the preview ([`Hover`]); clicking wears it (again: takes it off).

use arctic_core::cosmetic_models::{Geometry, Mesh};
use arctic_core::cosmetics::CapeChoice;
use arctic_core::skins::{SkinEntry, Variant};
use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Rect, RichText, Sense, pos2, vec2,
};

use super::model::{self, Pose};
use super::panels::{slot_key, slot_name};
use super::{Hover, SkinsView};
use crate::app::ArcticApp;
use crate::art::icons::Icon;
use crate::art::lerp_color;
use crate::theme::Palette;
use crate::widgets;

const SKIN_CARD: [f32; 2] = [132.0, 176.0];
const CAPE_CARD: [f32; 2] = [112.0, 168.0];
const COSMETIC_CARD: [f32; 2] = [150.0, 170.0];
const GAP: f32 = 12.0;
/// A card's fixed three-quarter angle; the one pointed at turns.
const CARD_YAW: f32 = 0.65;
const TURN_SPEED: f64 = 1.4;

impl ArcticApp {
    // ---- Headers -------------------------------------------------------------------

    /// A list's title, a line about it and (optionally) the search box.
    fn list_header(&mut self, ui: &mut egui::Ui, title: &str, about: &str, search: bool) {
        let p = self.palette();
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new(title).size(20.0).strong().color(p.text));
                ui.label(RichText::new(about).small().color(p.muted));
            });
            if search {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(
                        widgets::text_field(&mut self.skins.search)
                            .hint_text("Search")
                            .desired_width(200.0),
                    );
                });
            }
        });
        ui.add_space(12.0);
    }

    fn matches_search(&self, name: &str) -> bool {
        let q = self.skins.search.trim();
        q.is_empty() || name.to_lowercase().contains(&q.to_lowercase())
    }

    // ---- Skins ---------------------------------------------------------------------

    /// Everything above the cards: title, search, tabs, the player-name and gallery search rows.
    pub(super) fn skins_header(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let library = self.skins.view == SkinsView::Library;
        self.list_header(
            ui,
            "Skins",
            "Arctic players see the skin you pick here; others see your Minecraft skin.",
            library,
        );
        // Wraps instead of overlapping when the window is narrow.
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.skins.view, SkinsView::Library, "My skins");
            ui.selectable_value(&mut self.skins.view, SkinsView::Gallery, "Gallery");
            if library {
                ui.add_space(8.0);
                if widgets::button(ui, p, Some(Icon::Plus), "Add skin…", true).clicked() {
                    self.tasks.pick_skin_file();
                }
                if widgets::icon_button(ui, p, Icon::Folder, "Open skins folder").clicked() {
                    let dir = self.skins_dir();
                    let _ = std::fs::create_dir_all(&dir);
                    let _ = open::that_detached(&dir);
                }
            }
        });
        ui.add_space(10.0);
        match self.skins.view {
            SkinsView::Library => self.library_header(ui),
            SkinsView::Gallery => self.gallery_header(ui),
        }
    }

    /// The skin cards (the part that scrolls).
    pub(super) fn skins_body(&mut self, ui: &mut egui::Ui) {
        match self.skins.view {
            SkinsView::Library => self.library_body(ui),
            SkinsView::Gallery => self.gallery_body(ui),
        }
    }

    fn library_header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let field = ui.add(
                widgets::text_field(&mut self.skins.player_name)
                    .hint_text("Copy a player's skin by name")
                    .char_limit(16)
                    .desired_width(240.0),
            );
            let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let ready = !self.skins.player_name.trim().is_empty() && !self.skins.looking_up;
            let get = ui.add_enabled_ui(ready, |ui| ui.button("Get")).inner;
            if ready && (get.clicked() || enter) {
                self.skins.looking_up = true;
                self.tasks
                    .player_skin(self.skins.player_name.trim().to_owned());
            }
            if self.skins.looking_up {
                ui.spinner();
            }
        });
        self.rename_row(ui);
        ui.add_space(10.0);
    }

    fn library_body(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let entries: Vec<SkinEntry> = self
            .skins
            .library
            .skins
            .clone()
            .into_iter()
            .filter(|e| self.matches_search(&e.name))
            .collect();
        let current = self.current_look();
        let can_wear = self.arctic_state().is_some();
        let worn_hash = self
            .arctic_state()
            .and_then(|s| s.look.skin.clone())
            .filter(|_| self.skins.trying.skin.is_none());
        let trying_key = match &self.skins.trying.skin {
            Some(Some((key, _))) => Some(key.clone()),
            _ => None,
        };
        let mut wear: Option<SkinEntry> = None;
        let mut stop = false;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(GAP, GAP);
            // Your Minecraft skin: wearing it means no Arctic skin.
            let mc_key = if current.arctic_skin || current.skin_key.is_none() {
                self.skin_png("mc").map(|_| "mc".to_owned())
            } else {
                current.skin_key.clone()
            };
            let key = mc_key.unwrap_or_else(|| super::MANNEQUIN.to_owned());
            if self.matches_search("Minecraft skin") {
                let tile = self.skin_tile(ui, &key, "Minecraft skin", None, !current.arctic_skin);
                if tile
                    .on_hover_text("Your own Minecraft skin, for Arctic players too")
                    .clicked()
                    && current.arctic_skin
                    && can_wear
                {
                    stop = true;
                }
            }
            for e in &entries {
                let worn = trying_key.as_deref() == Some(format!("lib:{}", e.id).as_str())
                    || (worn_hash.is_some() && self.library_hash(&e.id) == worn_hash);
                let tile =
                    self.skin_tile(ui, &format!("lib:{}", e.id), &e.name, Some(e.variant), worn);
                if tile.clicked() && !worn && can_wear {
                    wear = Some(e.clone());
                }
                tile.context_menu(|ui| self.skin_menu(ui, e));
            }
        });
        if stop {
            self.stop_wearing_skin();
        }
        if let Some(entry) = wear {
            self.wear_skin(&entry);
        }
        ui.add_space(8.0);
        let hint = if self.skins.library.skins.is_empty() {
            "Drop .png skin files here, pick one with Add skin, or copy a player's skin."
        } else {
            "Right-click a skin to rename, share or delete it."
        };
        ui.label(RichText::new(hint).small().color(p.muted));
    }

    /// Renaming a library skin (started from its right-click menu).
    fn rename_row(&mut self, ui: &mut egui::Ui) {
        let Some((id, mut name)) = self.skins.renaming.clone() else {
            return;
        };
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Name");
            let field = ui.add(
                widgets::text_field(&mut name)
                    .char_limit(48)
                    .desired_width(240.0),
            );
            let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let save = ui.button("Save").clicked();
            let cancel = ui.button("Cancel").clicked();
            if save || enter {
                let trimmed = name.trim().to_owned();
                if !trimmed.is_empty() {
                    self.update_skin(&id, |e| e.name = trimmed);
                }
                self.skins.renaming = None;
            } else if cancel {
                self.skins.renaming = None;
            } else {
                self.skins.renaming = Some((id, name));
                if !field.has_focus() && !field.lost_focus() {
                    field.request_focus();
                }
            }
        });
    }

    /// A library skin's right-click menu.
    fn skin_menu(&mut self, ui: &mut egui::Ui, entry: &SkinEntry) {
        let id = entry.id.clone();
        if ui.button("Rename…").clicked() {
            self.skins.renaming = Some((id.clone(), entry.name.clone()));
            ui.close();
        }
        for v in [Variant::Classic, Variant::Slim] {
            let label = format!("{} arms", v.label());
            if ui.radio(entry.variant == v, label).clicked() && entry.variant != v {
                self.update_skin(&id, |e| e.variant = v);
                ui.close();
            }
        }
        ui.separator();
        if self.can_change_minecraft_skin()
            && ui
                .button("Set as Minecraft skin")
                .on_hover_text("Changes your real Minecraft skin, seen by everyone")
                .clicked()
        {
            self.set_minecraft_skin(entry);
            ui.close();
        }
        if self.share_button(ui, entry) {
            ui.close();
        }
        ui.separator();
        if ui.button("Delete").clicked() {
            self.remove_skin(&id);
            ui.close();
        }
    }

    /// A skin card: the skin in 3D and its name, outlined while worn.
    /// Pointing at it tries it on.
    pub(super) fn skin_tile(
        &mut self,
        ui: &mut egui::Ui,
        key: &str,
        name: &str,
        variant: Option<Variant>,
        worn: bool,
    ) -> egui::Response {
        let p = self.palette();
        let ctx = ui.ctx().clone();
        let (rect, response) = ui.allocate_exact_size(SKIN_CARD.into(), Sense::click());
        let hover = card_back(ui, p, rect, &response);
        let model_rect = Rect::from_min_size(
            rect.min + vec2(10.0, 10.0),
            vec2(rect.width() - 20.0, rect.height() - 44.0),
        );
        let now = ui.input(|i| i.time);
        if let Some(tex) = self.skin_texture(&ctx, key) {
            let variant = variant.unwrap_or(tex.guessed);
            let yaw = if response.hovered() {
                CARD_YAW + ((now * TURN_SPEED).sin() as f32) * 0.5
            } else {
                CARD_YAW * 0.6
            };
            model::paint(
                ui.painter(),
                model_rect,
                tex.handle.id(),
                variant,
                tex.overlay,
                None,
                &[],
                &[],
                Pose {
                    yaw,
                    pitch: 0.1,
                    swing: 0.0,
                },
            );
            if response.hovered() {
                self.skins.hover = Some(Hover::Skin(key.to_owned(), variant));
                ctx.request_repaint();
            }
        }
        card_label(ui, p, rect, name, worn.then_some("Wearing"), hover);
        worn_frame(ui, p, rect, worn);
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    }

    // ---- Capes ---------------------------------------------------------------------

    pub(super) fn capes_header(&mut self, ui: &mut egui::Ui) {
        self.list_header(
            ui,
            "Capes",
            "Arctic players see an Arctic cape; without one, your Minecraft cape shows.",
            true,
        );
    }

    pub(super) fn capes_body(&mut self, ui: &mut egui::Ui) {
        self.arctic_capes(ui);
        self.minecraft_capes(ui);
    }

    /// Arctic capes, your own image, or none. Capes that can animate carry a small
    /// animation icon; the Animate switch is in the Equipped card.
    fn arctic_capes(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let Some(state) = self.arctic_state().cloned() else {
            ui.label(RichText::new("Arctic capes show once your look has loaded.").color(p.muted));
            return;
        };
        let ctx = ui.ctx().clone();
        let worn = match &self.skins.trying.cape {
            Some(Some(key)) => key.strip_prefix("acape:").map(str::to_owned),
            Some(None) => None,
            None => state.look.cape.clone(),
        };
        let now = ui.input(|i| i.time);
        let presets: Vec<_> = state
            .presets
            .iter()
            .filter(|c| self.matches_search(&c.name))
            .cloned()
            .collect();
        let mut pick: Option<(Option<CapeChoice>, Option<String>)> = None;
        let mut upload = false;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(GAP, GAP);
            let r = cape_card(ui, p, None, Icon::Close, "No cape", None, worn.is_none());
            if r.hovered() {
                self.skins.hover = Some(Hover::Cape(None));
            }
            if r.clicked() && worn.is_some() {
                pick = Some((None, None));
            }
            for preset in &presets {
                let animate_on = !self.settings.capes_still.contains(&preset.id);
                let shown = match (&preset.still, animate_on) {
                    (Some(still), false) if preset.animated() => still.clone(),
                    _ => preset.texture.clone(),
                };
                let key = format!("acape:{shown}");
                let tex = self.skin_texture(&ctx, &key).map(|t| animate(&ctx, t, now));
                let active = worn.as_deref().is_some_and(|h| preset.is_texture(h));
                let r = cape_card(ui, p, tex, Icon::Close, &preset.name, None, active);
                if preset.has_still() {
                    animation_badge(ui, p, r.rect, animate_on);
                }
                if r.hovered() {
                    self.skins.hover = Some(Hover::Cape(Some(key.clone())));
                }
                if r.clicked() && !active {
                    pick = Some((Some(preset.choice(animate_on)), Some(key)));
                }
            }
            let custom = worn
                .as_ref()
                .filter(|h| !state.presets.iter().any(|p| p.is_texture(h)));
            if let Some(hash) = custom {
                let tex = self
                    .skin_texture(&ctx, &format!("acape:{hash}"))
                    .map(|t| animate(&ctx, t, now));
                cape_card(ui, p, tex, Icon::Close, "Your cape", None, true);
            }
            let r = cape_card(ui, p, None, Icon::Plus, "Your own…", None, false).on_hover_text(
                "Use your own cape image: 64×32 up to 1024×512; stack up to 32 frames (12288 pixels tall) to animate it, at 8 frames a second",
            );
            if r.clicked() {
                upload = true;
            }
        });
        ui.add_space(12.0);
        if upload {
            self.tasks.pick_cape_file();
        }
        if let Some((choice, key)) = pick {
            self.set_arctic_cape(choice, key);
        }
    }

    /// Official Minecraft capes the account owns (Microsoft only).
    fn minecraft_capes(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let Some(Ok(state)) = self
            .accounts
            .active()
            .and_then(|a| self.skins.account.get(&a.id))
        else {
            return;
        };
        if state.profile.capes.is_empty() {
            return;
        }
        let capes: Vec<(String, String, bool)> = state
            .profile
            .capes
            .iter()
            .map(|c| (c.id.clone(), c.alias.clone(), c.state == "ACTIVE"))
            .collect();
        let none_active = !capes.iter().any(|c| c.2);
        ui.horizontal(|ui| {
            section_label(ui, p, "Minecraft capes · seen by everyone");
            if self.skins.busy {
                ui.spinner();
            }
        });
        let ctx = ui.ctx().clone();
        let mut pick: Option<Option<String>> = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(GAP, GAP);
            if cape_card(ui, p, None, Icon::Close, "None", None, none_active).clicked()
                && !none_active
            {
                pick = Some(None);
            }
            for (id, alias, active) in &capes {
                let key = format!("mccape:{id}");
                let tex = self.skin_texture(&ctx, &key).map(|t| t.handle.id());
                let r = cape_card(ui, p, tex, Icon::Close, alias, None, *active);
                if r.hovered() {
                    self.skins.hover = Some(Hover::Cape(Some(key)));
                }
                if r.clicked() && !active {
                    pick = Some(Some(id.clone()));
                }
            }
        });
        if let Some(choice) = pick
            && !self.skins.busy
        {
            self.change_minecraft_skin(crate::skin_tasks::SkinChange::Cape(choice));
        }
    }

    // ---- 3D cosmetics --------------------------------------------------------------

    /// These cosmetics' models, with textures (for the preview): the cuboid
    /// ones, the sculpted ones, and a white texel for the sculpted ones that
    /// are painted with vertex colors alone.
    pub(super) fn cosmetic_models(
        &mut self,
        ctx: &egui::Context,
        ids: &[String],
    ) -> WornModels {
        let Some(state) = self.arctic_state().cloned() else {
            return WornModels::default();
        };
        let mut out = WornModels::default();
        for entry in cosmetic_entries(&state) {
            if !ids.contains(&entry.id) {
                continue;
            }
            match entry.shape {
                Shape3d::Cuboid { geometry, texture } => {
                    if let Some(t) = self.skin_texture(ctx, &format!("acos:{texture}")) {
                        out.cuboids.push((geometry, t.handle.id()));
                    }
                }
                Shape3d::Mesh { mesh, hash } => {
                    let textures = self.mesh_textures(ctx, &hash, mesh.images.len());
                    out.meshes.push((mesh, textures));
                }
            }
        }
        out.white = self
            .skin_texture(ctx, super::WHITE)
            .map_or(egui::TextureId::default(), |t| t.handle.id());
        out
    }

    /// The textures embedded in a mesh cosmetic (a gap where one isn't ready).
    fn mesh_textures(&mut self, ctx: &egui::Context, hash: &str, count: usize) -> Vec<egui::TextureId> {
        (0..count)
            .map(|i| {
                self.skin_texture(ctx, &format!("amesh:{hash}:{i}"))
                    .map_or(egui::TextureId::default(), |t| t.handle.id())
            })
            .collect()
    }

    /// One slot's cosmetics: pointing at one tries it on, clicking wears it
    /// (replacing what's in that slot) or takes it off.
    pub(super) fn cosmetics_header(&mut self, ui: &mut egui::Ui, slot: &'static str) {
        self.list_header(
            ui,
            slot_name(slot),
            "One per slot. Point at one to try it on; click to wear it, again to take it off.",
            true,
        );
    }

    pub(super) fn cosmetics_body(&mut self, ui: &mut egui::Ui, slot: &'static str) {
        let p = self.palette();
        let Some(state) = self.arctic_state().cloned() else {
            ui.label(RichText::new("Cosmetics show once your look has loaded.").color(p.muted));
            return;
        };
        let ctx = ui.ctx().clone();
        let now = ui.input(|i| i.time);
        let worn_ids = self.worn_ids();
        let entries = cosmetic_entries(&state);
        let items: Vec<&Entry3d> = entries
            .iter()
            .filter(|e| slot_key(&e.slot) == slot && self.matches_search(&e.name))
            .collect();
        if items.is_empty() {
            ui.label(RichText::new("Nothing here yet.").color(p.muted));
            return;
        }
        // What you'd wear after clicking `item`: the rest, plus it in its slot (or nothing).
        let after = |item_id: &str, worn: bool| -> Vec<String> {
            let slot_of = |id: &String| {
                entries
                    .iter()
                    .find(|e| &e.id == id)
                    .map(|e| slot_key(&e.slot))
            };
            let mut next: Vec<String> = worn_ids
                .iter()
                .filter(|id| slot_of(id) != Some(slot))
                .cloned()
                .collect();
            if !worn {
                next.push(item_id.to_owned());
            }
            next
        };
        let mut pick: Option<Vec<String>> = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(GAP, GAP);
            for entry in items {
                let worn = worn_ids.contains(&entry.id);
                let still = match &entry.shape {
                    Shape3d::Cuboid { geometry, texture } => {
                        let t = self
                            .skin_texture(&ctx, &format!("acos:{texture}"))
                            .map(|t| t.handle.id());
                        CardModel::Cuboid(geometry.clone(), t)
                    }
                    Shape3d::Mesh { mesh, hash } => {
                        let textures = self.mesh_textures(&ctx, hash, mesh.images.len());
                        let white = self
                            .skin_texture(&ctx, super::WHITE)
                            .map_or(egui::TextureId::default(), |t| t.handle.id());
                        CardModel::Mesh(mesh.clone(), textures, white)
                    }
                };
                let r = cosmetic_card(ui, p, &still, &entry.name, worn, now);
                if r.hovered() {
                    self.skins.hover = Some(Hover::Cosmetics(after(&entry.id, worn)));
                    ctx.request_repaint();
                }
                if r.clicked() {
                    pick = Some(after(&entry.id, worn));
                }
            }
        });
        if let Some(ids) = pick {
            self.set_cosmetics(ids);
        }
    }
}

// ---- Card pieces -------------------------------------------------------------------

/// A card's background; returns how hovered it is (0..1, animated).
fn card_back(ui: &egui::Ui, p: &Palette, rect: Rect, response: &egui::Response) -> f32 {
    let hover = ui
        .ctx()
        .animate_bool(response.id.with("h"), response.hovered());
    let fill = lerp_color(lerp_color(p.bg, p.surface, 0.5), p.surface_hover, hover);
    ui.painter().rect_filled(rect, CornerRadius::same(12), fill);
    hover
}

/// Name (and a small badge like "Wearing") at the bottom of a card.
fn card_label(ui: &egui::Ui, p: &Palette, rect: Rect, name: &str, badge: Option<&str>, hover: f32) {
    let text = lerp_color(p.text, Color32::WHITE, hover * 0.3);
    widgets::text_elided(
        ui.painter(),
        pos2(
            rect.left() + 12.0,
            rect.bottom() - (if badge.is_some() { 34.0 } else { 22.0 }),
        ),
        name,
        FontId::proportional(13.5),
        text,
        rect.width() - 24.0,
    );
    if let Some(badge) = badge {
        ui.painter().text(
            pos2(rect.left() + 12.0, rect.bottom() - 14.0),
            Align2::LEFT_CENTER,
            badge,
            FontId::proportional(11.5),
            p.accent,
        );
    }
}

/// The accent outline on something worn.
fn worn_frame(ui: &egui::Ui, p: &Palette, rect: Rect, worn: bool) {
    if worn {
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(12),
            egui::Stroke::new(2.0, p.accent),
            egui::StrokeKind::Inside,
        );
    }
}

fn section_label(ui: &mut egui::Ui, p: &Palette, text: &str) {
    ui.label(RichText::new(text.to_uppercase()).small().color(p.muted));
    ui.add_space(4.0);
}

/// The frame to show now; keeps repainting while a cape is animated.
fn animate(ctx: &egui::Context, texture: &super::SkinTexture, now: f64) -> egui::TextureId {
    if texture.animated() {
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(texture.frame_secs));
    }
    texture.id_at(now)
}

/// A cape card: the cape's back, big, and its name; an icon for "none"
/// or "your own".
fn cape_card(
    ui: &mut egui::Ui,
    p: &Palette,
    texture: Option<egui::TextureId>,
    empty_icon: Icon,
    name: &str,
    badge: Option<&str>,
    active: bool,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(CAPE_CARD.into(), Sense::click());
    let hover = card_back(ui, p, rect, &response);
    let area = Rect::from_min_max(rect.min + vec2(10.0, 12.0), rect.max - vec2(10.0, 48.0));
    // The back of the cape is 10×16 (it scales with HD capes).
    let h = area.height().min(area.width() * 1.6);
    let cape = Rect::from_center_size(area.center(), vec2(h / 1.6, h));
    match texture {
        Some(tex) => {
            let uv =
                Rect::from_min_max(pos2(1.0 / 64.0, 1.0 / 32.0), pos2(11.0 / 64.0, 17.0 / 32.0));
            ui.painter().rect_filled(
                cape.translate(vec2(3.0, 4.0)),
                CornerRadius::same(3),
                Color32::from_black_alpha(60),
            );
            ui.painter().image(tex, cape, uv, Color32::WHITE);
        }
        None => {
            let r = Rect::from_center_size(area.center(), vec2(22.0, 22.0));
            crate::art::icons::draw(ui.painter(), empty_icon, r, p.muted);
        }
    }
    let badge = if active { Some("Wearing") } else { badge };
    card_label(ui, p, rect, name, badge, hover);
    worn_frame(ui, p, rect, active);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// The "this cape can animate" mark in a card's corner: a play button that
/// glows in the accent color while the wearer's Animate switch is on, and
/// goes grey with a slash through it when it is off.
fn animation_badge(ui: &egui::Ui, p: &Palette, card: Rect, on: bool) {
    let painter = ui.painter();
    let chip = Rect::from_min_size(card.right_top() + vec2(-38.0, 8.0), vec2(30.0, 26.0));
    let round = CornerRadius::same(13);
    painter.rect_filled(chip, round, Color32::from_black_alpha(if on { 185 } else { 140 }));
    let ink = if on { p.accent } else { p.muted };
    painter.rect_stroke(
        chip,
        round,
        egui::Stroke::new(1.5, ink.gamma_multiply(if on { 0.9 } else { 0.55 })),
        egui::StrokeKind::Inside,
    );
    let c = chip.center() + vec2(1.0, 0.0);
    let (w, h) = (6.5, 7.5);
    painter.add(egui::Shape::convex_polygon(
        vec![
            pos2(c.x - w * 0.6, c.y - h),
            pos2(c.x - w * 0.6, c.y + h),
            pos2(c.x + w, c.y),
        ],
        ink,
        egui::Stroke::NONE,
    ));
    if !on {
        painter.line_segment(
            [
                chip.left_bottom() + vec2(6.0, -5.0),
                chip.right_top() + vec2(-6.0, 5.0),
            ],
            egui::Stroke::new(2.4, p.muted),
        );
    }
}

/// The 3D cosmetics, sculpted ones first, each once (a cuboid item that also
/// has a sculpted version is listed as the sculpted one).
#[derive(Debug, Clone)]
pub(super) struct Entry3d {
    pub id: String,
    pub name: String,
    pub slot: String,
    pub shape: Shape3d,
}

#[derive(Debug, Clone)]
pub(super) enum Shape3d {
    /// The model and the hash of its texture.
    Cuboid { geometry: Geometry, texture: String },
    /// The mesh and the hash it is stored under (its textures are keyed by it).
    Mesh { mesh: std::sync::Arc<Mesh>, hash: String },
}

pub(super) fn cosmetic_entries(state: &crate::skin_tasks::ArcticState) -> Vec<Entry3d> {
    let mut out: Vec<Entry3d> = state
        .meshes
        .iter()
        .map(|(item, mesh)| Entry3d {
            id: item.id.clone(),
            name: item.name.clone(),
            slot: item.slot.clone(),
            shape: Shape3d::Mesh {
                mesh: mesh.clone(),
                hash: item.mesh.clone(),
            },
        })
        .collect();
    for (item, geometry) in &state.items {
        if !out.iter().any(|e| e.id == item.id) {
            out.push(Entry3d {
                id: item.id.clone(),
                name: item.name.clone(),
                slot: item.slot.clone(),
                shape: Shape3d::Cuboid {
                    geometry: geometry.clone(),
                    texture: item.texture.clone(),
                },
            });
        }
    }
    out
}

/// What the preview wears: cuboid cosmetics and sculpted ones.
#[derive(Default)]
pub(super) struct WornModels {
    pub cuboids: Vec<(Geometry, egui::TextureId)>,
    pub meshes: Vec<(std::sync::Arc<Mesh>, Vec<egui::TextureId>)>,
    pub white: egui::TextureId,
}

/// What a cosmetic card draws.
enum CardModel {
    Cuboid(Geometry, Option<egui::TextureId>),
    Mesh(std::sync::Arc<Mesh>, Vec<egui::TextureId>, egui::TextureId),
}

/// A cosmetic card: the model at a three-quarter angle (turning, and
/// moving as its idle animation does, while pointed at), its name, and
/// "Wearing" when worn.
fn cosmetic_card(
    ui: &mut egui::Ui,
    p: &Palette,
    model_to_draw: &CardModel,
    name: &str,
    worn: bool,
    now: f64,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(COSMETIC_CARD.into(), Sense::click());
    let hover = card_back(ui, p, rect, &response);
    let model_rect = Rect::from_min_max(rect.min + vec2(14.0, 12.0), rect.max - vec2(14.0, 48.0));
    let yaw = if response.hovered() {
        CARD_YAW + (now * TURN_SPEED) as f32
    } else {
        CARD_YAW
    };
    match model_to_draw {
        CardModel::Cuboid(geometry, Some(texture)) => model::paint_cosmetic(
            ui.painter(),
            model_rect,
            model::Worn {
                geometry,
                texture: *texture,
            },
            yaw,
        ),
        CardModel::Mesh(mesh, textures, white) => {
            let moving = response.hovered() && !arctic_core::cosmetics::reduce_cape_motion();
            model::paint_mesh_cosmetic(
                ui.painter(),
                model_rect,
                model::WornMesh {
                    mesh,
                    textures,
                    white: *white,
                    time: moving.then_some(now),
                    swing: 0.0,
                },
                yaw,
            );
        }
        CardModel::Cuboid(_, None) => {
            ui.painter()
                .circle_filled(model_rect.center(), 4.0, p.muted);
        }
    }
    card_label(ui, p, rect, name, worn.then_some("Wearing"), hover);
    worn_frame(ui, p, rect, worn);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}
