//! The launcher's text in the player's language.
//!
//! Every piece of text the launcher shows goes through [`text`] (the `t!` macro), [`format`]
//! (`tf!`, with `{name}` placeholders) or [`plural`] (`tn!`, with `{n}`). English is the source:
//! `locales/en/<area>.json` holds every key, and `locales/<lang>/<area>.json` the translations,
//! all flat `"key": "text"` objects embedded at build time (see build.rs). Text missing from a
//! language falls back to English, and a key missing everywhere shows as itself, so nothing is
//! ever blank. See docs/i18n.md.

use std::collections::HashMap;
use std::fmt::Display;
use std::sync::{Mutex, OnceLock, PoisonError, RwLock};

// `LOCALE_FILES`: (language, file name, contents) of every locales/<language>/<file>.json.
include!(concat!(env!("OUT_DIR"), "/locales.rs"));

/// The setting that means "the language this computer uses".
pub const SYSTEM: &str = "system";

/// One language the launcher speaks.
pub struct Language {
    /// Two-letter code, also the folder under `locales/`.
    pub code: &'static str,
    /// Its name for itself, shown in the picker.
    pub name: &'static str,
    /// A character only its script has, so that script's font is loaded before the text
    /// shows (empty for Latin and Cyrillic, which the interface font covers).
    sample: &'static str,
}

/// Every language the launcher speaks, in the order the picker lists them.
pub const LANGUAGES: &[Language] = &[
    Language {
        code: "en",
        name: "English",
        sample: "",
    },
    Language {
        code: "tr",
        name: "Türkçe",
        sample: "",
    },
    Language {
        code: "de",
        name: "Deutsch",
        sample: "",
    },
    Language {
        code: "fr",
        name: "Français",
        sample: "",
    },
    Language {
        code: "es",
        name: "Español",
        sample: "",
    },
    Language {
        code: "pt",
        name: "Português (Brasil)",
        sample: "",
    },
    Language {
        code: "it",
        name: "Italiano",
        sample: "",
    },
    Language {
        code: "nl",
        name: "Nederlands",
        sample: "",
    },
    Language {
        code: "pl",
        name: "Polski",
        sample: "",
    },
    Language {
        code: "ru",
        name: "Русский",
        sample: "",
    },
    Language {
        code: "uk",
        name: "Українська",
        sample: "",
    },
    Language {
        code: "zh",
        name: "简体中文",
        sample: "中",
    },
    Language {
        code: "ja",
        name: "日本語",
        sample: "あ",
    },
    Language {
        code: "ko",
        name: "한국어",
        sample: "한",
    },
];

/// One language's text, loaded once and kept for the rest of the run.
struct Catalog {
    code: &'static str,
    strings: HashMap<&'static str, &'static str>,
}

static ENGLISH: OnceLock<&'static Catalog> = OnceLock::new();
static CURRENT: RwLock<Option<&'static Catalog>> = RwLock::new(None);
/// Catalogs loaded so far (each language is parsed at most once).
static LOADED: Mutex<Vec<&'static Catalog>> = Mutex::new(Vec::new());

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

fn load(code: &str) -> &'static Catalog {
    let mut loaded = LOADED.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(catalog) = loaded.iter().find(|c| c.code == code) {
        return catalog;
    }
    let mut strings = HashMap::new();
    for (lang, file, json) in LOCALE_FILES {
        if *lang != code {
            continue;
        }
        match serde_json::from_str::<HashMap<String, String>>(json) {
            Ok(map) => {
                for (key, value) in map {
                    strings.insert(leak(key), leak(value));
                }
            }
            Err(e) => log::error!("locales/{lang}/{file}: {e}"),
        }
    }
    let code = match LANGUAGES.iter().find(|l| l.code == code) {
        Some(language) => language.code,
        None => leak(code.to_owned()),
    };
    let catalog: &'static Catalog = Box::leak(Box::new(Catalog { code, strings }));
    loaded.push(catalog);
    catalog
}

fn english() -> &'static Catalog {
    ENGLISH.get_or_init(|| load("en"))
}

fn current() -> Option<&'static Catalog> {
    *CURRENT.read().unwrap_or_else(PoisonError::into_inner)
}

