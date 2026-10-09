//! Friends: your Arctic profile (accounts linked into one person, and what
//! friends may see), friend requests, invites and the friend list with
//! who's online and where, a Join button and invites.

use std::collections::HashSet;

use arctic_core::friends::{Friend, InviteTo, Overview, Profile, Settings};
use arctic_core::launch::QuickPlay;
use eframe::egui::{self, Color32, RichText, vec2};

use crate::app::ArcticApp;
use crate::art::icons::Icon;
use crate::friend_tasks::{FriendAction, FriendsView};
use crate::tasks::Outcome;
use crate::theme;
use crate::toasts::{Kind, ToastAction};
use crate::widgets;

/// Seconds between refreshes while the page is open, and otherwise (for
/// invites and "the launcher is open").
const REFRESH_OPEN: f64 = 20.0;
const REFRESH_AWAY: f64 = 90.0;
const DOT: f32 = 5.0;

#[derive(Default)]
pub struct FriendsUi {
    view: Option<Outcome<FriendsView>>,
    /// Account the view is for.
    account: Option<String>,
    add_name: String,
    busy: bool,
    loading: bool,
    refreshed_at: Option<f64>,
    /// Invites already announced with a toast.
    seen_invites: HashSet<String>,
    settings_open: bool,
    name_edit: String,
    /// Where the player's own game went (for "Invite to my server").
    pub(crate) my_server: Option<String>,
    /// A recovery code just made (shown until the page is left).
    #[cfg(feature = "offline-accounts")]
    recovery_code: Option<String>,
    restore_code: String,
}

impl ArcticApp {
    /// Keep friends fresh, even on other tabs (invites, launcher presence).
    pub(crate) fn friends_tick(&mut self, now: f64, page_open: bool) {
        let Some(account) = self.accounts.active().cloned() else {
            return;
        };
        let f = &mut self.friends;
        if f.account.as_ref() != Some(&account.id) {
            f.account = Some(account.id.clone());
            f.view = None;
            f.refreshed_at = None;
            f.seen_invites.clear();
        }
        let every = if page_open {
            REFRESH_OPEN
        } else {
            REFRESH_AWAY
        };
        let due = f.refreshed_at.is_none_or(|at| now - at >= every);
        if due && !f.loading {
            f.loading = true;
            f.refreshed_at = Some(now);
            self.tasks.friends_refresh(account);
        }
    }

    fn friend_act(&mut self, action: FriendAction) {
        if let Some(account) = self.accounts.active().cloned() {
            self.friends.busy = true;
            self.tasks.friends_action(account, action);
        }
    }

