//! Key bindings in `options.txt` across Minecraft versions: 1.13 and newer
//! name keys (`key_key.jump:key.keyboard.space`), older games store LWJGL 2
//! key codes (`key_key.jump:57`, mouse buttons as -100 + n). Default
//! settings copied from one kind of game are converted for the other.

use std::collections::BTreeMap;

use crate::loaders::util::compare_versions;

const KEY_PREFIX: &str = "key_";
const KEYBOARD: &str = "key.keyboard.";
const MOUSE: &str = "key.mouse.";
const UNKNOWN: &str = "key.keyboard.unknown";
/// Minecraft's code for mouse button n (LWJGL 2 games).
const MOUSE_OFFSET: i32 = -100;

/// Modern key name (after `key.keyboard.`) and its LWJGL 2 code.
const CODES: &[(&str, i32)] = &[
    ("escape", 1),
    ("1", 2),
    ("2", 3),
    ("3", 4),
    ("4", 5),
    ("5", 6),
    ("6", 7),
    ("7", 8),
    ("8", 9),
    ("9", 10),
    ("0", 11),
    ("minus", 12),
    ("equal", 13),
    ("backspace", 14),
    ("tab", 15),
    ("q", 16),
    ("w", 17),
    ("e", 18),
    ("r", 19),
    ("t", 20),
    ("y", 21),
    ("u", 22),
    ("i", 23),
    ("o", 24),
    ("p", 25),
    ("left.bracket", 26),
    ("right.bracket", 27),
    ("enter", 28),
    ("left.control", 29),
    ("a", 30),
    ("s", 31),
    ("d", 32),
    ("f", 33),
    ("g", 34),
    ("h", 35),
    ("j", 36),
    ("k", 37),
    ("l", 38),
    ("semicolon", 39),
    ("apostrophe", 40),
    ("grave.accent", 41),
    ("left.shift", 42),
    ("backslash", 43),
    ("z", 44),
    ("x", 45),
    ("c", 46),
    ("v", 47),
    ("b", 48),
    ("n", 49),
    ("m", 50),
    ("comma", 51),
    ("period", 52),
    ("slash", 53),
    ("right.shift", 54),
    ("keypad.multiply", 55),
    ("left.alt", 56),
    ("space", 57),
    ("caps.lock", 58),
    ("f1", 59),
    ("f2", 60),
    ("f3", 61),
    ("f4", 62),
    ("f5", 63),
    ("f6", 64),
    ("f7", 65),
    ("f8", 66),
    ("f9", 67),
    ("f10", 68),
    ("num.lock", 69),
    ("scroll.lock", 70),
    ("keypad.7", 71),
    ("keypad.8", 72),
    ("keypad.9", 73),
    ("keypad.subtract", 74),
    ("keypad.4", 75),
    ("keypad.5", 76),
    ("keypad.6", 77),
    ("keypad.add", 78),
    ("keypad.1", 79),
    ("keypad.2", 80),
    ("keypad.3", 81),
    ("keypad.0", 82),
    ("keypad.decimal", 83),
    ("f11", 87),
    ("f12", 88),
    ("f13", 100),
    ("f14", 101),
    ("f15", 102),
    ("keypad.enter", 156),
    ("right.control", 157),
    ("keypad.divide", 181),
    ("print.screen", 183),
    ("right.alt", 184),
    ("pause", 197),
    ("home", 199),
    ("up", 200),
    ("page.up", 201),
    ("left", 203),
    ("right", 205),
    ("end", 207),
    ("down", 208),
    ("page.down", 209),
    ("insert", 210),
    ("delete", 211),
    ("left.win", 219),
    ("right.win", 220),
    ("menu", 221),
];

/// Minecraft before 1.13 (LWJGL 2 key codes). `None` is the newest game.
pub fn uses_key_codes(game: Option<&str>) -> bool {
    game.is_some_and(|g| g.starts_with("1.") && compare_versions(g, "1.13").is_lt())
}

