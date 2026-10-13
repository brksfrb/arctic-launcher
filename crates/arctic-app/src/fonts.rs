//! Fonts. The interface uses the system's own UI font (Segoe UI on
//! Windows) so the launcher reads like a native app rather than a stock
//! toolkit demo; headings use its semibold weight. Fallback fonts for
//! scripts those don't cover (Chinese, Japanese, Korean, …) come from the
//! system too, loaded the first time such text shows up, so nothing is
//! bundled and nothing extra sits in memory for people who never see it.

use std::sync::atomic::{AtomicBool, Ordering};

use eframe::egui::{self, FontData, FontFamily};
use eframe::epaint::text::{FontInsert, FontPriority, InsertFontFamily};

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
        fonts
            .font_data
            .insert("ui".into(), std::sync::Arc::new(centered(bytes)));
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
        fonts
            .font_data
            .insert("ui-semibold".into(), std::sync::Arc::new(centered(bytes)));
        heading_list.insert(0, "ui-semibold".into());
    }
    fonts.families.insert(heading, heading_list);
    ctx.set_fonts(fonts);
}

/// A font whose letters sit in the middle of their line: centered text (button labels) lines
/// up with icons and boxes. Fonts like Segoe UI keep more room above the letters than below, so
/// text centered by its line box looks low; this moves the glyphs up by the difference.
fn centered(bytes: Vec<u8>) -> FontData {
    let shift = letters_low_by(&bytes);
    let mut data = FontData::from_owned(bytes);
    if let Some(shift) = shift {
        data.tweak.y_offset_factor = -shift;
    }
    data
}

/// How far (in font sizes) the middle of the capital letters is below the middle of the line
/// box: (ascender - descender - cap height) / 2, from the font's `hhea`, `OS/2` and `head`.
fn letters_low_by(font: &[u8]) -> Option<f32> {
    let u16_at = |at: usize| Some(u16::from_be_bytes(font.get(at..at + 2)?.try_into().ok()?));
    let i16_at = |at: usize| u16_at(at).map(|v| v as i16);
    let table = |tag: &[u8; 4]| -> Option<usize> {
        let count = usize::from(u16_at(4)?);
        for i in 0..count {
            let rec = 12 + i * 16;
            if font.get(rec..rec + 4)? == tag {
                let offset: [u8; 4] = font.get(rec + 8..rec + 12)?.try_into().ok()?;
                return Some(u32::from_be_bytes(offset) as usize);
            }
        }
        None
    };
    let units = f32::from(u16_at(table(b"head")? + 18)?);
    let hhea = table(b"hhea")?;
    let ascent = f32::from(i16_at(hhea + 4)?);
    let descent = f32::from(i16_at(hhea + 6)?).abs();
    let os2 = table(b"OS/2")?;
    // sCapHeight is in OS/2 version 2 and later.
    if u16_at(os2)? < 2 {
        return None;
    }
    let cap = f32::from(i16_at(os2 + 88)?);
    if units <= 0.0 || cap <= 0.0 {
        return None;
    }
    let low = (ascent - descent - cap) / 2.0 / units;
    // Only a nudge: a broken table shouldn't throw the text around.
    Some(low.clamp(-0.15, 0.15))
}

/// Heading text at `size`.
pub fn heading(size: f32) -> egui::FontId {
    egui::FontId::new(size, FontFamily::Name(HEADING.into()))
}

/// Scripts the built-in fonts can't draw; each loads its own system font, only when shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Script {
    Han,
    Kana,
    Hangul,
    Indic,
}

const SCRIPTS: [Script; 4] = [Script::Han, Script::Kana, Script::Hangul, Script::Indic];

impl Script {
    fn of(c: char) -> Option<Self> {
        match c as u32 {
            0x0900..=0x0DFF => Some(Self::Indic),
            0x3040..=0x30FF | 0x31F0..=0x31FF => Some(Self::Kana),
            0x1100..=0x11FF | 0x3130..=0x318F | 0xAC00..=0xD7AF => Some(Self::Hangul),
            0x3000..=0x9FFF | 0xF900..=0xFAFF | 0xFF00..=0xFFEF => Some(Self::Han),
            _ => None,
        }
    }

    fn index(self) -> usize {
        self as usize
    }

