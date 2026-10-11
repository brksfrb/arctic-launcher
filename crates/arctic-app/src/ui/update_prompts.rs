//! Update prompts: the overlay for a required update (it installs and restarts by itself after a
//! short countdown), and "What's new" once after an update.

use std::time::Duration;

use arctic_core::update::ReleaseNotes;
use arctic_core::update::notes::NoteLine;
use eframe::egui::{self, Color32, FontFamily, FontId, Id, Modal, RichText, text::LayoutJob};

use super::dialogs::dialog_frame;
use crate::app::{ArcticApp, UpdateState};
use crate::motion::format_bytes;
use crate::theme::Palette;
use crate::widgets;

/// Seconds before a required update restarts the launcher by itself.
const RESTART_COUNTDOWN: f64 = 5.0;
/// How often the countdown redraws.
const COUNTDOWN_TICK: Duration = Duration::from_millis(250);
const OVERLAY_WIDTH: f32 = 440.0;
const NOTES_WIDTH: f32 = 500.0;
const NOTES_HEIGHT: f32 = 380.0;
const NOTE_SIZE: f32 = 14.0;

/// The required-update overlay's countdown.
pub struct RequiredPrompt {
    /// When the launcher restarts into the update by itself (`INFINITY` after a failed restart).
    deadline: f64,
    /// Restart as soon as the update is in place ("Update & Restart", or the countdown ran out).
    confirmed: bool,
}

/// "What's new" for the version that's running now.
pub struct WhatsNew {
    pub version: String,
    pub notes: ReleaseNotes,
}

