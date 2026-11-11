//! Modal dialogs: add account (Microsoft browser / code, offline) and
//! remove-account confirmation.

#[cfg(feature = "offline-accounts")]
use arctic_core::auth::offline;
use eframe::egui::{
    self, Align2, CornerRadius, FontId, Id, Modal, RichText, Sense, Stroke, StrokeKind, vec2,
};

use crate::app::{AddAccount, ArcticApp};
use crate::art::icons::{self, Icon};
use crate::art::lerp_color;
use crate::theme::Palette;
use crate::toasts::Kind;
use crate::widgets;
use crate::{t, tf};
const DIALOG_WIDTH: f32 = 420.0;

enum Choice {
    Browser,
    Code,
    #[cfg(feature = "offline-accounts")]
    Offline,
}

impl ArcticApp {
    pub(crate) fn add_account_dialog(&mut self, ctx: &egui::Context) {
        if matches!(self.add_account, AddAccount::Closed) {
            return;
        }
        let p = self.palette();
        let modal = Modal::new(Id::new("add_account"))
            .frame(dialog_frame(p))
            .show(ctx, |ui| {
                ui.set_width(DIALOG_WIDTH);
                ui.label(
                    RichText::new(t!("play.add_account"))
                        .size(22.0)
                        .strong()
                        .color(p.text),
                );
                ui.add_space(10.0);
                match &self.add_account {
                    AddAccount::Choose => self.choose_view(ui),
                    #[cfg(feature = "offline-accounts")]
                    AddAccount::Offline { .. } => self.offline_view(ui),
                    AddAccount::Microsoft { .. } => self.microsoft_view(ui),
                    AddAccount::Failed(e) => {
                        let e = e.clone();
                        ui.label(
                            RichText::new(t!("play.sign_in_failed"))
                                .strong()
                                .color(p.error),
                        );
                        ui.label(RichText::new(e).color(p.muted));
                        ui.add_space(8.0);
                        if widgets::button(ui, p, None, t!("play.try_again"), true).clicked() {
                            self.add_account = AddAccount::Choose;
                        }
                    }
                    AddAccount::Closed => {}
                }
            });
        if modal.should_close() {
            self.cancel_login();
            self.add_account = AddAccount::Closed;
        }
    }

    fn choose_view(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let ms_note = if self.msa_configured {
            None
        } else {
            Some(t!("play.microsoft_sign_in_isnt_available_in_this"))
        };
        #[cfg_attr(not(feature = "offline-accounts"), allow(unused_mut))]
        let mut options = vec![
            (
                Choice::Browser,
                Icon::External,
                "Microsoft",
                t!("play.sign_in_with_your_browser_recommended"),
                ms_note,
            ),
            (
                Choice::Code,
                Icon::Copy,
                t!("play.microsoft_with_a_code"),
                t!("play.enter_a_short_code_on_any_device"),
                ms_note,
            ),
        ];
        #[cfg(feature = "offline-accounts")]
        {
            options.push((
                Choice::Offline,
                Icon::User,
                t!("play.offline"),
                t!("play.singleplayer_and_offline_mode_servers"),
                None,
            ));
        }
        for (choice, icon, title, desc, disabled) in options {
            if option_tile(
                ui,
                p,
                icon,
                title,
                disabled.unwrap_or(desc),
                disabled.is_none(),
                false,
            ) {
                match choice {
                    Choice::Browser => self.start_login(true),
                    Choice::Code => self.start_login(false),
                    #[cfg(feature = "offline-accounts")]
                    Choice::Offline => {
                        self.add_account = AddAccount::Offline {
                            name: String::new(),
                        }
                    }
                }
            }
            ui.add_space(6.0);
        }
    }

