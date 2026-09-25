//! Cross-fade between themes. Blended palettes are built once per
//! (from, to, step) and kept for the life of the process, so the rest of
//! the UI can keep using `&'static Palette`. At most 3 × 3 × `STEPS`.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use arctic_core::settings::ThemeMode;
use eframe::egui::Color32;

use crate::theme::{self, Palette, Scene};

/// Length of the fade in seconds.
pub const DURATION: f64 = 0.4;
/// Distinct blend levels between two themes.
const STEPS: u8 = 16;

/// A running theme change.
#[derive(Debug, Clone, Copy)]
pub struct ThemeFade {
    pub from: ThemeMode,
    pub start: f64,
    step: u8,
}

impl ThemeFade {
    pub fn new(from: ThemeMode, start: f64) -> Self {
        Self {
            from,
            start,
            step: 0,
        }
    }

    /// Advance to `now`. Returns false once the fade has finished.
    pub fn update(&mut self, now: f64) -> bool {
        let t = ((now - self.start) / DURATION).clamp(0.0, 1.0);
        // Ease out so the change lands quickly and settles softly.
        let eased = 1.0 - (1.0 - t).powi(3);
        self.step = (eased * f64::from(STEPS)).round() as u8;
        self.step < STEPS
    }

    pub fn palette(&self, to: ThemeMode) -> &'static Palette {
        blended(self.from, to, self.step)
    }
}

/// (from, to, step) → blended palette.
type BlendCache = HashMap<(u8, u8, u8), &'static Palette>;

fn blended(from: ThemeMode, to: ThemeMode, step: u8) -> &'static Palette {
    if from == to || step >= STEPS {
        return theme::palette(to);
    }
    if step == 0 {
        return theme::palette(from);
    }
    static CACHE: OnceLock<Mutex<BlendCache>> = OnceLock::new();
    let key = (index(from), index(to), step);
    let mut cache = CACHE
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    cache.entry(key).or_insert_with(|| {
        let t = f32::from(step) / f32::from(STEPS);
        Box::leak(Box::new(mix_palette(
            theme::palette(from),
            theme::palette(to),
            t,
        )))
    })
}

fn index(mode: ThemeMode) -> u8 {
    match mode {
        ThemeMode::Default => 0,
        ThemeMode::Dark => 1,
        ThemeMode::Light => 2,
    }
}

/// Blend premultiplied colors, alpha included.
fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let m = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Color32::from_rgba_premultiplied(
        m(a.r(), b.r()),
        m(a.g(), b.g()),
        m(a.b(), b.b()),
        m(a.a(), b.a()),
    )
}

fn mix_palette(a: &Palette, b: &Palette, t: f32) -> Palette {
    let c = |x: Color32, y: Color32| mix(x, y, t);
    Palette {
        dark: if t < 0.5 { a.dark } else { b.dark },
        bg: c(a.bg, b.bg),
        surface: c(a.surface, b.surface),
        surface_hover: c(a.surface_hover, b.surface_hover),
        accent: c(a.accent, b.accent),
        accent_deep: c(a.accent_deep, b.accent_deep),
        on_accent: c(a.on_accent, b.on_accent),
        text: c(a.text, b.text),
        muted: c(a.muted, b.muted),
        warn: c(a.warn, b.warn),
        error: c(a.error, b.error),
        card_fill: c(a.card_fill, b.card_fill),
        card_stroke: c(a.card_stroke, b.card_stroke),
        nav_fill: c(a.nav_fill, b.nav_fill),
        titlebar: c(a.titlebar, b.titlebar),
        shadow: c(a.shadow, b.shadow),
        scene: mix_scene(&a.scene, &b.scene, t),
    }
}

fn mix_scene(a: &Scene, b: &Scene, t: f32) -> Scene {
    let c = |x: Color32, y: Color32| mix(x, y, t);
    let f = |x: f32, y: f32| x + (y - x) * t;
    let pair = |i: usize| {
        (
            c(a.mountains[i].0, b.mountains[i].0),
            c(a.mountains[i].1, b.mountains[i].1),
        )
    };
    Scene {
        sky: [
            c(a.sky[0], b.sky[0]),
            c(a.sky[1], b.sky[1]),
            c(a.sky[2], b.sky[2]),
        ],
        star_strength: f(a.star_strength, b.star_strength),
        aurora_strength: f(a.aurora_strength, b.aurora_strength),
        snow: c(a.snow, b.snow),
        rim: c(a.rim, b.rim),
        snow_cap: c(a.snow_cap, b.snow_cap),
        mountains: [pair(0), pair(1), pair(2)],
        celestial: if t < 0.5 { a.celestial } else { b.celestial },
        mist: c(a.mist, b.mist),
        trees: c(a.trees, b.trees),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fade_ends_on_target_theme() {
        let mut fade = ThemeFade::new(ThemeMode::Default, 0.0);
        assert!(fade.update(0.0));
        assert!(std::ptr::eq(
            fade.palette(ThemeMode::Light),
            theme::palette(ThemeMode::Default)
        ));
        assert!(!fade.update(DURATION + 0.01));
        assert!(std::ptr::eq(
            fade.palette(ThemeMode::Light),
            theme::palette(ThemeMode::Light)
        ));
    }

    #[test]
    fn midpoint_blends_and_is_cached() {
        let a = blended(ThemeMode::Dark, ThemeMode::Light, STEPS / 2);
        let b = blended(ThemeMode::Dark, ThemeMode::Light, STEPS / 2);
        assert!(std::ptr::eq(a, b));
        let (dark, light) = (
            theme::palette(ThemeMode::Dark),
            theme::palette(ThemeMode::Light),
        );
        assert_ne!(a.bg, dark.bg);
        assert_ne!(a.bg, light.bg);
    }
}