    pub(crate) fn friends_card(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let Some(account) = self.accounts.active().cloned() else {
            return;
        };
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let view = match &self.friends.view {
                None => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(RichText::new("Loading friends…").color(p.muted));
                    });
                    return;
                }
                Some(Err(e)) => {
                    let e = e.clone();
                    ui.label(RichText::new(format!("Friends couldn't be loaded: {e}")).color(p.error));
                    // An offline name used on another PC: the recovery
                    // code made there brings the profile here.
                    if !account.is_microsoft() && e.contains("used by another") {
                        self.restore_form(ui);
                    }
                    return;
                }
                Some(Ok(v)) => v.clone(),
            };
            let (profile, overview) = view;
            self.friends_header(ui, &profile);
            if self.friends.settings_open {
                ui.add_space(8.0);
                self.profile_settings(ui, &profile);
            }
            ui.add_space(10.0);
            self.friend_requests(ui, &overview);
            if overview.friends.is_empty() {
                ui.label(
                    RichText::new(
                        "No friends yet. Add someone by their friend code (or a Microsoft account's name); \
                         they need to use Arctic too.",
                    )
                        .small()
                        .color(p.muted),
                );
            }
            for friend in &overview.friends {
                self.friend_row(ui, friend);
            }
        });
        self.chat_panel(ui);
        self.chat_viewer(&ui.ctx().clone());
    }

    fn friends_header(&mut self, ui: &mut egui::Ui, profile: &Profile) {
        let p = self.palette();
        crate::widgets::row(ui, |ui| {
            ui.label(RichText::new("Friends").size(18.0).strong().color(p.text));
            ui.label(RichText::new(format!("as {}", profile.name)).color(p.muted));
            if !profile.code.is_empty() {
                ui.label(
                    RichText::new(format!("code {}", profile.code))
                        .monospace()
                        .color(p.accent),
                )
                .on_hover_text("Your friend code: give it to people so they can add you");
                if widgets::icon_button(ui, p, Icon::Copy, "Copy your friend code").clicked() {
                    ui.ctx().copy_text(profile.code.clone());
                    self.toasts.push(Kind::Success, "Friend code copied", "");
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::icon_button(ui, p, Icon::Gear, "Profile & privacy").clicked() {
                    self.friends.settings_open = !self.friends.settings_open;
                    self.friends.name_edit = profile.name.clone();
                }
                if self.friends.busy || self.friends.loading {
                    ui.spinner();
                }
                let can_add = !self.friends.add_name.trim().is_empty() && !self.friends.busy;
                let add = ui
                    .add_enabled_ui(can_add, |ui| {
                        widgets::button(ui, p, Some(Icon::Plus), "Add", false)
                    })
                    .inner
                    .clicked();
                let field = ui.add(
                    widgets::text_field(&mut self.friends.add_name)
                        .hint_text("Friend code or Microsoft name")
                        .char_limit(16)
                        .desired_width(170.0),
                );
                let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if (add || (enter && can_add)) && !self.friends.busy {
                    let name = std::mem::take(&mut self.friends.add_name).trim().to_owned();
                    self.friend_act(FriendAction::Request(name));
                }
            });
        });
    }

    fn profile_settings(&mut self, ui: &mut egui::Ui, profile: &Profile) {
        let p = self.palette();
        egui::Frame::new()
            .fill(p.surface)
            .corner_radius(10)
            .inner_margin(12.0)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                crate::widgets::row(ui, |ui| {
                    ui.label(RichText::new("Shown to friends as").color(p.muted));
                    ui.add(
                        widgets::text_field(&mut self.friends.name_edit)
                            .char_limit(24)
                            .desired_width(180.0),
                    );
                    let changed = self.friends.name_edit.trim() != profile.name
                        && !self.friends.name_edit.trim().is_empty();
                    if changed && widgets::button(ui, p, None, "Save", true).clicked() {
                        let name = self.friends.name_edit.trim().to_owned();
                        self.friend_act(FriendAction::Update(Some(name), profile.settings.clone()));
                    }
                });
                ui.add_space(6.0);
                let mut s: Settings = profile.settings.clone();
                let mut changed = false;
                changed |= ui
                    .checkbox(&mut s.share_online, "Friends can see when I'm online")
                    .changed();
                changed |= ui
                    .add_enabled(
                        s.share_online,
                        egui::Checkbox::new(
                            &mut s.share_server,
                            "…and which server I'm playing on",
                        ),
                    )
                    .changed();
                changed |= ui
                    .checkbox(&mut s.allow_invites, "Friends can invite me")
                    .changed();
                changed |= ui
                    .checkbox(&mut s.show_accounts, "Show friends all my linked accounts")
                    .on_hover_text("Off: friends see only your profile name, so alts stay private.")
                    .changed();
                if changed {
                    self.friend_act(FriendAction::Update(None, s));
                }
                ui.add_space(8.0);
                ui.label(
                    RichText::new("ACCOUNTS ON THIS PROFILE")
                        .small()
                        .color(p.muted),
                );
                let linked: HashSet<&str> =
                    profile.accounts.iter().map(|a| a.uuid.as_str()).collect();
                let mut unlink = None;
                for a in &profile.accounts {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&a.name).color(p.text));
                        if profile.accounts.len() > 1
                            && widgets::icon_button(
                                ui,
                                p,
                                Icon::Close,
                                "Unlink (it keeps no friends)",
                            )
                            .clicked()
                        {
                            unlink = Some(a.uuid.clone());
                        }
                    });
                }
                let others: Vec<arctic_core::auth::Account> = self
                    .accounts
                    .accounts
                    .iter()
                    .filter(|a| !linked.contains(a.uuid.as_str()))
                    .cloned()
                    .collect();
                let mut link = None;
                for a in others {
                    crate::widgets::row(ui, |ui| {
                        ui.label(RichText::new(&a.username).color(p.muted));
                        if widgets::button(ui, p, Some(Icon::Plus), "Link to this profile", false)
                            .on_hover_text("Its friends move here too")
                            .clicked()
                        {
                            link = Some(a.clone());
                        }
                    });
                }
                if let Some(uuid) = unlink {
                    self.friend_act(FriendAction::Unlink(uuid));
                }
                if let Some(a) = link {
                    self.friend_act(FriendAction::Link(a));
                }
                // Offline accounts live on this PC: a recovery code moves
                // them (and this profile) to another one.
                #[cfg(feature = "offline-accounts")]
                let offline = profile
                    .accounts
                    .iter()
                    .any(|a| a.uuid.as_bytes().get(12) != Some(&b'4'));
                #[cfg(feature = "offline-accounts")]
                if offline {
                    ui.add_space(8.0);
                    self.recovery_section(ui, profile);
                }
            });
    }

    #[cfg(feature = "offline-accounts")]
    fn recovery_section(&mut self, ui: &mut egui::Ui, profile: &Profile) {
        let p = self.palette();
        ui.label(RichText::new("RECOVERY CODE").small().color(p.muted));
        match self.friends.recovery_code.clone() {
            Some(code) => {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&code).monospace().size(16.0).color(p.accent));
                    if widgets::icon_button(ui, p, Icon::Copy, "Copy").clicked() {
                        ui.ctx().copy_text(code.clone());
                    }
                });
                ui.label(
                    RichText::new(
                        "Save it somewhere safe: it's shown only this once. On a new PC, add the same \
                         offline name and enter it to bring this profile and its friends back.",
                    )
                    .small()
                    .color(p.warn),
                );
            }
            None => {
                ui.label(
                    RichText::new(if profile.has_recovery {
                        "You made one before. A new one replaces it."
                    } else {
                        "Offline accounts belong to this PC. Make a code to move them to another one."
                    })
                    .small()
                    .color(p.muted),
                );
                let label = if profile.has_recovery {
                    "Make a new code"
                } else {
                    "Make a recovery code"
                };
                if widgets::button(ui, p, None, label, false).clicked() {
                    self.friend_act(FriendAction::NewRecovery);
                }
            }
        }
    }

    /// On a new PC: bring an offline profile over with its recovery code.
    fn restore_form(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        ui.add_space(6.0);
        ui.label(
            RichText::new("Used this name on another PC? Enter the recovery code you saved there.")
                .color(p.muted),
        );
        crate::widgets::row(ui, |ui| {
            ui.add(
                widgets::text_field(&mut self.friends.restore_code)
                    .hint_text("xxxx-xxxx-xxxx-xxxx")
                    .desired_width(200.0),
            );
            let ready = !self.friends.restore_code.trim().is_empty() && !self.friends.busy;
            if ui
                .add_enabled_ui(ready, |ui| widgets::button(ui, p, None, "Restore", true))
                .inner
                .clicked()
            {
                let code = std::mem::take(&mut self.friends.restore_code);
                self.friend_act(FriendAction::Recover(code));
            }
        });
    }

    /// A new account was added: offer to link it to the current profile.
    pub(crate) fn offer_profile_link(&mut self, added: &arctic_core::auth::Account) {
        let Some(Ok((profile, _))) = &self.friends.view else {
            return;
        };
        let Some(into) = self.friends.account.clone() else {
            return;
        };
        if into == added.id || profile.accounts.iter().any(|a| a.uuid == added.uuid) {
            return;
        }
        self.toasts.push_with_action(
            Kind::Info,
            format!("Link {} to your friends profile?", added.username),
            format!(
                "Friends follow you on {} too. Skip to keep it separate.",
                added.username
            ),
            Some(ToastAction::LinkAccount {
                into,
                other: added.id.clone(),
            }),
        );
    }

    pub(crate) fn link_accounts(&mut self, into: &str, other: &str) {
        let find = |id: &str| self.accounts.accounts.iter().find(|a| a.id == id).cloned();
        if let (Some(into), Some(other)) = (find(into), find(other)) {
            self.friends.busy = true;
            self.tasks.friends_action(into, FriendAction::Link(other));
        }
    }

    #[cfg(feature = "offline-accounts")]
    pub(crate) fn on_recovery_code(&mut self, result: Outcome<String>) {
        self.friends.busy = false;
        match result {
            Ok(code) => self.friends.recovery_code = Some(code),
            Err(e) => self
                .toasts
                .push(Kind::Error, "Couldn't make a recovery code", e),
        }
    }

    fn friend_requests(&mut self, ui: &mut egui::Ui, o: &Overview) {
        let p = self.palette();
        let mut act = None;
        for invite in &o.invites {
            crate::widgets::row(ui, |ui| {
                let what = match invite.kind.as_str() {
                    "together" => "their world".to_owned(),
                    _ => invite.target.clone(),
                };
                ui.label(
                    RichText::new(format!("{} invited you to {what}", invite.from.name))
                        .color(p.accent),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::icon_button(ui, p, Icon::Close, "Dismiss").clicked() {
                        act = Some(FriendAction::Dismiss(invite.id.clone()));
                    }
                    if widgets::button(ui, p, Some(Icon::Play), "Join", true).clicked() {
                        act = Some(FriendAction::Dismiss(invite.id.clone()));
                        match invite.kind.as_str() {
                            "together" => {
                                self.together.code_input = invite.target.clone();
                                self.start_joining();
                            }
                            _ => self.launch_into(QuickPlay::Server(invite.target.clone())),
                        }
                    }
                });
            });
        }
        for person in &o.incoming {
            crate::widgets::row(ui, |ui| {
                ui.label(
                    RichText::new(format!("{} wants to be friends", person.name)).color(p.text),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::icon_button(ui, p, Icon::Close, "Decline").clicked() {
                        act = Some(FriendAction::Remove(person.id.clone()));
                    }
                    if widgets::button(ui, p, Some(Icon::Check), "Accept", true).clicked() {
                        act = Some(FriendAction::Accept(person.id.clone()));
                    }
                });
            });
        }
        for person in &o.outgoing {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("Asked {}", person.name))
                        .small()
                        .color(p.muted),
                );
                if widgets::icon_button(ui, p, Icon::Close, "Cancel request").clicked() {
                    act = Some(FriendAction::Remove(person.id.clone()));
                }
            });
        }
        if !(o.invites.is_empty() && o.incoming.is_empty() && o.outgoing.is_empty()) {
            ui.separator();
        }
        if let Some(a) = act {
            self.friend_act(a);
        }
    }

    fn friend_row(&mut self, ui: &mut egui::Ui, f: &Friend) {
        let p = self.palette();
        let invite_to = self.invite_target();
        let mut act = None;
        crate::widgets::row(ui, |ui| {
            let color = if f.in_game {
                Color32::from_rgb(0x86, 0xEF, 0xAC)
            } else if f.online {
                p.accent
            } else {
                p.muted.gamma_multiply(0.5)
            };
            let (rect, _) =
                ui.allocate_exact_size(vec2(DOT * 2.0, DOT * 2.0), egui::Sense::hover());
            ui.painter().circle_filled(rect.center(), DOT, color);
            ui.label(RichText::new(&f.name).strong().color(p.text));
            if f.verified {
                let (rect, resp) = ui.allocate_exact_size(vec2(12.0, 12.0), egui::Sense::hover());
                crate::art::icons::draw(ui.painter(), Icon::Check, rect, p.accent);
                resp.on_hover_text("Has a Microsoft account");
            }
            let status = match (&f.server, f.in_game, f.online) {
                (Some(s), _, _) => format!("playing on {s}"),
                (None, true, _) => "in game".into(),
                (None, false, true) => "online".into(),
                _ => "offline".into(),
            };
            ui.label(RichText::new(status).small().color(p.muted));
            if !f.accounts.is_empty() {
                let names: Vec<&str> = f.accounts.iter().map(|a| a.name.as_str()).collect();
                ui.label(
                    RichText::new(format!("({})", names.join(", ")))
                        .small()
                        .color(p.muted),
                );
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::icon_button(ui, p, Icon::Trash, "Remove friend").clicked() {
                    act = Some(FriendAction::Remove(f.id.clone()));
                }
                let chat = if f.unread > 0 {
                    format!("Chat ({})", f.unread)
                } else {
                    "Chat".to_owned()
                };
                if widgets::button(ui, p, None, &chat, f.unread > 0).clicked() {
                    self.open_chat(f.id.clone(), f.name.clone());
                }
                if f.takes_invites
                    && let Some(to) = invite_to.clone()
                {
                    let tip = match &to {
                        InviteTo::Server(s) => format!("Invite to {s}"),
                        InviteTo::Together(_) => "Invite to your shared world".to_owned(),
                    };
                    if widgets::button(ui, p, None, "Invite", false)
                        .on_hover_text(tip)
                        .clicked()
                    {
                        act = Some(FriendAction::Invite(f.id.clone(), to));
                    }
                }
                if f.online
                    && f.takes_invites
                    && widgets::button(ui, p, None, "Duel", false)
                        .on_hover_text(format!(
                            "Start the game in a duel arena and invite {}",
                            f.name
                        ))
                        .clicked()
                {
                    self.launch_into(QuickPlay::Duel {
                        kit: "sword".into(),
                        friend: Some((f.id.clone(), f.name.clone())),
                    });
                }
                if let Some(server) = &f.server
                    && widgets::button(ui, p, Some(Icon::Play), "Join", true)
                        .on_hover_text(format!("Start and join {server}"))
                        .clicked()
                {
                    self.launch_into(QuickPlay::Server(server.clone()));
                }
            });
        });
        if let Some(a) = act {
            self.friend_act(a);
        }
    }

    /// Where friends can be invited: the world you're sharing, or the
    /// server your running game joined.
    fn invite_target(&self) -> Option<InviteTo> {
        if let Some(code) = self.together.hosting_code() {
            return Some(InviteTo::Together(code));
        }
        let playing = self.runs.any_game();
        self.friends
            .my_server
            .clone()
            .filter(|_| playing)
            .map(InviteTo::Server)
    }

    pub(crate) fn friends_overview(&self) -> Option<&Overview> {
        match &self.friends.view {
            Some(Ok((_, o))) => Some(o),
            _ => None,
        }
    }

    pub(crate) fn on_friends(&mut self, result: Outcome<FriendsView>) {
        self.friends.loading = false;
        if let Ok((profile, _)) = &result {
            // The next game start tells the Arctic Client whether to say
            // which server it's on.
            let share = profile.settings.share_online && profile.settings.share_server;
            if self.settings.share_server_with_friends != share {
                self.settings.share_server_with_friends = share;
                self.persist_settings();
            }
        }
        if let Ok((_, overview)) = &result {
            let fresh: Vec<_> = overview
                .invites
                .iter()
                .filter(|i| !self.friends.seen_invites.contains(&i.id))
                .cloned()
                .collect();
            for invite in fresh {
                self.friends.seen_invites.insert(invite.id.clone());
                let what = match invite.kind.as_str() {
                    "together" => "their world".to_owned(),
                    _ => invite.target.clone(),
                };
                self.toasts.push_with_action(
                    Kind::Info,
                    format!("{} invited you", invite.from.name),
                    format!("to {what}"),
                    Some(ToastAction::OpenFriends),
                );
            }
        }
        self.friends.view = Some(result);
    }

    pub(crate) fn on_friend_done(&mut self, result: Outcome<String>) {
        self.friends.busy = false;
        match result {
            Ok(msg) if msg.is_empty() => {}
            Ok(msg) => self.toasts.push(Kind::Success, msg, ""),
            Err(e) => self.toasts.push(Kind::Error, "Friends", e),
        }
    }
}