impl ArcticApp {
    /// Covers the window while a required update is pending; Play can't be used anyway.
    pub(crate) fn required_update_overlay(&mut self, ctx: &egui::Context) {
        if !self.update_required() {
            self.required_prompt = None;
            return;
        }
        let now = ctx.input(|i| i.time);
        let prompt = self.required_prompt.get_or_insert(RequiredPrompt {
            deadline: now + RESTART_COUNTDOWN,
            confirmed: false,
        });
        prompt.confirmed |= now >= prompt.deadline;
        let (confirmed, deadline) = (prompt.confirmed, prompt.deadline);
        // Restarting would end the launcher under a running game: wait for it to close.
        let game_running = self.runs.any_active();
        if confirmed && let UpdateState::Available { info, .. } = &self.update {
            let info = info.clone();
            self.start_update(info);
        }
        if confirmed && !game_running && matches!(self.update, UpdateState::Installed { .. }) {
            // Only once: frames still run until the window closes, and after a failed restart
            // the player can press the button again.
            self.restart_after_update(ctx);
            self.required_prompt = Some(RequiredPrompt {
                deadline: f64::INFINITY,
                confirmed: false,
            });
            return;
        }
        if !confirmed && deadline.is_finite() {
            ctx.request_repaint_after(COUNTDOWN_TICK);
        }
        let seconds_left = (deadline - now).max(0.0).ceil() as u64;

        let p = self.palette();
        let mut update_clicked = false;
        let mut exit_clicked = false;
        Modal::new(Id::new("required_update"))
            .frame(dialog_frame(p))
            .show(ctx, |ui| {
                ui.set_width(OVERLAY_WIDTH);
                ui.label(
                    RichText::new("Update required")
                        .font(crate::fonts::heading(22.0))
                        .color(p.text),
                );
                ui.add_space(6.0);
                ui.label(RichText::new(self.required_about()).color(p.muted));
                ui.add_space(14.0);
                self.required_status(ui, p, confirmed, seconds_left, game_running);
                ui.add_space(16.0);
                let failed = matches!(self.update, UpdateState::Failed { .. });
                let waiting = game_running && matches!(self.update, UpdateState::Installed { .. });
                widgets::row(ui, |ui| {
                    let label = if failed { "Retry" } else { "Update & Restart" };
                    update_clicked = ui
                        .add_enabled_ui(!waiting, |ui| widgets::button(ui, p, None, label, true))
                        .inner
                        .clicked();
                    exit_clicked = widgets::button(ui, p, None, "Exit", false).clicked();
                });
            });
        if update_clicked {
            self.confirm_required_update();
        }
        if exit_clicked {
            crate::tray::set_quitting();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn required_about(&self) -> String {
        let version = match &self.update {
            UpdateState::Available { info, .. }
            | UpdateState::Installing { info, .. }
            | UpdateState::Failed {
                retry: Some(info), ..
            } => format!("Arctic Launcher {}", info.version),
            _ => "This Arctic Launcher update".to_owned(),
        };
        format!(
            "{version} is a mandatory update: games can't be started with this version until it's installed."
        )
    }

    fn required_status(
        &self,
        ui: &mut egui::Ui,
        p: &Palette,
        confirmed: bool,
        seconds_left: u64,
        game_running: bool,
    ) {
        let countdown = if confirmed {
            "Restarting as soon as it's ready…".to_owned()
        } else {
            format!("Restarting in {seconds_left} s…")
        };
        match &self.update {
            UpdateState::Available { .. } => {
                ui.label(RichText::new(format!("Updating in {seconds_left} s…")).color(p.accent));
            }
            UpdateState::Installing { done, total, .. } => {
                let fraction = if *total > 0 {
                    *done as f32 / *total as f32
                } else {
                    0.0
                };
                ui.label(format!(
                    "Downloading the update… {} / {}",
                    format_bytes(*done),
                    format_bytes(*total)
                ));
                ui.add(egui::ProgressBar::new(fraction).desired_width(OVERLAY_WIDTH));
                ui.label(RichText::new(countdown).small().color(p.muted));
            }
            UpdateState::Installed { .. } if game_running => {
                ui.label(
                    RichText::new("Installed. The launcher restarts when you close Minecraft.")
                        .color(p.warn),
                );
            }
            UpdateState::Installed { .. } => {
                ui.label(RichText::new(format!("Installed. {countdown}")).color(p.accent));
            }
            UpdateState::Failed { error, .. } => {
                ui.label(RichText::new(format!("The update failed: {error}")).color(p.error));
            }
            _ => {}
        }
    }

    fn confirm_required_update(&mut self) {
        let retry = match &self.update {
            UpdateState::Available { info, .. }
            | UpdateState::Failed {
                retry: Some(info), ..
            } => Some(info.clone()),
            _ => None,
        };
        if let Some(info) = retry {
            self.start_update(info);
        }
        if let Some(prompt) = &mut self.required_prompt {
            prompt.confirmed = true;
        }
    }

    /// Debug and developer builds: `ARCTIC_DEVSHOT_PAGE=whats-new` loads the real notes since
    /// 0.3.2; `required-update` shows the overlay mid-download (for screenshots).
    pub(crate) fn devshot_update_prompts(&mut self) {
        if !crate::devshot::enabled() {
            return;
        }
        match std::env::var("ARCTIC_DEVSHOT_PAGE").as_deref() {
            Ok("whats-new") => self
                .tasks
                .load_release_notes(arctic_core::update::UpdateChannel::Stable, "0.3.2".into()),
            Ok("required-update") => {
                let info = arctic_core::update::UpdateInfo {
                    version: "9.9.9".parse().expect("a version"),
                    required: true,
                    notes: String::new(),
                    page_url: String::new(),
                    asset_url: String::new(),
                    asset_size: 21 << 20,
                    sha256: String::new(),
                };
                self.update = UpdateState::Installing {
                    info,
                    done: 8 << 20,
                    total: 21 << 20,
                };
            }
            Ok("mod-clashes") => {
                // As if Play was pressed on the selected instance.
                let instance = self.selected_instance().clone();
                self.held_by_clashes(&instance, false, &None);
            }
            _ => {}
        }
    }

    /// Once after an update: what's new and what's fixed.
    pub(crate) fn whats_new_dialog(&mut self, ctx: &egui::Context) {
        // A required update comes first; this waits until it's done.
        if self.update_required() {
            return;
        }
        let Some(whats_new) = &self.whats_new else {
            return;
        };
        let p = self.palette();
        let mut close = false;
        let mut never = false;
        let modal = Modal::new(Id::new("whats_new"))
            .frame(dialog_frame(p))
            .show(ctx, |ui| {
                ui.set_width(NOTES_WIDTH);
                ui.label(
                    RichText::new(format!("Updated to Arctic Launcher {}", whats_new.version))
                        .font(crate::fonts::heading(22.0))
                        .color(p.text),
                );
                ui.add_space(10.0);
                egui::ScrollArea::vertical()
                    .max_height(NOTES_HEIGHT)
                    .auto_shrink([false, true])
                    .show(ui, |ui| notes_body(ui, p, &whats_new.notes));
                ui.add_space(12.0);
                ui.label(
                    RichText::new(
                        "You can turn this off in Settings → Updates → \"Show what's new after updating\".",
                    )
                    .small()
                    .color(p.muted),
                );
                ui.add_space(10.0);
                widgets::row(ui, |ui| {
                    close = widgets::button(ui, p, None, "Got it", true).clicked();
                    never = widgets::button(ui, p, None, "Don't show this again", false).clicked();
                    ui.hyperlink_to(
                        "Full release notes",
                        format!(
                            "https://github.com/{}/releases/tag/v{}",
                            arctic_core::update::GITHUB_REPO,
                            whats_new.version
                        ),
                    );
                });
            });
        if never {
            self.settings.show_release_notes = false;
            self.persist_settings();
        }
        if close || never || modal.should_close() {
            self.whats_new = None;
        }
    }
}

fn notes_body(ui: &mut egui::Ui, p: &Palette, notes: &ReleaseNotes) {
    if !notes.new.is_empty() {
        group_heading(ui, p, "What's new");
        for section in &notes.new {
            if let Some(title) = &section.title {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(title)
                        .font(crate::fonts::heading(15.0))
                        .color(p.text),
                );
            }
            note_lines(ui, p, &section.lines);
        }
    }
    if !notes.fixed.is_empty() {
        if !notes.new.is_empty() {
            ui.add_space(12.0);
        }
        group_heading(ui, p, "What's fixed");
        note_lines(ui, p, &notes.fixed);
    }
}

fn group_heading(ui: &mut egui::Ui, p: &Palette, text: &str) {
    ui.label(
        RichText::new(text)
            .font(crate::fonts::heading(17.0))
            .color(p.accent),
    );
    ui.add_space(2.0);
}

fn note_lines(ui: &mut egui::Ui, p: &Palette, lines: &[NoteLine]) {
    for line in lines {
        match line {
            NoteLine::Text(text) => {
                ui.add(egui::Label::new(inline(text, p)).wrap());
            }
            NoteLine::Bullet(text) => {
                ui.horizontal_top(|ui| {
                    ui.label(RichText::new("•").color(p.accent));
                    ui.add(egui::Label::new(inline(text, p)).wrap());
                });
            }
        }
        ui.add_space(3.0);
    }
}

/// The Markdown the notes use inside a line: `**bold**`, `` `code` `` and `[text](link)`
/// (shown as its text).
fn inline(text: &str, p: &Palette) -> LayoutJob {
    let mut job = LayoutJob::default();
    let regular = FontId::proportional(NOTE_SIZE);
    let bold = FontId::new(NOTE_SIZE, FontFamily::Name(crate::fonts::HEADING.into()));
    let code = FontId::monospace(NOTE_SIZE - 1.0);
    let mut rest = text;
    while !rest.is_empty() {
        if let Some((inner, after)) = rest.strip_prefix("**").and_then(|r| r.split_once("**")) {
            push(&mut job, inner, bold.clone(), p.text);
            rest = after;
        } else if let Some((inner, after)) = rest.strip_prefix('`').and_then(|r| r.split_once('`'))
        {
            push(&mut job, inner, code.clone(), p.text);
            rest = after;
        } else if let Some((label, after)) = link(rest) {
            push(&mut job, label, regular.clone(), p.accent);
            rest = after;
        } else {
            // Plain text up to the next marker (at least one character).
            let skip = rest.chars().next().map_or(1, char::len_utf8);
            let end = rest[skip..]
                .find(['*', '`', '['])
                .map_or(rest.len(), |i| i + skip);
            push(&mut job, &rest[..end], regular.clone(), p.muted);
            rest = &rest[end..];
        }
    }
    job
}

/// `[label](url)` at the start of `text`: (label, the text after it).
fn link(text: &str) -> Option<(&str, &str)> {
    let (label, after) = text.strip_prefix('[')?.split_once("](")?;
    let (_, after) = after.split_once(')')?;
    Some((label, after))
}

fn push(job: &mut LayoutJob, text: &str, font: FontId, color: Color32) {
    job.append(text, 0.0, egui::TextFormat::simple(font, color));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pieces(text: &str) -> Vec<String> {
        let p = crate::theme::palette(arctic_core::settings::ThemeMode::Default);
        let job = inline(text, p);
        job.sections
            .iter()
            .map(|s| job.text[s.byte_range.start.0..s.byte_range.end.0].to_owned())
            .collect()
    }

    #[test]
    fn inline_markdown() {
        assert_eq!(
            pieces("**Bold** then `code` and [a link](https://x) end"),
            ["Bold", " then ", "code", " and ", "a link", " end"]
        );
    }

    #[test]
    fn unmatched_markers_stay_text() {
        assert_eq!(pieces("2 * 3 [x"), ["2 * 3 [x"]);
    }
}
