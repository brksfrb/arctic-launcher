//! The public server list: servers from the curated seed
//! (`assets/servers.json`) and ones their owners submitted, proved by a
//! code in the MOTD and approved by an admin. Everyone gets them in a new
//! random order each time; nobody can pay for a better spot.

use std::path::Path;

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::store::Store;

/// Hidden from the list after this long without answering a ping.
pub const OFFLINE_AFTER: u64 = 3 * 24 * 60 * 60;
/// Submissions per player per day.
pub const MAX_PER_DAY: i64 = 5;
pub const MAX_NAME: usize = 40;
pub const MAX_DESCRIPTION: usize = 200;
pub const MAX_TAGS: usize = 5;
pub const MAX_TAG: usize = 16;
const DAY: u64 = 24 * 60 * 60;
const CODE_PREFIX: &str = "arctic-";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// From the seed file.
    Curated,
    /// Submitted; the MOTD code hasn't been seen yet.
    Unverified,
    /// Code seen; waiting for an admin.
    Pending,
    Listed,
    Rejected,
}

impl State {
    pub fn as_str(self) -> &'static str {
        match self {
            State::Curated => "curated",
            State::Unverified => "unverified",
            State::Pending => "pending",
            State::Listed => "listed",
            State::Rejected => "rejected",
        }
    }

    fn parse(s: &str) -> Self {
        match s {
            "curated" => State::Curated,
            "pending" => State::Pending,
            "listed" => State::Listed,
            "rejected" => State::Rejected,
            _ => State::Unverified,
        }
    }
}

/// A server as stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Listing {
    pub id: String,
    pub address: String,
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    #[serde(skip)]
    pub state: State,
    /// `None` until checked.
    pub cracked: Option<bool>,
    pub online: bool,
    pub players: u32,
    pub max_players: u32,
    pub version: String,
    #[serde(skip)]
    pub last_seen: u64,
    #[serde(skip)]
    pub owner: Option<String>,
    #[serde(skip)]
    pub code: String,
    /// Empty unless this server is a partner's (a sponsor or hosting partner).
    pub partner: String,
    /// `exclusive` (our own servers, on top of every list), `hosted` (hosted with
    /// Flash Hosting; their own tab) or empty.
    pub partner_tier: String,
}

/// One entry of `servers.json`.
#[derive(Debug, Clone, Deserialize)]
pub struct Seed {
    pub address: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    /// A sponsor or hosting partner's name; shown in its own section.
    #[serde(default)]
    pub partner: String,
    /// `exclusive` or `hosted` (the default for a partner).
    #[serde(default)]
    pub partner_tier: String,
}

/// Live numbers from a ping.
#[derive(Debug, Clone, Default)]
pub struct Live {
    pub players: u32,
    pub max_players: u32,
    pub version: String,
}

#[derive(Debug)]
pub enum SubmitError {
    /// Someone else already submitted (or it's curated).
    Taken,
    TooMany,
    Db(rusqlite::Error),
}

impl From<rusqlite::Error> for SubmitError {
    fn from(e: rusqlite::Error) -> Self {
        SubmitError::Db(e)
    }
}

const COLUMNS: &str = "id, address, name, description, tags, state, cracked, online, players,
     max_players, version, last_seen, owner, code, partner, partner_tier";

fn row(r: &rusqlite::Row) -> rusqlite::Result<Listing> {
    let tags: String = r.get(4)?;
    let state: String = r.get(5)?;
    let cracked: Option<i64> = r.get(6)?;
    Ok(Listing {
        id: r.get(0)?,
        address: r.get(1)?,
        name: r.get(2)?,
        description: r.get(3)?,
        tags: serde_json::from_str(&tags).unwrap_or_default(),
        state: State::parse(&state),
        cracked: cracked.map(|c| c != 0),
        online: r.get::<_, i64>(7)? != 0,
        players: r.get::<_, i64>(8)?.clamp(0, i64::from(u32::MAX)) as u32,
        max_players: r.get::<_, i64>(9)?.clamp(0, i64::from(u32::MAX)) as u32,
        version: r.get(10)?,
        last_seen: r.get::<_, i64>(11)?.max(0) as u64,
        owner: r.get(12)?,
        code: r.get(13)?,
        partner: r.get(14)?,
        partner_tier: r.get(15)?,
    })
}

