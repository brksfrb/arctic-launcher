//! The launcher bridge: lets a running Arctic Client switch accounts in game.
//!
//! A tiny HTTP server on 127.0.0.1 (random port) answers only requests with
//! the secret written into that game's session file. The game can list the
//! accounts (names, never tokens) and ask for a fresh session for one; the
//! launcher refreshes the login through its usual path and hands back just
//! what Minecraft needs to play as that account.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;

use crate::auth::LaunchIdentity;
use crate::{Error, Result};

/// Where the game reaches the bridge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BridgeInfo {
    pub port: u16,
    pub secret: String,
}

/// An account the game may switch to (no secrets).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AccountEntry {
    pub id: String,
    pub name: String,
    pub uuid: String,
    pub microsoft: bool,
    pub active: bool,
}

/// What the game needs to play as an account.
#[derive(Debug, Clone)]
pub struct SessionGrant {
    pub identity: LaunchIdentity,
    /// A session for the Arctic cosmetics server, if one could be made.
    pub arctic_token: Option<String>,
}

/// The launcher side: its accounts and how to get a fresh session.
pub trait Accounts: Send + Sync + 'static {
    fn list(&self) -> Vec<AccountEntry>;
    fn session(&self, id: &str) -> Result<SessionGrant>;
    /// Start adding an account: the launcher opens Microsoft's sign-in in
    /// the browser. Credentials never go through the game.
    fn add(&self) -> Result<()> {
        Err(crate::Error::Other(
            "add accounts in Arctic Launcher (or with `arctic accounts login`)".into(),
        ))
    }
    /// Put a screenshot (a `.png` in a game's `screenshots` folder) on the
    /// clipboard as a picture, which the game itself can't always do.
    fn copy_image(&self, _path: &str) -> Result<()> {
        Err(crate::Error::Other(
            "copying pictures needs Arctic Launcher".into(),
        ))
    }

    /// How the last [`Accounts::add`] is going (JSON): `{"state": "idle" |
    /// "waiting" | "done" | "failed", "name"?, "message"?}`.
    fn login(&self) -> String {
        r#"{"state":"idle"}"#.to_owned()
    }

    /// A small PNG of a web image (Modrinth icons are often WebP, which the
    /// game can't read). Only Modrinth's CDN is fetched.
    fn icon(&self, _url: &str) -> Result<Vec<u8>> {
        Err(crate::Error::Other(
            "icons come from Arctic Launcher".into(),
        ))
    }

    /// Proximity voice chat: the game reports where everyone is (JSON);
    /// the answer says who's speaking. Only the launcher does voice.
    fn voice(&self, _state: &[u8]) -> Result<String> {
        Err(crate::Error::Other(
            "voice chat runs in Arctic Launcher".into(),
        ))
    }

    /// FFmpeg for replay videos (JSON): `{"state": "ready" | "missing" |
    /// "downloading" | "failed" | "unavailable", "path"?, "progress"?, "error"?}`.
    /// `start` asks for the download.
    fn ffmpeg(&self, _start: bool) -> String {
        r#"{"state":"unavailable"}"#.to_owned()
    }

    /// Play together from the game: `{"action": "host" | "join" | "stop" |
    /// "status", "code": …}`; the answer is the current state.
    fn together(&self, _request: &[u8]) -> Result<String> {
        Err(crate::Error::Other(
            "play together runs in Arctic Launcher".into(),
        ))
    }
}

const READ_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_HEADER_LINES: usize = 64;
const MAX_LINE: usize = 8 * 1024;
/// Voice state lists nearby players, so bodies can be a few KB.
const MAX_BODY: usize = 32 * 1024;

