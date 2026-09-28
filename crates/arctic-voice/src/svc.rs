//! Simple Voice Chat's wire format (compatibility version 20, SVC 2.6.x),
//! so Arctic players can talk with Simple Voice Chat players on servers
//! that run its plugin.
//!
//! The server hands out a secret through the game connection (the
//! `voicechat:secret` payload, parsed by [`Hello::parse`]); voice then goes
//! over UDP. Every UDP packet is `0xFF`, (client to server only) the
//! player's UUID, then a length-prefixed AES-128-GCM blob: a 12-byte nonce
//! and the encrypted packet (type byte + body). Numbers are big-endian and
//! lengths are VarInts, as in Minecraft's `FriendlyByteBuf`.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes128Gcm, Nonce};

/// The version Arctic speaks; the server ignores other versions.
pub const COMPATIBILITY_VERSION: i32 = 20;
const MAGIC: u8 = 0xFF;
const NONCE_LEN: usize = 12;
/// SVC's own limit on one UDP packet.
pub const MAX_PACKET: usize = 2048;
const MAX_OPUS: usize = crate::packet::MAX_OPUS;
/// Longest voice host string accepted (as SVC).
const MAX_HOST: usize = 32_767;

const MIC: u8 = 0x1;
const PLAYER_SOUND: u8 = 0x2;
const GROUP_SOUND: u8 = 0x3;
const LOCATION_SOUND: u8 = 0x4;
const AUTHENTICATE: u8 = 0x5;
const AUTHENTICATE_ACK: u8 = 0x6;
const KEEP_ALIVE: u8 = 0x8;
const CONNECTION_CHECK: u8 = 0x9;
const CONNECTION_CHECK_ACK: u8 = 0xA;

const HAS_CATEGORY: u8 = 0b10;
/// Categories are at most this long (SVC's limit).
const MAX_CATEGORY: usize = 16;

/// What the server sent through the game: how to reach its voice server.
#[derive(Debug, Clone, PartialEq)]
pub struct Hello {
    pub secret: [u8; 16],
    pub port: u16,
    pub player: [u8; 16],
    /// Blocks beyond which the server stops sending voices.
    pub distance: f64,
    /// Milliseconds between the server's keep-alives.
    pub keep_alive_ms: u32,
    /// A different host and/or port for voice ("" = the game's server).
    pub voice_host: String,
}

impl Hello {
    /// The `voicechat:secret` payload's bytes.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let mut r = Reader(bytes);
        let secret = r.array::<16>()?;
        let port = u16::try_from(r.i32()?).ok().filter(|p| *p != 0)?;
        let player = r.array::<16>()?;
        let _codec = r.u8()?;
        let _mtu = r.i32()?;
        let distance = r.f64()?;
        let keep_alive_ms = u32::try_from(r.i32()?).ok()?;
        let _groups = r.bool()?;
        let voice_host = r.string(MAX_HOST)?;
        Some(Self {
            secret,
            port,
            player,
            distance,
            keep_alive_ms,
            voice_host,
        })
    }

    /// Where to send voice, from the game server's host (as SVC does: the
    /// voice host may be a port, a host or `host:port`).
    pub fn address(&self, game_host: &str) -> (String, u16) {
        let host = self.voice_host.trim();
        if host.is_empty() {
            return (game_host.to_string(), self.port);
        }
        if let Ok(port) = host.parse::<u16>() {
            return (
                game_host.to_string(),
                if port == 0 { self.port } else { port },
            );
        }
        let host = host.strip_prefix("voicechat://").unwrap_or(host);
        if let Some(rest) = host.strip_prefix('[') {
            // [ipv6]:port
            if let Some((ip, tail)) = rest.split_once(']') {
                let port = tail.strip_prefix(':').and_then(|p| p.parse().ok());
                return (
                    ip.to_string(),
                    port.filter(|p| *p != 0).unwrap_or(self.port),
                );
            }
        }
        match host.rsplit_once(':') {
            Some((h, p)) if !h.contains(':') => match p.parse::<u16>() {
                Ok(port) if port != 0 => (h.to_string(), port),
                _ => (h.to_string(), self.port),
            },
            _ => (host.to_string(), self.port),
        }
    }
}

/// Client to server.
#[derive(Debug, Clone, PartialEq)]
pub enum Outgoing {
    Authenticate,
    ConnectionCheck,
    KeepAlive,
    Mic { opus: Vec<u8>, seq: i64 },
}

