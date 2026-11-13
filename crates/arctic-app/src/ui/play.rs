//! Play tab: instance card, version picker, Play button and launch status.

use eframe::egui::{
    self, Align2, CornerRadius, CursorIcon, FontId, Popup, PopupCloseBehavior, RichText, Sense,
    vec2,
};

use arctic_core::instances::Instance;
use arctic_core::settings::VersionView;
use arctic_core::versions::{VersionEntry, VersionKind, VersionManifest};

use crate::app::{AddAccount, ArcticApp, ManifestState, Tab};
use crate::art::avatar::paint_face;
use crate::art::icons::{self, Icon};
use crate::art::lerp_color;
use crate::motion::{format_bytes, format_duration};
use crate::runs::{Run, RunState};
use crate::widgets::{self, PlayState};
use crate::{t, tf, tn};
const PLAY_SIZE: [f32; 2] = [300.0, 62.0];
const PICKER_WIDTH: f32 = 340.0;

/// Floored percent of one byte ratio. Never 100: a single stage's bytes being
/// finished (or 99.6% and rounded) is not the whole launch.
fn bytes_percent(done: u64, total: u64) -> Option<u8> {
    if total == 0 {
        return None;
    }
    let pct = u128::from(done.min(total)) * 100 / u128::from(total);
    Some(pct.min(99) as u8)
}

