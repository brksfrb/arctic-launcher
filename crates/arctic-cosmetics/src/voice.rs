//! Voice rooms: which Arctic players are on the same Minecraft server with
//! voice chat on, so their launchers can connect to each other directly.
//! A room is a hash of the server address (the address itself is never
//! sent); members check in every so often and drop out when they stop.

use rusqlite::params;
use serde::Serialize;

use crate::store::Store;

/// A member counts this long after their last check-in.
pub const MEMBER_SECS: u64 = 60;
/// Most members returned (the nearest are sorted out by the game).
pub const MAX_MEMBERS: i64 = 100;
pub const ROOM_LEN: usize = 64;
/// iroh node ids are 32 bytes (64 hex digits) or base32; keep it bounded.
pub const MAX_NODE: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Member {
    /// Their launcher's peer address (to connect to).
    pub node: String,
    /// The UUID they play as there (to find them in the world).
    pub uuid: String,
    pub name: String,
    /// A friend of the one asking.
    pub friend: bool,
}

pub fn valid_room(room: &str) -> bool {
    room.len() == ROOM_LEN && room.bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn valid_node(node: &str) -> bool {
    !node.is_empty() && node.len() <= MAX_NODE && node.bytes().all(|b| b.is_ascii_alphanumeric())
}

impl Store {
    pub(crate) fn migrate_voice(&self) -> rusqlite::Result<()> {
        self.conn().execute_batch(
            "CREATE TABLE IF NOT EXISTS voice_members (
                 uuid TEXT PRIMARY KEY,
                 room TEXT NOT NULL,
                 node TEXT NOT NULL,
                 playing_as TEXT NOT NULL,
                 seen INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS voice_members_room ON voice_members (room, seen);",
        )
    }

    /// Check in to a room (one room per account at a time) and get the
    /// others in it.
    pub fn voice_join(
        &self,
        uuid: &str,
        room: &str,
        node: &str,
        playing_as: &str,
        now: u64,
    ) -> rusqlite::Result<Vec<Member>> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO voice_members (uuid, room, node, playing_as, seen) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(uuid) DO UPDATE SET room = excluded.room, node = excluded.node,
                 playing_as = excluded.playing_as, seen = excluded.seen",
            params![uuid, room, node, playing_as, now as i64],
        )?;
        conn.execute(
            "DELETE FROM voice_members WHERE seen < ?1",
            [now.saturating_sub(MEMBER_SECS * 5) as i64],
        )?;
        let me: Option<String> = conn
            .query_row(
                "SELECT profile FROM profile_accounts WHERE uuid = ?1",
                [uuid],
                |r| r.get(0),
            )
            .ok();
        let mut stmt = conn.prepare(
            "SELECT v.node, v.playing_as, COALESCE(p.name, ''), a.profile
             FROM voice_members v
             LEFT JOIN players p ON p.uuid = v.uuid
             LEFT JOIN profile_accounts a ON a.uuid = v.uuid
             WHERE v.room = ?1 AND v.uuid != ?2 AND v.seen >= ?3
             ORDER BY v.seen DESC LIMIT ?4",
        )?;
        let rows: Vec<(String, String, String, Option<String>)> = stmt
            .query_map(
                params![
                    room,
                    uuid,
                    now.saturating_sub(MEMBER_SECS) as i64,
                    MAX_MEMBERS
                ],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )?
            .collect::<rusqlite::Result<_>>()?;
        let mut out = Vec::with_capacity(rows.len());
        for (node, playing_as, name, profile) in rows {
            let friend = match (&me, profile) {
                (Some(a), Some(b)) => crate::friends::are_friends(&conn, a, &b)?,
                _ => false,
            };
            out.push(Member {
                node,
                uuid: playing_as,
                name,
                friend,
            });
        }
        Ok(out)
    }

    pub fn voice_leave(&self, uuid: &str) -> rusqlite::Result<()> {
        self.conn()
            .execute("DELETE FROM voice_members WHERE uuid = ?1", [uuid])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALICE: &str = "aaaaaaaaaaaa4aaaaaaaaaaaaaaaaaaa";
    const BOB: &str = "bbbbbbbbbbbb4bbbbbbbbbbbbbbbbbbb";
    const CAROL: &str = "cccccccccccc4ccccccccccccccccccc";

    fn room(c: char) -> String {
        std::iter::repeat_n(c, ROOM_LEN).collect()
    }

    #[test]
    fn members_see_each_other_in_the_same_room() {
        let s = Store::memory().unwrap();
        for (u, n) in [(ALICE, "Alice"), (BOB, "Bob"), (CAROL, "Carol")] {
            s.touch(u, n, 1).unwrap();
        }
        let a = s.profile_id(ALICE, 1).unwrap();
        let b = s.profile_id(BOB, 1).unwrap();
        s.friend_request(&a, "Bob", 1).unwrap();
        s.friend_accept(&b, &a, 1).unwrap();
        assert!(
            s.voice_join(ALICE, &room('a'), "nodea", ALICE, 100)
                .unwrap()
                .is_empty()
        );
        s.voice_join(CAROL, &room('b'), "nodec", CAROL, 100)
            .unwrap();
        let seen = s.voice_join(BOB, &room('a'), "nodeb", BOB, 110).unwrap();
        assert_eq!(seen.len(), 1);
        assert_eq!(
            (seen[0].node.as_str(), seen[0].name.as_str(), seen[0].friend),
            ("nodea", "Alice", true)
        );
        // Alice stops checking in: gone after a while.
        assert!(
            s.voice_join(BOB, &room('a'), "nodeb", BOB, 100 + MEMBER_SECS + 1)
                .unwrap()
                .is_empty()
        );
        // Carol moves to room a; strangers are listed, not as friends.
        let seen = s
            .voice_join(CAROL, &room('a'), "nodec", CAROL, 200)
            .unwrap();
        assert_eq!((seen.len(), seen[0].friend), (1, false));
        s.voice_leave(BOB).unwrap();
        assert!(
            s.voice_join(CAROL, &room('a'), "nodec", CAROL, 201)
                .unwrap()
                .is_empty()
        );
        assert!(valid_room(&room('f')) && !valid_room("xyz"));
        assert!(valid_node("abc123") && !valid_node("a b"));
    }
}
