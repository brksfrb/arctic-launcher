//! Reusable, animated widgets built on the theme palette.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, CursorIcon, FontId, Rect, Response, Sense, Shadow, Stroke,
    StrokeKind, Ui, Vec2, pos2, vec2,
};

use crate::art::icons::{self, Icon};
use arctic_core::instances::InstanceIcon;

use crate::art::flakes;
use crate::art::{hash01, lerp_color, snowflake};
use crate::theme::Palette;

const RUNNING_FRAME: std::time::Duration = std::time::Duration::from_millis(50);

/// What the big Play button shows.
pub enum PlayState<'a> {
    Ready,
    Blocked,
    /// Launch in progress; `fraction = None` shows an indeterminate shimmer.
    Progress {
        fraction: Option<f32>,
        label: &'a str,
    },
    Running,
}

/// Big glowing ice button that turns into a progress bar while launching.
pub fn play_button(ui: &mut Ui, p: &Palette, state: PlayState, size: Vec2) -> Response {
    let enabled = matches!(state, PlayState::Ready);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    let ctx = ui.ctx().clone();
    let hover = ctx.animate_bool(response.id.with("hover"), enabled && response.hovered());
    let press = ctx.animate_bool_with_time(
        response.id.with("press"),
        response.is_pointer_button_down_on(),
        0.08,
    );
    let rect = rect.shrink(press * 2.0);
    let radius = CornerRadius::same(14);
    let painter = ui.painter();
    let t = ui.input(|i| i.time) as f32;

    let glow_strength = match state {
        PlayState::Ready => 0.4 + 0.35 * hover,
        PlayState::Running => 0.3 + 0.15 * (t * 2.2).sin().abs(),
        PlayState::Progress { .. } => 0.3,
        PlayState::Blocked => 0.0,
    };
    if glow_strength > 0.0 {
        painter.add(
            Shadow {
                offset: [0, 5],
                blur: (20.0 + 18.0 * hover) as u8,
                spread: (1.0 + 3.0 * hover) as u8,
                color: p.accent_deep.gamma_multiply(glow_strength),
            }
            .as_shape(rect, radius),
        );
    }

    let base = match state {
        PlayState::Blocked => p.surface,
        PlayState::Progress { .. } => p.surface_hover,
        _ => lerp_color(p.accent_deep, p.accent, 0.55 + 0.45 * hover),
    };
    painter.rect_filled(rect, radius, base);

    let (text_color, label) = match &state {
        PlayState::Ready => (p.on_accent, "PLAY"),
        PlayState::Blocked => (p.muted, "PLAY"),
        PlayState::Running => (p.on_accent, "PLAYING"),
        PlayState::Progress { label, .. } => (p.text, *label),
    };
    if let PlayState::Progress { fraction, .. } = state {
        paint_progress_fill(painter, rect, radius, p, fraction, t);
        ctx.request_repaint();
    } else if !matches!(state, PlayState::Blocked) {
        let sheen = Rect::from_min_max(rect.min, pos2(rect.max.x, rect.center().y));
        let top_only = CornerRadius {
            sw: 0,
            se: 0,
            ..radius
        };
        painter.rect_filled(sheen, top_only, Color32::from_white_alpha(26));
        painter.rect_stroke(
            rect,
            radius,
            Stroke::new(1.0, Color32::from_white_alpha(90)),
            StrokeKind::Inside,
        );
    }
    // Gentle spin while playing, but only when the launcher is focused so
    // the game keeps the CPU/GPU to itself.
    if matches!(state, PlayState::Running) && ui.input(|i| i.focused) {
        ctx.request_repaint_after(RUNNING_FRAME);
    }

    let icon_center = rect.left_center() + vec2(rect.height() * 0.6, 0.0);
    snowflake(
        painter,
        icon_center,
        11.0,
        if matches!(state, PlayState::Running) {
            t * 0.8
        } else {
            0.0
        },
        text_color,
    );
    let font = if label.len() > 10 { 16.0 } else { 22.0 };
    painter.text(
        rect.center() + vec2(12.0, 0.0),
        Align2::CENTER_CENTER,
        label,
        crate::fonts::heading(font),
        text_color,
    );

    if enabled {
        response.on_hover_cursor(CursorIcon::PointingHand)
    } else {
        response
    }
}

