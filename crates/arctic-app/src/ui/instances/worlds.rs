//! Worlds of an instance: list, import (other launchers, instances, .zip),
//! back up and move to trash.

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::SystemTime;

use arctic_core::instances::Instance;
use arctic_core::worlds::{self, Source, World};
use eframe::egui::{self, CornerRadius, Id, Modal, RichText, Sense, vec2};

use crate::app::ArcticApp;
use crate::art::icons::{self, Icon};
use crate::motion::format_bytes;
use crate::theme;
use crate::toasts::Kind;
use crate::widgets;

const ICON: f32 = 44.0;

/// The "Import worlds" dialog.
pub struct ImportDialog {
    sources: Vec<Source>,
    source: usize,
    picked: HashSet<PathBuf>,
}

impl ArcticApp {
    pub(super) fn worlds_page(&mut self, ui: &mut egui::Ui, instance: &Instance) {
        let p = self.palette();
        let saves = instance.game_dir(&self.dirs).join("saves");
        let list = match &self.inst.worlds {
            Some((id, list)) if *id == instance.id => list.clone(),
            _ => {
                self.refresh_worlds(instance);
                Vec::new()
            }
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{} worlds", list.len())).color(p.muted));
            if self.inst.worlds_busy {
                ui.spinner();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::button(ui, p, Some(Icon::Plus), "Import worlds", true).clicked() {
                    self.open_import(instance);
                }
                if widgets::button(ui, p, None, "From a .zip…", false).clicked() {
                    self.tasks
                        .import_world_zip(instance.id.clone(), saves.clone());
                    self.inst.worlds_busy = true;
                }
                if widgets::icon_button(ui, p, Icon::Folder, "Open saves folder").clicked() {
                    let _ = std::fs::create_dir_all(&saves);
                    let _ = open::that_detached(&saves);
                }
            });
        });
        ui.add_space(8.0);
        if list.is_empty() {
            theme::card(p).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.vertical_centered(|ui| {
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new("No worlds yet")
                            .size(17.0)
                            .strong()
                            .color(p.text),
                    );
                    ui.label(
                        RichText::new(
                            "Play to create one, or import worlds from another launcher.",
                        )
                        .color(p.muted),
                    );
                    ui.add_space(10.0);
                });
            });
            return;
        }
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            for world in &list {
                self.world_row(ui, instance, world);
                ui.add_space(4.0);
            }
        });
    }

    fn world_row(&mut self, ui: &mut egui::Ui, instance: &Instance, world: &World) {
        let p = self.palette();
        ui.horizontal(|ui| {
            match world
                .icon
                .as_ref()
                .and_then(|icon| world_icon_uri(ui.ctx(), icon))
            {
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
                    icons::draw(ui.painter(), Icon::Layers, rect.shrink(12.0), p.muted);
                }
            }
            ui.vertical(|ui| {
                ui.label(RichText::new(&world.name).strong().color(p.text));
                let detail = format!(
                    "{} · {} · {}",
                    world.folder,
                    ago(world.last_played),
                    format_bytes(world.size)
                );
                ui.label(RichText::new(detail).small().color(p.muted));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::icon_button(ui, p, Icon::Trash, "Move to trash").clicked() {
                    let saves = instance.game_dir(&self.dirs).join("saves");
                    match worlds::trash(world, &saves) {
                        Ok(()) => self.toasts.push(
                            Kind::Info,
                            format!("Moved {} to trash", world.name),
                            "saves/.trash",
                        ),
                        Err(e) => {
                            self.toasts
                                .push(Kind::Error, "Could not move world", e.to_string())
                        }
                    }
                    self.refresh_worlds(instance);
                }
                if widgets::icon_button(ui, p, Icon::Copy, "Back up to a .zip").clicked() {
                    let backups = self
                        .dirs
                        .instance_dir(&instance.id)
                        .join(worlds::BACKUPS_DIR);
                    self.tasks
                        .backup_world(instance.id.clone(), world.clone(), backups);
                    self.inst.worlds_busy = true;
                }
                if widgets::icon_button(ui, p, Icon::Folder, "Open world folder").clicked() {
                    let _ = open::that_detached(&world.path);
                }
            });
        });
    }

    pub(super) fn refresh_worlds(&mut self, instance: &Instance) {
        let saves = instance.game_dir(&self.dirs).join("saves");
        self.inst.worlds = Some((instance.id.clone(), worlds::list(&saves)));
    }

    fn open_import(&mut self, instance: &Instance) {
        let saves = instance.game_dir(&self.dirs).join("saves");
        let sources = worlds::sources(&self.dirs, &saves);
        if sources.is_empty() {
            self.toasts.push(
                Kind::Info,
                "No worlds found to import",
                "Checked the Minecraft Launcher, Prism Launcher and your other instances.",
            );
            return;
        }
        self.inst.import = Some(ImportDialog {
            sources,
            source: 0,
            picked: HashSet::new(),
        });
    }

    /// The import dialog, when open. Shown from the Instances tab.
    pub(crate) fn import_worlds_dialog(&mut self, ctx: &egui::Context) {
        let Some(id) = self.inst.open.clone() else {
            return;
        };
        let Some(instance) = self.instance_by_id(&id).cloned() else {
            return;
        };
        let Some(mut dialog) = self.inst.import.take() else {
            return;
        };
        let p = self.palette();
        let mut submit = false;
        let modal = Modal::new(Id::new("import_worlds"))
            .frame(super::detail::dialog_frame(p))
            .show(ctx, |ui| {
                ui.set_width(520.0);
                crate::widgets::lift_controls(ui, p);
                ui.label(
                    RichText::new("Import worlds")
                        .size(22.0)
                        .strong()
                        .color(p.text),
                );
                ui.label(
                    RichText::new("Copies are made; the originals stay where they are.")
                        .color(p.muted),
                );
                ui.add_space(8.0);
                egui::ComboBox::from_id_salt("import_source")
                    .width(ui.available_width())
                    .selected_text(dialog.sources[dialog.source].label.clone())
                    .show_ui(ui, |ui| {
                        for (i, s) in dialog.sources.iter().enumerate() {
                            if ui.selectable_label(dialog.source == i, &s.label).clicked() {
                                dialog.source = i;
                                dialog.picked.clear();
                            }
                        }
                    });
                ui.add_space(6.0);
                let found = worlds::list(&dialog.sources[dialog.source].saves);
                egui::ScrollArea::vertical()
                    .max_height(300.0)
                    .show(ui, |ui| {
                        for w in &found {
                            let mut on = dialog.picked.contains(&w.path);
                            let label = format!(
                                "{}  ({}, {})",
                                w.name,
                                ago(w.last_played),
                                format_bytes(w.size)
                            );
                            if ui.checkbox(&mut on, label).changed() {
                                if on {
                                    dialog.picked.insert(w.path.clone());
                                } else {
                                    dialog.picked.remove(&w.path);
                                }
                            }
                        }
                    });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        ui.close();
                    }
                    let n = dialog.picked.len();
                    let text = match n {
                        0 => "Import".to_owned(),
                        1 => "Import 1 world".to_owned(),
                        n => format!("Import {n} worlds"),
                    };
                    let go = ui
                        .add_enabled_ui(n > 0, |ui| {
                            widgets::button(ui, p, Some(Icon::Plus), &text, true)
                        })
                        .inner;
                    submit = n > 0 && go.clicked();
                });
            });
        if submit {
            let saves = instance.game_dir(&self.dirs).join("saves");
            let picked: Vec<PathBuf> = dialog.picked.into_iter().collect();
            self.tasks.import_worlds(instance.id.clone(), picked, saves);
            self.inst.worlds_busy = true;
            return;
        }
        if !modal.should_close() {
            self.inst.import = Some(dialog);
        }
    }

    /// Background world jobs finished.
    pub(crate) fn on_worlds_done(&mut self, instance_id: String, result: Result<String, String>) {
        self.inst.worlds_busy = false;
        match result {
            Ok(message) => self.toasts.push(Kind::Success, message, ""),
            Err(e) => self.toasts.push(Kind::Error, "World operation failed", e),
        }
        if let Some(instance) = self.instance_by_id(&instance_id).cloned() {
            self.refresh_worlds(&instance);
        }
    }
}

