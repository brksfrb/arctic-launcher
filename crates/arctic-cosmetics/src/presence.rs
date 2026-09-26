//! Who is playing with Arctic right now, and what emote they're playing.
//! While connected to a world, the Arctic Client checks in every minute
//! with the UUID it plays as there (on offline-mode servers and networks
//! that's the one derived from the name) and signs off when it leaves. A
//! lookup marks a player as on Arctic only if someone checked in as
//! exactly that UUID moments ago, so a player who switched clients, or
//! anyone else using the same name, doesn't get the snowflake. Where you
//! play is never sent.

use rusqlite::{OptionalExtension, params};
use serde::Serialize;

use crate::store::Store;

/// A check-in counts this long; clients check in every minute.
pub const ONLINE_WINDOW_SECS: u64 = 150;

/// One player in a lookup: their look (if any) and whether they're on Arctic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlayerEntry {
    pub skin: Option<String>,
    pub model: String,
    pub cape: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub cosmetics: Vec<String>,
    pub arctic: bool,
}

/// An emote someone is playing: which, and how long ago it started (ms,
/// so clients don't depend on their clock matching the server's).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Playing {
    pub id: String,
    pub elapsed: u64,
}

/// Look columns as read from a row.
type LookRow = (Option<String>, String, Option<String>, Vec<String>);