/// The language code a setting means: a supported code (any region or case, "pt_BR", "tr-TR"),
/// or `None` for one the launcher doesn't speak.
pub fn supported(code: &str) -> Option<&'static str> {
    let lower = code.trim().to_ascii_lowercase();
    let primary = lower.split(['-', '_', '.', '@']).next().unwrap_or("");
    LANGUAGES.iter().find(|l| l.code == primary).map(|l| l.code)
}

/// Use this language from now on: a code from [`LANGUAGES`], or [`SYSTEM`].
pub fn set_language(setting: &str) {
    let code = if setting.is_empty() || setting == SYSTEM {
        system_language()
    } else {
        supported(setting).unwrap_or("en")
    };
    let catalog = load(code);
    *CURRENT.write().unwrap_or_else(PoisonError::into_inner) = Some(catalog);
}

/// The language in use ("en" until [`set_language`] is called).
pub fn current_code() -> &'static str {
    current().map_or("en", |c| c.code)
}

/// A language's own name for itself, by code ("tr" → "Türkçe"); English for an unknown code.
pub fn name_of(code: &str) -> &'static str {
    let wanted = supported(code).unwrap_or("en");
    LANGUAGES
        .iter()
        .find(|l| l.code == wanted)
        .map_or("English", |l| l.name)
}

/// The language this computer uses, if the launcher speaks it; English otherwise.
pub fn system_language() -> &'static str {
    system_locale()
        .and_then(|locale| supported(&locale))
        .unwrap_or("en")
}

#[cfg(windows)]
fn system_locale() -> Option<String> {
    use windows_sys::Win32::Globalization::GetUserDefaultLocaleName;
    // LOCALE_NAME_MAX_LENGTH in the Windows headers.
    const LOCALE_NAME_MAX_LENGTH: u32 = 85;
    let mut name = [0u16; LOCALE_NAME_MAX_LENGTH as usize];
    // SAFETY: `name` holds LOCALE_NAME_MAX_LENGTH UTF-16 units, the most the call writes.
    let len = unsafe { GetUserDefaultLocaleName(name.as_mut_ptr(), name.len() as i32) };
    // The length counts the terminating zero.
    if len <= 1 {
        return None;
    }
    Some(String::from_utf16_lossy(&name[..len as usize - 1]))
}

#[cfg(not(windows))]
fn system_locale() -> Option<String> {
    let from_env = ["LC_ALL", "LC_MESSAGES", "LANG"].iter().find_map(|var| {
        std::env::var(var)
            .ok()
            .filter(|v| !v.is_empty() && v != "C" && v != "POSIX")
    });
    if from_env.is_some() {
        return from_env;
    }
    // Apps opened from the Finder get no LANG; the system's own setting does.
    if cfg!(target_os = "macos") {
        let out = std::process::Command::new("defaults")
            .args(["read", "-g", "AppleLocale"])
            .output()
            .ok()?;
        let locale = String::from_utf8_lossy(&out.stdout).trim().to_owned();
        if !locale.is_empty() {
            return Some(locale);
        }
    }
    None
}

/// Load the font the current language's script needs, if the interface font lacks it.
pub fn ensure_fonts(ctx: &eframe::egui::Context) {
    let code = current_code();
    if let Some(language) = LANGUAGES.iter().find(|l| l.code == code)
        && !language.sample.is_empty()
    {
        crate::fonts::ensure_for(ctx, language.sample);
    }
}

/// Text for `key` in the current language, else in English, else the key itself.
pub fn text(key: &'static str) -> &'static str {
    lookup(key).unwrap_or(key)
}

/// Text for a key built at run time (`None` when no language has it).
pub fn lookup(key: &str) -> Option<&'static str> {
    if let Some(catalog) = current()
        && let Some(s) = catalog.strings.get(key)
    {
        return Some(s);
    }
    english().strings.get(key).copied()
}

/// Text for `key` with its `{name}` placeholders filled from `args`.
pub fn format(key: &'static str, args: &[(&str, &dyn Display)]) -> String {
    substitute(text(key), args)
}