/// Server to client.
#[derive(Debug, Clone, PartialEq)]
pub enum Incoming {
    AuthenticateAck,
    ConnectionCheckAck,
    KeepAlive,
    Sound(Sound),
    /// Something Arctic doesn't use (pings, …).
    Other,
}

/// Someone's voice.
#[derive(Debug, Clone, PartialEq)]
pub struct Sound {
    pub sender: [u8; 16],
    pub opus: Vec<u8>,
    pub seq: i64,
    pub from: From,
}

/// Where a voice comes from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum From {
    /// The sending player (placed where they stand).
    Player,
    /// A fixed spot in the world.
    Location { x: f64, y: f64, z: f64 },
    /// A group: heard at full volume wherever they are.
    Group,
}

/// A UDP packet for the server.
pub fn encode(hello: &Hello, packet: &Outgoing) -> Option<Vec<u8>> {
    let mut body = Vec::new();
    match packet {
        Outgoing::Authenticate => {
            body.push(AUTHENTICATE);
            body.extend_from_slice(&hello.player);
            body.extend_from_slice(&hello.secret);
        }
        Outgoing::ConnectionCheck => body.push(CONNECTION_CHECK),
        Outgoing::KeepAlive => body.push(KEEP_ALIVE),
        Outgoing::Mic { opus, seq } => {
            body.push(MIC);
            write_bytes(&mut body, opus);
            body.extend_from_slice(&seq.to_be_bytes());
            body.push(0); // not whispering
        }
    }
    let sealed = seal(&hello.secret, &body)?;
    let mut out = Vec::with_capacity(1 + 16 + 3 + sealed.len());
    out.push(MAGIC);
    out.extend_from_slice(&hello.player);
    write_bytes(&mut out, &sealed);
    Some(out)
}

/// A UDP packet from the server (`None` = not for us or corrupt).
pub fn decode(secret: &[u8; 16], datagram: &[u8]) -> Option<Incoming> {
    let mut r = Reader(datagram);
    if r.u8()? != MAGIC {
        return None;
    }
    let sealed = r.bytes(MAX_PACKET)?;
    let body = open(secret, sealed)?;
    let mut r = Reader(&body);
    Some(match r.u8()? {
        AUTHENTICATE_ACK => Incoming::AuthenticateAck,
        CONNECTION_CHECK_ACK => Incoming::ConnectionCheckAck,
        KEEP_ALIVE => Incoming::KeepAlive,
        PLAYER_SOUND => {
            let _channel = r.array::<16>()?;
            let sender = r.array::<16>()?;
            let opus = r.bytes(MAX_OPUS)?.to_vec();
            let seq = r.i64()?;
            let _distance = r.f32()?;
            let _flags = r.u8()?;
            Incoming::Sound(Sound {
                sender,
                opus,
                seq,
                from: From::Player,
            })
        }
        GROUP_SOUND => {
            let _channel = r.array::<16>()?;
            let sender = r.array::<16>()?;
            let opus = r.bytes(MAX_OPUS)?.to_vec();
            let seq = r.i64()?;
            skip_category(&mut r)?;
            Incoming::Sound(Sound {
                sender,
                opus,
                seq,
                from: From::Group,
            })
        }
        LOCATION_SOUND => {
            let _channel = r.array::<16>()?;
            let sender = r.array::<16>()?;
            let (x, y, z) = (r.f64()?, r.f64()?, r.f64()?);
            let opus = r.bytes(MAX_OPUS)?.to_vec();
            let seq = r.i64()?;
            let _distance = r.f32()?;
            skip_category(&mut r)?;
            Incoming::Sound(Sound {
                sender,
                opus,
                seq,
                from: From::Location { x, y, z },
            })
        }
        _ => Incoming::Other,
    })
}

/// The `voicechat:request_secret` payload.
pub fn request_secret() -> Vec<u8> {
    COMPATIBILITY_VERSION.to_be_bytes().to_vec()
}

/// A UUID as SVC and Arctic write it (32 hex digits, lower case).
pub fn uuid_hex(id: &[u8; 16]) -> String {
    id.iter().map(|b| format!("{b:02x}")).collect()
}

fn skip_category(r: &mut Reader) -> Option<()> {
    if r.u8()? & HAS_CATEGORY != 0 {
        r.string(MAX_CATEGORY * 4)?;
    }
    Some(())
}

