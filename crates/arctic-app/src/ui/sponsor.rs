//! Arctic's supporter, Flash Hosting: a small row in the sidebar, a card
//! where a server that stays up is the natural next step (playing together),
//! and a credit in About. Never in the way; Settings → "Show supporter
//! mentions" hides all of it.

use eframe::egui::{self, CursorIcon, RichText, Sense, vec2};

use crate::app::ArcticApp;
use crate::art::flash;
use crate::art::icons::Icon;
use crate::theme;
use crate::widgets;
use crate::{t, tf};
const NAME: &str = "Flash Hosting";
const URL: &str = "https://flashhosting.net";
const PITCH: &str = "nav.sponsor_pitch";

impl ArcticApp {
    /// Sidebar, above the account: the mark and the name, quiet.
    pub(crate) fn sponsor_sidebar(&self, ui: &mut egui::Ui) {
        if !self.settings.show_sponsor {
            return;
        }
        let p = self.palette();
        let (rect, response) =
            ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
        let hovered = response.hovered();
        if hovered {
            ui.painter()
                .rect_filled(rect, 8.0, p.text.gamma_multiply(0.06));
        }
        let mark = egui::Rect::from_center_size(
            egui::pos2(rect.left() + 20.0, rect.center().y),
            vec2(18.0, 18.0),
        );
        flash::mark(ui.painter(), mark);
        let text = if hovered { p.text } else { p.muted };
        ui.painter().text(
            egui::pos2(rect.left() + 36.0, rect.center().y - 6.5),
            egui::Align2::LEFT_CENTER,
            t!("nav.supported_by"),
            egui::FontId::proportional(9.5),
            p.muted,
        );
        ui.painter().text(
            egui::pos2(rect.left() + 36.0, rect.center().y + 5.5),
            egui::Align2::LEFT_CENTER,
            NAME,
            egui::FontId::proportional(12.5),
            text,
        );
        if response
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text(format!("{} {URL}", t!(PITCH)))
            .clicked()
        {
            ui.ctx().open_url(egui::OpenUrl::new_tab(URL));
        }
    }

    /// About: "Supported by" with the mark and a link.
    pub(crate) fn sponsor_credit(&self, ui: &mut egui::Ui) {
        if !self.settings.show_sponsor {
            return;
        }
        let p = self.palette();
        // One button-high row, so the label sits level with the button.
        let row = vec2(ui.available_width(), 34.0);
        ui.allocate_ui_with_layout(
            row,
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.label(RichText::new(t!("nav.supported_by")).color(p.muted));
                let (mark, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
                flash::mark(ui.painter(), mark);
                if widgets::button(ui, p, Some(Icon::External), NAME, false)
                    .on_hover_text(URL)
                    .clicked()
                {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(URL));
                }
            },
        );
    }

    /// Playing together: a card for when the world should stay up without you.
    pub(crate) fn sponsor_card(&self, ui: &mut egui::Ui) {
        if !self.settings.show_sponsor {
            return;
        }
        let p = self.palette();
        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            crate::widgets::row(ui, |ui| {
                let (mark, _) = ui.allocate_exact_size(vec2(44.0, 44.0), Sense::hover());
                flash::mark(ui.painter(), mark);
                ui.add_space(6.0);
                // The button first (from the right), so the text wraps in what's left.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::button(ui, p, Some(Icon::External), "flashhosting.net", false)
                        .clicked()
                    {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(URL));
                    }
                    ui.add_space(12.0);
                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(NAME).size(16.0).strong().color(p.text));
                            ui.label(
                                RichText::new(t!("nav.supports_arctic"))
                                    .small()
                                    .color(flash::AMBER),
                            );
                        });
                        ui.add(
                            egui::Label::new(
                                RichText::new(tf!(
                                    "nav.want_your_world_up_while_youre_offline",
                                    PITCH = t!(PITCH)
                                ))
                                .color(p.muted),
                            )
                            .wrap(),
                        );
                    });
                });
            });
        });
    }
}
