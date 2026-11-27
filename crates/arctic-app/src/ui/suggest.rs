//! "Suggest a feature or report a bug": a short form that goes to the Arctic team,
//! with an optional launcher / game log attached (cleaned of names and tokens first).

use std::path::Path;

use arctic_core::cosmetics::{Suggestion, SuggestionReceipt, SuggestionReplies};
use arctic_core::storage::{DataDirs, load_json};
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
    pub reports: bool,
}

/// Kept per report so sending a report and updating an existing one write different files.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct Report {
    key: uuid::Uuid,
    at: u64,
    bug: bool,
    text: String,
    server: String,
    receipt: SuggestionReceipt,
    cached: Option<SuggestionReplies>,
    unavailable: bool,
    seen: i64,
}

fn reports_dir(dirs: &DataDirs) -> std::path::PathBuf {
    dirs.profile_root().join("suggestion-reports")
}

impl Report {
    pub(crate) fn new(server: String, receipt: SuggestionReceipt, suggestion: Suggestion) -> Self {
        let unavailable = !receipt.can_receive_replies();
        Self {
            key: uuid::Uuid::new_v4(),
            at: arctic_core::auth::now_secs(),
            bug: suggestion.kind == "bug",
            text: suggestion.text,
            server,
            receipt,
            cached: None,
            unavailable,
            seen: 0,
        }
    }

    pub(crate) fn save(&self, dirs: &DataDirs) -> arctic_core::Result<()> {
        let dir = reports_dir(dirs);
        let path = dir.join(format!("{}.json", self.key));
        let io_error = |e| arctic_core::Error::io(&path, e);
        std::fs::create_dir_all(&dir).map_err(io_error)?;
        let mut file = tempfile::NamedTempFile::new_in(&dir).map_err(io_error)?;
        serde_json::to_writer(file.as_file_mut(), self)?;
        file.as_file().sync_all().map_err(io_error)?;
        file.persist(&path).map_err(|e| io_error(e.error))?;
        Ok(())
    }

    fn load(dirs: &DataDirs) -> Vec<Self> {
        let mut reports: Vec<Self> = std::fs::read_dir(reports_dir(dirs))
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
            .filter_map(|path| load_json(&path).ok().flatten())
            .collect();
        reports.sort_by_key(|report| std::cmp::Reverse((report.at, report.receipt.id)));
        reports
    }

    fn latest(&self) -> i64 {
        self.cached
            .as_ref()
            .and_then(|cached| cached.replies.last())
            .map_or(0, |reply| reply.id)
    }

    fn unread(&self) -> bool {
        self.latest() > self.seen
    }

    fn status(&self) -> &'static str {
        if self.unavailable {
            t!("nav.report_unavailable")
        } else if self.cached.as_ref().is_some_and(|cached| cached.done) {
            t!("nav.report_closed")
        } else if self.cached.is_some() {
            t!("nav.report_open")
        } else {
            t!("nav.report_sent")
        }
    }
}

#[derive(Default)]
pub(crate) struct Reports {
    entries: Vec<Report>,
    selected: Option<uuid::Uuid>,
    checking: Option<uuid::Uuid>,
    check_failed: bool,
}

impl Reports {
    pub(crate) fn unread(&self) -> usize {
        self.entries.iter().filter(|report| report.unread()).count()
    }

