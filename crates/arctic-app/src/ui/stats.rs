//! The Stats page: where the play time went (instances, servers, singleplayer worlds, menus, and how
//! long the launcher was open). All of it stays on this PC.

use arctic_core::playtime::{self, Part, ago, format_duration};
use eframe::egui::{self, CornerRadius, RichText, Sense, vec2};

use crate::app::ArcticApp;
use crate::theme;
use crate::widgets;
use crate::{t, tf, tn};
/// Rows shown in the servers and worlds lists.
const TOP: usize = 10;

impl ArcticApp {
    pub(crate) fn stats_tab(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        widgets::page_header(
            ui,
            p,
            t!("tasks.stats"),
            t!("tasks.where_your_time_goes_it_stays_on_this_pc"),
        );
        let stats = self.playtime.clone();
        let now = arctic_core::auth::now_secs();

        if stats.total_secs() == 0 && stats.launcher_secs() == 0 {
            theme::card(p).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(
                    RichText::new(t!("tasks.nothing_yet"))
                        .size(17.0)
                        .strong()
                        .color(p.text),
                );
                ui.label(
                    RichText::new(t!("tasks.play_for_a_while_and_this_page_fills_up_time"))
                        .color(p.muted),
                );
            });
            return;
        }

        // Totals.
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(t!("tasks.total"))
                    .size(18.0)
                    .strong()
                    .color(p.text),
            );
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                let sessions = stats.sessions();
                let tiles = [
                    (t!("tasks.game_open"), format_duration(stats.total_secs())),
                    (
                        t!("tasks.in_a_world_or_server"),
                        format_duration(stats.in_world_secs()),
                    ),
                    (t!("tasks.in_menus"), format_duration(stats.menu_secs())),
                    (
                        t!("tasks.launcher_open"),
                        format_duration(stats.launcher_secs()),
                    ),
                    (t!("tasks.sessions"), sessions.to_string()),
                ];
                for (index, (label, value)) in tiles.into_iter().enumerate() {
                    if index > 0 {
                        // A faint line between the figures, so they don't read as one sentence.
                        let (rect, _) = ui.allocate_exact_size(vec2(1.0, 40.0), Sense::hover());
                        ui.painter().line_segment(
                            [rect.center_top(), rect.center_bottom()],
                            egui::Stroke::new(1.0, p.card_stroke),
                        );
                        ui.add_space(10.0);
                    }
                    ui.vertical(|ui| {
                        ui.set_min_width(170.0);
                        ui.label(RichText::new(label).small().color(p.muted));
                        ui.label(RichText::new(value).size(22.0).strong().color(p.text));
                    });
                }
            });
        });
        ui.add_space(12.0);

        // Instances.
        let instances = stats.instances();
        let longest = instances
            .iter()
            .map(|(_, e)| e.total_secs)
            .max()
            .unwrap_or(1)
            .max(1);
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(t!("tasks.instances"))
                    .size(18.0)
                    .strong()
                    .color(p.text),
            );
            ui.add_space(6.0);
            for (id, entry) in &instances {
                let name = self.stats_instance_name(id);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(name).color(p.text));
                    ui.label(
                        RichText::new(tn!(
                            "tasks.session_last",
                            entry.sessions,
                            format_duration = format_duration(entry.total_secs),
                            ago = ago(entry.last_played, now)
                        ))
                        .small()
                        .color(p.muted),
                    );
                });
                bar(ui, p, entry.total_secs as f32 / longest as f32);
                ui.add_space(6.0);
            }
        });

        // Servers and worlds; the names can be hidden (for a screenshot or a stream).
        let hide_id = egui::Id::new("stats_hide_names");
        let mut hide = ui.data(|d| d.get_temp::<bool>(hide_id)).unwrap_or(false);
        let servers = stats.servers();
        let worlds = stats.worlds();
        if !servers.is_empty() || !worlds.is_empty() {
            ui.add_space(12.0);
            if ui
                .checkbox(&mut hide, t!("tasks.hide_server_and_world_names"))
                .changed()
            {
                ui.data_mut(|d| d.insert_temp(hide_id, hide));
            }
            ui.add_space(6.0);
        }
        top_list(
            ui,
            p,
            t!("tasks.servers"),
            &servers,
            hide,
            now,
            t!("tasks.server"),
        );
        top_list(
            ui,
            p,
            t!("tasks.singleplayer_worlds"),
            &worlds,
            hide,
            now,
            t!("tasks.world"),
        );
        let _ = playtime::GAME_LOG;
    }

    /// An instance's name for the Stats page (it may be gone since).
    fn stats_instance_name(&self, id: &str) -> String {
        if self.instance.id == id {
            return self.instance.name.clone();
        }
        self.custom_instances
            .iter()
            .find(|i| i.id == id)
            .map_or_else(
                || t!("tasks.removed_instance").to_owned(),
                |i| i.name.clone(),
            )
    }
}

/// A card listing the most played of `list`.
fn top_list(
    ui: &mut egui::Ui,
    p: &crate::theme::Palette,
    title: &str,
    list: &[(String, Part)],
    hide: bool,
    now: u64,
    anonymous: &str,
) {
    if list.is_empty() {
        return;
    }
    ui.add_space(12.0);
    let longest = list.first().map_or(1, |(_, part)| part.secs).max(1);
    theme::card(p).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(title).size(18.0).strong().color(p.text));
        ui.add_space(6.0);
        for (index, (name, part)) in list.iter().take(TOP).enumerate() {
            let shown = if hide {
                format!("{anonymous} {}", index + 1)
            } else {
                name.clone()
            };
            ui.horizontal(|ui| {
                ui.label(RichText::new(shown).color(p.text));
                ui.label(
                    RichText::new(tf!(
                        "tasks.last",
                        format_duration = format_duration(part.secs),
                        ago = ago(part.last, now)
                    ))
                    .small()
                    .color(p.muted),
                );
            });
            bar(ui, p, part.secs as f32 / longest as f32);
            ui.add_space(6.0);
        }
        if list.len() > TOP {
            ui.label(
                RichText::new(tn!("social.and_more", list.len() - TOP))
                    .small()
                    .color(p.muted),
            );
        }
    });
}

/// A thin bar, `share` (0 to 1) of the width.
fn bar(ui: &mut egui::Ui, p: &crate::theme::Palette, share: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 6.0), Sense::hover());
    let radius = CornerRadius::same(3);
    ui.painter().rect_filled(rect, radius, p.surface_hover);
    let mut fill = rect;
    fill.set_width((rect.width() * share.clamp(0.0, 1.0)).max(4.0));
    ui.painter().rect_filled(fill, radius, p.accent);
}
