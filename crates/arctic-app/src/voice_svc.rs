//! Simple Voice Chat compatibility: on a server that runs its plugin, the
//! game asks for a voice secret and hands it here; this talks to the
//! server's voice port like a Simple Voice Chat client would. Your voice
//! goes to the server (which passes it to SVC players nearby), and theirs
//! comes back into the same mixer as Arctic voices.
//!
//! A player heard both ways (an Arctic player who is also connected to
//! SVC) is only played from Arctic's direct connection.

use std::collections::{HashMap, HashSet};
use std::net::{ToSocketAddrs, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use arctic_voice::engine::{Engine, PeerId};
use arctic_voice::packet::Packet;
use arctic_voice::spatial::Place;
use arctic_voice::svc::{self, From, Hello, Incoming, Outgoing, Sound};

/// Resend the handshake this often until the server answers.
const RETRY: Duration = Duration::from_secs(1);
/// How long a read waits (so the thread notices it should stop).
const READ_WAIT: Duration = Duration::from_millis(200);
/// SVC players heard this recently show in the in-game list.
const LISTED_FOR: Duration = Duration::from_secs(60);

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// The engine's number for an SVC speaker.
fn peer_for(uuid: &str) -> PeerId {
    crate::voice::peer_id(&format!("svc:{uuid}"))
}

struct Heard {
    from: From,
    at: Instant,
}

/// The UDP connection to one server's voice port.
struct Link {
    hello: Hello,
    socket: UdpSocket,
    connected: AtomicBool,
    seq: AtomicI64,
    stop: AtomicBool,
}

impl Link {
    fn send(&self, packet: &Outgoing) {
        if let Some(bytes) = svc::encode(&self.hello, packet) {
            let _ = self.socket.send(&bytes);
        }
    }

    /// Handshake, keep-alives and incoming voices, until stopped.
    fn run(&self, on_sound: impl Fn(Sound)) {
        let keep_alive = Duration::from_millis(u64::from(self.hello.keep_alive_ms.max(250)));
        let mut buf = vec![0u8; svc::MAX_PACKET + 64];
        let mut authenticated = false;
        let mut last_try: Option<Instant> = None;
        let mut last_alive = Instant::now();
        while !self.stop.load(Ordering::Relaxed) {
            let connected = self.connected.load(Ordering::Relaxed);
            if !connected && last_try.is_none_or(|t| t.elapsed() >= RETRY) {
                last_try = Some(Instant::now());
                self.send(if authenticated {
                    &Outgoing::ConnectionCheck
                } else {
                    &Outgoing::Authenticate
                });
            }
            if connected && last_alive.elapsed() > keep_alive * 10 {
                // The server stopped answering: start over.
                log::info!("simple voice chat: timed out, reconnecting");
                self.connected.store(false, Ordering::Relaxed);
                authenticated = false;
            }
            let Ok(n) = self.socket.recv(&mut buf) else {
                continue;
            };
            match svc::decode(&self.hello.secret, &buf[..n]) {
                Some(Incoming::AuthenticateAck) => authenticated = true,
                Some(Incoming::ConnectionCheckAck) if authenticated => {
                    if !self.connected.swap(true, Ordering::Relaxed) {
                        log::info!("simple voice chat: connected");
                    }
                    last_alive = Instant::now();
                }
                Some(Incoming::KeepAlive) => {
                    last_alive = Instant::now();
                    self.send(&Outgoing::KeepAlive);
                }
                Some(Incoming::Sound(sound)) => on_sound(sound),
                _ => {}
            }
        }
    }
}

/// What the game reports: the secret payload (hex) and the server's IP.
#[derive(serde::Deserialize, Clone, PartialEq)]
pub struct GameSvc {
    pub secret: String,
    pub host: String,
}

/// The SVC half of a voice session.
#[derive(Default)]
pub struct SvcSide {
    /// Shared with the engine's send callback.
    link: Arc<Mutex<Option<Arc<Link>>>>,
    /// The game report the link was made from.
    from_game: Option<GameSvc>,
    heard: Arc<Mutex<HashMap<String, Heard>>>,
    /// Arctic room members (their SVC copies are skipped).
    arctic: Arc<Mutex<HashSet<String>>>,
}

impl SvcSide {
    /// For the engine's send callback: your voice to the SVC server too.
    pub fn sender(&self) -> impl Fn(&[u8]) + Send + 'static {
        let slot = self.link.clone();
        move |bytes| {
            let link = lock(&slot).clone();
            let (Some(link), Some(packet)) = (link, Packet::decode(bytes)) else {
                return;
            };
            if link.connected.load(Ordering::Relaxed) {
                let seq = link.seq.fetch_add(1, Ordering::Relaxed);
                link.send(&Outgoing::Mic {
                    opus: packet.opus,
                    seq,
                });
            }
        }
    }

    pub fn connected(&self) -> bool {
        lock(&self.link)
            .as_ref()
            .is_some_and(|l| l.connected.load(Ordering::Relaxed))
    }

    /// Connect, reconnect or disconnect for what the game reports.
    pub fn update(&mut self, game: Option<&GameSvc>, engine: &Arc<Engine>) {
        if self.from_game.as_ref() == game {
            return;
        }
        self.close(engine);
        let Some(game) = game else {
            return;
        };
        self.from_game = Some(game.clone());
        match self.open(game, engine) {
            Ok(link) => *lock(&self.link) = Some(link),
            Err(e) => log::info!("simple voice chat: {e}"),
        }
    }

    fn open(&self, game: &GameSvc, engine: &Arc<Engine>) -> Result<Arc<Link>, String> {
        let bytes = hex_decode(&game.secret).ok_or("bad secret")?;
        let hello = Hello::parse(&bytes).ok_or("unreadable secret")?;
        let (host, port) = hello.address(&game.host);
        let addr = (host.as_str(), port)
            .to_socket_addrs()
            .map_err(|e| e.to_string())?
            .next()
            .ok_or("voice host not found")?;
        let bind = if addr.is_ipv6() {
            "[::]:0"
        } else {
            "0.0.0.0:0"
        };
        let socket = UdpSocket::bind(bind).map_err(|e| e.to_string())?;
        socket.connect(addr).map_err(|e| e.to_string())?;
        socket
            .set_read_timeout(Some(READ_WAIT))
            .map_err(|e| e.to_string())?;
        let link = Arc::new(Link {
            hello,
            socket,
            connected: AtomicBool::new(false),
            seq: AtomicI64::new(0),
            stop: AtomicBool::new(false),
        });
        let (runner, engine, heard, arctic) = (
            link.clone(),
            engine.clone(),
            self.heard.clone(),
            self.arctic.clone(),
        );
        std::thread::Builder::new()
            .name("arctic-svc".into())
            .spawn(move || {
                runner.run(|sound| {
                    let uuid = svc::uuid_hex(&sound.sender);
                    if lock(&arctic).contains(&uuid) {
                        return;
                    }
                    lock(&heard).insert(
                        uuid.clone(),
                        Heard {
                            from: sound.from,
                            at: Instant::now(),
                        },
                    );
                    let packet = Packet {
                        seq: sound.seq as u16,
                        opus: sound.opus,
                    };
                    engine.receive(peer_for(&uuid), &packet.encode());
                })
            })
            .map_err(|e| e.to_string())?;
        Ok(link)
    }

    /// Stop the link and forget SVC speakers.
    pub fn close(&mut self, engine: &Engine) {
        if let Some(link) = lock(&self.link).take() {
            link.stop.store(true, Ordering::Relaxed);
        }
        self.from_game = None;
        for uuid in lock(&self.heard).drain().map(|(k, _)| k) {
            engine.remove(peer_for(&uuid));
        }
    }

    /// Place SVC speakers, apply mutes, and list them for the game.
    pub fn tick(&self, engine: &Engine, frame: &Frame) -> Vec<serde_json::Value> {
        *lock(&self.arctic) = frame.arctic.clone();
        let speaking = engine.speaking();
        let mut heard = lock(&self.heard);
        heard.retain(|uuid, h| {
            let keep = h.at.elapsed() < LISTED_FOR && !frame.arctic.contains(uuid);
            if !keep {
                engine.remove(peer_for(uuid));
            }
            keep
        });
        heard
            .iter()
            .map(|(uuid, h)| {
                let peer = peer_for(uuid);
                let place = match h.from {
                    From::Player => frame.places.get(uuid).copied(),
                    From::Location { x, y, z } => Some(Place { x, y, z, yaw: 0.0 }),
                    From::Group => frame.listener,
                };
                engine.set_place(peer, place);
                let muted = frame.muted.iter().any(|m| m == uuid);
                engine.set_volume(peer, if muted { 0.0 } else { 1.0 });
                serde_json::json!({
                    "uuid": uuid,
                    "name": frame.names.get(uuid).cloned().unwrap_or_else(|| "Simple Voice Chat player".into()),
                    "friend": false,
                    "muted": muted,
                    "speaking": speaking.contains(&peer) && !muted,
                    "near": place.is_some(),
                    "svc": true,
                })
            })
            .collect()
    }
}

