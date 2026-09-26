//! Settings → Network: the SOCKS5 proxy for the launcher and the game.

use arctic_core::proxy::{DEFAULT_PORT, ProxySettings};
use eframe::egui::{self, RichText};

use crate::app::ArcticApp;
use crate::theme::Palette;
use crate::toasts::Kind;
use crate::widgets;

/// State of the "Test proxy" check.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ProxyCheck {
    #[default]
    Idle,
    Running,
    Works,
    Failed(String),
}

/// Editable copy of the proxy settings; saved with the Save button.
#[derive(Debug, Clone, Default)]
pub struct NetworkUi {
    pub draft: ProxySettings,
    pub check: ProxyCheck,
    /// The settings the running check is for (a newer edit makes it stale).
    pub checking: Option<ProxySettings>,
}

impl ArcticApp {
    pub(crate) fn network_section(&mut self, ui: &mut egui::Ui, p: &Palette) {
        ui.label(
            RichText::new(
                "Send the launcher and the game through a SOCKS5 proxy. Server connections go through it with the Arctic Client (Vanilla and Fabric instances); other instances only proxy Minecraft's login services.",
            )
            .small()
            .color(p.muted),
        );
        ui.add_space(6.0);
        let draft = &mut self.network.draft;
        ui.checkbox(&mut draft.enabled, "Use a SOCKS5 proxy");
        ui.add_enabled_ui(draft.enabled, |ui| {
            widgets::field_row(ui, |ui| {
                ui.label("Address");
                ui.add(
                    widgets::text_field(&mut draft.host)
                        .hint_text("127.0.0.1 or proxy.example.com")
                        .desired_width(220.0),
                );
                ui.label("Port");
                ui.add(egui::DragValue::new(&mut draft.port).range(1..=u16::MAX));
            });
            widgets::field_row(ui, |ui| {
                ui.label("User name");
                ui.add(
                    widgets::text_field(&mut draft.username)
                        .hint_text("optional")
                        .desired_width(160.0),
                );
                ui.label("Password");
                ui.add(
                    widgets::text_field(&mut draft.password)
                        .password(true)
                        .hint_text("optional")
                        .desired_width(160.0),
                );
            });
        });
        ui.label(
            RichText::new(
                "Some servers block proxies and VPNs and may refuse to let you join. The password is kept on this PC only.",
            )
            .small()
            .color(p.warn),
        );
        ui.add_space(4.0);
        let changed = self.network.draft != self.proxy;
        let problem = self
            .network
            .draft
            .enabled
            .then(|| self.network.draft.validate().err())
            .flatten();
        ui.horizontal(|ui| {
            let can_save = changed && problem.is_none();
            if ui
                .add_enabled_ui(can_save, |ui| widgets::button(ui, p, None, "Save", true))
                .inner
                .clicked()
            {
                self.save_proxy();
            }
            let can_test = self.network.draft.enabled
                && problem.is_none()
                && self.network.check != ProxyCheck::Running;
            if ui
                .add_enabled_ui(can_test, |ui| {
                    widgets::button(ui, p, None, "Test proxy", false)
                        .on_hover_text("Reach Mojang's services through the proxy")
                })
                .inner
                .clicked()
            {
                self.test_proxy();
            }
            if changed && ui.link("Undo").clicked() {
                self.network.draft = self.proxy.clone();
                self.network.check = ProxyCheck::Idle;
            }
        });
        let status = match (&problem, &self.network.check) {
            (Some(why), _) => Some((why.clone(), p.warn)),
            (None, ProxyCheck::Running) => Some(("Testing…".to_owned(), p.muted)),
            (None, ProxyCheck::Works) => Some(("The proxy works.".to_owned(), p.accent)),
            (None, ProxyCheck::Failed(e)) => Some((format!("The proxy didn't work: {e}"), p.warn)),
            (None, ProxyCheck::Idle) => None,
        };
        if let Some((text, color)) = status {
            ui.label(RichText::new(text).small().color(color));
        }
    }

    fn save_proxy(&mut self) {
        let mut next = self.network.draft.clone();
        next.host = next.host.trim().to_owned();
        if next.port == 0 {
            next.port = DEFAULT_PORT;
        }
        next.set = arctic_core::auth::now_secs();
        if let Err(e) = next.save(&self.dirs) {
            self.toasts
                .push(Kind::Error, "Could not save the proxy", e.to_string());
            return;
        }
        if let Err(e) = arctic_core::net::set_proxy(next.active()) {
            self.toasts
                .push(Kind::Error, "The proxy can't be used", e.to_string());
        }
        let message = if next.active().is_some() {
            "Proxy on. Games started from now on use it."
        } else {
            "Proxy off."
        };
        self.toasts.push(Kind::Success, message, "");
        self.proxy = next.clone();
        self.network.draft = next;
    }

    fn test_proxy(&mut self) {
        let proxy = self.network.draft.clone();
        self.network.check = ProxyCheck::Running;
        self.network.checking = Some(proxy.clone());
        self.tasks.test_proxy(proxy);
    }

    pub(crate) fn on_proxy_tested(&mut self, tested: ProxySettings, result: Result<(), String>) {
        // Only the latest test counts, and only while it still matches.
        if self.network.checking.as_ref() != Some(&tested) {
            return;
        }
        self.network.checking = None;
        self.network.check = match result {
            Ok(()) => ProxyCheck::Works,
            Err(e) => ProxyCheck::Failed(e),
        };
    }
}
