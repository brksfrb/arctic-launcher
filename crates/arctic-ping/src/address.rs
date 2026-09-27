//! Server addresses as typed in Minecraft (`host`, `host:port`,
//! `[v6]:port`), and where they point: an SRV record redirects the default
//! port, like the game does.

use std::net::IpAddr;

pub const DEFAULT_PORT: u16 = 25565;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Address {
    pub host: String,
    pub port: u16,
}

impl Address {
    /// Parse like Minecraft's `ServerAddress.parseString`.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        if text.is_empty() || text.len() > 300 {
            return None;
        }
        let (host, port) = if let Some(rest) = text.strip_prefix('[') {
            let (host, after) = rest.split_once(']')?;
            let port = match after.strip_prefix(':') {
                Some(p) => p.parse().ok()?,
                None if after.is_empty() => DEFAULT_PORT,
                None => return None,
            };
            (host, port)
        } else if text.matches(':').count() > 1 {
            // A bare IPv6 address.
            (text, DEFAULT_PORT)
        } else if let Some((host, port)) = text.split_once(':') {
            (host, port.parse().ok()?)
        } else {
            (text, DEFAULT_PORT)
        };
        if host.is_empty() || port == 0 {
            return None;
        }
        Some(Self {
            host: host.to_ascii_lowercase(),
            port,
        })
    }

    /// The game's `--quickPlayMultiplayer` / display form.
    pub fn display(&self) -> String {
        let host = if self.host.contains(':') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        if self.port == DEFAULT_PORT {
            host
        } else {
            format!("{host}:{}", self.port)
        }
    }

    /// This PC or the local network, judged without a DNS lookup (such
    /// servers skip the proxy, as in the game).
    pub fn is_local(&self) -> bool {
        let h = self.host.as_str();
        if h == "localhost"
            || [".localhost", ".local", ".lan"]
                .iter()
                .any(|s| h.ends_with(s))
        {
            return true;
        }
        match h.parse::<IpAddr>() {
            Ok(IpAddr::V4(ip)) => ip.is_loopback() || ip.is_private() || ip.is_link_local(),
            Ok(IpAddr::V6(ip)) => {
                ip.is_loopback() || ip.is_unique_local() || ip.is_unicast_link_local()
            }
            Err(_) => false,
        }
    }

    /// Where to connect: the SRV target when the default port is used and
    /// a `_minecraft._tcp` record exists, otherwise the address itself.
    pub fn redirected(&self) -> Self {
        if self.port != DEFAULT_PORT || self.host.parse::<IpAddr>().is_ok() {
            return self.clone();
        }
        super::dns::srv(&format!("_minecraft._tcp.{}", self.host))
            .map(|(host, port)| Self {
                host: host.trim_end_matches('.').to_ascii_lowercase(),
                port,
            })
            .unwrap_or_else(|| self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(host: &str, port: u16) -> Option<Address> {
        Some(Address {
            host: host.into(),
            port,
        })
    }

    #[test]
    fn parses_like_minecraft() {
        assert_eq!(
            Address::parse("Play.Example.com"),
            a("play.example.com", 25565)
        );
        assert_eq!(Address::parse(" mc.x.net:25570 "), a("mc.x.net", 25570));
        assert_eq!(Address::parse("[::1]:25566"), a("::1", 25566));
        assert_eq!(Address::parse("[::1]"), a("::1", 25565));
        assert_eq!(Address::parse("fe80::1"), a("fe80::1", 25565));
        assert_eq!(Address::parse("host:notaport"), None);
        assert_eq!(Address::parse("host:0"), None);
        assert_eq!(Address::parse(""), None);
    }

    #[test]
    fn displays_and_spots_local() {
        assert_eq!(Address::parse("a.b:25565").unwrap().display(), "a.b");
        assert_eq!(Address::parse("[::1]:7").unwrap().display(), "[::1]:7");
        assert!(Address::parse("192.168.1.4").unwrap().is_local());
        assert!(Address::parse("localhost:25570").unwrap().is_local());
        assert!(!Address::parse("mc.hypixel.net").unwrap().is_local());
    }
}
