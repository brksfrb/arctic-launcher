//! Accounts tab: a grid of account cards plus an "Add account" card.

use eframe::egui::{self, Align2, CornerRadius, FontId, Sense, Shape, Stroke, vec2};

use crate::app::{AddAccount, ArcticApp};
use crate::art::avatar::paint_face;
use crate::art::icons::{self, Icon};
use crate::toasts::Kind;
use crate::widgets;

const CARD: [f32; 2] = [236.0, 96.0];

impl ArcticApp {
    pub(crate) fn accounts_tab(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        widgets::page_header(
            ui,
            p,
            "Accounts",
            "Click an account to play as it. Tokens stay on this PC only.",
        );
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(14.0, 14.0);
            let ids: Vec<String> = self
                .accounts
                .accounts
                .iter()
                .map(|a| a.id.clone())
                .collect();
            for id in ids {
                self.account_card(ui, &id);
            }
            self.add_card(ui);
        });
    }

    fn account_card(&mut self, ui: &mut egui::Ui, id: &str) {
        let p = self.palette();
        let Some(account) = self.accounts.accounts.iter().find(|a| a.id == id).cloned() else {
            return;
        };
        let active = self.accounts.active.as_deref() == Some(id);
        let (rect, response, hover) = widgets::hover_card(ui, p, CARD.into(), active);
        let painter = ui.painter();
        let face = egui::Rect::from_min_size(rect.left_top() + vec2(18.0, 20.0), vec2(56.0, 56.0));
        let frame = if active { p.accent } else { p.card_stroke };
        paint_face(painter, face, &self.face(&account), frame);
        painter.text(
            rect.left_top() + vec2(90.0, 30.0),
            Align2::LEFT_CENTER,
            &account.username,
            FontId::proportional(17.0),
            p.text,
        );
        let (badge, color) = if account.is_microsoft() {
            ("Microsoft", p.accent)
        } else {
            ("Offline", p.muted)
        };
        let badge_rect = egui::Rect::from_min_size(
            rect.left_top() + vec2(90.0, 48.0),
            vec2(if account.is_microsoft() { 74.0 } else { 56.0 }, 20.0),
        );
        painter.rect_filled(
            badge_rect,
            CornerRadius::same(255),
            color.gamma_multiply(0.16),
        );
        painter.text(
            badge_rect.center(),
            Align2::CENTER_CENTER,
            badge,
            FontId::proportional(11.5),
            color,
        );
        if active {
            let check = egui::Rect::from_center_size(
                rect.right_top() + vec2(-20.0, 20.0),
                vec2(20.0, 20.0),
            );
            painter.circle_filled(check.center(), 11.0, p.accent);
            icons::draw(painter, Icon::Check, check.shrink(5.0), p.on_accent);
        }

        // Remove button fades in on hover (bottom-right).
        let trash = egui::Rect::from_center_size(
            rect.right_bottom() + vec2(-22.0, -22.0),
            vec2(28.0, 28.0),
        );
        let trash_response = ui.interact(trash, response.id.with("remove"), Sense::click());
        if hover > 0.01 {
            let color = if trash_response.hovered() {
                p.error
            } else {
                p.muted
            };
            let painter = ui.painter();
            painter.rect_filled(
                trash,
                CornerRadius::same(8),
                p.surface_hover.gamma_multiply(hover),
            );
            icons::draw(
                painter,
                Icon::Trash,
                trash.shrink(7.0),
                color.gamma_multiply(hover),
            );
        }
        if trash_response.on_hover_text("Remove account").clicked() {
            self.remove_confirm = Some(account.id.clone());
        } else if response.clicked() && !active {
            self.accounts.set_active(&account.id);
            self.save_accounts();
            self.toasts
                .push(Kind::Info, format!("Playing as {}", account.username), "");
        }
    }

    fn add_card(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let (rect, response) = ui.allocate_exact_size(CARD.into(), Sense::click());
        let hover = ui
            .ctx()
            .animate_bool(response.id.with("h"), response.hovered());
        let color = crate::art::lerp_color(p.muted, p.accent, hover);
        let outline = rect.shrink(1.0);
        let corners = [
            outline.left_top(),
            outline.right_top(),
            outline.right_bottom(),
            outline.left_bottom(),
            outline.left_top(),
        ];
        ui.painter().rect_filled(
            rect,
            CornerRadius::same(14),
            p.card_fill.gamma_multiply(0.4 + 0.4 * hover),
        );
        ui.painter().extend(Shape::dashed_line(
            &corners,
            Stroke::new(1.2, color),
            6.0,
            5.0,
        ));
        let plus = egui::Rect::from_center_size(rect.center() - vec2(0.0, 12.0), vec2(22.0, 22.0));
        icons::draw(ui.painter(), Icon::Plus, plus, color);
        ui.painter().text(
            rect.center() + vec2(0.0, 18.0),
            Align2::CENTER_CENTER,
            "Add account",
            FontId::proportional(14.5),
            color,
        );
        if response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            self.add_account = AddAccount::Choose;
        }
    }
}
