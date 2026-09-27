//! Arctic profiles, friends and invites.
//!
//! A profile is one person: it's made the first time one of their
//! Minecraft accounts needs it, and more accounts join it by proving they
//! hold them (a sign-in token for each). Friendships and requests are
//! between profiles, so they follow the person across accounts.
//!
//! Offline accounts take part too: their sign-in already proves the random
//! key the launcher made for them on that PC (never hardware IDs). Their
//! names aren't unique, so people find them by the profile's friend code;
//! Microsoft names work too. A recovery code (only its hash is kept) moves
//! an offline profile to a new PC. Profiles with a Microsoft account are
//! marked verified.
//!
//! What friends see is up to each profile: whether it's online, which
//! server it plays on, whether it takes invites, and whether its linked
//! accounts are shown (off by default, so alts stay private).

use rusqlite::{OptionalExtension, Transaction, params};
use serde::Serialize;

use crate::presence::ONLINE_WINDOW_SECS;
use crate::store::Store;

pub const MAX_FRIENDS: i64 = 200;
pub const MAX_ACCOUNTS: i64 = 16;
pub const MAX_PENDING: i64 = 50;
pub const MAX_NAME: usize = 24;
/// Invites last this long.
pub const INVITE_SECS: u64 = 10 * 60;
/// Invites one profile may send per hour.
pub const MAX_INVITES_PER_HOUR: i64 = 30;
pub const MAX_TARGET: usize = 300;
/// Friend requests one profile may send per day.
pub const MAX_REQUESTS_PER_DAY: i64 = 30;
const DAY: u64 = 24 * 60 * 60;
/// Friend codes: no look-alike characters.
const ALPHABET: &[u8] = b"abcdefghjkmnpqrstuvwxyz23456789";
const CODE_LEN: usize = 8;
const RECOVERY_LEN: usize = 16;

/// Mojang account UUIDs are version 4; offline ones (from the name) are 3.
pub fn is_microsoft(uuid: &str) -> bool {
    uuid.len() == 32 && uuid.as_bytes()[12] == b'4'
}

fn random_code(len: usize) -> String {
    let mut out = String::with_capacity(len);
    while out.len() < len {
        for b in uuid::Uuid::new_v4().into_bytes() {
            if out.len() < len {
                out.push(ALPHABET[b as usize % ALPHABET.len()] as char);
            }
        }
    }
    out
}

/// `ABCD-efgh` → `abcdefgh`, if it can be a friend code.
pub fn clean_code(text: &str, len: usize) -> Option<String> {
    let code: String = text
        .trim()
        .chars()
        .filter(|c| *c != '-' && !c.is_whitespace())
        .map(|c| c.to_ascii_lowercase())
        .collect();
    (code.len() == len && code.bytes().all(|b| ALPHABET.contains(&b))).then_some(code)
}

/// `abcdefgh` → `abcd-efgh` (and 16 characters in fours).
pub fn pretty(code: &str) -> String {
    code.as_bytes()
        .chunks(4)
        .map(|c| String::from_utf8_lossy(c).into_owned())
        .collect::<Vec<_>>()
        .join("-")
}

fn recovery_hash(code: &str) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(code.as_bytes()))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Settings {
    pub share_online: bool,
    pub share_server: bool,
    pub allow_invites: bool,
    pub show_accounts: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Account {
    pub uuid: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    /// Share this so people can add you (`abcd-efgh`).
    pub code: String,
    /// Has a Microsoft account.
    pub verified: bool,
    /// A recovery code was made (it's shown only once).
    pub has_recovery: bool,
    pub accounts: Vec<Account>,
    pub settings: Settings,
}

/// A friend as the other side is allowed to see them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Friend {
    pub id: String,
    pub name: String,
    /// Only when they show their accounts.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub accounts: Vec<Account>,
    pub online: bool,
    pub in_game: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    pub takes_invites: bool,
    pub verified: bool,
    /// Messages from them not read yet.
    pub unread: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Person {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Invite {
    pub id: String,
    pub from: Person,
    /// `server` (an address) or `together` (a play-together code).
    pub kind: String,
    pub target: String,
    pub sent: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum FriendError {
    NotFound,
    Yourself,
    Already,
    TooMany,
    NotFriends,
    NoInvites,
    LastAccount,
    Db(String),
}

impl From<rusqlite::Error> for FriendError {
    fn from(e: rusqlite::Error) -> Self {
        FriendError::Db(e.to_string())
    }
}

type FResult<T> = Result<T, FriendError>;

fn random_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..16].to_owned()
}

/// Canonical pair order for the friends table.
fn pair<'a>(a: &'a str, b: &'a str) -> (&'a str, &'a str) {
    if a < b { (a, b) } else { (b, a) }
}

