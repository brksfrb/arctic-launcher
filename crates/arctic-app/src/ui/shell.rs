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
/// Space between a page and the right edge of the window (the scroll bar sits on the edge itself).
const PAGE_SIDE: i8 = 28;

impl ArcticApp {
    pub(crate) fn shell(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        let backdrop = ctx.layer_painter(egui::LayerId::background());
        let mut painted = false;
        use arctic_core::settings::Backdrop;
        let custom = (self.settings.backdrop == Backdrop::Custom)
            .then(|| self.settings.custom_backdrop.clone())
            .flatten();
        if self.settings.backdrop == Backdrop::Picture || custom.is_some() {
            let instance = self.selected_instance();
            let game_dir = instance.game_dir(&self.dirs);
            let version = instance
                .version
                .clone()
                .or_else(|| self.settings.last_version.clone());
            self.world_backdrop
                .want(&ctx, &self.dirs, game_dir, version, custom);
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
                self.animated(),
            );
        }

        // Glass: the backdrop, blurred, behind every card and the sidebar (found once laid out).
        if p.glass && !self.software_gl {
            backdrop.add(crate::glass::backdrop_callback(ctx.content_rect()));
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
            // No margin on the right: the scroll bar sits on the window's edge, and the page keeps its own
            // space from it (PAGE_SIDE) inside the scroll area.
            .frame(egui::Frame::new().inner_margin(egui::Margin {
                left: PAGE_SIDE,
                right: 0,
                top: 24,
                bottom: 24,
            }))
            .show(ui, |ui| {
                let t = eased(now, self.tab_changed_at, TAB_TRANSITION);
                if t < 1.0 {
                    ctx.request_repaint();
                }
                ui.multiply_opacity(t);
                ui.add_space((1.0 - t) * TAB_SLIDE);
                if self.tab == Tab::Skins {
                    // Scrolls inside itself: the character stays in view while the catalog scrolls.
                    egui::Frame::new()
                        .inner_margin(egui::Margin {
                            right: PAGE_SIDE,
                            ..egui::Margin::ZERO
                        })
                        .show(ui, |ui| self.skins_tab(ui));
                    return;
                }
                // A scroll position of its own for each page (one shared by all would carry the
                // Settings page's scroll over to Play).
                egui::ScrollArea::vertical()
                    .id_salt(("page_scroll", self.tab as usize))
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        egui::Frame::new()
                            .inner_margin(egui::Margin {
                                right: PAGE_SIDE,
                                ..egui::Margin::ZERO
                            })
                            .show(ui, |ui| match self.tab {
                                Tab::Play => self.play_tab(ui),
                                Tab::Accounts => self.accounts_tab(ui),
                                Tab::Instances => self.instances_tab(ui),
                                Tab::Bedrock => self.bedrock_tab(ui),
                                Tab::Skins => self.skins_tab(ui),
                                Tab::Together => self.together_tab(ui),
                                Tab::Screenshots => self.screenshots_tab(ui),
                                Tab::Stats => self.stats_tab(ui),
                                Tab::Logs => self.logs_tab(ui),
                                Tab::Settings => self.settings_tab(ui),
                                Tab::About => self.about_tab(ui),
                            });
                    });
            });

        self.onboarding_dialog(&ctx);
        self.add_account_dialog(&ctx);
        self.remove_account_dialog(&ctx);
        self.performance_off_dialog(&ctx);
        self.gallery_name_dialog(&ctx);
        self.send_logs_dialog(&ctx);
        self.client_off_dialog(&ctx);
        self.game_switch_dialog(&ctx);
        self.profile_dialogs(&ctx);
        self.create_instance_dialog(&ctx);
        self.suggest_dialog(&ctx);
        self.import_worlds_dialog(&ctx);
        self.share_dialogs(&ctx);
        self.migrate_dialog(&ctx);
        self.storage_dialog(&ctx);
        self.whats_new_dialog(&ctx);
        self.close_prompt_dialog(&ctx);
        self.join_confirm_dialog(&ctx);
        self.world_confirm_dialog(&ctx);
        self.mod_clash_dialog(&ctx);
        self.required_update_overlay(&ctx);
        if p.glass && !self.software_gl {
            crate::glass::note_panes(&ctx, &[p.card_fill, p.nav_fill]);
        }
        self.friends_tick(now, self.tab == Tab::Together);
        self.chat_tick(now, self.tab == Tab::Together);
        if let Some(action) = self.toasts.show(&ctx, p) {
            match action {
                ToastAction::ShowLogs(id) => self.show_run_log(id, now),
                ToastAction::OpenFriends => self.set_tab(Tab::Together, now),
                ToastAction::LinkAccount { into, other } => self.link_accounts(&into, &other),
                ToastAction::SendCrash(id) => self.send_crash_report(id),
                ToastAction::WhatsNew(url) => ctx.open_url(egui::OpenUrl::new_tab(url)),
                ToastAction::Fix { instance, fix } => self.apply_crash_fix(&instance, &fix),
            }
        }
    }

    fn banner_visible(&self) -> bool {
        update_banner_visible(&self.update, self.settings.auto_install_updates)
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

    pub(crate) fn restart_after_update(&mut self, ctx: &egui::Context) {
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

// Required updates retain their dedicated overlay; normal automatic updates stay quiet.
fn update_banner_visible(state: &UpdateState, auto_install: bool) -> bool {
    match state {
        UpdateState::Available { info, dismissed } => !info.required && !dismissed,
        UpdateState::Installing { info, .. } => !info.required && !auto_install,
        UpdateState::Installed { required } => !required && !auto_install,
        UpdateState::Failed {
            retry: Some(info), ..
        } => !info.required && !auto_install,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arctic_core::update::UpdateInfo;

    fn info(required: bool) -> UpdateInfo {
        UpdateInfo {
            version: "1.2.3".parse().unwrap(),
            required,
            notes: String::new(),
            page_url: String::new(),
            asset_url: String::new(),
            asset_size: 0,
            sha256: String::new(),
        }
    }

    #[test]
    fn normal_install_banners_follow_auto_install_setting() {
        for state in [
            UpdateState::Installing {
                info: info(false),
                done: 0,
                total: 1,
            },
            UpdateState::Installed { required: false },
            UpdateState::Failed {
                error: "failed".into(),
                retry: Some(info(false)),
            },
        ] {
            assert!(!update_banner_visible(&state, true));
            assert!(update_banner_visible(&state, false));
        }
    }

    #[test]
    fn required_updates_use_overlay_and_available_updates_can_be_dismissed() {
        for auto in [false, true] {
            for state in [
                UpdateState::Available {
                    info: info(true),
                    dismissed: false,
                },
                UpdateState::Installing {
                    info: info(true),
                    done: 0,
                    total: 1,
                },
                UpdateState::Installed { required: true },
                UpdateState::Failed {
                    error: "failed".into(),
                    retry: Some(info(true)),
                },
                UpdateState::Failed {
                    error: "check failed".into(),
                    retry: None,
                },
                UpdateState::Idle,
                UpdateState::Checking,
                UpdateState::UpToDate,
            ] {
                assert!(!update_banner_visible(&state, auto));
            }
            for dismissed in [false, true] {
                assert_eq!(
                    update_banner_visible(
                        &UpdateState::Available {
                            info: info(false),
                            dismissed
                        },
                        auto
                    ),
                    !dismissed
                );
            }
        }
    }
}
