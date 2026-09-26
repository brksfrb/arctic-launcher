//! Resource packs and shader packs from Modrinth: find, install into an
//! instance, list, switch on or off (the game's `options.txt` for resource
//! packs, Iris's `iris.properties` for shaders) and remove.

use std::path::{Path, PathBuf};

use super::modrinth;
use crate::loaders::LoaderKind;
use crate::net::{DownloadJob, download_all};
use crate::{Error, Progress, ProgressInfo, Result};

/// Iris (Fabric, Quilt, NeoForge) and Oculus (Forge) on Modrinth.
pub const IRIS: &str = "YL57xq9U";
pub const OCULUS: &str = "GchcoXML";
const OPTIONS: &str = "options.txt";
const IRIS_CONFIG: &str = "config/iris.properties";
const RESOURCE_PACKS_KEY: &str = "resourcePacks:";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PackKind {
    Resource,
    Shader,
}

impl PackKind {
    pub fn folder(self) -> &'static str {
        match self {
            PackKind::Resource => "resourcepacks",
            PackKind::Shader => "shaderpacks",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            PackKind::Resource => "Resource packs",
            PackKind::Shader => "Shaders",
        }
    }

    /// Modrinth's project type.
    pub fn project_type(self) -> super::ProjectType {
        match self {
            PackKind::Resource => super::ProjectType::ResourcePack,
            PackKind::Shader => super::ProjectType::Shader,
        }
    }

    /// Modrinth's "loaders" for this kind of file.
    fn loaders(self) -> &'static [&'static str] {
        match self {
            PackKind::Resource => &["minecraft"],
            // Iris reads OptiFine-format shader packs too.
            PackKind::Shader => &["iris", "optifine"],
        }
    }
}

/// A pack in an instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pack {
    pub file_name: String,
    pub size: u64,
    pub active: bool,
}

/// Download the newest version of `project` that fits `game_version`.
/// Returns the file name.
pub fn install(
    kind: PackKind,
    project: &str,
    game_version: &str,
    game_dir: &Path,
    progress: Progress,
) -> Result<String> {
    progress(ProgressInfo::stage("Finding a version"));
    let loaders = kind.loaders();
    let versions = modrinth::project_versions(project, loaders, game_version)?;
    let version = modrinth::pick_version(&versions, loaders, game_version).ok_or_else(|| {
        Error::Other(format!(
            "it has no version for Minecraft {game_version} yet"
        ))
    })?;
    let file = version
        .primary_file()
        .ok_or_else(|| Error::Other("it has no file to download".into()))?;
    super::files::check_pack_name(&file.filename)?;
    let dir = game_dir.join(kind.folder());
    download_all(
        "Downloading",
        vec![DownloadJob {
            url: file.url.clone(),
            dest: dir.join(&file.filename),
            sha1: file.hashes.sha1.clone(),
            size: file.size,
            lzma: None,
        }],
        progress,
    )?;
    Ok(file.filename.clone())
}

