//! Minecraft's "Open to LAN" announcements: the game multicasts
//! `[MOTD]name[/MOTD][AD]port[/AD]` to 224.0.2.60:4445 every 1.5 s, and
//! the Multiplayer screen lists every announcement it hears.

use std::io::{Read, Seek, SeekFrom};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, SystemTime};

use socket2::{Domain, Protocol, Socket, Type};
use tokio::net::{TcpStream, UdpSocket};

pub const GROUP: Ipv4Addr = Ipv4Addr::new(224, 0, 2, 60);
pub const PORT: u16 = 4445;
/// How often the game (and we) announce.
pub const INTERVAL: Duration = Duration::from_millis(1500);

/// A world opened to LAN on this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanWorld {
    pub motd: String,
    pub port: u16,
}

pub fn format(motd: &str, port: u16) -> String {
    format!(
        "[MOTD]{}[/MOTD][AD]{port}[/AD]",
        motd.replace('[', "(").replace(']', ")")
    )
}

pub fn parse(text: &str) -> Option<LanWorld> {
    let motd = between(text, "[MOTD]", "[/MOTD]")?;
    let port = between(text, "[AD]", "[/AD]")?.trim().parse().ok()?;
    Some(LanWorld {
        motd: motd.to_owned(),
        port,
    })
}

fn between<'a>(text: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let start = text.find(open)? + open.len();
    let end = start + text[start..].find(close)?;
    Some(&text[start..end])
}

/// Where the launcher keeps its instances: the games' logs under it are read for the port of a world
/// opened to LAN, as a second way to find it when no announcement arrives (a VPN or virtual
/// adapter, a firewall, a network that drops multicast).
static LOG_ROOT: Mutex<Option<PathBuf>> = Mutex::new(None);

pub fn watch_logs_under(root: PathBuf) {
    *LOG_ROOT.lock().unwrap_or_else(PoisonError::into_inner) = Some(root);
}

/// A game that wrote to its log this recently may be the one running now.
const LOG_FRESH: Duration = Duration::from_secs(12 * 60 * 60);
/// The end of a log is enough: the line comes when the world opens.
const LOG_TAIL: u64 = 128 * 1024;

/// The newest game log under the launcher's folders (`instances/*/minecraft/logs/latest.log`, also one
/// level down in `profiles/*`), if one was written to recently.
fn newest_game_log(root: &Path) -> Option<PathBuf> {
    let mut best: Option<(SystemTime, PathBuf)> = None;
    let mut bases = vec![root.to_path_buf()];
    if let Ok(profiles) = std::fs::read_dir(root.join("profiles")) {
        bases.extend(profiles.flatten().map(|e| e.path()));
    }
    for base in bases {
        let Ok(instances) = std::fs::read_dir(base.join("instances")) else {
            continue;
        };
        for instance in instances.flatten() {
            let log = instance
                .path()
                .join("minecraft")
                .join("logs")
                .join("latest.log");
            let Ok(modified) = std::fs::metadata(&log).and_then(|m| m.modified()) else {
                continue;
            };
            let fresh = modified.elapsed().is_ok_and(|age| age < LOG_FRESH);
            if fresh && best.as_ref().is_none_or(|(t, _)| modified > *t) {
                best = Some((modified, log));
            }
        }
    }
    best.map(|(_, log)| log)
}

/// The last port a game says it opened to LAN: "Local game hosted on port 54321" (the chat line) or
/// "Started serving on 54321" (the server's own).
pub fn port_from_log(text: &str) -> Option<u16> {
    text.lines().rev().find_map(|line| {
        ["hosted on port ", "Started serving on "]
            .iter()
            .find_map(|marker| {
                let rest = &line[line.find(marker)? + marker.len()..];
                let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
                digits.parse::<u16>().ok().filter(|p| *p > 0)
            })
    })
}

/// The world the running game opened to LAN, read from its log (the caller checks that something
/// listens on the port).
pub fn world_from_logs() -> Option<LanWorld> {
    let root = LOG_ROOT
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()?;
    let log = newest_game_log(&root)?;
    let mut file = std::fs::File::open(log).ok()?;
    let len = file.metadata().ok()?.len();
    file.seek(SeekFrom::Start(len.saturating_sub(LOG_TAIL)))
        .ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    let port = port_from_log(&String::from_utf8_lossy(&bytes))?;
    Some(LanWorld {
        motd: format!("Port {port}"),
        port,
    })
}