fn paint_progress_fill(
    painter: &egui::Painter,
    rect: Rect,
    radius: CornerRadius,
    p: &Palette,
    fraction: Option<f32>,
    t: f32,
) {
    match fraction {
        Some(f) => {
            let fill = Rect::from_min_max(
                rect.min,
                pos2(rect.left() + rect.width() * f.clamp(0.0, 1.0), rect.max.y),
            );
            let clipped = painter.with_clip_rect(fill);
            clipped.rect_filled(rect, radius, p.accent_deep.gamma_multiply(0.85));
            // Moving highlight band over the filled part.
            let x = fill.left() + (t * 180.0) % (fill.width() + 120.0) - 60.0;
            let band = Rect::from_min_max(pos2(x, rect.top()), pos2(x + 60.0, rect.bottom()));
            clipped.rect_filled(band.intersect(fill), 0.0, Color32::from_white_alpha(22));
        }
        None => {
            let w = rect.width() * 0.3;
            let x = rect.left() - w + ((t * 0.9) % 1.0) * (rect.width() + w);
            let band = Rect::from_min_max(pos2(x, rect.top()), pos2(x + w, rect.bottom()));
            painter.with_clip_rect(rect).rect_filled(
                band,
                radius,
                p.accent_deep.gamma_multiply(0.55),
            );
        }
    }
}

/// Borderless icon button with a soft hover background.
pub fn icon_button(ui: &mut Ui, p: &Palette, icon: Icon, tooltip: &str) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::click());
    let hover = ui
        .ctx()
        .animate_bool(response.id.with("h"), response.hovered());
    ui.painter().rect_filled(
        rect,
        CornerRadius::same(8),
        p.surface_hover.gamma_multiply(hover),
    );
    let color = lerp_color(p.muted, p.text, hover);
    icons::draw(ui.painter(), icon, rect.shrink(7.0), color);
    response
        .on_hover_text(tooltip)
        .on_hover_cursor(CursorIcon::PointingHand)
}

/// Size of [`switch`]: a pill with a knob that slides across.
const SWITCH_SIZE: egui::Vec2 = vec2(46.0, 26.0);

/// On/off switch (a sliding knob, not a check mark). Returns a response
/// that is `changed()` when clicked.
pub fn switch(ui: &mut Ui, p: &Palette, on: &mut bool) -> Response {
    let (rect, mut response) = ui.allocate_exact_size(SWITCH_SIZE, Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    let t = ui.ctx().animate_bool_responsive(response.id, *on);
    let hover = ui
        .ctx()
        .animate_bool(response.id.with("h"), response.hovered());
    let radius = rect.height() / 2.0;
    let off_fill = lerp_color(p.surface, p.surface_hover, hover);
    ui.painter()
        .rect_filled(rect, radius, lerp_color(off_fill, p.accent, t));
    ui.painter().rect_stroke(
        rect,
        radius,
        egui::Stroke::new(1.0, lerp_color(p.card_stroke, p.accent, t)),
        egui::StrokeKind::Inside,
    );
    let knob_radius = radius - 4.0;
    let x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), t);
    ui.painter().circle_filled(
        pos2(x, rect.center().y),
        knob_radius,
        lerp_color(p.muted, p.on_accent, t),
    );
    response.on_hover_cursor(CursorIcon::PointingHand)
}

/// Square icon button with a visible tile, sized to sit next to a big
/// button (e.g. the Play button's quick actions).
pub fn tile_button(ui: &mut Ui, p: &Palette, icon: Icon, tooltip: &str, size: f32) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    let hover = ui
        .ctx()
        .animate_bool(response.id.with("h"), response.hovered());
    let radius = CornerRadius::same(12);
    ui.painter()
        .rect_filled(rect, radius, lerp_color(p.surface, p.surface_hover, hover));
    ui.painter().rect_stroke(
        rect,
        radius,
        egui::Stroke::new(1.0, lerp_color(p.card_stroke, p.accent, hover * 0.6)),
        egui::StrokeKind::Inside,
    );
    let icon_rect = egui::Rect::from_center_size(rect.center(), vec2(size * 0.36, size * 0.36));
    icons::draw(
        ui.painter(),
        icon,
        icon_rect,
        lerp_color(p.muted, p.text, hover),
    );
    response
        .on_hover_text(tooltip)
        .on_hover_cursor(CursorIcon::PointingHand)
}

