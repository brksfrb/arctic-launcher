//! An instance's saved servers (Minecraft's `servers.dat`) and their live
//! status: who's online, the MOTD, version and ping, so you can see if
//! friends are on before starting the game. The pinging itself is
//! `arctic-ping`, shared with the Arctic server.

mod nbt;
pub mod public;

use std::path::Path;

use base64::Engine;

pub use arctic_ping::{Address, DEFAULT_PORT, Status};

use crate::proxy::ProxySettings;
use crate::{Error, Result};

/// The multiplayer list in a game folder.
pub const SERVERS_FILE: &str = "servers.dat";
/// Largest servers.dat read (icons are stored inline).
const MAX_FILE: u64 = 16 * 1024 * 1024;

/// A server from the multiplayer list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Server {
    pub name: String,
    /// As typed in the game (`play.example.net`, `host:25570`).
    pub address: String,
    /// The icon the game cached last time (PNG).
    pub icon: Option<Vec<u8>>,
}

/// The multiplayer list of the game in `game_dir`, in order. Entries the
/// game hides are left out; a missing file is an empty list.
pub fn list(game_dir: &Path) -> Result<Vec<Server>> {
    let path = game_dir.join(SERVERS_FILE);
    let data = match std::fs::read(&path) {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(Error::io(&path, e)),
    };
    if data.len() as u64 > MAX_FILE {
        return Err(Error::Other("servers.dat is too big to read".into()));
    }
    let root = nbt::read_root(&data)
        .ok_or_else(|| Error::Other("servers.dat couldn't be read (damaged?)".into()))?;
    let Some(entries) = root.get("servers").and_then(nbt::Tag::as_list) else {
        return Ok(Vec::new());
    };
    Ok(entries
        .iter()
        .filter(|e| e.get("hidden").and_then(nbt::Tag::as_byte) != Some(1))
        .filter_map(|e| {
            let address = e.get("ip")?.as_str()?.trim().to_owned();
            if address.is_empty() {
                return None;
            }
            let name = e
                .get("name")
                .and_then(nbt::Tag::as_str)
                .filter(|n| !n.trim().is_empty())
                .unwrap_or(&address)
                .to_owned();
            let icon = e
                .get("icon")
                .and_then(nbt::Tag::as_str)
                .and_then(|b64| base64::engine::general_purpose::STANDARD.decode(b64).ok());
            Some(Server {
                name,
                address,
                icon,
            })
        })
        .collect())
}

/// One server list made from several `servers.dat` files: every server
/// once (by address), in the order first seen, entries kept as the game
/// wrote them. Written to `into`; returns how many servers it holds.
pub fn merge_files(sources: &[std::path::PathBuf], into: &Path) -> Result<usize> {
    let mut merged: Vec<nbt::Tag> = Vec::new();
    for path in sources {
        let Ok(data) = std::fs::read(path) else {
            continue;
        };
        if data.len() as u64 > MAX_FILE {
            continue;
        }
        let Some(root) = nbt::read_root(&data) else {
            continue;
        };
        for entry in root
            .get("servers")
            .and_then(nbt::Tag::as_list)
            .unwrap_or(&[])
        {
            let Some(ip) = entry.get("ip").and_then(nbt::Tag::as_str) else {
                continue;
            };
            let ip = ip.trim();
            let known = merged.iter().any(|e| {
                e.get("ip")
                    .and_then(nbt::Tag::as_str)
                    .is_some_and(|other| other.trim().eq_ignore_ascii_case(ip))
            });
            if !ip.is_empty() && !known {
                merged.push(entry.clone());
            }
        }
    }
    let count = merged.len();
    let root = nbt::Tag::Compound(vec![("servers".into(), nbt::Tag::List(merged))]);
    if let Some(dir) = into.parent() {
        std::fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    }
    let tmp = into.with_extension("dat.arctic-tmp");
    std::fs::write(&tmp, nbt::write_root(&root)).map_err(|e| Error::io(&tmp, e))?;
    std::fs::rename(&tmp, into).map_err(|e| Error::io(into, e))?;
    Ok(count)
}

