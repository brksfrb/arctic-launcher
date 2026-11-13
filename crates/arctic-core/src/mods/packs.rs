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

/// A downloaded pack and whether its file was tagged for another game version.
#[derive(Debug, Clone)]
pub struct InstalledPack {
    pub file_name: String,
    pub used_fallback: bool,
}

/// Prefer an exact game version, then the newest file for these pack loaders,
/// using the same release/beta/alpha preference as the exact picker.
fn pick_pack_version<'a>(
    versions: &'a [modrinth::Version],
    loaders: &[&str],
    game_version: &str,
) -> Option<&'a modrinth::Version> {
    modrinth::pick_version(versions, loaders, game_version).or_else(|| {
        versions
            .iter()
            .filter(|v| {
                v.loaders.iter().any(|l| loaders.contains(&l.as_str()))
                    && v.files.iter().any(|file| file.primary)
            })
            .min_by(|a, b| {
                modrinth::channel_rank(&a.version_type)
                    .cmp(&modrinth::channel_rank(&b.version_type))
                    .then_with(|| b.date_published.cmp(&a.date_published))
            })
    })
}

fn resolve_version(
    kind: PackKind,
    project: &str,
    game_version: &str,
) -> Result<(modrinth::Version, bool)> {
    let loaders = kind.loaders();
    let versions = modrinth::project_versions(project, loaders, game_version)?;
    if let Some(version) = modrinth::pick_version(&versions, loaders, game_version) {
        return Ok((version.clone(), false));
    }
    let versions: Vec<modrinth::Version> = modrinth::get_api(
        &modrinth::project_versions_any_game_url(project, loaders),
        &format!("project {project}"),
    )?;
    let version = pick_pack_version(&versions, loaders, game_version)
        .ok_or_else(|| Error::Other("it has no file for this pack type to download".into()))?;
    Ok((
        version.clone(),
        !version.is_compatible(loaders, game_version),
    ))
}

/// Download an exact game-version file when available, otherwise the newest
/// tagged file for this pack type, preferring release, then beta, then alpha.
pub fn install(
    kind: PackKind,
    project: &str,
    game_version: &str,
    game_dir: &Path,
    progress: Progress,
) -> Result<InstalledPack> {
    progress(ProgressInfo::stage("Finding a version"));
    let (version, used_fallback) = resolve_version(kind, project, game_version)?;
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
    Ok(InstalledPack {
        file_name: file.filename.clone(),
        used_fallback,
    })
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

/// A pack moved to the trash, with what it takes to put it back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrashedPack {
    pub file_name: String,
    was_active: bool,
    /// Where it waits: the game folder's `.trash/<packs folder>/`.
    trashed_to: PathBuf,
}

/// Switch a pack off and move it to the game folder's `.trash` (outside the packs
/// folder, so the game does not list it), so [`restore`] can undo it.
pub fn trash(kind: PackKind, game_dir: &Path, file_name: &str) -> Result<TrashedPack> {
    let path = pack_path(kind, game_dir, file_name)?;
    let was_active = active_names(kind, game_dir).iter().any(|n| n == file_name);
    set_active(kind, game_dir, file_name, false)?;
    let trash = game_dir.join(".trash").join(kind.folder());
    std::fs::create_dir_all(&trash).map_err(|e| Error::io(&trash, e))?;
    let trashed_to = trash.join(file_name);
    if trashed_to.is_dir() {
        std::fs::remove_dir_all(&trashed_to).map_err(|e| Error::io(&trashed_to, e))?;
    } else if trashed_to.exists() {
        std::fs::remove_file(&trashed_to).map_err(|e| Error::io(&trashed_to, e))?;
    }
    std::fs::rename(&path, &trashed_to).map_err(|e| Error::io(&trashed_to, e))?;
    Ok(TrashedPack {
        file_name: file_name.to_owned(),
        was_active,
        trashed_to,
    })
}

