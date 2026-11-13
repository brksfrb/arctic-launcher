//! "Worlds" on the Play tab: the instance's most recent singleplayer
//! worlds with their own icons; a click asks first, then starts the game straight into one.

use std::time::SystemTime;

use arctic_core::launch::QuickPlay;
use arctic_core::worlds::{self, World};
use eframe::egui::{self, CornerRadius, RichText, Sense, vec2};

use crate::app::ArcticApp;
use crate::art::icons::{self, Icon};
use crate::{t, tf};
/// Tiles shown (the rest are on the instance's Worlds page).
const SHOWN: usize = 4;
const TILE_W: f32 = 150.0;
const ICON: f32 = 48.0;
/// Re-read the saves folder at most this often (seconds).
const REFRESH: f64 = 20.0;

/// A world waiting for "Play" (from the Play tab or an instance's Worlds page).
#[derive(Clone)]
pub struct WorldConfirm {
    /// Folder name inside `saves`.
    pub folder: String,
    /// Name shown in game.
    pub name: String,
    /// Select this instance first (`Some(None)` = the default one); `None` = the selected one stays.
    pub select: Option<Option<String>>,
    /// Drawn at least once: Enter only counts from the next frame, so the Enter that activated a
    /// focused tile can't also answer the question.
    pub shown: bool,
}

#[derive(Default)]
pub struct RecentWorlds {
    /// The saves folder the list is for.
    key: String,
    read_at: f64,
    list: Vec<World>,
}

impl ArcticApp {
    pub(crate) fn worlds_section(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let now = ui.input(|i| i.time);
        let saves = self.selected_instance().game_dir(&self.dirs).join("saves");
        let key = saves.display().to_string();
        if self.recent_worlds.key != key || now - self.recent_worlds.read_at > REFRESH {
            self.recent_worlds.key = key;
            self.recent_worlds.read_at = now;
            self.recent_worlds.list = worlds::list(&saves).into_iter().take(SHOWN).collect();
        }
        if self.recent_worlds.list.is_empty() {
            return;
        }
        ui.add_space(18.0);
        ui.label(RichText::new(t!("play.worlds")).small().color(p.muted));
        ui.add_space(4.0);
        let mut open: Option<(String, String)> = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
            for world in &self.recent_worlds.list {
                let (rect, response) =
                    ui.allocate_exact_size(vec2(TILE_W, ICON + 16.0), Sense::click());
                let hover = ui.ctx().animate_bool(response.id, response.hovered());
                let fill = crate::art::lerp_color(p.surface, p.surface_hover, hover);
                ui.painter()
                    .rect_filled(rect, CornerRadius::same(8), fill.gamma_multiply(0.92));
                let icon_rect =
                    egui::Rect::from_min_size(rect.min + vec2(8.0, 8.0), vec2(ICON, ICON));
                match &world.icon {
                    Some(path) => {
                        egui::Image::new(crate::widgets::file_uri(path))
                            .corner_radius(CornerRadius::same(4))
                            .texture_options(egui::TextureOptions::NEAREST)
                            .paint_at(ui, icon_rect);
                    }
                    None => {
                        ui.painter()
                            .rect_filled(icon_rect, CornerRadius::same(4), p.surface_hover);
                        icons::draw(ui.painter(), Icon::Image, icon_rect.shrink(14.0), p.muted);
                    }
                }
                let text_x = icon_rect.right() + 8.0;
                let room = rect.right() - text_x - 6.0;
                // One line, cut with "…" when it's long.
                let mut job = egui::text::LayoutJob::simple_singleline(
                    world.name.clone(),
                    egui::FontId::proportional(13.5),
                    p.text,
                );
                job.wrap = egui::text::TextWrapping::truncate_at_width(room);
                let name = ui.painter().layout_job(job);
                let name_h = name.size().y;
                ui.painter()
                    .galley(egui::pos2(text_x, rect.top() + 14.0), name, p.text);
                ui.painter().text(
                    egui::pos2(text_x, rect.top() + 18.0 + name_h),
                    egui::Align2::LEFT_TOP,
                    ago(world.last_played),
                    egui::FontId::proportional(12.0),
                    p.muted,
                );
                if response
                    .on_hover_text(tf!("play.play_2", name = world.name))
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    open = Some((world.folder.clone(), world.name.clone()));
                }
            }
        });
        if let Some((folder, name)) = open {
            self.world_confirm = Some(WorldConfirm {
                folder,
                name,
                select: None,
                shown: false,
            });
        }
    }

    /// "Play <world>?" with Play and Cancel (Enter plays, Esc cancels). Only Play starts the game,
    /// the way a server needs its Join.
    pub(crate) fn world_confirm_dialog(&mut self, ctx: &egui::Context) {
        let Some(world) = self.world_confirm.clone() else {
            return;
        };
        let p = self.palette();
        let (mut yes, mut cancel) = (false, false);
        let modal = egui::Modal::new(egui::Id::new("world_confirm"))
            .frame(crate::ui::dialog_frame(p))
            .show(ctx, |ui| {
                ui.set_width(380.0);
                crate::widgets::lift_controls(ui, p);
                ui.label(
                    RichText::new(t!("play.do_you_want_to_launch_minecraft_to_play_this"))
                        .size(18.0)
                        .strong()
                        .color(p.text),
                );
                ui.add_space(4.0);
                ui.add(egui::Label::new(RichText::new(&world.name).color(p.muted)).truncate());
                ui.add_space(14.0);
                crate::widgets::row(ui, |ui| {
                    if crate::widgets::button(ui, p, None, t!("play.cancel"), false).clicked() {
                        cancel = true;
                    }
                    if crate::widgets::button(ui, p, None, t!("play.play"), true).clicked() {
                        yes = true;
                    }
                });
            });
        yes |= world.shown && ctx.input(|i| i.key_pressed(egui::Key::Enter));
        if let Some(open) = &mut self.world_confirm {
            open.shown = true;
        }
        if yes {
            self.world_confirm = None;
            if let Some(instance) = world.select {
                self.settings.last_instance = instance;
                let now = ctx.input(|i| i.time);
                self.set_tab(crate::app::Tab::Play, now);
            }
            self.launch_into(QuickPlay::World(world.folder));
        } else if cancel || modal.should_close() {
            self.world_confirm = None;
        }
    }
}

/// "Just now", "5 min ago", "3 h ago", "2 days ago".
fn ago(when: Option<SystemTime>) -> String {
    let Some(secs) = when.and_then(|t| t.elapsed().ok()).map(|d| d.as_secs()) else {
        return String::new();
    };
    match secs {
        0..60 => t!("play.just_now").into(),
        60..3600 => tf!("play.min_ago", value = secs / 60),
        3600..86_400 => tf!("common.h_ago", n = secs / 3600),
        86_400..172_800 => t!("play.yesterday").into(),
        _ => tf!("play.days_ago", _400 = secs / 86_400),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn times_read_naturally() {
        let at = |secs| Some(SystemTime::now() - Duration::from_secs(secs));
        assert_eq!(ago(at(5)), "Just now");
        assert_eq!(ago(at(600)), "10 min ago");
        assert_eq!(ago(at(7200)), "2 h ago");
        assert_eq!(ago(at(90_000)), "Yesterday");
        assert_eq!(ago(at(400_000)), "4 days ago");
        assert_eq!(ago(None), "");
    }
}