impl Store {
    pub(crate) fn migrate_friends(&self) -> rusqlite::Result<()> {
        let conn = self.conn();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS arctic_profiles (
                 id TEXT PRIMARY KEY,
                 name TEXT NOT NULL,
                 share_online INTEGER NOT NULL DEFAULT 1,
                 share_server INTEGER NOT NULL DEFAULT 1,
                 allow_invites INTEGER NOT NULL DEFAULT 1,
                 show_accounts INTEGER NOT NULL DEFAULT 0,
                 created INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS profile_accounts (
                 uuid TEXT PRIMARY KEY,
                 profile TEXT NOT NULL,
                 added INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS profile_accounts_profile ON profile_accounts (profile);
             CREATE TABLE IF NOT EXISTS friendships (
                 a TEXT NOT NULL,
                 b TEXT NOT NULL,
                 since INTEGER NOT NULL,
                 PRIMARY KEY (a, b)
             );
             CREATE INDEX IF NOT EXISTS friendships_b ON friendships (b);
             CREATE TABLE IF NOT EXISTS friend_requests (
                 sender TEXT NOT NULL,
                 receiver TEXT NOT NULL,
                 created INTEGER NOT NULL,
                 PRIMARY KEY (sender, receiver)
             );
             CREATE INDEX IF NOT EXISTS friend_requests_receiver ON friend_requests (receiver);
             CREATE TABLE IF NOT EXISTS invites (
                 id TEXT PRIMARY KEY,
                 sender TEXT NOT NULL,
                 receiver TEXT NOT NULL,
                 kind TEXT NOT NULL,
                 target TEXT NOT NULL,
                 created INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS invites_receiver ON invites (receiver, created);",
        )?;
        let columns = {
            let mut stmt = conn.prepare("SELECT name FROM pragma_table_info('players')")?;
            stmt.query_map([], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        for (column, kind) in [("server", "TEXT"), ("launcher_online", "INTEGER")] {
            if !columns.iter().any(|n| n == column) {
                conn.execute_batch(&format!("ALTER TABLE players ADD COLUMN {column} {kind}"))?;
            }
        }
        let profile_columns = {
            let mut stmt = conn.prepare("SELECT name FROM pragma_table_info('arctic_profiles')")?;
            stmt.query_map([], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        for column in ["code", "recovery"] {
            if !profile_columns.iter().any(|n| n == column) {
                conn.execute_batch(&format!(
                    "ALTER TABLE arctic_profiles ADD COLUMN {column} TEXT"
                ))?;
            }
        }
        conn.execute_batch(
            "CREATE UNIQUE INDEX IF NOT EXISTS arctic_profiles_code ON arctic_profiles (code)",
        )?;
        let missing: Vec<String> = {
            let mut stmt = conn.prepare("SELECT id FROM arctic_profiles WHERE code IS NULL")?;
            stmt.query_map([], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?
        };
        for id in missing {
            conn.execute(
                "UPDATE arctic_profiles SET code = ?2 WHERE id = ?1",
                params![id, random_code(CODE_LEN)],
            )?;
        }
        Ok(())
    }

    /// The account's profile, made on first use.
    pub fn profile_id(&self, uuid: &str, now: u64) -> FResult<String> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        let id = match profile_of(&tx, uuid)? {
            Some(id) => id,
            None => new_profile(&tx, uuid, now)?,
        };
        tx.commit()?;
        Ok(id)
    }

    /// A new recovery code for the profile (shown once; replaces the old).
    pub fn profile_new_recovery(&self, id: &str) -> FResult<String> {
        let code = random_code(RECOVERY_LEN);
        let n = self.conn().execute(
            "UPDATE arctic_profiles SET recovery = ?2 WHERE id = ?1",
            params![id, recovery_hash(&code)],
        )?;
        if n == 0 {
            return Err(FriendError::NotFound);
        }
        Ok(pretty(&code))
    }

    /// Whether `code` recovers the profile holding `uuid` (an offline
    /// account moving to a new PC).
    pub fn profile_recovers(&self, code: &str, uuid: &str) -> FResult<bool> {
        let Some(code) = clean_code(code, RECOVERY_LEN) else {
            return Ok(false);
        };
        let found: Option<String> = self
            .conn()
            .query_row(
                "SELECT a.profile FROM profile_accounts a JOIN arctic_profiles p ON p.id = a.profile
                 WHERE a.uuid = ?1 AND p.recovery = ?2",
                params![uuid, recovery_hash(&code)],
                |r| r.get(0),
            )
            .optional()?;
        Ok(found.is_some())
    }

    pub fn profile(&self, id: &str) -> FResult<Profile> {
        let conn = self.conn();
        let (name, code, has_recovery, settings) = conn
            .query_row(
                "SELECT name, share_online, share_server, allow_invites, show_accounts,
                     COALESCE(code, ''), recovery IS NOT NULL
                 FROM arctic_profiles WHERE id = ?1",
                [id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(5)?,
                        r.get::<_, bool>(6)?,
                        Settings {
                            share_online: r.get::<_, i64>(1)? != 0,
                            share_server: r.get::<_, i64>(2)? != 0,
                            allow_invites: r.get::<_, i64>(3)? != 0,
                            show_accounts: r.get::<_, i64>(4)? != 0,
                        },
                    ))
                },
            )
            .optional()?
            .ok_or(FriendError::NotFound)?;
        let accounts = accounts_of(&conn, id)?;
        Ok(Profile {
            id: id.to_owned(),
            name,
            code: pretty(&code),
            verified: accounts.iter().any(|a| is_microsoft(&a.uuid)),
            has_recovery,
            accounts,
            settings,
        })
    }

    pub fn profile_update(
        &self,
        id: &str,
        name: Option<&str>,
        settings: &[(&str, bool)],
    ) -> FResult<()> {
        let conn = self.conn();
        if let Some(name) = name {
            conn.execute(
                "UPDATE arctic_profiles SET name = ?2 WHERE id = ?1",
                params![id, name],
            )?;
        }
        for (key, on) in settings {
            // Keys come from a fixed list in the route, never from input.
            if matches!(
                *key,
                "share_online" | "share_server" | "allow_invites" | "show_accounts"
            ) {
                conn.execute(
                    &format!("UPDATE arctic_profiles SET {key} = ?2 WHERE id = ?1"),
                    params![id, i64::from(*on)],
                )?;
            }
        }
        Ok(())
    }

    /// Put `other` (proved by its own token) into profile `id`. If it had
    /// a profile of its own, that one is merged in: its accounts, friends
    /// and requests move over, then it's gone.
    pub fn profile_link(&self, id: &str, other: &str, now: u64) -> FResult<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        let count: i64 = tx.query_row(
            "SELECT COUNT(*) FROM profile_accounts WHERE profile = ?1",
            [id],
            |r| r.get(0),
        )?;
        match profile_of(&tx, other)? {
            Some(p) if p == id => return Ok(()),
            Some(old) => {
                let moving: i64 = tx.query_row(
                    "SELECT COUNT(*) FROM profile_accounts WHERE profile = ?1",
                    [&old],
                    |r| r.get(0),
                )?;
                if count + moving > MAX_ACCOUNTS {
                    return Err(FriendError::TooMany);
                }
                merge(&tx, &old, id)?;
            }
            None => {
                if count >= MAX_ACCOUNTS {
                    return Err(FriendError::TooMany);
                }
                tx.execute(
                    "INSERT INTO profile_accounts (uuid, profile, added) VALUES (?1, ?2, ?3)",
                    params![other, id, now as i64],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Take an account out of profile `id`; it starts over with a profile
    /// of its own (no friends). The last account can't leave.
    pub fn profile_unlink(&self, id: &str, uuid: &str) -> FResult<()> {
        let conn = self.conn();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM profile_accounts WHERE profile = ?1",
            [id],
            |r| r.get(0),
        )?;
        if count <= 1 {
            return Err(FriendError::LastAccount);
        }
        let removed = conn.execute(
            "DELETE FROM profile_accounts WHERE uuid = ?1 AND profile = ?2",
            params![uuid, id],
        )?;
        if removed == 0 {
            return Err(FriendError::NotFound);
        }
        Ok(())
    }

    /// [`Self::friend_request_to`], without the name (tests).
    #[cfg(test)]
    pub fn friend_request(&self, id: &str, who: &str, now: u64) -> FResult<bool> {
        self.friend_request_to(id, who, now)
            .map(|(friends, _)| friends)
    }

    /// Ask someone to be friends: by their friend code, or by the name of
    /// one of their Microsoft accounts (offline names aren't unique). If
    /// they already asked us, that makes us friends right away. Returns
    /// that, and their profile name.
    pub fn friend_request_to(&self, id: &str, who: &str, now: u64) -> FResult<(bool, String)> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        let by_code: Option<String> = match clean_code(who, CODE_LEN) {
            Some(code) => tx
                .query_row(
                    "SELECT id FROM arctic_profiles WHERE code = ?1",
                    [code],
                    |r| r.get(0),
                )
                .optional()?,
            None => None,
        };
        let target = match by_code {
            Some(p) => p,
            None => {
                let uuid: Option<String> = tx
                    .query_row(
                        "SELECT uuid FROM players WHERE name = ?1 COLLATE NOCASE
                             AND substr(uuid, 13, 1) = '4'
                         ORDER BY updated DESC LIMIT 1",
                        [who.trim()],
                        |r| r.get(0),
                    )
                    .optional()?;
                let Some(u) = uuid else {
                    return Err(FriendError::NotFound);
                };
                match profile_of(&tx, &u)? {
                    Some(p) => p,
                    // They used Arctic but never needed a profile yet.
                    None => new_profile(&tx, &u, now)?,
                }
            }
        };
        if target == id {
            return Err(FriendError::Yourself);
        }
        if are_friends(&tx, id, &target)? {
            return Err(FriendError::Already);
        }
        let name: String = tx.query_row(
            "SELECT name FROM arctic_profiles WHERE id = ?1",
            [&target],
            |r| r.get(0),
        )?;
        let theirs = tx.execute(
            "DELETE FROM friend_requests WHERE sender = ?1 AND receiver = ?2",
            params![target, id],
        )?;
        if theirs > 0 {
            befriend(&tx, id, &target, now)?;
            tx.commit()?;
            return Ok((true, name));
        }
        if friend_count(&tx, id)? >= MAX_FRIENDS {
            return Err(FriendError::TooMany);
        }
        let pending: i64 = tx.query_row(
            "SELECT COUNT(*) FROM friend_requests WHERE sender = ?1",
            [id],
            |r| r.get(0),
        )?;
        let today: i64 = tx.query_row(
            "SELECT COUNT(*) FROM friend_requests WHERE sender = ?1 AND created >= ?2",
            params![id, now.saturating_sub(DAY) as i64],
            |r| r.get(0),
        )?;
        if pending >= MAX_PENDING || today >= MAX_REQUESTS_PER_DAY {
            return Err(FriendError::TooMany);
        }
        tx.execute(
            "INSERT OR IGNORE INTO friend_requests (sender, receiver, created) VALUES (?1, ?2, ?3)",
            params![id, target, now as i64],
        )?;
        tx.commit()?;
        Ok((false, name))
    }

    pub fn friend_accept(&self, id: &str, from: &str, now: u64) -> FResult<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        let found = tx.execute(
            "DELETE FROM friend_requests WHERE sender = ?1 AND receiver = ?2",
            params![from, id],
        )?;
        if found == 0 {
            return Err(FriendError::NotFound);
        }
        if friend_count(&tx, id)? >= MAX_FRIENDS {
            return Err(FriendError::TooMany);
        }
        befriend(&tx, id, from, now)?;
        tx.commit()?;
        Ok(())
    }

    /// Decline their request, cancel ours, or end the friendship.
    pub fn friend_remove(&self, id: &str, other: &str) -> FResult<()> {
        let conn = self.conn();
        let (a, b) = pair(id, other);
        let n = conn.execute(
            "DELETE FROM friendships WHERE a = ?1 AND b = ?2",
            params![a, b],
        )? + conn.execute(
            "DELETE FROM friend_requests WHERE (sender = ?1 AND receiver = ?2)
                     OR (sender = ?2 AND receiver = ?1)",
            params![id, other],
        )?;
        if n == 0 {
            return Err(FriendError::NotFound);
        }
        conn.execute(
            "DELETE FROM invites WHERE (sender = ?1 AND receiver = ?2) OR (sender = ?2 AND receiver = ?1)",
            params![id, other],
        )?;
        Ok(())
    }

    /// Friends as each allows, and requests both ways.
    pub fn friends(&self, id: &str, now: u64) -> FResult<(Vec<Friend>, Vec<Person>, Vec<Person>)> {
        let conn = self.conn();
        let ids: Vec<String> = {
            let mut stmt = conn.prepare(
                "SELECT b FROM friendships WHERE a = ?1 UNION SELECT a FROM friendships WHERE b = ?1",
            )?;
            stmt.query_map([id], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?
        };
        let since = now.saturating_sub(ONLINE_WINDOW_SECS) as i64;
        let mut friends = Vec::new();
        for fid in ids {
            let Some((name, s)) = conn
                .query_row(
                    "SELECT name, share_online, share_server, allow_invites, show_accounts
                     FROM arctic_profiles WHERE id = ?1",
                    [&fid],
                    |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            [r.get::<_, i64>(1)?, r.get(2)?, r.get(3)?, r.get(4)?].map(|v| v != 0),
                        ))
                    },
                )
                .optional()?
            else {
                continue;
            };
            let [share_online, share_server, allow_invites, show_accounts] = s;
            // In a world: the Arctic Client checked in lately. Online: that
            // or the launcher did.
            let game: Option<Option<String>> = conn
                .query_row(
                    "SELECT p.server FROM players p JOIN profile_accounts a ON a.uuid = p.uuid
                     WHERE a.profile = ?1 AND p.playing_as IS NOT NULL AND p.last_online >= ?2
                     ORDER BY p.last_online DESC LIMIT 1",
                    params![fid, since],
                    |r| r.get(0),
                )
                .optional()?;
            let launcher: bool = conn.query_row(
                "SELECT EXISTS (SELECT 1 FROM players p JOIN profile_accounts a ON a.uuid = p.uuid
                 WHERE a.profile = ?1 AND (p.launcher_online >= ?2 OR p.last_online >= ?2))",
                params![fid, since],
                |r| r.get(0),
            )?;
            let in_game = share_online && game.is_some();
            let verified: bool = conn.query_row(
                "SELECT EXISTS (SELECT 1 FROM profile_accounts
                 WHERE profile = ?1 AND substr(uuid, 13, 1) = '4')",
                [&fid],
                |r| r.get(0),
            )?;
            let unread: i64 = conn.query_row(
                "SELECT COUNT(*) FROM messages WHERE receiver = ?1 AND sender = ?2 AND read = 0",
                params![id, fid],
                |r| r.get(0),
            )?;
            friends.push(Friend {
                verified,
                unread: unread.clamp(0, i64::from(u32::MAX)) as u32,
                accounts: if show_accounts {
                    accounts_of(&conn, &fid)?
                } else {
                    Vec::new()
                },
                online: share_online && (launcher || game.is_some()),
                in_game,
                server: game.flatten().filter(|_| in_game && share_server),
                takes_invites: allow_invites,
                id: fid,
                name,
            });
        }
        friends.sort_by(|a, b| {
            (b.in_game, b.online, a.name.to_lowercase()).cmp(&(
                a.in_game,
                a.online,
                b.name.to_lowercase(),
            ))
        });
        let people = |sql: &str| -> rusqlite::Result<Vec<Person>> {
            let mut stmt = conn.prepare(sql)?;
            stmt.query_map([id], |r| {
                Ok(Person {
                    id: r.get(0)?,
                    name: r.get(1)?,
                })
            })?
            .collect()
        };
        let incoming = people(
            "SELECT p.id, p.name FROM friend_requests r JOIN arctic_profiles p ON p.id = r.sender
             WHERE r.receiver = ?1 ORDER BY r.created",
        )?;
        let outgoing = people(
            "SELECT p.id, p.name FROM friend_requests r JOIN arctic_profiles p ON p.id = r.receiver
             WHERE r.sender = ?1 ORDER BY r.created",
        )?;
        Ok((friends, incoming, outgoing))
    }

