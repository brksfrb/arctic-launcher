//! Quick intro (~0.9 s): a snowflake spins in over the night sky, then the
//! whole overlay dissolves outward to reveal the launcher. Any click or key
//! skips it.

use std::f32::consts::PI;

use eframe::egui::emath::easing;
use eframe::egui::{Align2, Context, FontId, Id, LayerId, Order, vec2};

use super::{glow, snowflake};
use crate::theme::Palette;

const INTRO: f32 = 0.45;
const OUTRO: f32 = 0.45;
const LOGO_RADIUS: f32 = 46.0;

pub struct Splash {
    start: Option<f64>,
    done: bool,
}

impl Splash {
    pub fn new(enabled: bool) -> Self {
        Self {
            start: None,
            done: !enabled,
        }
    }

    pub fn show(&mut self, ctx: &Context, p: &Palette) {
        if self.done {
            return;
        }
        let now = ctx.input(|i| i.time);
        let t = (now - *self.start.get_or_insert(now)) as f32;
        let skipped = ctx.input(|i| i.pointer.any_click() || !i.keys_down.is_empty());
        if t >= INTRO + OUTRO || skipped {
            self.done = true;
            return;
        }
        ctx.request_repaint();

        let painter = ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("splash")));
        let rect = ctx.content_rect();
        let center = rect.center() - vec2(0.0, 16.0);

        let intro = easing::back_out((t / INTRO).min(1.0));
        let outro = easing::cubic_out(((t - INTRO) / OUTRO).clamp(0.0, 1.0));
        let visible = 1.0 - outro;

        painter.rect_filled(rect, 0.0, p.bg.gamma_multiply(visible));
        let radius = LOGO_RADIUS * intro + 90.0 * outro;
        let rotation = -(1.0 - intro) * PI / 2.0 + t * 0.4;
        glow(&painter, center, radius * 2.2, p.accent, 0.9 * visible);
        snowflake(
            &painter,
            center,
            radius,
            rotation,
            p.accent.gamma_multiply(visible),
        );

        let title_alpha = ((t - 0.15) / 0.3).clamp(0.0, 1.0) * visible;
        painter.text(
            center + vec2(0.0, LOGO_RADIUS + 34.0 + 8.0 * (1.0 - title_alpha)),
            Align2::CENTER_CENTER,
            "Arctic Launcher",
            FontId::proportional(24.0),
            p.text.gamma_multiply(title_alpha),
        );
    }
}