    #[cfg(feature = "offline-accounts")]
    fn offline_view(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let AddAccount::Offline { name } = &mut self.add_account else {
            return;
        };
        ui.label(RichText::new(t!("play.choose_a_username")).color(p.muted));
        let response = ui.add(
            widgets::text_field(name)
                .hint_text("Steve")
                .char_limit(16)
                .desired_width(f32::INFINITY),
        );
        response.request_focus();
        let valid = offline::validate_username(name.trim()).is_ok();
        let hint = if name.is_empty() || valid {
            t!("play.1_16_letters_digits_or")
        } else {
            t!("play.only_letters_digits_and_max_16")
        };
        ui.label(
            RichText::new(hint)
                .small()
                .color(if valid || name.is_empty() {
                    p.muted
                } else {
                    p.warn
                }),
        );
        ui.add_space(10.0);
        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
        let name = name.clone();
        crate::widgets::row(ui, |ui| {
            if crate::widgets::button(ui, p, None, t!("play.back"), false).clicked() {
                self.add_account = AddAccount::Choose;
            }
            let add = ui
                .add_enabled_ui(valid, |ui| {
                    widgets::button(ui, p, Some(Icon::Plus), t!("play.add"), true)
                })
                .inner;
            if valid
                && (add.clicked() || enter)
                && let Ok(account) = offline::create(&name)
            {
                self.add_account = AddAccount::Closed;
                self.toasts.push(
                    Kind::Success,
                    tf!("play.added", username = account.username),
                    "Offline account",
                );
                self.offer_profile_link(&account);
                self.accounts.upsert(account);
                self.save_accounts();
            }
        });
    }

    fn microsoft_view(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let AddAccount::Microsoft {
            device_code,
            browser,
            ..
        } = &self.add_account
        else {
            return;
        };
        let (code, browser) = (device_code.clone(), *browser);
        match (browser, code) {
            (true, _) => waiting(ui, p, t!("play.finish_signing_in_in_your_browser")),
            (false, None) => waiting(ui, p, t!("play.getting_a_sign_in_code")),
            (false, Some(code)) => {
                ui.label(
                    RichText::new(tf!(
                        "play.go_to_and_enter",
                        verification_uri = code.verification_uri
                    ))
                    .color(p.muted),
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&code.user_code)
                            .size(30.0)
                            .monospace()
                            .strong()
                            .color(p.accent),
                    );
                    if widgets::icon_button(ui, p, Icon::Copy, t!("play.copy_code")).clicked() {
                        ui.ctx().copy_text(code.user_code.clone());
                        self.toasts.push(Kind::Info, t!("play.code_copied"), "");
                    }
                });
                ui.add_space(6.0);
                if widgets::button(
                    ui,
                    p,
                    Some(Icon::External),
                    t!("play.open_sign_in_page"),
                    true,
                )
                .clicked()
                {
                    ui.ctx()
                        .open_url(egui::OpenUrl::new_tab(&code.verification_uri));
                }
                ui.add_space(4.0);
                waiting(ui, p, t!("play.waiting_for_you_to_finish"));
            }
        }
        ui.add_space(8.0);
        if crate::widgets::button(ui, p, None, t!("play.cancel"), false).clicked() {
            self.cancel_login();
        }
    }

    pub(crate) fn remove_account_dialog(&mut self, ctx: &egui::Context) {
        let Some(id) = self.remove_confirm.clone() else {
            return;
        };
        let Some(account) = self.accounts.accounts.iter().find(|a| a.id == id).cloned() else {
            self.remove_confirm = None;
            return;
        };
        let p = self.palette();
        let modal = Modal::new(Id::new("remove_account"))
            .frame(dialog_frame(p))
            .show(ctx, |ui| {
                ui.set_width(360.0);
                ui.label(
                    RichText::new(tf!("play.remove", username = account.username))
                        .size(20.0)
                        .strong(),
                );
                ui.label(RichText::new(t!("play.you_can_add_it_again_at_any_time")).color(p.muted));
                ui.add_space(12.0);
                crate::widgets::row(ui, |ui| {
                    if crate::widgets::button(ui, p, None, t!("play.cancel"), false).clicked() {
                        self.remove_confirm = None;
                    }
                    if widgets::button(ui, p, Some(Icon::Trash), t!("play.remove_2"), true)
                        .clicked()
                    {
                        self.accounts.remove(&id);
                        self.save_accounts();
                        self.remove_confirm = None;
                        self.toasts.push(
                            Kind::Info,
                            tf!("play.removed", username = account.username),
                            "",
                        );
                    }
                });
            });
        if modal.should_close() {
            self.remove_confirm = None;
        }
    }

    /// "Turn off performance mods?": asked before the switch goes off.
    pub(crate) fn performance_off_dialog(&mut self, ctx: &egui::Context) {
        let Some(id) = self.performance_off_confirm.clone() else {
            return;
        };
        let p = self.palette();
        let modal = Modal::new(Id::new("performance_off"))
            .frame(dialog_frame(p))
            .show(ctx, |ui| {
                ui.set_width(380.0);
                ui.label(
                    RichText::new(t!("play.turn_off_performance_mods"))
                        .size(20.0)
                        .strong(),
                );
                ui.label(
                    RichText::new(t!("play.sodium_lithium_and_the_rest_are_removed_from"))
                        .color(p.muted),
                );
                ui.add_space(12.0);
                crate::widgets::row(ui, |ui| {
                    if crate::widgets::button(ui, p, None, t!("play.keep_them"), false).clicked() {
                        self.performance_off_confirm = None;
                    }
                    if widgets::button(ui, p, None, t!("play.turn_off"), true).clicked() {
                        self.update_instance(&id, |i| i.performance = false);
                        self.performance_off_confirm = None;
                    }
                });
            });
        if modal.should_close() {
            self.performance_off_confirm = None;
        }
    }

    /// "Turn off the Arctic Client?": asked before it goes off in an instance.
    pub(crate) fn client_off_dialog(&mut self, ctx: &egui::Context) {
        let Some(id) = self.client_off_confirm.clone() else {
            return;
        };
        let vanilla = self
            .instance_mut(&id)
            .is_none_or(|i| i.loader.kind().is_none());
        let (title, about) = if vanilla {
            (
                t!("play.turn_off_the_arctic_client"),
                t!("play.the_game_runs_without_arctics_menus_hud"),
            )
        } else {
            (
                t!("play.turn_off_the_arctic_mod"),
                t!("play.you_wont_see_other_players_arctic_looks"),
            )
        };
        let p = self.palette();
        let modal = Modal::new(Id::new("client_off"))
            .frame(dialog_frame(p))
            .show(ctx, |ui| {
                ui.set_width(380.0);
                ui.label(RichText::new(title).size(20.0).strong());
                ui.label(RichText::new(about).color(p.muted));
                ui.add_space(12.0);
                crate::widgets::row(ui, |ui| {
                    if crate::widgets::button(ui, p, None, t!("play.keep_it"), false).clicked() {
                        self.client_off_confirm = None;
                    }
                    if widgets::button(ui, p, None, t!("play.turn_off"), true).clicked() {
                        self.update_instance(&id, |i| i.arctic_mod = false);
                        self.client_off_confirm = None;
                    }
                });
            });
        if modal.should_close() {
            self.client_off_confirm = None;
        }
    }
}

