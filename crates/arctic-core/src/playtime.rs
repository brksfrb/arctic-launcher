//! How long each instance has been played: total time, sessions and the last time, kept in
//! `meta/playtime.json` and shown on the Play page. Measured by the launcher from the game's start
//! to its exit, so it counts only games the launcher started.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::Result;
use crate::storage::{DataDirs, load_json, save_json};

/// Shorter sessions than this (a game that failed to start) aren't counted.
pub const MIN_SESSION_SECS: u64 = 30;

/// One instance's play time.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub total_secs: u64,
    pub sessions: u32,
    /// Unix seconds when the last session ended.
    pub last_played: u64,
}

/// Play time of every instance.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stats {
    instances: BTreeMap<String, Entry>,
}

impl Stats {
    /// The saved stats (empty when there are none or they can't be read).
    pub fn load(dirs: &DataDirs) -> Self {
        load_json(&path(dirs)).ok().flatten().unwrap_or_default()
    }

    pub fn of(&self, instance_id: &str) -> Option<&Entry> {
        self.instances.get(instance_id)
    }
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