/// `{name}` placeholders in `template` replaced by the matching value; unknown ones stay.
fn substitute(template: &str, args: &[(&str, &dyn Display)]) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find('}') else {
            out.push_str(&rest[start..]);
            return out;
        };
        let name = &after[..end];
        match args.iter().find(|(n, _)| *n == name) {
            Some((_, value)) => {
                let _ = write!(out, "{value}");
            }
            None => {
                out.push('{');
                out.push_str(name);
                out.push('}');
            }
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

/// Text for a count: `key.one`, `key.few`, `key.many` or `key.other` by the language's rules,
/// with `{n}` (and any `args`) filled in. A language without the exact form uses `.other`.
#[allow(dead_code)] // Part of the API: used as text with counts arrives.
pub fn plural(key: &'static str, n: i64, args: &[(&str, &dyn Display)]) -> String {
    let code = current_code();
    let form = plural_form(code, n);
    let exact = format!("{key}.{form}");
    let other = format!("{key}.other");
    let template = lookup(&exact)
        .or_else(|| lookup(&other))
        .unwrap_or(text(key));
    let count: &dyn Display = &n;
    let mut all: Vec<(&str, &dyn Display)> = Vec::with_capacity(args.len() + 1);
    all.push(("n", count));
    all.extend_from_slice(args);
    substitute(template, &all)
}

/// The CLDR plural category of `n` in a language.
#[allow(dead_code)] // Part of the API: used as text with counts arrives.
fn plural_form(code: &str, n: i64) -> &'static str {
    let n = n.abs();
    let (tens, hundreds) = (n % 10, n % 100);
    let few = (2..=4).contains(&tens) && !(12..=14).contains(&hundreds);
    match code {
        "ru" | "uk" => {
            if tens == 1 && hundreds != 11 {
                "one"
            } else if few {
                "few"
            } else {
                "many"
            }
        }
        "pl" => {
            if n == 1 {
                "one"
            } else if few {
                "few"
            } else {
                "many"
            }
        }
        "fr" | "pt" => {
            if n <= 1 {
                "one"
            } else {
                "other"
            }
        }
        "zh" | "ja" | "ko" => "other",
        _ => {
            if n == 1 {
                "one"
            } else {
                "other"
            }
        }
    }
}

/// Text that comes from arctic-core (progress stages like "Downloading game files", a few
/// messages) in the current language, when the catalog has it under `core.<the English text>`;
/// otherwise as it came.
#[allow(dead_code)] // Part of the API: used as text with counts arrives.
pub fn core_text(english: &str) -> String {
    let key = format!("core.{english}");
    match lookup(&key) {
        Some(s) => s.to_owned(),
        None => english.to_owned(),
    }
}

/// Text for a key: `t!("settings.title")`. Returns `&'static str`, so it goes wherever a
/// string literal went.
#[macro_export]
macro_rules! t {
    ($key:expr) => {
        $crate::i18n::text($key)
    };
}

/// Text with placeholders filled: `tf!("play.ready_in", seconds = 3)` for
/// `"Ready in {seconds} s"`. Returns a `String`.
#[macro_export]
macro_rules! tf {
    ($key:expr $(, $name:ident = $value:expr)* $(,)?) => {
        $crate::i18n::format(
            $key,
            &[$((stringify!($name), &$value as &dyn ::std::fmt::Display)),*],
        )
    };
}

