//! Version lists for Forge (files.minecraftforge.net) and NeoForge
//! (maven.neoforged.net), and where their installers live.

use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap};

use serde::Deserialize;

use super::LoaderVersion;
use super::util::{compare_versions, sort_newest_first};
use crate::net::get_json;
use crate::{Error, Result};

const FORGE_METADATA: &str =
    "https://files.minecraftforge.net/net/minecraftforge/forge/maven-metadata.json";
const FORGE_PROMOTIONS: &str =
    "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";
const FORGE_MAVEN: &str = "https://maven.minecraftforge.net/net/minecraftforge/forge";
const NEOFORGE_VERSIONS: &str =
    "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge";
const NEOFORGE_MAVEN: &str = "https://maven.neoforged.net/releases/net/neoforged/neoforge";

/// Oldest Minecraft version whose Forge installer we support.
pub const OLDEST_FORGE_GAME: &str = "1.7.10";

/// Where to fetch one installer jar from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallerSource {
    pub url: String,
    /// Also the cache file name.
    pub file_name: String,
}

// ---------------------------------------------------------------- Forge

/// `maven-metadata.json`: Minecraft version → full Forge versions.
type ForgeMetadata = HashMap<String, Vec<String>>;

#[derive(Debug, Deserialize)]
struct Promotions {
    promos: HashMap<String, String>,
}

pub fn forge_game_versions() -> Result<Vec<String>> {
    let meta: ForgeMetadata = get_json(FORGE_METADATA).map_err(|e| list_error("Forge", &e))?;
    Ok(supported_forge_games(&meta))
}

pub fn forge_loader_versions(game: &str) -> Result<Vec<LoaderVersion>> {
    // Both documents are small; fetch them concurrently.
    let (meta, promos) = std::thread::scope(|s| {
        let promos = s.spawn(|| get_json::<Promotions>(FORGE_PROMOTIONS));
        let meta = get_json::<ForgeMetadata>(FORGE_METADATA);
        let promos = promos
            .join()
            .unwrap_or_else(|_| Err(Error::Other("promotions fetch crashed".into())));
        (meta, promos)
    });
    let meta = meta.map_err(|e| list_error("Forge", &e))?;
    // Promotions only mark "recommended"; the list is usable without them.
    let promos = promos.map(|p| p.promos).unwrap_or_else(|e| {
        log::warn!("Forge promotions unavailable: {e}");
        HashMap::new()
    });
    Ok(forge_versions_for(&meta, &promos, game))
}

pub fn is_supported_forge_game(game: &str) -> bool {
    !game.contains("_pre") && compare_versions(game, OLDEST_FORGE_GAME) != Ordering::Less
}

fn supported_forge_games(meta: &ForgeMetadata) -> Vec<String> {
    let mut games: Vec<String> = meta
        .iter()
        .filter(|(game, versions)| is_supported_forge_game(game) && !versions.is_empty())
        .map(|(game, _)| game.clone())
        .collect();
    sort_newest_first(&mut games);
    games
}

/// Loader ids are the part after `<game>-` (e.g. `47.4.10`, or
/// `10.13.4.1614-1.7.10` for builds that carry a branch suffix).
fn forge_versions_for(
    meta: &ForgeMetadata,
    promos: &HashMap<String, String>,
    game: &str,
) -> Vec<LoaderVersion> {
    let prefix = format!("{game}-");
    let recommended = promos.get(&format!("{game}-recommended"));
    let mut ids: Vec<String> = meta
        .get(game)
        .into_iter()
        .flatten()
        .filter_map(|full| full.strip_prefix(&prefix).map(str::to_owned))
        .collect();
    sort_newest_first(&mut ids);
    ids.into_iter()
        .map(|id| LoaderVersion {
            stable: recommended.is_some_and(|r| {
                id == *r
                    || id
                        .strip_prefix(r.as_str())
                        .is_some_and(|s| s.starts_with('-'))
            }),
            id,
        })
        .collect()
}

