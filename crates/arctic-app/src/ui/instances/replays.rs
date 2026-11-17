//! Replays of an instance: what the Arctic Client kept (press the replay
//! key in game). Watch one (the game starts straight into it), open the
//! folder, or move one to trash.

use std::time::Duration;

use arctic_core::instances::Instance;
use arctic_core::launch::QuickPlay;
use arctic_core::replays::{self, Replay};
use eframe::egui::{self, CornerRadius, RichText, Sense, vec2};

use crate::app::{ArcticApp, Tab};
use crate::art::icons::{self, Icon};
use crate::motion::format_bytes;
use crate::theme;
use crate::toasts::Kind;
use crate::widgets;
use crate::{t, tf};
/// Most gameplay videos listed at once (the folder holds the rest).
const MAX_VIDEOS: usize = 30;
/// Thumbnails are 16:9.
const THUMB: egui::Vec2 = vec2(96.0, 54.0);

impl ArcticApp {
    pub(super) fn replays_page(&mut self, ui: &mut egui::Ui, instance: &Instance) {
        let p = self.palette();
        let game_dir = instance.game_dir(&self.dirs);
        let list = match &self.inst.replays {
            Some((id, list)) if *id == instance.id => list.clone(),
            _ => {
                self.refresh_replays(instance);
                Vec::new()
            }
        };
        crate::widgets::row(ui, |ui| {
            ui.label(RichText::new(tf!("instances.replays_2", count = list.len())).color(p.muted));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::icon_button(ui, p, Icon::Folder, t!("instances.open_replays_folder"))
                    .clicked()
                {
                    let folder = game_dir.join(replays::FOLDER);
                    let _ = std::fs::create_dir_all(&folder);
                    let _ = open::that_detached(&folder);
                }
                if widgets::button(ui, p, None, t!("instances.refresh"), false).clicked() {
                    self.refresh_replays(instance);
                }
            });
        });
        ui.add_space(8.0);
        self.videos_card(ui, &game_dir);
        if list.is_empty() {
            theme::card(p).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.vertical_centered(|ui| {
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new(t!("instances.no_replays_yet"))
                            .size(17.0)
                            .strong()
                            .color(p.text),
                    );
                    ui.label(
                        RichText::new(t!("instances.f9_saves_the_last_30_seconds_f10_starts_or"))
                            .color(p.muted),
                    );
                    ui.add_space(10.0);
                });
            });
            return;
        }
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            for replay in &list {
                self.replay_row(ui, instance, replay);
                ui.add_space(4.0);
            }
        });
    }

    /// The gameplay videos (MP4) the game saved: play in the system's player, show, trash.
    fn videos_card(&mut self, ui: &mut egui::Ui, game_dir: &std::path::Path) {
        let p = self.palette();
        let videos = replays::list_videos(game_dir);
        if videos.is_empty() {
            return;
        }
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(tf!("instances.gameplay_videos", count = videos.len()))
                    .size(17.0)
                    .strong()
                    .color(p.text),
            );
            ui.add_space(4.0);
            let mut trashed = false;
            for video in videos.iter().take(MAX_VIDEOS) {
                crate::widgets::row(ui, |ui| {
                    ui.vertical(|ui| {
                        let name = video
                            .path
                            .file_stem()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        ui.add(egui::Label::new(RichText::new(name).color(p.text)).truncate());
                        ui.label(
                            RichText::new(format!(
                                "{} · {}",
                                super::worlds::ago(Some(video.modified)),
                                format_bytes(video.size)
                            ))
                            .small()
                            .color(p.muted),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::icon_button(ui, p, Icon::Trash, t!("instances.move_to_trash"))
                            .clicked()
                        {
                            match replays::trash_video(video) {
                                Ok(()) => trashed = true,
                                Err(e) => self.toasts.push(
                                    Kind::Error,
                                    t!("instances.could_not_move_the_video"),
                                    e.to_string(),
                                ),
                            }
                        }
                        if widgets::icon_button(ui, p, Icon::Folder, t!("instances.show_in_folder"))
                            .clicked()
                            && let Some(dir) = video.path.parent()
                        {
                            let _ = open::that_detached(dir);
                        }
                        if widgets::button(ui, p, Some(Icon::Play), t!("instances.play"), true)
                            .clicked()
                        {
                            let _ = open::that_detached(&video.path);
                        }
                    });
                });
                ui.add_space(4.0);
            }
            if videos.len() > MAX_VIDEOS {
                ui.label(
                    RichText::new(tf!(
                        "instances.the_newest_are_shown_the_rest_are_in_the",
                        MAX_VIDEOS = MAX_VIDEOS
                    ))
                    .small()
                    .color(p.muted),
                );
            }
            if trashed {
                ui.ctx().request_repaint();
            }
        });
        ui.add_space(8.0);
    }

    fn replay_row(&mut self, ui: &mut egui::Ui, instance: &Instance, replay: &Replay) {
        let p = self.palette();
        crate::widgets::row(ui, |ui| {
            match &replay.thumbnail {
                Some(png) => {
                    let uri = format!("bytes://replay/{}.png", replay.path.display());
                    ui.add(
                        egui::Image::from_bytes(uri, png.clone())
                            .fit_to_exact_size(THUMB)
                            .corner_radius(CornerRadius::same(6)),
                    );
                }
                None => {
                    let (rect, _) = ui.allocate_exact_size(THUMB, Sense::hover());
                    ui.painter()
                        .rect_filled(rect, CornerRadius::same(6), p.surface_hover);
                    icons::draw(
                        ui.painter(),
                        Icon::Play,
                        rect.shrink2(vec2(36.0, 17.0)),
                        p.muted,
                    );
                }
            }
            ui.vertical(|ui| {
                ui.label(RichText::new(&replay.server).strong().color(p.text));
                let clips = match replay.clips {
                    0 => String::new(),
                    1 => " · 1 clip".into(),
                    n => format!(" · {n} clips"),
                };
                let detail = format!(
                    "{} · {}{} · {} · {}",
                    super::worlds::ago(replay.recorded),
                    length(replay.duration),
                    clips,
                    replay.mc_version,
                    format_bytes(replay.size)
                );
                ui.label(RichText::new(detail).small().color(p.muted));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::icon_button(ui, p, Icon::Trash, t!("instances.move_to_trash")).clicked()
                {
                    match replays::trash(replay) {
                        Ok(()) => self.toasts.push(
                            Kind::Info,
                            t!("instances.replay_moved_to_trash"),
                            "replay_recordings/.trash",
                        ),
                        Err(e) => self.toasts.push(
                            Kind::Error,
                            t!("instances.could_not_move_replay"),
                            e.to_string(),
                        ),
                    }
                    self.refresh_replays(instance);
                }
                if widgets::icon_button(ui, p, Icon::Folder, t!("instances.show_in_folder"))
                    .clicked()
                    && let Some(dir) = replay.path.parent()
                {
                    let _ = open::that_detached(dir);
                }
                if widgets::button(ui, p, Some(Icon::Play), t!("instances.watch"), true).clicked() {
                    self.settings.last_instance =
                        (!instance.is_default()).then(|| instance.id.clone());
                    let now = ui.input(|i| i.time);
                    self.set_tab(Tab::Play, now);
                    self.launch_into(QuickPlay::Replay(replay.path.clone()));
                }
            });
        });
    }

    pub(super) fn refresh_replays(&mut self, instance: &Instance) {
        let game_dir = instance.game_dir(&self.dirs);
        self.inst.replays = Some((instance.id.clone(), replays::list(&game_dir)));
    }
}

/// "4:05" or "1:02:03".
fn length(d: Duration) -> String {
    let s = d.as_secs();
    let (h, m, sec) = (s / 3600, (s / 60) % 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{sec:02}")
    } else {
        format!("{m}:{sec:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths_read_like_a_clock() {
        assert_eq!(length(Duration::from_secs(5)), "0:05");
        assert_eq!(length(Duration::from_secs(245)), "4:05");
        assert_eq!(length(Duration::from_secs(3723)), "1:02:03");
    }
}