/// Text for a count: `tn!("instances.mods_count", mods.len())` picks `.one`/`.few`/`.many`/
/// `.other` and fills `{n}`; more placeholders follow as in `tf!`. Returns a `String`.
#[macro_export]
macro_rules! tn {
    ($key:expr, $n:expr $(, $name:ident = $value:expr)* $(,)?) => {
        $crate::i18n::plural(
            $key,
            $n as i64,
            &[$((stringify!($name), &$value as &dyn ::std::fmt::Display)),*],
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders_are_filled_and_unknown_ones_kept() {
        let name = "Steve";
        let args: &[(&str, &dyn Display)] = &[("name", &name), ("n", &3)];
        assert_eq!(
            substitute("Hi {name}, {n} left {x}", args),
            "Hi Steve, 3 left {x}"
        );
        assert_eq!(substitute("no braces", args), "no braces");
        assert_eq!(substitute("open {brace", args), "open {brace");
        assert_eq!(substitute("{name}{name}", args), "SteveSteve");
    }

    #[test]
    fn plural_forms_follow_each_language() {
        assert_eq!(plural_form("en", 1), "one");
        assert_eq!(plural_form("en", 0), "other");
        assert_eq!(plural_form("en", 5), "other");
        assert_eq!(plural_form("fr", 0), "one");
        assert_eq!(plural_form("fr", 2), "other");
        assert_eq!(plural_form("ru", 1), "one");
        assert_eq!(plural_form("ru", 21), "one");
        assert_eq!(plural_form("ru", 11), "many");
        assert_eq!(plural_form("ru", 3), "few");
        assert_eq!(plural_form("ru", 12), "many");
        assert_eq!(plural_form("ru", 25), "many");
        assert_eq!(plural_form("pl", 1), "one");
        assert_eq!(plural_form("pl", 21), "many");
        assert_eq!(plural_form("pl", 22), "few");
        assert_eq!(plural_form("zh", 1), "other");
        assert_eq!(plural_form("ja", 2), "other");
        assert_eq!(plural_form("ko", 1), "other");
    }

    #[test]
    fn language_codes_are_recognised_in_any_spelling() {
        assert_eq!(supported("tr"), Some("tr"));
        assert_eq!(supported("tr-TR"), Some("tr"));
        assert_eq!(supported("pt_BR.UTF-8"), Some("pt"));
        assert_eq!(supported("ZH-Hans-CN"), Some("zh"));
        assert_eq!(supported("xx"), None);
        assert_eq!(supported(""), None);
    }

    #[test]
    fn english_is_the_fallback_and_keys_the_last_resort() {
        let english = english();
        assert!(!english.strings.is_empty(), "locales/en is embedded");
        assert_eq!(text("no.such.key.ever"), "no.such.key.ever");
        assert_eq!(lookup("no.such.key.ever"), None);
        assert_eq!(core_text("Not a stage"), "Not a stage");
    }

    /// Every translation has a key English has, with the same placeholders, and no language
    /// repeats a key across its files.
    #[test]
    fn translations_match_english() {
        fn placeholders(s: &str) -> Vec<&str> {
            let mut out: Vec<&str> = s
                .split('{')
                .skip(1)
                .filter_map(|part| part.split('}').next())
                .collect();
            out.sort_unstable();
            out
        }
        let english = english();
        let mut problems = Vec::new();
        let mut seen: HashMap<&str, Vec<&str>> = HashMap::new();
        for (lang, file, json) in LOCALE_FILES {
            let map: HashMap<String, String> = serde_json::from_str(json)
                .unwrap_or_else(|e| panic!("locales/{lang}/{file} is not a flat JSON object: {e}"));
            let keys = seen.entry(lang).or_default();
            for (key, value) in &map {
                let key_static = leak(key.clone());
                if keys.contains(&key_static) {
                    problems.push(format!("{lang}/{file}: {key} appears twice in {lang}"));
                }
                keys.push(key_static);
                if value.trim().is_empty() {
                    problems.push(format!("{lang}/{file}: {key} is empty"));
                }
                if *lang == "en" {
                    continue;
                }
                // A plural form ("key.few") a language needs and English doesn't is fine when
                // English has "key.other"; `{n}` may appear in any form.
                let (base, form) = key.rsplit_once('.').unwrap_or((key.as_str(), ""));
                let is_form = matches!(form, "zero" | "one" | "two" | "few" | "many" | "other");
                let other = format!("{base}.other");
                let source = english.strings.get(key.as_str()).copied().or(if is_form {
                    english.strings.get(other.as_str()).copied()
                } else {
                    None
                });
                fn strip_n(mut v: Vec<&str>, is_form: bool) -> Vec<&str> {
                    if is_form {
                        v.retain(|p| *p != "n");
                    }
                    v
                }
                match source {
                    None => problems.push(format!("{lang}/{file}: {key} is not in English")),
                    Some(source)
                        if strip_n(placeholders(source), is_form)
                            != strip_n(placeholders(value), is_form) =>
                    {
                        problems.push(format!(
                            "{lang}/{file}: {key} has other placeholders than English"
                        ))
                    }
                    Some(_) => {}
                }
            }
        }
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }

    #[test]
    fn every_language_in_the_picker_has_text() {
        for language in LANGUAGES {
            let has_files = LOCALE_FILES
                .iter()
                .any(|(lang, _, _)| *lang == language.code);
            assert!(has_files, "locales/{} has no files", language.code);
        }
    }
}
