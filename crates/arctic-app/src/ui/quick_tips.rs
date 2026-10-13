//! "Quick tip" in the sidebar: one of Arctic's features at a time, a new one every few minutes,
//! opening where it lives when clicked. Settings → "Show quick tips" hides it.

use std::time::{Duration, Instant};

use eframe::egui::{self, Align2, CursorIcon, FontId, Sense, vec2};

use crate::app::{ArcticApp, Tab};
use crate::art::icons::{self, Icon};

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

/// A tip and where clicking it goes (`None`: it's in game, nothing to open).
const TIPS: &[(&str, Option<Tab>)] = &[
    ("Drag servers to put your favorites first.", Some(Tab::Play)),
    (
        "Discover servers finds new places to play.",
        Some(Tab::Play),
    ),
    ("Right Shift in game moves or hides any HUD widget.", None),
    ("Controllers work in game on every version.", None),
    (
        "Tier Tags show PvP tiers by names (Right Shift in game).",
        None,
    ),
    ("Capes, wings, pets and emotes are free.", Some(Tab::Skins)),
    (
        "Cosmetics → Skin gallery: free skins to try on.",
        Some(Tab::Skins),
    ),
    (
        "Glasses off your eyes? Nudge face items in Cosmetics.",
        Some(Tab::Skins),
    ),
    (
        "Shoulder pets can sit on either shoulder.",
        Some(Tab::Skins),
    ),
    (
        "Play together without a server: share an invite code.",
        Some(Tab::Together),
    ),
    (
        "Friends see where you play and join in one click.",
        Some(Tab::Together),
    ),
    (
        "F9 saves the last 30 seconds; F10 starts or stops a recording.",
        Some(Tab::Instances),
    ),
    (
        "All your screenshots, every instance, one place.",
        Some(Tab::Screenshots),
    ),
    (
        "Modrinth modpacks install as their own instance.",
        Some(Tab::Instances),
    ),
    (
        "Mods that would stop the game get fixed before it starts.",
        Some(Tab::Instances),
    ),
    (
        "An instance can keep its worlds on any drive.",
        Some(Tab::Instances),
    ),
    (
        "Shaders: turn on Iris in an instance's settings.",
        Some(Tab::Instances),
    ),
    (
        "Short on space? Move Arctic in Settings → Storage.",
        Some(Tab::Settings),
    ),
    (
        "Bring instances and HUDs from other launchers.",
        Some(Tab::Settings),
    ),
    ("Share your setup or a HUD as a code.", Some(Tab::Settings)),
    (
        "Try the Glass theme in Settings → Appearance.",
        Some(Tab::Settings),
    ),
    (
        "One switch adds Sodium and friends for more FPS.",
        Some(Tab::Settings),
    ),
    (
        "Voice chat works in shared worlds and SVC servers.",
        Some(Tab::Settings),
    ),
    (
        "Animated capes too busy? Freeze them in Settings.",
        Some(Tab::Settings),
    ),
    (
        "Crashes are explained, and fixed where Arctic can.",
        Some(Tab::Logs),
    ),
    ("Every game's log is kept in Logs.", Some(Tab::Logs)),
    (
        "Profiles keep accounts and instances apart (top left).",
        None,
    ),
    (
        "Auto Reconnect brings you back when a server drops you.",
        None,
    ),
    ("Waypoints and a minimap come with the Arctic Client.", None),
    ("The arctic command does it all too: arctic --help.", None),
];

#[derive(Default)]
pub struct QuickTips {
    /// Index into `TIPS` and when it was picked.
    current: Option<(usize, Instant)>,
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
    /// Sidebar, above the supporter row: a feature worth knowing.
    pub(crate) fn quick_tip(&mut self, ui: &mut egui::Ui) {
        if !self.settings.quick_tips {
            return;
        }
        let p = self.palette();
        let index = self.quick_tips.tip(Instant::now());
        let (text, target) = TIPS[index];
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
            "Quick tip",
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
            .on_hover_text("Another tip");
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
            && let Some(tab) = target
        {
            let now = ui.input(|i| i.time);
            self.set_tab(tab, now);
        }
    }
}

impl ArcticApp {
    /// The tip without its heading, on two lines at most (cut short; all of it on hover), if
    /// that fits in `room`.
    fn quick_tip_short(&mut self, ui: &mut egui::Ui, room: f32, text: &str, target: Option<Tab>) {
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
        let response = response.on_hover_text(format!("Quick tip: {text}"));
        let response = match target {
            Some(_) => response.on_hover_cursor(CursorIcon::PointingHand),
            None => response,
        };
        if response.clicked()
            && let Some(tab) = target
        {
            let now = ui.input(|i| i.time);
            self.set_tab(tab, now);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