/// Normalized address (`Play.X.net:25565` → `play.x.net`), if valid.
pub fn clean_address(text: &str) -> Option<String> {
    arctic_ping::Address::parse(text).map(|a| a.display())
}

/// Tags: lowercase letters, digits and dashes; duplicates and bad ones dropped.
pub fn clean_tags(tags: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in tags {
        let t = t.trim().to_ascii_lowercase();
        let ok = !t.is_empty()
            && t.len() <= MAX_TAG
            && t.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if ok && !out.contains(&t) && out.len() < MAX_TAGS {
            out.push(t);
        }
    }
    out
}

/// The owner's code shows in the MOTD (formatting codes are already gone).
pub fn motd_has_code(motd: &str, code: &str) -> bool {
    motd.to_ascii_lowercase()
        .contains(&code.to_ascii_lowercase())
}

fn random_id(len: usize) -> String {
    const ALPHABET: &[u8] = b"abcdefghjkmnpqrstuvwxyz23456789";
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

impl Store {
    pub(crate) fn migrate_listing(&self) -> rusqlite::Result<()> {
        self.conn().execute_batch(
            "CREATE TABLE IF NOT EXISTS listed_servers (
                 id TEXT PRIMARY KEY,
                 address TEXT NOT NULL UNIQUE,
                 name TEXT NOT NULL,
                 description TEXT NOT NULL DEFAULT '',
                 tags TEXT NOT NULL DEFAULT '[]',
                 state TEXT NOT NULL,
                 cracked INTEGER,
                 online INTEGER NOT NULL DEFAULT 0,
                 players INTEGER NOT NULL DEFAULT 0,
                 max_players INTEGER NOT NULL DEFAULT 0,
                 version TEXT NOT NULL DEFAULT '',
                 last_seen INTEGER NOT NULL DEFAULT 0,
                 checked INTEGER NOT NULL DEFAULT 0,
                 owner TEXT,
                 code TEXT NOT NULL,
                 created INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS listed_servers_owner ON listed_servers (owner, created);",
        )?;
        let conn = self.conn();
        let has_partner = {
            let mut stmt = conn.prepare("SELECT name FROM pragma_table_info('listed_servers')")?;
            let names = stmt
                .query_map([], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            names.iter().any(|n| n == "partner")
        };
        if !has_partner {
            conn.execute_batch(
                "ALTER TABLE listed_servers ADD COLUMN partner TEXT NOT NULL DEFAULT ''",
            )?;
        }
        let has_tier = {
            let mut stmt = conn.prepare("SELECT name FROM pragma_table_info('listed_servers')")?;
            let names = stmt
                .query_map([], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            names.iter().any(|n| n == "partner_tier")
        };
        if !has_tier {
            conn.execute_batch(
                "ALTER TABLE listed_servers ADD COLUMN partner_tier TEXT NOT NULL DEFAULT ''",
            )?;
        }
        // A partner with no tier is a hosted one.
        conn.execute_batch(
            "UPDATE listed_servers SET partner_tier = 'hosted' WHERE partner != '' AND partner_tier = ''",
        )
    }

    /// Make a server a partner's (shown in the partners section), or an
    /// ordinary one again with an empty name. `Ok(false)`: no such server.
    pub fn listing_set_partner(
        &self,
        id: &str,
        partner: &str,
        tier: &str,
    ) -> rusqlite::Result<bool> {
        let name: String = partner
            .chars()
            .filter(|c| !c.is_control())
            .take(MAX_NAME)
            .collect::<String>()
            .trim()
            .to_owned();
        let tier = match (name.is_empty(), tier.trim()) {
            (true, _) => "",
            (false, "exclusive") => "exclusive",
            (false, _) => "hosted",
        };
        Ok(self.conn().execute(
            "UPDATE listed_servers SET partner = ?1, partner_tier = ?2 WHERE id = ?3",
            params![name, tier, id],
        )? > 0)
    }

    /// Add or refresh the curated servers (their live numbers are kept).
    pub fn listing_seed(&self, seeds: &[Seed], now: u64) -> rusqlite::Result<usize> {
        let conn = self.conn();
        let mut count = 0;
        for seed in seeds {
            let Some(address) = clean_address(&seed.address) else {
                log::warn!("servers.json: bad address {:?}", seed.address);
                continue;
            };
            let tags = serde_json::to_string(&clean_tags(&seed.tags)).unwrap_or_default();
            conn.execute(
                "INSERT INTO listed_servers (id, address, name, description, tags, state, code, created, partner, partner_tier)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'curated', '', ?6, ?7, ?8)
                 ON CONFLICT(address) DO UPDATE SET name = excluded.name,
                     description = excluded.description, tags = excluded.tags, state = 'curated',
                     partner = CASE WHEN excluded.partner != '' THEN excluded.partner ELSE partner END,
                     partner_tier = CASE WHEN excluded.partner_tier != '' THEN excluded.partner_tier ELSE partner_tier END",
                params![
                    random_id(8),
                    address,
                    seed.name.chars().take(MAX_NAME).collect::<String>(),
                    seed.description.chars().take(MAX_DESCRIPTION).collect::<String>(),
                    tags,
                    now as i64,
                    seed.partner.chars().take(MAX_NAME).collect::<String>(),
                    match (seed.partner.is_empty(), seed.partner_tier.as_str()) {
                        (true, _) => "",
                        (false, "exclusive") => "exclusive",
                        (false, _) => "hosted",
                    }
                ],
            )?;
            count += 1;
        }
        Ok(count)
    }

    /// What players see: curated and approved servers seen lately.
    pub fn listing_public(&self, now: u64) -> rusqlite::Result<Vec<Listing>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(&format!(
            "SELECT {COLUMNS} FROM listed_servers
             WHERE state IN ('curated', 'listed') AND last_seen > 0 AND last_seen >= ?1"
        ))?;
        let since = now.saturating_sub(OFFLINE_AFTER) as i64;
        stmt.query_map(params![since], row)?.collect()
    }

    pub fn listing_in_state(&self, state: State) -> rusqlite::Result<Vec<Listing>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(&format!(
            "SELECT {COLUMNS} FROM listed_servers WHERE state = ?1 ORDER BY created"
        ))?;
        stmt.query_map(params![state.as_str()], row)?.collect()
    }

    /// Everything the pinger keeps fresh.
    pub fn listing_to_ping(&self) -> rusqlite::Result<Vec<Listing>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(&format!(
            "SELECT {COLUMNS} FROM listed_servers WHERE state IN ('curated', 'listed', 'pending')"
        ))?;
        stmt.query_map([], row)?.collect()
    }

    pub fn listing_get(&self, id: &str) -> rusqlite::Result<Option<Listing>> {
        self.conn()
            .query_row(
                &format!("SELECT {COLUMNS} FROM listed_servers WHERE id = ?1"),
                params![id],
                row,
            )
            .optional()
    }

    /// A new submission (or the owner's own earlier one, still unverified).
    pub fn listing_submit(
        &self,
        owner: &str,
        address: &str,
        name: &str,
        description: &str,
        tags: &[String],
        now: u64,
    ) -> Result<Listing, SubmitError> {
        let existing: Option<(String, String, Option<String>)> = self
            .conn()
            .query_row(
                "SELECT id, state, owner FROM listed_servers WHERE address = ?1",
                params![address],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        if let Some((id, state, who)) = existing {
            let mine = who.as_deref() == Some(owner);
            if !mine || !matches!(State::parse(&state), State::Unverified | State::Rejected) {
                return Err(SubmitError::Taken);
            }
            self.conn().execute(
                "UPDATE listed_servers SET name = ?2, description = ?3, tags = ?4,
                     state = 'unverified' WHERE id = ?1",
                params![
                    id,
                    name,
                    description,
                    serde_json::to_string(tags).unwrap_or_default()
                ],
            )?;
            return self.listing_get(&id)?.ok_or(SubmitError::Taken);
        }
        let today: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM listed_servers WHERE owner = ?1 AND created >= ?2",
            params![owner, now.saturating_sub(DAY) as i64],
            |r| r.get(0),
        )?;
        if today >= MAX_PER_DAY {
            return Err(SubmitError::TooMany);
        }
        let id = random_id(8);
        let code = format!("{CODE_PREFIX}{}", random_id(6));
        self.conn().execute(
            "INSERT INTO listed_servers (id, address, name, description, tags, state, owner, code, created)
             VALUES (?1, ?2, ?3, ?4, ?5, 'unverified', ?6, ?7, ?8)",
            params![
                id,
                address,
                name,
                description,
                serde_json::to_string(tags).unwrap_or_default(),
                owner,
                code,
                now as i64
            ],
        )?;
        self.listing_get(&id)?.ok_or(SubmitError::Taken)
    }

    pub fn listing_set_state(&self, id: &str, state: State) -> rusqlite::Result<bool> {
        Ok(self.conn().execute(
            "UPDATE listed_servers SET state = ?2 WHERE id = ?1",
            params![id, state.as_str()],
        )? > 0)
    }

    pub fn listing_remove(&self, id: &str) -> rusqlite::Result<bool> {
        Ok(self
            .conn()
            .execute("DELETE FROM listed_servers WHERE id = ?1", params![id])?
            > 0)
    }

    /// A ping's outcome (`None` = no answer).
    pub fn listing_pinged(&self, id: &str, live: Option<&Live>, now: u64) -> rusqlite::Result<()> {
        match live {
            Some(l) => self.conn().execute(
                "UPDATE listed_servers SET online = 1, players = ?2, max_players = ?3,
                     version = ?4, last_seen = ?5, checked = ?5 WHERE id = ?1",
                params![id, l.players, l.max_players, l.version, now as i64],
            ),
            None => self.conn().execute(
                "UPDATE listed_servers SET online = 0, checked = ?2 WHERE id = ?1",
                params![id, now as i64],
            ),
        }?;
        Ok(())
    }

    pub fn listing_set_cracked(&self, id: &str, cracked: bool) -> rusqlite::Result<()> {
        self.conn().execute(
            "UPDATE listed_servers SET cracked = ?2 WHERE id = ?1",
            params![id, i64::from(cracked)],
        )?;
        Ok(())
    }
}

