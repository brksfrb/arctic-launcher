//! Telling a computer that keeps crashing in unrelated places from a game with one bug.
//!
//! A bug in a mod crashes at the same spot every time. When a game's native crashes (the ones that
//! print "Problematic frame") land in several different places within a few days, the cause is
//! usually the computer: unstable RAM, an overclock or XMP setting, or a graphics driver. The
//! launcher keeps a short history of those crashes and says so.

use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::Result;
use crate::storage::{DataDirs, load_json, save_json};

/// Only crashes this recent count.
const WINDOW_SECS: u64 = 7 * 24 * 60 * 60;
/// This many native crashes…
const MIN_CRASHES: usize = 3;
/// …in at least this many different places make the hint.
const MIN_PLACES: usize = 2;
/// The history keeps this many crashes.
const KEEP: usize = 30;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct History {
    crashes: Vec<Entry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Entry {
    at: u64,
    place: String,
}

/// Where a native crash happened, from the game log or crash text ("# Problematic frame:" and the
/// line after it), without the numbers that change from run to run. None if it isn't a native crash.
pub fn place(text: &str) -> Option<String> {
    static FRAME: OnceLock<Regex> = OnceLock::new();
    let re = FRAME.get_or_init(|| {
        Regex::new(r"(?m)^#\s*Problematic frame:\s*\r?\n#\s*(.+?)\s*$").expect("valid pattern")
    });
    let line = re.captures(text)?.get(1)?.as_str();
    // "J 33896 c2 com.example.Class.method(...)V (410 bytes) @ 0x..." -> "J c2 com.example.Class.method"
    let line = line.split('(').next().unwrap_or(line);
    let words: Vec<&str> = line
        .split_whitespace()
        .filter(|w| !w.chars().all(|c| c.is_ascii_digit()) && !w.starts_with("0x"))
        .collect();
    let place = words.join(" ");
    (!place.is_empty()).then(|| place.chars().take(120).collect())
}

/// Note a native crash at `place` and say whether the history now points at the computer.
pub fn record(dirs: &DataDirs, now: u64, place: &str) -> Result<Option<String>> {
    let path = dirs.meta().join("crash-history.json");
    let mut history: History = load_json(&path)?.unwrap_or_default();
    history
        .crashes
        .retain(|c| now.saturating_sub(c.at) < WINDOW_SECS);
    history.crashes.push(Entry {
        at: now,
        place: place.to_owned(),
    });
    let excess = history.crashes.len().saturating_sub(KEEP);
    history.crashes.drain(..excess);
    save_json(&path, &history)?;
    Ok(hint(&history.crashes))
}

fn hint(crashes: &[Entry]) -> Option<String> {
    let mut places: Vec<&str> = crashes.iter().map(|c| c.place.as_str()).collect();
    places.sort_unstable();
    places.dedup();
    if crashes.len() < MIN_CRASHES || places.len() < MIN_PLACES {
        return None;
    }
    Some(format!(
        "The game has crashed {} times in the last week, at {} different places in native code. \
         One bug in a mod would crash in the same place each time, so this points at the computer: \
         unstable RAM, an overclock or XMP setting, or the graphics driver. Try running the memory \
         at its default speed and run a memory test (MemTest86); then update the graphics driver.",
        crashes.len(),
        places.len()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(at: u64, place: &str) -> Entry {
        Entry {
            at,
            place: place.to_owned(),
        }
    }

    #[test]
    fn the_crashing_frame_loses_its_numbers() {
        let log = "# A fatal error has been detected\n#\n# Problematic frame:\n# J 33896 c2 com.arcticlauncher.client.hud.Keystrokes.render(Lcom/arcticlauncher/client/gfx/Gfx;)V (410 bytes) @ 0x00000158231af304 [0x0+0xe04]\n#\n";
        assert_eq!(
            place(log).as_deref(),
            Some("J c2 com.arcticlauncher.client.hud.Keystrokes.render")
        );
        assert_eq!(place("an ordinary Java exception"), None);
    }

    #[test]
    fn one_place_repeated_is_a_bug_not_the_computer() {
        let same = vec![
            entry(1, "J c2 a.B.c"),
            entry(2, "J c2 a.B.c"),
            entry(3, "J c2 a.B.c"),
        ];
        assert_eq!(hint(&same), None);
    }

    #[test]
    fn several_places_point_at_the_computer() {
        let spread = vec![
            entry(1, "J c2 a.B.c"),
            entry(2, "V [jvm.dll+0x3b72d3]"),
            entry(3, "C [nvoglv64.dll+0x10af62d]"),
        ];
        let text = hint(&spread).unwrap();
        assert!(text.contains("3 times") && text.contains("3 different places"));
        // Too few crashes say nothing.
        assert_eq!(hint(&spread[..2]), None);
    }

    #[test]
    fn old_crashes_are_forgotten() {
        let dir = tempfile::tempdir().unwrap();
        let dirs = DataDirs::new(dir.path());
        let day = 24 * 60 * 60;
        assert_eq!(record(&dirs, day, "a").unwrap(), None);
        assert_eq!(record(&dirs, 5 * day, "b").unwrap(), None);
        // The third crash comes after more than a week since the first, which is forgotten: with it
        // there would be three crashes in three places, without it only two.
        assert_eq!(record(&dirs, 9 * day, "c").unwrap(), None);
        assert!(record(&dirs, 9 * day + 1, "d").unwrap().is_some());
    }
}
