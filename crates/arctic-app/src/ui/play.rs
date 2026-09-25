//! Play tab: instance card, version picker, Play button and launch status.

use eframe::egui::{
    self, Align2, CornerRadius, CursorIcon, FontId, Popup, PopupCloseBehavior, RichText, Sense,
    vec2,
};

use arctic_core::instances::Instance;
use arctic_core::versions::{VersionEntry, VersionKind, VersionManifest};

use crate::app::{AddAccount, ArcticApp, LaunchState, ManifestState, Tab};
use crate::art::avatar::paint_face;
use crate::art::icons::{self, Icon};
use crate::art::lerp_color;
use crate::motion::{format_bytes, format_duration};
use crate::widgets::{self, PlayState};

const PLAY_SIZE: [f32; 2] = [300.0, 62.0];
const PICKER_WIDTH: f32 = 340.0;

impl ArcticApp {
    pub(crate) fn play_tab(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        widgets::page_header(ui, p, "Play", "Pick an instance and jump in.");
        let instance = self.selected_instance().clone();
        crate::theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let emblem = widgets::instance_emblem(ui, p, &instance.icon, 64.0);
                self.icon_picker(&emblem, &instance.id);
                ui.add_space(6.0);
                self.instance_switcher(ui, &instance);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    self.account_chip(ui);
                });
            });
            ui.add_space(12.0);
            ui.label(RichText::new("VERSION").small().color(p.muted));
            if instance.is_default() {
                self.version_picker(ui);
            } else {
                ui.label(
                    RichText::new(Self::instance_subtitle(&instance))
                        .size(16.0)
                        .color(p.text),
                );
            }
        });
        ui.add_space(22.0);
        ui.horizontal(|ui| {
            self.play_button(ui);
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.add_space(8.0);
                ui.horizontal(|ui| self.quick_actions(ui));
            });
        });
        ui.add_space(10.0);
        self.launch_status(ui);
    }

    /// Versions shown in pickers: releases, plus snapshots / old versions
    /// when enabled in Settings. Newest first.
    pub(crate) fn visible_versions(&self, manifest: &VersionManifest) -> Vec<VersionEntry> {
        manifest
            .versions
            .iter()
            .filter(|v| match v.kind {
                VersionKind::Release => true,
                VersionKind::Snapshot => self.settings.show_snapshots,
                VersionKind::OldBeta | VersionKind::OldAlpha => self.settings.show_old_versions,
                VersionKind::Other => false,
            })
            .cloned()
            .collect()
    }

    /// Instance name + subtitle; click to pick another instance.
    fn instance_switcher(&mut self, ui: &mut egui::Ui, instance: &Instance) {
        let p = self.palette();
        let response = ui
            .vertical(|ui| {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&instance.name)
                            .size(22.0)
                            .strong()
                            .color(p.text),
                    );
                    let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                    icons::draw(ui.painter(), Icon::ChevronDown, r, p.muted);
                });
                let subtitle = if instance.is_default() {
                    "Default instance".to_owned()
                } else {
                    format!("{} instance", instance.loader.label())
                };
                ui.label(RichText::new(subtitle).color(p.muted));
            })
            .response
            .interact(Sense::click())
            .on_hover_text("Switch instance")
            .on_hover_cursor(CursorIcon::PointingHand);
        let mut all = vec![self.instance.clone()];
        all.extend(self.custom_instances.iter().cloned());
        let mut pick = None;
        let mut create = false;
        Popup::from_toggle_button_response(&response)
            .close_behavior(PopupCloseBehavior::CloseOnClick)
            .width(300.0)
            .gap(6.0)
            .show(|ui| {
                for option in &all {
                    let (rect, row) =
                        ui.allocate_exact_size(vec2(ui.available_width(), 44.0), Sense::click());
                    if row.hovered() || option.id == instance.id {
                        ui.painter()
                            .rect_filled(rect, CornerRadius::same(8), p.surface_hover);
                    }
                    let emblem = egui::Rect::from_center_size(
                        rect.left_center() + vec2(22.0, 0.0),
                        vec2(32.0, 32.0),
                    );
                    widgets::paint_emblem(ui.painter(), p, &option.icon, emblem, 0.0);
                    ui.painter().text(
                        rect.left_center() + vec2(46.0, -8.0),
                        Align2::LEFT_CENTER,
                        &option.name,
                        FontId::proportional(14.5),
                        p.text,
                    );
                    ui.painter().text(
                        rect.left_center() + vec2(46.0, 10.0),
                        Align2::LEFT_CENTER,
                        Self::instance_subtitle(option),
                        FontId::proportional(11.5),
                        p.muted,
                    );
                    if row.clicked() {
                        pick = Some(option.id.clone());
                    }
                }
                ui.separator();
                create = ui.button("New instance…").clicked();
            });
        if let Some(id) = pick {
            self.settings.last_instance = (id != self.instance.id).then_some(id);
        }
        if create {
            self.open_create_dialog();
        }
    }

    fn account_chip(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let Some(account) = self.accounts.active().cloned() else {
            if widgets::button(ui, p, Some(Icon::Plus), "Add account", true).clicked() {
                self.add_account = AddAccount::Choose;
            }
            return;
        };
        let text = ui.painter().layout_no_wrap(
            account.username.clone(),
            FontId::proportional(14.5),
            p.text,
        );
        let size = vec2(text.size().x + 60.0, 40.0);
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());
        let hover = ui
            .ctx()
            .animate_bool(response.id.with("h"), response.hovered());
        ui.painter().rect_filled(
            rect,
            CornerRadius::same(20),
            lerp_color(p.surface, p.surface_hover, hover),
        );
        let face =
            egui::Rect::from_center_size(rect.left_center() + vec2(22.0, 0.0), vec2(26.0, 26.0));
        paint_face(ui.painter(), face, &self.face(&account), p.card_stroke);
        ui.painter().galley(
            rect.left_center() + vec2(42.0, -text.size().y / 2.0),
            text,
            p.text,
        );
        if response
            .on_hover_text("Switch account")
            .on_hover_cursor(CursorIcon::PointingHand)
            .clicked()
        {
            let now = ui.input(|i| i.time);
            self.set_tab(Tab::Accounts, now);
        }
    }

    fn version_picker(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let ManifestState::Ready(manifest) = &self.manifest else {
            match &self.manifest {
                ManifestState::Failed(e) => {
                    ui.label(RichText::new(format!("Could not load versions: {e}")).color(p.error));
                    if ui.button("Retry").clicked() {
                        self.manifest = ManifestState::Loading;
                        self.tasks.load_manifest();
                    }
                }
                _ => widgets::skeleton(ui, p, vec2(PICKER_WIDTH, 44.0), 1),
            }
            return;
        };
        let selected = self.settings.last_version.clone().unwrap_or_default();
        let latest = manifest.latest.release.clone();
        let installed = self.installed.contains(&selected);

        let (rect, response) = ui.allocate_exact_size(vec2(PICKER_WIDTH, 44.0), Sense::click());
        let hover = ui
            .ctx()
            .animate_bool(response.id.with("h"), response.hovered());
        ui.painter().rect_filled(
            rect,
            CornerRadius::same(10),
            lerp_color(p.surface, p.surface_hover, hover),
        );
        ui.painter().text(
            rect.left_center() + vec2(14.0, 0.0),
            Align2::LEFT_CENTER,
            &selected,
            FontId::proportional(17.0),
            p.text,
        );
        let (label, color) = if installed {
            ("Installed", p.accent)
        } else {
            ("Not installed", p.muted)
        };
        ui.painter().text(
            rect.right_center() - vec2(36.0, 0.0),
            Align2::RIGHT_CENTER,
            label,
            FontId::proportional(12.5),
            color,
        );
        let chevron =
            egui::Rect::from_center_size(rect.right_center() - vec2(18.0, 0.0), vec2(12.0, 12.0));
        icons::draw(ui.painter(), Icon::ChevronDown, chevron, p.muted);
        let response = response.on_hover_cursor(CursorIcon::PointingHand);

        let releases: Vec<(String, String)> = self
            .visible_versions(manifest)
            .into_iter()
            .map(|v| {
                let date: String = v.release_time.chars().take(10).collect();
                let label = match v.kind {
                    VersionKind::Release => date,
                    VersionKind::Snapshot => format!("{date} · snapshot"),
                    _ => format!("{date} · old"),
                };
                (v.id, label)
            })
            .collect();
        Popup::from_toggle_button_response(&response)
            .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
            .width(PICKER_WIDTH)
            .gap(6.0)
            .show(|ui| self.version_list(ui, &releases, &latest));
    }

    fn version_list(&mut self, ui: &mut egui::Ui, releases: &[(String, String)], latest: &str) {
        let p = self.palette();
        ui.add(
            widgets::text_field(&mut self.version_filter)
                .hint_text("Search versions")
                .desired_width(f32::INFINITY),
        );
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.installed_only, false, "All");
            ui.selectable_value(
                &mut self.installed_only,
                true,
                format!(
                    "Installed ({})",
                    releases
                        .iter()
                        .filter(|(id, _)| self.installed.contains(id))
                        .count()
                ),
            );
        });
        ui.separator();
        let filter = self.version_filter.trim().to_lowercase();
        let rows: Vec<&(String, String)> = releases
            .iter()
            .filter(|(id, _)| filter.is_empty() || id.to_lowercase().contains(&filter))
            .filter(|(id, _)| !self.installed_only || self.installed.contains(id))
            .collect();
        egui::ScrollArea::vertical()
            .max_height(300.0)
            .show(ui, |ui| {
                if rows.is_empty() {
                    ui.label(RichText::new("No matching versions").color(p.muted));
                }
                for (id, date) in rows {
                    let is_selected = self.settings.last_version.as_deref() == Some(id.as_str());
                    let (rect, response) =
                        ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::click());
                    let fill = if is_selected {
                        p.accent_deep.gamma_multiply(0.3)
                    } else if response.hovered() {
                        p.surface_hover
                    } else {
                        egui::Color32::TRANSPARENT
                    };
                    ui.painter().rect_filled(rect, CornerRadius::same(8), fill);
                    ui.painter().text(
                        rect.left_center() + vec2(10.0, 0.0),
                        Align2::LEFT_CENTER,
                        id,
                        FontId::proportional(14.5),
                        p.text,
                    );
                    ui.painter().text(
                        rect.left_center() + vec2(110.0, 0.0),
                        Align2::LEFT_CENTER,
                        date,
                        FontId::proportional(12.0),
                        p.muted,
                    );
                    let mut x = rect.right() - 10.0;
                    if self.installed.contains(id) {
                        let check = egui::Rect::from_center_size(
                            egui::pos2(x - 7.0, rect.center().y),
                            vec2(14.0, 14.0),
                        );
                        icons::draw(ui.painter(), Icon::Check, check, p.accent);
                        x -= 24.0;
                    }
                    if id == latest {
                        ui.painter().text(
                            egui::pos2(x, rect.center().y),
                            Align2::RIGHT_CENTER,
                            "LATEST",
                            FontId::proportional(11.0),
                            p.accent,
                        );
                    }
                    if response.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                        self.settings.last_version = Some(id.clone());
                        ui.close();
                    }
                }
            });
    }

    fn play_button(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let label;
        let state = match &self.launch {
            LaunchState::Preparing { progress, .. } => {
                let fraction = (progress.bytes_total > 0)
                    .then(|| progress.bytes_done as f32 / progress.bytes_total as f32);
                label = match fraction {
                    Some(f) => format!("{:.0}%", f * 100.0),
                    None => format!("{}…", progress.stage),
                };
                PlayState::Progress {
                    fraction,
                    label: &label,
                }
            }
            LaunchState::Starting { .. } => PlayState::Progress {
                fraction: None,
                label: "Starting…",
            },
            LaunchState::Running { .. } => PlayState::Running,
            LaunchState::Idle if self.blocked_reason().is_some() => PlayState::Blocked,
            LaunchState::Idle => PlayState::Ready,
        };
        let response = widgets::play_button(ui, p, state, PLAY_SIZE.into());
        let now = ui.input(|i| i.time);
        if response.clicked() {
            self.play_burst = Some((now, response.rect.center()));
            self.launch_selected();
        }
        if let Some((start, center)) = self.play_burst {
            let age = (now - start) as f32;
            if age < widgets::BURST_TIME {
                widgets::burst(ui.painter(), center, age, p.accent);
                ui.ctx().request_repaint();
            } else {
                self.play_burst = None;
            }
        }
    }

    fn quick_actions(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        if widgets::icon_button(ui, p, Icon::Folder, "Open game folder").clicked() {
            let dir = self.selected_instance().game_dir(&self.dirs);
            let _ = std::fs::create_dir_all(&dir);
            if let Err(e) = open::that_detached(&dir) {
                self.toasts.push(
                    crate::toasts::Kind::Error,
                    "Could not open folder",
                    e.to_string(),
                );
            }
        }
        let log = self
            .dirs
            .logs()
            .join(format!("game-{}.log", self.selected_instance().id));
        if log.is_file()
            && widgets::icon_button(ui, p, Icon::Document, "Open last game log").clicked()
        {
            let _ = open::that_detached(&log);
        }
        if let LaunchState::Starting { game, .. } | LaunchState::Running { game, .. } = &self.launch
            && widgets::icon_button(ui, p, Icon::Stop, "Force close Minecraft").clicked()
        {
            game.kill();
        }
    }

    fn blocked_reason(&self) -> Option<&'static str> {
        if self.update_required() {
            Some("A required launcher update must be installed first.")
        } else if self.accounts.active().is_none() {
            Some("Add an account to play.")
        } else if !matches!(self.manifest, ManifestState::Ready(_)) {
            Some("Loading versions…")
        } else {
            None
        }
    }

    fn launch_status(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let text = match &self.launch {
            LaunchState::Preparing { progress, meter } if progress.bytes_total > 0 => {
                let remaining = progress.bytes_total.saturating_sub(progress.bytes_done);
                let mut parts = vec![
                    progress.stage.clone(),
                    format!(
                        "{} of {}",
                        format_bytes(progress.bytes_done),
                        format_bytes(progress.bytes_total)
                    ),
                    format!("{}/{} files", progress.done, progress.total),
                ];
                if let Some(rate) = meter.bytes_per_sec() {
                    parts.push(format!("{}/s", format_bytes(rate as u64)));
                }
                if let Some(eta) = meter.eta_secs(remaining) {
                    parts.push(format!("{} left", format_duration(eta)));
                }
                parts.join("  ·  ")
            }
            LaunchState::Preparing { progress, .. } => format!("{}…", progress.stage),
            LaunchState::Starting { .. } => "Starting Minecraft…".into(),
            LaunchState::Running { .. } => "Minecraft is running. Have fun!".into(),
            LaunchState::Idle => match self.blocked_reason() {
                Some(reason) => reason.into(),
                None if self.selected_installed() => "Ready to play.".into(),
                None => "This version will be downloaded when you press Play.".into(),
            },
        };
        ui.label(RichText::new(text).color(p.muted));
    }

    fn selected_installed(&self) -> bool {
        self.settings
            .last_version
            .as_ref()
            .is_some_and(|v| self.installed.contains(v))
    }
}
