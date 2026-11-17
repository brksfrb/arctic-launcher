//! "Quick tip" in the sidebar: one of Arctic's features at a time, a new one every few minutes,
//! opening where it lives when clicked. Settings → "Show quick tips" hides it.

use std::time::{Duration, Instant};

use eframe::egui::{self, Align2, CursorIcon, FontId, Sense, vec2};

use crate::app::{ArcticApp, Tab};
use crate::art::icons::{self, Icon};
use crate::{t, tf};
/// How long one tip stays.
const TIP_EVERY: Duration = Duration::from_secs(5 * 60);
const PAD: f32 = 8.0;
const LABEL_HEIGHT: f32 = 12.0;
const TEXT_SIZE: f32 = 11.5;
/// The short card (no heading, two lines at most) when the window is too short for the whole one.
const SHORT_ROWS: usize = 2;
const SHORT_PAD: f32 = 7.0;
/// Room the sidebar's menu keeps above the card; without it the card isn't shown.
const MENU_ROOM: f32 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    Tab(Tab),
    Settings(&'static str),
    Migrate,
    Share,
    Discover,
    Gallery,
    Cosmetic(&'static str),
    Modpacks,
    Instances,
    Shaders,
    Instance(crate::ui::InstancePage),
    InstanceSetting(&'static str),
}

/// A tip and where clicking it goes (`None`: it's in game, nothing to open).
const TIPS: &[(&str, Option<Target>)] = &[
    ("nav.tip.drag_servers", Some(Target::Tab(Tab::Play))),
    ("nav.tip.discover_servers", Some(Target::Discover)),
    ("nav.tip.right_shift_hud", None),
    ("nav.tip.controllers", None),
    ("nav.tip.tier_tags", None),
    ("nav.tip.cosmetics_free", Some(Target::Tab(Tab::Skins))),
    ("nav.tip.skin_gallery", Some(Target::Gallery)),
    ("nav.tip.face_items", Some(Target::Cosmetic("face"))),
    ("nav.tip.shoulder_pets", Some(Target::Cosmetic("shoulders"))),
    ("nav.tip.play_together", Some(Target::Tab(Tab::Together))),
    ("nav.tip.friends_join", Some(Target::Tab(Tab::Together))),
    (
        "nav.tip.replays",
        Some(Target::Instance(crate::ui::InstancePage::Replays)),
    ),
    ("nav.tip.screenshots", Some(Target::Tab(Tab::Screenshots))),
    (
        "nav.modrinth_modpacks_install_as_their_own",
        Some(Target::Modpacks),
    ),
    (
        "nav.mods_that_would_stop_the_game_get_fixed",
        Some(Target::Instances),
    ),
    (
        "nav.an_instance_can_keep_its_worlds_on_any_drive",
        Some(Target::InstanceSetting("instances.game_folder")),
    ),
    (
        "nav.shaders_find_and_enable_them_in_packs",
        Some(Target::Shaders),
    ),
    (
        "nav.short_on_space_move_arctic_in_settings",
        Some(Target::Settings("settings.storage")),
    ),
    (
        "nav.bring_instances_and_huds_from_other",
        Some(Target::Migrate),
    ),
    (
        "nav.share_your_setup_or_a_hud_as_a_code",
        Some(Target::Share),
    ),
    (
        "nav.try_the_glass_theme_in_settings_appearance",
        Some(Target::Settings("settings.appearance")),
    ),
    (
        "nav.one_switch_adds_sodium_and_friends_for_more",
        Some(Target::Settings("settings.arctic_client")),
    ),
    (
        "nav.voice_chat_works_in_shared_worlds_and_svc",
        Some(Target::Settings("settings.voice_chat")),
    ),
    (
        "nav.animated_capes_too_busy_freeze_them_in",
        Some(Target::Settings(
            "settings.freeze_animated_capes_and_cosmetics",
        )),
    ),
    (
        "nav.crashes_are_explained_and_fixed_where_arctic",
        Some(Target::Tab(Tab::Logs)),
    ),
    (
        "nav.every_games_log_is_kept_in_logs",
        Some(Target::Tab(Tab::Logs)),
    ),
    ("nav.profiles_keep_accounts_and_instances_apart", None),
    ("nav.auto_reconnect_brings_you_back_when_a_server", None),
    ("nav.waypoints_and_a_minimap_come_with_the_arctic", None),
    ("nav.the_arctic_command_does_it_all_too_arctic", None),
];

#[derive(Default)]
pub struct QuickTips {
    /// Index into `TIPS` and when it was picked.
    current: Option<(usize, Instant)>,
    pub(super) instance_section: Option<&'static str>,
}

impl QuickTips {
    /// The tip to show now: a different random one every `TIP_EVERY`.
    fn tip(&mut self, now: Instant) -> usize {
        match self.current {
            Some((i, since)) if now.duration_since(since) < TIP_EVERY => i,
            last => {
                let next = pick(last.map(|(i, _)| i));
                self.current = Some((next, now));
                next
            }
        }
    }

    fn skip(&mut self) {
        let next = pick(self.current.map(|(i, _)| i));
        self.current = Some((next, Instant::now()));
    }
}

/// A random tip other than `last`.
fn pick(last: Option<usize>) -> usize {
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos() as usize ^ d.as_secs() as usize);
    let n = TIPS.len();
    match last {
        Some(last) if n > 1 => (last + 1 + seed % (n - 1)) % n,
        _ => seed % n,
    }
}

impl ArcticApp {
    fn open_quick_tip(&mut self, target: Target, now: f64) {
        match target {
            Target::Tab(tab) => self.set_tab(tab, now),
            Target::Settings(section) => {
                self.set_tab(Tab::Settings, now);
                super::settings::open_section(section);
            }
            Target::Migrate => self.open_migrate(),
            Target::Share => self.open_share(None),
            Target::Discover => {
                self.set_tab(Tab::Play, now);
                self.open_discover();
            }
            Target::Gallery => {
                self.set_tab(Tab::Skins, now);
                self.skins.section = super::skins::Section::Skins;
                self.skins.view = super::skins::SkinsView::Gallery;
            }
            Target::Cosmetic(slot) => {
                self.set_tab(Tab::Skins, now);
                self.skins.section = super::skins::Section::Slot(slot);
            }
            Target::Instances => {
                self.set_tab(Tab::Instances, now);
                self.inst.modpacks_open = false;
                self.inst.open = None;
            }
            Target::Modpacks => {
                self.set_tab(Tab::Instances, now);
                self.inst.modpacks_open = true;
            }
            Target::Shaders => {
                self.open_quick_tip(Target::Instance(crate::ui::InstancePage::Packs), now);
                self.inst.packs.show_shaders();
            }
            Target::InstanceSetting(section) => {
                self.open_quick_tip(Target::Instance(crate::ui::InstancePage::Settings), now);
                self.quick_tips.instance_section = Some(section);
            }
            Target::Instance(page) => {
                self.set_tab(Tab::Instances, now);
                self.inst.modpacks_open = false;
                let id = self.selected_instance().id.clone();
                self.open_instance(&id);
                self.inst.page = page;
            }
        }
    }

    /// Sidebar, above the supporter row: a feature worth knowing.
    pub(crate) fn quick_tip(&mut self, ui: &mut egui::Ui) {
        if !self.settings.quick_tips {
            return;
        }
        let p = self.palette();
        let index = if crate::devshot::enabled() {
            std::env::var("ARCTIC_DEVSHOT_TIP")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .filter(|i| *i < TIPS.len())
        } else {
            None
        }
        .unwrap_or_else(|| self.quick_tips.tip(Instant::now()));
        let (text, target) = TIPS[index];
        let text = t!(text);
        let width = ui.available_width();
        let galley = ui.painter().layout(
            text.to_owned(),
            FontId::proportional(TEXT_SIZE),
            p.text,
            width - 2.0 * PAD,
        );
        let height = PAD + LABEL_HEIGHT + 2.0 + galley.size().y + PAD;
        let room = ui.available_height() - MENU_ROOM;
        // A short window: the short card (the whole tip on hover), or nothing; the menu comes first.
        if room < height {
            self.quick_tip_short(ui, room, text, target);
            return;
        }
        let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
        let hovered = response.hovered();
        let fill = p.text.gamma_multiply(if hovered { 0.08 } else { 0.04 });
        ui.painter().rect_filled(rect, 10.0, fill);
        let label_y = rect.top() + PAD + LABEL_HEIGHT / 2.0;
        icons::draw(
            ui.painter(),
            Icon::StarFilled,
            egui::Rect::from_center_size(
                egui::pos2(rect.left() + PAD + 6.0, label_y),
                vec2(12.0, 12.0),
            ),
            p.accent,
        );
        ui.painter().text(
            egui::pos2(rect.left() + PAD + 17.0, label_y),
            Align2::LEFT_CENTER,
            t!("nav.quick_tip"),
            FontId::proportional(10.5),
            p.accent,
        );
        // Another tip, right away.
        let next = egui::Rect::from_center_size(
            egui::pos2(rect.right() - PAD - 4.0, label_y),
            vec2(18.0, 18.0),
        );
        let next_response = ui
            .interact(next, ui.id().with("quick_tip_next"), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text(t!("nav.another_tip"));
        icons::draw(
            ui.painter(),
            Icon::ChevronDown,
            next.shrink(3.0),
            if next_response.hovered() {
                p.text
            } else {
                p.muted
            },
        );
        let text_color = if hovered { p.text } else { p.muted };
        ui.painter().galley(
            egui::pos2(rect.left() + PAD, rect.top() + PAD + LABEL_HEIGHT + 2.0),
            galley,
            text_color,
        );
        if next_response.clicked() {
            self.quick_tips.skip();
            return;
        }
        let response = match target {
            Some(_) => response.on_hover_cursor(CursorIcon::PointingHand),
            None => response,
        };
        if response.clicked()
            && let Some(target) = target
        {
            let now = ui.input(|i| i.time);
            self.open_quick_tip(target, now);
        }
    }
}

impl ArcticApp {
    /// The tip without its heading, on two lines at most (cut short; all of it on hover), if
    /// that fits in `room`.
    fn quick_tip_short(
        &mut self,
        ui: &mut egui::Ui,
        room: f32,
        text: &str,
        target: Option<Target>,
    ) {
        let p = self.palette();
        let width = ui.available_width();
        let text_left = SHORT_PAD + 17.0;
        let mut job = egui::text::LayoutJob::single_section(
            text.to_owned(),
            egui::TextFormat::simple(FontId::proportional(TEXT_SIZE), p.muted),
        );
        job.wrap = egui::text::TextWrapping {
            max_width: width - text_left - SHORT_PAD,
            max_rows: SHORT_ROWS,
            break_anywhere: false,
            overflow_character: Some('…'),
        };
        let galley = ui.painter().layout_job(job);
        let height = galley.size().y + 2.0 * SHORT_PAD;
        if room < height {
            return;
        }
        let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
        let hovered = response.hovered();
        ui.painter().rect_filled(
            rect,
            8.0,
            p.text.gamma_multiply(if hovered { 0.08 } else { 0.04 }),
        );
        let first_row = rect.top() + SHORT_PAD + TEXT_SIZE * 0.65;
        icons::draw(
            ui.painter(),
            Icon::StarFilled,
            egui::Rect::from_center_size(
                egui::pos2(rect.left() + SHORT_PAD + 6.0, first_row),
                vec2(12.0, 12.0),
            ),
            p.accent,
        );
        let color = if hovered { p.text } else { p.muted };
        ui.painter().galley(
            egui::pos2(rect.left() + text_left, rect.top() + SHORT_PAD),
            galley,
            color,
        );
        let response = response.on_hover_text(tf!("nav.quick_tip_2", text = text));
        let response = match target {
            Some(_) => response.on_hover_cursor(CursorIcon::PointingHand),
            None => response,
        };
        if response.clicked()
            && let Some(target) = target
        {
            let now = ui.input(|i| i.time);
            self.open_quick_tip(target, now);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tip_has_its_intended_destination() {
        let expected = [
            ("nav.tip.drag_servers", Some(Target::Tab(Tab::Play))),
            ("nav.tip.discover_servers", Some(Target::Discover)),
            ("nav.tip.right_shift_hud", None),
            ("nav.tip.controllers", None),
            ("nav.tip.tier_tags", None),
            ("nav.tip.cosmetics_free", Some(Target::Tab(Tab::Skins))),
            ("nav.tip.skin_gallery", Some(Target::Gallery)),
            ("nav.tip.face_items", Some(Target::Cosmetic("face"))),
            ("nav.tip.shoulder_pets", Some(Target::Cosmetic("shoulders"))),
            ("nav.tip.play_together", Some(Target::Tab(Tab::Together))),
            ("nav.tip.friends_join", Some(Target::Tab(Tab::Together))),
            (
                "nav.tip.replays",
                Some(Target::Instance(crate::ui::InstancePage::Replays)),
            ),
            ("nav.tip.screenshots", Some(Target::Tab(Tab::Screenshots))),
            (
                "nav.modrinth_modpacks_install_as_their_own",
                Some(Target::Modpacks),
            ),
            (
                "nav.mods_that_would_stop_the_game_get_fixed",
                Some(Target::Instances),
            ),
            (
                "nav.an_instance_can_keep_its_worlds_on_any_drive",
                Some(Target::InstanceSetting("instances.game_folder")),
            ),
            (
                "nav.shaders_find_and_enable_them_in_packs",
                Some(Target::Shaders),
            ),
            (
                "nav.short_on_space_move_arctic_in_settings",
                Some(Target::Settings("settings.storage")),
            ),
            (
                "nav.bring_instances_and_huds_from_other",
                Some(Target::Migrate),
            ),
            (
                "nav.share_your_setup_or_a_hud_as_a_code",
                Some(Target::Share),
            ),
            (
                "nav.try_the_glass_theme_in_settings_appearance",
                Some(Target::Settings("settings.appearance")),
            ),
            (
                "nav.one_switch_adds_sodium_and_friends_for_more",
                Some(Target::Settings("settings.arctic_client")),
            ),
            (
                "nav.voice_chat_works_in_shared_worlds_and_svc",
                Some(Target::Settings("settings.voice_chat")),
            ),
            (
                "nav.animated_capes_too_busy_freeze_them_in",
                Some(Target::Settings(
                    "settings.freeze_animated_capes_and_cosmetics",
                )),
            ),
            (
                "nav.crashes_are_explained_and_fixed_where_arctic",
                Some(Target::Tab(Tab::Logs)),
            ),
            (
                "nav.every_games_log_is_kept_in_logs",
                Some(Target::Tab(Tab::Logs)),
            ),
            ("nav.profiles_keep_accounts_and_instances_apart", None),
            ("nav.auto_reconnect_brings_you_back_when_a_server", None),
            ("nav.waypoints_and_a_minimap_come_with_the_arctic", None),
            ("nav.the_arctic_command_does_it_all_too_arctic", None),
        ];
        assert_eq!(TIPS, expected);
    }

    #[test]
    fn a_new_tip_is_never_the_same_as_the_last() {
        for last in 0..TIPS.len() {
            for _ in 0..20 {
                assert_ne!(pick(Some(last)), last);
            }
        }
    }

    #[test]
    fn a_tip_stays_for_five_minutes() {
        let mut tips = QuickTips::default();
        let start = Instant::now();
        let first = tips.tip(start);
        assert_eq!(tips.tip(start + Duration::from_secs(60)), first);
        assert_ne!(tips.tip(start + TIP_EVERY + Duration::from_secs(1)), first);
    }
}