/// Installer candidates for Forge `loader_version` on `game`. Some old
/// builds carry a `-<game>` branch suffix that promotions omit.
pub fn forge_installers(game: &str, loader_version: &str) -> Vec<InstallerSource> {
    let prefix = format!("{game}-");
    let bare = loader_version
        .strip_prefix(&prefix)
        .unwrap_or(loader_version);
    let mut fulls = vec![format!("{game}-{bare}")];
    if !bare.ends_with(&format!("-{game}")) {
        fulls.push(format!("{game}-{bare}-{game}"));
    }
    fulls
        .into_iter()
        .map(|full| {
            let file_name = format!("forge-{full}-installer.jar");
            InstallerSource {
                url: format!("{FORGE_MAVEN}/{full}/{file_name}"),
                file_name,
            }
        })
        .collect()
}

// ------------------------------------------------------------- NeoForge

#[derive(Debug, Deserialize)]
struct MavenVersions {
    versions: Vec<String>,
}

pub fn neoforge_game_versions() -> Result<Vec<String>> {
    let list = fetch_neoforge()?;
    let mut games: Vec<String> = list
        .iter()
        .filter_map(|v| neoforge_minecraft_version(v))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    sort_newest_first(&mut games);
    Ok(games)
}

pub fn neoforge_loader_versions(game: &str) -> Result<Vec<LoaderVersion>> {
    Ok(neoforge_versions_for(&fetch_neoforge()?, game))
}

fn fetch_neoforge() -> Result<Vec<String>> {
    get_json::<MavenVersions>(NEOFORGE_VERSIONS)
        .map(|m| m.versions)
        .map_err(|e| list_error("NeoForge", &e))
}

fn neoforge_versions_for(list: &[String], game: &str) -> Vec<LoaderVersion> {
    let mut ids: Vec<String> = list
        .iter()
        .filter(|v| neoforge_minecraft_version(v).as_deref() == Some(game))
        .cloned()
        .collect();
    sort_newest_first(&mut ids);
    ids.into_iter()
        .map(|id| LoaderVersion {
            stable: !id.contains("-beta") && !id.contains("-alpha"),
            id,
        })
        .collect()
}

/// The Minecraft version a NeoForge version targets:
/// - `21.1.251` → `1.21.1`, `21.0.3-beta` → `1.21` (`<minor>.<patch>.<build>`)
/// - `26.1.0.5` → `26.1`, `26.1.2.0-beta` → `26.1.2`
///   (`<year>.<drop>.<hotfix>.<build>`), and
///   `26.1.0.0-alpha.3+snapshot-1` → `26.1-snapshot-1`
/// - `0.25w14craftmine.3-beta` → `25w14craftmine` (April Fools builds)
pub fn neoforge_minecraft_version(version: &str) -> Option<String> {
    let (core, pre) = match version.split_once('-') {
        Some((core, pre)) => (core, Some(pre)),
        None => (version, None),
    };
    let parts: Vec<&str> = core.split('.').collect();
    let game = match parts.as_slice() {
        ["0", special, _build] => return Some((*special).to_owned()),
        [year, drop, hotfix, _build] if year.parse::<u32>().ok()? >= 26 => {
            drop.parse::<u32>().ok()?;
            match *hotfix {
                "0" => format!("{year}.{drop}"),
                h => format!("{year}.{drop}.{}", h.parse::<u32>().ok()?),
            }
        }
        [minor, patch, _build] => {
            let minor: u32 = minor.parse().ok()?;
            match patch.parse::<u32>().ok()? {
                0 => format!("1.{minor}"),
                p => format!("1.{minor}.{p}"),
            }
        }
        _ => return None,
    };
    // Snapshot builds name their target after `+`.
    match pre.and_then(|p| p.split_once('+')) {
        Some((_, target)) if !target.is_empty() => Some(format!("{game}-{target}")),
        _ => Some(game),
    }
}

pub fn neoforge_installer(loader_version: &str) -> InstallerSource {
    let file_name = format!("neoforge-{loader_version}-installer.jar");
    InstallerSource {
        url: format!("{NEOFORGE_MAVEN}/{loader_version}/{file_name}"),
        file_name,
    }
}

