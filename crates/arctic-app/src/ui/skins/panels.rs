//! Layout of the Skins tab: preview with actions, capes and the library.

use arctic_core::skins::{SkinEntry, Variant};
use eframe::egui::{
    self, Align2, CornerRadius, FontId, Rect, RichText, Sense, Stroke, StrokeKind, pos2, vec2,
};

use super::Selection;
use super::model::{self, Pose};
use crate::app::ArcticApp;
use crate::art::icons::Icon;
use crate::art::lerp_color;
use crate::skin_tasks::SkinChange;
use crate::theme::{self, Palette};
use crate::widgets;

const PREVIEW: [f32; 2] = [300.0, 320.0];
const TILE: [f32; 2] = [118.0, 158.0];
const CAPE_TILE: f32 = 54.0;

impl ArcticApp {
    pub(super) fn skins_page(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        widgets::page_header(
            ui,
            p,
            "Skins",
            "Change how you look in game. Everyone sees your skin, on every server.",
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
                ui.vertical(|ui| self.library_panel(ui));
            });
        });
    }

    fn account_notice(&mut self, ui: &mut egui::Ui, p: &Palette) {
        let text = match self.accounts.active() {
            None => {
                "Add a Microsoft account to change your skin. You can still collect skins below."
            }
            Some(a) if !a.is_microsoft() => {
                "Offline accounts can't change skins. You can still collect skins below."
            }
            Some(a) => match self.skins.account.get(&a.id) {
                Some(Err(e)) => {
                    let e = e.clone();
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(e).color(p.error));
                        if ui.link("Retry").clicked() {
                            self.request_account_skin(true);
                        }
                    });
                    ui.add_space(6.0);
                    return;
                }
                _ => return,
            },
        };
        ui.label(RichText::new(text).color(p.muted));
        ui.add_space(6.0);
    }

    fn preview_panel(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        // Without a current skin to show, preview the newest library skin.
        if self.skins.selection == Selection::Current
            && self.skin_png("current").is_none()
            && !self.skins.busy
            && let Some(first) = self.skins.library.skins.first()
        {
            self.skins.selection = Selection::Library(first.id.clone());
        }
        let ctx = ui.ctx().clone();
        let (rect, response) = ui.allocate_exact_size(PREVIEW.into(), Sense::drag());
        if response.dragged() {
            let d = response.drag_delta();
            self.skins.yaw += d.x * 0.012;
            self.skins.pitch = model::clamp_pitch(self.skins.pitch + d.y * 0.008);
        }
        let key = match &self.skins.selection {
            Selection::Current => "current".to_owned(),
            Selection::Library(id) => format!("lib:{id}"),
        };
        let entry = self.selected_entry();
        let current_variant = self.current_profile_variant();
        let cape = self
            .active_cape_key()
            .and_then(|k| self.skin_texture(&ctx, &k).map(|t| t.handle.id()));
        let time = ui.input(|i| i.time);
        let pose = Pose {
            yaw: self.skins.yaw,
            pitch: self.skins.pitch,
            swing: model::idle_swing(time),
        };
        match self.skin_texture(&ctx, &key) {
            Some(tex) => {
                let variant = entry
                    .as_ref()
                    .map_or(current_variant.unwrap_or(tex.guessed), |e| e.variant);
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
                let text = if self.skins.busy {
                    "Loading skin…"
                } else {
                    "Default skin"
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
        ui.label(RichText::new("Drag to turn").small().color(p.muted));
        ui.add_space(6.0);
        match entry {
            Some(entry) => self.library_actions(ui, &entry),
            None => self.current_actions(ui),
        }
        self.capes_row(ui);
    }

    fn current_actions(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        ui.label(RichText::new("Your skin").size(18.0).strong().color(p.text));
        let can_change = self.can_change_skin();
        ui.horizontal(|ui| {
            if let Some(v) = self.current_profile_variant() {
                ui.label(RichText::new(v.label()).color(p.muted));
            }
            if self.skins.busy {
                ui.spinner();
            }
        });
        ui.horizontal(|ui| {
            if let Some(png) = self.skin_png("current")
                && widgets::button(ui, p, Some(Icon::Plus), "Save to library", false).clicked()
            {
                let name = self
                    .accounts
                    .active()
                    .map_or("Skin".to_owned(), |a| a.username.clone());
                let variant = self.current_profile_variant();
                self.add_skin(&name, &png, variant);
            }
            let reset = ui
                .add_enabled_ui(can_change, |ui| ui.button("Reset to default"))
                .inner;
            if reset.clicked() {
                self.change_account_skin(SkinChange::Reset);
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
                let done = field.lost_focus();
                if done {
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
            let can = self.can_change_skin();
            let apply = ui
                .add_enabled_ui(can, |ui| {
                    widgets::button(ui, p, Some(Icon::Check), "Use this skin", true)
                })
                .inner;
            if apply.clicked() {
                self.apply_library_skin(entry);
            }
            if self.skins.busy {
                ui.spinner();
            }
            if widgets::icon_button(ui, p, Icon::Trash, "Delete from library").clicked() {
                self.remove_skin(&id);
            }
        });
    }

    fn capes_row(&mut self, ui: &mut egui::Ui) {
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
        ui.label(RichText::new("CAPE").small().color(p.muted));
        let ctx = ui.ctx().clone();
        let mut pick: Option<Option<String>> = None;
        ui.horizontal_wrapped(|ui| {
            if cape_tile(ui, p, None, "No cape", none_active) && !none_active {
                pick = Some(None);
            }
            for (id, alias, active) in &capes {
                let tex = self
                    .skin_texture(&ctx, &format!("cape:{id}"))
                    .map(|t| t.handle.id());
                if cape_tile(ui, p, tex, alias, *active) && !active {
                    pick = Some(Some(id.clone()));
                }
            }
        });
        if let Some(choice) = pick
            && !self.skins.busy
        {
            self.change_account_skin(SkinChange::Cape(choice));
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
        let show_current = self.skin_png("current").is_some();
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
            if show_current {
                let variant = self.current_profile_variant();
                if self.skin_tile(
                    ui,
                    "current",
                    "Current",
                    variant,
                    self.skins.selection == Selection::Current,
                ) {
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
    fn skin_tile(
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
            Selection::Current => None,
        }
    }

    fn current_profile_variant(&self) -> Option<Variant> {
        let a = self.accounts.active()?;
        match self.skins.account.get(&a.id)? {
            Ok(state) => state.profile.active_skin().map(|s| s.variant()),
            Err(_) => None,
        }
    }

    fn active_cape_key(&self) -> Option<String> {
        let a = self.accounts.active()?;
        match self.skins.account.get(&a.id)? {
            Ok(state) => state
                .profile
                .active_cape()
                .map(|c| format!("cape:{}", c.id)),
            Err(_) => None,
        }
    }

    fn can_change_skin(&self) -> bool {
        !self.skins.busy
            && self.accounts.active().is_some_and(|a| {
                a.is_microsoft() && matches!(self.skins.account.get(&a.id), Some(Ok(_)))
            })
    }
}

/// Cape front, or an empty slot for "No cape". Returns true when clicked.
fn cape_tile(
    ui: &mut egui::Ui,
    p: &Palette,
    texture: Option<egui::TextureId>,
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
            // Cape front: (1,1) 10×16 in a 64×32 texture.
            let uv =
                Rect::from_min_max(pos2(1.0 / 64.0, 1.0 / 32.0), pos2(11.0 / 64.0, 17.0 / 32.0));
            ui.painter().image(tex, inner, uv, egui::Color32::WHITE);
        }
        None => {
            icon_close(ui, p, inner);
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

fn icon_close(ui: &egui::Ui, p: &Palette, rect: Rect) {
    let r = Rect::from_center_size(rect.center(), vec2(16.0, 16.0));
    crate::art::icons::draw(ui.painter(), Icon::Close, r, p.muted);
}
