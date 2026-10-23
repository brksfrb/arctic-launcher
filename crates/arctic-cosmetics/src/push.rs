//! Phone notifications: Web Push to the web app a moderator installs on their phone (`/notify`). A phone
//! (one "subscription") picks which kinds of events it hears about; the server keeps those choices and the
//! VAPID key that signs its pushes, and sends a short note when something happens (a crash report, a
//! suggestion, a skin or a server waiting for review).
//!
//! Notes carry only a headline: no logs, contacts or addresses.

use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use rusqlite::params;
use serde_json::json;
use web_push_native::jwt_simple::algorithms::ES256KeyPair;
use web_push_native::p256::PublicKey;
use web_push_native::{Auth, WebPushBuilder};

use crate::routes::AppState;
use crate::store::Store;

/// What a phone can listen for: (id, label for the app).
pub const TOPICS: [(&str, &str); 4] = [
    ("crash", "New crash reports"),
    ("suggestion", "New suggestions and bug reports"),
    ("skin", "Skins waiting for review"),
    ("server", "Servers waiting for review"),
];

/// Who the push services may contact about the pushes (required by VAPID).
const CONTACT: &str = "mailto:support@arcticlauncher.com";
const SEND_TIMEOUT: Duration = Duration::from_secs(10);
/// Most phones one account may have.
const MAX_PER_OWNER: i64 = 10;
/// Longest endpoint kept (they are about 200 characters).
const MAX_ENDPOINT: usize = 1024;

/// One phone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subscription {
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
    pub topics: Vec<String>,
}

/// Only known topics, each once, in the order of [`TOPICS`].
pub fn clean_topics(wanted: &[String]) -> Vec<String> {
    TOPICS
        .iter()
        .filter(|(id, _)| wanted.iter().any(|w| w == id))
        .map(|(id, _)| (*id).to_owned())
        .collect()
}

impl Store {
    pub(crate) fn migrate_push(&self) -> rusqlite::Result<()> {
        self.conn().execute_batch(
            "CREATE TABLE IF NOT EXISTS push_keys (
                 id INTEGER PRIMARY KEY CHECK (id = 1),
                 private TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS push_subs (
                 endpoint TEXT PRIMARY KEY,
                 owner TEXT NOT NULL,
                 p256dh TEXT NOT NULL,
                 auth TEXT NOT NULL,
                 topics TEXT NOT NULL,
                 created INTEGER NOT NULL
             );",
        )
    }

    /// The server's VAPID key pair (made the first time it is asked for), as URL-safe base64:
    /// (private, public).
    pub fn push_keys(&self) -> rusqlite::Result<(String, String)> {
        let existing: Option<String> = {
            let conn = self.conn();
            let mut stmt = conn.prepare("SELECT private FROM push_keys WHERE id = 1")?;
            let mut rows = stmt.query([])?;
            rows.next()?.map(|r| r.get(0)).transpose()?
        };
        let private = match existing {
            Some(p) => p,
            None => {
                let made = B64.encode(ES256KeyPair::generate().to_bytes());
                self.conn().execute(
                    "INSERT OR IGNORE INTO push_keys (id, private) VALUES (1, ?1)",
                    params![made],
                )?;
                let conn = self.conn();
                conn.query_row("SELECT private FROM push_keys WHERE id = 1", [], |r| {
                    r.get(0)
                })?
            }
        };
        let public = key_pair(&private)
            .map(|pair| B64.encode(pair.public_key().to_bytes_uncompressed()))
            .ok_or_else(|| rusqlite::Error::InvalidParameterName("push key".into()))?;
        Ok((private, public))
    }

