//! Fonts. The interface uses the system's own UI font (Segoe UI on
//! Windows) so the launcher reads like a native app rather than a stock
//! toolkit demo; headings use its semibold weight. Fallback fonts for
//! scripts those don't cover (Chinese, Japanese, Korean, …) come from the
//! system too, loaded the first time such text shows up, so nothing is
//! bundled and nothing extra sits in memory for people who never see it.

use std::sync::atomic::{AtomicBool, Ordering};

use eframe::egui::{self, FontData, FontFamily};
use eframe::epaint::text::{FontInsert, FontPriority, InsertFontFamily};

static REQUESTED: AtomicBool = AtomicBool::new(false);

/// The font family for headings (semibold); plain text uses Proportional.
pub const HEADING: &str = "heading";

/// System UI fonts: (regular candidates, semibold candidates), first found wins.
fn ui_fonts() -> (&'static [&'static str], &'static [&'static str]) {
    if cfg!(windows) {
        (
            &[
                r"C:\Windows\Fonts\SegUIVar.ttf",
                r"C:\Windows\Fonts\segoeui.ttf",
            ],
            &[
                r"C:\Windows\Fonts\seguisb.ttf",
                r"C:\Windows\Fonts\segoeuib.ttf",
            ],
        )
    } else {
        (
            &[
                "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
                "/usr/share/fonts/noto/NotoSans-Regular.ttf",
                "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            ],
            &[
                "/usr/share/fonts/truetype/noto/NotoSans-SemiBold.ttf",
                "/usr/share/fonts/noto/NotoSans-SemiBold.ttf",
                "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
            ],
        )
    }
}

fn first_readable(paths: &[&str]) -> Option<Vec<u8>> {
    paths.iter().find_map(|p| std::fs::read(p).ok())
}

/// Use the system UI font for text and its semibold for headings. Without
/// them (unusual systems), egui's built-in font stays and headings use it too.
pub fn install_ui(ctx: &egui::Context) {
    let (regular, semibold) = ui_fonts();
    let mut fonts = egui::FontDefinitions::default();
    if let Some(bytes) = first_readable(regular) {
        fonts.font_data.insert(
            "ui".into(),
            std::sync::Arc::new(FontData::from_owned(bytes)),
        );
        if let Some(list) = fonts.families.get_mut(&FontFamily::Proportional) {
            list.insert(0, "ui".into());
        }
    }
    let heading = FontFamily::Name(HEADING.into());
    let mut heading_list = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    if let Some(bytes) = first_readable(semibold) {
        fonts.font_data.insert(
            "ui-semibold".into(),
            std::sync::Arc::new(FontData::from_owned(bytes)),
        );
        heading_list.insert(0, "ui-semibold".into());
    }
    fonts.families.insert(heading, heading_list);
    ctx.set_fonts(fonts);
}

/// Heading text at `size`.
pub fn heading(size: f32) -> egui::FontId {
    egui::FontId::new(size, FontFamily::Name(HEADING.into()))
}

/// (path, face index in a collection) of system fonts to try, in order.
fn candidates() -> &'static [(&'static str, u32)] {
    if cfg!(windows) {
        &[
            (r"C:\Windows\Fonts\msyh.ttc", 0),    // Microsoft YaHei: Chinese
            (r"C:\Windows\Fonts\YuGothM.ttc", 0), // Yu Gothic: Japanese
            (r"C:\Windows\Fonts\malgun.ttf", 0),  // Malgun Gothic: Korean
            (r"C:\Windows\Fonts\Nirmala.ttc", 0), // Indic scripts
        ]
    } else {
        &[
            ("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", 0),
            ("/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc", 0),
            (
                "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
                0,
            ),
            ("/usr/share/fonts/truetype/wqy/wqy-microhei.ttc", 0),
        ]
    }
}

/// True if `text` has characters the built-in fonts can't draw.
fn needs_fallback(text: &str) -> bool {
    text.chars().any(|c| {
        matches!(c as u32,
            0x0900..=0x0DFF      // Indic
            | 0x1100..=0x11FF    // Hangul Jamo
            | 0x3000..=0x9FFF    // CJK symbols, kana, CJK ideographs
            | 0xAC00..=0xD7AF    // Hangul syllables
            | 0xF900..=0xFAFF    // CJK compatibility
            | 0xFF00..=0xFFEF) // full-width forms
    })
}

/// Load fallback fonts in the background if `text` needs them (once).
pub fn ensure_for(ctx: &egui::Context, text: &str) {
    if REQUESTED.load(Ordering::Relaxed) || !needs_fallback(text) {
        return;
    }
    if REQUESTED.swap(true, Ordering::Relaxed) {
        return;
    }
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        for (i, (path, index)) in candidates().iter().enumerate() {
            let Ok(bytes) = std::fs::read(path) else {
                continue;
            };
            let mut data = FontData::from_owned(bytes);
            data.index = *index;
            let families = [FontFamily::Proportional, FontFamily::Monospace]
                .into_iter()
                .map(|family| InsertFontFamily {
                    family,
                    priority: FontPriority::Lowest,
                })
                .collect();
            ctx.add_font(FontInsert::new(&format!("fallback-{i}"), data, families));
        }
        ctx.request_repaint();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_scripts_needing_fallback() {
        assert!(needs_fallback("僵尸入侵100天"));
        assert!(needs_fallback("ゾンビ"));
        assert!(needs_fallback("좀비"));
        assert!(!needs_fallback("Zombie Invade — 100 Days"));
        assert!(!needs_fallback("Привет"));
    }
}
