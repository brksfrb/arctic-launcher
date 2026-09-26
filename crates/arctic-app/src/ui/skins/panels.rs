//! Layout of the Skins tab: preview with actions, capes and the library.

use arctic_core::cosmetics::CapeChoice;
use arctic_core::skins::{SkinEntry, Variant};
use eframe::egui::{
    self, Align2, CornerRadius, FontId, Rect, RichText, Sense, Stroke, StrokeKind, pos2, vec2,
};

use super::model::{self, Pose};
use super::{Selection, SkinsView};
use crate::app::ArcticApp;
use crate::art::icons::Icon;
use crate::art::lerp_color;
use crate::skin_tasks::SkinChange;
use crate::theme::{self, Palette};
use crate::widgets;

const PREVIEW: [f32; 2] = [300.0, 320.0];
const TILE: [f32; 2] = [118.0, 158.0];
const CAPE_TILE: f32 = 54.0;

/// What "you" look like to others: Arctic skin first, then Minecraft's.
struct CurrentLook {
    skin_key: Option<String>,
    variant: Option<Variant>,
    cape_key: Option<String>,
    arctic_skin: bool,
}

impl ArcticApp {
    pub(super) fn skins_page(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        widgets::page_header(
            ui,
            p,
            "Skins",
            "Your look is shown to every Arctic player, on any account. Everything is free.",
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
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.skins.view, SkinsView::Library, "My skins");
                        ui.selectable_value(&mut self.skins.view, SkinsView::Gallery, "Gallery");
                    });
                    ui.add_space(6.0);
                    match self.skins.view {
                        SkinsView::Library => self.library_panel(ui),
                        SkinsView::Gallery => self.gallery_panel(ui),
                    }
                });
            });
        });
    }

    fn account_notice(&mut self, ui: &mut egui::Ui, p: &Palette) {
        let Some(account) = self.accounts.active() else {
            ui.label(
                RichText::new("Add an account to wear skins. You can still collect them below.")
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

    fn current_look(&self) -> CurrentLook {
        let arctic = self.arctic_state();
        let mc = self
            .accounts
            .active()
            .and_then(|a| self.skins.account.get(&a.id))
            .and_then(|r| r.as_ref().ok());
        let arctic_skin = arctic.and_then(|s| s.look.skin.clone());
        let (skin_key, variant) = match (&arctic_skin, mc) {
            (Some(hash), _) => (
                Some(format!("askin:{hash}")),
                arctic.map(|s| s.look.variant()),
            ),
            (None, Some(mc)) if mc.skin_png.is_some() => (
                Some("mc".to_owned()),
                mc.profile.active_skin().map(|s| s.variant()),
            ),
            _ => (None, None),
        };
        let cape_key = match arctic.and_then(|s| s.look.cape.clone()) {
            Some(hash) => Some(format!("acape:{hash}")),
            None => mc
                .and_then(|m| m.profile.active_cape())
                .map(|c| format!("mccape:{}", c.id)),
        };
        CurrentLook {
            skin_key,
            variant,
            cape_key,
            arctic_skin: arctic_skin.is_some(),
        }
    }

    fn preview_panel(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let ctx = ui.ctx().clone();
        let current = self.current_look();
        // Nothing worn yet: preview the newest library skin.
        if self.skins.selection == Selection::Current
            && current.skin_key.is_none()
            && !self.skins.arctic_busy
            && let Some(first) = self.skins.library.skins.first()
        {
            self.skins.selection = Selection::Library(first.id.clone());
        }
        let (rect, response) = ui.allocate_exact_size(PREVIEW.into(), Sense::drag());
        if response.dragged() {
            let d = response.drag_delta();
            self.skins.yaw += d.x * 0.012;
            self.skins.pitch = model::clamp_pitch(self.skins.pitch + d.y * 0.008);
        }
        let entry = self.selected_entry();
        let gallery_item = match &self.skins.selection {
            Selection::Gallery(item) => Some(item.clone()),
            _ => None,
        };
        let key = match (&entry, &gallery_item) {
            (Some(e), _) => Some(format!("lib:{}", e.id)),
            (None, Some(g)) => Some(format!("gal:{}", g.texture)),
            _ => current.skin_key.clone(),
        };
        let now = ui.input(|i| i.time);
        let cape = current
            .cape_key
            .as_deref()
            .and_then(|k| self.skin_texture(&ctx, k).map(|t| t.id_at(now)));
        let pose = Pose {
            yaw: self.skins.yaw,
            pitch: self.skins.pitch,
            swing: model::idle_swing(ui.input(|i| i.time)),
        };
        match key.as_deref().and_then(|k| self.skin_texture(&ctx, k)) {
            Some(tex) => {
                let variant = entry
                    .as_ref()
                    .map(|e| e.variant)
                    .or(gallery_item.as_ref().map(|g| g.variant()))
                    .or(current.variant)
                    .unwrap_or(tex.guessed);
                model::paint(
                    ui.painter(),
                    rect,
                    tex.handle.id(),
                    variant,
                    tex.overlay,
                    cape,
                    pose,
                );
                ctx.request_repaint();
            }
            None => {
                let busy = self.skins.arctic_busy || self.skins.busy;
                let text = if busy { "Loading…" } else { "No skin yet" };
                ui.painter().text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    text,
                    FontId::proportional(15.0),
                    p.muted,
                );
            }
        }
        ui.label(RichText::new("Drag to turn").small().color(p.muted));
        ui.add_space(6.0);
        match (entry, gallery_item) {
            (Some(entry), _) => self.library_actions(ui, &entry),
            (None, Some(item)) => self.gallery_actions(ui, &item),
            _ => self.current_actions(ui, &current),
        }
        self.arctic_capes_row(ui);
        self.minecraft_capes_row(ui);
    }

    fn current_actions(&mut self, ui: &mut egui::Ui, current: &CurrentLook) {
        let p = self.palette();
        ui.label(RichText::new("Your look").size(18.0).strong().color(p.text));
        let source = if current.arctic_skin {
            "Arctic skin"
        } else if current.skin_key.is_some() {
            "Your Minecraft skin"
        } else {
            "Pick a skin from your library and press Wear."
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new(source).color(p.muted));
            if self.skins.arctic_busy || self.skins.busy {
                ui.spinner();
            }
        });
        ui.horizontal(|ui| {
            if let Some(key) = &current.skin_key
                && let Some(png) = self.skin_png(key)
                && widgets::button(ui, p, Some(Icon::Plus), "Save to library", false).clicked()
            {
                let name = self
                    .accounts
                    .active()
                    .map_or("Skin".to_owned(), |a| a.username.clone());
                self.add_skin(&name, &png, current.variant);
            }
            let can = self.arctic_state().is_some() && !self.skins.arctic_busy;
            if current.arctic_skin
                && ui
                    .add_enabled(can, egui::Button::new("Stop wearing"))
                    .on_hover_text("Go back to your Minecraft skin for Arctic players")
                    .clicked()
            {
                self.change_look(|l| l.skin = None);
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

    fn library_actions(&mut self, ui: &mut egui::Ui, entry: &SkinEntry) {
        let p = self.palette();
        let id = entry.id.clone();
        match self.skins.renaming.clone() {
            Some((rid, mut name)) if rid == id => {
                let field = ui.add(
                    widgets::text_field(&mut name)
                        .char_limit(48)
                        .desired_width(f32::INFINITY),
                );
                if field.lost_focus() {
                    let trimmed = name.trim().to_owned();
                    if !trimmed.is_empty() {
                        self.update_skin(&id, |e| e.name = trimmed);
                    }
                    self.skins.renaming = None;
                } else {
                    self.skins.renaming = Some((rid, name));
                    field.request_focus();
                }
            }
            _ => {
                let title = ui.add(
                    egui::Label::new(RichText::new(&entry.name).size(18.0).strong().color(p.text))
                        .sense(Sense::click()),
                );
                if title.on_hover_text("Click to rename").clicked() {
                    self.skins.renaming = Some((id.clone(), entry.name.clone()));
                }
            }
        }
        ui.horizontal(|ui| {
            for v in [Variant::Classic, Variant::Slim] {
                if ui.selectable_label(entry.variant == v, v.label()).clicked()
                    && entry.variant != v
                {
                    self.update_skin(&id, |e| e.variant = v);
                }
            }
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let can = self.arctic_state().is_some() && !self.skins.arctic_busy;
            let wear = ui
                .add_enabled_ui(can, |ui| {
                    widgets::button(ui, p, Some(Icon::Check), "Wear", true)
                })
                .inner
                .on_hover_text("Every Arctic player sees you with this skin");
            if wear.clicked() {
                self.wear_skin(entry);
            }
            if self.can_change_minecraft_skin()
                && ui
                    .button("Set as Minecraft skin")
                    .on_hover_text("Changes your real Minecraft skin, seen by everyone")
                    .clicked()
            {
                self.set_minecraft_skin(entry);
            }
            if widgets::icon_button(ui, p, Icon::Trash, "Delete from library").clicked() {
                self.remove_skin(&id);
            }
        });
        ui.horizontal(|ui| {
            self.share_button(ui, entry);
        });
    }

    /// Arctic capes: presets, your own image, or none.
    fn arctic_capes_row(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let Some(state) = self.arctic_state().cloned() else {
            return;
        };
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("CAPE").small().color(p.muted));
            if self.skins.arctic_busy {
                ui.spinner();
            }
        });
        let ctx = ui.ctx().clone();
        let worn = state.look.cape.clone();
        let mut pick: Option<Option<CapeChoice>> = None;
        let mut upload = false;
        ui.horizontal_wrapped(|ui| {
            if cape_tile(ui, p, None, Icon::Close, "No cape", worn.is_none()) && worn.is_some() {
                pick = Some(None);
            }
            let now = ui.input(|i| i.time);
            for preset in &state.presets {
                let tex = self
                    .skin_texture(&ctx, &format!("acape:{}", preset.texture))
                    .map(|t| animate(&ctx, t, now));
                let active = worn.as_deref() == Some(preset.texture.as_str());
                if cape_tile(ui, p, tex, Icon::Close, &preset.name, active) && !active {
                    pick = Some(Some(CapeChoice::Preset(preset.id.clone())));
                }
            }
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
        if upload && !self.skins.arctic_busy {
            self.tasks.pick_cape_file();
        }
        if let Some(choice) = pick
            && !self.skins.arctic_busy
        {
            self.set_arctic_cape(choice);
        }
    }

    /// Official Minecraft capes the account owns (Microsoft only).
    fn minecraft_capes_row(&mut self, ui: &mut egui::Ui) {
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
        ui.add_space(10.0);
        ui.label(RichText::new("MINECRAFT CAPES").small().color(p.muted));
        let ctx = ui.ctx().clone();
        let mut pick: Option<Option<String>> = None;
        ui.horizontal_wrapped(|ui| {
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

    fn library_panel(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        ui.horizontal(|ui| {
            ui.label(RichText::new("Library").size(18.0).strong().color(p.text));
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
        });
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
        ui.add_space(8.0);
        let entries = self.skins.library.skins.clone();
        let current = self.current_look();
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
            if let Some(key) = &current.skin_key {
                let selected = self.skins.selection == Selection::Current;
                if self.skin_tile(ui, key, "You", current.variant, selected) {
                    self.skins.selection = Selection::Current;
                }
            }
            for e in &entries {
                let selected = self.skins.selection == Selection::Library(e.id.clone());
                if self.skin_tile(
                    ui,
                    &format!("lib:{}", e.id),
                    &e.name,
                    Some(e.variant),
                    selected,
                ) {
                    self.skins.selection = Selection::Library(e.id.clone());
                }
            }
        });
        if entries.is_empty() {
            ui.add_space(8.0);
            ui.label(
                RichText::new(
                    "Drop .png skin files here, pick one with Add skin, or copy a player's skin.",
                )
                .color(p.muted),
            );
        }
    }

    /// Small 3D thumbnail with a name. Returns true when clicked.
    pub(super) fn skin_tile(
        &mut self,
        ui: &mut egui::Ui,
        key: &str,
        name: &str,
        variant: Option<Variant>,
        selected: bool,
    ) -> bool {
        let p = self.palette();
        let ctx = ui.ctx().clone();
        let (rect, response) = ui.allocate_exact_size(TILE.into(), Sense::click());
        let hover = ctx.animate_bool(response.id.with("h"), response.hovered());
        let fill = lerp_color(p.bg, p.surface_hover, 0.35 + 0.5 * hover);
        ui.painter().rect_filled(rect, CornerRadius::same(12), fill);
        let stroke = if selected {
            Stroke::new(2.0, p.accent)
        } else {
            Stroke::new(1.0, p.card_stroke)
        };
        ui.painter()
            .rect_stroke(rect, CornerRadius::same(12), stroke, StrokeKind::Inside);
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
        response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
    }

    fn selected_entry(&self) -> Option<SkinEntry> {
        match &self.skins.selection {
            Selection::Library(id) => self
                .skins
                .library
                .skins
                .iter()
                .find(|s| &s.id == id)
                .cloned(),
            _ => None,
        }
    }

    fn can_change_minecraft_skin(&self) -> bool {
        !self.skins.busy
            && self.accounts.active().is_some_and(|a| {
                a.is_microsoft() && matches!(self.skins.account.get(&a.id), Some(Ok(_)))
            })
    }
}

/// Cape front, or an icon for "none"/"upload". Returns true when clicked.
/// The frame to show now; keeps repainting while a cape is animated.
fn animate(ctx: &egui::Context, texture: &super::SkinTexture, now: f64) -> egui::TextureId {
    if texture.animated() {
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(
            arctic_core::cosmetics::CAPE_FRAME_SECS,
        ));
    }
    texture.id_at(now)
}

fn cape_tile(
    ui: &mut egui::Ui,
    p: &Palette,
    texture: Option<egui::TextureId>,
    empty_icon: Icon,
    name: &str,
    active: bool,
) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(CAPE_TILE, CAPE_TILE * 1.3), Sense::click());
    let hover = ui
        .ctx()
        .animate_bool(response.id.with("h"), response.hovered());
    ui.painter().rect_filled(
        rect,
        CornerRadius::same(8),
        lerp_color(p.bg, p.surface_hover, 0.4 + 0.5 * hover),
    );
    let inner = rect.shrink2(vec2(12.0, 8.0));
    match texture {
        Some(tex) => {
            // Cape front: (1,1) 10×16 of the 64×32 layout (UVs scale with HD capes).
            let uv =
                Rect::from_min_max(pos2(1.0 / 64.0, 1.0 / 32.0), pos2(11.0 / 64.0, 17.0 / 32.0));
            ui.painter().image(tex, inner, uv, egui::Color32::WHITE);
        }
        None => {
            let r = Rect::from_center_size(inner.center(), vec2(16.0, 16.0));
            crate::art::icons::draw(ui.painter(), empty_icon, r, p.muted);
        }
    }
    let stroke = if active {
        Stroke::new(2.0, p.accent)
    } else {
        Stroke::new(1.0, p.card_stroke)
    };
    ui.painter()
        .rect_stroke(rect, CornerRadius::same(8), stroke, StrokeKind::Inside);
    response
        .on_hover_text(name)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}
