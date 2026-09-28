//! "Worlds" on the Play tab: the instance's most recent singleplayer
//! worlds with their own icons; a click starts the game straight into one.

use std::time::SystemTime;

use arctic_core::launch::QuickPlay;
use arctic_core::worlds::{self, World};
use eframe::egui::{self, CornerRadius, RichText, Sense, vec2};

use crate::app::ArcticApp;
use crate::art::icons::{self, Icon};

/// Tiles shown (the rest are on the instance's Worlds page).
const SHOWN: usize = 4;
const TILE_W: f32 = 150.0;
const ICON: f32 = 48.0;
/// Re-read the saves folder at most this often (seconds).
const REFRESH: f64 = 20.0;

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
        ui.label(RichText::new("WORLDS").small().color(p.muted));
        ui.add_space(4.0);
        let mut open = None;
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
                    .on_hover_text(format!("Play {}", world.name))
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    open = Some(world.folder.clone());
                }
            }
        });
        if let Some(folder) = open {
            self.launch_into(QuickPlay::World(folder));
        }
    }
}

/// "Just now", "5 min ago", "3 h ago", "2 days ago".
fn ago(when: Option<SystemTime>) -> String {
    let Some(secs) = when.and_then(|t| t.elapsed().ok()).map(|d| d.as_secs()) else {
        return String::new();
    };
    match secs {
        0..60 => "Just now".into(),
        60..3600 => format!("{} min ago", secs / 60),
        3600..86_400 => format!("{} h ago", secs / 3600),
        86_400..172_800 => "Yesterday".into(),
        _ => format!("{} days ago", secs / 86_400),
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
