//! SKlauncher. 3.x keeps its profiles in the official `.minecraft` (the same
//! `launcher_profiles.json`, read in `minecraft.rs`); 4.x is a separate app
//! with its own `instances.json`. Accounts are never read.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::{found, loader, memory, read_json};
use crate::migrate::{Found, FoundLoader, Launcher, Scan};

/// SKlauncher 3 lives in `.minecraft`; these files tell it apart from the
/// official launcher (the same check SKlauncher 4 uses on it).
pub fn is_v3(root: &Path) -> bool {
    let own = root.join("sklauncher");
    own.join("sklauncher.json").is_file()
        || own.join("accounts.bin").is_file()
        || root.join("sklauncher-fx.jar").is_file()
}

/// What a profile's version id names when its folder isn't downloaded yet
/// (SKlauncher only fetches it on the first Play): game version and loader.
pub fn from_version_id(id: &str) -> Option<(String, Option<FoundLoader>)> {
    if let Some(rest) = id.strip_prefix("fabric-loader-") {
        let (version, game) = split_loader_game(rest)?;
        return Some((game.to_owned(), loader("fabric", Some(version), game)));
    }
    if let Some(rest) = id.strip_prefix("quilt-loader-") {
        let (version, game) = split_loader_game(rest)?;
        return Some((game.to_owned(), loader("quilt", Some(version), game)));
    }
    if let Some(rest) = id.strip_prefix("neoforge-") {
        let game = neoforge_game(rest)?;
        return Some((game.clone(), loader("neoforge", Some(rest), &game)));
    }
    if let Some((game, forge)) = id.split_once("-forge-") {
        return Some((game.to_owned(), loader("forge", Some(forge), game)));
    }
    // A plain version id.
    looks_like_game(id).then(|| (id.to_owned(), None))
}

/// "0.20.1-26.3" → ("0.20.1", "26.3"); the loader's own version may hold a
/// dash ("0.30.0-beta.1-1.21.1"), so the game version is the first tail
/// that looks like one.
fn split_loader_game(rest: &str) -> Option<(&str, &str)> {
    rest.match_indices('-')
        .map(|(i, _)| (&rest[..i], &rest[i + 1..]))
        .find(|(_, game)| looks_like_game(game))
}

fn looks_like_game(text: &str) -> bool {
    let mut digits = text.chars().take_while(char::is_ascii_digit).count();
    if digits == 0 {
        return false;
    }
    let rest = &text[digits..];
    // "1.21.1", "26.3", or a snapshot ("24w14a").
    if rest.starts_with('.') {
        digits = rest[1..].chars().take_while(char::is_ascii_digit).count();
        return digits > 0;
    }
    rest.starts_with('w')
}

/// NeoForge's id leaves the game out: 21.1.x is 1.21.1, 26.3.0.99 is 26.3.
fn neoforge_game(version: &str) -> Option<String> {
    let mut parts = version.split('.');
    let major: u32 = parts.next()?.parse().ok()?;
    let minor: u32 = parts.next()?.parse().ok()?;
    Some(if major >= 26 {
        format!("{major}.{minor}")
    } else if minor == 0 {
        format!("1.{major}")
    } else {
        format!("1.{major}.{minor}")
    })
}

/// SKlauncher 3's own memory setting for a profile, in MB.
pub fn memory_max(profile: &Value) -> Option<u32> {
    memory(profile.get("memoryMax").and_then(Value::as_u64))
}

/// SKlauncher 4: `~/.sklauncher` (Linux), `%APPDATA%\.sklauncher`, or
/// `Application Support/sklauncher` (macOS).
fn v4_base() -> Option<PathBuf> {
    if cfg!(windows) {
        dirs::config_dir().map(|d| d.join(".sklauncher"))
    } else if cfg!(target_os = "macos") {
        dirs::data_dir().map(|d| d.join("sklauncher"))
    } else {
        dirs::home_dir().map(|h| h.join(".sklauncher"))
    }
}

pub fn v4(scan: &mut Scan) {
    if let Some(base) = v4_base().filter(|b| b.join("instances.json").is_file()) {
        v4_in(scan, &base);
    }
}

