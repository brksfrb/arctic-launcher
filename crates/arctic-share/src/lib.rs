//! Play together without a server: a host's "Open to LAN" world is
//! tunnelled over a peer-to-peer QUIC connection (iroh: hole punching,
//! with a relay as fallback) and shows up in the guest's LAN list.
//!
//! The host's invite code is its endpoint id. Every tunnel stream starts
//! with one byte: [`KIND_INFO`] asks for the world name, [`KIND_GAME`]
//! carries one Minecraft connection.

mod guest;
mod host;
pub mod lan;

use std::path::Path;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use iroh::{EndpointId, SecretKey};
use tokio::runtime::Runtime;
use tokio::sync::oneshot;

pub use lan::LanWorld;

const ALPN: &[u8] = b"arctic/lan-tunnel/1";
const KIND_INFO: u8 = 0;
const KIND_GAME: u8 = 1;
/// Marks our own announcements so the host never re-shares a tunnel.
const GUEST_SUFFIX: &str = " · via Arctic";
const CODE_PREFIX: &str = "arctic-";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShareEvent {
    /// Hosting started; give this code to friends.
    HostReady { code: String },
    /// The LAN world being shared appeared, changed or closed.
    HostWorld(Option<LanWorld>),
    /// Number of friends currently connected.
    Guests(usize),
    /// Joined a friend: the world is announced to local games and reachable
    /// at `localhost:port`.
    Joined { motd: String, port: u16 },
    /// The session ended, with a reason if it failed.
    Stopped { error: Option<String> },
}

type Sink = Arc<dyn Fn(ShareEvent) + Send + Sync>;
/// Identifies one hosting or joining session; events carry it so late
/// events from a replaced session can be ignored.
pub type SessionId = u64;
type Callback = Arc<dyn Fn(SessionId, ShareEvent) + Send + Sync>;

/// Owns the networking runtime and at most one session (hosting or joined).
pub struct Share {
    runtime: Runtime,
    stop: Mutex<Option<oneshot::Sender<()>>>,
    callback: Callback,
    sessions: AtomicU64,
}

impl Share {
    pub fn new(
        callback: impl Fn(SessionId, ShareEvent) + Send + Sync + 'static,
    ) -> std::io::Result<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("arctic-share")
            .enable_all()
            .build()?;
        Ok(Self {
            runtime,
            stop: Mutex::new(None),
            callback: Arc::new(callback),
            sessions: AtomicU64::new(0),
        })
    }

    /// Share this machine's LAN world. `port` skips detection and shares a
    /// fixed port instead.
    pub fn host(&self, key: [u8; 32], port: Option<u16>) -> SessionId {
        let stop = self.replace_session();
        let (id, sink) = self.session_sink();
        self.runtime
            .spawn(host::run(SecretKey::from_bytes(&key), port, sink, stop));
        id
    }

    /// Join a friend's world by invite code.
    pub fn join(&self, code: &str) -> Result<SessionId, String> {
        let host = parse_code(code)?;
        let stop = self.replace_session();
        let (id, sink) = self.session_sink();
        self.runtime.spawn(guest::run(host, sink, stop));
        Ok(id)
    }

    fn session_sink(&self) -> (SessionId, Sink) {
        let id = self.sessions.fetch_add(1, Ordering::SeqCst) + 1;
        let callback = self.callback.clone();
        (id, Arc::new(move |event| callback(id, event)))
    }

    /// End the current session, if any.
    pub fn stop(&self) {
        if let Some(stop) = self.lock_stop().take() {
            let _ = stop.send(());
        }
    }

    fn replace_session(&self) -> oneshot::Receiver<()> {
        let (tx, rx) = oneshot::channel();
        if let Some(old) = self.lock_stop().replace(tx) {
            let _ = old.send(());
        }
        rx
    }

    fn lock_stop(&self) -> std::sync::MutexGuard<'_, Option<oneshot::Sender<()>>> {
        self.stop.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl Drop for Share {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Endpoint settings shared by host and guest. Certificates are checked
/// against the OS store so HTTPS-scanning antivirus doesn't break relays.
fn endpoint_builder() -> iroh::endpoint::Builder {
    iroh::Endpoint::builder(iroh::endpoint::presets::N0)
        .ca_tls_config(iroh::tls::CaTlsConfig::system())
}

/// The host key, created on first use so the invite code stays the same.
pub fn load_or_create_key(path: &Path) -> std::io::Result<[u8; 32]> {
    if let Ok(bytes) = std::fs::read(path)
        && let Ok(key) = <[u8; 32]>::try_from(bytes.as_slice())
    {
        return Ok(key);
    }
    let key = SecretKey::generate().to_bytes();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, key)?;
    Ok(key)
}

/// `arctic-abcd-efgh-…`: the endpoint id in base32, grouped for reading.
pub fn code_for(id: &EndpointId) -> String {
    let raw = data_encoding::BASE32_NOPAD
        .encode(id.as_bytes())
        .to_ascii_lowercase();
    let groups: Vec<&str> = raw
        .as_bytes()
        .chunks(4)
        .map(|c| std::str::from_utf8(c).unwrap_or_default())
        .collect();
    format!("{CODE_PREFIX}{}", groups.join("-"))
}

/// Accepts codes with or without the prefix, dashes, spaces or case.
pub fn parse_code(code: &str) -> Result<EndpointId, String> {
    let compact: String = code
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .collect::<String>()
        .to_ascii_lowercase();
    let prefix = CODE_PREFIX.trim_end_matches('-');
    // 52 base32 characters, optionally preceded by the prefix.
    let body = match compact.strip_prefix(prefix) {
        Some(rest) if rest.len() == 52 => rest,
        _ => compact.as_str(),
    };
    EndpointId::from_str(body).map_err(|_| "That invite code isn't valid.".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_round_trip_in_any_format() {
        let id = SecretKey::generate().public();
        let code = code_for(&id);
        assert!(code.starts_with(CODE_PREFIX));
        assert_eq!(parse_code(&code), Ok(id));
        assert_eq!(parse_code(&code.to_uppercase()), Ok(id));
        let spaced = format!("  {}  ", code.replace('-', " "));
        assert_eq!(parse_code(&spaced), Ok(id));
    }

    #[test]
    fn bad_codes_are_rejected() {
        assert!(parse_code("").is_err());
        assert!(parse_code("arctic-hello").is_err());
    }

    #[test]
    fn key_is_created_once() {
        let dir = std::env::temp_dir().join(format!("arctic-share-test-{}", std::process::id()));
        let path = dir.join("share.key");
        let a = load_or_create_key(&path).unwrap();
        let b = load_or_create_key(&path).unwrap();
        assert_eq!(a, b);
        let _ = std::fs::remove_dir_all(dir);
    }
}