    /// Add or update a phone. `false` when the account already has too many.
    pub fn push_subscribe(
        &self,
        owner: &str,
        sub: &Subscription,
        now: u64,
    ) -> rusqlite::Result<bool> {
        let conn = self.conn();
        let have: i64 = conn.query_row(
            "SELECT COUNT(*) FROM push_subs WHERE owner = ?1 AND endpoint <> ?2",
            params![owner, sub.endpoint],
            |r| r.get(0),
        )?;
        if have >= MAX_PER_OWNER {
            return Ok(false);
        }
        conn.execute(
            "INSERT INTO push_subs (endpoint, owner, p256dh, auth, topics, created)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(endpoint) DO UPDATE SET
                 owner = ?2, p256dh = ?3, auth = ?4, topics = ?5",
            params![
                sub.endpoint,
                owner,
                sub.p256dh,
                sub.auth,
                sub.topics.join(","),
                now as i64
            ],
        )?;
        Ok(true)
    }

    /// The topics a phone listens for; `None` when it isn't subscribed.
    pub fn push_topics(
        &self,
        owner: &str,
        endpoint: &str,
    ) -> rusqlite::Result<Option<Vec<String>>> {
        let conn = self.conn();
        let mut stmt =
            conn.prepare("SELECT topics FROM push_subs WHERE endpoint = ?1 AND owner = ?2")?;
        let mut rows = stmt.query(params![endpoint, owner])?;
        Ok(rows
            .next()?
            .map(|r| r.get::<_, String>(0))
            .transpose()?
            .map(|t| split_topics(&t)))
    }

    /// A phone of this account, with its keys.
    pub fn push_get(&self, owner: &str, endpoint: &str) -> rusqlite::Result<Option<Subscription>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT endpoint, p256dh, auth, topics FROM push_subs WHERE endpoint = ?1 AND owner = ?2",
        )?;
        let mut rows = stmt.query(params![endpoint, owner])?;
        rows.next()?
            .map(|r| {
                Ok(Subscription {
                    endpoint: r.get(0)?,
                    p256dh: r.get(1)?,
                    auth: r.get(2)?,
                    topics: split_topics(&r.get::<_, String>(3)?),
                })
            })
            .transpose()
    }

    /// Change what a phone listens for. `false` when it isn't one of this account's.
    pub fn push_set_topics(
        &self,
        owner: &str,
        endpoint: &str,
        topics: &[String],
    ) -> rusqlite::Result<bool> {
        let changed = self.conn().execute(
            "UPDATE push_subs SET topics = ?3 WHERE endpoint = ?1 AND owner = ?2",
            params![endpoint, owner, topics.join(",")],
        )?;
        Ok(changed > 0)
    }

    pub fn push_unsubscribe(&self, owner: &str, endpoint: &str) -> rusqlite::Result<()> {
        self.conn().execute(
            "DELETE FROM push_subs WHERE endpoint = ?1 AND owner = ?2",
            params![endpoint, owner],
        )?;
        Ok(())
    }

    /// A phone the push service says is gone.
    fn push_forget(&self, endpoint: &str) {
        let _ = self.conn().execute(
            "DELETE FROM push_subs WHERE endpoint = ?1",
            params![endpoint],
        );
    }

    /// Every phone listening for `topic`, with the account it belongs to.
    fn push_listeners(&self, topic: &str) -> rusqlite::Result<Vec<(String, Subscription)>> {
        let conn = self.conn();
        let mut stmt =
            conn.prepare("SELECT owner, endpoint, p256dh, auth, topics FROM push_subs")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                Subscription {
                    endpoint: r.get(1)?,
                    p256dh: r.get(2)?,
                    auth: r.get(3)?,
                    topics: split_topics(&r.get::<_, String>(4)?),
                },
            ))
        })?;
        Ok(rows
            .filter_map(Result::ok)
            .filter(|(_, s)| s.topics.iter().any(|t| t == topic))
            .collect())
    }
}

fn split_topics(text: &str) -> Vec<String> {
    text.split(',')
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
        .collect()
}

fn key_pair(private: &str) -> Option<ES256KeyPair> {
    ES256KeyPair::from_bytes(&B64.decode(private).ok()?).ok()
}

