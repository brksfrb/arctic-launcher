//! Chat between friends (profiles), for the launcher and the game: short
//! text messages and screenshots, kept 30 days (and at most the last few
//! hundred per pair), read by polling for anything newer than the last one
//! seen. Screenshots are only ever served to the two people in that chat.

use rusqlite::{OptionalExtension, params};
use serde::Serialize;

use crate::friends::FriendError;
use crate::store::Store;

pub const MAX_TEXT: usize = 500;
/// Messages one profile may send per minute.
pub const MAX_PER_MINUTE: i64 = 30;
/// Messages returned per request.
pub const PAGE: i64 = 50;
const KEEP_SECS: u64 = 30 * 24 * 60 * 60;
/// Messages kept per pair of friends.
const KEEP_PER_PAIR: i64 = 500;
/// Largest screenshot accepted (clients shrink them first).
pub const MAX_IMAGE_BYTES: usize = 2 * 1024 * 1024;
const MAX_IMAGE_SIDE: u32 = 4096;
/// Screenshots one profile may upload per hour.
const MAX_IMAGES_PER_HOUR: i64 = 30;
const DAY_SECS: u64 = 24 * 3600;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Message {
    pub id: i64,
    /// Sender's profile id.
    pub from: String,
    pub to: String,
    pub text: String,
    /// A screenshot (attachment id), if the message has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    pub sent: u64,
    pub read: bool,
}

fn row(r: &rusqlite::Row) -> rusqlite::Result<Message> {
    Ok(Message {
        id: r.get(0)?,
        from: r.get(1)?,
        to: r.get(2)?,
        text: r.get(3)?,
        sent: r.get::<_, i64>(4)?.max(0) as u64,
        read: r.get::<_, i64>(5)? != 0,
        image: r.get(6)?,
    })
}

const COLUMNS: &str = "id, sender, receiver, text, created, read, image";

/// A PNG of sensible size: `Some((width, height))`.
pub fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() > MAX_IMAGE_BYTES {
        return None;
    }
    let reader = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .ok()?;
    let info = reader.info();
    let ok = info.width > 0
        && info.height > 0
        && info.width <= MAX_IMAGE_SIDE
        && info.height <= MAX_IMAGE_SIDE;
    ok.then_some((info.width, info.height))
}

/// Text as sent, where a screenshot may come without words.
pub fn clean_text_for(text: &str, with_image: bool) -> Option<String> {
    if with_image && text.trim().is_empty() {
        return Some(String::new());
    }
    clean_text(text)
}

/// Text as sent: trimmed, no control characters, not empty, not too long.
pub fn clean_text(text: &str) -> Option<String> {
    let text: String = text
        .trim()
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let ok = !text.is_empty() && text.chars().count() <= MAX_TEXT;
    ok.then_some(text)
}

impl Store {
    pub(crate) fn migrate_chat(&self) -> rusqlite::Result<()> {
        self.conn().execute_batch(
            "CREATE TABLE IF NOT EXISTS messages (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 sender TEXT NOT NULL,
                 receiver TEXT NOT NULL,
                 text TEXT NOT NULL,
                 created INTEGER NOT NULL,
                 read INTEGER NOT NULL DEFAULT 0
             );
             CREATE INDEX IF NOT EXISTS messages_receiver ON messages (receiver, id);
             CREATE INDEX IF NOT EXISTS messages_pair ON messages (sender, receiver, id);
             CREATE TABLE IF NOT EXISTS attachments (
                 id TEXT PRIMARY KEY,
                 owner TEXT NOT NULL,
                 png BLOB NOT NULL,
                 created INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS attachments_owner ON attachments (owner, created);",
        )?;
        let has_image: bool = self.conn().query_row(
            "SELECT EXISTS (SELECT 1 FROM pragma_table_info('messages') WHERE name = 'image')",
            [],
            |r| r.get(0),
        )?;
        if !has_image {
            self.conn()
                .execute_batch("ALTER TABLE messages ADD COLUMN image TEXT")?;
        }
        Ok(())
    }

    /// Keep an uploaded screenshot; its id is its hash (same picture, same id).
    pub fn attachment_put(&self, owner: &str, png: &[u8], now: u64) -> Result<String, FriendError> {
        use sha2::{Digest, Sha256};
        let conn = self.conn();
        let recent: i64 = conn.query_row(
            "SELECT COUNT(*) FROM attachments WHERE owner = ?1 AND created >= ?2",
            params![owner, now.saturating_sub(3600) as i64],
            |r| r.get(0),
        )?;
        if recent >= MAX_IMAGES_PER_HOUR {
            return Err(FriendError::TooMany);
        }
        let id = hex::encode(Sha256::digest(png));
        conn.execute(
            "INSERT OR IGNORE INTO attachments (id, owner, png, created) VALUES (?1, ?2, ?3, ?4)",
            params![id, owner, png, now as i64],
        )?;
        prune_attachments(&conn, now)?;
        Ok(id)
    }