    fn show(&mut self, ui: &mut egui::Ui, p: &crate::theme::Palette, dirs: &DataDirs) {
        if self.checking.is_some() {
            ui.label(
                RichText::new(t!("nav.checking_replies"))
                    .small()
                    .color(p.muted),
            );
        } else if self.check_failed {
            ui.label(
                RichText::new(t!("nav.could_not_check_replies"))
                    .small()
                    .color(p.muted),
            );
        }
        if self.entries.is_empty() {
            ui.label(t!("nav.no_reports_sent"));
            return;
        }
        egui::ScrollArea::vertical()
            .id_salt("sent_reports")
            .max_height(340.0)
            .show(ui, |ui| {
                for report in &mut self.entries {
                    let kind = if report.bug {
                        t!("nav.bug")
                    } else {
                        t!("nav.idea")
                    };
                    let preview: String = report
                        .text
                        .trim()
                        .lines()
                        .next()
                        .unwrap_or_default()
                        .chars()
                        .take(48)
                        .collect();
                    let marker = if report.unread() { " ●" } else { "" };
                    let selected = self.selected == Some(report.key);
                    if ui
                        .selectable_label(
                            selected,
                            format!(
                                "{kind} #{} · {}{marker}",
                                report.receipt.id,
                                report.status()
                            ),
                        )
                        .clicked()
                    {
                        self.selected = if selected { None } else { Some(report.key) };
                    }
                    if self.selected != Some(report.key) {
                        ui.label(RichText::new(preview).small().color(p.muted));
                        ui.add_space(4.0);
                        continue;
                    }
                    ui.label(&report.text);
                    ui.add_space(4.0);
                    ui.label(RichText::new(t!("nav.report_replies")).strong());
                    if let Some(cached) = &report.cached
                        && !cached.replies.is_empty()
                    {
                        for reply in &cached.replies {
                            ui.label(&reply.text);
                        }
                    } else {
                        ui.label(RichText::new(t!("nav.no_replies_yet")).color(p.muted));
                    }
                    let latest = report.latest();
                    if report.seen < latest {
                        report.seen = latest;
                        if let Err(e) = report.save(dirs) {
                            log::warn!("could not save report read state: {e}");
                        }
                    }
                    ui.separator();
                }
            });
    }
}

impl ArcticApp {
    pub(crate) fn open_suggest(&mut self) {
        self.suggest.get_or_insert_with(SuggestForm::default);
    }

