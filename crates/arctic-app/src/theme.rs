//! Color themes. `Default` is the aurora night, `Dark` is a quieter graphite
//! night, `Light` is a daytime arctic. Each palette also drives the painted
//! scenery (`art::scenery`).

use arctic_core::settings::ThemeMode;
use eframe::egui::{self, Color32, CornerRadius, Stroke, Theme};

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// Unmultiplied `#rrggbb` + alpha, premultiplied at compile time.
const fn rgba(hex: u32, alpha: u8) -> Color32 {
    Color32::from_rgba_premultiplied(
        premultiply(hex >> 16, alpha),
        premultiply(hex >> 8, alpha),
        premultiply(hex, alpha),
        alpha,
    )
}

const fn premultiply(channel: u32, alpha: u8) -> u8 {
    ((channel & 0xff) * alpha as u32 / 255) as u8
}

/// Colors for the painted backdrop.
pub struct Scene {
    /// Sky gradient: top, middle, horizon.
    pub sky: [Color32; 3],
    pub star_strength: f32,
    pub aurora_strength: f32,
    pub snow: Color32,
    /// Ridge outline color (alpha-blended).
    pub rim: Color32,
    pub snow_cap: Color32,
    /// (ridge color, base color) for the far, middle and near layers.
    pub mountains: [(Color32, Color32); 3],
    pub celestial: Celestial,
    /// Haze drawn between mountain layers (alpha-blended).
    pub mist: Color32,
    /// Pine tree silhouettes along the bottom edge.
    pub trees: Color32,
}

/// The body in the sky.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Celestial {
    Moon,
    Sun,
}

pub struct Palette {
    pub dark: bool,
    pub bg: Color32,
    pub surface: Color32,
    pub surface_hover: Color32,
    pub accent: Color32,
    pub accent_deep: Color32,
    /// Text drawn on top of the accent color (Play button).
    pub on_accent: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub warn: Color32,
    pub error: Color32,
    pub card_fill: Color32,
    pub card_stroke: Color32,
    pub nav_fill: Color32,
    /// Native title bar color (matches the sidebar over the sky).
    pub titlebar: Color32,
    pub shadow: Color32,
    pub scene: Scene,
}

pub static DEFAULT: Palette = Palette {
    dark: true,
    bg: rgb(0x0b1320),
    surface: rgb(0x16243a),
    surface_hover: rgb(0x1e314d),
    accent: rgb(0x7dd3fc),
    accent_deep: rgb(0x38bdf8),
    on_accent: rgb(0x0b1320),
    text: rgb(0xe6f1ff),
    muted: rgb(0x8aa4c2),
    warn: rgb(0xfbbf24),
    error: rgb(0xf87171),
    card_fill: rgba(0x101b2c, 205),
    card_stroke: rgba(0x7dd3fc, 56),
    nav_fill: rgba(0x070f1c, 215),
    titlebar: rgb(0x070e1b),
    shadow: Color32::from_black_alpha(90),
    scene: Scene {
        sky: [rgb(0x050a14), rgb(0x0a172b), rgb(0x122e46)],
        star_strength: 1.0,
        aurora_strength: 1.0,
        snow: rgb(0xe6f1ff),
        rim: rgba(0xb8e8ff, 80),
        snow_cap: rgba(0xe6f1ff, 140),
        mountains: [
            (rgb(0x2a4d6e), rgb(0x122a42)),
            (rgb(0x193452), rgb(0x0c1d31)),
            (rgb(0x0d1c2f), rgb(0x07101c)),
        ],
        celestial: Celestial::Moon,
        mist: rgba(0x2a4d6e, 70),
        trees: rgb(0x06101b),
    },
};

pub static DARK: Palette = Palette {
    dark: true,
    bg: rgb(0x0c0f13),
    surface: rgb(0x1d232b),
    surface_hover: rgb(0x28303a),
    accent: rgb(0xa5cde8),
    accent_deep: rgb(0x6aa6cc),
    on_accent: rgb(0x0c0f13),
    text: rgb(0xe4e8ed),
    muted: rgb(0x8a95a2),
    warn: rgb(0xf2b84b),
    error: rgb(0xf08080),
    card_fill: rgba(0x151a20, 220),
    card_stroke: rgba(0xa5cde8, 34),
    nav_fill: rgba(0x0a0c10, 230),
    titlebar: rgb(0x0a0c10),
    shadow: Color32::from_black_alpha(110),
    scene: Scene {
        sky: [rgb(0x06080b), rgb(0x0d1117), rgb(0x161d26)],
        star_strength: 0.7,
        aurora_strength: 0.35,
        snow: rgb(0xd5dce4),
        rim: rgba(0xc8d6e3, 50),
        snow_cap: rgba(0xcfd8e2, 90),
        mountains: [
            (rgb(0x2a323d), rgb(0x161b22)),
            (rgb(0x1d232b), rgb(0x101419)),
            (rgb(0x11151a), rgb(0x0a0c0f)),
        ],
        celestial: Celestial::Moon,
        mist: rgba(0x2a323d, 70),
        trees: rgb(0x08090c),
    },
};

