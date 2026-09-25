//! Old Forge installers (Minecraft 1.7.10 – early 1.12.2), whose
//! `install_profile.json` holds `install` (where the universal jar goes)
//! and `versionInfo` (the launcher profile itself).

use std::path::PathBuf;

use serde_json::{Map, Value, json};

use super::installer::Installer;
use crate::storage::DataDirs;
use crate::versions::maven_path;
use crate::{Error, Result};

/// Forge's old Maven host, still referenced by old profiles.
const OLD_FORGE_MAVEN: &str = "files.minecraftforge.net/maven";
const FORGE_MAVEN: &str = "https://maven.minecraftforge.net";

pub fn is_legacy(install_profile: &Value) -> bool {
    install_profile.get("install").is_some() && install_profile.get("versionInfo").is_some()
}

/// Extract the universal jar and return the cleaned-up profile plus the
/// files (relative to `shared/`) it depends on.
pub fn install(
    dirs: &DataDirs,
    inst: &mut Installer,
    install_profile: &Value,
) -> Result<(Value, Vec<String>)> {
    let install = &install_profile["install"];
    let (Some(coords), Some(file_path)) = (
        install.get("path").and_then(Value::as_str),
        install.get("filePath").and_then(Value::as_str),
    ) else {
        return Err(Error::Other(
            "the Forge installer does not name its universal jar".into(),
        ));
    };
    let rel = maven_path(coords)
        .ok_or_else(|| Error::Other(format!("invalid Forge library name {coords}")))?;
    let dest: PathBuf = dirs.libraries().join(&rel);
    if !inst.extract(file_path, &dest)? {
        return Err(Error::Other(format!(
            "the Forge installer has no {file_path}"
        )));
    }
    let profile = clean_profile(&install_profile["versionInfo"], coords, &rel)?;
    Ok((profile, vec![format!("libraries/{rel}")]))
}

/// Make `versionInfo` loadable by our launcher:
/// - drop server-only libraries (`clientreq: false`);
/// - point the Forge jar at the extracted local file (no URL);
/// - move libraries off Forge's retired Maven host.
fn clean_profile(version_info: &Value, forge_coords: &str, forge_rel: &str) -> Result<Value> {
    let mut profile = version_info
        .as_object()
        .cloned()
        .ok_or_else(|| Error::Other("the Forge installer has no version profile".into()))?;
    let libraries = profile
        .get("libraries")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let cleaned: Vec<Value> = libraries
        .into_iter()
        .filter(|lib| lib.get("clientreq").and_then(Value::as_bool) != Some(false))
        .filter_map(|lib| clean_library(lib, forge_coords, forge_rel))
        .collect();
    profile.insert("libraries".into(), Value::Array(cleaned));
    Ok(Value::Object(profile))
}

fn clean_library(lib: Value, forge_coords: &str, forge_rel: &str) -> Option<Value> {
    let mut lib: Map<String, Value> = lib.as_object()?.clone();
    let name = lib.get("name")?.as_str()?.to_owned();
    if name == forge_coords {
        return Some(json!({
            "name": name,
            "downloads": { "artifact": { "path": forge_rel, "url": "" } }
        }));
    }
    if let Some(url) = lib.get("url").and_then(Value::as_str) {
        let url = modern_maven_url(url);
        lib.insert("url".into(), Value::String(url));
    }
    Some(Value::Object(lib))
}