    pub(crate) fn suggest_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut form) = self.suggest.clone() else {
            return;
        };
        let p = self.palette();
        let mut send = false;
        let sending = self.suggestion_sending.contains(self.dirs.profile_root());
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
                let unread = self.reports.unread();
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut form.reports, false, t!("nav.new_report"));
                    let label = t!("nav.my_reports");
                    let label = if unread == 0 {
                        label.to_owned()
                    } else {
                        format!("{label} ({unread})")
                    };
                    if ui
                        .selectable_value(&mut form.reports, true, label)
                        .changed()
                    {
                        self.check_suggestion_replies();
                    }
                });
                ui.add_space(8.0);
                if form.reports {
                    self.reports.show(ui, p, &self.dirs);
                    return;
                }
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
                if (form.launcher_log || form.game_log) && form.contact.trim().is_empty() {
                    ui.label(
                        RichText::new(t!("nav.report_log_contact_hint"))
                            .small()
                            .color(p.muted),
                    );
                }
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
                    if !sending
                        && crate::widgets::button(ui, p, None, t!("nav.cancel"), false).clicked()
                    {
                        ui.close();
                    }
                    let ready = form.text.trim().chars().count() >= 3 && !sending;
                    let label = if sending {
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
        if modal.should_close() && !sending {
            self.suggest = None;
            return;
        }
        if send {
            self.suggestion_sending
                .insert(self.dirs.profile_root().to_owned());
            self.send_suggestion(&form);
        }
        self.suggest = Some(form);
    }

    fn send_suggestion(&mut self, form: &SuggestForm) {
        let launcher_log = form
            .launcher_log
            .then(|| self.dirs.launcher_logs().join("launcher.log"));
        let game_log = form.game_log.then(|| {
            let instance = self.selected_instance();
            self.runs
                .for_instance(&instance.id)
                .and_then(|run| run.log_file.clone())
                .unwrap_or_else(|| {
                    instance
                        .game_dir(&self.dirs)
                        .join("logs")
                        .join("latest.log")
                })
        });
        let suggestion = Suggestion {
            kind: if form.bug { "bug" } else { "idea" }.to_owned(),
            text: form.text.clone(),
            contact: form.contact.clone(),
            log: attached_logs(launcher_log.as_deref(), game_log.as_deref()),
        };
        self.tasks.send_suggestion(suggestion);
    }

    pub(crate) fn load_suggestion_reports(&mut self) {
        self.suggest = None;
        self.reports = Reports {
            entries: Report::load(&self.dirs),
            ..Default::default()
        };
        self.check_suggestion_replies();
    }

    fn check_suggestion_replies(&mut self) {
        if self.reports.checking.is_some() {
            return;
        }
        let reports: Vec<_> = self
            .reports
            .entries
            .iter()
            .filter(|report| report.receipt.can_receive_replies())
            .map(|report| (report.key, report.server.clone(), report.receipt.clone()))
            .collect();
        if reports.is_empty() {
            return;
        }
        let request = uuid::Uuid::new_v4();
        self.reports.checking = Some(request);
        self.reports.check_failed = false;
        self.tasks.check_suggestion_replies(request, reports);
    }

    pub(crate) fn on_suggestion_replies(
        &mut self,
        request: uuid::Uuid,
        updates: Vec<(uuid::Uuid, Option<SuggestionReplies>)>,
        failed: bool,
    ) {
        if self.reports.checking != Some(request) {
            return;
        }
        self.reports.checking = None;
        self.reports.check_failed = failed;
        for (key, update) in updates {
            if let Some(report) = self
                .reports
                .entries
                .iter_mut()
                .find(|report| report.key == key)
            {
                if report.unavailable == update.is_none()
                    && (update.is_none() || report.cached == update)
                {
                    continue;
                }
                report.unavailable = update.is_none();
                if let Some(replies) = update {
                    report.cached = Some(replies);
                }
                if let Err(e) = report.save(&self.dirs) {
                    log::warn!("could not save report replies: {e}");
                }
            }
        }
    }

    /// The server answered: close the form on success, keep it (with the text) on failure.
    pub(crate) fn on_suggestion_sent(
        &mut self,
        profile: &Path,
        result: Result<(Option<Report>, bool), String>,
    ) {
        self.suggestion_sending.remove(profile);
        if self.dirs.profile_root() != profile {
            return;
        }
        match result {
            Ok((report, saved)) => {
                if let Some(report) = report
                    && !self
                        .reports
                        .entries
                        .iter()
                        .any(|entry| entry.key == report.key)
                {
                    self.reports.entries.insert(0, report);
                }
                self.suggest = None;
                self.toasts.push(
                    Kind::Success,
                    t!("nav.sent_thank_you"),
                    t!("nav.we_read_every_one"),
                );
                if !saved {
                    self.toasts
                        .push(Kind::Error, t!("nav.could_not_save_report"), "");
                }
            }
            Err(e) => {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_receipt_cache_and_read_state_are_scoped_to_the_profile() {
        let dir = tempfile::tempdir().unwrap();
        let dirs = DataDirs::new(dir.path());
        let first = dirs.with_profile("first");
        let second = dirs.with_profile("second");
        let receipt = serde_json::from_str(r#"{"id":1,"reply_token":"a-test-token"}"#).unwrap();
        let mut report = Report::new(
            "https://example.test".into(),
            receipt,
            Suggestion {
                kind: "bug".into(),
                text: "Menus lag".into(),
                contact: "private@example.test".into(),
                log: "private log".into(),
            },
        );
        report.cached = Some(
            serde_json::from_str(r#"{"done":true,"replies":[{"id":3,"text":"Fixed"}]}"#).unwrap(),
        );
        report.save(&first).unwrap();

        assert!(Report::load(&second).is_empty());
        let mut reloaded = Report::load(&first).pop().unwrap();
        assert_eq!(reloaded.text, "Menus lag");
        assert!(reloaded.cached.as_ref().unwrap().done);
        assert!(reloaded.unread());
        reloaded.seen = reloaded.latest();
        reloaded.save(&first).unwrap();
        assert!(!Report::load(&first)[0].unread());
        let path = reports_dir(&first).join(format!("{}.json", report.key));
        let bytes = std::fs::read_to_string(path).unwrap();
        assert!(bytes.contains("a-test-token"));
        assert!(!bytes.contains("private"));
    }

    #[test]
    fn older_servers_can_submit_without_reply_tokens() {
        let receipt: SuggestionReceipt = serde_json::from_str(r#"{"id":1}"#).unwrap();
        assert!(!receipt.can_receive_replies());
    }
}
