//! Layout of the Cosmetics tab: a large live preview with what you're
//! wearing under it, and on the right a rail of categories (skins, capes,
//! each cosmetic slot) next to the list for the one picked. Pointing at
//! something tries it on in the preview; clicking wears it.

use arctic_core::skins::Variant;
use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Rect, RichText, Sense, Stroke, StrokeKind, pos2,
    vec2,
};

use super::model::{self, Pose};
use super::{Hover, Section};
use crate::app::ArcticApp;
use crate::art::icons::Icon;
use crate::art::lerp_color;
use crate::skin_tasks::SkinChange;
use crate::theme::{self, Palette};
use crate::widgets;

const PREVIEW_W: f32 = 300.0;
const PREVIEW_H: f32 = 380.0;
const RAIL_W: f32 = 150.0;
const RAIL_ROW: f32 = 34.0;
/// How long "Saved" shows after a change is saved.
const SAVED_NOTE_SECS: f64 = 3.0;
/// The preview turns by itself this long after it was last dragged.
const AUTO_TURN_AFTER: f64 = 4.0;
const AUTO_TURN_SPEED: f32 = 0.35;
const ZOOM: std::ops::RangeInclusive<f32> = 0.8..=1.8;
/// Cosmetic slots in the order they're listed.
pub(super) const SLOTS: [&str; 5] = ["head", "face", "back", "shoulders", "body"];

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
        // What was pointed at last frame is tried on in the preview now.
        let hover = self.skins.hover.take();
        ui.horizontal_top(|ui| {
            theme::card(p).show(ui, |ui| {
                ui.set_width(PREVIEW_W);
                ui.vertical(|ui| {
                    self.preview_panel(ui, hover.as_ref());
                    ui.add_space(10.0);
                    self.equipped_panel(ui);
                });
            });
            ui.add_space(12.0);
            theme::card(p).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(RAIL_W);
                        self.category_rail(ui);
                    });
                    ui.add_space(14.0);
                    ui.vertical(|ui| match self.skins.section {
                        Section::Skins => self.skins_section(ui),
                        Section::Capes => self.capes_section(ui),
                        Section::Slot(slot) => self.cosmetics_section(ui, slot),
                    });
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
    pub(super) fn worn_ids(&self) -> Vec<String> {
        match (&self.skins.trying.cosmetics, self.arctic_state()) {
            (Some(ids), _) => ids.clone(),
            (None, Some(state)) => state.look.cosmetics.clone(),
            (None, None) => Vec::new(),
        }
    }

    /// You, large: drag to turn, scroll to zoom; turns by itself when left alone.
    fn preview_panel(&mut self, ui: &mut egui::Ui, hover: Option<&Hover>) {
        let p = self.palette();
        let ctx = ui.ctx().clone();
        let now = ui.input(|i| i.time);
        let mut current = self.current_look();
        let mut worn_ids = self.worn_ids();
        // Trying on what's pointed at.
        match hover {
            Some(Hover::Skin(key, variant)) => {
                current.skin_key = Some(key.clone());
                current.variant = Some(*variant);
            }
            Some(Hover::Cape(key)) => current.cape_key = key.clone(),
            Some(Hover::Cosmetics(ids)) => worn_ids = ids.clone(),
            None => {}
        }
        let (rect, response) =
            ui.allocate_exact_size(vec2(PREVIEW_W, PREVIEW_H), Sense::click_and_drag());
        if response.dragged() {
            let d = response.drag_delta();
            self.skins.yaw += d.x * 0.012;
            self.skins.pitch = model::clamp_pitch(self.skins.pitch + d.y * 0.008);
            self.skins.dragged_at = now;
        } else if now - self.skins.dragged_at > AUTO_TURN_AFTER && self.skins.dragged_at >= 0.0 {
            let dt = ui.input(|i| i.stable_dt).min(0.1);
            self.skins.yaw += dt * AUTO_TURN_SPEED;
        }
        if response.double_clicked() {
            self.skins.yaw = 0.5;
            self.skins.pitch = 0.12;
            self.skins.zoom = 1.0;
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                let zoom = if self.skins.zoom == 0.0 {
                    1.0
                } else {
                    self.skins.zoom
                };
                self.skins.zoom = (zoom * (1.0 + scroll * 0.002)).clamp(*ZOOM.start(), *ZOOM.end());
            }
        }
        stage(ui.painter(), rect, p);
        let zoom = if self.skins.zoom == 0.0 {
            1.0
        } else {
            self.skins.zoom
        };
        let model_rect = Rect::from_center_size(rect.center() + vec2(0.0, 6.0), rect.size() * zoom);
        let painter = ui.painter().with_clip_rect(rect);
        // No skin yet: a plain figure, so a cape or cosmetics still show.
        let key = current
            .skin_key
            .clone()
            .unwrap_or_else(|| super::MANNEQUIN.to_owned());
        let cape = current
            .cape_key
            .as_deref()
            .and_then(|k| self.skin_texture(&ctx, k).map(|t| t.id_at(now)));
        let pose = Pose {
            yaw: self.skins.yaw,
            pitch: self.skins.pitch,
            swing: model::idle_swing(now),
        };
        let wearing = self.cosmetic_models(&ctx, &worn_ids);
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
                    &painter,
                    model_rect,
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
                painter.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    text,
                    FontId::proportional(15.0),
                    p.muted,
                );
            }
        }
        if hover.is_some() {
            painter.text(
                rect.left_top() + vec2(12.0, 10.0),
                Align2::LEFT_TOP,
                "Trying on",
                FontId::proportional(12.0),
                p.accent,
            );
        }
        response.on_hover_text("Drag to turn · scroll to zoom · double-click to reset");
    }

    /// What you're wearing, each with a way to take it off, and how saving goes.
    fn equipped_panel(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let current = self.current_look();
        ui.horizontal(|ui| {
            ui.label(RichText::new("Equipped").strong().color(p.text));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                self.save_note(ui, p);
            });
        });
        ui.add_space(4.0);
        let skin = if current.arctic_skin {
            "Arctic skin"
        } else if current.skin_key.is_some() {
            "Minecraft skin"
        } else {
            "None"
        };
        if equipped_row(ui, p, "Skin", skin, current.arctic_skin) {
            self.stop_wearing_skin();
        }
        let (cape, arctic_cape) = self.equipped_cape_name();
        if equipped_row(ui, p, "Cape", &cape, arctic_cape) {
            self.set_arctic_cape(None, None);
        }
        let worn = self.worn_ids();
        let names: Vec<(String, String, String)> = match self.arctic_state() {
            Some(state) => worn
                .iter()
                .filter_map(|id| state.items.iter().find(|(i, _)| &i.id == id))
                .map(|(i, _)| (i.id.clone(), slot_name(&i.slot).to_owned(), i.name.clone()))
                .collect(),
            None => Vec::new(),
        };
        let mut remove: Option<String> = None;
        for (id, slot, name) in &names {
            if equipped_row(ui, p, slot, name, true) {
                remove = Some(id.clone());
            }
        }
        if let Some(id) = remove {
            self.set_cosmetics(worn.iter().filter(|w| **w != id).cloned().collect());
        }
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            if names.len() > 1
                && widgets::button(ui, p, Some(Icon::Close), "Remove all", false).clicked()
            {
                self.set_cosmetics(Vec::new());
            }
            if let Some(key) = &current.skin_key
                && let Some(png) = self.skin_png(key)
                && ui
                    .small_button("Save to library")
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
                    .small_button("Reset Minecraft skin")
                    .on_hover_text("Go back to the default Minecraft skin (seen by everyone)")
                    .clicked()
            {
                self.change_minecraft_skin(SkinChange::Reset);
            }
        });
    }

    /// The cape's name for the Equipped list, and whether it's an Arctic one
    /// (which can be taken off here).
    fn equipped_cape_name(&self) -> (String, bool) {
        let state = self.arctic_state();
        let hash = match &self.skins.trying.cape {
            Some(Some(key)) => key.strip_prefix("acape:").map(str::to_owned),
            Some(None) => None,
            None => state.and_then(|s| s.look.cape.clone()),
        };
        match hash {
            Some(hash) => {
                let name = state
                    .and_then(|s| s.presets.iter().find(|p| p.texture == hash))
                    .map_or("Your cape".to_owned(), |p| p.name.clone());
                (name, true)
            }
            None => {
                let mc = self
                    .accounts
                    .active()
                    .and_then(|a| self.skins.account.get(&a.id))
                    .and_then(|r| r.as_ref().ok())
                    .and_then(|m| m.profile.active_cape())
                    .map(|c| format!("{} (Minecraft)", c.alias));
                (mc.unwrap_or_else(|| "None".to_owned()), false)
            }
        }
    }

    /// "Saving…" while a change is on its way, "Saved" for a moment after.
    fn save_note(&self, ui: &mut egui::Ui, p: &Palette) {
        if self.skins.arctic_busy && self.arctic_state().is_some() {
            ui.spinner();
            ui.label(RichText::new("Saving…").small().color(p.muted));
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

    /// Skins, Capes, then one row per cosmetic slot (with how many there are).
    fn category_rail(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let counts: Vec<(&'static str, usize)> = SLOTS
            .iter()
            .map(|slot| {
                let n = self.arctic_state().map_or(0, |s| {
                    s.items
                        .iter()
                        .filter(|(i, _)| slot_key(&i.slot) == *slot)
                        .count()
                });
                (*slot, n)
            })
            .collect();
        let capes = self.arctic_state().map(|s| s.presets.len());
        let mut pick = None;
        if rail_row(ui, p, "Skins", None, self.skins.section == Section::Skins) {
            pick = Some(Section::Skins);
        }
        if rail_row(ui, p, "Capes", capes, self.skins.section == Section::Capes) {
            pick = Some(Section::Capes);
        }
        ui.add_space(10.0);
        ui.label(RichText::new("COSMETICS").small().color(p.muted));
        ui.add_space(2.0);
        for (slot, n) in counts {
            if rail_row(
                ui,
                p,
                slot_name(slot),
                Some(n),
                self.skins.section == Section::Slot(slot),
            ) {
                pick = Some(Section::Slot(slot));
            }
        }
        if let Some(section) = pick
            && section != self.skins.section
        {
            self.skins.section = section;
            self.skins.search.clear();
        }
    }

    pub(super) fn can_change_minecraft_skin(&self) -> bool {
        !self.skins.busy
            && self.accounts.active().is_some_and(|a| {
                a.is_microsoft() && matches!(self.skins.account.get(&a.id), Some(Ok(_)))
            })
    }
}

/// A soft spotlight and floor shadow behind the preview.
fn stage(painter: &egui::Painter, rect: Rect, p: &Palette) {
    painter.rect_filled(
        rect,
        CornerRadius::same(12),
        lerp_color(p.bg, p.surface, 0.35),
    );
    let glow = Color32::from_rgba_unmultiplied(p.accent.r(), p.accent.g(), p.accent.b(), 14);
    for r in [150.0, 115.0, 80.0] {
        painter.circle_filled(rect.center() - vec2(0.0, 20.0), r, glow);
    }
    let floor = Rect::from_center_size(
        pos2(rect.center().x, rect.bottom() - 34.0),
        vec2(150.0, 22.0),
    );
    painter.add(egui::Shape::ellipse_filled(
        floor.center(),
        floor.size() / 2.0,
        Color32::from_black_alpha(70),
    ));
    painter.rect_stroke(
        rect,
        CornerRadius::same(12),
        Stroke::new(1.0, p.card_stroke),
        StrokeKind::Inside,
    );
}

/// One line of the Equipped list; true when its × was clicked.
fn equipped_row(ui: &mut egui::Ui, p: &Palette, what: &str, name: &str, removable: bool) -> bool {
    let mut clicked = false;
    ui.horizontal(|ui| {
        ui.add_sized(
            [70.0, 20.0],
            egui::Label::new(RichText::new(what).small().color(p.muted)),
        );
        ui.label(RichText::new(name).color(p.text));
        if removable {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                clicked = widgets::icon_button(ui, p, Icon::Close, "Take it off").clicked();
            });
        }
    });
    clicked
}

