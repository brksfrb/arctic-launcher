//! Polarium (Arctic's own entity-rendering performance mod) is part of the
//! Performance switch: its jars come with the launcher (mod/dist/polarium,
//! copied from the Polarium repository's releases) rather than from
//! Modrinth, and are kept in sync in each instance's mods folder like the
//! Arctic Client. An instance that has Polarium already (the player's own
//! copy) is left with that one.

use std::fs;
use std::path::Path;

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

/// Put Polarium in `mods_dir` (when `enabled`, there's a build for the
/// version and the player hasn't their own) or take it out.
pub fn sync(mods_dir: &Path, game_version: &str, enabled: bool) -> Result<()> {
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