    /// A screenshot, for someone in a chat that has it.
    pub fn attachment_get(&self, me: &str, id: &str) -> Result<Option<Vec<u8>>, FriendError> {
        let conn = self.conn();
        let allowed: bool = conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM messages WHERE image = ?1 AND (sender = ?2 OR receiver = ?2))",
            params![id, me],
            |r| r.get(0),
        )?;
        if !allowed {
            return Ok(None);
        }
        Ok(conn
            .query_row("SELECT png FROM attachments WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .optional()?)
    }

    /// Send `text` from profile `from` to friend `to`.
    pub fn chat_send(
        &self,
        from: &str,
        to: &str,
        text: &str,
        image: Option<&str>,
        now: u64,
    ) -> Result<Message, FriendError> {
        let conn = self.conn();
        if !crate::friends::are_friends(&conn, from, to)? {
            return Err(FriendError::NotFriends);
        }
        if let Some(id) = image {
            let mine: bool = conn.query_row(
                "SELECT EXISTS (SELECT 1 FROM attachments WHERE id = ?1 AND owner = ?2)",
                params![id, from],
                |r| r.get(0),
            )?;
            if !mine {
                return Err(FriendError::NotFound);
            }
        }
        let recent: i64 = conn.query_row(
            "SELECT COUNT(*) FROM messages WHERE sender = ?1 AND created >= ?2",
            params![from, now.saturating_sub(60) as i64],
            |r| r.get(0),
        )?;
        if recent >= MAX_PER_MINUTE {
            return Err(FriendError::TooMany);
        }
        conn.execute(
            "INSERT INTO messages (sender, receiver, text, created, image) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![from, to, text, now as i64, image],
        )?;
        let id = conn.last_insert_rowid();
        // Keep the table small: old messages, and all but the latest per pair.
        conn.execute(
            "DELETE FROM messages WHERE created < ?1",
            [now.saturating_sub(KEEP_SECS) as i64],
        )?;
        conn.execute(
            "DELETE FROM messages WHERE ((sender = ?1 AND receiver = ?2) OR (sender = ?2 AND receiver = ?1))
                 AND id <= ?3 - ?4",
            params![from, to, id, KEEP_PER_PAIR * 4],
        )?;
        prune_attachments(&conn, now)?;
        Ok(Message {
            id,
            from: from.to_owned(),
            to: to.to_owned(),
            text: text.to_owned(),
            image: image.map(str::to_owned),
            sent: now,
            read: false,
        })
    }

    /// The conversation with `other`, newest last: the page before
    /// message `before` (or the latest page).
    pub fn chat_history(
        &self,
        me: &str,
        other: &str,
        before: Option<i64>,
    ) -> Result<Vec<Message>, FriendError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(&format!(
            "SELECT {COLUMNS} FROM messages
             WHERE ((sender = ?1 AND receiver = ?2) OR (sender = ?2 AND receiver = ?1)) AND id < ?3
             ORDER BY id DESC LIMIT ?4"
        ))?;
        let mut list: Vec<Message> = stmt
            .query_map(params![me, other, before.unwrap_or(i64::MAX), PAGE], row)?
            .collect::<rusqlite::Result<_>>()?;
        list.reverse();
        Ok(list)
    }

    /// Messages to `me` newer than `after` (for polling), oldest first.
    /// A negative `after` gives just the latest one, so a client can learn
    /// where "new" starts without being handed old messages.
    pub fn chat_new(&self, me: &str, after: i64) -> Result<Vec<Message>, FriendError> {
        let conn = self.conn();
        if after < 0 {
            let mut stmt = conn.prepare(&format!(
                "SELECT {COLUMNS} FROM messages WHERE receiver = ?1 ORDER BY id DESC LIMIT 1"
            ))?;
            return Ok(stmt
                .query_map([me], row)?
                .collect::<rusqlite::Result<_>>()?);
        }
        let mut stmt = conn.prepare(&format!(
            "SELECT {COLUMNS} FROM messages WHERE receiver = ?1 AND id > ?2 ORDER BY id LIMIT ?3"
        ))?;
        Ok(stmt
            .query_map(params![me, after, PAGE * 2], row)?
            .collect::<rusqlite::Result<_>>()?)
    }

    /// Everything from `other` to `me` has been seen.
    pub fn chat_read(&self, me: &str, other: &str) -> Result<(), FriendError> {
        self.conn().execute(
            "UPDATE messages SET read = 1 WHERE receiver = ?1 AND sender = ?2 AND read = 0",
            params![me, other],
        )?;
        Ok(())
    }

    #[cfg(test)]
    pub fn chat_unread(&self, me: &str, from: &str) -> rusqlite::Result<u32> {
        self.conn().query_row(
            "SELECT COUNT(*) FROM messages WHERE receiver = ?1 AND sender = ?2 AND read = 0",
            params![me, from],
            |r| {
                r.get::<_, i64>(0)
                    .map(|n| n.clamp(0, i64::from(u32::MAX)) as u32)
            },
        )
    }
}

