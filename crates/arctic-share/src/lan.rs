//! Minecraft's "Open to LAN" announcements: the game multicasts
//! `[MOTD]name[/MOTD][AD]port[/AD]` to 224.0.2.60:4445 every 1.5 s, and
//! the Multiplayer screen lists every announcement it hears.

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::Duration;

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

/// Socket that hears LAN announcements alongside a running game (which
/// binds the same port), so address reuse is required.
pub fn listener() -> std::io::Result<UdpSocket> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_reuse_address(true)?;
    #[cfg(unix)]
    socket.set_reuse_port(true)?;
    socket.bind(&SocketAddr::from(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, PORT)).into())?;
    socket.join_multicast_v4(&GROUP, &Ipv4Addr::UNSPECIFIED)?;
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
    fn rejects_garbage() {
        assert_eq!(parse("hello"), None);
        assert_eq!(parse("[MOTD]x[/MOTD][AD]notaport[/AD]"), None);
        assert_eq!(parse("[MOTD]x[/MOTD]"), None);
    }
}
