//! Polarium (Arctic's own entity-rendering performance mod) is part of the
//! Performance switch: its jars come with the launcher (mod/dist/polarium,
//! copied from the Polarium repository's releases) rather than from
//! Modrinth, and are kept in sync in each instance's mods folder like the
//! Arctic Client. An instance that has Polarium already (the player's own
//! copy) is left with that one.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::Result;
use crate::error::IoContext;

/// File name inside an instance's `mods` folder.
pub const FILE_NAME: &str = "polarium-arctic.jar";
/// Its name in the launcher.
pub const NAME: &str = "Polarium";
/// The mod id its jars declare.
const MOD_ID: &str = "polarium";

/// (Minecraft version, jar) for every version Polarium has a build for.
const JARS: &[(&str, &[u8])] = &[
    (
        "26.2",
        include_bytes!("../../../../mod/dist/polarium/polarium-26.2.jar"),
    ),
    (
        "26.1.2",
        include_bytes!("../../../../mod/dist/polarium/polarium-26.1.2.jar"),
    ),
];

fn jar_for(game_version: &str) -> Option<&'static [u8]> {
    JARS.iter()
        .find(|(v, _)| *v == game_version)
        .map(|(_, jar)| *jar)
}

/// Whether another jar in `mods_dir` (the player's) is Polarium.
fn players_own(mods_dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(mods_dir) else {
        return false;
    };
    entries.flatten().any(|e| {
        let name = e.file_name().to_string_lossy().into_owned();
        name.ends_with(".jar")
            && name != FILE_NAME
            && crate::migrate::detect::mod_identity(&e.path()).is_some_and(|(id, _)| id == MOD_ID)
    })
}

/// The Minecraft versions a jar says it needs (`depends.minecraft` in its `fabric.mod.json`).
fn minecraft_requirement(jar: &Path) -> Option<String> {
    let mut archive = zip::ZipArchive::new(fs::File::open(jar).ok()?).ok()?;
    let mut text = String::new();
    archive
        .by_name("fabric.mod.json")
        .ok()?
        .read_to_string(&mut text)
        .ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    match json.get("depends")?.get("minecraft")? {
        serde_json::Value::String(one) => Some(one.clone()),
        serde_json::Value::Array(many) => Some(
            many.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(" "),
        ),
        _ => None,
    }
}

/// Whether a requirement like `26.2` or `>=1.21 <1.22` is about another Minecraft generation than
/// `game_version` (`1.8.9`, `26.2`): the first number of the version tells the generation.
fn other_generation(requirement: &str, game_version: &str) -> bool {
    let major = |v: &str| v.split('.').next().map(str::to_owned);
    let Some(game) = major(game_version) else {
        return false;
    };
    let wanted: Vec<String> = requirement
        .split(|c: char| !(c.is_ascii_digit() || c == '.'))
        .filter(|t| t.starts_with(|c: char| c.is_ascii_digit()))
        .filter_map(major)
        .collect();
    !wanted.is_empty() && !wanted.contains(&game)
}

/// Polarium jars in `mods_dir` that can't run on `game_version`: the player's own copy for another
/// Minecraft generation (an instance whose version was changed), which Fabric would refuse to start with.
fn unusable_copies(mods_dir: &Path, game_version: &str) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(mods_dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".jar"))
        .map(|e| e.path())
        .filter(|path| {
            crate::migrate::detect::mod_identity(path).is_some_and(|(id, _)| id == MOD_ID)
                && minecraft_requirement(path)
                    .is_some_and(|requirement| other_generation(&requirement, game_version))
        })
        .collect()
}

