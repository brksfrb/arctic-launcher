//! Prefer platform UI fonts. Small embedded faces keep text available even
//! without system fonts, and large system script fonts are mapped on demand.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use eframe::egui::{self, FontData, FontFamily};
use eframe::epaint::text::{FontInsert, FontPriority, InsertFontFamily};
use skrifa::{FontRef, MetadataProvider};

/// The font family for headings (semibold); plain text uses Proportional.
pub const HEADING: &str = "heading";

type Faces = &'static [(&'static str, u32)];

macro_rules! bundled {
    ($file:literal) => {
        bundled_font(include_bytes!(concat!(
            env!("OUT_DIR"),
            "/",
            $file,
            ".lzma"
        )))
    };
}

fn bundled_font(mut bytes: &[u8]) -> FontData {
    let mut plain = Vec::new();
    lzma_rs::lzma_decompress(&mut bytes, &mut plain).expect("decode embedded font");
    FontData::from_owned(plain)
}

fn ui_fonts() -> (Faces, Faces, Faces) {
    if cfg!(windows) {
        (
            &[("segoeui.ttf", 0), ("SegUIVar.ttf", 0)],
            &[("seguisb.ttf", 0), ("segoeuib.ttf", 0)],
            &[("consola.ttf", 0)],
        )
    } else if cfg!(target_os = "macos") {
        (
            &[
                ("/System/Library/Fonts/Helvetica.ttc", 0),
                ("/System/Library/Fonts/Supplemental/Arial.ttf", 0),
            ],
            &[("/System/Library/Fonts/Supplemental/Arial Bold.ttf", 0)], // i18n-ignore: path
            &[("/System/Library/Fonts/Menlo.ttc", 0)],
        )
    } else {
        (
            &[
                ("/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf", 0),
                ("/usr/share/fonts/noto/NotoSans-Regular.ttf", 0),
                ("/usr/share/fonts/google-noto-vf/NotoSans[wght].ttf", 0),
                ("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 0),
                ("/usr/share/fonts/dejavu/DejaVuSans.ttf", 0),
                ("/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf", 0),
            ],
            &[
                ("/usr/share/fonts/truetype/noto/NotoSans-SemiBold.ttf", 0),
                ("/usr/share/fonts/noto/NotoSans-SemiBold.ttf", 0),
                ("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf", 0),
                ("/usr/share/fonts/dejavu/DejaVuSans-Bold.ttf", 0),
                ("/usr/share/fonts/dejavu-sans-fonts/DejaVuSans-Bold.ttf", 0),
            ],
            &[
                ("/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf", 0),
                ("/usr/share/fonts/dejavu/DejaVuSansMono.ttf", 0),
                (
                    "/usr/share/fonts/dejavu-sans-mono-fonts/DejaVuSansMono.ttf",
                    0,
                ),
                ("/usr/share/fonts/truetype/noto/NotoSansMono-Regular.ttf", 0),
            ],
        )
    }
}

fn symbol_fonts() -> Faces {
    if cfg!(windows) {
        &[("seguisym.ttf", 0)]
    } else if cfg!(target_os = "macos") {
        &[
            ("/System/Library/Fonts/Supplemental/Arial Unicode.ttf", 0), // i18n-ignore: path
            ("/Library/Fonts/Arial Unicode.ttf", 0),                     // i18n-ignore: path
        ]
    } else {
        &[
            (
                "/usr/share/fonts/truetype/noto/NotoSansSymbols2-Regular.ttf",
                0,
            ),
            ("/usr/share/fonts/noto/NotoSansSymbols2-Regular.ttf", 0),
            (
                "/usr/share/fonts/google-noto-sans-symbols2-fonts/NotoSansSymbols2-Regular.ttf",
                0,
            ),
        ]
    }
}

fn emoji_fonts() -> Faces {
    if cfg!(windows) {
        &[("seguiemj.ttf", 0)]
    } else if cfg!(target_os = "macos") {
        // Apple Color Emoji is bitmap-only. Use the outline fallbacks.
        &[]
    } else {
        &[
            ("/usr/share/fonts/truetype/noto/NotoEmoji-Regular.ttf", 0),
            ("/usr/share/fonts/noto/NotoEmoji-Regular.ttf", 0),
            ("/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf", 0),
            ("/usr/share/fonts/noto/NotoColorEmoji.ttf", 0),
        ]
    }
}

pub fn install_ui(ctx: &egui::Context) {
    let (regular, semibold, monospace) = ui_fonts();
    ctx.set_fonts(definitions(
        regular,
        semibold,
        monospace,
        symbol_fonts(),
        emoji_fonts(),
    ));
}

fn definitions(
    regular: Faces,
    semibold: Faces,
    monospace: Faces,
    symbols: Faces,
    emoji: Faces,
) -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::empty();
    let mut proportional = Vec::new();
    let mut mono = vec!["ui-mono".into()];
    fonts.font_data.insert(
        "ui-mono".into(),
        Arc::new(system_font(monospace).unwrap_or_else(|| bundled!("DejaVuSansMono-Subset.ttf"))),
    );
    for (name, data) in [
        ("ui", system_font(regular).map(centered)),
        ("ui-symbols", system_font(symbols)),
        ("text-fallback", Some(bundled!("DejaVuSans-Subset.ttf"))),
        // Keep the outline emoji that egui drew before, including on systems
        // where the native emoji glyphs only have color layers.
        (
            "emoji-fallback",
            Some(bundled!("NotoEmoji-Regular.ttf").tweak(egui::FontTweak {
                scale: 0.81,
                ..Default::default()
            })),
        ),
        (
            "icon-fallback",
            Some(bundled!("emoji-icon-font.ttf").tweak(egui::FontTweak {
                scale: 0.90,
                ..Default::default()
            })),
        ),
        ("ui-emoji", system_font(emoji)),
    ] {
        if let Some(data) = data {
            fonts.font_data.insert(name.into(), Arc::new(data));
            proportional.push(name.into());
        }
    }
    mono.extend(proportional.iter().cloned());
    let mut heading = proportional.clone();
    if let Some(data) = system_font(semibold).map(centered) {
        fonts.font_data.insert("ui-semibold".into(), Arc::new(data));
        heading.insert(0, "ui-semibold".into());
    }
    fonts
        .families
        .insert(FontFamily::Proportional, proportional);
    fonts.families.insert(FontFamily::Monospace, mono);
    fonts
        .families
        .insert(FontFamily::Name(HEADING.into()), heading);
    fonts
}

