//! "Play together": share a world opened to LAN with friends, or join
//! theirs, over a peer-to-peer tunnel (`arctic-share`).

use arctic_share::{LanWorld, SessionId, Share, ShareEvent};
use eframe::egui::{self, RichText};

use crate::app::ArcticApp;
use crate::art::icons::Icon;
use crate::theme::{self, Palette};
use crate::toasts::Kind;
use crate::widgets;
use crate::{t, tf};
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Session {
    #[default]
    Idle,
    Starting,
    Hosting {
        code: String,
        world: Option<LanWorld>,
        guests: usize,
    },
    Joining,
    Joined {
        motd: String,
        port: u16,
    },
}

#[derive(Default)]
pub struct TogetherUi {
    /// Started on first use so nothing touches the network until then.
    service: Option<Share>,
    /// Session whose events we still care about.
    current: SessionId,
    pub session: Session,
    pub(crate) code_input: String,
    port_input: String,
    error: Option<String>,
}

impl TogetherUi {
    /// The invite code while hosting.
    pub(crate) fn hosting_code(&self) -> Option<String> {
        match &self.session {
            Session::Hosting { code, .. } => Some(code.clone()),
            _ => None,
        }
    }
}

impl ArcticApp {
    /// Requests from the game (a duel), and the state it reads back.
    pub(crate) fn together_bridge(&mut self) {
        use crate::bridge_host::TogetherRequest;
        match self.bridge.take_together_request() {
            Some(TogetherRequest::Host) if self.together.hosting_code().is_none() => {
                self.stop_together();
                self.start_hosting();
            }
            Some(TogetherRequest::Join(code)) => {
                self.stop_together();
                self.together.code_input = code;
                self.start_joining();
            }
            Some(TogetherRequest::Stop) => self.stop_together(),
            _ => {}
        }
        self.bridge
            .set_together_status(together_status_json(&self.together));
    }

    pub(crate) fn together_tab(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        widgets::page_header(ui, p, t!("social.friends"), "");
        self.friends_card(ui);
        ui.add_space(16.0);
        ui.label(
            RichText::new(t!("social.play_together"))
                .size(18.0)
                .strong()
                .color(p.text),
        );
        ui.label(
            RichText::new(t!("social.invite_friends_into_your_world_no_server_and")).color(p.muted),
        );
        ui.add_space(8.0);
        if let Some(error) = self.together.error.clone() {
            ui.label(RichText::new(error).color(p.error));
            ui.add_space(6.0);
        }
        ui.columns(2, |cols| {
            theme::card(p).show(&mut cols[0], |ui| {
                ui.set_width(ui.available_width());
                self.host_card(ui);
            });
            theme::card(p).show(&mut cols[1], |ui| {
                ui.set_width(ui.available_width());
                self.join_card(ui);
            });
        });
        ui.add_space(12.0);
        ui.label(
            RichText::new(t!("social.everyone_needs_the_same_minecraft_version"))
                .small()
                .color(p.muted),
        );
        ui.add_space(12.0);
        self.sponsor_card(ui);
    }

