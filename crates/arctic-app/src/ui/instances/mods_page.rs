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
/// While the Mods page is open, the folder is looked at this often (seconds).
const MODS_RESCAN_EVERY: f64 = 2.0;

/// A cheap fingerprint of a folder: its files' names, sizes and times.
fn folder_stamp(dir: &std::path::Path) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| {
            let meta = e.metadata().ok();
            (
                e.file_name(),
                meta.as_ref().map(|m| m.len()),
                meta.and_then(|m| m.modified().ok()),
            )
        })
        .collect();
    entries.sort();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    entries.hash(&mut hasher);
    hasher.finish()
}

impl ArcticApp {
    pub(super) fn mods_page(&mut self, ui: &mut egui::Ui, instance: &Instance) {
        let p = self.palette();
        let mods_dir = instance.game_dir(&self.dirs).join("mods");
        self.rescan_mods_if_changed(ui, &instance.id, &mods_dir);
        self.add_dropped_mods(ui, &instance.id, &mods_dir);
        let files = match &self.inst.mod_files {
            Some((id, files)) if *id == instance.id => files.clone(),
            _ => {
                self.refresh_mods(&instance.id);
                Vec::new()
            }
        };
        crate::widgets::row(ui, |ui| {
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

    /// .jar files dropped on the window go into this instance's mods folder.
    fn add_dropped_mods(&mut self, ui: &egui::Ui, id: &str, mods_dir: &std::path::Path) {
        let is_jar =
            |p: &std::path::Path| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("jar"));
        if ui.input(|i| {
            i.raw
                .hovered_files
                .iter()
                .any(|f| f.path.as_deref().is_some_and(is_jar))
        }) {
            let p = self.palette();
            let rect = ui.max_rect();
            ui.painter()
                .rect_filled(rect, 10.0, p.accent.gamma_multiply(0.12));
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Drop to add these mods",
                egui::FontId::proportional(18.0),
                p.text,
            );
        }
        let dropped: Vec<std::path::PathBuf> = ui.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .filter(|p| is_jar(p))
                .collect()
        });
        if dropped.is_empty() {
            return;
        }
        let _ = std::fs::create_dir_all(mods_dir);
        let mut added = 0;
        for jar in &dropped {
            let Some(name) = jar.file_name() else {
                continue;
            };
            match std::fs::copy(jar, mods_dir.join(name)) {
                Ok(_) => added += 1,
                Err(e) => self.toasts.push(
                    Kind::Error,
                    format!("Couldn't add {}", name.to_string_lossy()),
                    e.to_string(),
                ),
            }
        }
        if added > 0 {
            self.toasts.push(
                Kind::Success,
                if added == 1 {
                    "Mod added".to_owned()
                } else {
                    format!("{added} mods added")
                },
                "",
            );
            self.refresh_mods(id);
        }
    }

    /// Jars added, removed or replaced in the folder (by hand, or by a game
    /// that updates its mods) show up here without leaving the page.
    fn rescan_mods_if_changed(&mut self, ui: &egui::Ui, id: &str, mods_dir: &std::path::Path) {
        let now = ui.input(|i| i.time);
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs_f64(MODS_RESCAN_EVERY));
        if now - self.inst.mod_watch.0 < MODS_RESCAN_EVERY {
            return;
        }
        let stamp = folder_stamp(mods_dir);
        let changed = stamp != self.inst.mod_watch.1;
        self.inst.mod_watch = (now, stamp);
        if changed {
            self.refresh_mods(id);
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
                if widgets::switch(ui, p, &mut enabled)
                    .on_hover_text(if enabled { "On" } else { "Off" })
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
