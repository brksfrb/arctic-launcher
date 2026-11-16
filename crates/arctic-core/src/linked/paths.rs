//! Which files a linked instance may carry besides its mod list: plain-text
//! config files under `config/`, and nothing personal. Every path in a
//! manifest goes through [`check_config_path`] before anything is read or
//! written, on both sides.

use std::path::{Component, Path, PathBuf};

use crate::{Error, Result};

/// The only folder config files come from and go to.
pub const CONFIG_DIR: &str = "config";
/// Longest relative path (`config/…`).
const MAX_PATH: usize = 240;
/// Folders deep, `config` included.
const MAX_DEPTH: usize = 8;

/// Config formats mods write as text. Anything else (binary caches, databases, jars) stays home.
const EXTENSIONS: &[&str] = &[
    "json",
    "json5",
    "jsonc",
    "toml",
    "properties",
    "cfg",
    "conf",
    "txt",
    "yml",
    "yaml",
    "ini",
    "snbt",
];

/// Folders under `config/` that hold history, caches or per-world data, never settings to share.
const PRIVATE_DIRS: &[&str] = &[
    "backup",
    "backups",
    "cache",
    "caches",
    "crash-reports",
    "log",
    "logs",
    "recordings",
    "replays",
    "saves",
    "screenshots",
    "waypoints",
    "world",
    "worlds",
];

/// Words in a file or folder name that mean it holds something personal: logins, keys, key
/// bindings, server lists.
const PRIVATE_WORDS: &[&str] = &[
    "account",
    "apikey",
    "api_key",
    "api-key",
    "auth",
    "cookie",
    "credential",
    "hotkey",
    "keybind",
    "key_bind",
    "key-bind",
    "keymap",
    "password",
    "passwd",
    "private",
    "secret",
    "servers",
    "session",
    "token",
];

/// Names Windows keeps for devices (`con.json` can't be a file there).
const RESERVED: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// Why a config file can't be shared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// Not a plain relative path inside `config/` (`..`, absolute, odd characters).
    OutsideConfig,
    /// Not a text config format.
    NotConfig,
    /// Arctic's own settings, logins, key bindings, server lists, caches…
    Personal,
}

impl Refused {
    pub fn message(self) -> &'static str {
        match self {
            Refused::OutsideConfig => "not a file inside the config folder",
            Refused::NotConfig => "not a text config file",
            Refused::Personal => "personal (logins, keys, key bindings or Arctic's own settings)",
        }
    }
}

/// `path` (with `/` between folders, relative to the game folder) if a linked instance may carry
/// it, else why not. Path traversal, absolute paths, drive letters, backslashes, hidden or
/// reserved names, and anything outside `config/` are refused.
pub fn check_config_path(path: &str) -> std::result::Result<String, Refused> {
    use Refused::*;
    if path.is_empty() || path.len() > MAX_PATH {
        return Err(OutsideConfig);
    }
    let plain = path.chars().all(|c| {
        c.is_alphanumeric()
            || matches!(c, '/' | '.' | '_' | '-' | ' ' | '+' | '(' | ')' | '[' | ']')
    });
    if !plain || path.starts_with('/') {
        return Err(OutsideConfig);
    }
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() < 2 || parts.len() > MAX_DEPTH || parts[0] != CONFIG_DIR {
        return Err(OutsideConfig);
    }
    for part in &parts {
        let stem = part
            .split('.')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        let bad = part.is_empty()
            || part.starts_with('.')
            || part.ends_with('.')
            || part.ends_with(' ')
            || part.starts_with(' ')
            || RESERVED.contains(&stem.as_str());
        if bad {
            return Err(OutsideConfig);
        }
    }
    let file = parts[parts.len() - 1];
    let ext = file
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    if !EXTENSIONS.contains(&ext.as_str()) {
        return Err(NotConfig);
    }
    for part in &parts[1..] {
        let lower = part.to_ascii_lowercase();
        let personal = lower.starts_with("arctic")
            || PRIVATE_DIRS.contains(&lower.as_str())
            || PRIVATE_WORDS.iter().any(|w| lower.contains(w));
        if personal {
            return Err(Personal);
        }
    }
    Ok(path.to_owned())
}

/// The path of an allowed config file inside `game_dir`. Refuses (as an error) anything
/// [`check_config_path`] refuses, and any folder on the way that is a link (a symlink or
/// junction could point outside the game folder).
pub fn resolve(game_dir: &Path, path: &str) -> Result<PathBuf> {
    let clean = check_config_path(path)
        .map_err(|why| Error::Other(format!("refused config path {path:?}: {}", why.message())))?;
    let mut out = game_dir.to_path_buf();
    for part in clean.split('/') {
        out.push(part);
        if let Ok(meta) = std::fs::symlink_metadata(&out)
            && meta.file_type().is_symlink()
        {
            return Err(Error::Other(format!(
                "refused config path {path:?}: it goes through a link"
            )));
        }
    }
    // Belt and braces: nothing but plain names were joined.
    if out
        .strip_prefix(game_dir)
        .map(|rel| rel.components().all(|c| matches!(c, Component::Normal(_))))
        != Ok(true)
    {
        return Err(Error::Other(format!("refused config path {path:?}")));
    }
    Ok(out)
}

/// `file` (inside `game_dir`) as a manifest path: `config/…` with `/`.
pub fn relative(game_dir: &Path, file: &Path) -> Option<String> {
    let rel = file.strip_prefix(game_dir).ok()?;
    let mut parts = Vec::new();
    for c in rel.components() {
        match c {
            Component::Normal(name) => parts.push(name.to_str()?.to_owned()),
            _ => return None,
        }
    }
    Some(parts.join("/"))
}
