//! Patcher (with the OneConfig it loads) used to be part of the Forge 1.8.9 Performance set. It no
//! longer is: at every Forge start, what the launcher itself put in place for it is taken out again.
//!
//! Only files the launcher can prove are its own go: the Patcher jar named in the launcher's Forge
//! index (and its copy parked aside when OneConfig couldn't be fetched), and the OneConfig loader and
//! runtime jars whose bytes match the hashes the launcher recorded when it downloaded them. A Patcher,
//! OneConfig or other Polyfrost mod the player added is never touched, and while one is there the
//! OneConfig jars it needs stay too. OneConfig's settings folder is the player's and stays.

use std::io::Read;
use std::path::Path;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::super::index::ModIndex;
use super::forge_index_path;
use crate::Result;
use crate::error::IoContext;

/// Modrinth project of Patcher (as recorded in the Forge index by older launchers).
const PATCHER: &str = "YknNc5nN";

/// Where older launchers parked their Patcher when OneConfig couldn't be fetched.
const PARKED_DIR: &str = "arctic-patcher-offline";
/// Marker older launchers wrote after moving Patcher's clashing defaults aside.
const SETTINGS_MARKER: &str = "arctic-patcher-settings.json";
/// OneConfig's folder in the game directory, the launcher's record of what it downloaded there, and
/// the two jars it downloaded.
const ONECONFIG_DIR: &str = "OneConfig";
const ONECONFIG_RECORD: &str = "arctic-cache.json";
const ONECONFIG_JARS: &[&str] = &[
    "launchwrapper/OneConfig-Loader.jar",
    "OneConfig (1.8.9-forge).jar",
];

#[derive(Deserialize)]
struct Artifact {
    sha256: String,
}

#[derive(Deserialize)]
struct Record {
    loader: Artifact,
    release: Artifact,
    snapshot: Artifact,
}

/// Take out the Patcher and OneConfig files the launcher installed. Never fails a start: problems are
/// logged and tried again next time.
pub(super) fn remove_installed_patcher(game_dir: &Path) {
    if let Err(e) = remove_patcher_jar(game_dir) {
        // Our Patcher may still be there and need OneConfig: leave its jars until it is gone.
        log::warn!("couldn't remove the Patcher Arctic installed: {e}");
        return;
    }
    if let Err(e) = remove_oneconfig_jars(game_dir) {
        log::warn!("couldn't remove the OneConfig jars Arctic installed: {e}");
    }
}

fn remove_patcher_jar(game_dir: &Path) -> Result<()> {
    let index_path = forge_index_path(game_dir);
    let index = ModIndex::load(&index_path)?;
    if let Some(m) = index.by_project(PATCHER) {
        let file = m.file_name.clone();
        super::super::files::delete(&game_dir.join("mods"), &file)?;
        let parked_dir = game_dir.join(PARKED_DIR);
        let parked = parked_dir.join(&file);
        if parked.is_file() {
            std::fs::remove_file(&parked).at(&parked)?;
        }
        // Gone only once empty: nothing else of anyone's is removed with it.
        let _ = std::fs::remove_dir(&parked_dir);
        index.without_file(&file).save(&index_path)?;
        log::info!("Patcher is no longer part of Arctic's performance mods; removed {file}");
    }
    let marker = game_dir.join(SETTINGS_MARKER);
    if marker.is_file() {
        std::fs::remove_file(&marker).at(&marker)?;
    }
    Ok(())
}

fn remove_oneconfig_jars(game_dir: &Path) -> Result<()> {
    let dir = game_dir.join(ONECONFIG_DIR);
    let record_path = dir.join(ONECONFIG_RECORD);
    let Some(record) = crate::storage::load_json::<Record>(&record_path).unwrap_or(None) else {
        return Ok(()); // The launcher never downloaded OneConfig here.
    };
    if let Some(theirs) = players_oneconfig_mod(&game_dir.join("mods"), game_dir)? {
        log::info!("OneConfig jars kept: {theirs} in mods needs them");
        return Ok(());
    }
    let ours = [
        &record.loader.sha256,
        &record.release.sha256,
        &record.snapshot.sha256,
    ];
    for name in ONECONFIG_JARS {
        let path = dir.join(name);
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let hash = hex::encode(Sha256::digest(&bytes));
        if ours.iter().any(|h| h.eq_ignore_ascii_case(&hash)) {
            std::fs::remove_file(&path).at(&path)?;
            log::info!("removed the OneConfig jar Arctic installed: {name}");
        }
        let part = path.with_extension("arctic-part");
        if part.is_file() {
            std::fs::remove_file(&part).at(&part)?;
        }
    }
    let _ = std::fs::remove_dir(dir.join("launchwrapper"));
    std::fs::remove_file(&record_path).at(&record_path)
}