impl Store {
    /// Older databases get the check-in, cosmetics and emote columns.
    pub(crate) fn migrate_presence(&self) -> rusqlite::Result<()> {
        let conn = self.conn();
        let columns = {
            let mut stmt = conn.prepare("SELECT name FROM pragma_table_info('players')")?;
            stmt.query_map([], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        for (column, kind) in [
            ("last_online", "INTEGER"),
            ("playing_as", "TEXT"),
            ("cosmetics", "TEXT"),
            ("emote", "TEXT"),
            ("emote_at", "INTEGER"),
        ] {
            if !columns.iter().any(|n| n == column) {
                conn.execute_batch(&format!("ALTER TABLE players ADD COLUMN {column} {kind}"))?;
            }
        }
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS players_playing_as ON players (playing_as)",
        )?;
        Ok(())
    }

    /// A signed-in player is in a world as `playing_as` (`None` = left).
    pub fn check_in(&self, uuid: &str, playing_as: Option<&str>, now: u64) -> rusqlite::Result<()> {
        self.conn().execute(
            "UPDATE players SET playing_as = ?2, last_online = ?3 WHERE uuid = ?1",
            params![uuid, playing_as, now as i64],
        )?;
        Ok(())
    }

    /// For each asked UUID: the look of the player it belongs to, or of the
    /// Arctic player currently playing as it; and whether one is. Players
    /// with neither are left out.
    pub fn entries(
        &self,
        uuids: &[String],
        now: u64,
    ) -> rusqlite::Result<Vec<(String, PlayerEntry)>> {
        let conn = self.conn();
        let since = now.saturating_sub(ONLINE_WINDOW_SECS) as i64;
        let mut own = conn
            .prepare_cached("SELECT skin, model, cape, cosmetics FROM players WHERE uuid = ?1")?;
        let mut playing = conn.prepare_cached(
            "SELECT skin, model, cape, cosmetics FROM players
             WHERE playing_as = ?1 AND last_online >= ?2
             ORDER BY last_online DESC LIMIT 1",
        )?;
        let look = |r: &rusqlite::Row| -> rusqlite::Result<LookRow> {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                crate::store::cosmetics_column(r.get(3)?),
            ))
        };
        let mut out = Vec::new();
        for uuid in uuids {
            let current = playing.query_row(params![uuid, since], look).optional()?;
            let owned = own.query_row([uuid], look).optional()?;
            let arctic = current.is_some();
            // The account's own look first; on offline servers, the look of
            // whoever is on Arctic as this UUID right now.
            let Some((skin, model, cape, cosmetics)) = owned.or(current) else {
                continue;
            };
            if arctic || skin.is_some() || cape.is_some() || !cosmetics.is_empty() {
                out.push((
                    uuid.clone(),
                    PlayerEntry {
                        skin,
                        model,
                        cape,
                        cosmetics,
                        arctic,
                    },
                ));
            }
        }
        Ok(out)
    }

    /// Start an emote (`None` stops it); `now_ms` is Unix milliseconds.
    pub fn set_emote(&self, uuid: &str, emote: Option<&str>, now_ms: u64) -> rusqlite::Result<()> {
        self.conn().execute(
            "UPDATE players SET emote = ?2, emote_at = ?3 WHERE uuid = ?1",
            params![uuid, emote, now_ms as i64],
        )?;
        Ok(())
    }

    /// Emotes being played right now by the Arctic players in `uuids`
    /// (in-world UUIDs, as for lookups). `length_ms` gives how long each
    /// emote runs (`None` for unknown ids, which are skipped).
    pub fn emotes(
        &self,
        uuids: &[String],
        now_ms: u64,
        length_ms: impl Fn(&str) -> Option<u64>,
    ) -> rusqlite::Result<Vec<(String, Playing)>> {
        let conn = self.conn();
        let since = (now_ms / 1000).saturating_sub(ONLINE_WINDOW_SECS) as i64;
        let mut stmt = conn.prepare_cached(
            "SELECT emote, emote_at FROM players
             WHERE playing_as = ?1 AND last_online >= ?2 AND emote IS NOT NULL
             ORDER BY last_online DESC LIMIT 1",
        )?;
        let mut out = Vec::new();
        for uuid in uuids {
            let row = stmt
                .query_row(params![uuid, since], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
                })
                .optional()?;
            let Some((id, at)) = row else { continue };
            let started = at.max(0) as u64;
            let Some(length) = length_ms(&id) else {
                continue;
            };
            if now_ms < started.saturating_add(length) {
                let elapsed = now_ms.saturating_sub(started);
                out.push((uuid.clone(), Playing { id, elapsed }));
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Look;

    const ALICE: &str = "0123456789abcdef0123456789abcdef";

    fn store() -> Store {
        let s = Store::memory().unwrap();
        s.touch(ALICE, "Alice", 1).unwrap();
        s.set_look(
            ALICE,
            &Look {
                skin: None,
                model: "classic".into(),
                cape: Some("c".repeat(40)),
                cosmetics: vec!["halo".into()],
            },
            1,
        )
        .unwrap();
        s
    }

    #[test]
    fn on_arctic_only_while_checked_in_as_that_uuid() {
        let s = store();
        let me = [ALICE.to_owned()];
        assert!(
            !s.entries(&me, 1000).unwrap()[0].1.arctic,
            "look, but not playing"
        );
        s.check_in(ALICE, Some(ALICE), 1000).unwrap();
        assert!(s.entries(&me, 1030).unwrap()[0].1.arctic);
        let later = s.entries(&me, 1000 + ONLINE_WINDOW_SECS + 1).unwrap();
        assert!(!later[0].1.arctic, "went quiet");
        s.check_in(ALICE, None, 1040).unwrap();
        assert!(!s.entries(&me, 1041).unwrap()[0].1.arctic, "signed off");
    }

    #[test]
    fn offline_servers_match_the_uuid_played_as_not_the_name() {
        let s = store();
        let offline = crate::images::offline_uuid("Alice");
        let asked = std::slice::from_ref(&offline);
        // Someone else called Alice on another client: nothing.
        assert!(s.entries(asked, 1000).unwrap().is_empty());
        // Alice herself, on Arctic, on an offline server.
        s.check_in(ALICE, Some(&offline), 1000).unwrap();
        let found = s.entries(asked, 1010).unwrap();
        assert_eq!(found.len(), 1);
        assert!(found[0].1.arctic);
        assert_eq!(
            found[0].1.cape,
            Some("c".repeat(40)),
            "her cape shows there too"
        );
        assert_eq!(found[0].1.cosmetics, ["halo"]);
    }

    #[test]
    fn emotes_play_for_their_length_while_on_arctic() {
        let s = store();
        let me = [ALICE.to_owned()];
        let length = |id: &str| (id == "wave").then_some(2000);
        s.set_emote(ALICE, Some("wave"), 1_000_000).unwrap();
        assert!(
            s.emotes(&me, 1_000_500, length).unwrap().is_empty(),
            "not checked in"
        );
        s.check_in(ALICE, Some(ALICE), 1000).unwrap();
        let now = s.emotes(&me, 1_000_500, length).unwrap();
        assert_eq!(
            now[0].1,
            Playing {
                id: "wave".into(),
                elapsed: 500
            }
        );
        assert!(
            s.emotes(&me, 1_002_001, length).unwrap().is_empty(),
            "finished"
        );
        s.set_emote(ALICE, Some("gone"), 1_000_600).unwrap();
        assert!(
            s.emotes(&me, 1_000_700, length).unwrap().is_empty(),
            "unknown emote"
        );
    }
}
