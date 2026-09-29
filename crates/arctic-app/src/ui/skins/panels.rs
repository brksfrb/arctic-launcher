//! Layout of the Cosmetics tab: the live preview on the left, and on the
//! right skins, capes and 3D cosmetics. One click wears something.

use arctic_core::cosmetics::CapeChoice;
use arctic_core::skins::{SkinEntry, Variant};
use eframe::egui::{
    self, Align2, CornerRadius, FontId, Rect, RichText, Sense, Stroke, StrokeKind, pos2, vec2,
};

use super::model::{self, Pose};
use super::{Section, SkinsView};
use crate::app::ArcticApp;
use crate::art::icons::Icon;
use crate::art::lerp_color;
use crate::skin_tasks::SkinChange;
use crate::theme::{self, Palette};
use crate::widgets;

const PREVIEW: [f32; 2] = [300.0, 340.0];
const TILE: [f32; 2] = [118.0, 158.0];
const CAPE_TILE: [f32; 2] = [72.0, 94.0];
const COSMETIC_TILE: [f32; 2] = [104.0, 128.0];
/// How long "Saved" shows after a change is saved.
const SAVED_NOTE_SECS: f64 = 3.0;
/// Cosmetic slots in the order they're listed.
const SLOTS: [&str; 5] = ["head", "face", "back", "shoulders", "body"];

/// What "you" look like to others: Arctic skin first, then Minecraft's.
/// Changes still being saved are already in it.
pub(super) struct CurrentLook {
    pub skin_key: Option<String>,
    pub variant: Option<Variant>,
    pub cape_key: Option<String>,
    pub arctic_skin: bool,
}

