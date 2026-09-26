//! Picking the Arctic Client's in-game menu style (onboarding and Settings).

use arctic_core::settings::{ClientStyle, Settings};
use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, vec2,
};

use crate::art::icons::{self, Icon};
use crate::art::lerp_color;
use crate::theme::Palette;

/// Preview height over width, as in the launcher theme tiles (58 / 112).
const PREVIEW_RATIO: f32 = 58.0 / 112.0;
const LABEL_HEIGHT: f32 = 26.0;
/// Width of a tile in Settings, next to the theme tiles.
pub const SETTINGS_TILE_WIDTH: f32 = 112.0;

/// Colors of a style's miniature, matching the mod's palettes.
struct Preview {
    sky_top: Color32,
    sky_bottom: Color32,
    ground: Color32,
    primary: Color32,
    button: Color32,
}

fn preview(style: ClientStyle) -> Preview {
    match style {
        ClientStyle::Arctic => Preview {
            sky_top: Color32::from_rgb(0x07, 0x0E, 0x1C),
            sky_bottom: Color32::from_rgb(0x14, 0x28, 0x45),
            ground: Color32::from_rgb(0x0C, 0x18, 0x2B),
            primary: Color32::from_rgb(0x7D, 0xD3, 0xFC),
            button: Color32::from_rgb(0x1E, 0x33, 0x50),
        },
        ClientStyle::Aurora => Preview {
            sky_top: Color32::from_rgb(0x0B, 0x0A, 0x1C),
            sky_bottom: Color32::from_rgb(0x23, 0x1A, 0x45),
            ground: Color32::from_rgb(0x15, 0x11, 0x30),
            primary: Color32::from_rgb(0x86, 0xEF, 0xAC),
            button: Color32::from_rgb(0x2A, 0x23, 0x50),
        },
        ClientStyle::Classic => Preview {
            sky_top: Color32::from_rgb(0x2B, 0x2B, 0x2B),
            sky_bottom: Color32::from_rgb(0x45, 0x45, 0x45),
            ground: Color32::from_rgb(0x1C, 0x1C, 0x1C),
            primary: Color32::from_rgb(0x70, 0x70, 0x70),
            button: Color32::from_rgb(0x55, 0x55, 0x55),
        },
    }
}

/// A row of style tiles; picking one records when, so the game adopts it.
/// `tile_width` matches the tiles next to it (Settings or onboarding).
pub fn picker(ui: &mut egui::Ui, p: &Palette, settings: &mut Settings, tile_width: f32) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        for style in ClientStyle::ALL {
            if tile(ui, p, style, settings.client_style == style, tile_width) {
                settings.client_style = style;
                settings.client_style_set = arctic_core::auth::now_secs();
            }
        }
    });
}

/// A miniature menu in `style`, like the launcher theme tiles. Returns
/// true when clicked.
fn tile(ui: &mut egui::Ui, p: &Palette, style: ClientStyle, selected: bool, width: f32) -> bool {
    let preview_h = (width * PREVIEW_RATIO).round();
    let size = vec2(width, preview_h + LABEL_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let hover = ui
        .ctx()
        .animate_bool(response.id.with("h"), response.hovered());
    let sel = ui.ctx().animate_bool(response.id.with("s"), selected);
    let c = preview(style);
    let painter = ui.painter();
    let preview_rect = Rect::from_min_size(rect.min, vec2(width, preview_h));
    painter.rect_filled(
        preview_rect,
        CornerRadius::same(10),
        lerp_color(c.sky_top, c.sky_bottom, 0.4),
    );
    let ground = Rect::from_min_max(
        Pos2::new(preview_rect.left(), preview_rect.bottom() - 10.0),
        preview_rect.max,
    );
    painter.rect_filled(
        ground,
        CornerRadius {
            nw: 0,
            ne: 0,
            sw: 10,
            se: 10,
        },
        c.ground,
    );
    let button_w = (width * 0.55).round();
    let mut button = Rect::from_center_size(
        Pos2::new(
            preview_rect.center().x,
            preview_rect.top() + preview_h * 0.24,
        ),
        vec2(button_w, preview_h * 0.14),
    );
    painter.rect_filled(button, CornerRadius::same(2), c.primary);
    for _ in 0..2 {
        button = button.translate(vec2(0.0, preview_h * 0.2));
        painter.rect_filled(button, CornerRadius::same(2), c.button);
    }
    let ring = lerp_color(p.card_stroke, p.accent, sel.max(hover * 0.5));
    painter.rect_stroke(
        preview_rect,
        CornerRadius::same(10),
        Stroke::new(1.0 + 1.5 * sel, ring),
        StrokeKind::Outside,
    );
    painter.text(
        Pos2::new(rect.left() + 2.0, rect.bottom() - 10.0),
        Align2::LEFT_CENTER,
        style.name(),
        FontId::proportional(13.5),
        ui.visuals().text_color(),
    );
    if selected {
        let check = Rect::from_center_size(
            Pos2::new(rect.right() - 8.0, rect.bottom() - 10.0),
            vec2(12.0, 12.0),
        );
        icons::draw(painter, Icon::Check, check, p.accent);
    }
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}
