//! Arctic's companion mod (capes from Arctic and the in-game Arctic menu),
//! bundled with the launcher and kept in sync in Fabric and Quilt instances.

use std::fs;
use std::path::Path;

use crate::Result;
use crate::error::IoContext;

/// File name inside an instance's `mods` folder.
pub const FILE_NAME: &str = "arctic-mod.jar";
/// Overrides the cosmetics server the mod talks to (for testing).
pub const COSMETICS_URL_ENV: &str = "ARCTIC_COSMETICS_URL";

/// (Minecraft version, jar) for every version the mod is built for.
const JARS: &[(&str, &[u8])] = &[(
    "26.3",
    include_bytes!("../../../mod/fabric/dist/arctic-mod-26.3.jar"),
)];

fn jar_for(game_version: &str) -> Option<&'static [u8]> {
    JARS.iter()
        .find(|(v, _)| *v == game_version)
        .map(|(_, jar)| *jar)
}

/// True if the mod exists for this Minecraft version.
pub fn supports(game_version: &str) -> bool {
    jar_for(game_version).is_some()
}

/// Put the mod in `mods_dir` (when enabled and available) or take it out.
pub fn sync(mods_dir: &Path, game_version: &str, enabled: bool) -> Result<()> {
    let path = mods_dir.join(FILE_NAME);
    match jar_for(game_version).filter(|_| enabled) {
        Some(jar) => {
            if fs::read(&path).is_ok_and(|existing| existing == jar) {
                return Ok(());
            }
            fs::create_dir_all(mods_dir).at(mods_dir)?;
            fs::write(&path, jar).at(&path)
        }
        None if path.exists() => fs::remove_file(&path).at(&path),
        None => Ok(()),
    }
}

/// JVM flag pointing the mod at a different cosmetics server, if set.
pub fn jvm_flag() -> Option<String> {
    std::env::var(COSMETICS_URL_ENV)
        .ok()
        .filter(|url| url.starts_with("http://") || url.starts_with("https://"))
        .map(|url| format!("-Darctic.cosmetics.url={url}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installs_updates_and_removes() {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("mods");
        sync(&mods, "26.3", true).unwrap();
        let path = mods.join(FILE_NAME);
        assert!(path.exists());
        fs::write(&path, b"old").unwrap();
        sync(&mods, "26.3", true).unwrap();
        assert_ne!(fs::read(&path).unwrap(), b"old");
        sync(&mods, "26.3", false).unwrap();
        assert!(!path.exists());
        // Unsupported versions never get it.
        sync(&mods, "1.20.1", true).unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn bundled_jar_is_a_fabric_mod() {
        let jar = jar_for("26.3").unwrap();
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(jar)).unwrap();
        assert!(zip.by_name("fabric.mod.json").is_ok());
    }
}
