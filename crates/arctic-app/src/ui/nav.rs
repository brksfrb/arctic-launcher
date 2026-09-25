//! Sidebar: logo, animated navigation, and the account switcher.

use eframe::egui::{
    self, Align2, CornerRadius, CursorIcon, FontId, Popup, PopupCloseBehavior, RectAlign, Sense,
    pos2, vec2,
};

use crate::app::{AddAccount, ArcticApp, Tab};
use crate::art::avatar::paint_face;
use crate::art::icons::{self, Icon};
use crate::art::{self, lerp_color};

const ITEM_HEIGHT: f32 = 40.0;
const ITEM_GAP: f32 = 4.0;
/// Highlight slide duration (cubic ease-out, so it feels snappy).
const SLIDE_TIME: f32 = 0.14;

fn tab_info(tab: Tab) -> (Icon, &'static str) {
    match tab {
        Tab::Play => (Icon::Play, "Play"),
        Tab::Accounts => (Icon::User, "Accounts"),
        Tab::Instances => (Icon::Layers, "Instances"),
        Tab::Together => (Icon::Friends, "Play together"),
        Tab::Logs => (Icon::Document, "Logs"),
        Tab::Settings => (Icon::Gear, "Settings"),
        Tab::About => (Icon::Info, "About"),
    }
}