fn seal(secret: &[u8; 16], body: &[u8]) -> Option<Vec<u8>> {
    let mut nonce = [0u8; NONCE_LEN];
    getrandom::fill(&mut nonce).ok()?;
    let cipher = Aes128Gcm::new_from_slice(secret).ok()?;
    let sealed = cipher.encrypt(Nonce::from_slice(&nonce), body).ok()?;
    let mut out = Vec::with_capacity(NONCE_LEN + sealed.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&sealed);
    Some(out)
}

fn open(secret: &[u8; 16], sealed: &[u8]) -> Option<Vec<u8>> {
    if sealed.len() <= NONCE_LEN {
        return None;
    }
    let (nonce, data) = sealed.split_at(NONCE_LEN);
    let cipher = Aes128Gcm::new_from_slice(secret).ok()?;
    cipher.decrypt(Nonce::from_slice(nonce), data).ok()
}

fn write_varint(out: &mut Vec<u8>, mut v: u32) {
    loop {
        if v & !0x7F == 0 {
            out.push(v as u8);
            return;
        }
        out.push((v & 0x7F) as u8 | 0x80);
        v >>= 7;
    }
}

fn write_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    write_varint(out, bytes.len() as u32);
    out.extend_from_slice(bytes);
}

/// Reads `FriendlyByteBuf` values; `None` when the data runs out.
struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.0.len() < n {
            return None;
        }
        let (head, rest) = self.0.split_at(n);
        self.0 = rest;
        Some(head)
    }

    fn array<const N: usize>(&mut self) -> Option<[u8; N]> {
        self.take(N)?.try_into().ok()
    }

    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }

    fn bool(&mut self) -> Option<bool> {
        Some(self.u8()? != 0)
    }

    fn i32(&mut self) -> Option<i32> {
        Some(i32::from_be_bytes(self.array()?))
    }

    fn i64(&mut self) -> Option<i64> {
        Some(i64::from_be_bytes(self.array()?))
    }

    fn f32(&mut self) -> Option<f32> {
        Some(f32::from_be_bytes(self.array()?))
    }

    fn f64(&mut self) -> Option<f64> {
        Some(f64::from_be_bytes(self.array()?))
    }

    fn varint(&mut self) -> Option<u32> {
        let mut v = 0u32;
        for i in 0..5 {
            let b = self.u8()?;
            v |= u32::from(b & 0x7F) << (7 * i);
            if b & 0x80 == 0 {
                return Some(v);
            }
        }
        None
    }

    fn bytes(&mut self, max: usize) -> Option<&'a [u8]> {
        let n = self.varint()? as usize;
        if n > max {
            return None;
        }
        self.take(n)
    }

    fn string(&mut self, max_chars: usize) -> Option<String> {
        let raw = self.bytes(max_chars * 3)?;
        String::from_utf8(raw.to_vec()).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: [u8; 16] = [7; 16];
    const PLAYER: [u8; 16] = [
        0x0f, 0x0e, 0x0d, 0x0c, 0x0b, 0x0a, 0x49, 0x08, 0x87, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01,
        0x00,
    ];

    fn hello(voice_host: &str) -> Hello {
        Hello {
            secret: SECRET,
            port: 24454,
            player: PLAYER,
            distance: 48.0,
            keep_alive_ms: 1000,
            voice_host: voice_host.into(),
        }
    }

    /// The payload as SVC's `SecretPacket.toBytes` writes it.
    fn secret_payload(voice_host: &str) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&SECRET);
        b.extend_from_slice(&24454i32.to_be_bytes());
        b.extend_from_slice(&PLAYER);
        b.push(0); // codec: VOIP
        b.extend_from_slice(&1024i32.to_be_bytes());
        b.extend_from_slice(&48.0f64.to_be_bytes());
        b.extend_from_slice(&1000i32.to_be_bytes());
        b.push(1); // groups
        write_bytes(&mut b, voice_host.as_bytes());
        b.push(0); // recording
        b
    }

    /// A server packet: magic, then the sealed body (no UUID).
    fn from_server(body: &[u8]) -> Vec<u8> {
        let mut out = vec![MAGIC];
        write_bytes(&mut out, &seal(&SECRET, body).unwrap());
        out
    }

    #[test]
    fn parses_the_secret_payload() {
        let h = Hello::parse(&secret_payload("")).unwrap();
        assert_eq!(h, hello(""));
        assert!(Hello::parse(&secret_payload("")[..20]).is_none());
    }

    #[test]
    fn voice_host_forms() {
        assert_eq!(hello("").address("1.2.3.4"), ("1.2.3.4".into(), 24454));
        assert_eq!(hello("25000").address("1.2.3.4"), ("1.2.3.4".into(), 25000));
        assert_eq!(
            hello("voice.example.com").address("1.2.3.4"),
            ("voice.example.com".into(), 24454)
        );
        assert_eq!(
            hello("voice.example.com:30000").address("x"),
            ("voice.example.com".into(), 30000)
        );
        assert_eq!(hello("[::1]:30000").address("x"), ("::1".into(), 30000));
    }

    #[test]
    fn client_packets_carry_the_player_and_decrypt() {
        let packet = encode(
            &hello(""),
            &Outgoing::Mic {
                opus: vec![1, 2, 3],
                seq: 9,
            },
        )
        .unwrap();
        assert_eq!(packet[0], MAGIC);
        assert_eq!(&packet[1..17], &PLAYER);
        let mut r = Reader(&packet[17..]);
        let body = open(&SECRET, r.bytes(MAX_PACKET).unwrap()).unwrap();
        let mut expected = vec![MIC, 3, 1, 2, 3];
        expected.extend_from_slice(&9i64.to_be_bytes());
        expected.push(0);
        assert_eq!(body, expected);

        let auth = encode(&hello(""), &Outgoing::Authenticate).unwrap();
        let mut r = Reader(&auth[17..]);
        let body = open(&SECRET, r.bytes(MAX_PACKET).unwrap()).unwrap();
        assert_eq!(body[0], AUTHENTICATE);
        assert_eq!(&body[1..17], &PLAYER);
        assert_eq!(&body[17..], &SECRET);
    }

    #[test]
    fn decodes_server_packets() {
        assert_eq!(
            decode(&SECRET, &from_server(&[AUTHENTICATE_ACK])),
            Some(Incoming::AuthenticateAck)
        );
        assert_eq!(
            decode(&SECRET, &from_server(&[KEEP_ALIVE])),
            Some(Incoming::KeepAlive)
        );
        assert_eq!(decode(&SECRET, &from_server(&[0x7])), Some(Incoming::Other));

        let mut body = vec![PLAYER_SOUND];
        body.extend_from_slice(&[1; 16]);
        body.extend_from_slice(&PLAYER);
        write_bytes(&mut body, &[5, 6]);
        body.extend_from_slice(&42i64.to_be_bytes());
        body.extend_from_slice(&48.0f32.to_be_bytes());
        body.push(0);
        let Some(Incoming::Sound(s)) = decode(&SECRET, &from_server(&body)) else {
            panic!("not a sound");
        };
        assert_eq!(
            (s.sender, s.opus, s.seq, s.from),
            (PLAYER, vec![5, 6], 42, From::Player)
        );

        let mut body = vec![LOCATION_SOUND];
        body.extend_from_slice(&[1; 16]);
        body.extend_from_slice(&PLAYER);
        for v in [1.5f64, 64.0, -3.0] {
            body.extend_from_slice(&v.to_be_bytes());
        }
        write_bytes(&mut body, &[9]);
        body.extend_from_slice(&1i64.to_be_bytes());
        body.extend_from_slice(&16.0f32.to_be_bytes());
        body.push(HAS_CATEGORY);
        write_bytes(&mut body, b"music");
        let Some(Incoming::Sound(s)) = decode(&SECRET, &from_server(&body)) else {
            panic!("not a sound");
        };
        assert_eq!(
            s.from,
            From::Location {
                x: 1.5,
                y: 64.0,
                z: -3.0
            }
        );
    }

    #[test]
    fn rejects_wrong_secret_and_junk() {
        let packet = from_server(&[KEEP_ALIVE]);
        assert!(decode(&[8; 16], &packet).is_none());
        assert!(decode(&SECRET, &[0x00, 1, 2]).is_none());
        assert!(decode(&SECRET, &[MAGIC, 0xFF, 0xFF, 0xFF, 0xFF, 0x0F]).is_none());
        let mut cut = packet.clone();
        cut.truncate(packet.len() - 3);
        assert!(decode(&SECRET, &cut).is_none());
    }

    #[test]
    fn request_and_uuid_format() {
        assert_eq!(request_secret(), vec![0, 0, 0, 20]);
        assert_eq!(uuid_hex(&PLAYER), "0f0e0d0c0b0a49088706050403020100");
    }
}