    /// The launcher is open for this account (or the game says where it is).
    pub fn launcher_online(&self, uuid: &str, now: u64) -> rusqlite::Result<()> {
        self.conn().execute(
            "UPDATE players SET launcher_online = ?2 WHERE uuid = ?1",
            params![uuid, now as i64],
        )?;
        Ok(())
    }

    /// The server the account plays on (`None` = not saying / left).
    pub fn set_server(&self, uuid: &str, server: Option<&str>) -> rusqlite::Result<()> {
        self.conn().execute(
            "UPDATE players SET server = ?2 WHERE uuid = ?1",
            params![uuid, server],
        )?;
        Ok(())
    }

    pub fn invite(
        &self,
        id: &str,
        to: &str,
        kind: &str,
        target: &str,
        now: u64,
    ) -> FResult<String> {
        let conn = self.conn();
        let (a, b) = pair(id, to);
        let friends: bool = conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM friendships WHERE a = ?1 AND b = ?2)",
            params![a, b],
            |r| r.get(0),
        )?;
        if !friends {
            return Err(FriendError::NotFriends);
        }
        let open: bool = conn.query_row(
            "SELECT allow_invites FROM arctic_profiles WHERE id = ?1",
            [to],
            |r| Ok(r.get::<_, i64>(0)? != 0),
        )?;
        if !open {
            return Err(FriendError::NoInvites);
        }
        let recent: i64 = conn.query_row(
            "SELECT COUNT(*) FROM invites WHERE sender = ?1 AND created >= ?2",
            params![id, now.saturating_sub(3600) as i64],
            |r| r.get(0),
        )?;
        if recent >= MAX_INVITES_PER_HOUR {
            return Err(FriendError::TooMany);
        }
        // One live invite per friend: a new one replaces the old.
        conn.execute(
            "DELETE FROM invites WHERE sender = ?1 AND receiver = ?2",
            params![id, to],
        )?;
        let invite = random_id();
        conn.execute(
            "INSERT INTO invites (id, sender, receiver, kind, target, created)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![invite, id, to, kind, target, now as i64],
        )?;
        Ok(invite)
    }

    pub fn invites(&self, id: &str, now: u64) -> FResult<Vec<Invite>> {
        let conn = self.conn();
        conn.execute(
            "DELETE FROM invites WHERE created < ?1",
            [now.saturating_sub(INVITE_SECS) as i64],
        )?;
        let mut stmt = conn.prepare(
            "SELECT i.id, p.id, p.name, i.kind, i.target, i.created FROM invites i
             JOIN arctic_profiles p ON p.id = i.sender WHERE i.receiver = ?1 ORDER BY i.created",
        )?;
        let list = stmt
            .query_map([id], |r| {
                Ok(Invite {
                    id: r.get(0)?,
                    from: Person {
                        id: r.get(1)?,
                        name: r.get(2)?,
                    },
                    kind: r.get(3)?,
                    target: r.get(4)?,
                    sent: r.get::<_, i64>(5)?.max(0) as u64,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(list)
    }

    pub fn invite_dismiss(&self, id: &str, invite: &str) -> FResult<()> {
        let n = self.conn().execute(
            "DELETE FROM invites WHERE id = ?1 AND receiver = ?2",
            params![invite, id],
        )?;
        if n == 0 {
            return Err(FriendError::NotFound);
        }
        Ok(())
    }
}

/// A profile for `uuid`, named after the account, with a fresh friend code.
fn new_profile(tx: &rusqlite::Connection, uuid: &str, now: u64) -> rusqlite::Result<String> {
    let id = random_id();
    let name: String = tx
        .query_row("SELECT name FROM players WHERE uuid = ?1", [uuid], |r| {
            r.get(0)
        })
        .optional()?
        .unwrap_or_else(|| "Player".into());
    tx.execute(
        "INSERT INTO arctic_profiles (id, name, code, created) VALUES (?1, ?2, ?3, ?4)",
        params![id, name, random_code(CODE_LEN), now as i64],
    )?;
    tx.execute(
        "INSERT INTO profile_accounts (uuid, profile, added) VALUES (?1, ?2, ?3)",
        params![uuid, id, now as i64],
    )?;
    Ok(id)
}

fn profile_of(tx: &rusqlite::Connection, uuid: &str) -> rusqlite::Result<Option<String>> {
    tx.query_row(
        "SELECT profile FROM profile_accounts WHERE uuid = ?1",
        [uuid],
        |r| r.get(0),
    )
    .optional()
}

fn accounts_of(conn: &rusqlite::Connection, id: &str) -> rusqlite::Result<Vec<Account>> {
    let mut stmt = conn.prepare(
        "SELECT a.uuid, COALESCE(p.name, '') FROM profile_accounts a
         LEFT JOIN players p ON p.uuid = a.uuid WHERE a.profile = ?1 ORDER BY a.added",
    )?;
    stmt.query_map([id], |r| {
        Ok(Account {
            uuid: r.get(0)?,
            name: r.get(1)?,
        })
    })?
    .collect()
}

pub(crate) fn are_friends(tx: &rusqlite::Connection, x: &str, y: &str) -> rusqlite::Result<bool> {
    let (a, b) = pair(x, y);
    tx.query_row(
        "SELECT EXISTS (SELECT 1 FROM friendships WHERE a = ?1 AND b = ?2)",
        params![a, b],
        |r| r.get(0),
    )
}

fn friend_count(tx: &rusqlite::Connection, id: &str) -> rusqlite::Result<i64> {
    tx.query_row(
        "SELECT COUNT(*) FROM friendships WHERE a = ?1 OR b = ?1",
        [id],
        |r| r.get(0),
    )
}

fn befriend(tx: &rusqlite::Connection, x: &str, y: &str, now: u64) -> rusqlite::Result<()> {
    let (a, b) = pair(x, y);
    tx.execute(
        "INSERT OR IGNORE INTO friendships (a, b, since) VALUES (?1, ?2, ?3)",
        params![a, b, now as i64],
    )?;
    tx.execute(
        "DELETE FROM friend_requests WHERE (sender = ?1 AND receiver = ?2) OR (sender = ?2 AND receiver = ?1)",
        params![x, y],
    )?;
    Ok(())
}

/// Move everything of profile `from` into `into`, then drop `from`.
/// Friendships and requests between the two just vanish; duplicates merge.
fn merge(tx: &Transaction, from: &str, into: &str) -> rusqlite::Result<()> {
    tx.execute(
        "UPDATE profile_accounts SET profile = ?2 WHERE profile = ?1",
        params![from, into],
    )?;
    let their_friends: Vec<(String, i64)> = {
        let mut stmt = tx.prepare(
            "SELECT b, since FROM friendships WHERE a = ?1 UNION ALL SELECT a, since FROM friendships WHERE b = ?1",
        )?;
        stmt.query_map([from], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?
    };
    tx.execute("DELETE FROM friendships WHERE a = ?1 OR b = ?1", [from])?;
    for (friend, since) in their_friends {
        if friend != into {
            let (a, b) = pair(into, &friend);
            tx.execute(
                "INSERT OR IGNORE INTO friendships (a, b, since) VALUES (?1, ?2, ?3)",
                params![a, b, since],
            )?;
        }
    }
    for (col, other) in [("sender", "receiver"), ("receiver", "sender")] {
        tx.execute(
            &format!(
                "UPDATE OR IGNORE friend_requests SET {col} = ?2 WHERE {col} = ?1 AND {other} != ?2"
            ),
            params![from, into],
        )?;
    }
    tx.execute(
        "DELETE FROM friend_requests WHERE sender = ?1 OR receiver = ?1",
        [from],
    )?;
    // A request to someone who's now a friend is settled.
    tx.execute(
        "DELETE FROM friend_requests WHERE EXISTS (SELECT 1 FROM friendships f
             WHERE (f.a = sender AND f.b = receiver) OR (f.a = receiver AND f.b = sender))",
        [],
    )?;
    tx.execute(
        "DELETE FROM invites WHERE sender = ?1 OR receiver = ?1",
        [from],
    )?;
    tx.execute("DELETE FROM arctic_profiles WHERE id = ?1", [from])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Version-4 UUIDs (Microsoft accounts) and one offline (version 3).
    const ALICE: &str = "aaaaaaaaaaaa4aaaaaaaaaaaaaaaaaaa";
    const ALICE_ALT: &str = "aaaaaaaaaaaa4aaaaaaaaaaaaaaaaaab";
    const BOB: &str = "bbbbbbbbbbbb4bbbbbbbbbbbbbbbbbbb";
    const CAROL: &str = "cccccccccccc4ccccccccccccccccccc";
    const OFFLINE: &str = "dddddddddddd3ddddddddddddddddddd";

    fn store() -> Store {
        let s = Store::memory().unwrap();
        for (u, n) in [
            (ALICE, "Alice"),
            (ALICE_ALT, "AliceAlt"),
            (BOB, "Bob"),
            (CAROL, "Carol"),
            (OFFLINE, "Dan"),
        ] {
            s.touch(u, n, 1).unwrap();
        }
        s
    }

    #[test]
    fn requests_become_friendships() {
        let s = store();
        let alice = s.profile_id(ALICE, 1).unwrap();
        let bob = s.profile_id(BOB, 1).unwrap();
        let dan = s.profile_id(OFFLINE, 1).unwrap();
        assert_eq!(
            s.friend_request(&alice, "alice", 2),
            Err(FriendError::Yourself)
        );
        // Offline names aren't unique: found by friend code, not by name.
        assert_eq!(
            s.friend_request(&alice, "Dan", 2),
            Err(FriendError::NotFound)
        );
        let code = s.profile(&dan).unwrap().code;
        assert_eq!(code.len(), 9);
        assert_eq!(s.friend_request(&alice, &code.to_uppercase(), 2), Ok(false));
        assert!(!s.profile(&dan).unwrap().verified);
        assert!(s.profile(&alice).unwrap().verified);
        assert_eq!(s.friend_request(&alice, "BOB", 2), Ok(false));
        let (_, incoming, _) = s.friends(&bob, 3).unwrap();
        assert_eq!(incoming[0].name, "Alice");
        s.friend_accept(&bob, &alice, 4).unwrap();
        let (friends, incoming, _) = s.friends(&bob, 4).unwrap();
        assert!(incoming.is_empty());
        assert_eq!(friends[0].name, "Alice");
        assert_eq!(
            s.friend_request(&alice, "Bob", 5),
            Err(FriendError::Already)
        );
        // Carol asks first; Alice asking back makes them friends at once.
        s.friend_request(&s.profile_id(CAROL, 1).unwrap(), "Alice", 6)
            .unwrap();
        assert_eq!(s.friend_request(&alice, "Carol", 7), Ok(true));
        assert_eq!(s.friends(&alice, 7).unwrap().0.len(), 2);
        s.friend_remove(&alice, &bob).unwrap();
        assert_eq!(s.friends(&alice, 8).unwrap().0.len(), 1);
    }

    #[test]
    fn presence_follows_privacy() {
        let s = store();
        let alice = s.profile_id(ALICE, 1).unwrap();
        let bob = s.profile_id(BOB, 1).unwrap();
        s.friend_request(&alice, "Bob", 1).unwrap();
        s.friend_accept(&bob, &alice, 1).unwrap();
        s.check_in(BOB, Some(BOB), 100).unwrap();
        s.set_server(BOB, Some("mc.example.net")).unwrap();
        let f = &s.friends(&alice, 110).unwrap().0[0];
        assert!(f.online && f.in_game);
        assert_eq!(f.server.as_deref(), Some("mc.example.net"));
        s.profile_update(&bob, None, &[("share_server", false)])
            .unwrap();
        let f = &s.friends(&alice, 110).unwrap().0[0];
        assert!(f.in_game && f.server.is_none());
        s.profile_update(&bob, None, &[("share_online", false)])
            .unwrap();
        let f = &s.friends(&alice, 110).unwrap().0[0];
        assert!(!f.online && !f.in_game);
        // Long gone.
        s.profile_update(&bob, None, &[("share_online", true)])
            .unwrap();
        assert!(!s.friends(&alice, 100 + ONLINE_WINDOW_SECS + 1).unwrap().0[0].online);
        // Just the launcher open.
        s.launcher_online(BOB, 1000).unwrap();
        let f = &s.friends(&alice, 1000).unwrap().0[0];
        assert!(f.online && !f.in_game);
    }

    #[test]
    fn linking_merges_profiles_and_hides_alts() {
        let s = store();
        let alice = s.profile_id(ALICE, 1).unwrap();
        let alt = s.profile_id(ALICE_ALT, 1).unwrap();
        let bob = s.profile_id(BOB, 1).unwrap();
        let carol = s.profile_id(CAROL, 1).unwrap();
        // The alt has Bob as a friend and a request out to Carol.
        s.friend_request(&alt, "Bob", 1).unwrap();
        s.friend_accept(&bob, &alt, 1).unwrap();
        s.friend_request(&alt, "Carol", 1).unwrap();
        s.profile_link(&alice, ALICE_ALT, 2).unwrap();
        assert_eq!(s.profile_id(ALICE_ALT, 3).unwrap(), alice);
        assert_eq!(s.profile(&alt), Err(FriendError::NotFound));
        let (friends, _, outgoing) = s.friends(&alice, 3).unwrap();
        assert_eq!(friends[0].id, bob);
        assert!(friends[0].accounts.is_empty());
        assert_eq!(outgoing[0].id, carol);
        // Bob sees one Alice; her accounts only once she shows them.
        s.profile_update(&alice, None, &[("show_accounts", true)])
            .unwrap();
        let seen = &s.friends(&bob, 3).unwrap().0[0];
        assert_eq!(seen.accounts.len(), 2);
        // Linking again is a no-op; unlinking gives the alt a fresh start.
        s.profile_link(&alice, ALICE_ALT, 4).unwrap();
        s.profile_unlink(&alice, ALICE_ALT).unwrap();
        assert_ne!(s.profile_id(ALICE_ALT, 5).unwrap(), alice);
        assert_eq!(
            s.profile_unlink(&alice, ALICE),
            Err(FriendError::LastAccount)
        );
        // Offline accounts link too.
        s.profile_link(&alice, OFFLINE, 6).unwrap();
        assert_eq!(s.profile_id(OFFLINE, 6).unwrap(), alice);
    }

    #[test]
    fn invites_only_between_friends_who_take_them() {
        let s = store();
        let alice = s.profile_id(ALICE, 1).unwrap();
        let bob = s.profile_id(BOB, 1).unwrap();
        assert_eq!(
            s.invite(&alice, &bob, "server", "mc.x.net", 1),
            Err(FriendError::NotFriends)
        );
        s.friend_request(&alice, "Bob", 1).unwrap();
        s.friend_accept(&bob, &alice, 1).unwrap();
        s.invite(&alice, &bob, "server", "mc.x.net", 10).unwrap();
        s.invite(&alice, &bob, "server", "mc.y.net", 11).unwrap();
        let got = s.invites(&bob, 12).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(
            (got[0].target.as_str(), got[0].from.name.as_str()),
            ("mc.y.net", "Alice")
        );
        assert!(s.invites(&bob, 11 + INVITE_SECS + 1).unwrap().is_empty());
        s.profile_update(&bob, None, &[("allow_invites", false)])
            .unwrap();
        assert_eq!(
            s.invite(&alice, &bob, "server", "mc.x.net", 20),
            Err(FriendError::NoInvites)
        );
    }

    #[test]
    fn recovery_codes_move_offline_profiles() {
        let s = store();
        let dan = s.profile_id(OFFLINE, 1).unwrap();
        assert!(!s.profile(&dan).unwrap().has_recovery);
        let code = s.profile_new_recovery(&dan).unwrap();
        assert_eq!(code.len(), 19);
        assert!(s.profile(&dan).unwrap().has_recovery);
        assert!(s.profile_recovers(&code, OFFLINE).unwrap());
        assert!(
            s.profile_recovers(&code.replace('-', "").to_uppercase(), OFFLINE)
                .unwrap()
        );
        assert!(!s.profile_recovers(&code, ALICE).unwrap());
        assert!(!s.profile_recovers("nope", OFFLINE).unwrap());
        // A new code replaces the old one.
        let newer = s.profile_new_recovery(&dan).unwrap();
        assert!(!s.profile_recovers(&code, OFFLINE).unwrap());
        assert!(s.profile_recovers(&newer, OFFLINE).unwrap());
    }

    #[test]
    fn requests_per_day_are_capped() {
        let s = store();
        let alice = s.profile_id(ALICE, 1).unwrap();
        let mut codes = Vec::new();
        for i in 0..=MAX_REQUESTS_PER_DAY {
            let uuid = format!("{:012x}4{:019x}", i + 100, i);
            s.touch(&uuid, &format!("P{i}"), 1).unwrap();
            codes.push(s.profile(&s.profile_id(&uuid, 1).unwrap()).unwrap().code);
        }
        for code in &codes[..MAX_REQUESTS_PER_DAY as usize] {
            s.friend_request(&alice, code, 10).unwrap();
        }
        assert_eq!(
            s.friend_request(&alice, codes.last().unwrap(), 10),
            Err(FriendError::TooMany)
        );
        assert_eq!(
            s.friend_request(&alice, codes.last().unwrap(), 11 + DAY),
            Ok(false)
        );
    }
}