/// Register a world's icon.png with egui (once per file version).
fn world_icon_uri(ctx: &egui::Context, path: &std::path::Path) -> Option<String> {
    use std::hash::{Hash, Hasher};
    let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut hasher);
    modified.hash(&mut hasher);
    let uri = format!("bytes://world-icon/{:016x}.png", hasher.finish());
    let known = ctx.data(|d| d.get_temp::<bool>(Id::new(&uri)).is_some());
    if !known {
        let bytes = std::fs::read(path).ok()?;
        ctx.include_bytes(uri.clone(), bytes);
        ctx.data_mut(|d| d.insert_temp(Id::new(&uri), true));
    }
    Some(uri)
}

/// "3 days ago"
fn ago(time: Option<SystemTime>) -> String {
    let Some(secs) = time.and_then(|t| t.elapsed().ok()).map(|d| d.as_secs()) else {
        return "never played".into();
    };
    match secs {
        0..60 => "just now".into(),
        60..3600 => plural(secs / 60, "minute"),
        3600..86_400 => plural(secs / 3600, "hour"),
        86_400..2_592_000 => plural(secs / 86_400, "day"),
        _ => plural(secs / 2_592_000, "month"),
    }
}

fn plural(n: u64, unit: &str) -> String {
    if n == 1 {
        format!("1 {unit} ago")
    } else {
        format!("{n} {unit}s ago")
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn relative_times() {
        let at = |secs| Some(SystemTime::now() - Duration::from_secs(secs));
        assert_eq!(ago(at(5)), "just now");
        assert_eq!(ago(at(120)), "2 minutes ago");
        assert_eq!(ago(at(3600)), "1 hour ago");
        assert_eq!(ago(at(3 * 86_400)), "3 days ago");
        assert_eq!(ago(None), "never played");
    }
}