/// Put Polarium in `mods_dir` (when `enabled`, there's a build for the
/// version and the player hasn't their own) or take it out.
pub fn sync(mods_dir: &Path, game_version: &str, enabled: bool) -> Result<()> {
    // Switched to a game version the player's own copy isn't made for: it's put aside (not deleted)
    // so the game starts, and is picked up again by the mod manager when they switch back.
    for jar in unusable_copies(mods_dir, game_version) {
        let mut aside = jar.clone().into_os_string();
        aside.push(".disabled");
        let _ = fs::rename(&jar, PathBuf::from(aside));
    }
    let path = mods_dir.join(FILE_NAME);
    let jar = jar_for(game_version).filter(|_| enabled && !players_own(mods_dir));
    let Some(jar) = jar else {
        return match path.exists() {
            true => fs::remove_file(&path).at(&path),
            false => Ok(()),
        };
    };
    if fs::read(&path).is_ok_and(|current| current == jar) {
        return Ok(());
    }
    fs::create_dir_all(mods_dir).at(mods_dir)?;
    // Written next to it, then moved in: a game never sees half a jar.
    let part = mods_dir.join(format!("{FILE_NAME}.part"));
    fs::write(&part, jar).at(&part)?;
    fs::rename(&part, &path).at(&path)
}

/// Whether the switch's Polarium is in `mods_dir`.
pub fn installed(mods_dir: &Path) -> bool {
    mods_dir.join(FILE_NAME).exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_without_a_build_get_no_jar_and_lose_an_old_one() {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("mods");
        sync(&mods, "26.2", true).unwrap();
        assert!(installed(&mods));
        // The same folder played on 26.3 (no build): the 26.2 jar must go, or Fabric refuses to start.
        sync(&mods, "26.3", true).unwrap();
        assert!(!installed(&mods));
        sync(&mods, "1.21.11", true).unwrap();
        assert!(!installed(&mods));
    }

    #[test]
    fn added_for_a_covered_version_and_taken_out_when_off() {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("mods");
        sync(&mods, "26.2", true).unwrap();
        assert!(installed(&mods));
        assert_eq!(
            crate::migrate::detect::mod_identity(&mods.join(FILE_NAME)).map(|(id, _)| id),
            Some(MOD_ID.to_owned())
        );
        sync(&mods, "26.2", false).unwrap();
        assert!(!installed(&mods));
    }

    #[test]
    fn not_added_for_a_version_without_a_build() {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("mods");
        sync(&mods, "1.21.11", true).unwrap();
        assert!(!installed(&mods));
    }

    #[test]
    fn a_players_copy_for_another_generation_is_put_aside_not_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("mods");
        fs::create_dir_all(&mods).unwrap();
        fs::write(
            mods.join("polarium-26.2-mine.jar"),
            jar_for("26.2").unwrap(),
        )
        .unwrap();
        // The instance was switched to 1.8.9: Fabric would refuse to start with it.
        sync(&mods, "1.8.9", true).unwrap();
        assert!(!mods.join("polarium-26.2-mine.jar").exists());
        assert!(mods.join("polarium-26.2-mine.jar.disabled").exists());
        // Still on a 26.x version: left alone.
        fs::write(mods.join("polarium-mine.jar"), jar_for("26.2").unwrap()).unwrap();
        sync(&mods, "26.3", true).unwrap();
        assert!(mods.join("polarium-mine.jar").exists());
    }

    #[test]
    fn generations_are_told_apart() {
        assert!(other_generation("26.2", "1.8.9"));
        assert!(other_generation(">=26.1 <27", "1.21.11"));
        assert!(!other_generation("26.2", "26.3"));
        assert!(!other_generation(">=1.21", "1.21.11"));
        assert!(!other_generation("*", "1.8.9"));
    }

    #[test]
    fn the_players_own_copy_is_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("mods");
        fs::create_dir_all(&mods).unwrap();
        fs::write(mods.join("polarium-mine.jar"), jar_for("26.2").unwrap()).unwrap();
        sync(&mods, "26.2", true).unwrap();
        assert!(!installed(&mods));
        assert!(mods.join("polarium-mine.jar").exists());
    }
}
