//! A Bedrock player's own server list (`minecraftpe\external_servers.txt`,
//! one `id:name:host:port:time` line per server) and live status over
//! RakNet (see `arctic_ping::ping_bedrock`).

use std::fs;
use std::path::{Path, PathBuf};

use super::Player;
use crate::error::IoContext;
use crate::{Error, Result};

pub use arctic_ping::{BEDROCK_PORT, BedrockStatus};

/// A server from the game's Servers tab (the ones added by the player).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Server {
    pub name: String,
    pub host: String,
    pub port: u16,
}

impl Server {
    /// `host`, or `host:port` when it isn't Bedrock's usual port.
    pub fn address(&self) -> String {
        let host = if self.host.contains(':') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        if self.port == BEDROCK_PORT {
            host
        } else {
            format!("{host}:{}", self.port)
        }
    }
}

fn file(player: &Player) -> PathBuf {
    player
        .com_mojang
        .join("minecraftpe")
        .join("external_servers.txt")
}

/// The player's servers, in the game's order. A missing file is an empty list.
pub fn list(player: &Player) -> Vec<Server> {
    list_file(&file(player))
}

fn list_file(path: &Path) -> Vec<Server> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines().filter_map(parse_line).collect()
}

/// `id:name:host:port:time` (the name may itself hold colons).
fn parse_line(line: &str) -> Option<Server> {
    let line = line.trim();
    let (_id, rest) = line.split_once(':')?;
    let mut tail = rest.rsplitn(4, ':');
    let _time = tail.next()?;
    let port: u16 = tail.next()?.trim().parse().ok()?;
    let host = tail.next()?.trim().to_owned();
    let name = tail.next()?.trim().to_owned();
    if host.is_empty() || port == 0 {
        return None;
    }
    Some(Server {
        name: if name.is_empty() { host.clone() } else { name },
        host,
        port,
    })
}

fn write(path: &Path, servers: &[Server]) -> Result<()> {
    let now = crate::auth::now_secs();
    let text: String = servers
        .iter()
        .enumerate()
        .map(|(i, s)| {
            // The format has no escaping: a colon in a name would shift the fields.
            let name = s.name.replace([':', '\n', '\r'], " ");
            format!("{}:{}:{}:{}:{now}\n", i + 1, name.trim(), s.host, s.port)
        })
        .collect();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).at(dir)?;
    }
    let tmp = path.with_extension("txt.arctic-tmp");
    fs::write(&tmp, text).at(&tmp)?;
    fs::rename(&tmp, path).at(path)
}

/// Add a server to the end of the list. `Ok(false)` when it's there already.
pub fn add(player: &Player, name: &str, address: &str) -> Result<bool> {
    let (host, port) = arctic_ping::parse_bedrock_address(address)
        .ok_or_else(|| Error::Other(format!("\"{}\" isn't a server address", address.trim())))?;
    if host.contains(':') {
        return Err(Error::Other(
            "Bedrock's server list can't hold IPv6 addresses".into(),
        ));
    }
    let path = file(player);
    let mut servers = list_file(&path);
    if servers
        .iter()
        .any(|s| s.host.eq_ignore_ascii_case(&host) && s.port == port)
    {
        return Ok(false);
    }
    let name = name.trim();
    servers.push(Server {
        name: if name.is_empty() {
            host.clone()
        } else {
            name.to_owned()
        },
        host,
        port,
    });
    write(&path, &servers)?;
    Ok(true)
}

/// Remove a server (by address). `Ok(false)` when it isn't on the list.
pub fn remove(player: &Player, address: &str) -> Result<bool> {
    let Some((host, port)) = arctic_ping::parse_bedrock_address(address) else {
        return Ok(false);
    };
    let path = file(player);
    let mut servers = list_file(&path);
    let before = servers.len();
    servers.retain(|s| !(s.host.eq_ignore_ascii_case(&host) && s.port == port));
    if servers.len() == before {
        return Ok(false);
    }
    write(&path, &servers)?;
    Ok(true)
}

/// Ping one Bedrock server.
pub fn ping(address: &str) -> Result<BedrockStatus> {
    arctic_ping::ping_bedrock(address).map_err(|e| Error::Other(e.0))
}

/// Ping many Bedrock servers at once, calling `done(index, result)` as each answers.
pub fn ping_each(addresses: &[String], done: impl Fn(usize, Result<BedrockStatus>) + Sync) {
    arctic_ping::ping_bedrock_each(addresses, |i, r| done(i, r.map_err(|e| Error::Other(e.0))));
}

/// Well-known public Bedrock servers (the game's own featured ones), to
/// add in one click.
pub const FEATURED: &[(&str, &str)] = &[
    ("The Hive", "geo.hivebedrock.network"),
    ("CubeCraft", "mco.cubecraft.net"),
    ("Lifeboat", "play.lbsg.net"),
    ("Mineville", "play.inpvp.net"),
    ("Galaxite", "play.galaxite.net"),
    ("Enchanted Dragons", "play.enchanted.gg"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_writes_the_list() {
        let dir = tempfile::tempdir().unwrap();
        let player = Player {
            id: "1".into(),
            com_mojang: dir.path().to_path_buf(),
        };
        assert!(list(&player).is_empty());
        let path = file(&player);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            "1:My: Server:play.example.net:19132:1700000000\n2::10.0.0.2:19140:1700000001\nbroken\n",
        )
        .unwrap();
        let servers = list(&player);
        assert_eq!(servers.len(), 2);
        assert_eq!(servers[0].name, "My: Server");
        assert_eq!(servers[0].address(), "play.example.net");
        assert_eq!(servers[1].name, "10.0.0.2");
        assert_eq!(servers[1].address(), "10.0.0.2:19140");

        assert!(add(&player, "Hive", "geo.hivebedrock.network").unwrap());
        assert!(!add(&player, "Again", "PLAY.example.net:19132").unwrap());
        assert!(add(&player, "x", "host:nope").is_err());
        assert!(remove(&player, "10.0.0.2:19140").unwrap());
        assert!(!remove(&player, "10.0.0.2:19140").unwrap());
        let names: Vec<String> = list(&player).into_iter().map(|s| s.name).collect();
        assert_eq!(names, ["My  Server", "Hive"]);
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("1:My  Server:play.example.net:19132:"));
        assert!(text.contains("\n2:Hive:geo.hivebedrock.network:19132:"));
    }
}
