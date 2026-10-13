//! How big the launcher's interface is, on top of the system's display scaling.
//!
//! A 1080p laptop at Windows' 150% leaves the launcher 1280 points across, which it was not laid
//! out for. "Auto" scales down (never below 80%) until the screen is at least [`ROOM`] points;
//! a size picked in Settings is used as it is.

use eframe::egui;

/// The room the layout is made for (points across, points down).
const ROOM: egui::Vec2 = egui::vec2(1600.0, 900.0);
/// Auto never makes the interface smaller than this.
const AUTO_MIN: f32 = 0.8;
/// Changes smaller than this aren't worth a new layout.
const STEP: f32 = 0.005;

/// The choices: 0 is Auto, the rest percent.
pub const SIZES: [(u16, &str); 7] = [
    (0, "Auto"),
    (80, "80%"),
    (90, "90%"),
    (100, "100%"),
    (110, "110%"),
    (125, "125%"),
    (150, "150%"),
];

/// Set the zoom for `size` (0: Auto). Every frame; only a change does anything.
pub fn apply(ctx: &egui::Context, size: u16) {
    let Some(zoom) = zoom_for(ctx, size) else {
        return;
    };
    if (ctx.zoom_factor() - zoom).abs() > STEP {
        ctx.set_zoom_factor(zoom);
    }
}

fn zoom_for(ctx: &egui::Context, size: u16) -> Option<f32> {
    if size != 0 {
        return Some(f32::from(size.clamp(50, 200)) / 100.0);
    }
    let (monitor, native) = ctx.input(|i| {
        let v = i.viewport();
        (v.monitor_size, v.native_pixels_per_point)
    });
    // The monitor comes in points at the current zoom: back to pixels, then to the system's points.
    let pixels = monitor? * ctx.pixels_per_point();
    Some(auto_zoom(pixels / native?))
}

/// Auto's zoom for a screen this many points (at the system's scaling) across and down.
fn auto_zoom(points: egui::Vec2) -> f32 {
    let fit = (points.x / ROOM.x).min(points.y / ROOM.y);
    fit.clamp(AUTO_MIN, 1.0)
}

/// The size picker, for Settings and the first start.
pub fn size_row(ui: &mut egui::Ui, size: &mut u16) {
    crate::widgets::field_row(ui, |ui| {
        ui.label("Interface size").on_hover_text(
            "Auto makes the launcher a little smaller on small or highly scaled screens (a laptop at 150%). \
             The sizes are on top of your system's display scaling.",
        );
        for (value, name) in SIZES {
            ui.selectable_value(size, value, name);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_shrinks_a_crowded_screen_only() {
        // 1920x1080 at 100%: as it is.
        assert_eq!(auto_zoom(egui::vec2(1920.0, 1080.0)), 1.0);
        // The same screen at 150% (1280x720 points): down to 80%.
        assert!((auto_zoom(egui::vec2(1280.0, 720.0)) - 0.8).abs() < 1e-6);
        // At 125% (1536x864): a little smaller.
        let z = auto_zoom(egui::vec2(1536.0, 864.0));
        assert!(z > 0.9 && z < 1.0, "{z}");
        // A tiny screen never goes below 80%.
        assert_eq!(auto_zoom(egui::vec2(800.0, 600.0)), AUTO_MIN);
    }
}