/// The push services phones use (a host is one of these, or a subdomain of one).
const PUSH_HOSTS: [&str; 6] = [
    "fcm.googleapis.com",
    "push.apple.com",
    "push.services.mozilla.com",
    "notify.windows.com",
    "android.googleapis.com",
    "push.microsoft.com",
];

/// Whether a phone's endpoint is an address of a known push service: https on the usual port, no
/// credentials, and a host that is (a subdomain of) one of [`PUSH_HOSTS`]. Anything else could aim the
/// server at some other machine.
pub fn endpoint_ok(endpoint: &str) -> bool {
    if endpoint.len() > MAX_ENDPOINT
        || endpoint
            .chars()
            .any(|c| c.is_whitespace() || c.is_control())
    {
        return false;
    }
    let Some(rest) = endpoint.strip_prefix("https://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.contains('@') {
        return false;
    }
    let host = match authority.split_once(':') {
        Some((host, "443")) => host,
        Some(_) => return false,
        None => authority,
    };
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    PUSH_HOSTS
        .iter()
        .any(|allowed| host == *allowed || host.ends_with(&format!(".{allowed}")))
}

/// Tell the phones listening for `topic` (in the background): `title` and `body` are shown, `url` opens
/// on a tap.
pub fn notify(
    state: &Arc<AppState>,
    topic: &'static str,
    title: String,
    body: String,
    url: String,
) {
    let state = Arc::clone(state);
    tokio::task::spawn_blocking(move || {
        let phones = match state.store.push_listeners(topic) {
            Ok(phones) => phones,
            Err(e) => {
                log::warn!("push: {e}");
                return;
            }
        };
        for (owner, phone) in phones {
            // A key that was taken back stops its phones too.
            if !state.admins.is_moderator(&owner) {
                state.store.push_forget(&phone.endpoint);
                continue;
            }
            send(&state.store, &phone, topic, &title, &body, &url);
        }
    });
}