/// Socket that hears LAN announcements alongside a running game (which
/// binds the same port), so address reuse is required. The group is joined on every network
/// adapter: the game announces on the one its system picks (a VPN, Hamachi, WSL or a virtual
/// adapter is often first), and a socket that joined only the default one never hears it.
pub fn listener() -> std::io::Result<UdpSocket> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_reuse_address(true)?;
    #[cfg(unix)]
    socket.set_reuse_port(true)?;
    socket.bind(&SocketAddr::from(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, PORT)).into())?;
    let mut joined = false;
    if let Ok(adapters) = if_addrs::get_if_addrs() {
        for adapter in adapters {
            if let std::net::IpAddr::V4(ip) = adapter.ip() {
                joined |= socket.join_multicast_v4(&GROUP, &ip).is_ok();
            }
        }
    }
    if !joined {
        socket.join_multicast_v4(&GROUP, &Ipv4Addr::UNSPECIFIED)?;
    }
    socket.set_nonblocking(true)?;
    UdpSocket::from_std(socket.into())
}

/// Socket for announcing a tunnelled world to games on this machine only
/// (TTL 0 keeps the packets off the network).
pub async fn announcer() -> std::io::Result<UdpSocket> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await?;
    socket.set_multicast_loop_v4(true)?;
    socket.set_multicast_ttl_v4(0)?;
    Ok(socket)
}

/// True if `ip` is an address of this machine: the loopback, or one it can bind (only an address assigned to
/// one of its interfaces can be). An announcement from any other address is another device on the network.
pub fn is_this_machine(ip: std::net::IpAddr) -> bool {
    ip.is_loopback() || std::net::UdpSocket::bind((ip, 0)).is_ok()
}

fn varint(mut value: u32, out: &mut Vec<u8>) {
    loop {
        let byte = (value & 0x7F) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

/// A packet: its length, then its id and body.
fn packet(id: u8, body: &[u8]) -> Vec<u8> {
    let mut inner = vec![id];
    inner.extend_from_slice(body);
    let mut out = Vec::new();
    varint(inner.len() as u32, &mut out);
    out.extend(inner);
    out
}

/// True if a Minecraft server answers a status request on `port` of this machine. A port read from a
/// game's log is only trusted after this: chat from a server can put any line in that log.
pub async fn is_minecraft_server(port: u16) -> bool {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let ask = async {
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).await.ok()?;
        // Handshake (any protocol number is answered), then the status request.
        let mut body = Vec::new();
        varint(765, &mut body);
        varint(9, &mut body);
        body.extend_from_slice(b"localhost");
        body.extend_from_slice(&port.to_be_bytes());
        varint(1, &mut body);
        let mut request = packet(0x00, &body);
        request.extend(packet(0x00, &[]));
        stream.write_all(&request).await.ok()?;
        let mut reply = [0u8; 512];
        let n = stream.read(&mut reply).await.ok()?;
        // The status answer is a JSON text naming the version.
        let text = String::from_utf8_lossy(&reply[..n]);
        Some(text.contains("\"version\"") || text.contains("\"description\""))
    };
    matches!(
        tokio::time::timeout(Duration::from_millis(1500), ask).await,
        Ok(Some(true))
    )
}

/// True if something on this machine accepts connections on `port`, i.e.
/// the announcement came from our own game and not another PC.
pub async fn is_local(port: u16) -> bool {
    let connect = TcpStream::connect((Ipv4Addr::LOCALHOST, port));
    matches!(
        tokio::time::timeout(Duration::from_millis(500), connect).await,
        Ok(Ok(_))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let text = format("Steve's world", 51234);
        assert_eq!(text, "[MOTD]Steve's world[/MOTD][AD]51234[/AD]");
        assert_eq!(
            parse(&text),
            Some(LanWorld {
                motd: "Steve's world".into(),
                port: 51234
            })
        );
    }

    #[test]
    fn brackets_in_names_do_not_break_parsing() {
        let text = format("[cool] world", 1);
        assert_eq!(parse(&text).map(|w| w.motd), Some("(cool) world".into()));
    }

    #[test]
    fn only_addresses_of_this_machine_count() {
        assert!(is_this_machine(Ipv4Addr::LOCALHOST.into()));
        // TEST-NET-1: never assigned to an interface.
        assert!(!is_this_machine(Ipv4Addr::new(192, 0, 2, 1).into()));
    }

    #[test]
    fn finds_the_port_in_a_game_log() {
        let log = "[12:00:01] [Server thread/INFO]: Started serving on 40001
                   [12:00:01] [Render thread/INFO]: [System] [CHAT] Local game hosted on port 40001
                   [12:05:00] [Render thread/INFO]: [CHAT] hello";
        assert_eq!(port_from_log(log), Some(40001));
        assert_eq!(
            port_from_log("Local game hosted on port 1234 and more"),
            Some(1234)
        );
        assert_eq!(port_from_log("nothing here"), None);
        assert_eq!(port_from_log("hosted on port abc"), None);
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(parse("hello"), None);
        assert_eq!(parse("[MOTD]x[/MOTD][AD]notaport[/AD]"), None);
        assert_eq!(parse("[MOTD]x[/MOTD]"), None);
    }
}
