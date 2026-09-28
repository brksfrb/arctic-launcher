//! Chat with friends in the launcher: one conversation open at a time
//! under the friend list, new messages polled every few seconds (and shown
//! as toasts when that chat isn't open).

use arctic_core::friends::Message;
use eframe::egui::{self, Align, Layout, RichText, vec2};

use crate::app::ArcticApp;
use crate::tasks::Outcome;
use crate::theme;
use crate::toasts::{Kind, ToastAction};
use crate::widgets;

/// Seconds between checks for new messages: on the Friends page, and elsewhere.
const POLL_OPEN: f64 = 4.0;
const POLL_AWAY: f64 = 20.0;
const HEIGHT: f32 = 260.0;
const BUBBLE_MAX: f32 = 360.0;
const IMAGE_W: f32 = 240.0;
const IMAGE_H: f32 = 135.0;
const PICKER_COUNT: usize = 12;
const PICKER_W: f32 = 128.0;
const PICKER_H: f32 = 72.0;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Picture {
    Loading,
    Ready(String),
    Failed,
}

/// A screenshot in a chat bubble (click it to see it large).
fn picture(
    ui: &mut egui::Ui,
    p: &crate::theme::Palette,
    state: Option<&Picture>,
    view: &mut Option<String>,
) {
    match state {
        Some(Picture::Ready(uri)) => {
            let r = ui.add(
                egui::Image::new(uri.clone())
                    .max_size(vec2(IMAGE_W, IMAGE_H))
                    .corner_radius(6)
                    .sense(egui::Sense::click()),
            );
            if r.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                *view = Some(uri.clone());
            }
        }
        Some(Picture::Failed) => {
            ui.label(
                RichText::new("(screenshot unavailable)")
                    .small()
                    .color(p.muted),
            );
        }
        _ => {
            ui.add_sized(vec2(IMAGE_W, IMAGE_H), egui::Spinner::new());
        }
    }
}

#[derive(Default)]
pub struct ChatUi {
    /// (friend profile id, name) of the open conversation.
    open: Option<(String, String)>,
    messages: Vec<Message>,
    loading: bool,
    draft: String,
    sending: bool,
    /// Newest message id seen; `None` until the first poll (which only
    /// sets it, so old messages don't pop up as new).
    last_id: Option<i64>,
    polling: bool,
    polled_at: Option<f64>,
    /// Account the chat state is for.
    account: Option<String>,
    /// Screenshots in the conversation, by attachment id.
    pictures: std::collections::HashMap<String, Picture>,
    /// The screenshot picker is open.
    picking: bool,
    /// A screenshot shown large (image URI).
    viewing: Option<String>,
}

impl ArcticApp {
    /// Keep polling for messages while the launcher runs.
    pub(crate) fn chat_tick(&mut self, now: f64, page_open: bool) {
        let Some(account) = self.accounts.active().cloned() else {
            return;
        };
        let c = &mut self.chat;
        if c.account.as_ref() != Some(&account.id) {
            *c = ChatUi {
                account: Some(account.id.clone()),
                ..ChatUi::default()
            };
        }
        let every = if page_open { POLL_OPEN } else { POLL_AWAY };
        if c.polling || c.polled_at.is_some_and(|at| now - at < every) {
            return;
        }
        c.polling = true;
        c.polled_at = Some(now);
        let after = c.last_id.unwrap_or(-1);
        let reading = c
            .open
            .as_ref()
            .map(|(id, _)| id.clone())
            .filter(|_| page_open);
        self.tasks.chat_poll(account, after, reading);
    }

    pub(crate) fn open_chat(&mut self, friend: String, name: String) {
        let Some(account) = self.accounts.active().cloned() else {
            return;
        };
        self.chat.open = Some((friend.clone(), name));
        self.chat.messages.clear();
        self.chat.loading = true;
        self.tasks.chat_open(account, friend);
    }

