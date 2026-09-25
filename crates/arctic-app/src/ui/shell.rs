//! Window layout: backdrop, sidebar, update banner, animated tab content,
//! dialogs and toasts.

use eframe::egui::{self, RichText};

use crate::app::{ArcticApp, Tab, UpdateState};
use crate::art::scenery;
use crate::motion::{TAB_TRANSITION, eased, format_bytes};
use crate::theme;
use crate::toasts::ToastAction;
use crate::widgets;

const NAV_WIDTH: f32 = 200.0;
/// How far (px) a tab slides while fading in.
const TAB_SLIDE: f32 = 14.0;

impl ArcticApp {
    pub(crate) fn shell(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        let backdrop = ctx.layer_painter(egui::LayerId::background());
        scenery::paint(
            &backdrop,
            ctx.content_rect(),
            &p.scene,
            now as f32,
            self.settings.animations,
        );

        let nav = egui::Panel::left("nav")
            .resizable(false)
            .show_separator_line(false)
            .exact_size(NAV_WIDTH)
            .frame(theme::nav_frame(p))
            .show(ui, |ui| self.nav(ui));
        // Only the right edge gets a border.
        let edge = nav.response.rect.right() - 0.5;
        ui.painter().vline(
            edge,
            nav.response.rect.y_range(),
            egui::Stroke::new(1.0, p.card_stroke),
        );

        if self.banner_visible() {
            egui::Panel::top("update_banner")
                .frame(theme::strip(p))
                .show(ui, |ui| self.update_banner(ui));
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(28, 24)))
            .show(ui, |ui| {
                let t = eased(now, self.tab_changed_at, TAB_TRANSITION);
                if t < 1.0 {
                    ctx.request_repaint();
                }
                ui.multiply_opacity(t);
                ui.add_space((1.0 - t) * TAB_SLIDE);
                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .show(ui, |ui| match self.tab {
                        Tab::Play => self.play_tab(ui),
                        Tab::Accounts => self.accounts_tab(ui),
                        Tab::Instances => self.instances_tab(ui),
                        Tab::Logs => self.logs_tab(ui),
                        Tab::Settings => self.settings_tab(ui),
                        Tab::About => self.about_tab(ui),
                    });
            });

        self.add_account_dialog(&ctx);
        self.remove_account_dialog(&ctx);
        if let Some(action) = self.toasts.show(&ctx, p) {
            match action {
                ToastAction::ShowLogs => {
                    self.log_source = crate::app::LogSource::Game;
                    self.set_tab(Tab::Logs, now);
                }
            }
        }
    }

    fn banner_visible(&self) -> bool {
        match &self.update {
            UpdateState::Available { dismissed, .. } => !dismissed,
            UpdateState::Installing { .. } | UpdateState::Installed { .. } => true,
            UpdateState::Failed { retry, .. } => retry.is_some(),
            _ => false,
        }
    }

    fn update_banner(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        ui.horizontal(|ui| match &self.update {
            UpdateState::Available { info, .. } => {
                let info = info.clone();
                let text = if info.required {
                    RichText::new(format!(
                        "Required update {} — install to keep playing.",
                        info.version
                    ))
                    .color(p.warn)
                } else {
                    RichText::new(format!("Arctic Launcher {} is available.", info.version))
                        .color(p.accent)
                };
                ui.label(text);
                if widgets::button(ui, p, None, "Update", true).clicked() {
                    self.start_update(info.clone());
                }
                ui.hyperlink_to("Release notes", &info.page_url);
                if !info.required && ui.button("Later").clicked() {
                    self.update = UpdateState::Available {
                        info,
                        dismissed: true,
                    };
                }
            }
            UpdateState::Installing { done, total, .. } => {
                let fraction = if *total > 0 {
                    *done as f32 / *total as f32
                } else {
                    0.0
                };
                ui.label(format!(
                    "Downloading update… {} / {}",
                    format_bytes(*done),
                    format_bytes(*total)
                ));
                ui.add(egui::ProgressBar::new(fraction).desired_width(220.0));
            }
            UpdateState::Installed { .. } => {
                ui.label(RichText::new("Update installed.").color(p.accent));
                if widgets::button(ui, p, None, "Restart now", true).clicked() {
                    self.restart_after_update(ui.ctx());
                }
            }
            UpdateState::Failed {
                error,
                retry: Some(info),
            } => {
                let info = info.clone();
                ui.label(RichText::new(format!("Update failed: {error}")).color(p.error));
                if ui.button("Retry").clicked() {
                    self.start_update(info);
                }
            }
            _ => {}
        });
    }

    fn restart_after_update(&mut self, ctx: &egui::Context) {
        if let Err(e) = arctic_core::update::restart() {
            // Keep the required flag so Play stays blocked on the old binary.
            self.toasts.push(
                crate::toasts::Kind::Error,
                "Could not restart",
                format!("{e}. Please reopen the launcher."),
            );
            return;
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}