    fn host_card(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        card_title(ui, p, t!("social.host_a_world"));
        match self.together.session.clone() {
            Session::Hosting {
                code,
                world,
                guests,
            } => {
                ui.label(
                    RichText::new(t!("social.your_invite_code"))
                        .small()
                        .color(p.muted),
                );
                // Copy first, then the code wrapping in what's left: on one line it ran out of the
                // card (and under the next one) in a narrow window.
                ui.horizontal_top(|ui| {
                    if widgets::icon_button(ui, p, Icon::Copy, t!("social.copy_code")).clicked() {
                        ui.ctx().copy_text(code.clone());
                        self.toasts
                            .push(Kind::Success, t!("social.invite_code_copied"), "");
                    }
                    ui.add(
                        egui::Label::new(
                            RichText::new(&code).monospace().size(13.0).color(p.accent),
                        )
                        .wrap(),
                    );
                });
                ui.add_space(8.0);
                match world {
                    Some(w) => {
                        ui.label(
                            RichText::new(tf!("social.sharing", motd = w.motd))
                                .strong()
                                .color(p.text),
                        );
                    }
                    None => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(
                                RichText::new(t!(
                                    "social.waiting_for_a_world_in_game_press_esc_and"
                                ))
                                .color(p.muted),
                            );
                        });
                    }
                }
                let friends = match guests {
                    0 => t!("social.no_friends_connected_yet").to_owned(),
                    1 => "1 friend connected".to_owned(),
                    n => tf!("social.friends_connected", n_value = n),
                };
                ui.label(RichText::new(friends).color(p.muted));
                ui.add_space(8.0);
                if widgets::button(ui, p, Some(Icon::Stop), t!("social.stop_sharing"), false)
                    .clicked()
                {
                    self.stop_together();
                }
            }
            Session::Starting => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new(t!("social.starting")).color(p.muted));
                });
            }
            session => {
                steps(
                    ui,
                    p,
                    &[
                        t!("social.start_a_world_in_singleplayer"),
                        t!("social.press_esc_choose_open_to_lan_then_start_lan"),
                        t!("social.send_your_friends_the_invite_code"),
                    ],
                );
                ui.add_space(8.0);
                let busy = session != Session::Idle;
                let start = ui
                    .add_enabled_ui(!busy, |ui| {
                        widgets::button(ui, p, Some(Icon::Play), t!("social.start_sharing"), true)
                    })
                    .inner;
                if start.clicked() {
                    self.start_hosting();
                }
                ui.collapsing(t!("social.share_a_specific_port"), |ui| {
                    ui.add(
                        widgets::text_field(&mut self.together.port_input)
                            .hint_text(t!("social.port_hint"))
                            .desired_width(120.0),
                    );
                    ui.label(
                        RichText::new(t!("social.leave_empty_to_find_the_lan_world"))
                            .small()
                            .color(p.muted),
                    );
                });
            }
        }
    }

    fn join_card(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        card_title(ui, p, t!("social.join_a_friend"));
        match self.together.session.clone() {
            Session::Joined { motd, port } => {
                ui.label(
                    RichText::new(tf!("social.connected_to", motd = motd))
                        .strong()
                        .color(p.text),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new(t!("social.join_the_world_it_uses_the_same_version_as"))
                        .color(p.muted),
                );
                let address = format!("localhost:{port}");
                ui.add_space(8.0);
                crate::widgets::row(ui, |ui| {
                    // Joins in the running game, or launches straight into the world.
                    if widgets::button(ui, p, Some(Icon::Play), t!("social.join_the_world"), true)
                        .on_hover_text(tf!("social.connect_to", address = address))
                        .clicked()
                    {
                        self.join_server(&address);
                    }
                    if widgets::icon_button(ui, p, Icon::Copy, t!("social.copy_address")).clicked()
                    {
                        ui.ctx().copy_text(address.clone());
                    }
                    if widgets::button(ui, p, Some(Icon::Close), t!("social.leave"), false)
                        .clicked()
                    {
                        self.stop_together();
                    }
                });
            }
            Session::Joining => {
                crate::widgets::row(ui, |ui| {
                    ui.spinner();
                    ui.label(RichText::new(t!("social.connecting_to_your_friend")).color(p.muted));
                });
                if crate::widgets::button(ui, p, None, t!("social.cancel"), false).clicked() {
                    self.stop_together();
                }
            }
            session => {
                ui.label(
                    RichText::new(t!("social.paste_the_invite_code_your_friend_sent_you"))
                        .color(p.muted),
                );
                ui.add_space(6.0);
                let field = ui.add(
                    widgets::text_field(&mut self.together.code_input)
                        .hint_text("arctic-xxxx-xxxx-…") // i18n-ignore
                        .desired_width(f32::INFINITY),
                );
                let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                ui.add_space(8.0);
                let ready = session == Session::Idle && !self.together.code_input.trim().is_empty();
                let join = ui
                    .add_enabled_ui(ready, |ui| {
                        widgets::button(ui, p, Some(Icon::Play), t!("social.join"), true)
                    })
                    .inner;
                if ready && (join.clicked() || enter) {
                    self.start_joining();
                }
            }
        }
    }

    /// Short status for Discord while sharing or joined.
    pub(crate) fn together_status(&self) -> Option<String> {
        match &self.together.session {
            Session::Hosting { guests: 0, .. } => Some(t!("social.hosting_a_world").into()),
            Session::Hosting { guests: 1, .. } => {
                Some(t!("social.hosting_a_world_1_friend").into())
            }
            Session::Hosting { guests, .. } => {
                Some(tf!("social.hosting_a_world_friends", guests = guests))
            }
            Session::Joined { .. } => Some(t!("social.playing_with_friends").into()),
            _ => None,
        }
    }

    fn share_service(&mut self) -> Option<&Share> {
        if self.together.service.is_none() {
            match Share::new(self.tasks.share_sink()) {
                Ok(s) => self.together.service = Some(s),
                Err(e) => {
                    self.together.error = Some(tf!("social.could_not_start_networking", e = e));
                    return None;
                }
            }
        }
        self.together.service.as_ref()
    }

    pub(crate) fn start_hosting(&mut self) {
        let port = match self.together.port_input.trim() {
            "" => None,
            text => match text.parse::<u16>() {
                Ok(p) if p > 0 => Some(p),
                _ => {
                    self.together.error =
                        Some(t!("social.that_port_isnt_a_number_from_1_to_65535").into());
                    return;
                }
            },
        };
        let key_path = self.dirs.profile_root().join("share.key");
        let key = match arctic_share::load_or_create_key(&key_path) {
            Ok(k) => k,
            Err(e) => {
                self.together.error = Some(tf!("social.could_not_create_your_invite_key", e = e));
                return;
            }
        };
        self.together.error = None;
        arctic_share::lan::watch_logs_under(self.dirs.profile_root().to_path_buf());
        if let Some(service) = self.share_service() {
            self.together.current = service.host(key, port);
            self.together.session = Session::Starting;
        }
    }

    pub(crate) fn start_joining(&mut self) {
        let code = self.together.code_input.trim().to_owned();
        self.together.error = None;
        let Some(service) = self.share_service() else {
            return;
        };
        match service.join(&code) {
            Ok(id) => {
                self.together.current = id;
                self.together.session = Session::Joining;
            }
            Err(e) => self.together.error = Some(e),
        }
    }

    pub(crate) fn stop_together(&mut self) {
        if let Some(service) = &self.together.service {
            service.stop();
        }
        self.together.current = 0;
        self.together.session = Session::Idle;
    }

    pub(crate) fn on_share_event(&mut self, session: SessionId, event: ShareEvent) {
        let t = &mut self.together;
        if session != t.current {
            return;
        }
        match event {
            ShareEvent::HostReady { code } => {
                t.session = Session::Hosting {
                    code,
                    world: None,
                    guests: 0,
                };
            }
            ShareEvent::HostWorld(found) => {
                if let Session::Hosting { world, .. } = &mut t.session {
                    *world = found;
                }
            }
            ShareEvent::Guests(n) => {
                if let Session::Hosting { guests, .. } = &mut t.session {
                    if n > *guests {
                        self.toasts
                            .push(Kind::Info, t!("social.a_friend_joined"), "");
                    }
                    *guests = n;
                }
            }
            ShareEvent::Joined { motd, port } => {
                t.session = Session::Joined { motd, port };
                self.toasts.push(
                    Kind::Success,
                    t!("social.connected"),
                    t!("social.the_world_now_shows_up_in_multiplayer"),
                );
            }
            ShareEvent::Stopped { error } => {
                t.session = Session::Idle;
                if let Some(e) = error {
                    t.error = Some(e);
                }
            }
        }
    }
}

