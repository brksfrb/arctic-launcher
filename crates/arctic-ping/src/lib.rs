//! Minecraft's server list ping, as the game does it: addresses with SRV
//! redirects, the status handshake and a round-trip ping, optionally through
//! a SOCKS5 proxy; plus a login probe that tells premium (online-mode)
//! servers from cracked ones. Shared by the launcher and the Arctic server.

mod address;
mod dns;
mod ping;

use base64::Engine;
use serde_json::Value;

pub use address::{Address, DEFAULT_PORT};
pub use ping::{Login, login_check, status_json};

/// Player names shown from a server's sample.
const MAX_SAMPLE: usize = 12;
/// Servers pinged at once.
const PARALLEL: usize = 8;

/// Why a ping failed, in words for players.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(pub String);

impl Error {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

/// A SOCKS5 proxy to connect through (names are resolved by the proxy).
#[derive(Debug, Clone, Copy)]
pub struct Socks<'a> {
    pub host: &'a str,
    pub port: u16,
    /// Empty for none.
    pub username: &'a str,
    pub password: &'a str,
}

/// What a server said when pinged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    /// Version name ("Paper 1.21.8", "Velocity 3.4.0").
    pub version: String,
    pub protocol: i32,
    pub online: u32,
    pub max: u32,
    /// Real player names from the server's sample (not always everyone).
    pub players: Vec<String>,
    /// Message of the day, formatting codes removed.
    pub motd: String,
    pub icon: Option<Vec<u8>>,
    /// Round trip in milliseconds (0 when the server didn't answer the ping).
    pub ping_ms: u32,
}

/// Ping one server (`host`, `host:port`), through `proxy` if given.
pub fn ping(address: &str, proxy: Option<Socks>) -> Result<Status> {
    let parsed = Address::parse(address)
        .ok_or_else(|| Error::new(format!("\"{address}\" isn't a server address")))?;
    let (json, ping_ms) = status_json(&parsed, proxy)?;
    parse_status(&json, ping_ms)
}

/// Ping many servers at once; results come back in the same order.
pub fn ping_all(addresses: &[String], proxy: Option<Socks>) -> Vec<Result<Status>> {
    let mut results: Vec<Option<Result<Status>>> = addresses.iter().map(|_| None).collect();
    for (chunk_addrs, chunk_out) in addresses.chunks(PARALLEL).zip(results.chunks_mut(PARALLEL)) {
        std::thread::scope(|s| {
            for (addr, out) in chunk_addrs.iter().zip(chunk_out.iter_mut()) {
                s.spawn(move || *out = Some(ping(addr, proxy)));
            }
        });
    }
    results
        .into_iter()
        .map(|r| r.unwrap_or_else(|| Err(Error::new("not pinged"))))
        .collect()
}

/// A status response (`json`) as a [`Status`].
pub fn parse_status(json: &str, ping_ms: u32) -> Result<Status> {
    let v: Value = serde_json::from_str(json)
        .map_err(|_| Error::new("the server's status couldn't be read"))?;
    let players = &v["players"];
    let count = |key: &str| {
        players[key]
            .as_i64()
            .map_or(0, |n| n.clamp(0, i64::from(u32::MAX)) as u32)
    };
    let sample = players["sample"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter(|p| p["id"].as_str() != Some(NIL_UUID))
                .filter_map(|p| p["name"].as_str())
                .filter(|n| is_username(n))
                .map(str::to_owned)
                .take(MAX_SAMPLE)
                .collect()
        })
        .unwrap_or_default();
    let icon = v["favicon"]
        .as_str()
        .and_then(|f| f.split_once("base64,"))
        .and_then(|(_, b64)| {
            let clean: String = b64.chars().filter(|c| !c.is_whitespace()).collect();
            base64::engine::general_purpose::STANDARD.decode(clean).ok()
        });
    Ok(Status {
        version: strip_codes(v["version"]["name"].as_str().unwrap_or_default()),
        protocol: v["version"]["protocol"].as_i64().unwrap_or(-1) as i32,
        online: count("online"),
        max: count("max"),
        players: sample,
        motd: tidy_motd(&component_text(&v["description"])),
        icon,
        ping_ms,
    })
}

/// Servers put messages ("Store > example.net") in the player sample with
/// a nil id; real players have Minecraft usernames.
const NIL_UUID: &str = "00000000-0000-0000-0000-000000000000";

fn is_username(name: &str) -> bool {
    (3..=16).contains(&name.len()) && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Plain text of a chat component (string, object with `extra`, or array).
pub(crate) fn component_text(v: &Value) -> String {
    fn walk(v: &Value, out: &mut String, depth: usize) {
        if depth > 32 {
            return;
        }
        match v {
            Value::String(s) => out.push_str(s),
            Value::Array(items) => items.iter().for_each(|i| walk(i, out, depth + 1)),
            Value::Object(map) => {
                if let Some(t) = map.get("text").and_then(Value::as_str) {
                    out.push_str(t);
                } else if let Some(t) = map.get("translate").and_then(Value::as_str) {
                    out.push_str(t);
                }
                if let Some(extra) = map.get("extra") {
                    walk(extra, out, depth + 1);
                }
            }
            _ => {}
        }
    }
    let mut out = String::new();
    walk(v, &mut out, 0);
    out
}

/// Remove `§x` formatting codes.
fn strip_codes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '§' {
            chars.next();
        } else {
            out.push(c);
        }
    }
    out
}

/// Codes removed, each line trimmed, at most two lines.
pub(crate) fn tidy_motd(s: &str) -> String {
    strip_codes(s)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .take(2)
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_status_json() {
        let json = r#"{
            "version": {"name": "§6Paper 1.21.8", "protocol": 772},
            "players": {"max": 100, "online": 3, "sample": [
                {"name": "Alice", "id": "x"}, {"name": "Bob_2", "id": "y"},
                {"name": "§aStore > shop.net", "id": "z"},
                {"name": "Fake", "id": "00000000-0000-0000-0000-000000000000"}]},
            "description": {"text": "§bWelcome ", "extra": [{"text": "home"}, "\n  line two  \nthree"]},
            "favicon": "data:image/png;base64,iVBORw0KGgo="
        }"#;
        let s = parse_status(json, 42).unwrap();
        assert_eq!(s.version, "Paper 1.21.8");
        assert_eq!((s.online, s.max), (3, 100));
        assert_eq!(s.players, vec!["Alice", "Bob_2"]);
        assert_eq!(s.motd, "Welcome home\nline two");
        assert_eq!(s.icon.as_deref(), Some(&b"\x89PNG\r\n\x1a\n"[..]));
        assert_eq!(s.ping_ms, 42);
        let plain = parse_status(r#"{"description":"Hi","players":{}}"#, 0).unwrap();
        assert_eq!((plain.motd.as_str(), plain.online), ("Hi", 0));
    }

    #[test]
    fn pings_all_in_order() {
        let a = ping::tests::fake_server(r#"{"description":"A","players":{"online":1,"max":2}}"#);
        let b = ping::tests::fake_server(r#"{"description":"B","players":{"online":5,"max":9}}"#);
        let addrs = vec![
            format!("127.0.0.1:{a}"),
            "host:notaport".to_owned(),
            format!("127.0.0.1:{b}"),
        ];
        let results = ping_all(&addrs, None);
        assert_eq!(results[0].as_ref().unwrap().motd, "A");
        assert!(results[1].is_err());
        assert_eq!(results[2].as_ref().unwrap().online, 5);
    }
}