/// The settings as that game version stores them.
pub fn for_game(values: &BTreeMap<String, String>, game: Option<&str>) -> BTreeMap<String, String> {
    let codes = uses_key_codes(game);
    values
        .iter()
        .map(|(k, v)| {
            let v = if k.starts_with(KEY_PREFIX) {
                if codes { to_code(v) } else { to_name(v) }
            } else {
                v.clone()
            };
            (k.clone(), v)
        })
        .collect()
}

/// A key name as an LWJGL 2 code (codes are kept as they are).
fn to_code(value: &str) -> String {
    if value.parse::<i32>().is_ok() {
        return value.to_owned();
    }
    let code = if let Some(button) = value.strip_prefix(MOUSE) {
        match button {
            "left" => Some(MOUSE_OFFSET),
            "right" => Some(MOUSE_OFFSET + 1),
            "middle" => Some(MOUSE_OFFSET + 2),
            n => n.parse::<i32>().ok().map(|n| MOUSE_OFFSET + n - 1),
        }
    } else {
        value
            .strip_prefix(KEYBOARD)
            .and_then(|k| CODES.iter().find(|(name, _)| *name == k))
            .map(|(_, code)| *code)
    };
    code.unwrap_or(0).to_string()
}

/// An LWJGL 2 code as a key name (names are kept as they are).
fn to_name(value: &str) -> String {
    let Ok(code) = value.parse::<i32>() else {
        return value.to_owned();
    };
    if code < 0 {
        return match code - MOUSE_OFFSET {
            0 => format!("{MOUSE}left"),
            1 => format!("{MOUSE}right"),
            2 => format!("{MOUSE}middle"),
            n => format!("{MOUSE}{}", n + 1),
        };
    }
    CODES.iter().find(|(_, c)| *c == code).map_or_else(
        || UNKNOWN.to_owned(),
        |(name, _)| format!("{KEYBOARD}{name}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn old_games_get_key_codes() {
        let modern = opts(&[
            ("key_key.jump", "key.keyboard.space"),
            ("key_key.attack", "key.mouse.left"),
            ("key_key.sneak", "key.keyboard.left.shift"),
            ("key_key.smoothCamera", "key.keyboard.unknown"),
            ("soundCategory_music", "0.0"),
        ]);
        let old = for_game(&modern, Some("1.8.9"));
        assert_eq!(old["key_key.jump"], "57");
        assert_eq!(old["key_key.attack"], "-100");
        assert_eq!(old["key_key.sneak"], "42");
        assert_eq!(old["key_key.smoothCamera"], "0");
        assert_eq!(old["soundCategory_music"], "0.0");
    }

    #[test]
    fn new_games_get_key_names() {
        let old = opts(&[
            ("key_key.jump", "57"),
            ("key_key.use", "-99"),
            ("key_key.pickItem", "-98"),
            ("key_key.x", "0"),
        ]);
        let modern = for_game(&old, Some("1.20.1"));
        assert_eq!(modern["key_key.jump"], "key.keyboard.space");
        assert_eq!(modern["key_key.use"], "key.mouse.right");
        assert_eq!(modern["key_key.pickItem"], "key.mouse.middle");
        assert_eq!(modern["key_key.x"], UNKNOWN);
        assert_eq!(for_game(&old, None)["key_key.jump"], "key.keyboard.space");
    }

    #[test]
    fn version_boundary() {
        assert!(uses_key_codes(Some("1.12.2")));
        assert!(!uses_key_codes(Some("1.13")));
        assert!(!uses_key_codes(Some("26.3")));
        assert!(!uses_key_codes(None));
    }

    #[test]
    fn round_trip() {
        for (name, _) in CODES {
            let v = format!("{KEYBOARD}{name}");
            assert_eq!(to_name(&to_code(&v)), v);
        }
    }
}
