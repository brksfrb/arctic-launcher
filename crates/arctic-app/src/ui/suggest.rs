//! "Suggest a feature or report a bug": a short form that goes to the Arctic team,
//! with an optional launcher / game log attached (cleaned of names and tokens first).

use arctic_core::cosmetics::Suggestion;
use eframe::egui::{self, Id, Modal, RichText};

use crate::app::ArcticApp;
use crate::art::icons::Icon;
use crate::toasts::Kind;
use crate::widgets;
use crate::{t, tf};
/// Most characters of the message (the server keeps this many).
const MAX_CHARS: usize = 2000;
const TEXT_HEIGHT: f32 = 150.0;
/// Most of each log's end that is sent (bytes).
const LOG_BYTES: usize = 12 * 1024;

#[derive(Debug, Clone, Default)]
pub struct SuggestForm {
    pub bug: bool,
    pub text: String,
    pub contact: String,
    pub launcher_log: bool,
    pub game_log: bool,
    pub sending: bool,
}

impl ArcticApp {
    pub(crate) fn open_suggest(&mut self) {
        self.suggest = Some(SuggestForm::default());
    }

    pub(crate) fn suggest_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut form) = self.suggest.clone() else {
            return;
        };
        let p = self.palette();
        let mut send = false;
        let modal = Modal::new(Id::new("suggest"))
            .frame(super::instances::dialog_frame(p))
            .show(ctx, |ui| {
                ui.set_width(460.0);
                ui.label(
                    RichText::new(t!("nav.suggest_a_feature_or_report_a_bug"))
                        .size(22.0)
                        .strong()
                        .color(p.text),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new(t!("nav.it_goes_straight_to_the_arctic_team_nothing"))
                        .color(p.muted),
                );
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut form.bug, false, t!("nav.idea"));
                    ui.selectable_value(&mut form.bug, true, t!("nav.bug"));
                });
                ui.add_space(6.0);
                egui::ScrollArea::vertical()
                    .max_height(TEXT_HEIGHT)
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut form.text)
                                .hint_text(if form.bug {
                                    t!("nav.what_went_wrong_and_what_were_you_doing")
                                } else {
                                    t!("nav.what_would_you_like_arctic_to_do")
                                })
                                .char_limit(MAX_CHARS)
                                .desired_rows(6)
                                .desired_width(f32::INFINITY),
                        );
                    });
                ui.add_space(8.0);
                ui.label(
                    RichText::new(t!("nav.how_to_reach_you_optional"))
                        .small()
                        .color(p.muted),
                );
                ui.add(
                    widgets::text_field(&mut form.contact)
                        .hint_text(t!("nav.discord_name_or_email_if_you_want_an_answer"))
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(8.0);
                ui.checkbox(&mut form.launcher_log, t!("nav.attach_the_launchers_log"));
                ui.checkbox(&mut form.game_log, t!("nav.attach_the_last_games_log"));
                ui.label(
                    RichText::new(t!("nav.logs_help_find_what_went_wrong_your_user"))
                        .small()
                        .color(p.muted),
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(t!("nav.need_an_answer_sooner_our_discord_is_quick"))
                            .small()
                            .color(p.muted),
                    );
                    ui.hyperlink_to(
                        RichText::new(t!("nav.join_the_discord")).small(),
                        crate::ui::about::DISCORD,
                    );
                });
                ui.add_space(12.0);
                crate::widgets::row(ui, |ui| {
                    if crate::widgets::button(ui, p, None, t!("nav.cancel"), false).clicked() {
                        ui.close();
                    }
                    let ready = form.text.trim().chars().count() >= 3 && !form.sending;
                    let label = if form.sending {
                        t!("nav.sending")
                    } else {
                        t!("nav.send")
                    };
                    let sent = ui
                        .add_enabled_ui(ready, |ui| {
                            widgets::button(ui, p, Some(Icon::External), label, true)
                        })
                        .inner;
                    send = ready && sent.clicked();
                });
            });
        if modal.should_close() {
            self.suggest = None;
            return;
        }
        if send {
            form.sending = true;
            self.send_suggestion(&form);
        }
        self.suggest = Some(form);
    }

    fn send_suggestion(&mut self, form: &SuggestForm) {
        let game_dir = {
            let id = self
                .settings
                .last_instance
                .clone()
                .unwrap_or_else(|| self.instance.id.clone());
            self.instance_by_id(&id)
                .map(|i| i.game_dir(&self.dirs))
                .or_else(|| Some(self.instance.game_dir(&self.dirs)))
        };
        let launcher_log = form
            .launcher_log
            .then(|| self.dirs.launcher_logs().join("launcher.log"));
        let game_log = form
            .game_log
            .then(|| game_dir.map(|d| d.join("logs").join("latest.log")))
            .flatten();
        let suggestion = Suggestion {
            kind: if form.bug { "bug" } else { "idea" }.to_owned(),
            text: form.text.clone(),
            contact: form.contact.clone(),
            log: attached_logs(launcher_log.as_deref(), game_log.as_deref()),
        };
        self.tasks.send_suggestion(suggestion);
    }

    /// The server answered: close the form on success, keep it (with the text) on failure.
    pub(crate) fn on_suggestion_sent(&mut self, result: Result<(), String>) {
        match result {
            Ok(()) => {
                self.suggest = None;
                self.toasts.push(
                    Kind::Success,
                    t!("nav.sent_thank_you"),
                    t!("nav.we_read_every_one"),
                );
            }
            Err(e) => {
                if let Some(form) = &mut self.suggest {
                    form.sending = false;
                }
                self.toasts.push(
                    Kind::Error,
                    t!("nav.couldnt_send_it"),
                    tf!("nav.your_text_is_still_here", e = e),
                );
            }
        }
    }
}

/// The ends of the chosen logs, with names and tokens taken out.
fn attached_logs(launcher: Option<&std::path::Path>, game: Option<&std::path::Path>) -> String {
    let mut out = String::new();
    for (title, path) in [
        (t!("nav.launcher_log"), launcher),
        (t!("nav.game_log"), game),
    ] {
        let Some(path) = path else {
            continue;
        };
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|_| format!("(couldn't read {})", path.display()));
        out.push_str(&format!("==== {title} ====\n"));
        out.push_str(&arctic_core::crash::scrubbed_tail(&text, LOG_BYTES));
        out.push_str("\n\n");
    }
    out
}