/// Add a server to the end of the game's multiplayer list (as the game
/// does). Everything already in the file is kept as it was. `Ok(false)`
/// when that address is on the list already.
pub fn add(game_dir: &Path, name: &str, address: &str) -> Result<bool> {
    let address = address.trim();
    if Address::parse(address).is_none() {
        return Err(Error::Other(format!(
            "\"{address}\" isn't a server address"
        )));
    }
    let path = game_dir.join(SERVERS_FILE);
    let mut root = match std::fs::read(&path) {
        Ok(data) => nbt::read_root(&data)
            .ok_or_else(|| Error::Other("servers.dat couldn't be read (damaged?)".into()))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => nbt::Tag::Compound(Vec::new()),
        Err(e) => return Err(Error::io(&path, e)),
    };
    let nbt::Tag::Compound(entries) = &mut root else {
        return Err(Error::Other(
            "servers.dat couldn't be read (damaged?)".into(),
        ));
    };
    if !entries.iter().any(|(k, _)| k == "servers") {
        entries.push(("servers".into(), nbt::Tag::List(Vec::new())));
    }
    let Some((_, nbt::Tag::List(servers))) = entries.iter_mut().find(|(k, _)| k == "servers")
    else {
        return Err(Error::Other(
            "servers.dat couldn't be read (damaged?)".into(),
        ));
    };
    let same = |e: &nbt::Tag| {
        e.get("ip")
            .and_then(nbt::Tag::as_str)
            .is_some_and(|ip| ip.trim().eq_ignore_ascii_case(address))
    };
    if servers.iter().any(same) {
        return Ok(false);
    }
    let name = if name.trim().is_empty() {
        address
    } else {
        name.trim()
    };
    servers.push(nbt::Tag::Compound(vec![
        ("name".into(), nbt::Tag::String(name.to_owned())),
        ("ip".into(), nbt::Tag::String(address.to_owned())),
    ]));
    std::fs::create_dir_all(game_dir).map_err(|e| Error::io(game_dir, e))?;
    let tmp = path.with_extension("dat.arctic-tmp");
    std::fs::write(&tmp, nbt::write_root(&root)).map_err(|e| Error::io(&tmp, e))?;
    std::fs::rename(&tmp, &path).map_err(|e| Error::io(&path, e))?;
    Ok(true)
}

/// Ping one server (through the proxy when it's on).
pub fn ping(address: &str, proxy: Option<&ProxySettings>) -> Result<Status> {
    arctic_ping::ping(address, proxy.map(socks)).map_err(|e| Error::Other(e.0))
}

/// Ping many servers at once; results come back in the same order.
pub fn ping_all(addresses: &[String], proxy: Option<&ProxySettings>) -> Vec<Result<Status>> {
    arctic_ping::ping_all(addresses, proxy.map(socks))
        .into_iter()
        .map(|r| r.map_err(|e| Error::Other(e.0)))
        .collect()
}

fn socks(p: &ProxySettings) -> arctic_ping::Socks<'_> {
    arctic_ping::Socks {
        host: p.host.trim(),
        port: p.port,
        username: &p.username,
        password: &p.password,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_visible_servers_in_order() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(SERVERS_FILE),
            nbt::fake_servers_dat(&[
                ("Hypixel", "mc.hypixel.net", false),
                ("", "play.example.net:25570", false),
                ("Direct", "10.0.0.2", true),
            ]),
        )
        .unwrap();
        let servers = list(dir.path()).unwrap();
        assert_eq!(servers.len(), 2);
        assert_eq!(servers[0].name, "Hypixel");
        assert_eq!(servers[1].name, "play.example.net:25570");
        assert!(list(&dir.path().join("none")).unwrap().is_empty());
    }

    #[test]
    fn damaged_file_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(SERVERS_FILE), [10, 0, 0, 9, 0]).unwrap();
        assert!(list(dir.path()).is_err());
    }

    #[test]
    fn adds_to_the_list_keeping_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        assert!(add(dir.path(), "", "new.example.net").unwrap());
        assert_eq!(list(dir.path()).unwrap()[0].name, "new.example.net");
        std::fs::write(
            dir.path().join(SERVERS_FILE),
            nbt::fake_servers_dat(&[
                ("Hypixel", "mc.hypixel.net", false),
                ("Hidden", "10.0.0.2", true),
            ]),
        )
        .unwrap();
        assert!(add(dir.path(), "Cube", "play.cubecraft.net").unwrap());
        assert!(!add(dir.path(), "Again", "MC.hypixel.net").unwrap());
        assert!(add(dir.path(), "x", "host:0").is_err());
        let names: Vec<String> = list(dir.path())
            .unwrap()
            .into_iter()
            .map(|s| s.name)
            .collect();
        assert_eq!(names, ["Hypixel", "Cube"]);
        // The hidden entry is still in the file.
        let data = std::fs::read(dir.path().join(SERVERS_FILE)).unwrap();
        assert!(String::from_utf8_lossy(&data).contains("10.0.0.2"));
    }
}
