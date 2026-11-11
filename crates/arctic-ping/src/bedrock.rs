//! Bedrock Edition's server list ping: RakNet's "unconnected ping" over UDP,
//! answered with one `MCPE;MOTD;protocol;version;online;max;…` line. Bedrock
//! has no SRV records, icons or player samples, and SOCKS5 isn't used (the
//! game itself never pings through a proxy).

use std::net::{ToSocketAddrs, UdpSocket};
use std::time::{Duration, Instant};

use crate::{Error, Result, Status};

/// Bedrock servers listen here unless told otherwise.
pub const BEDROCK_PORT: u16 = 19132;
/// RakNet's "offline message" marker, in every unconnected packet.
const MAGIC: [u8; 16] = [
    0x00, 0xff, 0xff, 0x00, 0xfe, 0xfe, 0xfe, 0xfe, 0xfd, 0xfd, 0xfd, 0xfd, 0x12, 0x34, 0x56, 0x78,
];
const UNCONNECTED_PING: u8 = 0x01;
const UNCONNECTED_PONG: u8 = 0x1c;
/// How long one try waits for the answer.
const WAIT: Duration = Duration::from_millis(1500);
/// UDP can drop a packet: ask this many times before giving up.
const TRIES: u32 = 3;

/// What a Bedrock server said when pinged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BedrockStatus {
    /// Version, players, MOTD (first line) and ping, like a Java server's.
    pub status: Status,
    /// "MCPE" for Bedrock, "MCEE" for Education Edition.
    pub edition: String,
    /// The second MOTD line (often the world's name).
    pub level: String,
    /// "Survival", "Creative", … (empty when not given).
    pub game_mode: String,
}

/// A Bedrock address as typed (`host`, `host:port`, `[v6]:port`): the
/// default port is 19132.
pub fn parse_address(text: &str) -> Option<(String, u16)> {
    let parsed = crate::Address::parse(text)?;
    let explicit = text.trim().rsplit_once(':').is_some_and(|(before, port)| {
        // `host:port` or `[v6]:port`, not a bare IPv6 address.
        (!before.contains(':') || before.ends_with(']')) && port.parse::<u16>().is_ok()
    });
    let port = if explicit { parsed.port } else { BEDROCK_PORT };
    Some((parsed.host, port))
}

/// Ping one Bedrock server (`host`, `host:port`).
pub fn ping_bedrock(address: &str) -> Result<BedrockStatus> {
    let (host, port) = parse_address(address)
        .ok_or_else(|| Error(format!("\"{address}\" isn't a server address")))?;
    let target = (host.as_str(), port)
        .to_socket_addrs()
        .map_err(|_| Error(format!("couldn't find {host}")))?
        .next()
        .ok_or_else(|| Error(format!("couldn't find {host}")))?;
    let bind = if target.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    };
    let socket = UdpSocket::bind(bind).map_err(|e| Error(format!("network error: {e}")))?;
    socket
        .connect(target)
        .map_err(|e| Error(format!("couldn't reach the server: {e}")))?;
    socket
        .set_read_timeout(Some(WAIT))
        .map_err(|e| Error(e.to_string()))?;
    let guid: u64 = std::process::id() as u64 ^ 0x4172_6374_6963_0000;
    let mut buf = [0u8; 2048];
    for _ in 0..TRIES {
        let sent = Instant::now();
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        socket
            .send(&ping_packet(stamp, guid))
            .map_err(|e| Error(format!("couldn't reach the server: {e}")))?;
        while let Ok(n) = socket.recv(&mut buf) {
            if let Some(line) = pong_line(&buf[..n]) {
                let ping_ms = sent.elapsed().as_millis().min(u32::MAX as u128) as u32;
                return parse_pong(&line, ping_ms);
            }
            if sent.elapsed() >= WAIT {
                break;
            }
        }
    }
    Err(Error("the server didn't answer".into()))
}

/// Ping many Bedrock servers at once, calling `done(index, result)` as each answers.
pub fn ping_bedrock_each(addresses: &[String], done: impl Fn(usize, Result<BedrockStatus>) + Sync) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let next = AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..crate::PARALLEL.min(addresses.len()) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(address) = addresses.get(i) else {
                        break;
                    };
                    done(i, ping_bedrock(address));
                }
            });
        }
    });
}

