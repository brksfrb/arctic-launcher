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
        let mut painted = false;
        if self.settings.backdrop == arctic_core::settings::Backdrop::Picture {
            let instance = self.selected_instance();
            let game_dir = instance.game_dir(&self.dirs);
            let version = instance
                .version
                .clone()
                .or_else(|| self.settings.last_version.clone());
            self.world_backdrop
                .want(&ctx, &self.dirs, game_dir, version);
            if !self.world_backdrop.missing() {
                self.world_backdrop
                    .paint(&backdrop, ctx.content_rect(), p.bg);
                painted = true;
            }
        }
        // No picture yet (a new player): the painted arctic night.
        if !painted {
            scenery::paint(
                &backdrop,
                ctx.content_rect(),
                &p.scene,
                now as f32,
                self.settings.animations,
            );
        }

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
                if self.tab == Tab::Skins {
                    // Scrolls inside itself: the character stays in view while the catalog scrolls.
                    self.skins_tab(ui);
                    return;
                }
                // A scroll position of its own for each page (one shared by all would carry the
                // Settings page's scroll over to Play).
                egui::ScrollArea::vertical()
                    .id_salt(("page_scroll", self.tab as usize))
                    .auto_shrink(false)
                    .show(ui, |ui| match self.tab {
                        Tab::Play => self.play_tab(ui),
                        Tab::Accounts => self.accounts_tab(ui),
                        Tab::Instances => self.instances_tab(ui),
                        Tab::Skins => self.skins_tab(ui),
                        Tab::Together => self.together_tab(ui),
                        Tab::Screenshots => self.screenshots_tab(ui),
                        Tab::Logs => self.logs_tab(ui),
                        Tab::Settings => self.settings_tab(ui),
                        Tab::About => self.about_tab(ui),
                    });
            });

        self.onboarding_dialog(&ctx);
        self.add_account_dialog(&ctx);
        self.remove_account_dialog(&ctx);
        self.performance_off_dialog(&ctx);
        self.client_off_dialog(&ctx);
        self.profile_dialogs(&ctx);
        self.create_instance_dialog(&ctx);
        self.suggest_dialog(&ctx);
        self.import_worlds_dialog(&ctx);
        self.share_dialogs(&ctx);
        self.migrate_dialog(&ctx);
        self.friends_tick(now, self.tab == Tab::Together);
        self.chat_tick(now, self.tab == Tab::Together);
        if let Some(action) = self.toasts.show(&ctx, p) {
            match action {
                ToastAction::ShowLogs(id) => self.show_run_log(id, now),
                ToastAction::OpenFriends => self.set_tab(Tab::Together, now),
                ToastAction::LinkAccount { into, other } => self.link_accounts(&into, &other),
                ToastAction::SendCrash(id) => self.send_crash_report(id),
                ToastAction::WhatsNew(url) => ctx.open_url(egui::OpenUrl::new_tab(url)),
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
        crate::widgets::row(ui, |ui| match &self.update {
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
                if !info.required && crate::widgets::button(ui, p, None, "Later", false).clicked() {
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
                ui.label(
                    RichText::new("Update installed: it's used next time you open the launcher.")
                        .color(p.accent),
                );
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
                if crate::widgets::button(ui, p, None, "Retry", false).clicked() {
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
        crate::tray::set_quitting();
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}
