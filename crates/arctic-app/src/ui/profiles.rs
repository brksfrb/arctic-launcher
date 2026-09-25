//! Profile switcher (top of the sidebar) and the create / manage dialogs.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, CursorIcon, FontId, Id, Modal, Popup, PopupCloseBehavior,
    RectAlign, RichText, Sense, Stroke, vec2,
};

use crate::app::ArcticApp;
use crate::art::flakes::SWATCHES;
use crate::art::icons::{self, Icon};
use crate::art::lerp_color;
use crate::theme::Palette;
use crate::toasts::Kind;
use crate::widgets;

/// State of the profile dialogs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileDialog {
    Closed,
    Create {
        name: String,
    },
    Manage {
        renaming: Option<(String, String)>,
        confirm_remove: Option<String>,
    },
}

fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

impl ArcticApp {
    /// Compact "current profile" chip; opens the switcher.
    pub(crate) fn profile_chip(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let active = self.profiles.active().clone();
        let (rect, response) =
            ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::click());
        let hover = ui
            .ctx()
            .animate_bool(response.id.with("h"), response.hovered());
        ui.painter().rect_filled(
            rect,
            CornerRadius::same(10),
            p.surface_hover.gamma_multiply(0.35 + 0.5 * hover),
        );
        let dot = rect.left_center() + vec2(16.0, 0.0);
        ui.painter().circle_filled(dot, 5.5, rgb(active.color));
        ui.painter().text(
            dot + vec2(14.0, 0.0),
            Align2::LEFT_CENTER,
            &active.name,
            FontId::proportional(14.0),
            p.text,
        );
        let chevron =
            egui::Rect::from_center_size(rect.right_center() - vec2(14.0, 0.0), vec2(10.0, 10.0));
        icons::draw(ui.painter(), Icon::ChevronDown, chevron, p.muted);
        let response = response
            .on_hover_text("Switch profile")
            .on_hover_cursor(CursorIcon::PointingHand);
        Popup::from_toggle_button_response(&response)
            .align(RectAlign::BOTTOM_START)
            .gap(6.0)
            .width(rect.width() + 40.0)
            .close_behavior(PopupCloseBehavior::CloseOnClick)
            .show(|ui| self.profile_menu(ui));
    }

    fn profile_menu(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let blocked = self.profile_switch_blocked();
        let mut pick = None;
        for profile in &self.profiles.profiles {
            let (rect, response) =
                ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::click());
            if response.hovered() {
                ui.painter()
                    .rect_filled(rect, CornerRadius::same(8), p.surface_hover);
            }
            let dot = rect.left_center() + vec2(14.0, 0.0);
            ui.painter().circle_filled(dot, 5.0, rgb(profile.color));
            ui.painter().text(
                dot + vec2(14.0, 0.0),
                Align2::LEFT_CENTER,
                &profile.name,
                FontId::proportional(14.0),
                p.text,
            );
            if profile.id == self.profiles.active {
                let check = egui::Rect::from_center_size(
                    rect.right_center() - vec2(14.0, 0.0),
                    vec2(13.0, 13.0),
                );
                icons::draw(ui.painter(), Icon::Check, check, p.accent);
            }
            let response = match blocked {
                Some(reason) => response.on_hover_text(reason),
                None => response,
            };
            if response.clicked() {
                pick = Some(profile.id.clone());
            }
        }
        ui.separator();
        if ui.button("New profile…").clicked() {
            self.profile_dialog = ProfileDialog::Create {
                name: String::new(),
            };
        }
        if ui.button("Manage profiles…").clicked() {
            self.profile_dialog = ProfileDialog::Manage {
                renaming: None,
                confirm_remove: None,
            };
        }
        if let Some(id) = pick {
            self.switch_profile(&id);
        }
    }

    pub(crate) fn profile_dialogs(&mut self, ctx: &egui::Context) {
        let p = self.palette();
        let open = !matches!(self.profile_dialog, ProfileDialog::Closed);
        if !open {
            return;
        }
        let modal = Modal::new(Id::new("profiles_dialog"))
            .frame(dialog_frame(p))
            .show(ctx, |ui| match self.profile_dialog.clone() {
                ProfileDialog::Create { name } => self.create_view(ui, name),
                ProfileDialog::Manage {
                    renaming,
                    confirm_remove,
                } => self.manage_view(ui, renaming, confirm_remove),
                ProfileDialog::Closed => {}
            });
        if modal.should_close() {
            self.profile_dialog = ProfileDialog::Closed;
        }
    }

    fn create_view(&mut self, ui: &mut egui::Ui, mut name: String) {
        let p = self.palette();
        ui.set_width(380.0);
        ui.label(
            RichText::new("New profile")
                .size(22.0)
                .strong()
                .color(p.text),
        );
        ui.label(
            RichText::new("A profile has its own accounts, settings, instances and worlds.")
                .color(p.muted),
        );
        ui.add_space(10.0);
        let edit = ui.add(
            widgets::text_field(&mut name)
                .hint_text("e.g. Speedruns, Family, Testing")
                .char_limit(32)
                .desired_width(f32::INFINITY),
        );
        edit.request_focus();
        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
        ui.add_space(10.0);
        let mut close = false;
        ui.horizontal(|ui| {
            if ui.button("Cancel").clicked() {
                close = true;
            }
            let valid = !name.trim().is_empty();
            let create = ui
                .add_enabled_ui(valid, |ui| {
                    widgets::button(ui, p, Some(Icon::Plus), "Create and switch", true)
                })
                .inner;
            if valid && (create.clicked() || enter) {
                match self.profiles.create(&self.root, &name) {
                    Ok(id) => {
                        close = true;
                        self.switch_profile(&id);
                    }
                    Err(e) => {
                        self.toasts
                            .push(Kind::Error, "Could not create profile", e.to_string())
                    }
                }
            }
        });
        self.profile_dialog = if close {
            ProfileDialog::Closed
        } else {
            ProfileDialog::Create { name }
        };
    }

    fn manage_view(
        &mut self,
        ui: &mut egui::Ui,
        mut renaming: Option<(String, String)>,
        mut confirm: Option<String>,
    ) {
        let p = self.palette();
        ui.set_width(460.0);
        ui.label(RichText::new("Profiles").size(22.0).strong().color(p.text));
        ui.add_space(8.0);
        let profiles = self.profiles.profiles.clone();
        let mut changed = false;
        for profile in &profiles {
            let active = profile.id == self.profiles.active;
            egui::Frame::new()
                .fill(lerp_color(p.bg, p.surface_hover, 0.35))
                .corner_radius(CornerRadius::same(10))
                .inner_margin(10.0)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let (dot, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                        ui.painter().circle_filled(dot.center(), 6.0, rgb(profile.color));
                        match &mut renaming {
                            Some((id, text)) if *id == profile.id => {
                                let r = ui.add(widgets::text_field(text).char_limit(32).desired_width(180.0));
                                let done = ui.button("Save").clicked()
                                    || (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
                                if done {
                                    match self.profiles.rename(&profile.id, text) {
                                        Ok(()) => changed = true,
                                        Err(e) => self.toasts.push(Kind::Error, "Could not rename", e.to_string()),
                                    }
                                    renaming = None;
                                }
                            }
                            _ => {
                                ui.label(RichText::new(&profile.name).strong().color(p.text));
                                if active {
                                    ui.label(RichText::new("active").small().color(p.accent));
                                }
                            }
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if !active && widgets::icon_button(ui, p, Icon::Trash, "Remove").clicked() {
                                confirm = Some(profile.id.clone());
                            }
                            if widgets::icon_button(ui, p, Icon::Document, "Rename").clicked() {
                                renaming = Some((profile.id.clone(), profile.name.clone()));
                            }
                            if !active && ui.small_button("Switch").clicked() {
                                self.switch_profile(&profile.id);
                            }
                        });
                    });
                    ui.horizontal(|ui| {
                        ui.add_space(22.0);
                        for c in SWATCHES {
                            let (r, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
                            ui.painter().circle_filled(r.center(), 6.5, rgb(c));
                            if profile.color == c {
                                ui.painter().circle_stroke(r.center(), 8.5, Stroke::new(1.5, rgb(c)));
                            }
                            if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                self.profiles.set_color(&profile.id, c);
                                changed = true;
                            }
                        }
                    });
                    if confirm.as_deref() == Some(profile.id.as_str()) {
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new("Remove this profile? Its folder (worlds included) is moved to profiles/.trash, not deleted.")
                                .small()
                                .color(p.warn),
                        );
                        ui.horizontal(|ui| {
                            if ui.button("Keep").clicked() {
                                confirm = None;
                            }
                            if widgets::button(ui, p, Some(Icon::Trash), "Remove", true).clicked() {
                                match self.profiles.remove(&self.root, &profile.id) {
                                    Ok(()) => changed = true,
                                    Err(e) => self.toasts.push(Kind::Error, "Could not remove profile", e.to_string()),
                                }
                                confirm = None;
                            }
                        });
                    }
                });
            ui.add_space(6.0);
        }
        if changed && let Err(e) = self.profiles.save(&self.root) {
            self.toasts
                .push(Kind::Error, "Could not save profiles", e.to_string());
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if widgets::button(ui, p, Some(Icon::Plus), "New profile", false).clicked() {
                self.profile_dialog = ProfileDialog::Create {
                    name: String::new(),
                };
                return;
            }
            if widgets::button(ui, p, Some(Icon::Folder), "Open folder", false).clicked() {
                let _ = open::that_detached(self.dirs.profile_root());
            }
            if ui.button("Done").clicked() {
                self.profile_dialog = ProfileDialog::Closed;
            }
        });
        if matches!(self.profile_dialog, ProfileDialog::Manage { .. }) {
            self.profile_dialog = ProfileDialog::Manage {
                renaming,
                confirm_remove: confirm,
            };
        }
    }
}

fn dialog_frame(p: &Palette) -> egui::Frame {
    egui::Frame::new()
        .fill(p.surface)
        .stroke(Stroke::new(1.0, p.card_stroke))
        .corner_radius(CornerRadius::same(16))
        .inner_margin(22.0)
}
