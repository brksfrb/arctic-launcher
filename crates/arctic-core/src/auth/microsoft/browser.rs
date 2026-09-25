//! Browser login: authorization code + PKCE with a loopback redirect.
//!
//! We listen on an ephemeral port on 127.0.0.1 (and ::1 when available) and
//! use `http://localhost:<port>` as the redirect URI. For public clients the
//! Microsoft identity platform ignores the port of a registered
//! `http://localhost` redirect, so any free port works.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};

use super::MsaConfig;
use super::oauth::{self, AUTHORIZE_URL, MsaTokens, SCOPE};
use crate::{Error, Result};

const LOGIN_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const ACCEPT_POLL: Duration = Duration::from_millis(100);
const DONE_PAGE: &str = "<!doctype html><html><head><meta charset=\"utf-8\"><title>Arctic Launcher</title></head>\
<body style=\"font-family:sans-serif;background:#0b1320;color:#e6f1ff;text-align:center;padding-top:15vh\">\
<h2>You can close this tab and return to Arctic Launcher.</h2></body></html>";

pub(super) fn authorize(cfg: &MsaConfig, cancel: &AtomicBool) -> Result<MsaTokens> {
    let v4 = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| Error::Auth(format!("cannot open loopback port: {e}")))?;
    let port = v4
        .local_addr()
        .map_err(|e| Error::Auth(e.to_string()))?
        .port();
    // Browsers may resolve `localhost` to ::1 first; listen there too if possible.
    let v6 = TcpListener::bind(("::1", port)).ok();
    let listeners: Vec<&TcpListener> = std::iter::once(&v4).chain(v6.as_ref()).collect();
    for l in &listeners {
        l.set_nonblocking(true)
            .map_err(|e| Error::Auth(e.to_string()))?;
    }

    let redirect_uri = format!("http://localhost:{port}");
    let verifier = random_token();
    let state = random_token();
    let url = format!(
        "{AUTHORIZE_URL}?client_id={}&response_type=code&redirect_uri={}&scope={}&state={}\
         &code_challenge={}&code_challenge_method=S256&prompt=select_account",
        encode(&cfg.client_id),
        encode(&redirect_uri),
        encode(SCOPE),
        state,
        pkce_challenge(&verifier),
    );
    open::that_detached(&url).map_err(|e| Error::Auth(format!("could not open browser: {e}")))?;

    let deadline = Instant::now() + LOGIN_TIMEOUT;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Auth("login cancelled".into()));
        }
        if Instant::now() >= deadline {
            return Err(Error::Auth(
                "timed out waiting for the browser login".into(),
            ));
        }
        for l in &listeners {
            if let Ok((stream, _)) = l.accept()
                && let Some(params) = handle_request(stream)
            {
                return finish(cfg, &params, &state, &redirect_uri, &verifier);
            }
        }
        std::thread::sleep(ACCEPT_POLL);
    }
}

fn finish(
    cfg: &MsaConfig,
    params: &[(String, String)],
    state: &str,
    redirect_uri: &str,
    verifier: &str,
) -> Result<MsaTokens> {
    let get = |k: &str| {
        params
            .iter()
            .find(|(key, _)| key == k)
            .map(|(_, v)| v.as_str())
    };
    if let Some(err) = get("error") {
        return Err(Error::Auth(
            get("error_description").unwrap_or(err).to_owned(),
        ));
    }
    if get("state") != Some(state) {
        return Err(Error::Auth("state mismatch in login redirect".into()));
    }
    let code =
        get("code").ok_or_else(|| Error::Auth("no authorization code in redirect".into()))?;
    oauth::exchange_code(cfg, code, redirect_uri, verifier)
}

/// Read the request line, reply with a small page, return query params.
/// Returns `None` for requests that are not the redirect (e.g. favicon).
fn handle_request(stream: TcpStream) -> Option<Vec<(String, String)>> {
    stream.set_nonblocking(false).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    let mut line = String::new();
    BufReader::new(&stream).read_line(&mut line).ok()?;
    let target = line.split_whitespace().nth(1)?;
    let query = target.strip_prefix("/?")?;
    let mut out = &stream;
    let _ = write!(
        out,
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{DONE_PAGE}",
        DONE_PAGE.len()
    );
    Some(parse_query(query))
}

fn parse_query(query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .map(|(k, v)| (decode(k), decode(v)))
        .collect()
}

/// 256 bits of randomness from two v4 UUIDs, as 64 hex chars (valid PKCE verifier).
fn random_token() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

fn pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let escaped = match bytes[i] {
            b'%' if i + 2 < bytes.len() => hex_val(bytes[i + 1]).zip(hex_val(bytes[i + 2])),
            _ => None,
        };
        match (escaped, bytes[i]) {
            (Some((hi, lo)), _) => {
                out.push(hi << 4 | lo);
                i += 2;
            }
            (None, b'+') => out.push(b' '),
            (None, b) => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    (b as char).to_digit(16).map(|d| d as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_rfc7636_vector() {
        assert_eq!(
            pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn url_encoding_roundtrip() {
        let raw = "XboxLive.signin offline_access&x=y/z";
        assert_eq!(decode(&encode(raw)), raw);
        assert_eq!(encode("a b"), "a%20b");
    }

    #[test]
    fn parses_redirect_query() {
        let q = parse_query("code=M.C5_abc%21&state=xyz");
        assert_eq!(q[0], ("code".into(), "M.C5_abc!".into()));
        assert_eq!(q[1], ("state".into(), "xyz".into()));
    }

    #[test]
    fn truncated_percent_is_literal() {
        assert_eq!(decode("ab%2"), "ab%2");
    }

    #[test]
    fn random_tokens_are_valid_verifiers() {
        let t = random_token();
        assert_eq!(t.len(), 64);
        assert_ne!(t, random_token());
    }
}