/// A category in the rail: name, count, highlighted while picked.
fn rail_row(
    ui: &mut egui::Ui,
    p: &Palette,
    name: &str,
    count: Option<usize>,
    selected: bool,
) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(RAIL_W, RAIL_ROW), Sense::click());
    let hover = ui
        .ctx()
        .animate_bool(response.id.with("h"), response.hovered());
    let fill = if selected {
        Color32::from_rgba_unmultiplied(p.accent.r(), p.accent.g(), p.accent.b(), 38)
    } else {
        // (lerp_color is opaque: it would turn "no fill" black.)
        p.surface_hover.gamma_multiply(0.8 * hover)
    };
    ui.painter().rect_filled(rect, CornerRadius::same(8), fill);
    if selected {
        let bar = Rect::from_min_size(rect.min + vec2(0.0, 8.0), vec2(3.0, rect.height() - 16.0));
        ui.painter()
            .rect_filled(bar, CornerRadius::same(2), p.accent);
    }
    let color = if selected {
        p.text
    } else {
        lerp_color(p.muted, p.text, hover)
    };
    ui.painter().text(
        pos2(rect.left() + 14.0, rect.center().y),
        Align2::LEFT_CENTER,
        name,
        FontId::proportional(14.0),
        color,
    );
    if let Some(n) = count {
        ui.painter().text(
            pos2(rect.right() - 10.0, rect.center().y),
            Align2::RIGHT_CENTER,
            n.to_string(),
            FontId::proportional(12.0),
            p.muted,
        );
    }
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

/// The slot a catalog slot name belongs to (unknown ones count as body).
pub(super) fn slot_key(slot: &str) -> &'static str {
    SLOTS.into_iter().find(|s| *s == slot).unwrap_or("body")
}

/// Words for a cosmetic slot.
pub(super) fn slot_name(slot: &str) -> &'static str {
    match slot {
        "head" => "Head",
        "face" => "Face",
        "back" => "Back",
        "shoulders" => "Shoulders",
        _ => "Body",
    }
}