    /// The open conversation, if any.
    pub(crate) fn chat_panel(&mut self, ui: &mut egui::Ui) {
        let Some((friend, name)) = self.chat.open.clone() else {
            return;
        };
        let p = self.palette();
        ui.add_space(12.0);
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("Chat with {name}"))
                        .size(16.0)
                        .strong()
                        .color(p.text),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::icon_button(ui, p, crate::art::icons::Icon::Close, "Close chat")
                        .clicked()
                    {
                        self.chat.open = None;
                    }
                });
            });
            ui.add_space(6.0);
            let mut want: Vec<String> = Vec::new();
            let mut view: Option<String> = None;
            egui::ScrollArea::vertical()
                .max_height(HEIGHT)
                .min_scrolled_height(HEIGHT)
                .stick_to_bottom(true)
                .auto_shrink([false, false])
                .id_salt("chat_scroll")
                .show(ui, |ui| {
                    if self.chat.loading {
                        ui.spinner();
                    } else if self.chat.messages.is_empty() {
                        ui.label(RichText::new("No messages yet. Say hi!").color(p.muted));
                    }
                    for m in &self.chat.messages {
                        let mine = m.from != friend;
                        let layout = if mine {
                            Layout::right_to_left(Align::Min)
                        } else {
                            Layout::left_to_right(Align::Min)
                        };
                        ui.with_layout(layout, |ui| {
                            let fill = if mine {
                                p.accent.gamma_multiply(0.25)
                            } else {
                                p.surface_hover
                            };
                            egui::Frame::new()
                                .fill(fill)
                                .corner_radius(10)
                                .inner_margin(vec2(10.0, 6.0))
                                .show(ui, |ui| {
                                    ui.set_max_width(BUBBLE_MAX);
                                    if let Some(image) = &m.image {
                                        want.push(image.clone());
                                        picture(ui, p, self.chat.pictures.get(image), &mut view);
                                    }
                                    if !m.text.is_empty() {
                                        ui.add(
                                            egui::Label::new(RichText::new(&m.text).color(p.text))
                                                .wrap(),
                                        );
                                    }
                                });
                        });
                        ui.add_space(4.0);
                    }
                });
            self.load_pictures(want);
            if view.is_some() {
                self.chat.viewing = view;
            }
            ui.add_space(6.0);
            if self.chat.picking {
                self.screenshot_picker(ui, &friend);
            }
            ui.horizontal(|ui| {
                if widgets::icon_button(ui, p, crate::art::icons::Icon::Image, "Send a screenshot")
                    .clicked()
                {
                    self.chat.picking = !self.chat.picking;
                }
                let field = ui.add(
                    widgets::text_field(&mut self.chat.draft)
                        .hint_text(format!("Message {name}"))
                        .char_limit(500)
                        .desired_width(ui.available_width() - 90.0),
                );
                let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                let ready = !self.chat.draft.trim().is_empty() && !self.chat.sending;
                let send = ui
                    .add_enabled_ui(ready, |ui| widgets::button(ui, p, None, "Send", true))
                    .inner
                    .clicked();
                if (send || enter) && ready {
                    if let Some(account) = self.accounts.active().cloned() {
                        let text = std::mem::take(&mut self.chat.draft).trim().to_owned();
                        self.chat.sending = true;
                        self.tasks.chat_send(account, friend.clone(), text);
                    }
                    field.request_focus();
                }
            });
        });
    }

    /// Fetch screenshots shown in the chat that aren't loaded yet.
    fn load_pictures(&mut self, ids: Vec<String>) {
        let Some(account) = self.accounts.active().cloned() else {
            return;
        };
        for id in ids {
            if !self.chat.pictures.contains_key(&id) {
                self.chat.pictures.insert(id.clone(), Picture::Loading);
                self.tasks.chat_image(account.clone(), id);
            }
        }
    }

    /// The newest screenshots; pick one to send it.
    fn screenshot_picker(&mut self, ui: &mut egui::Ui, friend: &str) {
        let p = self.palette();
        let Some(shots) = self.recent_shots(PICKER_COUNT) else {
            ui.spinner();
            return;
        };
        if shots.is_empty() {
            ui.label(
                RichText::new("No screenshots yet (F2 in game).")
                    .small()
                    .color(p.muted),
            );
            return;
        }
        let mut chosen = None;
        egui::ScrollArea::horizontal()
            .id_salt("chat_picker")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for shot in &shots {
                        let r = self.shot_thumb(ui, shot, vec2(PICKER_W, PICKER_H));
                        if r.on_hover_text("Send this").clicked() {
                            chosen = Some(shot.path.clone());
                        }
                    }
                });
            });
        ui.add_space(4.0);
        if let Some(path) = chosen
            && let Some(account) = self.accounts.active().cloned()
        {
            self.chat.picking = false;
            self.chat.sending = true;
            self.tasks.chat_send_image(account, friend.to_owned(), path);
        }
    }

    /// A chat screenshot shown large.
    pub(crate) fn chat_viewer(&mut self, ctx: &egui::Context) {
        let Some(uri) = self.chat.viewing.clone() else {
            return;
        };
        let p = self.palette();
        let modal = egui::Modal::new(egui::Id::new("chat_viewer"))
            .frame(super::instances::dialog_frame(p))
            .show(ctx, |ui| {
                let max = ctx.content_rect().size() * 0.8;
                ui.add(egui::Image::new(uri).max_size(max).corner_radius(8));
            });
        if modal.should_close() {
            self.chat.viewing = None;
        }
    }

    pub(crate) fn on_chat_image(
        &mut self,
        ctx: &egui::Context,
        id: String,
        result: Outcome<Vec<u8>>,
    ) {
        let picture = match result {
            Ok(png) => {
                let uri = format!("bytes://chat/{id}.png");
                ctx.include_bytes(uri.clone(), png);
                Picture::Ready(uri)
            }
            Err(_) => Picture::Failed,
        };
        self.chat.pictures.insert(id, picture);
    }

    pub(crate) fn on_chat_history(&mut self, friend: String, result: Outcome<Vec<Message>>) {
        if self.chat.open.as_ref().map(|(id, _)| id) != Some(&friend) {
            return;
        }
        self.chat.loading = false;
        match result {
            Ok(list) => {
                if let Some(last) = list.last() {
                    self.chat.last_id = Some(self.chat.last_id.map_or(last.id, |l| l.max(last.id)));
                }
                self.chat.messages = list;
            }
            Err(e) => self.toasts.push(Kind::Error, "Couldn't load the chat", e),
        }
    }

    pub(crate) fn on_chat_sent(&mut self, result: Outcome<Message>) {
        self.chat.sending = false;
        match result {
            Ok(m) => {
                if self.chat.open.as_ref().is_some_and(|(id, _)| *id == m.to) {
                    self.chat.messages.push(m);
                }
            }
            Err(e) => self.toasts.push(Kind::Error, "Message not sent", e),
        }
    }

    pub(crate) fn on_chat_new(&mut self, after: i64, result: Outcome<Vec<Message>>) {
        self.chat.polling = false;
        let Ok(list) = result else {
            return;
        };
        // The first poll only learns where "new" starts.
        if after < 0 {
            let newest = list.iter().map(|m| m.id).max().unwrap_or(0);
            self.chat.last_id = Some(self.chat.last_id.unwrap_or(0).max(newest));
            return;
        }
        for m in list {
            self.chat.last_id = Some(self.chat.last_id.map_or(m.id, |l| l.max(m.id)));
            let open_here = self.chat.open.as_ref().is_some_and(|(id, _)| *id == m.from);
            if open_here {
                if !self.chat.messages.iter().any(|x| x.id == m.id) {
                    self.chat.messages.push(m);
                }
            } else {
                let name = self
                    .friend_name(&m.from)
                    .unwrap_or_else(|| "A friend".into());
                self.toasts.push_with_action(
                    Kind::Info,
                    name,
                    m.text.clone(),
                    Some(ToastAction::OpenFriends),
                );
            }
        }
    }

    fn friend_name(&self, profile: &str) -> Option<String> {
        self.friends_overview().and_then(|o| {
            o.friends
                .iter()
                .find(|f| f.id == profile)
                .map(|f| f.name.clone())
        })
    }
}
