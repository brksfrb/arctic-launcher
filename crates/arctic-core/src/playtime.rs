//! Where the time goes, for the Stats page: per instance, per server, per singleplayer world, time
//! in a world or server versus in the menus, and how long the launcher itself was open. Kept in
//! `meta/playtime.json`.
//!
//! Two sources: the launcher times each game from its start to its exit (the instance's total and
//! session count), and the Arctic Client writes the stretches spent in a server, a world or the menus
//! to `arctic-playtime.jsonl` in the game folder, which is read and cleared when the game closes.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::Result;
use crate::storage::{DataDirs, load_json, save_json};

/// Shorter sessions than this (a game that failed to start) aren't counted.
pub const MIN_SESSION_SECS: u64 = 30;
/// The file the game writes its stretches to, in the game folder.
pub const GAME_LOG: &str = "arctic-playtime.jsonl";
/// A single stretch longer than this is a clock mishap, not play time.
const MAX_STRETCH_SECS: u64 = 24 * 60 * 60;
/// A game log with more lines than this is read only up to here.
const MAX_LINES: usize = 50_000;

/// Time spent in one server or one world.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Part {
    pub secs: u64,
    /// Unix seconds when it was last played.
    pub last: u64,
}

/// One instance's play time.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// The game open, from its start to its exit (as the launcher saw it).
    pub total_secs: u64,
    pub sessions: u32,
    /// Unix seconds when the last session ended.
    pub last_played: u64,
    /// Time in a world or server (the game's own record), and in menus.
    #[serde(default)]
    pub in_world_secs: u64,
    #[serde(default)]
    pub menu_secs: u64,
    #[serde(default)]
    pub servers: BTreeMap<String, Part>,
    #[serde(default)]
    pub worlds: BTreeMap<String, Part>,
}

/// Play time of every instance.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stats {
    instances: BTreeMap<String, Entry>,
    /// How long the launcher window was open, in all.
    #[serde(default)]
    launcher_secs: u64,
}

impl Stats {
    /// The saved stats (empty when there are none or they can't be read).
    pub fn load(dirs: &DataDirs) -> Self {
        load_json(&path(dirs)).ok().flatten().unwrap_or_default()
    }

    pub fn of(&self, instance_id: &str) -> Option<&Entry> {
        self.instances.get(instance_id)
    }

    /// Every instance with play time, longest first.
    pub fn instances(&self) -> Vec<(&str, &Entry)> {
        let mut all: Vec<(&str, &Entry)> = self
            .instances
            .iter()
            .map(|(id, entry)| (id.as_str(), entry))
            .collect();
        all.sort_by_key(|(_, e)| std::cmp::Reverse(e.total_secs));
        all
    }

    pub fn total_secs(&self) -> u64 {
        self.instances.values().map(|e| e.total_secs).sum()
    }

    pub fn sessions(&self) -> u32 {
        self.instances.values().map(|e| e.sessions).sum()
    }

    pub fn in_world_secs(&self) -> u64 {
        self.instances.values().map(|e| e.in_world_secs).sum()
    }

    pub fn menu_secs(&self) -> u64 {
        self.instances.values().map(|e| e.menu_secs).sum()
    }

    pub fn launcher_secs(&self) -> u64 {
        self.launcher_secs
    }

    /// The servers played most, over all instances (the same address in two instances adds up).
    pub fn servers(&self) -> Vec<(String, Part)> {
        merged(self.instances.values().map(|e| &e.servers))
    }

    /// The singleplayer worlds played most, over all instances.
    pub fn worlds(&self) -> Vec<(String, Part)> {
        merged(self.instances.values().map(|e| &e.worlds))
    }
}

fn merged<'a>(maps: impl Iterator<Item = &'a BTreeMap<String, Part>>) -> Vec<(String, Part)> {
    let mut all: BTreeMap<String, Part> = BTreeMap::new();
    for map in maps {
        for (name, part) in map {
            let sum = all.entry(name.clone()).or_default();
            sum.secs += part.secs;
            sum.last = sum.last.max(part.last);
        }
    }
    let mut list: Vec<(String, Part)> = all.into_iter().collect();
    list.sort_by_key(|(_, p)| std::cmp::Reverse(p.secs));
    list
}

fn path(dirs: &DataDirs) -> std::path::PathBuf {
    dirs.meta().join("playtime.json")
}

/// Add a session of `secs` that ended at `now` (Unix seconds) to an instance.
pub fn record(dirs: &DataDirs, instance_id: &str, secs: u64, now: u64) -> Result<()> {
    if secs < MIN_SESSION_SECS {
        return Ok(());
    }
    let mut stats = Stats::load(dirs);
    let entry = stats.instances.entry(instance_id.to_owned()).or_default();
    entry.total_secs += secs;
    entry.sessions += 1;
    entry.last_played = now;
    save_json(&path(dirs), &stats)
}

/// Add time the launcher window was open.
pub fn add_launcher_time(dirs: &DataDirs, secs: u64) -> Result<()> {
    if secs == 0 {
        return Ok(());
    }
    let mut stats = Stats::load(dirs);
    stats.launcher_secs += secs;
    save_json(&path(dirs), &stats)
}