/// Nudge glyphs to center the letters vertically in their line box.
fn centered(mut data: FontData) -> FontData {
    if data.index == 0
        && let Some(shift) = letters_low_by(data.font.as_ref())
    {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Script {
    Han,
    Kana,
    Hangul,
    Indic,
}

impl Script {
    fn of(c: char) -> Option<Self> {
        match c as u32 {
            0x0900..=0x0DFF => Some(Self::Indic),
            0x3040..=0x30FF | 0x31F0..=0x31FF => Some(Self::Kana),
            0x1100..=0x11FF | 0x3130..=0x318F | 0xAC00..=0xD7AF => Some(Self::Hangul),
            0x3000..=0x9FFF | 0xF900..=0xFAFF | 0xFF00..=0xFFEF | 0x20000..=0x323AF => {
                Some(Self::Han)
            }
            _ => None,
        }
    }

    fn windows_fonts(self) -> Faces {
        match self {
            Self::Han => &[("msyh.ttc", 0), ("simsun.ttc", 0)],
            Self::Kana => &[("YuGothM.ttc", 0), ("msgothic.ttc", 0)],
            Self::Hangul => &[("malgun.ttf", 0)],
            Self::Indic => &[("Nirmala.ttc", 0), ("Nirmala.ttf", 0)],
        }
    }
}

static CJK_REQUESTED: AtomicBool = AtomicBool::new(false);
static INDIC_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Map CJK or Indic fonts in the background the first time those scripts are shown.
pub fn ensure_for(ctx: &egui::Context, text: &str) {
    let mut cjk = None;
    let mut indic = false;
    for script in text.chars().filter_map(Script::of) {
        if script == Script::Indic {
            indic = true;
        } else if cjk.is_none() {
            cjk = Some(script);
        }
    }
    let cjk = cjk.filter(|_| !CJK_REQUESTED.swap(true, Ordering::Relaxed));
    let indic = indic && !INDIC_REQUESTED.swap(true, Ordering::Relaxed);
    if cjk.is_none() && !indic {
        return;
    }

    let ctx = ctx.clone();
    std::thread::spawn(move || {
        if let Some(preferred) = cjk {
            add_cjk(&ctx, preferred);
        }
        if indic {
            let paths: Faces = if cfg!(windows) {
                Script::Indic.windows_fonts()
            } else if cfg!(target_os = "macos") {
                &[
                    ("/System/Library/Fonts/Kohinoor.ttc", 0),
                    (
                        "/System/Library/Fonts/Supplemental/Devanagari Sangam MN.ttc", // i18n-ignore: path
                        0,
                    ),
                ]
            } else {
                &[(
                    "/usr/share/fonts/truetype/noto/NotoSansDevanagari-Regular.ttf",
                    0,
                )]
            };
            if let Some(font) = system_font(paths) {
                add_fallback(&ctx, "system-indic", font);
            }
        }
        ctx.request_repaint();
    });
}

fn add_cjk(ctx: &egui::Context, preferred: Script) {
    if cfg!(windows) {
        for script in std::iter::once(preferred).chain(
            [Script::Han, Script::Kana, Script::Hangul]
                .into_iter()
                .filter(|&script| script != preferred),
        ) {
            if let Some(font) = system_font(script.windows_fonts()) {
                add_fallback(ctx, &format!("system-{script:?}"), font);
            }
        }
    } else if cfg!(target_os = "macos") {
        let faces: &[(&str, Faces)] = &[
            (
                "system-kana",
                &[
                    ("/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc", 0),
                    ("/System/Library/Fonts/ヒラギノ角ゴシック W4.ttc", 0),
                ],
            ),
            (
                "system-hangul",
                &[("/System/Library/Fonts/AppleSDGothicNeo.ttc", 0)],
            ),
            ("system-han", &[("/System/Library/Fonts/PingFang.ttc", 0)]),
        ];
        for &(name, paths) in faces {
            if let Some(font) = system_font(paths) {
                add_fallback(ctx, name, font);
            }
        }
    } else {
        // Noto's collection order is JP, KR, SC, TC, HK. Pick the first
        // requested regional face, which also covers the other two scripts.
        let index = match preferred {
            Script::Kana => 0,
            Script::Hangul => 1,
            _ => 2,
        };
        let paths = [
            (
                "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
                index,
            ),
            ("/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc", index),
            (
                "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
                index,
            ),
            ("/usr/share/fonts/truetype/wqy/wqy-microhei.ttc", 0),
        ];
        if let Some(font) = system_font(&paths) {
            add_fallback(ctx, "system-cjk", font);
        }
    }
    add_fallback(ctx, "cjk-fallback", bundled!("NotoSansCJKsc-Subset.otf"));
}

fn add_fallback(ctx: &egui::Context, name: &str, data: FontData) {
    let families = [
        FontFamily::Proportional,
        FontFamily::Monospace,
        FontFamily::Name(HEADING.into()),
    ]
    .into_iter()
    .map(|family| InsertFontFamily {
        family,
        priority: FontPriority::Lowest,
    })
    .collect();
    ctx.add_font(FontInsert::new(name, data, families));
}

fn system_font(paths: &[(&str, u32)]) -> Option<FontData> {
    paths.iter().find_map(|&(path, index)| {
        let path = if cfg!(windows) {
            std::env::var_os("WINDIR") // i18n-ignore: environment variable
                .map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from)
                .join("Fonts")
                .join(path)
        } else {
            PathBuf::from(path)
        };
        let file = std::fs::File::open(path).ok()?;
        // SAFETY: a system font, mapped read-only. System fonts are replaced
        // as files on updates; Windows also prevents writes to a mapped file.
        let mapped = unsafe { memmap2::Mmap::map(&file) }.ok()?;
        let face = FontRef::from_index(&mapped, index).ok()?;
        // egui 0.36 rasterizes outline glyphs. Skip bitmap-only faces.
        face.outline_glyphs().format()?;
        let mut data = FontData::from_static(Box::leak(Box::new(mapped)));
        data.index = index;
        Some(data)
    })
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
        assert!("僵尸入侵100天".chars().any(|c| Script::of(c).is_some()));
        assert!("ゾンビ".chars().any(|c| Script::of(c).is_some()));
        assert!("좀비".chars().any(|c| Script::of(c).is_some()));
        assert!(
            !"Zombie Invade — 100 Days"
                .chars()
                .any(|c| Script::of(c).is_some())
        );
        assert!(!"Привет".chars().any(|c| Script::of(c).is_some()));
    }

    #[test]
    fn bundled_fonts_render_without_system_fonts() {
        let ctx = egui::Context::default();
        ctx.set_fonts(definitions(&[], &[], &[], &[], &[]));
        add_fallback(&ctx, "test-cjk", bundled!("NotoSansCJKsc-Subset.otf"));
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            // i18n-ignore: exercise text rendering across the supported scripts.
            let text = "English Türkçe ı ş ğ İ Русский 简体中文 日本語 한국어 ✓ ⚙ ▶";
            for id in [
                egui::FontId::proportional(16.0),
                egui::FontId::monospace(16.0),
                heading(16.0),
            ] {
                assert!(ui.fonts_mut(|fonts| fonts.has_glyphs(&id, text)));
                ui.label(egui::RichText::new(text).font(id));
            }
        });
        assert!(!output.shapes.is_empty());
        output.textures_delta.clear();
    }
}

#[cfg(test)]
mod metric_tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn segoe_letters_sit_low_in_their_line() {
        // Only where the font is (Windows).
        let Some(data) = system_font(&[("segoeui.ttf", 0)]) else {
            return;
        };
        let low = letters_low_by(data.font.as_ref()).expect("Segoe UI has the tables");
        // ascender 2210, descender 514, cap height 1434, 2048 units: about 0.064.
        assert!((low - 0.064).abs() < 0.01, "{low}");
    }

    #[test]
    fn not_a_font_is_left_alone() {
        assert_eq!(letters_low_by(b"not a font"), None);
        assert_eq!(letters_low_by(&[]), None);
    }
}