/// Screenshots no message points to (after a day to get sent) go.
fn prune_attachments(conn: &rusqlite::Connection, now: u64) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM attachments WHERE created < ?1
             AND id NOT IN (SELECT image FROM messages WHERE image IS NOT NULL)",
        [now.saturating_sub(DAY_SECS) as i64],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALICE: &str = "aaaaaaaaaaaa4aaaaaaaaaaaaaaaaaaa";
    const BOB: &str = "bbbbbbbbbbbb4bbbbbbbbbbbbbbbbbbb";
    const CAROL: &str = "cccccccccccc4ccccccccccccccccccc";

    fn setup() -> (Store, String, String, String) {
        let s = Store::memory().unwrap();
        for (u, n) in [(ALICE, "Alice"), (BOB, "Bob"), (CAROL, "Carol")] {
            s.touch(u, n, 1).unwrap();
        }
        let (a, b, c) = (
            s.profile_id(ALICE, 1).unwrap(),
            s.profile_id(BOB, 1).unwrap(),
            s.profile_id(CAROL, 1).unwrap(),
        );
        s.friend_request(&a, "Bob", 1).unwrap();
        s.friend_accept(&b, &a, 1).unwrap();
        (s, a, b, c)
    }

    #[test]
    fn friends_chat_and_read() {
        let (s, a, b, c) = setup();
        assert_eq!(
            s.chat_send(&a, &c, "hi", None, 5),
            Err(FriendError::NotFriends)
        );
        let first = s.chat_send(&a, &b, "hi bob", None, 5).unwrap();
        s.chat_send(&b, &a, "hey", None, 6).unwrap();
        s.chat_send(&a, &b, "join me?", None, 7).unwrap();
        let history = s.chat_history(&b, &a, None).unwrap();
        assert_eq!(
            history.iter().map(|m| m.text.as_str()).collect::<Vec<_>>(),
            ["hi bob", "hey", "join me?"]
        );
        assert_eq!(s.chat_unread(&b, &a).unwrap(), 2);
        let new = s.chat_new(&b, first.id).unwrap();
        assert_eq!(new.len(), 1);
        assert_eq!(new[0].text, "join me?");
        s.chat_read(&b, &a).unwrap();
        assert_eq!(s.chat_unread(&b, &a).unwrap(), 0);
        let older = s.chat_history(&b, &a, Some(new[0].id)).unwrap();
        assert_eq!(older.len(), 2);
        let latest = s.chat_new(&b, -1).unwrap();
        assert_eq!((latest.len(), latest[0].text.as_str()), (1, "join me?"));
    }

    #[test]
    fn rate_limit_and_text_rules() {
        let (s, a, b, _) = setup();
        for i in 0..MAX_PER_MINUTE {
            s.chat_send(&a, &b, &format!("m{i}"), None, 100).unwrap();
        }
        assert_eq!(
            s.chat_send(&a, &b, "more", None, 100),
            Err(FriendError::TooMany)
        );
        assert!(s.chat_send(&a, &b, "later", None, 161).is_ok());
        assert_eq!(clean_text("  hi\nthere  ").as_deref(), Some("hi there"));
        assert_eq!(clean_text("   "), None);
        assert_eq!(clean_text(&"x".repeat(MAX_TEXT + 1)), None);
        assert_eq!(clean_text_for("  ", true).as_deref(), Some(""));
        assert_eq!(clean_text_for("  ", false), None);
    }

    #[test]
    fn screenshots_only_reach_their_chat() {
        let (s, a, b, c) = setup();
        let png = crate::images::test_png(64, 32);
        assert_eq!(png_size(&png), Some((64, 32)));
        assert_eq!(png_size(b"not a png"), None);
        let id = s.attachment_put(&a, &png, 10).unwrap();
        // Bob can't send Alice's upload as his own.
        assert_eq!(
            s.chat_send(&b, &a, "", Some(&id), 11),
            Err(FriendError::NotFound)
        );
        // Not sent yet: nobody can fetch it.
        assert_eq!(s.attachment_get(&a, &id).unwrap(), None);
        let m = s.chat_send(&a, &b, "look", Some(&id), 12).unwrap();
        assert_eq!(m.image.as_deref(), Some(id.as_str()));
        assert_eq!(
            s.attachment_get(&b, &id).unwrap().as_deref(),
            Some(png.as_slice())
        );
        assert_eq!(s.attachment_get(&c, &id).unwrap(), None);
        assert_eq!(
            s.chat_history(&b, &a, None).unwrap()[0].image.as_deref(),
            Some(id.as_str())
        );
    }
}