impl ArcticApp {
    pub(crate) fn nav(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        ui.add_space(4.0);
        art::logo(ui, p);
        ui.add_space(6.0);
        self.profile_chip(ui);
        ui.add_space(12.0);

        let now = ui.input(|i| i.time);
        let width = ui.available_width();
        let count = Tab::ALL.len() as f32;
        // One allocation for the whole list; rows sit at fixed offsets so
        // the sliding highlight always lines up with them.
        let (area, _) = ui.allocate_exact_size(
            vec2(width, count * ITEM_HEIGHT + (count - 1.0) * ITEM_GAP),
            Sense::hover(),
        );
        let row_rect = |index: f32| {
            egui::Rect::from_min_size(
                pos2(area.left(), area.top() + index * (ITEM_HEIGHT + ITEM_GAP)),
                vec2(width, ITEM_HEIGHT),
            )
        };
        let selected_index = Tab::ALL.iter().position(|t| *t == self.tab).unwrap_or(0) as f32;
        let anim = eased_slide(ui.ctx(), selected_index, now);
        let pill = row_rect(anim);
        ui.painter().rect_filled(
            pill,
            CornerRadius::same(10),
            p.accent_deep.gamma_multiply(0.28),
        );
        let bar = egui::Rect::from_min_size(
            pill.left_top() + vec2(0.0, 10.0),
            vec2(3.0, ITEM_HEIGHT - 20.0),
        );
        ui.painter()
            .rect_filled(bar, CornerRadius::same(2), p.accent);

        for (i, tab) in Tab::ALL.into_iter().enumerate() {
            let rect = row_rect(i as f32);
            let response = ui.interact(rect, egui::Id::new(("nav", i)), Sense::click());
            let hover = ui
                .ctx()
                .animate_bool(response.id.with("h"), response.hovered());
            let active = (1.0 - (anim - i as f32).abs()).clamp(0.0, 1.0);
            if tab != self.tab {
                ui.painter().rect_filled(
                    rect,
                    CornerRadius::same(10),
                    p.surface_hover.gamma_multiply(0.5 * hover),
                );
            }
            let color = lerp_color(lerp_color(p.muted, p.text, hover), p.accent, active);
            let (icon, label) = tab_info(tab);
            let icon_rect = egui::Rect::from_center_size(
                rect.left_center() + vec2(22.0, 0.0),
                vec2(18.0, 18.0),
            );
            icons::draw(ui.painter(), icon, icon_rect, color);
            ui.painter().text(
                rect.left_center() + vec2(44.0 + 2.0 * hover, 0.0),
                Align2::LEFT_CENTER,
                label,
                FontId::proportional(15.0),
                color,
            );
            if response.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                self.set_tab(tab, now);
            }
        }

        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            ui.add_space(4.0);
            self.account_switcher(ui);
        });
    }

    fn account_switcher(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let width = ui.available_width();
        let (rect, response) = ui.allocate_exact_size(vec2(width, 52.0), Sense::click());
        let hover = ui
            .ctx()
            .animate_bool(response.id.with("h"), response.hovered());
        ui.painter().rect_filled(
            rect,
            CornerRadius::same(12),
            lerp_color(p.surface, p.surface_hover, hover),
        );
        let face_rect =
            egui::Rect::from_center_size(rect.left_center() + vec2(26.0, 0.0), vec2(32.0, 32.0));
        match self.accounts.active() {
            Some(account) => {
                paint_face(ui.painter(), face_rect, &self.face(account), p.card_stroke);
                ui.painter().text(
                    rect.left_center() + vec2(52.0, -8.0),
                    Align2::LEFT_CENTER,
                    &account.username,
                    FontId::proportional(14.5),
                    p.text,
                );
                ui.painter().text(
                    rect.left_center() + vec2(52.0, 10.0),
                    Align2::LEFT_CENTER,
                    account.kind_label(),
                    FontId::proportional(11.5),
                    p.muted,
                );
            }
            None => {
                icons::draw(ui.painter(), Icon::User, face_rect.shrink(6.0), p.muted);
                ui.painter().text(
                    rect.left_center() + vec2(52.0, 0.0),
                    Align2::LEFT_CENTER,
                    "Add account",
                    FontId::proportional(14.5),
                    p.text,
                );
            }
        }
        let chevron =
            egui::Rect::from_center_size(rect.right_center() - vec2(18.0, 0.0), vec2(12.0, 12.0));
        icons::draw(ui.painter(), Icon::ChevronDown, chevron, p.muted);
        let response = response.on_hover_cursor(CursorIcon::PointingHand);

        if self.accounts.accounts.is_empty() {
            if response.clicked() {
                self.add_account = AddAccount::Choose;
            }
            return;
        }
        Popup::from_toggle_button_response(&response)
            .align(RectAlign::TOP_START)
            .gap(6.0)
            .width(width + 40.0)
            .close_behavior(PopupCloseBehavior::CloseOnClick)
            .show(|ui| self.switcher_menu(ui));
    }

    fn switcher_menu(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let mut select = None;
        for account in &self.accounts.accounts {
            let active = self.accounts.active.as_deref() == Some(account.id.as_str());
            let (rect, response) =
                ui.allocate_exact_size(vec2(ui.available_width(), 36.0), Sense::click());
            if response.hovered() {
                ui.painter()
                    .rect_filled(rect, CornerRadius::same(8), p.surface_hover);
            }
            let face_rect = egui::Rect::from_center_size(
                rect.left_center() + vec2(18.0, 0.0),
                vec2(22.0, 22.0),
            );
            paint_face(ui.painter(), face_rect, &self.face(account), p.card_stroke);
            ui.painter().text(
                rect.left_center() + vec2(38.0, 0.0),
                Align2::LEFT_CENTER,
                &account.username,
                FontId::proportional(14.0),
                p.text,
            );
            if active {
                let check = egui::Rect::from_center_size(
                    rect.right_center() - vec2(16.0, 0.0),
                    vec2(14.0, 14.0),
                );
                icons::draw(ui.painter(), Icon::Check, check, p.accent);
            }
            if response.clicked() {
                select = Some(account.id.clone());
            }
        }
        ui.separator();
        if ui.button("Add account…").clicked() {
            self.add_account = AddAccount::Choose;
        }
        if ui.button("Manage accounts").clicked() {
            let now = ui.input(|i| i.time);
            self.set_tab(Tab::Accounts, now);
        }
        if let Some(id) = select {
            self.accounts.set_active(&id);
            self.save_accounts();
        }
    }
}

/// Position of the nav highlight, eased toward `target` (row index).
/// Remembers (from, to, start) in egui memory between frames.
fn eased_slide(ctx: &egui::Context, target: f32, now: f64) -> f32 {
    let id = egui::Id::new("nav_pill_slide");
    let (from, to, start) = ctx
        .data(|d| d.get_temp::<(f32, f32, f64)>(id))
        .unwrap_or((target, target, now));
    let current = from + (to - from) * crate::motion::eased(now, start, SLIDE_TIME);
    let state = if to == target {
        (from, to, start)
    } else {
        (current, target, now)
    };
    ctx.data_mut(|d| d.insert_temp(id, state));
    let t = crate::motion::eased(now, state.2, SLIDE_TIME);
    if t < 1.0 {
        ctx.request_repaint();
    }
    state.0 + (state.1 - state.0) * t
}