/// One note to one phone; a phone the push service no longer knows is forgotten.
pub fn send(
    store: &Store,
    phone: &Subscription,
    tag: &str,
    title: &str,
    body: &str,
    url: &str,
) -> bool {
    let Ok((private, _)) = store.push_keys() else {
        return false;
    };
    let Some(pair) = key_pair(&private) else {
        return false;
    };
    let payload = json!({ "title": title, "body": body, "url": url, "tag": tag }).to_string();
    let request = (|| {
        let endpoint = phone.endpoint.parse().ok()?;
        let public = PublicKey::from_sec1_bytes(&B64.decode(&phone.p256dh).ok()?).ok()?;
        let auth = Auth::clone_from_slice(&B64.decode(&phone.auth).ok()?);
        WebPushBuilder::new(endpoint, public, auth)
            .with_vapid(&pair, CONTACT)
            .build(payload)
            .ok()
    })();
    let Some(request) = request else {
        log::warn!("push: a subscription could not be used");
        store.push_forget(&phone.endpoint);
        return false;
    };
    let (parts, body) = request.into_parts();
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(SEND_TIMEOUT))
        .http_status_as_error(false)
        .max_redirects(0)
        .build()
        .into();
    let mut post = agent.post(parts.uri.to_string());
    for (name, value) in &parts.headers {
        if let Ok(value) = value.to_str() {
            post = post.header(name.as_str(), value);
        }
    }
    match post.send(&body[..]) {
        Ok(response) => {
            let status = response.status().as_u16();
            if status == 404 || status == 410 {
                store.push_forget(&phone.endpoint);
            }
            (200..300).contains(&status)
        }
        Err(e) => {
            log::warn!("push: {e}");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Store {
        Store::memory().unwrap()
    }

    fn phone(endpoint: &str, topics: &[&str]) -> Subscription {
        Subscription {
            endpoint: endpoint.into(),
            p256dh: "k".into(),
            auth: "a".into(),
            topics: topics.iter().map(|t| (*t).to_owned()).collect(),
        }
    }

    #[test]
    fn only_known_topics_are_kept() {
        let wanted: Vec<String> = ["skin", "bogus", "crash", "skin"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        assert_eq!(clean_topics(&wanted), vec!["crash", "skin"]);
    }

    #[test]
    fn a_phone_hears_only_what_it_chose() {
        let s = store();
        assert!(
            s.push_subscribe("admin", &phone("https://p/1", &["crash"]), 1)
                .unwrap()
        );
        assert!(
            s.push_subscribe("admin", &phone("https://p/2", &["skin", "crash"]), 2)
                .unwrap()
        );
        let crash = s.push_listeners("crash").unwrap();
        assert_eq!(crash.len(), 2);
        let skin = s.push_listeners("skin").unwrap();
        assert_eq!(skin.len(), 1);
        assert_eq!(skin[0].1.endpoint, "https://p/2");
        assert!(s.push_listeners("server").unwrap().is_empty());
    }

    #[test]
    fn topics_change_and_a_phone_can_leave() {
        let s = store();
        s.push_subscribe("admin", &phone("https://p/1", &["crash"]), 1)
            .unwrap();
        assert!(
            s.push_set_topics("admin", "https://p/1", &["server".into()])
                .unwrap()
        );
        assert_eq!(
            s.push_topics("admin", "https://p/1").unwrap(),
            Some(vec!["server".to_owned()])
        );
        // Another account's phone is not changed.
        assert!(
            !s.push_set_topics("other", "https://p/1", &["skin".into()])
                .unwrap()
        );
        s.push_unsubscribe("admin", "https://p/1").unwrap();
        assert_eq!(s.push_topics("admin", "https://p/1").unwrap(), None);
    }

    #[test]
    fn an_account_has_a_limited_number_of_phones() {
        let s = store();
        for i in 0..MAX_PER_OWNER {
            assert!(
                s.push_subscribe("admin", &phone(&format!("https://p/{i}"), &[]), 1)
                    .unwrap()
            );
        }
        assert!(
            !s.push_subscribe("admin", &phone("https://p/extra", &[]), 1)
                .unwrap()
        );
        // The same phone again is only an update.
        assert!(
            s.push_subscribe("admin", &phone("https://p/0", &["crash"]), 2)
                .unwrap()
        );
    }

    #[test]
    fn the_push_key_is_made_once() {
        let s = store();
        let (private, public) = s.push_keys().unwrap();
        assert_eq!(s.push_keys().unwrap(), (private, public.clone()));
        // An uncompressed P-256 point is 65 bytes.
        assert_eq!(B64.decode(public).unwrap().len(), 65);
    }

    #[test]
    fn only_https_endpoints_are_accepted() {
        assert!(endpoint_ok("https://web.push.apple.com/abc"));
        assert!(endpoint_ok("https://fcm.googleapis.com/fcm/send/xyz"));
        assert!(endpoint_ok(
            "https://updates.push.services.mozilla.com:443/wpush/v2/x"
        ));
        assert!(endpoint_ok("https://WEB.PUSH.APPLE.COM./abc"));
        assert!(!endpoint_ok("http://web.push.apple.com/x"));
        assert!(!endpoint_ok("https://a b"));
        // Not a push service, or made to look like one.
        assert!(!endpoint_ok("https://example.com/push.apple.com"));
        assert!(!endpoint_ok("https://evilpush.apple.com.example.com/x"));
        assert!(!endpoint_ok("https://notpush.apple.com/x"));
        assert!(!endpoint_ok("https://web.push.apple.com@169.254.169.254/x"));
        assert!(!endpoint_ok("https://127.0.0.1/x"));
        assert!(!endpoint_ok("https://web.push.apple.com:8080/x"));
    }
}
