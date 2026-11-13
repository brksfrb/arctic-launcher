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
use crate::{t, tf, tn};
const ICON: f32 = 36.0;

/// Where an instance's "look for newer mods" stands.
pub(crate) enum UpdateState {
    Checking,
    Ready(Vec<mods::ModUpdate>),
    Applying,
    Failed(String),
}

impl ArcticApp {
    pub(super) fn mods_page(&mut self, ui: &mut egui::Ui, instance: &Instance) {
        let p = self.palette();
        if self.inst.mod_filter.0 != instance.id {
            self.inst.mod_filter = (instance.id.clone(), String::new());
        }
        let mods_dir = instance.game_dir(&self.dirs).join("mods");
        self.add_dropped_mods(ui, &instance.id, &mods_dir);
        let state = self.inst.mod_scans.get(&instance.id);
        let data = state.and_then(|s| s.data.clone());
        let error = state.and_then(|s| s.error.clone());
        let files = data.as_ref().map_or(&[][..], |d| d.files.as_slice());
        crate::widgets::row(ui, |ui| {
            let enabled = files.iter().filter(|f| f.enabled).count();
            if data.is_none() && error.is_none() {
                ui.spinner();
                ui.label(RichText::new(t!("instances.loading_mods")).color(p.muted));
            } else if data.is_some() {
                ui.label(
                    RichText::new(tf!(
                        "instances.mods_enabled",
                        count = files.len(),
                        enabled = enabled
                    ))
                    .color(p.muted),
                );
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::button(ui, p, Some(Icon::Plus), t!("instances.add_mods"), true)
                    .clicked()
                {
                    self.inst.page = InstancePage::Browse;
                }
                if widgets::button(
                    ui,
                    p,
                    Some(Icon::Folder),
                    t!("instances.open_folder"),
                    false,
                )
                .clicked()
                {
                    let _ = std::fs::create_dir_all(&mods_dir);
                    let _ = open::that_detached(&mods_dir);
                }
                let busy = matches!(
                    self.inst.mod_updates.get(&instance.id),
                    Some(UpdateState::Checking | UpdateState::Applying)
                );
                if !busy
                    && instance.loader.kind().is_some()
                    && widgets::button(
                        ui,
                        p,
                        Some(Icon::Import),
                        t!("instances.look_for_updates"),
                        false,
                    )
                    .clicked()
                {
                    self.inst
                        .mod_updates
                        .insert(instance.id.clone(), UpdateState::Checking);
                    self.tasks.mod_updates_check(instance.clone());
                }
            });
        });
        ui.add_space(8.0);
        crate::widgets::row(ui, |ui| {
            ui.add(
                widgets::text_field(&mut self.inst.mod_filter.1)
                    .hint_text(t!("instances.search_installed_mods"))
                    .desired_width((ui.available_width() - 80.0).max(120.0)),
            );
            if !self.inst.mod_filter.1.is_empty()
                && widgets::button(ui, p, None, t!("instances.clear"), false).clicked()
            {
                self.inst.mod_filter.1.clear();
            }
        });
        ui.add_space(8.0);
        if let Some(error) = error {
            crate::widgets::row(ui, |ui| {
                ui.label(
                    RichText::new(tf!("instances.could_not_refresh_mods", error = error))
                        .color(p.error),
                );
                if widgets::button(ui, p, None, t!("instances.retry"), false).clicked() {
                    self.refresh_mods(&instance.id);
                }
            });
        }
        if data.is_none() {
            return;
        }
        self.mod_updates_card(ui, instance);
        self.clash_card(ui, instance);
        if files.is_empty() {
            theme::card(p).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.vertical_centered(|ui| {
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new(t!("instances.no_mods_yet"))
                            .size(17.0)
                            .strong()
                            .color(p.text),
                    );
                    ui.label(
                        RichText::new(t!("instances.browse_modrinth_or_drop_jar_files_into_the"))
                            .color(p.muted),
                    );
                    ui.add_space(10.0);
                });
            });
            return;
        }
        let index = mods::index_path(&self.dirs.instance_dir(&instance.id));
        let query = self.inst.mod_filter.1.trim().to_lowercase();
        let matches: Vec<_> = files
            .iter()
            .filter(|file| {
                query.is_empty()
                    || file.file_name.to_lowercase().contains(&query)
                    || file.tracked.as_ref().is_some_and(|m| {
                        m.title.to_lowercase().contains(&query)
                            || m.version_number.to_lowercase().contains(&query)
                            || m.project_id.to_lowercase().contains(&query)
                    })
            })
            .collect();
        let mut changed = false;
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            if matches.is_empty() {
                ui.label(
                    RichText::new(t!("instances.no_installed_mods_match_your_search"))
                        .color(p.muted),
                );
            } else if !query.is_empty() {
                ui.label(
                    RichText::new(tf!(
                        "instances.of_mods",
                        count = matches.len(),
                        count2 = files.len()
                    ))
                    .color(p.muted),
                );
            }
            visible_rows(ui, matches.len(), |ui, row| {
                let file = matches[row];
                ui.push_id((&instance.id, &file.file_name, file.enabled), |ui| {
                    changed |= self.mod_row(ui, &instance.id, file, &mods_dir, &index);
                });
            });
        });
        if changed {
            self.refresh_mods(&instance.id);
        }
    }

    /// The result of "Look for updates": the mods with a newer build, and a button to update them all.
    fn mod_updates_card(&mut self, ui: &mut egui::Ui, instance: &Instance) {
        let p = self.palette();
        let Some(state) = self.inst.mod_updates.get(&instance.id) else {
            return;
        };
        let mut apply: Option<Vec<String>> = None;
        let mut dismiss = false;
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            match state {
                UpdateState::Checking => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(
                            RichText::new(t!("instances.looking_for_newer_builds")).color(p.muted),
                        );
                    });
                }
                UpdateState::Applying => {
                    crate::widgets::row(ui, |ui| {
                        ui.spinner();
                        ui.label(RichText::new(t!("instances.updating_mods")).color(p.muted));
                    });
                }
                UpdateState::Failed(e) => {
                    ui.label(
                        RichText::new(tf!("instances.couldnt_look_for_updates", e = e))
                            .color(p.error),
                    );
                    if widgets::button(ui, p, None, t!("instances.close"), false).clicked() {
                        dismiss = true;
                    }
                }
                UpdateState::Ready(list) if list.is_empty() => {
                    crate::widgets::row(ui, |ui| {
                        ui.label(
                            RichText::new(t!("instances.all_your_mods_are_up_to_date"))
                                .color(p.text),
                        );
                        if widgets::button(ui, p, None, t!("instances.close"), false).clicked() {
                            dismiss = true;
                        }
                    });
                }
                UpdateState::Ready(list) => {
                    ui.label(
                        RichText::new(tn!("instances.update_available", list.len()))
                            .size(17.0)
                            .strong()
                            .color(p.text),
                    );
                    ui.add_space(4.0);
                    for update in list {
                        crate::widgets::row(ui, |ui| {
                            ui.label(RichText::new(&update.title).color(p.text));
                            ui.label(
                                RichText::new(format!("{} → {}", update.current, update.latest))
                                    .small()
                                    .color(p.muted),
                            );
                        });
                    }
                    ui.add_space(8.0);
                    crate::widgets::row(ui, |ui| {
                        if widgets::button(
                            ui,
                            p,
                            Some(Icon::Import),
                            t!("instances.update_all"),
                            true,
                        )
                        .clicked()
                        {
                            apply = Some(list.iter().map(|u| u.project_id.clone()).collect());
                        }
                        if widgets::button(ui, p, None, t!("instances.not_now"), false).clicked() {
                            dismiss = true;
                        }
                    });
                }
            }
        });
        ui.add_space(8.0);
        if dismiss {
            self.inst.mod_updates.remove(&instance.id);
        }
        if let Some(projects) = apply {
            self.inst
                .mod_updates
                .insert(instance.id.clone(), UpdateState::Applying);
            self.tasks.mod_updates_apply(instance.clone(), projects);
        }
    }

    /// .jar files dropped on the window go into this instance's mods folder (from any of its
    /// tabs); returns whether any were added.
    pub(super) fn add_dropped_mods(
        &mut self,
        ui: &egui::Ui,
        id: &str,
        mods_dir: &std::path::Path,
    ) -> bool {
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
                t!("instances.drop_to_add_these_mods"),
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
            return false;
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
                    tf!(
                        "instances.couldnt_add",
                        to_string_lossy = name.to_string_lossy()
                    ),
                    e.to_string(),
                ),
            }
        }
        if added > 0 {
            self.toasts.push(
                Kind::Success,
                if added == 1 {
                    t!("instances.mod_added").to_owned()
                } else {
                    tf!("instances.mods_added", added = added)
                },
                "",
            );
            self.refresh_mods(id);
        }
        added > 0
    }

    /// One installed mod. Returns true if the folder changed.
    fn mod_row(
        &mut self,
        ui: &mut egui::Ui,
        instance_id: &str,
        file: &ModFile,
        mods_dir: &std::path::Path,
        index: &std::path::Path,
    ) -> bool {
        let p = self.palette();
        let mut changed = false;
        ui.horizontal(|ui| {
            let icon_url = file.tracked.as_ref().and_then(|m| m.icon_url.clone());
            mod_icon(ui, self, icon_url.as_deref());
            let text_width = (ui.available_width() - 110.0).max(40.0);
            ui.allocate_ui_with_layout(
                vec2(text_width, ICON),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    let title = file
                        .tracked
                        .as_ref()
                        .map_or(file.file_name.as_str(), |m| m.title.as_str());
                    let color = if file.enabled { p.text } else { p.muted };
                    ui.add(egui::Label::new(RichText::new(title).strong().color(color)).truncate())
                        .on_hover_text(title);
                    let detail = match &file.tracked {
                        Some(m) if m.dependency => {
                            tf!("instances.dependency", version_number = m.version_number)
                        }
                        Some(m) => m.version_number.clone(),
                        None => format!("{} · {}", file.file_name, format_bytes(file.size)),
                    };
                    ui.add(
                        egui::Label::new(RichText::new(&detail).small().color(p.muted)).truncate(),
                    )
                    .on_hover_text(detail);
                },
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::icon_button(ui, p, Icon::Trash, t!("instances.remove_with_undo"))
                    .clicked()
                {
                    match mods::trash(mods_dir, index, &file.file_name) {
                        Ok(trashed) => {
                            changed = true;
                            let shown = file
                                .tracked
                                .as_ref()
                                .map_or(file.file_name.as_str(), |m| m.title.as_str());
                            self.toasts.push_undo(
                                tf!("instances.removed_3", shown = shown),
                                crate::undo::Undo::Mod {
                                    instance_id: instance_id.to_owned(),
                                    index: index.to_owned(),
                                    trashed,
                                },
                            );
                        }
                        Err(e) => self.toasts.push(
                            Kind::Error,
                            t!("instances.could_not_remove_mod"),
                            e.to_string(),
                        ),
                    }
                }
                let mut enabled = file.enabled;
                if widgets::switch(ui, p, &mut enabled)
                    .on_hover_text(if enabled { "On" } else { t!("instances.off") })
                    .changed()
                {
                    match mods::set_enabled(mods_dir, &file.file_name, enabled) {
                        Ok(()) => changed = true,
                        Err(e) => self.toasts.push(
                            Kind::Error,
                            t!("instances.could_not_change_mod"),
                            e.to_string(),
                        ),
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

/// Use the page's existing scroll area, but only create widgets and request
/// icons for visible rows. The full height keeps its scrollbar accurate.
fn visible_rows(ui: &mut egui::Ui, count: usize, mut draw: impl FnMut(&mut egui::Ui, usize)) {
    if count == 0 {
        return;
    }
    let text_height = ui.text_style_height(&egui::TextStyle::Body)
        + ui.text_style_height(&egui::TextStyle::Small)
        + ui.spacing().item_spacing.y;
    let height = ICON.max(text_height) + 12.0;
    let (rect, _) = ui.allocate_exact_size(
        vec2(ui.available_width(), count as f32 * height),
        Sense::hover(),
    );
    let clip = ui.clip_rect();
    if !rect.intersects(clip) {
        return;
    }
    let first = (((clip.top() - rect.top()) / height).floor().max(0.0) as usize).min(count);
    let end = (((clip.bottom() - rect.top()) / height).ceil().max(0.0) as usize).min(count);
    for row in first..end {
        let row_rect = egui::Rect::from_min_size(
            rect.min + vec2(0.0, row as f32 * height),
            vec2(rect.width(), height),
        );
        let mut child = ui.new_child(egui::UiBuilder::new().id_salt(row).max_rect(row_rect));
        draw(&mut child, row);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn large_mod_list_only_builds_visible_rows_including_after_scrolling() {
        let ctx = egui::Context::default();
        for offset in [0.0, 24_000.0, 479_600.0] {
            let mut rows = Vec::new();
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    vec2(800.0, 400.0),
                )),
                ..Default::default()
            };
            let output = ctx.run_ui(input, |ui| {
                rows.clear();
                let rect = ui.max_rect().translate(vec2(0.0, -offset));
                let mut content = ui.new_child(egui::UiBuilder::new().max_rect(rect));
                visible_rows(&mut content, 10_000, |_, row| rows.push(row));
            });
            output.drop_without_applying_deltas();
            assert!(!rows.is_empty());
            assert!(
                rows.len() < 20,
                "built {} rows at offset {offset}",
                rows.len()
            );
            if offset > 0.0 {
                assert!(rows[0] > 0);
            }
            assert!(rows.windows(2).all(|w| w[1] == w[0] + 1));
        }
    }
}