/// A jar in `mods` or a folder right inside it (Forge also loads `mods/1.8.9/`), on or turned off
/// and not the launcher's, that is Patcher, OneConfig or another mod loading OneConfig: by its name,
/// or by the OneConfig tweaker its manifest names.
fn players_oneconfig_mod(mods_dir: &Path, game_dir: &Path) -> Result<Option<String>> {
    let ours: Vec<String> = ModIndex::load(&forge_index_path(game_dir))?
        .mods
        .into_iter()
        .map(|m| m.file_name)
        .collect();
    let Ok(entries) = std::fs::read_dir(mods_dir) else {
        return Ok(None);
    };
    let mut files = Vec::new();
    for entry in entries.flatten() {
        if entry.file_type().is_ok_and(|t| t.is_dir()) {
            let inner = std::fs::read_dir(entry.path())
                .into_iter()
                .flatten()
                .flatten();
            files.extend(inner.map(|e| (e.path(), e.file_name(), false)));
        } else {
            files.push((entry.path(), entry.file_name(), true));
        }
    }
    for (path, file_name, top) in files {
        let name = file_name.to_string_lossy().into_owned();
        let lower = name.to_ascii_lowercase();
        let base = lower.strip_suffix(".disabled").unwrap_or(&lower);
        let launchers = top && ours.iter().any(|o| o.eq_ignore_ascii_case(base));
        if !base.ends_with(".jar") || launchers {
            continue;
        }
        if ["patcher", "oneconfig", "polyfrost"]
            .iter()
            .any(|w| base.contains(w))
            || loads_oneconfig(&path)
        {
            return Ok(Some(name));
        }
    }
    Ok(None)
}

