//! How many players come back: one row per account per day it was active (signed in, or any
//! signed-in call), and nothing else. It gives daily active players and cohort retention (of the
//! accounts first seen in a week, how many were back a day, a week and a month later).

use std::collections::{BTreeSet, HashMap};

use rusqlite::params;
use serde_json::{Value, json};

use crate::store::Store;

const DAY: u64 = 86_400;
/// The accounts remembered as written today, so a busy account costs one write a day.
const REMEMBERED: usize = 100_000;
/// Cohorts shown (newest weeks).
const COHORTS: usize = 12;

impl Store {
    pub(crate) fn migrate_activity(&self) -> rusqlite::Result<()> {
        self.conn().execute_batch(
            "CREATE TABLE IF NOT EXISTS activity (
                 uuid TEXT NOT NULL,
                 day INTEGER NOT NULL,
                 PRIMARY KEY (uuid, day)
             ) WITHOUT ROWID;",
        )
    }

    /// `uuid` was active at `now`: kept as one row for that day.
    pub fn note_active(&self, uuid: &str, now: u64) {
        let day = now / DAY;
        {
            let mut noted = self.noted.lock().unwrap_or_else(|e| e.into_inner());
            if noted.get(uuid) == Some(&day) {
                return;
            }
            if noted.len() >= REMEMBERED {
                noted.clear();
            }
            noted.insert(uuid.to_owned(), day);
        }
        let written = self.conn().execute(
            "INSERT OR IGNORE INTO activity (uuid, day) VALUES (?1, ?2)",
            params![uuid, day as i64],
        );
        if let Err(e) = written {
            log::warn!("activity: {e}");
        }
    }

    /// Distinct accounts active in the last 1, 7 and 30 days (today counts).
    pub fn active_counts(&self, now: u64) -> rusqlite::Result<(i64, i64, i64)> {
        let today = (now / DAY) as i64;
        let conn = self.conn();
        let count = |days: i64| -> rusqlite::Result<i64> {
            conn.query_row(
                "SELECT COUNT(DISTINCT uuid) FROM activity WHERE day > ?1",
                [today - days],
                |r| r.get(0),
            )
        };
        Ok((count(1)?, count(7)?, count(30)?))
    }

    /// Daily active accounts for the last 30 days and the retention of each week's new accounts.
    pub fn retention(&self, now: u64) -> rusqlite::Result<Value> {
        let today = (now / DAY) as i64;
        let mut days: HashMap<String, BTreeSet<i64>> = HashMap::new();
        {
            let conn = self.conn();
            let mut stmt = conn.prepare("SELECT uuid, day FROM activity")?;
            let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
            for row in rows {
                let (uuid, day) = row?;
                days.entry(uuid).or_default().insert(day);
            }
        }
        Ok(summarize(&days, today))
    }
}

/// The numbers for an `uuid → days active` map, as of `today` (a day number).
fn summarize(days: &HashMap<String, BTreeSet<i64>>, today: i64) -> Value {
    let since = days.values().filter_map(|d| d.first().copied()).min();
    let daily: Vec<Value> = (today - 29..=today)
        .map(|day| {
            let users = days.values().filter(|d| d.contains(&day)).count();
            json!({"day": ymd(day), "users": users})
        })
        .collect();
    // Weeks start on Monday (day 0, 1970-01-01, was a Thursday).
    let week_of = |day: i64| day - (day + 3).rem_euclid(7);
    let mut cohorts: std::collections::BTreeMap<i64, Vec<(&BTreeSet<i64>, i64)>> =
        std::collections::BTreeMap::new();
    for set in days.values() {
        if let Some(&first) = set.first() {
            cohorts
                .entry(week_of(first))
                .or_default()
                .push((set, first));
        }
    }
    let newest: Vec<Value> = cohorts
        .iter()
        .rev()
        .take(COHORTS)
        .map(|(week, members)| {
            // Back `after` days after the first one: only members old enough to have had that day.
            let back = |after: i64| {
                let eligible: Vec<_> = members.iter().filter(|(_, first)| first + after < today).collect();
                let returned = eligible.iter().filter(|(set, first)| set.contains(&(first + after))).count();
                json!({"back": returned, "of": eligible.len()})
            };
            json!({"week": ymd(*week), "new": members.len(), "d1": back(1), "d7": back(7), "d30": back(30)})
        })
        .collect();
    json!({
        "tracking_since": since.map(ymd),
        "today": ymd(today),
        "daily_active": daily,
        "cohorts": newest,
    })
}

/// "2026-10-21" for a day number since 1970-01-01.
fn ymd(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(day: u64) -> u64 {
        day * DAY + 100
    }

    #[test]
    fn days_are_dates() {
        assert_eq!(ymd(0), "1970-01-01");
        assert_eq!(ymd(20_747), "2026-10-21");
    }

    #[test]
    fn one_row_a_day_and_counts_by_window() {
        let store = Store::memory().unwrap();
        for _ in 0..5 {
            store.note_active("a", at(100));
        }
        store.note_active("a", at(101));
        store.note_active("b", at(101));
        let rows: i64 = store
            .conn()
            .query_row("SELECT COUNT(*) FROM activity", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 3);
        assert_eq!(store.active_counts(at(101)).unwrap(), (2, 2, 2));
        assert_eq!(store.active_counts(at(102)).unwrap(), (0, 2, 2));
        assert_eq!(store.active_counts(at(140)).unwrap(), (0, 0, 0));
    }

    #[test]
    fn a_cohorts_returns_count_only_when_the_day_has_come() {
        let store = Store::memory().unwrap();
        // Both first seen on day 10; "a" is back on days 11 and 17, "b" never.
        for (uuid, day) in [("a", 10), ("b", 10), ("a", 11), ("a", 17)] {
            store.note_active(uuid, at(day));
        }
        let out = store.retention(at(20)).unwrap();
        let cohort = &out["cohorts"][0];
        assert_eq!(cohort["new"], 2);
        assert_eq!(cohort["d1"], json!({"back": 1, "of": 2}));
        assert_eq!(cohort["d7"], json!({"back": 1, "of": 2}));
        // Day 40 hasn't happened yet: nobody counts.
        assert_eq!(cohort["d30"], json!({"back": 0, "of": 0}));
        assert_eq!(out["daily_active"].as_array().unwrap().len(), 30);
        assert_eq!(out["tracking_since"], ymd(10));
    }
}