/// Start the bridge on a random local port.
pub fn start(accounts: impl Accounts) -> Result<BridgeInfo> {
    let listener =
        TcpListener::bind(("127.0.0.1", 0)).map_err(|e| Error::Other(format!("bridge: {e}")))?;
    let port = listener
        .local_addr()
        .map_err(|e| Error::Other(format!("bridge: {e}")))?
        .port();
    let secret =
        uuid::Uuid::new_v4().simple().to_string() + &uuid::Uuid::new_v4().simple().to_string();
    let accounts: Arc<dyn Accounts> = Arc::new(accounts);
    let expected = secret.clone();
    std::thread::Builder::new()
        .name("arctic-bridge".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                let accounts = Arc::clone(&accounts);
                let expected = expected.clone();
                std::thread::spawn(move || {
                    if let Err(e) = serve(stream, &*accounts, &expected) {
                        log::debug!("bridge request: {e}");
                    }
                });
            }
        })
        .map_err(|e| Error::Other(format!("bridge: {e}")))?;
    Ok(BridgeInfo { port, secret })
}

struct Request {
    method: String,
    path: String,
    authorized: bool,
    body: Vec<u8>,
}

fn serve(stream: TcpStream, accounts: &dyn Accounts, secret: &str) -> std::io::Result<()> {
    stream.set_read_timeout(Some(READ_TIMEOUT))?;
    stream.set_write_timeout(Some(READ_TIMEOUT))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let request = read_request(&mut reader, secret)?;
    let (status, body) = respond(&request, accounts);
    let mut out = stream;
    write!(
        out,
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )?;
    out.flush()
}

fn read_request(reader: &mut impl BufRead, secret: &str) -> std::io::Result<Request> {
    let bad = || std::io::Error::new(std::io::ErrorKind::InvalidData, "bad request");
    let mut line = String::new();
    read_line(reader, &mut line)?;
    let mut parts = line.split_whitespace();
    let method = parts.next().ok_or_else(bad)?.to_owned();
    let path = parts.next().ok_or_else(bad)?.to_owned();
    let mut authorized = false;
    let mut length = 0usize;
    for _ in 0..MAX_HEADER_LINES {
        line.clear();
        read_line(reader, &mut line)?;
        let header = line.trim_end();
        if header.is_empty() {
            break;
        }
        let Some((name, value)) = header.split_once(':') else {
            return Err(bad());
        };
        let value = value.trim();
        if name.eq_ignore_ascii_case("authorization") {
            authorized = value
                .strip_prefix("Bearer ")
                .is_some_and(|token| same(token.as_bytes(), secret.as_bytes()));
        } else if name.eq_ignore_ascii_case("content-length") {
            length = value.parse().map_err(|_| bad())?;
        }
    }
    if length > MAX_BODY {
        return Err(bad());
    }
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body)?;
    Ok(Request {
        method,
        path,
        authorized,
        body,
    })
}

fn read_line(reader: &mut impl BufRead, line: &mut String) -> std::io::Result<()> {
    let n = reader.take(MAX_LINE as u64).read_line(line)?;
    if n == 0 || !line.ends_with('\n') {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "line too long or cut off",
        ));
    }
    Ok(())
}