fn ping_packet(stamp: u64, guid: u64) -> Vec<u8> {
    let mut packet = Vec::with_capacity(33);
    packet.push(UNCONNECTED_PING);
    packet.extend_from_slice(&stamp.to_be_bytes());
    packet.extend_from_slice(&MAGIC);
    packet.extend_from_slice(&guid.to_be_bytes());
    packet
}

/// The status line of an unconnected pong, or `None` for anything else.
fn pong_line(packet: &[u8]) -> Option<String> {
    // id, time (8), server guid (8), magic (16), length (2), text
    let (&id, rest) = packet.split_first()?;
    if id != UNCONNECTED_PONG || rest.len() < 34 || rest[16..32] != MAGIC {
        return None;
    }
    let len = u16::from_be_bytes([rest[32], rest[33]]) as usize;
    let text = rest.get(34..34 + len)?;
    Some(String::from_utf8_lossy(text).into_owned())
}

/// `MCPE;MOTD;protocol;version;online;max;server id;level;game mode;…`
fn parse_pong(line: &str, ping_ms: u32) -> Result<BedrockStatus> {
    let fields: Vec<&str> = line.split(';').collect();
    if fields.len() < 6 {
        return Err(Error("the server's status couldn't be read".into()));
    }
    let field = |i: usize| fields.get(i).copied().unwrap_or("").to_owned();
    let motd_formatted = field(1);
    Ok(BedrockStatus {
        status: Status {
            version: field(3),
            protocol: field(2).parse().unwrap_or(0),
            online: field(4).parse().unwrap_or(0),
            max: field(5).parse().unwrap_or(0),
            players: Vec::new(),
            motd: strip_codes(&motd_formatted),
            motd_formatted,
            icon: None,
            ping_ms,
        },
        edition: field(0),
        level: strip_codes(&field(7)),
        game_mode: field(8),
    })
}

/// Text without `§x` formatting codes.
fn strip_codes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '§' {
            chars.next();
        } else {
            out.push(c);
        }
    }
    out.trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_pong() {
        let line = "MCPE;§aDedicated Server;827;1.21.100;3;10;13253860892328930865;Bedrock level;Survival;1;19132;19133;";
        let mut packet = vec![UNCONNECTED_PONG];
        packet.extend_from_slice(&5u64.to_be_bytes());
        packet.extend_from_slice(&7u64.to_be_bytes());
        packet.extend_from_slice(&MAGIC);
        packet.extend_from_slice(&(line.len() as u16).to_be_bytes());
        packet.extend_from_slice(line.as_bytes());
        let read = pong_line(&packet).unwrap();
        let s = parse_pong(&read, 42).unwrap();
        assert_eq!(s.status.version, "1.21.100");
        assert_eq!(s.status.protocol, 827);
        assert_eq!((s.status.online, s.status.max), (3, 10));
        assert_eq!(s.status.motd, "Dedicated Server");
        assert_eq!(s.status.motd_formatted, "§aDedicated Server");
        assert_eq!(s.level, "Bedrock level");
        assert_eq!(s.game_mode, "Survival");
        assert_eq!(s.edition, "MCPE");
        assert_eq!(s.status.ping_ms, 42);
        assert!(pong_line(&packet[..20]).is_none());
        assert!(parse_pong("MCPE;x", 1).is_err());
    }

    #[test]
    fn a_ping_is_33_bytes() {
        let p = ping_packet(1, 2);
        assert_eq!(p.len(), 33);
        assert_eq!(p[0], UNCONNECTED_PING);
        assert_eq!(&p[9..25], &MAGIC);
    }

    #[test]
    fn bedrock_addresses_default_to_19132() {
        assert_eq!(
            parse_address("Play.Example.net"),
            Some(("play.example.net".into(), 19132))
        );
        assert_eq!(
            parse_address("play.example.net:19140"),
            Some(("play.example.net".into(), 19140))
        );
        assert_eq!(parse_address("[::1]:25565"), Some(("::1".into(), 25565)));
        assert_eq!(parse_address("fe80::1"), Some(("fe80::1".into(), 19132)));
        assert_eq!(parse_address("host:nope"), None);
    }
}