impl ArcticApp {
    pub(super) fn skins_page(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        widgets::page_header(
            ui,
            p,
            "Cosmetics",
            "Skins, capes and cosmetics every Arctic player sees, on any account.",
        );
        self.account_notice(ui, p);
        ui.horizontal_top(|ui| {
            theme::card(p).show(ui, |ui| {
                ui.set_width(PREVIEW[0]);
                ui.vertical(|ui| self.preview_panel(ui));
            });
            ui.add_space(12.0);
            theme::card(p).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.vertical(|ui| {
                    widgets::field_row(ui, |ui| {
                        let s = &mut self.skins.section;
                        ui.selectable_value(s, Section::Skins, "Skins");
                        ui.selectable_value(s, Section::Capes, "Capes");
                        ui.selectable_value(s, Section::Cosmetics, "Cosmetics");
                    });
                    ui.add_space(6.0);
                    match self.skins.section {
                        Section::Skins => self.skins_section(ui),
                        Section::Capes => self.capes_section(ui),
                        Section::Cosmetics => self.cosmetics_section(ui),
                    }
                });
            });
        });
    }

    fn account_notice(&mut self, ui: &mut egui::Ui, p: &Palette) {
        let Some(account) = self.accounts.active() else {
            ui.label(
                RichText::new(
                    "Add an account to wear cosmetics. You can still collect skins below.",
                )
                .color(p.muted),
            );
            ui.add_space(6.0);
            return;
        };
        let error = self
            .skins
            .arctic
            .get(&account.id)
            .and_then(|r| r.as_ref().err().cloned());
        if let Some(e) = error {
            ui.horizontal(|ui| {
                // Connection problems get a plain message; details go to the log.
                let text = if e.starts_with("network error") {
                    log::info!("looks server: {e}");
                    "Can't reach the Arctic looks server right now.".to_owned()
                } else {
                    format!("Arctic looks are unavailable: {e}")
                };
                ui.label(RichText::new(text).color(p.muted));
                if ui.link("Retry").clicked() {
                    self.request_skin_state(true);
                }
            });
            ui.add_space(6.0);
        }
    }

    pub(super) fn current_look(&self) -> CurrentLook {
        let arctic = self.arctic_state();
        let mc = self
            .accounts
            .active()
            .and_then(|a| self.skins.account.get(&a.id))
            .and_then(|r| r.as_ref().ok());
        let minecraft_skin = || match mc {
            Some(mc) if mc.skin_png.is_some() => (
                Some("mc".to_owned()),
                mc.profile.active_skin().map(|s| s.variant()),
            ),
            _ => (None, None),
        };
        let (skin_key, variant, arctic_skin) = match &self.skins.trying.skin {
            Some(Some((key, v))) => (Some(key.clone()), Some(*v), true),
            Some(None) => {
                let (k, v) = minecraft_skin();
                (k, v, false)
            }
            None => match arctic.and_then(|s| s.look.skin.clone()) {
                Some(hash) => (
                    Some(format!("askin:{hash}")),
                    arctic.map(|s| s.look.variant()),
                    true,
                ),
                None => {
                    let (k, v) = minecraft_skin();
                    (k, v, false)
                }
            },
        };
        let minecraft_cape = || {
            mc.and_then(|m| m.profile.active_cape())
                .map(|c| format!("mccape:{}", c.id))
        };
        let cape_key = match &self.skins.trying.cape {
            Some(Some(key)) => Some(key.clone()),
            Some(None) => minecraft_cape(),
            None => match arctic.and_then(|s| s.look.cape.clone()) {
                Some(hash) => Some(format!("acape:{hash}")),
                None => minecraft_cape(),
            },
        };
        CurrentLook {
            skin_key,
            variant,
            cape_key,
            arctic_skin,
        }
    }

    /// Worn cosmetic ids, including a change still being saved.
    fn worn_ids(&self) -> Vec<String> {
        match (&self.skins.trying.cosmetics, self.arctic_state()) {
            (Some(ids), _) => ids.clone(),
            (None, Some(state)) => state.look.cosmetics.clone(),
            (None, None) => Vec::new(),
        }
    }

    fn preview_panel(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let ctx = ui.ctx().clone();
        let current = self.current_look();
        let (rect, response) = ui.allocate_exact_size(PREVIEW.into(), Sense::drag());
        if response.dragged() {
            let d = response.drag_delta();
            self.skins.yaw += d.x * 0.012;
            self.skins.pitch = model::clamp_pitch(self.skins.pitch + d.y * 0.008);
        }
        // No skin yet: a plain figure, so a cape or cosmetics still show.
        let key = current
            .skin_key
            .clone()
            .unwrap_or_else(|| super::MANNEQUIN.to_owned());
        let now = ui.input(|i| i.time);
        let cape = current
            .cape_key
            .as_deref()
            .and_then(|k| self.skin_texture(&ctx, k).map(|t| t.id_at(now)));
        let pose = Pose {
            yaw: self.skins.yaw,
            pitch: self.skins.pitch,
            swing: model::idle_swing(now),
        };
        let wearing = self.worn_cosmetics(&ctx);
        let worn: Vec<model::Worn> = wearing
            .iter()
            .map(|(geometry, texture)| model::Worn {
                geometry,
                texture: *texture,
            })
            .collect();
        match self.skin_texture(&ctx, &key) {
            Some(tex) => {
                let variant = current.variant.unwrap_or(tex.guessed);
                model::paint(
                    ui.painter(),
                    rect,
                    tex.handle.id(),
                    variant,
                    tex.overlay,
                    cape,
                    &worn,
                    pose,
                );
                ctx.request_repaint();
            }
            None => {
                let text = if self.skins.arctic_busy || self.skins.busy {
                    "Loading…"
                } else {
                    "No skin yet"
                };
                ui.painter().text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    text,
                    FontId::proportional(15.0),
                    p.muted,
                );
            }
        }
        ui.horizontal(|ui| {
            ui.label(RichText::new("Drag to turn").small().color(p.muted));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                self.save_note(ui, p);
            });
        });
        ui.add_space(6.0);
        self.current_actions(ui, &current);
    }

    /// "Saving…" while a change is on its way, "Saved" for a moment after.
    fn save_note(&self, ui: &mut egui::Ui, p: &Palette) {
        if self.skins.arctic_busy && self.arctic_state().is_some() {
            ui.label(RichText::new("Saving…").small().color(p.muted));
            ui.spinner();
            return;
        }
        let Some(at) = self.skins.saved_at else {
            return;
        };
        let age = super::now_secs_f64() - at;
        if age < SAVED_NOTE_SECS {
            ui.label(RichText::new("Saved").small().color(p.accent));
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_secs_f64(SAVED_NOTE_SECS - age));
        }
    }

    fn current_actions(&mut self, ui: &mut egui::Ui, current: &CurrentLook) {
        let p = self.palette();
        let source = if current.arctic_skin {
            "Arctic skin"
        } else if current.skin_key.is_some() {
            "Your Minecraft skin"
        } else {
            "Pick a skin on the right to wear it."
        };
        ui.label(RichText::new("Your look").size(18.0).strong().color(p.text));
        ui.label(RichText::new(source).color(p.muted));
        ui.horizontal(|ui| {
            if let Some(key) = &current.skin_key
                && let Some(png) = self.skin_png(key)
                && widgets::button(ui, p, Some(Icon::Plus), "Save to library", false)
                    .on_hover_text("Keep a copy of the skin you're wearing")
                    .clicked()
            {
                let name = self
                    .accounts
                    .active()
                    .map_or("Skin".to_owned(), |a| a.username.clone());
                let _ = self.add_skin(&name, &png, current.variant);
            }
            if self.can_change_minecraft_skin()
                && ui
                    .button("Reset Minecraft skin")
                    .on_hover_text("Go back to the default Minecraft skin (seen by everyone)")
                    .clicked()
            {
                self.change_minecraft_skin(SkinChange::Reset);
            }
        });
    }

    // ---- Skins -------------------------------------------------------------------

    fn skins_section(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        widgets::field_row(ui, |ui| {
            ui.selectable_value(&mut self.skins.view, SkinsView::Library, "My skins");
            ui.selectable_value(&mut self.skins.view, SkinsView::Gallery, "Gallery");
            if self.skins.view == SkinsView::Library {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::button(ui, p, Some(Icon::Plus), "Add skin…", true).clicked() {
                        self.tasks.pick_skin_file();
                    }
                    if widgets::icon_button(ui, p, Icon::Folder, "Open skins folder").clicked() {
                        let dir = self.skins_dir();
                        let _ = std::fs::create_dir_all(&dir);
                        let _ = open::that_detached(&dir);
                    }
                });
            }
        });
        ui.add_space(4.0);
        match self.skins.view {
            SkinsView::Library => self.library_panel(ui),
            SkinsView::Gallery => self.gallery_panel(ui),
        }
    }

    fn library_panel(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
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
        ui.add_space(8.0);
        let entries = self.skins.library.skins.clone();
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
            ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
            // Your Minecraft skin: wearing it means no Arctic skin.
            let mc_key = if current.arctic_skin || current.skin_key.is_none() {
                self.skin_png("mc").map(|_| "mc".to_owned())
            } else {
                current.skin_key.clone()
            };
            let key = mc_key.unwrap_or_else(|| super::MANNEQUIN.to_owned());
            let tile = self.skin_tile(ui, &key, "Minecraft skin", None, !current.arctic_skin);
            if tile
                .on_hover_text("Your own Minecraft skin, for Arctic players too")
                .clicked()
                && current.arctic_skin
                && can_wear
            {
                stop = true;
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
        ui.add_space(6.0);
        let hint = if entries.is_empty() {
            "Drop .png skin files here, pick one with Add skin, or copy a player's skin."
        } else {
            "Click a skin to wear it. Right-click one to rename, share or delete it."
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

    /// Small 3D thumbnail with a name, outlined while worn.
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
        let (rect, response) = ui.allocate_exact_size(TILE.into(), Sense::click());
        let hover = ctx.animate_bool(response.id.with("h"), response.hovered());
        let fill = lerp_color(p.bg, p.surface_hover, 0.35 + 0.5 * hover);
        ui.painter().rect_filled(rect, CornerRadius::same(12), fill);
        let model_rect = Rect::from_min_size(
            rect.min + vec2(8.0, 8.0),
            vec2(rect.width() - 16.0, rect.height() - 38.0),
        );
        if let Some(tex) = self.skin_texture(&ctx, key) {
            let pose = Pose {
                yaw: 0.45,
                pitch: 0.1,
                swing: 0.0,
            };
            let variant = variant.unwrap_or(tex.guessed);
            model::paint(
                ui.painter(),
                model_rect,
                tex.handle.id(),
                variant,
                tex.overlay,
                None,
                &[],
                pose,
            );
        }
        widgets::text_elided(
            ui.painter(),
            pos2(rect.left() + 10.0, rect.bottom() - 16.0),
            name,
            FontId::proportional(13.0),
            p.text,
            rect.width() - 20.0,
        );
        worn_frame(ui, p, rect, worn, CornerRadius::same(12));
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    }

    // ---- Capes -------------------------------------------------------------------

    fn capes_section(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        self.arctic_capes(ui);
        self.minecraft_capes(ui);
        ui.add_space(6.0);
        ui.label(
            RichText::new("Arctic capes are seen by Arctic players; a Minecraft cape shows when you wear no Arctic cape.")
                .small()
                .color(p.muted),
        );
    }

    /// Arctic capes: presets, your own image, or none.
    fn arctic_capes(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let Some(state) = self.arctic_state().cloned() else {
            ui.label(RichText::new("Arctic capes show once your look has loaded.").color(p.muted));
            return;
        };
        ui.label(RichText::new("ARCTIC CAPES").small().color(p.muted));
        let ctx = ui.ctx().clone();
        let worn = match &self.skins.trying.cape {
            Some(Some(key)) => key.strip_prefix("acape:").map(str::to_owned),
            Some(None) => None,
            None => state.look.cape.clone(),
        };
        let mut pick: Option<(Option<CapeChoice>, Option<String>)> = None;
        let mut upload = false;
        let now = ui.input(|i| i.time);
        let (animated, still): (Vec<_>, Vec<_>) = state.presets.iter().partition(|c| c.frames > 1);
        // Tiles for these presets; the one clicked, if any.
        type Picked = Option<(Option<CapeChoice>, Option<String>)>;
        let mut presets = |ui: &mut egui::Ui, list: &[&arctic_core::cosmetics::Preset]| -> Picked {
            let mut clicked = None;
            for preset in list {
                let key = format!("acape:{}", preset.texture);
                let tex = self.skin_texture(&ctx, &key).map(|t| animate(&ctx, t, now));
                let active = worn.as_deref() == Some(preset.texture.as_str());
                if cape_tile(ui, p, tex, Icon::Close, &preset.name, active) && !active {
                    clicked = Some((Some(CapeChoice::Preset(preset.id.clone())), Some(key)));
                }
            }
            clicked
        };
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
            if cape_tile(ui, p, None, Icon::Close, "No cape", worn.is_none()) && worn.is_some() {
                pick = Some((None, None));
            }
            if let Some(clicked) = presets(ui, &still) {
                pick = Some(clicked);
            }
        });
        if !animated.is_empty() {
            ui.add_space(4.0);
            ui.label(RichText::new("ANIMATED").small().color(p.muted));
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
                if let Some(clicked) = presets(ui, &animated) {
                    pick = Some(clicked);
                }
            });
        }
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
            let custom = worn
                .as_ref()
                .filter(|h| !state.presets.iter().any(|p| &p.texture == *h));
            if let Some(hash) = custom {
                let tex = self
                    .skin_texture(&ctx, &format!("acape:{hash}"))
                    .map(|t| animate(&ctx, t, now));
                cape_tile(ui, p, tex, Icon::Close, "Your cape", true);
            }
            if cape_tile(
                ui,
                p,
                None,
                Icon::Plus,
                "Use your own cape image (64×32 PNG; stack up to 8 frames to animate it)",
                false,
            ) {
                upload = true;
            }
        });
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
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("MINECRAFT CAPES").small().color(p.muted));
            if self.skins.busy {
                ui.spinner();
            }
        });
        let ctx = ui.ctx().clone();
        let mut pick: Option<Option<String>> = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
            if cape_tile(ui, p, None, Icon::Close, "No Minecraft cape", none_active) && !none_active
            {
                pick = Some(None);
            }
            for (id, alias, active) in &capes {
                let tex = self
                    .skin_texture(&ctx, &format!("mccape:{id}"))
                    .map(|t| t.handle.id());
                if cape_tile(ui, p, tex, Icon::Close, alias, *active) && !active {
                    pick = Some(Some(id.clone()));
                }
            }
        });
        if let Some(choice) = pick
            && !self.skins.busy
        {
            self.change_minecraft_skin(SkinChange::Cape(choice));
        }
    }

    // ---- 3D cosmetics ------------------------------------------------------------

    /// The Arctic cosmetics being worn, with textures (for the preview).
    fn worn_cosmetics(
        &mut self,
        ctx: &egui::Context,
    ) -> Vec<(arctic_core::cosmetic_models::Geometry, egui::TextureId)> {
        let Some(state) = self.arctic_state().cloned() else {
            return Vec::new();
        };
        let ids = self.worn_ids();
        state
            .items
            .iter()
            .filter(|(item, _)| ids.contains(&item.id))
            .filter_map(|(item, geometry)| {
                let texture = self.skin_texture(ctx, &format!("acos:{}", item.texture))?;
                Some((geometry.clone(), texture.handle.id()))
            })
            .collect()
    }

    /// Cosmetics by slot; clicking one wears it (replacing the one in its
    /// slot) or takes it off.
    fn cosmetics_section(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let Some(state) = self.arctic_state().cloned() else {
            ui.label(RichText::new("Cosmetics show once your look has loaded.").color(p.muted));
            return;
        };
        if state.items.is_empty() {
            ui.label(RichText::new("No cosmetics on this server yet.").color(p.muted));
            return;
        }
        let ctx = ui.ctx().clone();
        let yaw = (ui.input(|i| i.time) * 0.6) as f32;
        let worn_ids = self.worn_ids();
        let mut pick: Option<Vec<String>> = None;
        for slot in SLOTS {
            let items: Vec<_> = state
                .items
                .iter()
                .filter(|(item, _)| slot_key(&item.slot) == slot)
                .collect();
            if items.is_empty() {
                continue;
            }
            ui.add_space(4.0);
            ui.label(
                RichText::new(slot_name(slot).to_uppercase())
                    .small()
                    .color(p.muted),
            );
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
                for (item, geometry) in items {
                    let worn = worn_ids.contains(&item.id);
                    let texture = self
                        .skin_texture(&ctx, &format!("acos:{}", item.texture))
                        .map(|t| t.handle.id());
                    if cosmetic_tile(ui, p, geometry, texture, &item.name, worn, yaw) {
                        // Keep the others; this item's slot gets it (or nothing).
                        let slot_of = |id: &String| {
                            state
                                .items
                                .iter()
                                .find(|(i, _)| &i.id == id)
                                .map(|(i, _)| slot_key(&i.slot))
                        };
                        let mut next: Vec<String> = worn_ids
                            .iter()
                            .filter(|id| slot_of(id) != Some(slot))
                            .cloned()
                            .collect();
                        if !worn {
                            next.push(item.id.clone());
                        }
                        pick = Some(next);
                    }
                }
            });
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(33));
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("One per slot. Click a worn one to take it off.")
                    .small()
                    .color(p.muted),
            );
            if !worn_ids.is_empty()
                && widgets::button(ui, p, Some(Icon::Close), "Remove all", false).clicked()
            {
                pick = Some(Vec::new());
            }
        });
        if let Some(ids) = pick {
            self.set_cosmetics(ids);
        }
    }

    fn can_change_minecraft_skin(&self) -> bool {
        !self.skins.busy
            && self.accounts.active().is_some_and(|a| {
                a.is_microsoft() && matches!(self.skins.account.get(&a.id), Some(Ok(_)))
            })
    }
}