/// Text button with a leading icon; `primary` uses the accent color.
/// Height of every button.
pub const BUTTON_HEIGHT: f32 = 34.0;

/// A row of controls with buttons in it. Like `ui.horizontal`, but everything is centered on
/// the buttons' height: egui centers a row on its default control height (18), so text beside
/// a button sat near its top.
pub fn row<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<R> {
    let normal = ui.spacing().interact_size.y;
    ui.spacing_mut().interact_size.y = BUTTON_HEIGHT;
    let response = ui.horizontal(|ui| {
        ui.spacing_mut().interact_size.y = normal;
        add(ui)
    });
    ui.spacing_mut().interact_size.y = normal;
    response
}

pub fn button(
    ui: &mut Ui,
    p: &Palette,
    icon: Option<Icon>,
    label: &str,
    primary: bool,
) -> Response {
    let (fg, bg) = if primary {
        (p.on_accent, p.accent)
    } else {
        (p.text, p.surface)
    };
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), FontId::proportional(14.0), fg);
    let icon_w = if icon.is_some() { 22.0 } else { 0.0 };
    let size = vec2(galley.size().x + icon_w + 28.0, BUTTON_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let hover = ui
        .ctx()
        .animate_bool(response.id.with("h"), response.hovered());
    let fill = if primary {
        lerp_color(p.accent_deep, p.accent, 0.6 + 0.4 * hover)
    } else {
        lerp_color(p.surface, p.surface_hover, hover)
    };
    ui.painter().rect_filled(rect, CornerRadius::same(9), fill);
    let mut x = rect.left() + 14.0;
    if let Some(icon) = icon {
        icons::draw(
            ui.painter(),
            icon,
            Rect::from_center_size(pos2(x + 8.0, rect.center().y), vec2(16.0, 16.0)),
            fg,
        );
        x += icon_w;
    }
    ui.painter()
        .galley(pos2(x, rect.center().y - galley.size().y / 2.0), galley, fg);
    let _ = bg;
    response.on_hover_cursor(CursorIcon::PointingHand)
}

/// A clickable card surface that lifts and glows on hover. Returns the
/// content rect, the response, and the hover amount for extra effects.
pub fn hover_card(ui: &mut Ui, p: &Palette, size: Vec2, selected: bool) -> (Rect, Response, f32) {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    // Pointer-in-rect rather than `hovered()`, so child buttons (like a
    // trash icon) don't make the card think the pointer left.
    let inside = ui.rect_contains_pointer(rect);
    let hover = ui.ctx().animate_bool(response.id.with("h"), inside);
    let sel = ui.ctx().animate_bool(response.id.with("s"), selected);
    let lifted = rect.translate(vec2(0.0, -2.0 * hover));
    let radius = CornerRadius::same(14);
    let painter = ui.painter();
    painter.add(
        Shadow {
            offset: [0, (4.0 + 4.0 * hover) as i8],
            blur: (14.0 + 10.0 * hover) as u8,
            spread: 0,
            color: lerp_color(Color32::BLACK, p.accent_deep, sel)
                .gamma_multiply(0.25 + 0.2 * hover + 0.2 * sel),
        }
        .as_shape(lifted, radius),
    );
    painter.rect_filled(lifted, radius, p.card_fill);
    let stroke_color = lerp_color(p.card_stroke, p.accent, (0.35 * hover).max(sel));
    painter.rect_stroke(
        lifted,
        radius,
        Stroke::new(1.0 + sel, stroke_color),
        StrokeKind::Inside,
    );
    (
        lifted,
        response.on_hover_cursor(CursorIcon::PointingHand),
        hover,
    )
}

/// Heading + optional subtitle used at the top of every tab.
pub fn page_header(ui: &mut Ui, p: &Palette, title: &str, subtitle: &str) {
    ui.label(
        egui::RichText::new(title)
            .font(crate::fonts::heading(30.0))
            .color(p.text),
    );
    if !subtitle.is_empty() {
        ui.label(egui::RichText::new(subtitle).color(p.muted));
    }
    ui.add_space(14.0);
}

