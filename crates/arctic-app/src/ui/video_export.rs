//! Settings → Video export: whether replay videos can be exported (the game needs FFmpeg),
//! with a visible status, the launcher's own download where it has one (Windows), and a way to
//! point at an FFmpeg that is already installed everywhere else.

use std::path::PathBuf;

use arctic_core::ffmpeg;
use eframe::egui::{self, RichText};

use crate::app::ArcticApp;
use crate::art::icons::Icon;
use crate::tasks::Outcome;
use crate::theme::Palette;
use crate::toasts::Kind;
use crate::widgets;

impl ArcticApp {
    pub(crate) fn video_export_section(&mut self, ui: &mut egui::Ui, p: &Palette) {
        ui.label(
            RichText::new(
                "Replays are exported to video by FFmpeg. Arctic uses the one you pick here, \
                 its own copy, or one found on your PATH.",
            )
            .small()
            .color(p.muted),
        );
        ui.add_space(6.0);
        if crate::video_encoder::available() {
            ui.label(RichText::new("Video export: built into this Arctic build.").color(p.accent));
            return;
        }
        let chosen = ffmpeg::chosen(&self.dirs);
        let found = ffmpeg::find(&self.dirs);
        let (running, progress, error) = self.bridge.ffmpeg_download();
        match &found {
            Some(path) => {
                ui.label(RichText::new("FFmpeg: ready").color(p.accent));
                ui.label(
                    RichText::new(path.display().to_string())
                        .small()
                        .color(p.muted),
                );
            }
            None if running => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new("Downloading FFmpeg…").color(p.muted));
                });
                ui.add(egui::ProgressBar::new(progress as f32).desired_width(320.0));
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(250));
            }
            None => {
                ui.label(
                    RichText::new("FFmpeg: not found. Videos can't be exported yet.")
                        .color(p.error),
                );
                if let Some(e) = &error {
                    ui.label(
                        RichText::new(format!("The last download failed: {e}"))
                            .small()
                            .color(p.error),
                    );
                }
                if !ffmpeg::can_download() {
                    let hint = ffmpeg::install_hint();
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            RichText::new("Install it with your package manager:")
                                .small()
                                .color(p.muted),
                        );
                        ui.label(RichText::new(hint).small().monospace().color(p.text));
                        if widgets::icon_button(ui, p, Icon::Copy, "Copy command").clicked() {
                            ui.ctx()
                                .copy_text(hint.split(" · ").next().unwrap_or(hint).to_owned());
                            self.toasts.push(Kind::Info, "Command copied", "");
                        }
                    });
                }
            }
        }
        ui.add_space(6.0);
        widgets::row(ui, |ui| {
            if found.is_none()
                && !running
                && ffmpeg::can_download()
                && widgets::button(ui, p, Some(Icon::Import), "Install FFmpeg", true)
                    .on_hover_text(
                        "Downloads gyan.dev's FFmpeg build, checked against its published SHA-256",
                    )
                    .clicked()
            {
                self.bridge.start_ffmpeg_download();
            }
            if widgets::button(ui, p, Some(Icon::Folder), "Choose ffmpeg…", found.is_none())
                .on_hover_text("Point Arctic at an FFmpeg you already have")
                .clicked()
            {
                self.tasks.ffmpeg_pick();
            }
            if chosen.is_some() && widgets::button(ui, p, None, "Forget my choice", false).clicked()
            {
                match ffmpeg::set_chosen(&self.dirs, None) {
                    Ok(()) => {
                        self.toasts
                            .push(Kind::Info, "Arctic looks for FFmpeg itself again", "")
                    }
                    Err(e) => self
                        .toasts
                        .push(Kind::Error, "Couldn't forget it", e.to_string()),
                }
            }
        });
    }

    /// The file picked for "Choose ffmpeg…", once checked to really be FFmpeg.
    pub(crate) fn on_ffmpeg_picked(&mut self, result: Outcome<PathBuf>) {
        match result {
            Ok(path) => match ffmpeg::set_chosen(&self.dirs, Some(&path)) {
                Ok(()) => self.toasts.push(
                    Kind::Success,
                    "FFmpeg ready",
                    format!("Videos are exported with {}.", path.display()),
                ),
                Err(e) => {
                    self.toasts
                        .push(Kind::Error, "Couldn't remember that FFmpeg", e.to_string())
                }
            },
            Err(e) => self.toasts.push(Kind::Error, "That isn't FFmpeg", e),
        }
    }
}