impl ArcticApp {
    pub(crate) fn play_tab(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        widgets::page_header(
            ui,
            p,
            t!("play.play"),
            t!("play.pick_an_instance_and_jump_in"),
        );
        let instance = self.selected_instance().clone();
        crate::theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                // One box for the instance: its icon, name and settings, so the gear isn't alone.
                egui::Frame::new()
                    .fill(p.surface.gamma_multiply(0.55))
                    .corner_radius(CornerRadius::same(18))
                    .inner_margin(egui::Margin::same(8))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let emblem = widgets::instance_emblem(ui, p, &instance.icon, 64.0);
                            self.icon_picker(&emblem, &instance.id);
                            ui.add_space(6.0);
                            self.instance_switcher(ui, &instance);
                            ui.add_space(14.0);
                            let (line, _) = ui.allocate_exact_size(vec2(1.0, 38.0), Sense::hover());
                            ui.painter()
                                .rect_filled(line, 0.0, p.muted.gamma_multiply(0.3));
                            ui.add_space(2.0);
                            if self.instance_settings_button(ui).clicked() {
                                let now = ui.input(|i| i.time);
                                self.set_tab(Tab::Instances, now);
                                self.open_instance(&instance.id);
                                self.inst.page = crate::ui::InstancePage::Settings;
                            }
                            ui.add_space(2.0);
                        });
                    });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    self.account_chip(ui);
                });
            });
            ui.add_space(12.0);
            ui.label(RichText::new(t!("play.version")).small().color(p.muted));
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
        // How long this instance has been played.
        if let Some(entry) = self.playtime.of(&instance.id) {
            ui.add_space(8.0);
            ui.label(
                RichText::new(tn!(
                    "play.played_session_last",
                    entry.sessions,
                    format_duration = arctic_core::playtime::format_duration(entry.total_secs),
                    value2 = arctic_core::playtime::ago(
                        entry.last_played,
                        arctic_core::auth::now_secs()
                    )
                ))
                .small()
                .color(p.muted),
            );
        }
        ui.add_space(22.0);
        ui.horizontal(|ui| {
            self.play_button(ui);
            ui.add_space(4.0);
            self.quick_actions(ui);
        });
        ui.add_space(10.0);
        self.launch_status(ui);
        self.other_runs(ui);
        self.worlds_section(ui);
        self.servers_section(ui);
        self.discover_dialog(&ui.ctx().clone());
    }

    /// The selected instance's game, while it's being prepared or runs.
    /// The selected instance's running game (for Vanilla, on the picked version).
    fn selected_run(&self) -> Option<&Run> {
        let instance = self.selected_instance();
        let version = instance
            .is_default()
            .then_some(self.settings.last_version.as_deref())
            .flatten();
        self.runs.active_for(&instance.id, version)
    }

    /// Other games running at the same time, each with its log and a stop.
    fn other_runs(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let selected = self.selected_run().map(|r| r.id);
        let others: Vec<(crate::tasks::LaunchId, String, bool)> = self
            .runs
            .active()
            .filter(|r| Some(r.id) != selected)
            .map(|r| (r.id, r.title.clone(), r.game().is_some()))
            .collect();
        if others.is_empty() {
            return;
        }
        ui.add_space(14.0);
        ui.label(
            RichText::new(t!("play.also_running"))
                .small()
                .color(p.muted),
        );
        let now = ui.input(|i| i.time);
        for (id, title, started) in others {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&title).color(p.text));
                if widgets::icon_button(ui, p, Icon::Document, t!("play.view_its_log")).clicked() {
                    self.show_run_log(id, now);
                }
                if started
                    && widgets::icon_button(ui, p, Icon::Stop, t!("play.force_close_this_game"))
                        .clicked()
                    && let Some(run) = self.runs.get_mut(id)
                {
                    run.force_close();
                }
            });
        }
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

    /// The gear that opens the instance's settings: the last part of the instance's box, after a divider.
    fn instance_settings_button(&mut self, ui: &mut egui::Ui) -> egui::Response {
        let p = self.palette();
        let (rect, response) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::click());
        let response = response
            .on_hover_text(t!("play.instance_settings"))
            .on_hover_cursor(CursorIcon::PointingHand);
        let hover = ui
            .ctx()
            .animate_bool(response.id.with("settings_h"), response.hovered());
        // Part of the instance's box: no frame of its own, only a wash when the pointer is on it.
        ui.painter().rect_filled(
            rect,
            CornerRadius::same(12),
            p.surface_hover.gamma_multiply(hover),
        );
        let icon = egui::Rect::from_center_size(rect.center(), vec2(24.0, 24.0));
        icons::draw(
            ui.painter(),
            Icon::Gear,
            icon,
            lerp_color(p.muted, p.text, hover),
        );
        response
    }

    /// Instance name + subtitle; click to pick another instance.
    fn instance_switcher(&mut self, ui: &mut egui::Ui, instance: &Instance) {
        let p = self.palette();
        // Painted behind the text once we know the hover state (last frame's).
        let bg = ui.painter().add(egui::Shape::Noop);
        let response = ui
            .vertical(|ui| {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    // Not selectable, so a click anywhere opens the switcher.
                    ui.add(
                        egui::Label::new(
                            RichText::new(&instance.name)
                                .size(22.0)
                                .strong()
                                .color(p.text),
                        )
                        .selectable(false),
                    );
                    let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                    icons::draw(ui.painter(), Icon::ChevronDown, r, p.muted);
                });
                let subtitle = if instance.is_default() {
                    t!("play.default_instance").to_owned()
                } else {
                    tf!("play.instance_2", label = instance.loader.label())
                };
                ui.add(egui::Label::new(RichText::new(subtitle).color(p.muted)).selectable(false));
            })
            .response
            .interact(Sense::click())
            .on_hover_text(t!("play.switch_instance"))
            .on_hover_cursor(CursorIcon::PointingHand);
        let hover = ui
            .ctx()
            .animate_bool(response.id.with("switch_h"), response.hovered());
        if hover > 0.0 {
            ui.painter().set(
                bg,
                egui::Shape::rect_filled(
                    response.rect.expand2(vec2(8.0, 4.0)),
                    CornerRadius::same(10),
                    p.surface_hover.gamma_multiply(hover),
                ),
            );
        }
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
                create = ui.button(t!("play.new_instance")).clicked();
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
            if widgets::button(ui, p, Some(Icon::Plus), t!("play.add_account"), true).clicked() {
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
            .on_hover_text(t!("play.switch_account"))
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
                    ui.label(
                        RichText::new(tf!("play.could_not_load_versions", e = e)).color(p.error),
                    );
                    if crate::widgets::button(ui, p, None, t!("play.retry"), false).clicked() {
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
            (t!("play.installed"), p.accent)
        } else {
            (t!("play.not_installed"), p.muted)
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
                    VersionKind::Snapshot => tf!("play.snapshot", date = date),
                    _ => tf!("play.old", date = date),
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
                .hint_text(t!("play.search_versions"))
                .desired_width(f32::INFINITY),
        );
        let mut picked = None;
        ui.horizontal(|ui| {
            let installed = releases
                .iter()
                .filter(|(id, _)| self.installed.contains(id))
                .count();
            let favorites = self.settings.favorite_versions.len();
            let tabs = [
                (VersionView::All, t!("play.all").to_string()),
                (
                    VersionView::Installed,
                    tf!("play.installed_2", installed = installed),
                ),
                (
                    VersionView::Favorites,
                    tf!("play.favorites", favorites = favorites),
                ),
            ];
            for (view, label) in tabs {
                if ui
                    .selectable_value(&mut self.version_view, view, label)
                    .clicked()
                {
                    picked = Some(view);
                }
            }
        });
        // A click is saved even on an empty tab; the session fallback never is.
        if let Some(view) = picked {
            self.settings.version_view = view;
            self.persist_settings();
        }
        ui.separator();
        let filter = self.version_filter.trim().to_lowercase();
        let rows: Vec<&(String, String)> = releases
            .iter()
            .filter(|(id, _)| filter.is_empty() || id.to_lowercase().contains(&filter))
            .filter(|(id, _)| match self.version_view {
                VersionView::All => true,
                VersionView::Installed => self.installed.contains(id),
                VersionView::Favorites => self.settings.favorite_versions.contains(id),
            })
            .collect();
        // The same height on every tab (Installed and Favorites are often short
        // lists), growing with the window.
        let height = (ui.ctx().content_rect().height() * 0.55).clamp(300.0, 520.0);
        egui::ScrollArea::vertical()
            .max_height(height)
            .min_scrolled_height(height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if rows.is_empty() {
                    let text = if self.version_view == VersionView::Favorites {
                        t!("play.star_a_version_to_keep_it_here")
                    } else {
                        t!("play.no_matching_versions")
                    };
                    ui.label(RichText::new(text).color(p.muted));
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
                    let star_rect = egui::Rect::from_center_size(
                        rect.left_center() + vec2(18.0, 0.0),
                        vec2(26.0, 26.0),
                    );
                    let star = ui
                        .interact(star_rect, response.id.with("star"), Sense::click())
                        .on_hover_text(t!("play.favorite"))
                        .on_hover_cursor(CursorIcon::PointingHand);
                    let favorite = self.settings.favorite_versions.contains(id);
                    let (icon, color) = match (favorite, star.hovered()) {
                        (true, _) => (Icon::StarFilled, p.accent),
                        (false, true) => (Icon::Star, p.text),
                        (false, false) if response.hovered() => (Icon::Star, p.muted),
                        (false, false) => (Icon::Star, p.muted.gamma_multiply(0.4)),
                    };
                    icons::draw(ui.painter(), icon, star_rect.shrink(6.0), color);
                    ui.painter().text(
                        rect.left_center() + vec2(36.0, 0.0),
                        Align2::LEFT_CENTER,
                        id,
                        FontId::proportional(14.5),
                        p.text,
                    );
                    ui.painter().text(
                        rect.left_center() + vec2(136.0, 0.0),
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
                            t!("play.latest"),
                            FontId::proportional(11.0),
                            p.accent,
                        );
                    }
                    if star.clicked() {
                        let favs = &mut self.settings.favorite_versions;
                        if favorite {
                            favs.retain(|v| v != id);
                        } else {
                            favs.push(id.clone());
                        }
                    } else if response.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                        self.settings.last_version = Some(id.clone());
                        ui.close();
                    }
                }
            });
    }

    fn play_button(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let label;
        let state = match self.selected_run().map(|r| &r.state) {
            Some(RunState::Preparing { progress, .. }) => {
                // Overall launch percent (game files, loader, mods), floored, 100 only at the end.
                let percent = progress
                    .percent
                    .or_else(|| bytes_percent(progress.bytes_done, progress.bytes_total));
                match percent {
                    Some(p) if p > 0 || progress.bytes_total > 0 => {
                        label = format!("{p}%");
                        PlayState::Progress {
                            fraction: Some(p as f32 / 100.0),
                            label: &label,
                        }
                    }
                    _ => {
                        label = format!("{}…", progress.stage);
                        PlayState::Progress {
                            fraction: None,
                            label: &label,
                        }
                    }
                }
            }
            Some(RunState::Starting { .. }) => {
                // No percentage: nothing is downloading, the game is loading.
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(250));
                PlayState::Progress {
                    fraction: None,
                    label: t!("play.starting"),
                }
            }
            Some(RunState::Running { .. }) => PlayState::Running,
            _ if self.blocked_reason().is_some() => PlayState::Blocked,
            _ => PlayState::Ready,
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
        if self.selected_run().is_some()
            && widgets::tile_button(
                ui,
                p,
                Icon::Plus,
                t!("play.play_another_copy_switch_account_first_for_a"),
                PLAY_SIZE[1],
            )
            .clicked()
        {
            self.launch_another_copy();
        }
        if widgets::tile_button(
            ui,
            p,
            Icon::Folder,
            t!("play.open_game_folder"),
            PLAY_SIZE[1],
        )
        .clicked()
        {
            let dir = self.selected_instance().game_dir(&self.dirs);
            let _ = std::fs::create_dir_all(&dir);
            if let Err(e) = open::that_detached(&dir) {
                self.toasts.push(
                    crate::toasts::Kind::Error,
                    t!("play.could_not_open_folder"),
                    e.to_string(),
                );
            }
        }
        let log = self
            .dirs
            .logs()
            .join(format!("game-{}.log", self.selected_instance().id));
        if log.is_file()
            && widgets::tile_button(
                ui,
                p,
                Icon::Document,
                t!("play.open_last_game_log"),
                PLAY_SIZE[1],
            )
            .clicked()
        {
            let _ = open::that_detached(&log);
        }
        if let Some(id) = self
            .selected_run()
            .filter(|r| r.game().is_some())
            .map(|r| r.id)
            && widgets::tile_button(
                ui,
                p,
                Icon::Stop,
                t!("play.force_close_minecraft"),
                PLAY_SIZE[1],
            )
            .clicked()
            && let Some(run) = self.runs.get_mut(id)
        {
            run.force_close();
        }
    }

    fn blocked_reason(&self) -> Option<&'static str> {
        if self.update_required() {
            Some(t!("play.a_required_launcher_update_must_be_installed"))
        } else if self.accounts.active().is_none() {
            Some(t!("play.add_an_account_to_play"))
        } else if !matches!(self.manifest, ManifestState::Ready(_)) {
            Some(t!("play.loading_versions"))
        } else {
            None
        }
    }

    fn launch_status(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let text = match self.selected_run().map(|r| &r.state) {
            Some(RunState::Preparing { progress, meter }) if progress.bytes_total > 0 => {
                let remaining = progress.bytes_total.saturating_sub(progress.bytes_done);
                let mut parts = vec![
                    progress.stage.clone(),
                    format!(
                        "{} of {}",
                        format_bytes(progress.bytes_done),
                        format_bytes(progress.bytes_total)
                    ),
                    tf!("play.files", done = progress.done, total = progress.total),
                ];
                if let Some(rate) = meter.bytes_per_sec() {
                    parts.push(format!("{}/s", format_bytes(rate as u64)));
                }
                if let Some(eta) = meter.eta_secs(remaining) {
                    parts.push(tf!("play.left", format_duration = format_duration(eta)));
                }
                parts.join("  ·  ")
            }
            Some(RunState::Preparing { progress, .. }) => format!("{}…", progress.stage),
            Some(RunState::Starting { stage, since, .. }) => {
                format!("{}…  ·  {} s", stage.label(), since.elapsed().as_secs())
            }
            Some(RunState::Running { .. }) => "Minecraft is running. Have fun!".into(),
            _ => match self.blocked_reason() {
                Some(reason) => reason.into(),
                None if self.selected_installed() => t!("play.ready_to_play").into(),
                None => t!("play.this_version_will_be_downloaded_when_you").into(),
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