/// Comparison that takes the same time wherever the first difference is.
fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn respond(request: &Request, accounts: &dyn Accounts) -> (&'static str, String) {
    let error =
        |status, message: &str| (status, serde_json::json!({ "error": message }).to_string());
    if !request.authorized {
        return error("401 Unauthorized", "wrong secret");
    }
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/v1/accounts") => (
            "200 OK",
            serde_json::to_string(&accounts.list()).unwrap_or_default(),
        ),
        ("GET", "/v1/accounts/login") => ("200 OK", accounts.login()),
        ("POST", "/v1/accounts/add") => match accounts.add() {
            Ok(()) => ("200 OK", "{}".to_owned()),
            Err(e) => error("501 Not Implemented", &e.to_string()),
        },
        ("POST", "/v1/clipboard") => {
            let path = serde_json::from_slice::<serde_json::Value>(&request.body)
                .ok()
                .and_then(|v| v.get("path").and_then(|p| p.as_str()).map(str::to_owned));
            match path.map(|p| accounts.copy_image(&p)) {
                Some(Ok(())) => ("200 OK", "{}".to_owned()),
                Some(Err(e)) => error("400 Bad Request", &e.to_string()),
                None => error("400 Bad Request", "no path"),
            }
        }
        ("POST", "/v1/icon") => {
            let url = serde_json::from_slice::<serde_json::Value>(&request.body)
                .ok()
                .and_then(|v| v.get("url").and_then(|u| u.as_str()).map(str::to_owned));
            match url.map(|u| accounts.icon(&u)) {
                Some(Ok(png)) => {
                    use base64::Engine as _;
                    let data = base64::engine::general_purpose::STANDARD.encode(png);
                    ("200 OK", serde_json::json!({ "png": data }).to_string())
                }
                Some(Err(e)) => error("502 Bad Gateway", &e.to_string()),
                None => error("400 Bad Request", "no url"),
            }
        }
        ("GET", "/v1/ffmpeg") => ("200 OK", accounts.ffmpeg(false)),
        ("POST", "/v1/ffmpeg") => ("200 OK", accounts.ffmpeg(true)),
        ("POST", "/v1/voice") => match accounts.voice(&request.body) {
            Ok(reply) => ("200 OK", reply),
            Err(e) => error("501 Not Implemented", &e.to_string()),
        },
        ("POST", "/v1/together") => match accounts.together(&request.body) {
            Ok(reply) => ("200 OK", reply),
            Err(e) => error("501 Not Implemented", &e.to_string()),
        },
        ("POST", path) => {
            let Some(id) = path
                .strip_prefix("/v1/accounts/")
                .and_then(|rest| rest.strip_suffix("/session"))
            else {
                return error("404 Not Found", "not found");
            };
            match accounts.session(id) {
                Ok(grant) => (
                    "200 OK",
                    serde_json::json!({
                        "name": grant.identity.username,
                        "uuid": grant.identity.uuid,
                        "access_token": grant.identity.access_token,
                        "xuid": grant.identity.xuid,
                        "user_type": grant.identity.user_type,
                        "arctic_token": grant.arctic_token,
                    })
                    .to_string(),
                ),
                Err(e) => error("502 Bad Gateway", &e.to_string()),
            }
        }
        _ => error("404 Not Found", "not found"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake;

    impl Accounts for Fake {
        fn list(&self) -> Vec<AccountEntry> {
            vec![AccountEntry {
                id: "a1".into(),
                name: "Alice".into(),
                uuid: "u1".into(),
                microsoft: true,
                active: true,
            }]
        }

        fn session(&self, id: &str) -> Result<SessionGrant> {
            if id != "a1" {
                return Err(Error::Other("no such account".into()));
            }
            Ok(SessionGrant {
                identity: LaunchIdentity {
                    username: "Alice".into(),
                    uuid: "u1".into(),
                    access_token: "tok".into(),
                    user_type: "msa",
                    xuid: "x".into(),
                },
                arctic_token: None,
            })
        }
    }

    fn call(info: &BridgeInfo, method: &str, path: &str, secret: &str) -> String {
        let mut s = TcpStream::connect(("127.0.0.1", info.port)).unwrap();
        write!(
            s,
            "{method} {path} HTTP/1.1\r\nHost: x\r\nAuthorization: Bearer {secret}\r\nContent-Length: 0\r\n\r\n"
        )
        .unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        out
    }

    #[test]
    fn lists_and_grants_only_with_the_secret() {
        let info = start(Fake).unwrap();
        let listed = call(&info, "GET", "/v1/accounts", &info.secret);
        assert!(listed.starts_with("HTTP/1.1 200") && listed.contains("\"Alice\""));
        assert!(!listed.contains("tok"), "the list never has tokens");
        let granted = call(&info, "POST", "/v1/accounts/a1/session", &info.secret);
        assert!(granted.contains("\"access_token\":\"tok\""));
        assert!(call(&info, "GET", "/v1/accounts", "wrong").starts_with("HTTP/1.1 401"));
        assert!(
            call(&info, "POST", "/v1/accounts/zz/session", &info.secret)
                .starts_with("HTTP/1.1 502")
        );
        assert!(call(&info, "GET", "/v1/other", &info.secret).starts_with("HTTP/1.1 404"));
    }

    #[test]
    fn secrets_compare_fully() {
        assert!(same(b"abc", b"abc"));
        assert!(!same(b"abc", b"abd") && !same(b"abc", b"ab"));
    }
}