/// The accent outline on something worn; a faint one otherwise.
fn worn_frame(ui: &egui::Ui, p: &Palette, rect: Rect, worn: bool, radius: CornerRadius) {
    let stroke = if worn {
        Stroke::new(2.0, p.accent)
    } else {
        Stroke::new(1.0, p.card_stroke)
    };
    ui.painter()
        .rect_stroke(rect, radius, stroke, StrokeKind::Inside);
}

/// The frame to show now; keeps repainting while a cape is animated.
fn animate(ctx: &egui::Context, texture: &super::SkinTexture, now: f64) -> egui::TextureId {
    if texture.animated() {
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(
            arctic_core::cosmetics::CAPE_FRAME_SECS,
        ));
    }
    texture.id_at(now)
}

/// Cape front, or an icon for "none"/"upload". Returns true when clicked.
fn cape_tile(
    ui: &mut egui::Ui,
    p: &Palette,
    texture: Option<egui::TextureId>,
    empty_icon: Icon,
    name: &str,
    active: bool,
) -> bool {
    let (rect, response) = ui.allocate_exact_size(CAPE_TILE.into(), Sense::click());
    let hover = ui
        .ctx()
        .animate_bool(response.id.with("h"), response.hovered());
    ui.painter().rect_filled(
        rect,
        CornerRadius::same(8),
        lerp_color(p.bg, p.surface_hover, 0.4 + 0.5 * hover),
    );
    let inner = rect.shrink2(vec2(16.0, 10.0));
    match texture {
        Some(tex) => {
            // Cape front: (1,1) 10×16 of the 64×32 layout (UVs scale with HD capes).
            let uv =
                Rect::from_min_max(pos2(1.0 / 64.0, 1.0 / 32.0), pos2(11.0 / 64.0, 17.0 / 32.0));
            ui.painter().image(tex, inner, uv, egui::Color32::WHITE);
        }
        None => {
            let r = Rect::from_center_size(inner.center(), vec2(18.0, 18.0));
            crate::art::icons::draw(ui.painter(), empty_icon, r, p.muted);
        }
    }
    worn_frame(ui, p, rect, active, CornerRadius::same(8));
    response
        .on_hover_text(name)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

/// The slot a catalog slot name belongs to (unknown ones count as body).
fn slot_key(slot: &str) -> &'static str {
    SLOTS.into_iter().find(|s| *s == slot).unwrap_or("body")
}