pub static LIGHT: Palette = Palette {
    dark: false,
    bg: rgb(0xeef5fb),
    surface: rgb(0xdbe8f4),
    surface_hover: rgb(0xc8dcee),
    accent: rgb(0x0284c7),
    accent_deep: rgb(0x0369a1),
    on_accent: rgb(0xffffff),
    text: rgb(0x0f2436),
    muted: rgb(0x587086),
    warn: rgb(0xb45309),
    error: rgb(0xdc2626),
    card_fill: rgba(0xffffff, 215),
    card_stroke: rgba(0x0284c7, 40),
    nav_fill: rgba(0xf5faff, 225),
    titlebar: rgb(0xe6f2fc),
    shadow: rgba(0x1e3a5f, 40),
    scene: Scene {
        sky: [rgb(0x7cc4ee), rgb(0xcde8f9), rgb(0xf4fbff)],
        star_strength: 0.0,
        aurora_strength: 0.0,
        snow: rgb(0x8fb8d8),
        rim: rgba(0x5f9fcf, 110),
        snow_cap: rgba(0xffffff, 235),
        mountains: [
            (rgb(0xb9d6ec), rgb(0x94bbd9)),
            (rgb(0xd6e8f5), rgb(0xadcce3)),
            (rgb(0xf7fbfe), rgb(0xd3e5f2)),
        ],
        celestial: Celestial::Sun,
        mist: rgba(0xffffff, 110),
        trees: rgb(0x4d7a99),
    },
};

pub fn palette(mode: ThemeMode) -> &'static Palette {
    match mode {
        ThemeMode::Default => &DEFAULT,
        ThemeMode::Dark => &DARK,
        ThemeMode::Light => &LIGHT,
    }
}

const RADIUS: u8 = 6;

pub fn apply(ctx: &egui::Context, p: &Palette) {
    let (theme, mut v) = if p.dark {
        (Theme::Dark, egui::Visuals::dark())
    } else {
        (Theme::Light, egui::Visuals::light())
    };
    ctx.set_theme(theme);
    v.override_text_color = Some(p.text);
    v.panel_fill = p.bg;
    v.window_fill = p.surface;
    v.extreme_bg_color = if p.dark {
        p.bg.gamma_multiply(0.7)
    } else {
        rgb(0xffffff)
    };
    v.faint_bg_color = p.surface;
    v.hyperlink_color = p.accent;
    v.warn_fg_color = p.warn;
    v.error_fg_color = p.error;
    v.selection.bg_fill = p.accent_deep.gamma_multiply(0.45);
    v.selection.stroke = Stroke::new(1.0, p.accent);
    v.window_stroke = Stroke::new(1.0, p.surface_hover);

    let radius = CornerRadius::same(RADIUS);
    let w = &mut v.widgets;
    for (state, fill) in [
        (&mut w.inactive, p.surface),
        (&mut w.hovered, p.surface_hover),
        (&mut w.active, p.accent_deep.gamma_multiply(0.6)),
        (&mut w.open, p.surface_hover),
    ] {
        state.bg_fill = fill;
        state.weak_bg_fill = fill;
        state.corner_radius = radius;
    }
    w.noninteractive.bg_fill = p.surface;
    w.noninteractive.weak_bg_fill = p.surface;
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.surface);
    w.noninteractive.corner_radius = radius;
    w.hovered.bg_stroke = Stroke::new(1.0, p.accent.gamma_multiply(0.6));

    ctx.set_visuals_of(theme, v);
    ctx.style_mut_of(theme, |style| {
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(12.0, 6.0);
    });
}

/// Frosted-ice card: translucent so the scenery shows through, with a
/// faint edge and a soft drop shadow.
pub fn card(p: &Palette) -> egui::Frame {
    egui::Frame::new()
        .fill(p.card_fill)
        .stroke(Stroke::new(1.0, p.card_stroke))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(16.0)
        .shadow(egui::Shadow {
            offset: [0, 6],
            blur: 18,
            spread: 0,
            color: p.shadow,
        })
}

/// Side navigation: frosted glass with a faint right edge.
pub fn nav_frame(p: &Palette) -> egui::Frame {
    egui::Frame::new().fill(p.nav_fill).inner_margin(12.0)
}

/// Solid strip for the update banner and notices.
pub fn strip(p: &Palette) -> egui::Frame {
    egui::Frame::new().fill(p.surface).inner_margin(8.0)
}