/// `servers.json` from the assets folder (missing = no curated servers).
pub fn load_seed(assets: &Path) -> Result<Vec<Seed>, String> {
    let path = assets.join("servers.json");
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed(address: &str) -> Seed {
        Seed {
            address: address.into(),
            name: "Seed".into(),
            description: String::new(),
            tags: vec!["Minigames".into(), "bad tag!".into()],
            partner: String::new(),
            partner_tier: String::new(),
        }
    }

    #[test]
    fn partners_are_set_cleared_and_survive_reseeding() {
        let store = Store::memory().unwrap();
        store.listing_seed(&[seed("a.example.net")], 100).unwrap();
        let id = store.listing_in_state(State::Curated).unwrap()[0]
            .id
            .clone();
        assert!(
            store
                .listing_set_partner(&id, "  Flash Hosting ", "exclusive")
                .unwrap()
        );
        assert_eq!(
            store.listing_in_state(State::Curated).unwrap()[0].partner,
            "Flash Hosting"
        );
        // The seed file saying nothing about partners doesn't undo it.
        store.listing_seed(&[seed("a.example.net")], 200).unwrap();
        assert_eq!(
            store.listing_in_state(State::Curated).unwrap()[0].partner,
            "Flash Hosting"
        );
        assert!(store.listing_set_partner(&id, "", "").unwrap());
        assert_eq!(
            store.listing_in_state(State::Curated).unwrap()[0].partner,
            ""
        );
        assert!(!store.listing_set_partner("nope", "x", "").unwrap());
    }

    #[test]
    fn seeds_and_shows_only_servers_seen_lately() {
        let store = Store::memory().unwrap();
        assert_eq!(
            store
                .listing_seed(&[seed("A.example.net:25565"), seed("host:0")], 100)
                .unwrap(),
            1
        );
        let all = store.listing_to_ping().unwrap();
        let a = all.iter().find(|l| l.address == "a.example.net").unwrap();
        assert_eq!(a.tags, vec!["minigames"]);
        // Never answered: hidden.
        assert!(
            store
                .listing_public(1000)
                .unwrap()
                .iter()
                .all(|l| l.id != a.id)
        );
        let live = Live {
            players: 5,
            max_players: 10,
            version: "1.21".into(),
        };
        store.listing_pinged(&a.id, Some(&live), 1000).unwrap();
        let shown = store.listing_public(1000).unwrap();
        assert!(
            shown
                .iter()
                .any(|l| l.id == a.id && l.players == 5 && l.online)
        );
        // Silent for too long: hidden again.
        assert!(
            store
                .listing_public(1000 + OFFLINE_AFTER + 1)
                .unwrap()
                .iter()
                .all(|l| l.id != a.id)
        );
    }

    #[test]
    fn submissions_are_owned_and_capped() {
        let store = Store::memory().unwrap();
        let first = store
            .listing_submit("alice", "mc.x.net", "X", "", &[], 10)
            .unwrap();
        assert_eq!(first.state, State::Unverified);
        assert!(first.code.starts_with("arctic-"));
        // Resubmitting your own unverified server keeps its code.
        let again = store
            .listing_submit("alice", "mc.x.net", "X2", "", &[], 11)
            .unwrap();
        assert_eq!(
            (again.id.as_str(), again.code.as_str(), again.name.as_str()),
            (first.id.as_str(), first.code.as_str(), "X2")
        );
        assert!(matches!(
            store.listing_submit("bob", "mc.x.net", "Mine", "", &[], 12),
            Err(SubmitError::Taken)
        ));
        for i in 0..4 {
            store
                .listing_submit("alice", &format!("s{i}.x.net"), "S", "", &[], 20)
                .unwrap();
        }
        assert!(matches!(
            store.listing_submit("alice", "late.x.net", "L", "", &[], 30),
            Err(SubmitError::TooMany)
        ));
        // Unverified and pending never show.
        store.listing_set_state(&first.id, State::Pending).unwrap();
        store
            .listing_pinged(&first.id, Some(&Live::default()), 40)
            .unwrap();
        assert!(store.listing_public(40).unwrap().is_empty());
        store.listing_set_state(&first.id, State::Listed).unwrap();
        assert_eq!(store.listing_public(40).unwrap().len(), 1);
    }

    #[test]
    fn codes_and_tags() {
        assert!(motd_has_code("Welcome! ARCTIC-ab12cd", "arctic-ab12cd"));
        assert!(!motd_has_code("Welcome", "arctic-ab12cd"));
        assert_eq!(
            clean_address("Play.Example.NET:25565").as_deref(),
            Some("play.example.net")
        );
        assert_eq!(
            clean_tags(&["PvP".into(), "pvp".into(), "x".repeat(20)]),
            vec!["pvp"]
        );
    }
}
