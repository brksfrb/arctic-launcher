//! Installed mods of an instance: enable/disable, remove, open folder.

use arctic_core::instances::Instance;
use arctic_core::mods::{self, ModFile};
use eframe::egui::{self, CornerRadius, RichText, Sense, vec2};

use super::InstancePage;
use crate::app::ArcticApp;
use crate::art::icons::{self, Icon};
use crate::motion::format_bytes;
use crate::theme;
use crate::toasts::Kind;
use crate::widgets;

const ICON: f32 = 36.0;

impl ArcticApp {
    pub(super) fn mods_page(&mut self, ui: &mut egui::Ui, instance: &Instance) {
        let p = self.palette();
        let files = match &self.inst.mod_files {
            Some((id, files)) if *id == instance.id => files.clone(),
            _ => {
                self.refresh_mods(&instance.id);
                Vec::new()
            }
        };
        let mods_dir = instance.game_dir(&self.dirs).join("mods");
        ui.horizontal(|ui| {
            let enabled = files.iter().filter(|f| f.enabled).count();
            ui.label(
                RichText::new(format!("{} mods · {enabled} enabled", files.len())).color(p.muted),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::button(ui, p, Some(Icon::Plus), "Add mods", true).clicked() {
                    self.inst.page = InstancePage::Browse;
                }
                if widgets::button(ui, p, Some(Icon::Folder), "Open folder", false).clicked() {
                    let _ = std::fs::create_dir_all(&mods_dir);
                    let _ = open::that_detached(&mods_dir);
                }
            });
        });
        ui.add_space(8.0);
        if files.is_empty() {
            theme::card(p).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.vertical_centered(|ui| {
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new("No mods yet")
                            .size(17.0)
                            .strong()
                            .color(p.text),
                    );
                    ui.label(
                        RichText::new("Browse Modrinth, or drop .jar files into the mods folder.")
                            .color(p.muted),
                    );
                    ui.add_space(10.0);
                });
            });
            return;
        }
        let index = mods::index_path(&self.dirs.instance_dir(&instance.id));
        let mut changed = false;
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            for file in &files {
                changed |= self.mod_row(ui, file, &mods_dir, &index);
            }
        });
        if changed {
            self.refresh_mods(&instance.id);
        }
    }

    /// One installed mod. Returns true if the folder changed.
    fn mod_row(
        &mut self,
        ui: &mut egui::Ui,
        file: &ModFile,
        mods_dir: &std::path::Path,
        index: &std::path::Path,
    ) -> bool {
        let p = self.palette();
        let mut changed = false;
        ui.horizontal(|ui| {
            let icon_url = file.tracked.as_ref().and_then(|m| m.icon_url.clone());
            mod_icon(ui, self, icon_url.as_deref());
            ui.vertical(|ui| {
                let title = file
                    .tracked
                    .as_ref()
                    .map_or(file.file_name.as_str(), |m| m.title.as_str());
                let color = if file.enabled { p.text } else { p.muted };
                ui.label(RichText::new(title).strong().color(color));
                let detail = match &file.tracked {
                    Some(m) if m.dependency => format!("{} · dependency", m.version_number),
                    Some(m) => m.version_number.clone(),
                    None => format!("{} · {}", file.file_name, format_bytes(file.size)),
                };
                ui.label(RichText::new(detail).small().color(p.muted));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::icon_button(ui, p, Icon::Trash, "Remove").clicked() {
                    match mods::remove(mods_dir, index, &file.file_name) {
                        Ok(()) => changed = true,
                        Err(e) => {
                            self.toasts
                                .push(Kind::Error, "Could not remove mod", e.to_string())
                        }
                    }
                }
                let mut enabled = file.enabled;
                if ui
                    .checkbox(&mut enabled, "")
                    .on_hover_text("Enabled")
                    .changed()
                {
                    match mods::set_enabled(mods_dir, &file.file_name, enabled) {
                        Ok(()) => changed = true,
                        Err(e) => {
                            self.toasts
                                .push(Kind::Error, "Could not change mod", e.to_string())
                        }
                    }
                }
            });
        });
        ui.add_space(4.0);
        changed
    }
}

/// Mod icon from Modrinth, or a placeholder while loading / missing.
pub(super) fn mod_icon(ui: &mut egui::Ui, app: &mut ArcticApp, url: Option<&str>) {
    let p = app.palette();
    let uri = url.and_then(|u| app.icon_uri(u));
    match uri {
        Some(uri) => {
            ui.add(
                egui::Image::new(uri)
                    .fit_to_exact_size(vec2(ICON, ICON))
                    .corner_radius(CornerRadius::same(8)),
            );
        }
        None => {
            let (rect, _) = ui.allocate_exact_size(vec2(ICON, ICON), Sense::hover());
            ui.painter()
                .rect_filled(rect, CornerRadius::same(8), p.surface_hover);
            icons::draw(ui.painter(), Icon::Layers, rect.shrink(10.0), p.muted);
        }
    }
}
