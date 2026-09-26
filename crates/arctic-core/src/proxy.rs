//! SOCKS5 proxy for the launcher's own traffic and the game.
//!
//! Kept in `proxy.json` next to `accounts.json` (it can hold a password),
//! not in `settings.json`. The launcher's HTTP goes through it with remote
//! DNS (`socks5h`), the game gets it as Minecraft's `--proxy*` arguments
//! (login and skin services), and the Arctic Client routes server
//! connections through it too.

use serde::{Deserialize, Serialize};

use crate::Result;
use crate::storage::{DataDirs, load_json, save_json};

/// The usual SOCKS port, used when none is given.
pub const DEFAULT_PORT: u16 = 1080;
const MAX_HOST_LEN: usize = 253;
const MAX_CREDENTIAL_LEN: usize = 255;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProxySettings {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub username: String,
    /// Encrypted on disk (see [`crate::secret`]).
    #[serde(with = "crate::secret::on_disk")]
    pub password: String,
    /// When these were last changed in the launcher (Unix seconds). The game
    /// adopts a newer change but keeps one made in game until then.
    pub set: u64,
}

impl Default for ProxySettings {
    fn default() -> Self {
        Self {
            enabled: false,
            host: String::new(),
            port: DEFAULT_PORT,
            username: String::new(),
            password: String::new(),
            set: 0,
        }
    }
}

impl ProxySettings {
    pub fn load(dirs: &DataDirs) -> Self {
        let path = dirs.proxy_file();
        let settings = load_json::<Self>(&path).ok().flatten().unwrap_or_default();
        // Files from before encryption: save them encrypted right away.
        if std::fs::read_to_string(&path).is_ok_and(|t| crate::secret::has_plain(&t, &["password"]))
        {
            let _ = settings.save(dirs);
        }
        settings
    }

    pub fn save(&self, dirs: &DataDirs) -> Result<()> {
        save_json(&dirs.proxy_file(), self)
    }

    /// The proxy to use: `Some` only when switched on and valid.
    pub fn active(&self) -> Option<&Self> {
        (self.enabled && self.validate().is_ok()).then_some(self)
    }

    /// Why these settings can't be used, in words for the settings page.
    pub fn validate(&self) -> std::result::Result<(), String> {
        let host = self.host.trim();
        if host.is_empty() {
            return Err("Enter the proxy's address.".into());
        }
        if host.len() > MAX_HOST_LEN
            || !host
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '[' | ']'))
        {
            return Err("That doesn't look like a host name or IP address.".into());
        }
        if self.port == 0 {
            return Err("Enter the proxy's port (1080 is common).".into());
        }
        if self.username.len() > MAX_CREDENTIAL_LEN || self.password.len() > MAX_CREDENTIAL_LEN {
            return Err("The user name or password is too long for SOCKS5.".into());
        }
        if !self.password.is_empty() && self.username.is_empty() {
            return Err("A password needs a user name too.".into());
        }
        Ok(())
    }

    /// The host without IPv6 brackets.
    pub fn bare_host(&self) -> &str {
        self.host
            .trim()
            .trim_start_matches('[')
            .trim_end_matches(']')
    }

    /// `socks5h://` URL for the launcher's HTTP client (the proxy resolves
    /// names, so lookups don't leak to the local DNS).
    pub fn url(&self) -> String {
        let host = if self.bare_host().contains(':') {
            format!("[{}]", self.bare_host())
        } else {
            self.bare_host().to_owned()
        };
        let auth = if self.username.is_empty() {
            String::new()
        } else if self.password.is_empty() {
            format!("{}@", encode(&self.username))
        } else {
            format!("{}:{}@", encode(&self.username), encode(&self.password))
        };
        format!("socks5h://{auth}{host}:{}", self.port)
    }

    /// Minecraft's own proxy arguments (its login, skin and Realms services).
    pub fn game_args(&self) -> Vec<String> {
        let mut args = vec![
            "--proxyHost".into(),
            self.bare_host().to_owned(),
            "--proxyPort".into(),
            self.port.to_string(),
        ];
        if !self.username.is_empty() {
            args.extend(["--proxyUser".into(), self.username.clone()]);
            args.extend(["--proxyPass".into(), self.password.clone()]);
        }
        args
    }
}

/// File name of the game's private hosts file (in the game folder).
pub const HOSTS_FILE: &str = ".arctic-hosts";

impl ProxySettings {
    /// A hosts file for the game while proxied. Java resolves names only
    /// from it (`-Djdk.net.hosts.file`), so every other name fails to
    /// resolve locally and Java hands it to the proxy by name instead:
    /// no DNS lookups leak from Minecraft's web services. Only this PC and
    /// the proxy itself are listed.
    pub fn hosts_file(&self) -> String {
        let mut lines = vec![
            "# Written by Arctic Launcher while a proxy is on.".to_owned(),
            "127.0.0.1 localhost".to_owned(),
            "::1 localhost".to_owned(),
        ];
        if let Some(name) = crate::system::host_name() {
            lines.push(format!("127.0.0.1 {name}"));
        }
        let host = self.bare_host();
        if host.parse::<std::net::IpAddr>().is_err() {
            use std::net::ToSocketAddrs;
            if let Ok(addrs) = (host, self.port).to_socket_addrs() {
                lines.extend(addrs.map(|a| format!("{} {host}", a.ip())));
            }
        }
        lines.join(
            "
",
        ) + "
"
    }
}

/// Percent-encode everything but unreserved URL characters.
fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proxy(host: &str, user: &str, pass: &str) -> ProxySettings {
        ProxySettings {
            enabled: true,
            host: host.into(),
            username: user.into(),
            password: pass.into(),
            ..ProxySettings::default()
        }
    }

    #[test]
    fn url_encodes_credentials_and_resolves_remotely() {
        assert_eq!(proxy("127.0.0.1", "", "").url(), "socks5h://127.0.0.1:1080");
        assert_eq!(
            proxy("proxy.example", "me", "p@ss:word").url(),
            "socks5h://me:p%40ss%3Aword@proxy.example:1080"
        );
        assert_eq!(proxy("::1", "", "").url(), "socks5h://[::1]:1080");
    }

    #[test]
    fn validation() {
        assert!(proxy("", "", "").validate().is_err());
        assert!(proxy("bad host", "", "").validate().is_err());
        assert!(proxy("host/../x", "", "").validate().is_err());
        assert!(proxy("10.0.0.2", "", "secret").validate().is_err());
        assert!(proxy("10.0.0.2", "me", "secret").validate().is_ok());
        assert!(proxy("[2001:db8::1]", "", "").validate().is_ok());
        let off = ProxySettings {
            enabled: false,
            ..proxy("10.0.0.2", "", "")
        };
        assert!(off.active().is_none());
    }

    #[test]
    fn game_args_include_credentials_only_when_set() {
        assert_eq!(
            proxy("10.0.0.2", "", "").game_args(),
            ["--proxyHost", "10.0.0.2", "--proxyPort", "1080"]
        );
        assert_eq!(proxy("10.0.0.2", "me", "pw").game_args().len(), 8);
    }
}
