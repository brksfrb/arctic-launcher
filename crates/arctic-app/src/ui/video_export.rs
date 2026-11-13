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
use crate::{t, tf};
impl ArcticApp {
    pub(crate) fn video_export_section(&mut self, ui: &mut egui::Ui, p: &Palette) {
        ui.label(
            RichText::new(t!("tasks.replays_are_exported_to_video_by_ffmpeg"))
                .small()
                .color(p.muted),
        );
        ui.add_space(6.0);
        if crate::video_encoder::available() {
            ui.label(
                RichText::new(t!("tasks.video_export_built_into_this_arctic_build"))
                    .color(p.accent),
            );
            return;
        }
        let chosen = ffmpeg::chosen(&self.dirs);
        let found = ffmpeg::find(&self.dirs);
        let (running, progress, error) = self.bridge.ffmpeg_download();
        match &found {
            Some(path) => {
                ui.label(RichText::new(t!("tasks.ffmpeg_ready")).color(p.accent));
                ui.label(
                    RichText::new(path.display().to_string())
                        .small()
                        .color(p.muted),
                );
            }
            None if running => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new(t!("tasks.downloading_ffmpeg")).color(p.muted));
                });
                ui.add(egui::ProgressBar::new(progress as f32).desired_width(320.0));
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(250));
            }
            None => {
                ui.label(
                    RichText::new(t!("tasks.ffmpeg_not_found_videos_cant_be_exported_yet"))
                        .color(p.error),
                );
                if let Some(e) = &error {
                    ui.label(
                        RichText::new(tf!("tasks.the_last_download_failed", e = e))
                            .small()
                            .color(p.error),
                    );
                }
                if !ffmpeg::can_download() {
                    let hint = ffmpeg::install_hint();
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            RichText::new(t!("tasks.install_it_with_your_package_manager"))
                                .small()
                                .color(p.muted),
                        );
                        ui.label(RichText::new(hint).small().monospace().color(p.text));
                        if widgets::icon_button(ui, p, Icon::Copy, t!("tasks.copy_command"))
                            .clicked()
                        {
                            ui.ctx()
                                .copy_text(hint.split(" · ").next().unwrap_or(hint).to_owned());
                            self.toasts.push(Kind::Info, t!("tasks.command_copied"), "");
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
                && widgets::button(ui, p, Some(Icon::Import), t!("tasks.install_ffmpeg"), true)
                    .on_hover_text(t!("tasks.downloads_gyan_devs_ffmpeg_build_checked"))
                    .clicked()
            {
                self.bridge.start_ffmpeg_download();
            }
            if widgets::button(
                ui,
                p,
                Some(Icon::Folder),
                t!("tasks.choose_ffmpeg"),
                found.is_none(),
            )
            .on_hover_text(t!("tasks.point_arctic_at_an_ffmpeg_you_already_have"))
            .clicked()
            {
                self.tasks.ffmpeg_pick();
            }
            if chosen.is_some()
                && widgets::button(ui, p, None, t!("tasks.forget_my_choice"), false).clicked()
            {
                match ffmpeg::set_chosen(&self.dirs, None) {
                    Ok(()) => self.toasts.push(
                        Kind::Info,
                        t!("tasks.arctic_looks_for_ffmpeg_itself_again"),
                        "",
                    ),
                    Err(e) => {
                        self.toasts
                            .push(Kind::Error, t!("tasks.couldnt_forget_it"), e.to_string())
                    }
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
                    t!("tasks.ffmpeg_ready_2"),
                    tf!("tasks.videos_are_exported_with", path = path.display()),
                ),
                Err(e) => self.toasts.push(
                    Kind::Error,
                    t!("tasks.couldnt_remember_that_ffmpeg"),
                    e.to_string(),
                ),
            },
            Err(e) => self
                .toasts
                .push(Kind::Error, t!("tasks.that_isnt_ffmpeg"), e),
        }
    }
}