/// Read what the game wrote in `game_dir` (the stretches in servers, worlds and menus), add it to
/// the instance and delete the file. Returns how many stretches were added.
pub fn ingest_game_log(dirs: &DataDirs, instance_id: &str, game_dir: &Path) -> Result<usize> {
    let file = game_dir.join(GAME_LOG);
    let Ok(text) = std::fs::read_to_string(&file) else {
        return Ok(0);
    };
    let mut stats = Stats::load(dirs);
    let entry = stats.instances.entry(instance_id.to_owned()).or_default();
    let mut added = 0;
    for line in text.lines().take(MAX_LINES) {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let (Some(kind), Some(start), Some(end)) = (
            value["k"].as_str(),
            value["s"].as_u64(),
            value["e"].as_u64(),
        ) else {
            continue;
        };
        let secs = end.saturating_sub(start);
        if secs == 0 || secs > MAX_STRETCH_SECS {
            continue;
        }
        let name = value["n"].as_str().unwrap_or_default();
        match kind {
            "menu" => entry.menu_secs += secs,
            "server" | "world" => {
                entry.in_world_secs += secs;
                let map = if kind == "server" {
                    &mut entry.servers
                } else {
                    &mut entry.worlds
                };
                if !name.is_empty() {
                    let part = map.entry(name.to_owned()).or_default();
                    part.secs += secs;
                    part.last = part.last.max(end);
                }
            }
            _ => continue,
        }
        added += 1;
    }
    save_json(&path(dirs), &stats)?;
    let _ = std::fs::remove_file(&file);
    Ok(added)
}

/// "3 h 20 min", "45 min", "under a minute".
pub fn format_duration(secs: u64) -> String {
    let minutes = secs / 60;
    match (minutes / 60, minutes % 60) {
        (0, 0) => "under a minute".to_owned(),
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    }
}

/// "today", "yesterday", "3 days ago" for a Unix time `then` seen at `now`.
pub fn ago(then: u64, now: u64) -> String {
    match now.saturating_sub(then) / 86_400 {
        0 => "today".to_owned(),
        1 => "yesterday".to_owned(),
        d => format!("{d} days ago"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sessions_add_up_and_short_ones_do_not_count() {
        let dir = tempfile::tempdir().unwrap();
        let dirs = DataDirs::new(dir.path());
        record(&dirs, "vanilla", 10, 100).unwrap();
        assert!(Stats::load(&dirs).of("vanilla").is_none());
        record(&dirs, "vanilla", 3600, 1_000).unwrap();
        record(&dirs, "vanilla", 1800, 2_000).unwrap();
        let stats = Stats::load(&dirs);
        let entry = stats.of("vanilla").unwrap();
        assert_eq!(entry.total_secs, 5400);
        assert_eq!(entry.sessions, 2);
        assert_eq!(entry.last_played, 2_000);
        assert!(stats.of("other").is_none());
        assert_eq!(stats.total_secs(), 5400);
    }

    #[test]
    fn the_games_stretches_are_sorted_into_servers_worlds_and_menus() {
        let dir = tempfile::tempdir().unwrap();
        let dirs = DataDirs::new(dir.path());
        let game = tempfile::tempdir().unwrap();
        std::fs::write(
            game.path().join(GAME_LOG),
            concat!(
                "{\"k\":\"menu\",\"n\":\"\",\"s\":100,\"e\":160}\n",
                "{\"k\":\"server\",\"n\":\"play.example.net\",\"s\":160,\"e\":1160}\n",
                "{\"k\":\"server\",\"n\":\"play.example.net\",\"s\":1160,\"e\":1460}\n",
                "{\"k\":\"world\",\"n\":\"Castle\",\"s\":1460,\"e\":1760}\n",
                "not json\n",
                "{\"k\":\"server\",\"n\":\"x\",\"s\":5,\"e\":3}\n",
                "{\"k\":\"other\",\"n\":\"x\",\"s\":1,\"e\":9}\n",
            ),
        )
        .unwrap();
        assert_eq!(ingest_game_log(&dirs, "vanilla", game.path()).unwrap(), 4);
        // Read once: the file is gone, so nothing is counted twice.
        assert!(!game.path().join(GAME_LOG).exists());
        assert_eq!(ingest_game_log(&dirs, "vanilla", game.path()).unwrap(), 0);
        let stats = Stats::load(&dirs);
        let entry = stats.of("vanilla").unwrap();
        assert_eq!(entry.menu_secs, 60);
        assert_eq!(entry.in_world_secs, 1300 + 300);
        assert_eq!(stats.servers()[0].0, "play.example.net");
        assert_eq!(stats.servers()[0].1.secs, 1300);
        assert_eq!(stats.worlds()[0].1.last, 1760);
    }

    #[test]
    fn launcher_time_adds_up() {
        let dir = tempfile::tempdir().unwrap();
        let dirs = DataDirs::new(dir.path());
        add_launcher_time(&dirs, 60).unwrap();
        add_launcher_time(&dirs, 120).unwrap();
        assert_eq!(Stats::load(&dirs).launcher_secs(), 180);
    }

    #[test]
    fn durations_read_naturally() {
        assert_eq!(format_duration(20), "under a minute");
        assert_eq!(format_duration(45 * 60), "45 min");
        assert_eq!(format_duration(2 * 3600), "2 h");
        assert_eq!(format_duration(3 * 3600 + 20 * 60), "3 h 20 min");
        assert_eq!(ago(1_000, 1_000 + 100), "today");
        assert_eq!(ago(0, 86_400), "yesterday");
        assert_eq!(ago(0, 3 * 86_400), "3 days ago");
    }
}