/// Play together's state for the game: `{"state": "idle" | "starting" |
/// "hosting" | "joining" | "joined", "code", "guests", "port", "error"}`.
fn together_status_json(t: &TogetherUi) -> String {
    let mut out = match &t.session {
        Session::Idle => serde_json::json!({"state": "idle"}),
        Session::Starting => serde_json::json!({"state": "starting"}),
        Session::Hosting {
            code,
            world,
            guests,
        } => serde_json::json!({
            "state": "hosting",
            "code": code,
            "guests": guests,
            "world": world.as_ref().map(|w| w.motd.clone()),
        }),
        Session::Joining => serde_json::json!({"state": "joining"}),
        Session::Joined { motd, port } => {
            serde_json::json!({"state": "joined", "motd": motd, "port": port})
        }
    };
    if let Some(e) = &t.error {
        out["error"] = serde_json::json!(e);
    }
    out.to_string()
}

fn card_title(ui: &mut egui::Ui, p: &Palette, title: &str) {
    ui.label(RichText::new(title).size(18.0).strong().color(p.text));
    ui.add_space(6.0);
}

fn steps(ui: &mut egui::Ui, p: &Palette, lines: &[&str]) {
    for (i, line) in lines.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{}.", i + 1)).color(p.accent));
            ui.label(RichText::new(*line).color(p.text));
        });
    }
}