/// Undo [`trash`]: the pack comes back, switched on again if it was.
pub fn restore(kind: PackKind, game_dir: &Path, trashed: &TrashedPack) -> Result<()> {
    let folder = game_dir.join(kind.folder());
    std::fs::create_dir_all(&folder).map_err(|e| Error::io(&folder, e))?;
    let back = folder.join(&trashed.file_name);
    std::fs::rename(&trashed.trashed_to, &back).map_err(|e| Error::io(&back, e))?;
    if trashed.was_active {
        set_active(kind, game_dir, &trashed.file_name, true)?;
    }
    Ok(())
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

    fn version(id: &str, channel: &str, date: &str, game: &str, loader: &str) -> modrinth::Version {
        serde_json::from_value(serde_json::json!({
            "id": id, "project_id": "pack", "version_number": id,
            "version_type": channel, "date_published": date,
            "game_versions": [game], "loaders": [loader],
            "files": [{"url": format!("https://cdn/{id}.zip"),
                       "filename": format!("{id}.zip"), "primary": true}]
        }))
        .unwrap()
    }

    #[test]
    fn pack_fallback_prefers_channel_then_date_for_both_pack_kinds() {
        for kind in [PackKind::Shader, PackKind::Resource] {
            let loader = kind.loaders()[0];
            let all = vec![
                version("alpha", "alpha", "2026-06-01", "1.21.10", loader),
                version("beta-new", "beta", "2026-05-01", "1.21.10", loader),
                version("beta-old", "beta", "2026-04-01", "1.21.10", loader),
                version("release-old", "release", "2026-02-01", "1.21.10", loader),
                version("release-new", "release", "2026-03-01", "1.21.10", loader),
            ];
            for game in ["1.21.11", "26.2", "26.3"] {
                for (candidates, expected) in [
                    (all.as_slice(), "release-new"),
                    (&all[..3], "beta-new"),
                    (&all[..1], "alpha"),
                ] {
                    let chosen = pick_pack_version(candidates, kind.loaders(), game).unwrap();
                    assert_eq!(chosen.id, expected);
                    assert!(chosen.primary_file().unwrap().primary);
                    assert!(!chosen.is_compatible(kind.loaders(), game));
                }
            }
        }
    }

    #[test]
    fn exact_pack_file_wins_over_a_newer_more_stable_fallback() {
        for kind in [PackKind::Shader, PackKind::Resource] {
            for game in ["1.21.11", "26.3"] {
                let all = vec![
                    version(
                        "fallback",
                        "release",
                        "2026-06-01",
                        "1.21.10",
                        kind.loaders()[0],
                    ),
                    version("exact", "alpha", "2026-01-01", game, kind.loaders()[0]),
                ];
                assert_eq!(
                    pick_pack_version(&all, kind.loaders(), game).unwrap().id,
                    "exact"
                );
            }
        }
    }

    #[test]
    fn fallback_requires_a_file_and_a_pack_loader() {
        for kind in [PackKind::Shader, PackKind::Resource] {
            let mut missing = version(
                "missing",
                "release",
                "2026-06-01",
                "1.21.10",
                kind.loaders()[0],
            );
            missing.files.clear();
            let mut no_primary = version(
                "no-primary",
                "release",
                "2026-06-01",
                "1.21.10",
                kind.loaders()[0],
            );
            no_primary.files[0].primary = false;
            let all = vec![
                missing,
                no_primary,
                version("mod", "release", "2026-06-01", "1.21.10", "fabric"),
            ];
            assert!(pick_pack_version(&all, kind.loaders(), "26.3").is_none());
        }
        let optifine = [version(
            "optifine",
            "beta",
            "2026-01-01",
            "1.21.10",
            "optifine",
        )];
        assert!(pick_pack_version(&optifine, PackKind::Shader.loaders(), "26.3").is_some());
        assert!(pick_pack_version(&optifine, PackKind::Resource.loaders(), "26.3").is_none());
    }

    #[test]
    #[ignore = "live Modrinth API proof; run explicitly with --ignored --nocapture"]
    fn live_rethinking_voxels_search_and_install_fallback() {
        use super::super::{SearchQuery, search};

        for game in ["1.21.11", "26.3"] {
            let query = SearchQuery {
                text: "Rethinking Voxels".into(),
                game_version: game.into(),
                project_type: super::super::ProjectType::Shader,
                ..SearchQuery::default()
            };
            println!("search: {}", modrinth::search_url(&query));
            let page = search(&query).unwrap();
            let hit = page
                .hits
                .iter()
                .find(|h| h.project_id == "kmwfVOoi")
                .unwrap();
            assert_eq!(hit.slug, "rethinking-voxels");
            assert!(hit.is_incompatible(game));
            println!(
                "Minecraft {game}: found {} / {}, incompatible=true, tags={:?}",
                hit.project_id, hit.slug, hit.game_versions
            );

            let facets = serde_json::json!([["project_type:shader"], [format!("versions:{game}")]]);
            let filtered_url = format!(
                "{}/search?query={}&facets={}&index=relevance&offset=0&limit=20",
                modrinth::API,
                modrinth::encode(&query.text),
                modrinth::encode(&facets.to_string())
            );
            println!("negative control: {filtered_url}");
            let filtered: modrinth::SearchResponse =
                modrinth::get_api(&filtered_url, "filtered search").unwrap();
            assert!(filtered.hits.iter().all(|h| h.project_id != "kmwfVOoi"));
            println!(
                "Exact-version facet hides kmwfVOoi: true ({} hits)",
                filtered.total_hits
            );

            let (version, fallback) =
                resolve_version(PackKind::Shader, &hit.project_id, game).unwrap();
            assert!(fallback);
            let file = version.primary_file().unwrap();
            assert!(file.primary);
            println!(
                "Install resolution for {game}: fallback={fallback}, id={}, version={}, channel={}, published={}, loaders={:?}, game_versions={:?}",
                version.id,
                version.version_number,
                version.version_type,
                version.date_published,
                version.loaders,
                version.game_versions
            );
            println!(
                "Primary file: {} (primary={}), URL={}, size={:?}, sha1={:?}",
                file.filename, file.primary, file.url, file.size, file.hashes.sha1
            );
        }
    }

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
    fn a_trashed_pack_leaves_the_list_and_comes_back_active() {
        let g = game();
        set_active(PackKind::Resource, g.path(), "Faithful.zip", true).unwrap();
        let trashed = trash(PackKind::Resource, g.path(), "Faithful.zip").unwrap();
        let names: Vec<String> = list(PackKind::Resource, g.path())
            .into_iter()
            .map(|p| p.file_name)
            .collect();
        assert_eq!(names, ["Other.zip"], "the trash folder is not a pack");
        assert!(!read(&g.path().join(OPTIONS)).contains("Faithful"));
        assert!(g.path().join(".trash/resourcepacks/Faithful.zip").is_file());
        restore(PackKind::Resource, g.path(), &trashed).unwrap();
        let back = list(PackKind::Resource, g.path());
        let faithful = back.iter().find(|p| p.file_name == "Faithful.zip").unwrap();
        assert!(faithful.active, "switched on again");
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
