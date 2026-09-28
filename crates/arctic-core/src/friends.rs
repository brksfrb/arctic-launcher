//! Friends on the Arctic server. One Arctic profile is one person: their
//! Minecraft accounts are linked into it (each proved by signing in with
//! it), and friends, requests and invites belong to the profile. What
//! friends see (online, which server, invites, linked accounts) is each
//! person's own choice. Offline accounts take part too (found by friend
//! code; a recovery code moves them to a new PC).

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::cosmetics::server_error;
use crate::net::agent;
use crate::{Error, Result};

const MAX_ANSWER: u64 = 1024 * 1024;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    pub share_online: bool,
    pub share_server: bool,
    pub allow_invites: bool,
    pub show_accounts: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LinkedAccount {
    pub uuid: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    /// Share this so people can add you (`abcd-efgh`).
    #[serde(default)]
    pub code: String,
    /// Has a Microsoft account.
    #[serde(default)]
    pub verified: bool,
    #[serde(default)]
    pub has_recovery: bool,
    pub accounts: Vec<LinkedAccount>,
    pub settings: Settings,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Friend {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub accounts: Vec<LinkedAccount>,
    pub online: bool,
    pub in_game: bool,
    #[serde(default)]
    pub server: Option<String>,
    pub takes_invites: bool,
    #[serde(default)]
    pub verified: bool,
    /// Messages from them not read yet.
    #[serde(default)]
    pub unread: u32,
}

/// A chat message between friends (profile ids).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Message {
    pub id: i64,
    pub from: String,
    pub to: String,
    pub text: String,
    /// A screenshot's attachment id ([`image`] fetches it).
    #[serde(default)]
    pub image: Option<String>,
    pub sent: u64,
    #[serde(default)]
    pub read: bool,
}

#[derive(Deserialize)]
struct Messages {
    messages: Vec<Message>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Person {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Invite {
    pub id: String,
    pub from: Person,
    /// `server` or `together`.
    pub kind: String,
    pub target: String,
    pub sent: u64,
}

/// Everything the Friends page shows.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Overview {
    pub friends: Vec<Friend>,
    pub incoming: Vec<Person>,
    pub outgoing: Vec<Person>,
    pub invites: Vec<Invite>,
}

/// What an invite points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InviteTo {
    Server(String),
    Together(String),
}

fn call(
    method: &str,
    url: &str,
    token: &str,
    body: Option<Value>,
) -> Result<ureq::http::Response<ureq::Body>> {
    let auth = format!("Bearer {token}");
    let a = agent();
    let resp = match (method, body) {
        ("GET", _) => a
            .get(url)
            .header("Authorization", &auth)
            .config()
            .http_status_as_error(false)
            .build()
            .call(),
        ("DELETE", _) => a
            .delete(url)
            .header("Authorization", &auth)
            .config()
            .http_status_as_error(false)
            .build()
            .call(),
        ("PUT", Some(b)) => a
            .put(url)
            .header("Authorization", &auth)
            .config()
            .http_status_as_error(false)
            .build()
            .send_json(b),
        (_, Some(b)) => a
            .post(url)
            .header("Authorization", &auth)
            .config()
            .http_status_as_error(false)
            .build()
            .send_json(b),
        (_, None) => a
            .post(url)
            .header("Authorization", &auth)
            .config()
            .http_status_as_error(false)
            .build()
            .send_empty(),
    };
    let mut resp = resp?;
    let status = resp.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(Error::Other(server_error(&mut resp, status)));
    }
    Ok(resp)
}

fn json_of<T: serde::de::DeserializeOwned>(
    mut resp: ureq::http::Response<ureq::Body>,
) -> Result<T> {
    Ok(resp
        .body_mut()
        .with_config()
        .limit(MAX_ANSWER)
        .read_json()?)
}

pub fn profile(base: &str, token: &str) -> Result<Profile> {
    json_of(call("GET", &format!("{base}/v1/profile"), token, None)?)
}

/// Change the display name and/or privacy settings.
pub fn update(base: &str, token: &str, name: Option<&str>, settings: &Settings) -> Result<Profile> {
    let mut body = serde_json::to_value(settings)?;
    if let Some(name) = name {
        body["name"] = json!(name);
    }
    json_of(call(
        "PUT",
        &format!("{base}/v1/profile"),
        token,
        Some(body),
    )?)
}

/// Link the account behind `other_token` into this profile.
pub fn link(base: &str, token: &str, other_token: &str) -> Result<Profile> {
    json_of(call(
        "POST",
        &format!("{base}/v1/profile/link"),
        token,
        Some(json!({ "token": other_token })),
    )?)
}

pub fn unlink(base: &str, token: &str, uuid: &str) -> Result<Profile> {
    json_of(call(
        "POST",
        &format!("{base}/v1/profile/unlink"),
        token,
        Some(json!({ "uuid": uuid })),
    )?)
}

pub fn overview(base: &str, token: &str) -> Result<Overview> {
    json_of(call("GET", &format!("{base}/v1/friends"), token, None)?)
}

/// A new recovery code for moving this profile's offline accounts to
/// another PC (shown once; it replaces any earlier one).
pub fn new_recovery(base: &str, token: &str) -> Result<String> {
    let v: Value = json_of(call(
        "POST",
        &format!("{base}/v1/profile/recovery"),
        token,
        None,
    )?)?;
    v["code"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| Error::Other("the Arctic server sent no code".into()))
}

