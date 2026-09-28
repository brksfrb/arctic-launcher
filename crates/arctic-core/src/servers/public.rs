//! The public server list on the Arctic server: browse it (a new random
//! order each time, nothing paid or pinned), list your own server (prove
//! it with a code in the MOTD, then an admin approves), and the admin
//! review itself.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::cosmetics::server_error;
use crate::net::agent;
use crate::{Error, Result};

/// Largest list answer read.
const MAX_ANSWER: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PublicServer {
    pub id: String,
    pub address: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    /// `None` until the Arctic server could tell.
    #[serde(default)]
    pub cracked: Option<bool>,
    #[serde(default)]
    pub online: bool,
    #[serde(default)]
    pub players: u32,
    #[serde(default)]
    pub max_players: u32,
    #[serde(default)]
    pub version: String,
}

/// Which servers to show.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Access {
    #[default]
    Any,
    /// Needs a Microsoft account (online mode).
    Premium,
    /// Lets any name in.
    Cracked,
}

impl Access {
    pub fn allows(self, s: &PublicServer) -> bool {
        match self {
            Access::Any => true,
            Access::Premium => s.cracked == Some(false),
            Access::Cracked => s.cracked == Some(true),
        }
    }
}

/// A submission: its id and the code to put in the MOTD.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Submitted {
    pub id: String,
    pub address: String,
    pub code: String,
    pub state: String,
}

fn check(resp: &mut ureq::http::Response<ureq::Body>) -> Result<()> {
    let status = resp.status().as_u16();
    if (200..300).contains(&status) {
        Ok(())
    } else {
        Err(Error::Other(server_error(resp, status)))
    }
}

/// The public list, in the server's random order.
pub fn browse(base: &str) -> Result<Vec<PublicServer>> {
    #[derive(Deserialize)]
    struct List {
        servers: Vec<PublicServer>,
    }
    let mut resp = agent()
        .get(&format!("{base}/v1/servers"))
        .config()
        .http_status_as_error(false)
        .build()
        .call()?;
    check(&mut resp)?;
    let list: List = resp
        .body_mut()
        .with_config()
        .limit(MAX_ANSWER)
        .read_json()?;
    Ok(list.servers)
}

/// One at random among `servers` that `access` allows.
pub fn pick(servers: &[PublicServer], access: Access, seed: u64) -> Option<&PublicServer> {
    let allowed: Vec<&PublicServer> = servers.iter().filter(|s| access.allows(s)).collect();
    (!allowed.is_empty()).then(|| allowed[(seed % allowed.len() as u64) as usize])
}

/// List your server (signed in with an Arctic token).
pub fn submit(
    base: &str,
    token: &str,
    address: &str,
    name: &str,
    description: &str,
    tags: &[String],
) -> Result<Submitted> {
    let mut resp = agent()
        .post(&format!("{base}/v1/servers"))
        .header("Authorization", &format!("Bearer {token}"))
        .config()
        .http_status_as_error(false)
        .build()
        .send_json(
            json!({"address": address, "name": name, "description": description, "tags": tags}),
        )?;
    check(&mut resp)?;
    Ok(resp.body_mut().with_config().limit(4096).read_json()?)
}

/// Ask the Arctic server to look for the code; `pending` when it's found.
pub fn verify(base: &str, token: &str, id: &str) -> Result<String> {
    let mut resp = agent()
        .post(&format!("{base}/v1/servers/{id}/verify"))
        .header("Authorization", &format!("Bearer {token}"))
        .config()
        .http_status_as_error(false)
        .build()
        .send_empty()?;
    check(&mut resp)?;
    let v: Value = resp.body_mut().with_config().limit(4096).read_json()?;
    Ok(v["state"].as_str().unwrap_or("pending").to_owned())
}

/// Admin: submissions in `state` (`pending` by default), with owners and codes.
pub fn review(base: &str, admin_key: &str, state: &str) -> Result<Vec<Value>> {
    let mut resp = agent()
        .get(&format!("{base}/v1/admin/servers?state={state}"))
        .header("X-Admin-Key", admin_key)
        .config()
        .http_status_as_error(false)
        .build()
        .call()?;
    check(&mut resp)?;
    let v: Value = resp
        .body_mut()
        .with_config()
        .limit(MAX_ANSWER)
        .read_json()?;
    Ok(v["servers"].as_array().cloned().unwrap_or_default())
}

/// Admin: `approve`, `reject` or `remove` a server.
pub fn decide(base: &str, admin_key: &str, id: &str, action: &str) -> Result<()> {
    let mut resp = agent()
        .post(&format!("{base}/v1/admin/servers/{id}"))
        .header("X-Admin-Key", admin_key)
        .config()
        .http_status_as_error(false)
        .build()
        .send_json(json!({ "action": action }))?;
    check(&mut resp)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(id: &str, cracked: Option<bool>) -> PublicServer {
        serde_json::from_value(json!({"id": id, "address": "a", "name": id, "cracked": cracked}))
            .unwrap()
    }

    #[test]
    fn picks_within_the_filter() {
        let list = vec![
            server("p", Some(false)),
            server("c", Some(true)),
            server("u", None),
        ];
        assert_eq!(pick(&list, Access::Cracked, 7).unwrap().id, "c");
        assert_eq!(pick(&list, Access::Premium, 3).unwrap().id, "p");
        assert_eq!(pick(&list, Access::Any, 2).unwrap().id, "u");
        assert!(pick(&[], Access::Any, 1).is_none());
    }
}
