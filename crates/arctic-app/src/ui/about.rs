use arctic_core::update::GITHUB_REPO;
use arctic_core::{APP_NAME, APP_VERSION};
use eframe::egui::{self, RichText, Sense, vec2};

use crate::app::{ArcticApp, UpdateState};
use crate::art::icons::{self, Icon};
use crate::art::{brand_mark, glow};
use crate::theme;
use crate::widgets;
use crate::{t, tf};
const WEBSITE: &str = "https://arcticlauncher.com";
const DISCORD: &str = "https://discord.arcticlauncher.com";
const SUPPORT_EMAIL: &str = "support@arcticlauncher.com";

impl ArcticApp {
    pub(crate) fn about_tab(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        widgets::page_header(ui, p, t!("nav.about"), "");
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(72.0, 72.0), Sense::hover());
                let t = ui.input(|i| i.time) as f32;
                glow(ui.painter(), rect.center(), 44.0, p.accent, 0.8);
                brand_mark(ui.ctx(), ui.painter(), rect.center(), 28.0, t * 0.12);
                ui.vertical(|ui| {
                    ui.add_space(8.0);
                    ui.label(RichText::new(APP_NAME).size(24.0).strong().color(p.text));
                    ui.label(
                        RichText::new(tf!("nav.version", APP_VERSION = APP_VERSION)).color(p.muted),
                    );
                });
            });
            ui.add_space(8.0);
            ui.label(t!("nav.a_lightweight_open_source_minecraft_launcher"));
            ui.add_space(8.0);
            crate::widgets::row(ui, |ui| {
                if widgets::button(ui, p, Some(Icon::External), "arcticlauncher.com", false)
                    .clicked()
                {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(WEBSITE));
                }
                if widgets::button(
                    ui,
                    p,
                    Some(Icon::External),
                    t!("nav.source_on_github"),
                    false,
                )
                .clicked()
                {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(format!(
                        "https://github.com/{GITHUB_REPO}"
                    )));
                }
                if widgets::button(ui, p, Some(Icon::Friends), "Discord", false).clicked() {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(DISCORD));
                }
                if widgets::button(
                    ui,
                    p,
                    Some(Icon::Plus),
                    t!("nav.suggest_a_feature_or_report_a_bug"),
                    true,
                )
                .clicked()
                {
                    self.open_suggest();
                }
                if widgets::button(ui, p, Some(Icon::External), t!("nav.email_support"), false)
                    .on_hover_text(SUPPORT_EMAIL)
                    .clicked()
                {
                    ui.ctx()
                        .open_url(egui::OpenUrl::new_tab(format!("mailto:{SUPPORT_EMAIL}")));
                }
            });
            ui.add_space(6.0);
            self.sponsor_credit(ui);
            ui.add_space(8.0);
            ui.label(
                RichText::new(t!("nav.licensed_under_the_gnu_general_public"))
                    .small()
                    .color(p.muted),
            );
        });
        ui.add_space(14.0);
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(t!("nav.updates"))
                    .size(17.0)
                    .strong()
                    .color(p.text),
            );
            ui.add_space(4.0);
            self.update_status(ui);
        });
        ui.add_space(12.0);
        ui.label(
            RichText::new(tf!("nav.data_folder", root = self.dirs.root().display()))
                .small()
                .color(p.muted),
        );
    }

    fn update_status(&mut self, ui: &mut egui::Ui) {
        if arctic_core::update::managed_by_flatpak() {
            ui.label(t!("nav.updates_are_managed_by_your_linux_software"));
            return;
        }
        let p = self.palette();
        let busy = matches!(
            self.update,
            UpdateState::Checking | UpdateState::Installing { .. }
        );
        match &self.update {
            UpdateState::Idle => {}
            UpdateState::Checking => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(t!("nav.checking"));
                });
            }
            UpdateState::UpToDate => {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                    icons::draw(ui.painter(), Icon::Check, r, p.accent);
                    ui.label(RichText::new(t!("nav.youre_on_the_latest_version")).color(p.accent));
                });
            }
            UpdateState::Available { info, .. } => {
                ui.label(
                    RichText::new(tf!("nav.version_is_available", version = info.version))
                        .color(p.accent),
                );
            }
            UpdateState::Installing { done, total, .. } => {
                let fraction = if *total > 0 {
                    *done as f32 / *total as f32
                } else {
                    0.0
                };
                ui.add(
                    egui::ProgressBar::new(fraction)
                        .desired_width(320.0)
                        .show_percentage(),
                );
            }
            UpdateState::Installed { .. } => {
                ui.label(t!("nav.update_installed_restart_to_finish"));
            }
            UpdateState::Failed { error, retry } => {
                let what = if retry.is_some() {
                    t!("nav.update_failed")
                } else {
                    t!("nav.couldnt_check_for_updates")
                };
                ui.label(RichText::new(format!("{what}: {error}")).color(p.muted));
            }
        }
        ui.add_space(4.0);
        if ui
            .add_enabled_ui(!busy, |ui| {
                widgets::button(ui, p, None, t!("nav.check_for_updates"), false)
            })
            .inner
            .clicked()
        {
            self.update = UpdateState::Checking;
            self.tasks.check_update(self.settings.update_channel);
        }
    }
}