fn loads_oneconfig(jar: &Path) -> bool {
    let Ok(file) = std::fs::File::open(jar) else {
        return false;
    };
    let Ok(mut zip) = zip::ZipArchive::new(file) else {
        return false;
    };
    let Ok(manifest) = zip.by_name("META-INF/MANIFEST.MF") else {
        return false;
    };
    let mut text = String::new();
    if manifest.take(64 * 1024).read_to_string(&mut text).is_err() {
        return false;
    }
    text.lines().any(|line| {
        let line = line.to_ascii_lowercase();
        line.starts_with("tweakclass:") && line.contains("oneconfig")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mods::InstalledMod;

    fn entry(project: &str, file: &str) -> InstalledMod {
        InstalledMod {
            project_id: project.into(),
            version_id: "v".into(),
            title: "t".into(),
            version_number: "1".into(),
            file_name: file.into(),
            icon_url: None,
            dependency: false,
        }
    }

    fn sha(bytes: &[u8]) -> String {
        hex::encode(Sha256::digest(bytes))
    }

    /// A game dir as an older launcher left it: its Patcher (one parked copy), EntityCulling, the
    /// OneConfig jars with their record, the settings marker, and the player's OneConfig settings.
    fn old_launcher_dir(game: &Path) -> ModIndex {
        let mods = game.join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        std::fs::write(mods.join("Patcher-1.10.4.jar"), b"patcher").unwrap();
        std::fs::write(mods.join("EntityCulling-forge.jar"), b"ec").unwrap();
        std::fs::create_dir_all(game.join(PARKED_DIR)).unwrap();
        std::fs::write(game.join(PARKED_DIR).join("Patcher-1.10.4.jar"), b"patcher").unwrap();
        std::fs::write(game.join(SETTINGS_MARKER), b"{}").unwrap();
        let one = game.join(ONECONFIG_DIR);
        std::fs::create_dir_all(one.join("launchwrapper")).unwrap();
        std::fs::write(one.join(ONECONFIG_JARS[0]), b"loader").unwrap();
        std::fs::write(one.join(ONECONFIG_JARS[1]), b"runtime").unwrap();
        std::fs::write(one.join("OneConfig.json"), b"{}").unwrap();
        std::fs::write(
            one.join(ONECONFIG_RECORD),
            serde_json::json!({
                "loader": {"url": "https://x/l", "sha256": sha(b"loader")},
                "release": {"url": "https://x/r", "sha256": sha(b"runtime")},
                "snapshot": {"url": "https://x/s", "sha256": sha(b"snap")},
            })
            .to_string(),
        )
        .unwrap();
        let index = ModIndex::default()
            .with(entry(PATCHER, "Patcher-1.10.4.jar"))
            .with(entry(
                super::super::ENTITY_CULLING,
                "EntityCulling-forge.jar",
            ));
        index.save(&forge_index_path(game)).unwrap();
        index
    }

    #[test]
    fn installed_patcher_and_oneconfig_are_removed_player_files_stay() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path();
        old_launcher_dir(game);
        std::fs::write(game.join("mods").join("my-own-mod.jar"), b"mine").unwrap();
        remove_installed_patcher(game);

        let mods = game.join("mods");
        assert!(!mods.join("Patcher-1.10.4.jar").exists());
        assert!(!game.join(PARKED_DIR).exists());
        assert!(!game.join(SETTINGS_MARKER).exists());
        assert!(mods.join("EntityCulling-forge.jar").is_file());
        assert!(mods.join("my-own-mod.jar").is_file());
        let index = ModIndex::load(&forge_index_path(game)).unwrap();
        assert!(index.by_project(PATCHER).is_none());
        assert!(index.by_project(super::super::ENTITY_CULLING).is_some());
        let one = game.join(ONECONFIG_DIR);
        for jar in ONECONFIG_JARS {
            assert!(!one.join(jar).exists(), "{jar}");
        }
        assert!(!one.join(ONECONFIG_RECORD).exists());
        assert!(
            one.join("OneConfig.json").is_file(),
            "the player's settings stay"
        );
        // Nothing left to do: a second start changes nothing.
        remove_installed_patcher(game);
        assert!(mods.join("EntityCulling-forge.jar").is_file());
    }

    #[test]
    fn the_players_own_patcher_and_its_oneconfig_are_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path();
        old_launcher_dir(game);
        let mods = game.join("mods");
        std::fs::write(mods.join("PolyPatcher-1.8.9-forge-1.10.5.jar"), b"theirs").unwrap();
        remove_installed_patcher(game);

        assert!(!mods.join("Patcher-1.10.4.jar").exists(), "ours goes");
        assert!(mods.join("PolyPatcher-1.8.9-forge-1.10.5.jar").is_file());
        let one = game.join(ONECONFIG_DIR);
        for jar in ONECONFIG_JARS {
            assert!(
                one.join(jar).is_file(),
                "{jar} is needed by the player's Patcher"
            );
        }
    }

    #[test]
    fn a_players_patcher_in_the_version_folder_keeps_the_jars() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path();
        old_launcher_dir(game);
        let nested = game.join("mods").join("1.8.9");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("Patcher-1.8.9.jar"), b"theirs").unwrap();
        remove_installed_patcher(game);
        assert!(!game.join("mods").join("Patcher-1.10.4.jar").exists());
        assert!(nested.join("Patcher-1.8.9.jar").is_file());
        assert!(game.join(ONECONFIG_DIR).join(ONECONFIG_JARS[0]).is_file());
    }

    #[test]
    fn a_mod_loading_oneconfig_by_its_manifest_keeps_the_jars() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path();
        old_launcher_dir(game);
        let mut jar = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        jar.start_file::<_, ()>("META-INF/MANIFEST.MF", Default::default())
            .unwrap();
        std::io::Write::write_all(
            &mut jar,
            b"Manifest-Version: 1.0\r\nTweakClass: org.polyfrost.oneconfig.loader.stage0.LaunchWrapperTweaker\r\n",
        )
        .unwrap();
        let bytes = jar.finish().unwrap().into_inner();
        std::fs::write(game.join("mods").join("SomeHud.jar.disabled"), bytes).unwrap();
        remove_installed_patcher(game);
        assert!(game.join(ONECONFIG_DIR).join(ONECONFIG_JARS[1]).is_file());
    }

    #[test]
    fn oneconfig_jars_the_launcher_did_not_download_stay() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path();
        old_launcher_dir(game);
        let one = game.join(ONECONFIG_DIR);
        // OneConfig updated itself (or the player put another build there): not the bytes we fetched.
        std::fs::write(one.join(ONECONFIG_JARS[1]), b"newer runtime").unwrap();
        remove_installed_patcher(game);
        assert!(!one.join(ONECONFIG_JARS[0]).exists());
        assert!(one.join(ONECONFIG_JARS[1]).is_file());

        // No record at all: the launcher never downloaded OneConfig here, so nothing is touched.
        let other = tempfile::tempdir().unwrap();
        let jar = other.path().join(ONECONFIG_DIR).join(ONECONFIG_JARS[1]);
        std::fs::create_dir_all(jar.parent().unwrap()).unwrap();
        std::fs::write(&jar, b"runtime").unwrap();
        remove_installed_patcher(other.path());
        assert!(jar.is_file());
    }

    #[test]
    fn a_player_patcher_never_in_our_index_is_not_removed() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path();
        let mods = game.join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        std::fs::write(mods.join("Patcher-1.10.4.jar"), b"theirs").unwrap();
        ModIndex::default()
            .with(entry(
                super::super::ENTITY_CULLING,
                "EntityCulling-forge.jar",
            ))
            .save(&forge_index_path(game))
            .unwrap();
        remove_installed_patcher(game);
        assert!(mods.join("Patcher-1.10.4.jar").is_file());
    }
}
