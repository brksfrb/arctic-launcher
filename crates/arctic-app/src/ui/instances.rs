//! Instances tab and the instance icon picker (snowflake style + color).

use arctic_core::instances::FlakeStyle;
use eframe::egui::{
    self, CornerRadius, Popup, PopupCloseBehavior, Response, RichText, Sense, Stroke, StrokeKind,
    vec2,
};

use crate::app::ArcticApp;
use crate::art::flakes::{self, SWATCHES};
use crate::art::icons::{self, Icon};
use crate::art::lerp_color;
use crate::theme;
use crate::toasts::Kind;
use crate::widgets;

const STYLE_TILE: f32 = 58.0;

impl ArcticApp {
    /// Placeholder until mod loaders land. The data model and directory
    /// layout (`instances/<id>/`) already support multiple instances.
    pub(crate) fn instances_tab(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        widgets::page_header(
            ui,
            p,
            "Instances",
            "Separate game folders, each with its own version, mods and snowflake.",
        );

        theme::card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let emblem = widgets::instance_emblem(ui, p, &self.instance.icon, 56.0);
                self.icon_picker(&emblem);
                ui.add_space(4.0);
                ui.vertical(|ui| {
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(&self.instance.name)
                            .size(18.0)
                            .strong()
                            .color(p.text),
                    );
                    ui.label(
                        RichText::new(
                            "Default instance · plays any release · click the icon to customize",
                        )
                        .color(p.muted),
                    );
                });
            });
        });
        ui.add_space(28.0);

        ui.vertical_centered(|ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(64.0, 64.0), Sense::hover());
            icons::draw(ui.painter(), Icon::Layers, rect.shrink(8.0), p.muted);
            ui.add_space(6.0);
            ui.label(
                RichText::new("Modded instances are coming")
                    .size(18.0)
                    .strong()
                    .color(p.text),
            );
            ui.label(
                RichText::new(
                    "Fabric, Quilt and Forge instances will live here, each with its own snowflake style and color.",
                )
                .color(p.muted),
            );
            for instance in &self.custom_instances {
                ui.label(format!("{} · {:?}", instance.name, instance.loader));
            }
        });
    }

    /// Popup anchored to an instance emblem for choosing style and color.
    pub(crate) fn icon_picker(&mut self, anchor: &Response) {
        let before = self.instance.icon;
        Popup::from_toggle_button_response(anchor)
            .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
            .gap(8.0)
            .width(3.0 * (STYLE_TILE + 8.0) + 8.0)
            .show(|ui| self.icon_picker_body(ui));
        if self.instance.icon != before
            && let Err(e) = self.instance.save(&self.dirs)
        {
            self.toasts
                .push(Kind::Error, "Could not save instance", e.to_string());
        }
    }

    fn icon_picker_body(&mut self, ui: &mut egui::Ui) {
        let p = self.palette();
        let color = flakes::icon_color(&self.instance.icon, p.accent);
        ui.label(RichText::new("Snowflake").strong().color(p.text));
        egui::Grid::new("flake_styles")
            .spacing(vec2(8.0, 8.0))
            .show(ui, |ui| {
                for (i, style) in FlakeStyle::ALL.into_iter().enumerate() {
                    if style_tile(ui, style, self.instance.icon.style == style, color, p) {
                        self.instance.icon.style = style;
                    }
                    if i % 3 == 2 {
                        ui.end_row();
                    }
                }
            });
        ui.add_space(6.0);
        ui.label(RichText::new("Color").strong().color(p.text));
        ui.horizontal_wrapped(|ui| {
            if swatch(
                ui,
                p.accent,
                self.instance.icon.color.is_none(),
                "Theme accent",
            ) {
                self.instance.icon.color = None;
            }
            for rgb in SWATCHES {
                let c = egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                if swatch(ui, c, self.instance.icon.color == Some(rgb), "") {
                    self.instance.icon.color = Some(rgb);
                }
            }
            let mut custom = self.instance.icon.color.unwrap_or([0x7d, 0xd3, 0xfc]);
            if ui.color_edit_button_srgb(&mut custom).changed() {
                self.instance.icon.color = Some(custom);
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