/// Words for a cosmetic slot.
fn slot_name(slot: &str) -> &'static str {
    match slot {
        "head" => "Head",
        "face" => "Face",
        "back" => "Back",
        "shoulders" => "Shoulders",
        _ => "Body",
    }
}

/// A cosmetic's tile: its model turning slowly and its name; outlined while worn.
fn cosmetic_tile(
    ui: &mut egui::Ui,
    p: &Palette,
    geometry: &arctic_core::cosmetic_models::Geometry,
    texture: Option<egui::TextureId>,
    name: &str,
    worn: bool,
    yaw: f32,
) -> bool {
    let (rect, response) = ui.allocate_exact_size(COSMETIC_TILE.into(), Sense::click());
    let hover = ui
        .ctx()
        .animate_bool(response.id.with("h"), response.hovered());
    let fill = lerp_color(p.bg, p.surface_hover, 0.35 + 0.5 * hover);
    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(10), fill);
    let model_rect = Rect::from_min_max(rect.min + vec2(6.0, 6.0), rect.max - vec2(6.0, 26.0));
    if let Some(texture) = texture {
        model::paint_cosmetic(
            ui.painter(),
            model_rect,
            model::Worn { geometry, texture },
            yaw,
        );
    }
    widgets::text_elided(
        ui.painter(),
        pos2(rect.left() + 8.0, rect.bottom() - 15.0),
        name,
        FontId::proportional(12.5),
        p.text,
        rect.width() - 16.0,
    );
    worn_frame(ui, p, rect, worn, CornerRadius::same(10));
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}