/// Ask someone, by friend code or a Microsoft account's name. `true` when that made
/// you friends right away (they had asked you).
pub fn request(base: &str, token: &str, name: &str) -> Result<Requested> {
    let v: Value = json_of(call(
        "POST",
        &format!("{base}/v1/friends"),
        token,
        Some(json!({ "name": name.trim() })),
    )?)?;
    Ok(Requested {
        friends: v["friends"].as_bool().unwrap_or(false),
        name: v["name"].as_str().unwrap_or(name).to_owned(),
    })
}

/// A friend request's outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Requested {
    /// They had asked you too, so you're friends now.
    pub friends: bool,
    /// Their profile name.
    pub name: String,
}

pub fn accept(base: &str, token: &str, profile: &str) -> Result<()> {
    call(
        "POST",
        &format!("{base}/v1/friends/{profile}/accept"),
        token,
        None,
    )
    .map(|_| ())
}

/// Decline, cancel or unfriend.
pub fn remove(base: &str, token: &str, profile: &str) -> Result<()> {
    call(
        "DELETE",
        &format!("{base}/v1/friends/{profile}"),
        token,
        None,
    )
    .map(|_| ())
}

pub fn invite(base: &str, token: &str, friend: &str, to: &InviteTo) -> Result<()> {
    let (kind, target) = match to {
        InviteTo::Server(a) => ("server", a),
        InviteTo::Together(c) => ("together", c),
    };
    call(
        "POST",
        &format!("{base}/v1/invites"),
        token,
        Some(json!({ "to": friend, "kind": kind, "target": target })),
    )
    .map(|_| ())
}

pub fn dismiss(base: &str, token: &str, invite: &str) -> Result<()> {
    call(
        "DELETE",
        &format!("{base}/v1/invites/{invite}"),
        token,
        None,
    )
    .map(|_| ())
}

pub fn send(base: &str, token: &str, friend: &str, text: &str) -> Result<Message> {
    send_with(base, token, friend, text, None)
}

/// A message with an uploaded screenshot ([`upload`]).
pub fn send_with(
    base: &str,
    token: &str,
    friend: &str,
    text: &str,
    image: Option<&str>,
) -> Result<Message> {
    json_of(call(
        "POST",
        &format!("{base}/v1/messages"),
        token,
        Some(json!({ "to": friend, "text": text, "image": image })),
    )?)
}

/// Largest screenshot the Arctic server takes.
pub const MAX_UPLOAD: usize = 2 * 1024 * 1024;

/// Upload a screenshot (a PNG, already shrunk); returns its id.
pub fn upload(base: &str, token: &str, png: &[u8]) -> Result<String> {
    if png.len() > MAX_UPLOAD {
        return Err(Error::Other("that screenshot is too big to send".into()));
    }
    let mut resp = agent()
        .post(&format!("{base}/v1/attachments"))
        .header("Authorization", &format!("Bearer {token}"))
        .header("Content-Type", "image/png")
        .config()
        .http_status_as_error(false)
        .build()
        .send(png)?;
    let status = resp.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(Error::Other(server_error(&mut resp, status)));
    }
    let v: Value = resp.body_mut().with_config().limit(4096).read_json()?;
    v["id"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| Error::Other("the Arctic server sent no id".into()))
}

/// A screenshot from one of your chats (PNG).
pub fn image(base: &str, token: &str, id: &str) -> Result<Vec<u8>> {
    let mut resp = call("GET", &format!("{base}/v1/attachments/{id}"), token, None)?;
    Ok(resp
        .body_mut()
        .with_config()
        .limit(MAX_UPLOAD as u64)
        .read_to_vec()?)
}

/// The conversation with a friend, newest last (the page before `before`).
pub fn history(base: &str, token: &str, friend: &str, before: Option<i64>) -> Result<Vec<Message>> {
    let mut url = format!("{base}/v1/messages?with={friend}");
    if let Some(b) = before {
        url.push_str(&format!("&before={b}"));
    }
    Ok(json_of::<Messages>(call("GET", &url, token, None)?)?.messages)
}

/// Messages to you newer than `after`, oldest first.
pub fn new_messages(base: &str, token: &str, after: i64) -> Result<Vec<Message>> {
    Ok(json_of::<Messages>(call(
        "GET",
        &format!("{base}/v1/messages/new?after={after}"),
        token,
        None,
    )?)?
    .messages)
}

/// You've seen everything from `friend`.
pub fn mark_read(base: &str, token: &str, friend: &str) -> Result<()> {
    call(
        "POST",
        &format!("{base}/v1/messages/read"),
        token,
        Some(json!({ "with": friend })),
    )
    .map(|_| ())
}

/// "The launcher is open" (every minute or so while it runs).
pub fn presence(base: &str, token: &str) -> Result<()> {
    call("POST", &format!("{base}/v1/presence"), token, None).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_servers_answers() {
        let o: Overview = serde_json::from_value(json!({
            "friends": [{"id": "p1", "name": "Bob", "online": true, "in_game": true,
                "server": "mc.x.net", "takes_invites": true}],
            "incoming": [{"id": "p2", "name": "Carol"}], "outgoing": [],
            "invites": [{"id": "i", "from": {"id": "p1", "name": "Bob"}, "kind": "server",
                "target": "mc.x.net", "sent": 5}]
        }))
        .unwrap();
        assert_eq!(o.friends[0].server.as_deref(), Some("mc.x.net"));
        assert!(o.friends[0].accounts.is_empty());
        assert_eq!(o.invites[0].from.name, "Bob");
        let p: Profile = serde_json::from_value(json!({"id": "p", "name": "Me",
            "accounts": [{"uuid": "u", "name": "Me"}],
            "settings": {"share_online": true, "share_server": true, "allow_invites": true, "show_accounts": false}}))
        .unwrap();
        assert!(!p.settings.show_accounts);
    }
}