/// Faint pulsing placeholder bar while something loads.
pub fn skeleton(ui: &mut Ui, p: &Palette, size: Vec2, seed: u32) {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let t = ui.input(|i| i.time) as f32;
    let pulse = 0.5 + 0.5 * (t * 3.0 + hash01(seed, 1) * 6.0).sin();
    ui.painter().rect_filled(
        rect,
        CornerRadius::same(6),
        lerp_color(p.surface, p.surface_hover, pulse),
    );
    ui.ctx().request_repaint();
}

/// Seconds the Play burst lasts.
pub const BURST_TIME: f32 = 0.8;

/// Little snowflakes flying out of the Play button when it's pressed.
pub fn burst(painter: &egui::Painter, center: egui::Pos2, age: f32, color: Color32) {
    const COUNT: u32 = 18;
    let t = (age / BURST_TIME).clamp(0.0, 1.0);
    let ease = 1.0 - (1.0 - t).powi(3);
    for i in 0..COUNT {
        let angle = i as f32 / COUNT as f32 * std::f32::consts::TAU + hash01(i, 50) * 0.4;
        let distance = (60.0 + 90.0 * hash01(i, 51)) * ease;
        let pos = center
            + Vec2::angled(angle) * distance * egui::vec2(1.6, 1.0)
            + egui::vec2(0.0, 30.0 * t * t);
        let size = 2.5 + 2.5 * hash01(i, 52);
        snowflake(
            painter,
            pos,
            size,
            angle + t * 3.0,
            color.gamma_multiply(1.0 - t),
        );
    }
}

/// Rounded tile with the instance's snowflake. Clickable (opens the icon
/// picker); glows a little brighter on hover.
pub fn instance_emblem(ui: &mut Ui, p: &Palette, icon: &InstanceIcon, size: f32) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    let hover = ui
        .ctx()
        .animate_bool(response.id.with("h"), response.hovered());
    let t = ui.input(|i| i.time) as f32;
    paint_emblem_with_hover(ui.painter(), p, icon, rect, t, hover);
    response
        .on_hover_text("Change icon")
        .on_hover_cursor(CursorIcon::PointingHand)
}

/// Clickable emblem at a fixed `rect` (for emblems painted inside cards).
pub fn emblem_at(
    ui: &mut Ui,
    p: &Palette,
    icon: &InstanceIcon,
    rect: Rect,
    id: egui::Id,
) -> Response {
    let response = ui.interact(rect, id, Sense::click());
    let hover = ui.ctx().animate_bool(id.with("h"), response.hovered());
    let t = ui.input(|i| i.time) as f32;
    paint_emblem_with_hover(ui.painter(), p, icon, rect, t, hover);
    response
        .on_hover_text("Change icon")
        .on_hover_cursor(CursorIcon::PointingHand)
}

/// One line of text, left-center aligned at `pos`, cut with "…" past `max_width`.
pub fn text_elided(
    painter: &egui::Painter,
    pos: egui::Pos2,
    text: &str,
    font: egui::FontId,
    color: egui::Color32,
    max_width: f32,
) {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(max_width.max(0.0));
    let galley = painter.layout_job(job);
    let top_left = egui::pos2(pos.x, pos.y - galley.size().y / 2.0);
    painter.galley(top_left, galley, color);
}

/// Paint an instance emblem into `rect` (non-interactive).
pub fn paint_emblem(painter: &egui::Painter, p: &Palette, icon: &InstanceIcon, rect: Rect, t: f32) {
    paint_emblem_with_hover(painter, p, icon, rect, t, 0.0);
}

fn paint_emblem_with_hover(
    painter: &egui::Painter,
    p: &Palette,
    icon: &InstanceIcon,
    rect: Rect,
    t: f32,
    hover: f32,
) {
    let size = rect.width();
    let color = flakes::icon_color(icon, p.accent);
    let radius = CornerRadius::same((size * 0.25) as u8);
    painter.rect_filled(rect, radius, color.gamma_multiply(0.14 + 0.08 * hover));
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(1.0, color.gamma_multiply(0.4 + 0.3 * hover)),
        StrokeKind::Inside,
    );
    crate::art::glow(
        painter,
        rect.center(),
        size * 0.55,
        color,
        0.5 + 0.3 * hover,
    );
    flakes::draw(
        painter,
        icon.style,
        rect.center(),
        size * 0.32,
        t * 0.1,
        color,
    );
}