fn list_error(label: &str, e: &Error) -> Error {
    Error::Other(format!("could not load the {label} version list: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FORGE_META: &str = r#"{
        "1.7.2": ["1.7.2-10.12.2.1161-mc172"],
        "1.7.10_pre4": ["1.7.10_pre4-10.12.2.1149-prerelease"],
        "1.7.10": ["1.7.10-10.13.0.1150", "1.7.10-10.13.4.1614-1.7.10"],
        "1.12.2": ["1.12.2-14.23.5.2859", "1.12.2-14.23.5.2860", "1.12.2-14.23.5.2864"],
        "1.20.1": ["1.20.1-47.0.0", "1.20.1-47.4.10", "1.20.1-47.4.23"],
        "26.1": ["26.1-62.0.9"],
        "1.21.10": []
    }"#;
    const PROMOS: &str = r#"{"homepage":"x","promos":{
        "1.7.10-recommended":"10.13.4.1614","1.12.2-recommended":"14.23.5.2859",
        "1.20.1-recommended":"47.4.10","1.20.1-latest":"47.4.23"}}"#;

    fn forge() -> (ForgeMetadata, HashMap<String, String>) {
        let promos: Promotions = serde_json::from_str(PROMOS).unwrap();
        (serde_json::from_str(FORGE_META).unwrap(), promos.promos)
    }

    #[test]
    fn forge_games_skip_old_and_prerelease_versions() {
        let (meta, _) = forge();
        assert_eq!(
            supported_forge_games(&meta),
            ["26.1", "1.20.1", "1.12.2", "1.7.10"]
        );
    }

    #[test]
    fn forge_versions_are_short_ids_with_recommended_marked() {
        let (meta, promos) = forge();
        let list = forge_versions_for(&meta, &promos, "1.20.1");
        let ids: Vec<(&str, bool)> = list.iter().map(|v| (v.id.as_str(), v.stable)).collect();
        assert_eq!(
            ids,
            [("47.4.23", false), ("47.4.10", true), ("47.0.0", false)]
        );
        let old = forge_versions_for(&meta, &promos, "1.7.10");
        assert_eq!(old[0].id, "10.13.4.1614-1.7.10");
        assert!(old[0].stable && !old[1].stable);
        assert!(forge_versions_for(&meta, &promos, "9.9").is_empty());
    }

    #[test]
    fn forge_installer_urls_cover_branch_suffix() {
        let c = forge_installers("1.7.10", "10.13.4.1614");
        assert_eq!(c.len(), 2);
        assert_eq!(
            c[1].url,
            "https://maven.minecraftforge.net/net/minecraftforge/forge/1.7.10-10.13.4.1614-1.7.10/forge-1.7.10-10.13.4.1614-1.7.10-installer.jar"
        );
        let c = forge_installers("1.20.1", "47.4.10");
        assert_eq!(c[0].file_name, "forge-1.20.1-47.4.10-installer.jar");
        assert_eq!(forge_installers("1.7.10", "10.13.4.1614-1.7.10").len(), 1);
        assert_eq!(
            forge_installers("1.20.1", "1.20.1-47.4.10")[0].file_name,
            c[0].file_name
        );
    }

    #[test]
    fn maps_neoforge_versions_to_minecraft() {
        let cases = [
            ("20.2.3-beta", Some("1.20.2")),
            ("20.4.237", Some("1.20.4")),
            ("21.0.0-beta", Some("1.21")),
            ("21.1.251", Some("1.21.1")),
            ("21.11.3", Some("1.21.11")),
            ("26.1.0.5", Some("26.1")),
            ("26.1.2.0-beta", Some("26.1.2")),
            ("26.1.0.0-alpha.15+pre-3", Some("26.1-pre-3")),
            ("26.1.0.0-alpha.1+snapshot-1", Some("26.1-snapshot-1")),
            ("0.25w14craftmine.3-beta", Some("25w14craftmine")),
            ("garbage", None),
            ("1.2", None),
        ];
        for (v, want) in cases {
            assert_eq!(neoforge_minecraft_version(v).as_deref(), want, "{v}");
        }
    }

    #[test]
    fn neoforge_versions_filter_sort_and_stability() {
        let list: MavenVersions = serde_json::from_str(
            r#"{"isSnapshot":false,"versions":["21.1.9","21.1.10","21.0.1-beta","21.1.0-beta","26.1.0.1"]}"#,
        )
        .unwrap();
        let out = neoforge_versions_for(&list.versions, "1.21.1");
        let ids: Vec<(&str, bool)> = out.iter().map(|v| (v.id.as_str(), v.stable)).collect();
        assert_eq!(
            ids,
            [("21.1.10", true), ("21.1.9", true), ("21.1.0-beta", false)]
        );
        assert_eq!(
            neoforge_installer("21.1.251").url,
            "https://maven.neoforged.net/releases/net/neoforged/neoforge/21.1.251/neoforge-21.1.251-installer.jar"
        );
    }
}