fn modern_maven_url(url: &str) -> String {
    match url.find(OLD_FORGE_MAVEN) {
        Some(pos) => format!("{FORGE_MAVEN}{}", &url[pos + OLD_FORGE_MAVEN.len()..]),
        None => url.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loaders::util::parse_profile;
    use crate::versions::merge;

    const INSTALL_PROFILE: &str = r#"{
        "install": {
            "profileName": "Forge", "target": "1.7.10-Forge10.13.4.1614-1.7.10",
            "path": "net.minecraftforge:forge:1.7.10-10.13.4.1614-1.7.10",
            "filePath": "forge-1.7.10-10.13.4.1614-1.7.10-universal.jar", "minecraft": "1.7.10"
        },
        "versionInfo": {
            "id": "1.7.10-Forge10.13.4.1614-1.7.10", "type": "release", "inheritsFrom": "1.7.10", "jar": "1.7.10",
            "mainClass": "net.minecraft.launchwrapper.Launch",
            "minecraftArguments": "--username ${auth_player_name} --tweakClass cpw.mods.fml.common.launcher.FMLTweaker",
            "libraries": [
                {"name": "net.minecraftforge:forge:1.7.10-10.13.4.1614-1.7.10", "url": "https://maven.minecraftforge.net/"},
                {"name": "net.minecraft:launchwrapper:1.12", "serverreq": true},
                {"name": "com.typesafe:config:1.2.1", "url": "http://files.minecraftforge.net/maven/",
                 "checksums": ["a", "b"], "serverreq": true, "clientreq": true},
                {"name": "server:only:1", "clientreq": false},
                {"name": "com.google.guava:guava:17.0"}
            ]
        }
    }"#;

    fn profile() -> Value {
        let root: Value = serde_json::from_str(INSTALL_PROFILE).unwrap();
        assert!(is_legacy(&root));
        clean_profile(
            &root["versionInfo"],
            "net.minecraftforge:forge:1.7.10-10.13.4.1614-1.7.10",
            "net/minecraftforge/forge/1.7.10-10.13.4.1614-1.7.10/forge-1.7.10-10.13.4.1614-1.7.10.jar",
        )
        .unwrap()
    }

    #[test]
    fn cleans_libraries() {
        let p = profile();
        let libs = p["libraries"].as_array().unwrap();
        let names: Vec<&str> = libs.iter().map(|l| l["name"].as_str().unwrap()).collect();
        assert!(!names.contains(&"server:only:1"));
        assert_eq!(libs.len(), 4);
        assert_eq!(libs[0]["downloads"]["artifact"]["url"], "");
        assert!(libs[0].get("url").is_none());
        assert_eq!(libs[2]["url"], "https://maven.minecraftforge.net/");
    }

    #[test]
    fn merges_onto_vanilla_replacing_legacy_arguments() {
        let vanilla: crate::versions::VersionJson = serde_json::from_str(
            r#"{"id":"1.7.10","type":"release","mainClass":"net.minecraft.client.main.Main",
                "minecraftArguments":"--username ${auth_player_name} --session ${auth_session}",
                "assets":"1.7.10",
                "downloads":{"client":{"sha1":"s","size":1,"url":"u"}},
                "libraries":[{"name":"com.google.guava:guava:15.0"},{"name":"org.lwjgl.lwjgl:lwjgl:2.9.1"}]}"#,
        )
        .unwrap();
        let merged = merge(vanilla, parse_profile(&profile()).unwrap());
        assert_eq!(merged.main_class, "net.minecraft.launchwrapper.Launch");
        assert!(merged.minecraft_arguments.unwrap().contains("FMLTweaker"));
        assert!(merged.downloads.is_some());
        let names: Vec<&str> = merged.libraries.iter().map(|l| l.name.as_str()).collect();
        assert!(names.contains(&"com.google.guava:guava:17.0"));
        assert!(!names.contains(&"com.google.guava:guava:15.0"));
        assert!(names.contains(&"org.lwjgl.lwjgl:lwjgl:2.9.1"));
    }

    #[test]
    fn modern_installers_are_not_legacy() {
        let v: Value = serde_json::from_str(r#"{"spec":1,"version":"x","data":{}}"#).unwrap();
        assert!(!is_legacy(&v));
        assert_eq!(
            modern_maven_url("http://files.minecraftforge.net/maven/"),
            "https://maven.minecraftforge.net/"
        );
    }
}
