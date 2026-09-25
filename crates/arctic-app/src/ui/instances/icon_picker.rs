//! Instance icon picker: snowflake style + color, per instance.

use arctic_core::instances::{FlakeStyle, InstanceIcon};
use eframe::egui::{
    self, CornerRadius, Popup, PopupCloseBehavior, Response, RichText, Sense, Stroke, StrokeKind,
    vec2,
};

use crate::app::ArcticApp;
use crate::art::flakes::{self, SWATCHES};

use crate::art::lerp_color;
use crate::theme;
use crate::toasts::Kind;

const STYLE_TILE: f32 = 58.0;

impl ArcticApp {
    /// Popup anchored to an instance emblem for choosing style and color.
    pub(crate) fn icon_picker(&mut self, anchor: &Response, instance_id: &str) {
        let Some(before) = self.instance_by_id(instance_id).map(|i| i.icon) else {
            return;
        };
        let mut icon = before;
        let p = self.palette();
        Popup::from_toggle_button_response(anchor)
            .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
            .gap(8.0)
            .width(3.0 * (STYLE_TILE + 8.0) + 8.0)
            .show(|ui| icon_picker_body(ui, p, &mut icon));
        if icon != before {
            let dirs = self.dirs.clone();
            if let Some(instance) = self.instance_mut(instance_id) {
                instance.icon = icon;
                if let Err(e) = instance.save(&dirs) {
                    self.toasts
                        .push(Kind::Error, "Could not save instance", e.to_string());
                }
            }
        }
    }
}

fn icon_picker_body(ui: &mut egui::Ui, p: &theme::Palette, icon: &mut InstanceIcon) {
    {
        let color = flakes::icon_color(icon, p.accent);
        ui.label(RichText::new("Snowflake").strong().color(p.text));
        egui::Grid::new("flake_styles")
            .spacing(vec2(8.0, 8.0))
            .show(ui, |ui| {
                for (i, style) in FlakeStyle::ALL.into_iter().enumerate() {
                    if style_tile(ui, style, icon.style == style, color, p) {
                        icon.style = style;
                    }
                    if i % 3 == 2 {
                        ui.end_row();
                    }
                }
            });
        ui.add_space(6.0);
        ui.label(RichText::new("Color").strong().color(p.text));
        ui.horizontal_wrapped(|ui| {
            if swatch(ui, p.accent, icon.color.is_none(), "Theme accent") {
                icon.color = None;
            }
            for rgb in SWATCHES {
                let c = egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                if swatch(ui, c, icon.color == Some(rgb), "") {
                    icon.color = Some(rgb);
                }
            }
            let mut custom = icon.color.unwrap_or([0x7d, 0xd3, 0xfc]);
            if ui.color_edit_button_srgb(&mut custom).changed() {
                icon.color = Some(custom);
            }
        });
    }
}

fn style_tile(
    ui: &mut egui::Ui,
    style: FlakeStyle,
    selected: bool,
    color: egui::Color32,
    p: &theme::Palette,
) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(STYLE_TILE, STYLE_TILE), Sense::click());
    let hover = ui
        .ctx()
        .animate_bool(response.id.with("h"), response.hovered());
    let painter = ui.painter();
    let fill = lerp_color(p.bg, p.surface_hover, 0.3 + 0.7 * hover);
    painter.rect_filled(rect, CornerRadius::same(10), fill);
    let ring = if selected { color } else { p.card_stroke };
    painter.rect_stroke(
        rect,
        CornerRadius::same(10),
        Stroke::new(1.0 + selected as u8 as f32, ring),
        StrokeKind::Inside,
    );
    flakes::draw(painter, style, rect.center(), STYLE_TILE * 0.32, 0.0, color);
    response.on_hover_text(style.label()).clicked()
}

fn swatch(ui: &mut egui::Ui, color: egui::Color32, selected: bool, tip: &str) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
    ui.painter().circle_filled(rect.center(), 9.0, color);
    if selected {
        ui.painter()
            .circle_stroke(rect.center(), 11.0, Stroke::new(2.0, color));
    }
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    let response = if tip.is_empty() {
        response
    } else {
        response.on_hover_text(tip)
    };
    response.clicked()
}