impl Drop for SvcSide {
    fn drop(&mut self) {
        if let Some(link) = lock(&self.link).take() {
            link.stop.store(true, Ordering::Relaxed);
        }
    }
}

/// This tick's view of the world, from the game's report.
pub struct Frame {
    pub listener: Option<Place>,
    pub places: HashMap<String, Place>,
    pub names: HashMap<String, String>,
    pub muted: Vec<String>,
    pub arctic: HashSet<String>,
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) || s.len() > svc::MAX_PACKET * 2 {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trip_and_rejects() {
        assert_eq!(hex_decode("00ff10"), Some(vec![0, 255, 16]));
        assert_eq!(hex_decode("0"), None);
        assert_eq!(hex_decode("zz"), None);
    }

    /// A fake SVC server on localhost: the link authenticates, checks the
    /// connection, answers keep-alives and passes voices on.
    #[test]
    fn handshake_with_a_fake_server() {
        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        server
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let port = server.local_addr().unwrap().port();
        let hello = Hello {
            secret: [3; 16],
            port,
            player: [9; 16],
            distance: 48.0,
            keep_alive_ms: 1000,
            voice_host: String::new(),
        };
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        socket.connect(("127.0.0.1", port)).unwrap();
        socket.set_read_timeout(Some(READ_WAIT)).unwrap();
        let link = Arc::new(Link {
            hello: hello.clone(),
            socket,
            connected: AtomicBool::new(false),
            seq: AtomicI64::new(0),
            stop: AtomicBool::new(false),
        });
        let sounds = Arc::new(Mutex::new(Vec::new()));
        let (runner, got) = (link.clone(), sounds.clone());
        let thread = std::thread::spawn(move || runner.run(|s| lock(&got).push(s)));

        let mut buf = [0u8; 4096];
        let reply = |to, body: &[u8]| {
            server
                .send_to(&fake_server_packet(&hello.secret, body), to)
                .unwrap();
        };
        // Authenticate → ack; connection check → ack.
        let (n, client) = server.recv_from(&mut buf).unwrap();
        assert_eq!(client_body(&hello, &buf[..n])[0], 0x5);
        reply(client, &[0x6]);
        let (n, _) = server.recv_from(&mut buf).unwrap();
        assert_eq!(client_body(&hello, &buf[..n]), vec![0x9]);
        reply(client, &[0xA]);
        // Keep-alive is answered.
        reply(client, &[0x8]);
        let (n, _) = server.recv_from(&mut buf).unwrap();
        assert_eq!(client_body(&hello, &buf[..n]), vec![0x8]);
        assert!(link.connected.load(Ordering::Relaxed));
        // A group voice arrives.
        let mut body = vec![0x3];
        body.extend_from_slice(&[1; 16]);
        body.extend_from_slice(&[2; 16]);
        body.extend_from_slice(&[1, 7]);
        body.extend_from_slice(&5i64.to_be_bytes());
        body.push(0);
        reply(client, &body);
        let deadline = Instant::now() + Duration::from_secs(5);
        while lock(&sounds).is_empty() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        link.stop.store(true, Ordering::Relaxed);
        thread.join().unwrap();
        let sounds = lock(&sounds);
        assert_eq!(sounds.len(), 1);
        assert_eq!(
            (sounds[0].sender, sounds[0].seq, sounds[0].from),
            ([2; 16], 5, From::Group)
        );
    }

    /// How the SVC server writes: magic, then length + nonce + ciphertext.
    fn fake_server_packet(secret: &[u8; 16], body: &[u8]) -> Vec<u8> {
        use aes_gcm::aead::{Aead, KeyInit};
        let nonce = [1u8; 12];
        let sealed = aes_gcm::Aes128Gcm::new_from_slice(secret)
            .unwrap()
            .encrypt(aes_gcm::Nonce::from_slice(&nonce), body)
            .unwrap();
        let len = nonce.len() + sealed.len();
        assert!(len < 128);
        let mut out = vec![0xFF, len as u8];
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&sealed);
        out
    }

    /// Decrypt a client packet the way the SVC server does.
    fn client_body(hello: &Hello, packet: &[u8]) -> Vec<u8> {
        use aes_gcm::aead::{Aead, KeyInit};
        assert_eq!(packet[0], 0xFF);
        assert_eq!(&packet[1..17], &hello.player);
        let len = packet[17] as usize;
        let sealed = &packet[18..18 + len];
        aes_gcm::Aes128Gcm::new_from_slice(&hello.secret)
            .unwrap()
            .decrypt(aes_gcm::Nonce::from_slice(&sealed[..12]), &sealed[12..])
            .unwrap()
    }
}
