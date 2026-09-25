//! "Play together": share a world opened to LAN with friends, or join
//! theirs, over a peer-to-peer tunnel (`arctic-share`).

use arctic_share::{LanWorld, SessionId, Share, ShareEvent};
use eframe::egui::{self, RichText};

use crate::app::{ArcticApp, LaunchState};
use crate::art::icons::Icon;
use crate::theme::{self, Palette};
use crate::toasts::Kind;
use crate::widgets;

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
    code_input: String,
    port_input: String,
    error: Option<String>,
}

impl ArcticApp {
    pub(crate) fn together_tab(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        widgets::page_header(
            ui,
            p,
            "Play together",
            "Invite friends into your world. No server and no port forwarding.",
        );
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
            RichText::new(
                "Everyone needs the same Minecraft version and mods. Connections are direct \
                 between your PCs when possible, and relayed (still encrypted) when not.",
            )
            .small()
            .color(p.muted),
        );
    }

    fn host_card(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        card_title(ui, p, "Host a world");
        match self.together.session.clone() {
            Session::Hosting {
                code,
                world,
                guests,
            } => {
                ui.label(RichText::new("Your invite code").small().color(p.muted));
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&code).monospace().size(13.0).color(p.accent));
                    if widgets::icon_button(ui, p, Icon::Copy, "Copy code").clicked() {
                        ui.ctx().copy_text(code.clone());
                        self.toasts.push(Kind::Success, "Invite code copied", "");
                    }
                });
                ui.add_space(8.0);
                match world {
                    Some(w) => {
                        ui.label(
                            RichText::new(format!("Sharing \"{}\"", w.motd))
                                .strong()
                                .color(p.text),
                        );
                    }
                    None => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(
                                RichText::new("Waiting for a world. In game, press Esc and choose Open to LAN.")
                                    .color(p.muted),
                            );
                        });
                    }
                }
                let friends = match guests {
                    0 => "No friends connected yet".to_owned(),
                    1 => "1 friend connected".to_owned(),
                    n => format!("{n} friends connected"),
                };
                ui.label(RichText::new(friends).color(p.muted));
                ui.add_space(8.0);
                if widgets::button(ui, p, Some(Icon::Stop), "Stop sharing", false).clicked() {
                    self.stop_together();
                }
            }
            Session::Starting => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new("Starting…").color(p.muted));
                });
            }
            session => {
                steps(
                    ui,
                    p,
                    &[
                        "Start a world in singleplayer.",
                        "Press Esc, choose Open to LAN, then Start LAN World.",
                        "Send your friends the invite code.",
                    ],
                );
                ui.add_space(8.0);
                let busy = session != Session::Idle;
                let start = ui
                    .add_enabled_ui(!busy, |ui| {
                        widgets::button(ui, p, Some(Icon::Play), "Start sharing", true)
                    })
                    .inner;
                if start.clicked() {
                    self.start_hosting();
                }
                ui.collapsing("Share a specific port", |ui| {
                    ui.add(
                        widgets::text_field(&mut self.together.port_input)
                            .hint_text("e.g. 25565")
                            .desired_width(120.0),
                    );
                    ui.label(
                        RichText::new("Leave empty to find the LAN world automatically.")
                            .small()
                            .color(p.muted),
                    );
                });
            }
        }
    }

    fn join_card(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        card_title(ui, p, "Join a friend");
        match self.together.session.clone() {
            Session::Joined { motd, port } => {
                ui.label(
                    RichText::new(format!("Connected to \"{motd}\""))
                        .strong()
                        .color(p.text),
                );
                ui.add_space(4.0);
                steps(
                    ui,
                    p,
                    &[
                        "Start Minecraft (same version as your friend).",
                        "Open Multiplayer. The world is listed under the LAN section.",
                    ],
                );
                let address = format!("localhost:{port}");
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Or Direct Connect to").color(p.muted));
                    ui.label(RichText::new(&address).monospace().color(p.accent));
                    if widgets::icon_button(ui, p, Icon::Copy, "Copy address").clicked() {
                        ui.ctx().copy_text(address.clone());
                    }
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let idle = matches!(self.launch, LaunchState::Idle);
                    let play = ui
                        .add_enabled_ui(idle, |ui| {
                            widgets::button(ui, p, Some(Icon::Play), "Launch Minecraft", true)
                        })
                        .inner;
                    if play.clicked() {
                        self.launch_selected();
                    }
                    if widgets::button(ui, p, Some(Icon::Close), "Leave", false).clicked() {
                        self.stop_together();
                    }
                });
            }
            Session::Joining => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new("Connecting to your friend…").color(p.muted));
                });
                if ui.button("Cancel").clicked() {
                    self.stop_together();
                }
            }
            session => {
                ui.label(
                    RichText::new("Paste the invite code your friend sent you.").color(p.muted),
                );
                ui.add_space(6.0);
                let field = ui.add(
                    widgets::text_field(&mut self.together.code_input)
                        .hint_text("arctic-xxxx-xxxx-…")
                        .desired_width(f32::INFINITY),
                );
                let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                ui.add_space(8.0);
                let ready = session == Session::Idle && !self.together.code_input.trim().is_empty();
                let join = ui
                    .add_enabled_ui(ready, |ui| {
                        widgets::button(ui, p, Some(Icon::Play), "Join", true)
                    })
                    .inner;
                if ready && (join.clicked() || enter) {
                    self.start_joining();
                }
            }
        }
    }

    fn share_service(&mut self) -> Option<&Share> {
        if self.together.service.is_none() {
            match Share::new(self.tasks.share_sink()) {
                Ok(s) => self.together.service = Some(s),
                Err(e) => {
                    self.together.error = Some(format!("Could not start networking: {e}"));
                    return None;
                }
            }
        }
        self.together.service.as_ref()
    }

    fn start_hosting(&mut self) {
        let port = match self.together.port_input.trim() {
            "" => None,
            text => match text.parse::<u16>() {
                Ok(p) if p > 0 => Some(p),
                _ => {
                    self.together.error = Some("That port isn't a number from 1 to 65535.".into());
                    return;
                }
            },
        };
        let key_path = self.dirs.profile_root().join("share.key");
        let key = match arctic_share::load_or_create_key(&key_path) {
            Ok(k) => k,
            Err(e) => {
                self.together.error = Some(format!("Could not create your invite key: {e}"));
                return;
            }
        };
        self.together.error = None;
        if let Some(service) = self.share_service() {
            self.together.current = service.host(key, port);
            self.together.session = Session::Starting;
        }
    }

    fn start_joining(&mut self) {
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

    fn stop_together(&mut self) {
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
                        self.toasts.push(Kind::Info, "A friend joined", "");
                    }
                    *guests = n;
                }
            }
            ShareEvent::Joined { motd, port } => {
                t.session = Session::Joined { motd, port };
                self.toasts.push(
                    Kind::Success,
                    "Connected",
                    "The world now shows up in Multiplayer.",
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