pub(super) fn dialog_frame(p: &Palette) -> egui::Frame {
    egui::Frame::new()
        .fill(crate::theme::dialog_fill(p))
        .stroke(Stroke::new(1.0, p.card_stroke))
        .corner_radius(CornerRadius::same(16))
        .inner_margin(22.0)
}

/// Large clickable option; returns true when clicked.
pub(super) fn option_tile(
    ui: &mut egui::Ui,
    p: &Palette,
    icon: Icon,
    title: &str,
    desc: &str,
    enabled: bool,
    selected: bool,
) -> bool {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 64.0), sense);
    let hover = ui
        .ctx()
        .animate_bool(response.id.with("h"), enabled && response.hovered());
    let painter = ui.painter();
    painter.rect_filled(
        rect,
        CornerRadius::same(12),
        lerp_color(p.bg, p.surface_hover, 0.4 + 0.6 * hover),
    );
    painter.rect_stroke(
        rect,
        CornerRadius::same(12),
        if selected {
            Stroke::new(2.0, p.accent)
        } else {
            Stroke::new(1.0, lerp_color(p.card_stroke, p.accent, hover))
        },
        StrokeKind::Inside,
    );
    let fg = if enabled { p.text } else { p.muted };
    let icon_rect =
        egui::Rect::from_center_size(rect.left_center() + vec2(28.0, 0.0), vec2(20.0, 20.0));
    icons::draw(
        painter,
        icon,
        icon_rect,
        if enabled { p.accent } else { p.muted },
    );
    painter.text(
        rect.left_center() + vec2(54.0, -10.0),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(15.5),
        fg,
    );
    // Long descriptions are cut to fit, never drawn past the tile.
    crate::widgets::text_elided(
        painter,
        rect.left_center() + vec2(54.0, 11.0),
        desc,
        FontId::proportional(12.5),
        p.muted,
        rect.width() - 54.0 - 14.0,
    );
    enabled
        && response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
}

fn waiting(ui: &mut egui::Ui, p: &Palette, text: &str) {
    ui.horizontal(|ui| {
        ui.spinner();
        ui.label(RichText::new(text).color(p.muted));
    });
}