pub(super) fn v4_in(scan: &mut Scan, base: &Path) {
    let Some(json) = read_json(&base.join("instances.json")) else {
        scan.seen
            .push((Launcher::SkLauncher, "Its instance list can't be read".into()));
        return;
    };
    // `location.json` can move the instance folders somewhere else.
    let data = read_json(&base.join("location.json"))
        .and_then(|j| j.get("dataDir").and_then(Value::as_str).map(expand))
        .unwrap_or_else(|| base.to_path_buf());
    let before = scan.instances.len();
    let list = json
        .get("instances")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for instance in &list {
        if let Some(entry) = v4_instance(scan, instance, &data) {
            scan.instances.push(entry);
        }
    }
    if scan.instances.len() == before {
        scan.seen
            .push((Launcher::SkLauncher, "No instances of its own".into()));
    }
}

fn v4_instance(scan: &mut Scan, instance: &Value, data: &Path) -> Option<Found> {
    let text = |k: &str| instance.get(k).and_then(Value::as_str).unwrap_or("");
    let id = text("id");
    let kind = text("type");
    let name = Some(text("name"))
        .filter(|n| !n.is_empty())
        .unwrap_or(id)
        .to_owned();
    // The rolling "Latest snapshot" brings nothing a release doesn't.
    if kind == "latest-snapshot" {
        return None;
    }
    let dir = Some(text("directory"))
        .filter(|d| !d.is_empty())
        .map(expand)
        .unwrap_or_else(|| data.join("instances").join(id));
    if !dir.is_dir() {
        scan.seen.push((
            Launcher::SkLauncher,
            format!("\"{name}\": its folder isn't there"),
        ));
        return None;
    }
    if kind == "latest-release" {
        let mut entry = found(Launcher::SkLauncher, &name, "latest", None, dir);
        entry.into_vanilla = true;
        return Some(entry);
    }
    let game = text("minecraftVersion");
    if game.is_empty() {
        scan.seen.push((
            Launcher::SkLauncher,
            format!("\"{name}\": it names no Minecraft version"),
        ));
        return None;
    }
    let version = Some(text("loaderVersion")).filter(|v| !v.is_empty());
    let kind = text("gameType");
    let found_loader = loader(kind, version, game);
    let mut entry = found(Launcher::SkLauncher, &name, game, found_loader.clone(), dir);
    if found_loader.is_none() && !matches!(kind, "" | "vanilla") {
        entry.notes.push(format!(
            "It runs on {kind}, which Arctic doesn't have; it comes in as plain {game}"
        ));
    }
    entry.memory_mb = memory(instance.get("memoryMax").and_then(Value::as_u64));
    Some(entry)
}

/// A stored path, with a leading `~` meaning the home folder.
fn expand(path: &str) -> PathBuf {
    match path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\")) {
        Some(rest) => dirs::home_dir().map(|h| h.join(rest)).unwrap_or_else(|| path.into()),
        None => path.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loaders::LoaderKind;

    fn known(id: &str) -> (String, Option<(LoaderKind, Option<String>)>) {
        let (game, l) = from_version_id(id).expect(id);
        (game, l.map(|l| (l.kind, l.version)))
    }

    #[test]
    fn reads_loader_ids_before_the_version_is_downloaded() {
        assert_eq!(
            known("fabric-loader-0.20.1-26.3"),
            ("26.3".into(), Some((LoaderKind::Fabric, Some("0.20.1".into()))))
        );
        assert_eq!(
            known("quilt-loader-0.30.0-beta.1-1.21.1"),
            ("1.21.1".into(), Some((LoaderKind::Quilt, Some("0.30.0-beta.1".into()))))
        );
        assert_eq!(
            known("1.20.1-forge-47.4.28"),
            ("1.20.1".into(), Some((LoaderKind::Forge, Some("47.4.28".into()))))
        );
        assert_eq!(
            known("neoforge-21.1.172"),
            ("1.21.1".into(), Some((LoaderKind::NeoForge, Some("21.1.172".into()))))
        );
        assert_eq!(
            known("neoforge-26.3.0.99"),
            ("26.3".into(), Some((LoaderKind::NeoForge, Some("26.3.0.99".into()))))
        );
        assert_eq!(known("1.21.1"), ("1.21.1".into(), None));
        assert_eq!(known("24w14a"), ("24w14a".into(), None));
        assert!(from_version_id("Some custom thing").is_none());
    }
}
