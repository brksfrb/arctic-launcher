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
/// What the browser shows after signing in: self-contained (it's served from a loopback
/// port, with nothing else to load), dark, matching the launcher's look.
const DONE_PAGE: &str = concat!(
    "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">",
    "<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">",
    "<title>Signed in - Arctic Launcher</title><style>",
    "*{box-sizing:border-box}html,body{height:100%;margin:0}",
    "body{display:flex;align-items:center;justify-content:center;font-family:system-ui,-apple-system,'Segoe UI',Roboto,sans-serif;",
    "color:#e6f1ff;background:radial-gradient(120% 90% at 50% 0%,#14304d 0%,#0b1320 55%,#070c16 100%)}",
    ".card{width:min(420px,calc(100vw - 32px));padding:40px 32px 32px;text-align:center;border-radius:20px;",
    "background:rgba(255,255,255,.04);border:1px solid rgba(125,211,252,.18);",
    "box-shadow:0 24px 80px rgba(2,8,20,.55)}",
    ".flake{width:64px;height:64px;margin:0 auto 20px;color:#7dd3fc}",
    ".tick{display:inline-flex;align-items:center;justify-content:center;width:28px;height:28px;border-radius:50%;",
    "background:#22c55e;color:#04210f;font-weight:700;margin-bottom:14px}",
    "h1{font-size:22px;margin:0 0 8px;font-weight:650}p{margin:0;color:#9fb6d3;line-height:1.5;font-size:15px}",
    ".fine{margin-top:24px;font-size:12px;color:#5f7794}",
    "</style></head><body><main class=\"card\">",
    "<svg class=\"flake\" viewBox=\"0 0 64 64\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"3\" stroke-linecap=\"round\">",
    "<path d=\"M32 6v52M9.5 19l45 26M54.5 19l-45 26\"/>",
    "<path d=\"M26 10l6 6 6-6M26 54l6-6 6 6M6 27l8.5 2.3-2.3 8.5M58 37l-8.5-2.3 2.3-8.5M6 37l8.5-2.3-2.3-8.5M58 27l-8.5 2.3 2.3 8.5\"/>",
    "</svg><div class=\"tick\">&#10003;</div>",
    "<h1>You're signed in</h1>",
    "<p>You can close this tab and go back to Arctic Launcher.</p>",
    "<p class=\"fine\">Arctic Launcher</p>",
    "</main></body></html>"
);

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