/// Height of text fields, and of controls placed next to them.
pub const FIELD_HEIGHT: f32 = 36.0;

/// A row as tall as a text field, with its contents vertically centered
/// (so a label lines up with the field next to it).
pub fn field_row<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<R> {
    ui.allocate_ui_with_layout(
        vec2(ui.available_width(), FIELD_HEIGHT),
        egui::Layout::left_to_right(egui::Align::Center),
        add,
    )
}

/// Roomy single-line text field used everywhere (taller than egui's
/// default, with comfortable padding and a slightly larger font).
pub fn text_field(text: &mut dyn egui::TextBuffer) -> egui::TextEdit<'_> {
    egui::TextEdit::singleline(text)
        .margin(egui::Margin::symmetric(10, 8))
        .font(FontId::proportional(15.0))
        .min_size(vec2(0.0, FIELD_HEIGHT))
}

/// Dialogs are the color of an empty checkbox or slider track; lift those
/// so unchecked boxes show.
pub fn lift_controls(ui: &mut Ui, p: &Palette) {
    let widgets = &mut ui.visuals_mut().widgets;
    widgets.inactive.bg_fill = p.surface_hover;
    widgets.inactive.bg_stroke = egui::Stroke::new(1.0, p.card_stroke);
}

/// Minecraft's 16 text colors, for `§0`..`§f`.
const MC_COLORS: [u32; 16] = [
    0x000000, 0x0000aa, 0x00aa00, 0x00aaaa, 0xaa0000, 0xaa00aa, 0xffaa00, 0xaaaaaa, 0x555555,
    0x5555ff, 0x55ff55, 0x55ffff, 0xff5555, 0xff55ff, 0xffff55, 0xffffff,
];

/// Text with Minecraft `§` color codes (and `§#rrggbb`), colored like the
/// game draws it; text before any code (or after `§r`) uses `plain`.
pub fn mc_text(text: &str, plain: Color32, font: FontId) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let mut color = plain;
    let mut run = String::new();
    let mut chars = text.chars().peekable();
    let flush = |job: &mut egui::text::LayoutJob, run: &mut String, color: Color32| {
        if !run.is_empty() {
            job.append(run, 0.0, egui::TextFormat::simple(font.clone(), color));
            run.clear();
        }
    };
    while let Some(c) = chars.next() {
        if c != '§' {
            run.push(c);
            continue;
        }
        let Some(code) = chars.next() else {
            break;
        };
        if code == '#' {
            let hex: String = chars.by_ref().take(6).collect();
            if let Ok(v) = u32::from_str_radix(&hex, 16) {
                flush(&mut job, &mut run, color);
                color = Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8);
            }
            continue;
        }
        if let Some(i) = code.to_digit(16) {
            flush(&mut job, &mut run, color);
            let v = MC_COLORS[i as usize];
            color = Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8);
        } else if code == 'r' {
            flush(&mut job, &mut run, color);
            color = plain;
        }
        // Bold, italic and the rest are left out.
    }
    flush(&mut job, &mut run, color);
    job
}

/// A `file://` URI egui's loader reads back as `path`. On Windows it needs
/// `file:///C:/…`: with two slashes, `C:` is taken for a network host.
pub fn file_uri(path: &std::path::Path) -> String {
    let path = path.to_string_lossy();
    if cfg!(windows) {
        format!("file:///{}", path.replace('\\', "/"))
    } else {
        format!("file://{path}")
    }
}

#[cfg(test)]
mod file_uri_tests {
    #[cfg(windows)]
    #[test]
    fn windows_paths_get_three_slashes() {
        let uri = super::file_uri(std::path::Path::new(r"C:\Users\a b\shot.png"));
        assert_eq!(uri, "file:///C:/Users/a b/shot.png");
    }
}