/// Packs in the instance (files and unpacked folders), by name.
pub fn list(kind: PackKind, game_dir: &Path) -> Vec<Pack> {
    let active = active_names(kind, game_dir);
    let mut out: Vec<Pack> = std::fs::read_dir(game_dir.join(kind.folder()))
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().into_owned();
                    let meta = e.metadata().ok()?;
                    let usable = meta.is_dir() || name.to_ascii_lowercase().ends_with(".zip");
                    usable.then(|| Pack {
                        active: active.contains(&name),
                        size: if meta.is_dir() { 0 } else { meta.len() },
                        file_name: name,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_by_key(|p| p.file_name.to_lowercase());
    out
}

/// Delete a pack (and switch it off).
pub fn remove(kind: PackKind, game_dir: &Path, file_name: &str) -> Result<()> {
    let path = pack_path(kind, game_dir, file_name)?;
    set_active(kind, game_dir, file_name, false)?;
    if path.is_dir() {
        std::fs::remove_dir_all(&path).map_err(|e| Error::io(&path, e))
    } else {
        std::fs::remove_file(&path).map_err(|e| Error::io(&path, e))
    }
}

/// Switch a pack on or off for the game. Resource packs stack (the newest
/// one switched on wins); only one shader pack is used at a time.
pub fn set_active(kind: PackKind, game_dir: &Path, file_name: &str, on: bool) -> Result<()> {
    pack_path(kind, game_dir, file_name)?;
    if crate::sharing::client::in_use(game_dir) {
        return Err(Error::Other(
            "Minecraft is running from this instance; close it first (it saves its own settings when it quits)"
                .into(),
        ));
    }
    match kind {
        PackKind::Resource => set_resource_pack(game_dir, file_name, on),
        PackKind::Shader => set_shader(game_dir, file_name, on),
    }
}

/// `file_name` must be a plain name of an existing pack (no paths).
fn pack_path(kind: PackKind, game_dir: &Path, file_name: &str) -> Result<PathBuf> {
    let plain = !file_name.is_empty()
        && !file_name.contains(['/', '\\'])
        && file_name != "."
        && file_name != "..";
    let path = game_dir.join(kind.folder()).join(file_name);
    if plain && path.exists() {
        Ok(path)
    } else {
        Err(Error::Other(format!("there's no pack called {file_name}")))
    }
}

fn active_names(kind: PackKind, game_dir: &Path) -> Vec<String> {
    match kind {
        PackKind::Resource => resource_packs(&read(&game_dir.join(OPTIONS)))
            .into_iter()
            .filter_map(|p| p.strip_prefix("file/").map(str::to_owned))
            .collect(),
        PackKind::Shader => {
            let props = read(&game_dir.join(IRIS_CONFIG));
            let get = |k: &str| {
                props.lines().find_map(|l| {
                    l.strip_prefix(k)
                        .and_then(|r| r.strip_prefix('='))
                        .map(|v| v.trim().to_owned())
                })
            };
            match (get("enableShaders").as_deref(), get("shaderPack")) {
                (Some("true"), Some(pack)) => vec![unescape(&pack)],
                _ => Vec::new(),
            }
        }
    }
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

/// The `resourcePacks:[...]` list from options.txt.
fn resource_packs(options: &str) -> Vec<String> {
    options
        .lines()
        .find_map(|l| l.strip_prefix(RESOURCE_PACKS_KEY))
        .and_then(|list| serde_json::from_str::<Vec<String>>(list.trim()).ok())
        .unwrap_or_else(|| vec!["vanilla".into()])
}

fn set_resource_pack(game_dir: &Path, file_name: &str, on: bool) -> Result<()> {
    let path = game_dir.join(OPTIONS);
    let options = read(&path);
    let entry = format!("file/{file_name}");
    let mut packs: Vec<String> = resource_packs(&options)
        .into_iter()
        .filter(|p| *p != entry)
        .collect();
    if on {
        // Last in the list is on top.
        packs.push(entry);
    }
    let line = format!(
        "{RESOURCE_PACKS_KEY}{}",
        serde_json::to_string(&packs).unwrap_or_else(|_| "[]".into())
    );
    write_setting(&path, &options, RESOURCE_PACKS_KEY, &line)
}

fn set_shader(game_dir: &Path, file_name: &str, on: bool) -> Result<()> {
    let path = game_dir.join(IRIS_CONFIG);
    let mut props = read(&path);
    if on {
        let line = format!("shaderPack={}", escape(file_name));
        props = replace_or_add(&props, "shaderPack=", &line);
    }
    let enabled = if on {
        "enableShaders=true"
    } else if active_names(PackKind::Shader, game_dir).contains(&file_name.to_owned()) {
        "enableShaders=false"
    } else {
        return Ok(());
    };
    props = replace_or_add(&props, "enableShaders=", enabled);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    }
    std::fs::write(&path, props).map_err(|e| Error::io(&path, e))
}

fn write_setting(path: &Path, text: &str, key: &str, line: &str) -> Result<()> {
    std::fs::write(path, replace_or_add(text, key, line)).map_err(|e| Error::io(path, e))
}

/// `text` with the line starting `key` replaced by `line` (or added).
fn replace_or_add(text: &str, key: &str, line: &str) -> String {
    let mut found = false;
    let mut out: Vec<String> = text
        .lines()
        .map(|l| {
            if l.starts_with(key) {
                found = true;
                line.to_owned()
            } else {
                l.to_owned()
            }
        })
        .collect();
    if !found {
        out.push(line.to_owned());
    }
    out.join("\n") + "\n"
}

/// Java .properties escaping for the characters a file name can hold.
fn escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace(':', "\\:")
        .replace('=', "\\=")
}

fn unescape(value: &str) -> String {
    let mut out = String::new();
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            out.extend(chars.next());
        } else {
            out.push(c);
        }
    }
    out
}

/// The shader loader mod for a loader (the Vanilla instance uses its own
/// switch instead).
pub fn shader_mod(loader: LoaderKind) -> &'static str {
    match loader {
        LoaderKind::Forge => OCULUS,
        _ => IRIS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for f in [
            "resourcepacks/Faithful.zip",
            "resourcepacks/Other.zip",
            "shaderpacks/BSL v8.2.zip",
        ] {
            let p = dir.path().join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, b"zip").unwrap();
        }
        dir
    }

    #[test]
    fn resource_packs_stack_in_options() {
        let g = game();
        std::fs::write(
            g.path().join(OPTIONS),
            "fov:0.5\nresourcePacks:[\"vanilla\"]\nlang:en_us\n",
        )
        .unwrap();
        set_active(PackKind::Resource, g.path(), "Faithful.zip", true).unwrap();
        set_active(PackKind::Resource, g.path(), "Other.zip", true).unwrap();
        let options = read(&g.path().join(OPTIONS));
        assert!(
            options
                .contains("resourcePacks:[\"vanilla\",\"file/Faithful.zip\",\"file/Other.zip\"]")
        );
        assert!(options.contains("fov:0.5") && options.contains("lang:en_us"));
        set_active(PackKind::Resource, g.path(), "Faithful.zip", false).unwrap();
        let on: Vec<_> = list(PackKind::Resource, g.path())
            .into_iter()
            .filter(|p| p.active)
            .map(|p| p.file_name)
            .collect();
        assert_eq!(on, vec!["Other.zip".to_owned()]);
    }

    #[test]
    fn one_shader_at_a_time_through_iris() {
        let g = game();
        set_active(PackKind::Shader, g.path(), "BSL v8.2.zip", true).unwrap();
        let props = read(&g.path().join(IRIS_CONFIG));
        assert!(props.contains("shaderPack=BSL v8.2.zip") && props.contains("enableShaders=true"));
        assert!(list(PackKind::Shader, g.path())[0].active);
        set_active(PackKind::Shader, g.path(), "BSL v8.2.zip", false).unwrap();
        assert!(!list(PackKind::Shader, g.path())[0].active);
    }

    #[test]
    fn refuses_paths_and_missing_packs() {
        let g = game();
        assert!(set_active(PackKind::Resource, g.path(), "../options.txt", true).is_err());
        assert!(remove(PackKind::Resource, g.path(), "nope.zip").is_err());
        remove(PackKind::Resource, g.path(), "Other.zip").unwrap();
        assert_eq!(list(PackKind::Resource, g.path()).len(), 1);
    }
}
