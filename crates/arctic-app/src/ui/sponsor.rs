//! Arctic's supporter, mentioned where it fits (About, playing together)
//! and never in the way. Settings → "Show supporter mentions" hides it all.

use eframe::egui::{self, RichText};

use crate::app::ArcticApp;
use crate::art::icons::Icon;
use crate::theme::Palette;
use crate::widgets;

const NAME: &str = "Flash Hosting";
const URL: &str = "https://flashhosting.net";

impl ArcticApp {
    /// About page: a quiet "Supported by" line with a link.
    pub(crate) fn sponsor_credit(&self, ui: &mut egui::Ui) {
        if !self.settings.show_sponsor {
            return;
        }
        let p = self.palette();
        // One button-high row, so the label sits level with the button.
        let row = egui::vec2(ui.available_width(), 34.0);
        let centered = egui::Layout::left_to_right(egui::Align::Center);
        ui.allocate_ui_with_layout(row, centered, |ui| {
            ui.label(RichText::new("Supported by").color(p.muted));
            if widgets::button(ui, p, Some(Icon::External), NAME, false)
                .on_hover_text(URL)
                .clicked()
            {
                ui.ctx().open_url(egui::OpenUrl::new_tab(URL));
            }
        });
    }

    /// Playing together: where a server that stays up is the natural next step.
    pub(crate) fn sponsor_hint(&self, ui: &mut egui::Ui, text: &str) {
        if !self.settings.show_sponsor {
            return;
        }
        hint(ui, self.palette(), text);
    }
}

fn hint(ui: &mut egui::Ui, p: &Palette, text: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.label(RichText::new(text).small().color(p.muted));
        let link = ui
            .add(
                egui::Label::new(RichText::new(format!("{NAME} ↗")).small().color(p.accent))
                    .sense(egui::Sense::click()),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(URL);
        if link.clicked() {
            ui.ctx().open_url(egui::OpenUrl::new_tab(URL));
        }
    });
}
