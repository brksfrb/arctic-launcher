//! Voice rooms on the Arctic server: one per Minecraft server, named by a
//! hash of its address (the address itself never leaves this PC). Checking
//! in returns the other members, whose launchers we connect to directly.

use serde::Deserialize;
use serde_json::json;

use crate::cosmetics::server_error;
use crate::net::agent;
use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Member {
    /// Their launcher's peer id.
    pub node: String,
    /// The UUID they play as on that server.
    pub uuid: String,
    pub name: String,
    pub friend: bool,
}

/// The room for a server address: same server, same room, whatever the
/// capitalization or default port.
pub fn room_for(address: &str) -> String {
    use sha2::{Digest, Sha256};
    let normal = arctic_ping::Address::parse(address)
        .map(|a| a.display())
        .unwrap_or_else(|| address.trim().to_ascii_lowercase());
    hex::encode(Sha256::digest(format!("arctic-voice:{normal}").as_bytes()))
}

/// Check in (every 20 seconds or so while playing) and get the others.
pub fn join(
    base: &str,
    token: &str,
    room: &str,
    node: &str,
    playing_as: &str,
) -> Result<Vec<Member>> {
    #[derive(Deserialize)]
    struct Members {
        members: Vec<Member>,
    }
    let mut resp = agent()
        .post(&format!("{base}/v1/voice/join"))
        .header("Authorization", &format!("Bearer {token}"))
        .config()
        .http_status_as_error(false)
        .build()
        .send_json(json!({ "room": room, "node": node, "as": playing_as }))?;
    let status = resp.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(Error::Other(server_error(&mut resp, status)));
    }
    let m: Members = resp
        .body_mut()
        .with_config()
        .limit(256 * 1024)
        .read_json()?;
    Ok(m.members)
}

pub fn leave(base: &str, token: &str) -> Result<()> {
    let mut resp = agent()
        .post(&format!("{base}/v1/voice/leave"))
        .header("Authorization", &format!("Bearer {token}"))
        .config()
        .http_status_as_error(false)
        .build()
        .send_empty()?;
    let status = resp.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(Error::Other(server_error(&mut resp, status)));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_server_same_room() {
        assert_eq!(
            room_for("Play.Example.NET"),
            room_for("play.example.net:25565")
        );
        assert_ne!(
            room_for("play.example.net"),
            room_for("play.example.net:25570")
        );
        assert_eq!(room_for("x").len(), 64);
    }
}