    /// (path, face index in a collection) of system fonts that draw it, tried in order.
    fn fonts(self) -> &'static [(&'static str, u32)] {
        if cfg!(windows) {
            match self {
                Self::Han => &[(r"C:\Windows\Fonts\msyh.ttc", 0)], // Microsoft YaHei
                Self::Kana => &[
                    (r"C:\Windows\Fonts\YuGothM.ttc", 0),
                    (r"C:\Windows\Fonts\msgothic.ttc", 0),
                ],
                Self::Hangul => &[(r"C:\Windows\Fonts\malgun.ttf", 0)], // Malgun Gothic
                Self::Indic => &[(r"C:\Windows\Fonts\Nirmala.ttc", 0)],
            }
        } else {
            // One Noto CJK covers Han, kana and Hangul (mapped once, see LOADED).
            const CJK: &[(&str, u32)] = &[
                ("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", 0),
                ("/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc", 0),
                (
                    "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
                    0,
                ),
                ("/usr/share/fonts/truetype/wqy/wqy-microhei.ttc", 0),
                ("/System/Library/Fonts/PingFang.ttc", 0),
            ];
            match self {
                Self::Indic => &[(
                    "/usr/share/fonts/truetype/noto/NotoSansDevanagari-Regular.ttf",
                    0,
                )],
                _ => CJK,
            }
        }
    }
}

/// Scripts asked for so far (each loads once).
static REQUESTED: [AtomicBool; SCRIPTS.len()] = [const { AtomicBool::new(false) }; SCRIPTS.len()];
/// Font files already added (a file serving two scripts is added once).
static LOADED: std::sync::Mutex<Vec<&'static str>> = std::sync::Mutex::new(Vec::new());

/// True if `text` has characters the built-in fonts can't draw.
fn needs_fallback(text: &str) -> bool {
    text.chars().any(|c| Script::of(c).is_some())
}

/// Load the fallback fonts `text` needs in the background (each script once). Only the script
/// shown is loaded (the four Windows ones together are some 50 MB), and the file is mapped,
/// not read: its pages are the system's file cache, shared, not this program's memory.
pub fn ensure_for(ctx: &egui::Context, text: &str) {
    if !needs_fallback(text) {
        return;
    }
    let mut wanted = Vec::new();
    for c in text.chars() {
        if let Some(script) = Script::of(c)
            && !wanted.contains(&script)
            && !REQUESTED[script.index()].swap(true, Ordering::Relaxed)
        {
            wanted.push(script);
        }
    }
    if wanted.is_empty() {
        return;
    }
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        for script in wanted {
            add_fallback(&ctx, script);
        }
        ctx.request_repaint();
    });
}

fn add_fallback(ctx: &egui::Context, script: Script) {
    for &(path, index) in script.fonts() {
        let mut loaded = LOADED
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if loaded.contains(&path) {
            return;
        }
        let Some(bytes) = map(path) else {
            continue;
        };
        loaded.push(path);
        drop(loaded);
        let mut data = FontData::from_static(bytes);
        data.index = index;
        let families = [FontFamily::Proportional, FontFamily::Monospace]
            .into_iter()
            .map(|family| InsertFontFamily {
                family,
                priority: FontPriority::Lowest,
            })
            .collect();
        ctx.add_font(FontInsert::new(
            &format!("fallback-{script:?}"),
            data,
            families,
        ));
        return;
    }
}

/// A font file mapped for the rest of the run (fonts are never removed once added).
fn map(path: &str) -> Option<&'static [u8]> {
    let file = std::fs::File::open(path).ok()?;
    // SAFETY: a system font, mapped read-only; Windows keeps a mapped file from being changed,
    // and fonts aren't rewritten in place elsewhere.
    let mapped = unsafe { memmap2::Mmap::map(&file) }.ok()?;
    Some(&**Box::leak(Box::new(mapped)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_script_gets_its_own_font() {
        assert_eq!(Script::of('僵'), Some(Script::Han));
        assert_eq!(Script::of('ゾ'), Some(Script::Kana));
        assert_eq!(Script::of('좀'), Some(Script::Hangul));
        assert_eq!(Script::of('क'), Some(Script::Indic));
        assert_eq!(Script::of('Z'), None);
        assert_eq!(Script::of('П'), None);
    }

    #[test]
    fn detects_scripts_needing_fallback() {
        assert!(needs_fallback("僵尸入侵100天"));
        assert!(needs_fallback("ゾンビ"));
        assert!(needs_fallback("좀비"));
        assert!(!needs_fallback("Zombie Invade — 100 Days"));
        assert!(!needs_fallback("Привет"));
    }
}

#[cfg(test)]
mod metric_tests {
    use super::*;

    #[test]
    fn segoe_letters_sit_low_in_their_line() {
        // Only where the font is (Windows).
        let Some(bytes) = first_readable(&[r"C:\Windows\Fonts\segoeui.ttf"]) else {
            return;
        };
        let low = letters_low_by(&bytes).expect("Segoe UI has the tables");
        // ascender 2210, descender 514, cap height 1434, 2048 units: about 0.064.
        assert!((low - 0.064).abs() < 0.01, "{low}");
    }

    #[test]
    fn not_a_font_is_left_alone() {
        assert_eq!(letters_low_by(b"not a font"), None);
        assert_eq!(letters_low_by(&[]), None);
    }
}
