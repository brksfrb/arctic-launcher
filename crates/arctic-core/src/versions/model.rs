//! Serde model of Mojang's per-version JSON (`<id>.json`).
//!
//! Only the fields the launcher uses are modelled; unknown fields are ignored
//! so future format additions do not break parsing.

use std::collections::HashMap;

use serde::Deserialize;

use super::rules::Rule;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionJson {
    pub id: String,
    #[serde(rename = "type", default)]
    pub kind: String,
    pub main_class: String,
    /// Modern (1.13+) argument format.
    pub arguments: Option<Arguments>,
    /// Legacy (<1.13) space-separated game arguments.
    pub minecraft_arguments: Option<String>,
    pub asset_index: Option<AssetIndexRef>,
    pub assets: Option<String>,
    pub downloads: Option<VersionDownloads>,
    #[serde(default)]
    pub libraries: Vec<Library>,
    pub java_version: Option<JavaVersion>,
    pub logging: Option<Logging>,
    /// Set by mod-loader profiles; vanilla never uses it. Kept so loaders can
    /// be layered on later without changing the model.
    pub inherits_from: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<Argument>,
    #[serde(default)]
    pub jvm: Vec<Argument>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Argument {
    Plain(String),
    Conditional { rules: Vec<Rule>, value: ArgValue },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ArgValue {
    One(String),
    Many(Vec<String>),
}

impl ArgValue {
    pub fn values(&self) -> Vec<&str> {
        match self {
            Self::One(s) => vec![s.as_str()],
            Self::Many(v) => v.iter().map(String::as_str).collect(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetIndexRef {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VersionDownloads {
    pub client: Download,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Download {
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Library {
    pub name: String,
    pub downloads: Option<LibraryDownloads>,
    /// Maven repository base URL (Fabric/Quilt/Forge style libraries that
    /// have no `downloads` block). The file path comes from `name`.
    pub url: Option<String>,
    pub sha1: Option<String>,
    pub size: Option<u64>,
    /// Legacy natives: OS name → classifier (may contain `${arch}`).
    pub natives: Option<HashMap<String, String>>,
    pub rules: Option<Vec<Rule>>,
    pub extract: Option<Extract>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LibraryDownloads {
    pub artifact: Option<Artifact>,
    pub classifiers: Option<HashMap<String, Artifact>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Artifact {
    pub path: Option<String>,
    pub sha1: Option<String>,
    pub size: Option<u64>,
    /// Empty for files generated locally (e.g. Forge's patched client jar).
    #[serde(default)]
    pub url: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Extract {
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersion {
    pub component: String,
    pub major_version: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Logging {
    pub client: Option<LoggingClient>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingClient {
    pub argument: String,
    pub file: LoggingFile,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingFile {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

/// Asset index file (`assets/indexes/<id>.json`).
#[derive(Debug, Clone, Deserialize)]
pub struct AssetIndex {
    pub objects: HashMap<String, AssetObject>,
    /// Pre-1.7 "legacy" index: objects are also copied to `assets/virtual/<id>`.
    #[serde(default, rename = "virtual")]
    pub is_virtual: bool,
    /// Pre-1.6 index: objects are copied into `<game_dir>/resources`.
    #[serde(default)]
    pub map_to_resources: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetObject {
    pub hash: String,
    pub size: u64,
}

/// Convert a maven coordinate (`group:artifact:version[:classifier][@ext]`)
/// into a repository-relative path.
pub fn maven_path(name: &str) -> Option<String> {
    let (coords, ext) = name.split_once('@').unwrap_or((name, "jar"));
    let mut parts = coords.split(':');
    let group = parts.next()?;
    let artifact = parts.next()?;
    let version = parts.next()?;
    let classifier = parts.next().map(|c| format!("-{c}")).unwrap_or_default();
    Some(format!(
        "{}/{artifact}/{version}/{artifact}-{version}{classifier}.{ext}",
        group.replace('.', "/")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maven_paths() {
        assert_eq!(
            maven_path("org.lwjgl:lwjgl:3.3.3:natives-windows").unwrap(),
            "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-windows.jar"
        );
        assert_eq!(maven_path("a.b:c:1@zip").unwrap(), "a/b/c/1/c-1.zip");
        assert!(maven_path("broken").is_none());
    }

    #[test]
    fn parses_modern_arguments() {
        let json = r#"{
            "id": "1.21", "type": "release", "mainClass": "net.minecraft.client.main.Main",
            "arguments": {
                "game": ["--username", "${auth_player_name}",
                    {"rules": [{"action": "allow", "features": {"has_custom_resolution": true}}],
                     "value": ["--width", "${resolution_width}"]}],
                "jvm": [{"rules": [{"action": "allow", "os": {"name": "osx"}}], "value": "-XstartOnFirstThread"}]
            },
            "javaVersion": {"component": "java-runtime-delta", "majorVersion": 21}
        }"#;
        let v: VersionJson = serde_json::from_str(json).unwrap();
        let args = v.arguments.unwrap();
        assert_eq!(args.game.len(), 3);
        assert!(
            matches!(&args.jvm[0], Argument::Conditional { value: ArgValue::One(s), .. } if s == "-XstartOnFirstThread")
        );
        assert_eq!(v.java_version.unwrap().major_version, 21);
    }

    #[test]
    fn parses_legacy_asset_index_flags() {
        let idx: AssetIndex = serde_json::from_str(
            r#"{"virtual": true, "objects": {"a/b.ogg": {"hash": "abcd", "size": 3}}}"#,
        )
        .unwrap();
        assert!(idx.is_virtual && !idx.map_to_resources);
        assert_eq!(idx.objects["a/b.ogg"].size, 3);
    }
}
