//! Resource packs and shader packs from Modrinth: find, install into an
//! instance, list, switch on or off (the game's `options.txt` for resource
//! packs, Iris's `iris.properties` for shaders) and remove.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::modrinth;
use super::resolve::{self, Planned, Target};
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
        self.project_type().folder().unwrap_or("resourcepacks")
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
    pub(super) fn loaders(self) -> &'static [&'static str] {
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
    kind: PackKind,
    dependencies: Vec<String>,
}

impl InstalledPack {
    /// Enable resource-pack dependencies below the pack in the same options edit.
    pub fn activate(&self, game_dir: &Path, game: &str) -> Result<()> {
        if self.dependencies.is_empty() {
            return set_active(self.kind, game_dir, game, &self.file_name, true);
        }
        let _reservation = reserve_for_edit(game_dir)?;
        let names: Vec<&str> = self
            .dependencies
            .iter()
            .map(String::as_str)
            .chain(std::iter::once(self.file_name.as_str()))
            .collect();
        for name in &names {
            pack_path(self.kind, game_dir, name)?;
        }
        set_resource_packs(game_dir, game, &names, true)
    }
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
    download_version(kind, &version, used_fallback, game_dir, progress)
}

/// Fetch and check an explicitly chosen pack version before anything is installed.
/// A file tagged for another game version passes only with `allow_other_game` (the player
/// confirmed it); the wrong pack type or a missing file never does.
pub fn selected_version(
    kind: PackKind,
    version_id: &str,
    game_version: &str,
    allow_other_game: bool,
) -> Result<SelectedPack> {
    let version = modrinth::version(version_id)?;
    let other_game = check_selected_version(kind, &version, game_version, allow_other_game)?;
    Ok(SelectedPack {
        kind,
        version,
        other_game,
    })
}

/// A checked pack version, ready to download.
#[derive(Debug, Clone)]
pub struct SelectedPack {
    kind: PackKind,
    version: modrinth::Version,
    /// Tagged for another game version (the player confirmed it).
    pub other_game: bool,
}

/// Install a version picked with [`selected_version`].
pub fn install_selected(
    selected: &SelectedPack,
    game_dir: &Path,
    progress: Progress,
) -> Result<InstalledPack> {
    download_version(
        selected.kind,
        &selected.version,
        selected.other_game,
        game_dir,
        progress,
    )
}

fn check_selected_version(
    kind: PackKind,
    version: &modrinth::Version,
    game: &str,
    allow_other_game: bool,
) -> Result<bool> {
    check_install_version(kind.loaders(), version, game, allow_other_game)
}

pub(super) fn check_install_version(
    loaders: &[&str],
    version: &modrinth::Version,
    game: &str,
    allow_other_game: bool,
) -> Result<bool> {
    let right_type = version
        .loaders
        .iter()
        .any(|l| loaders.contains(&l.as_str()));
    if !right_type || version.primary_file().is_none() {
        return Err(Error::IncompatibleContentVersion);
    }
    let other_game = !game.is_empty() && !version.is_compatible(loaders, game);
    if other_game && !allow_other_game {
        return Err(Error::IncompatibleContentVersion);
    }
    Ok(other_game)
}

fn download_version(
    kind: PackKind,
    version: &modrinth::Version,
    used_fallback: bool,
    game_dir: &Path,
    progress: Progress,
) -> Result<InstalledPack> {
    let job = download_job(version, &game_dir.join(kind.folder()))?;
    let file_name = version
        .primary_file()
        .expect("validated file")
        .filename
        .clone();
    download_all("Downloading", vec![job], progress)?;
    Ok(InstalledPack {
        file_name,
        used_fallback,
        kind,
        dependencies: Vec::new(),
    })
}

fn download_job(version: &modrinth::Version, dest: &Path) -> Result<DownloadJob> {
    let file = version
        .primary_file()
        .ok_or_else(|| Error::Other("it has no file to download".into()))?;
    super::files::check_pack_name(&file.filename)?;
    Ok(DownloadJob {
        url: file.url.clone(),
        dest: dest.join(&file.filename),
        sha1: file.hashes.sha1.clone(),
        size: file.size,
        lzma: None,
        md5: None,
    })
}

/// Install the selected archive and the required Modrinth archives that go with it.
pub fn install_with_dependencies(
    selected: &SelectedPack,
    game: &str,
    game_dir: &Path,
    present: &HashSet<String>,
    progress: Progress,
) -> Result<InstalledPack> {
    let dest = game_dir.join(selected.kind.folder());
    let plan = plan_archives(
        selected.version.clone(),
        selected.kind.loaders(),
        game,
        selected.other_game,
        present,
    )?;
    let file_name = selected
        .version
        .primary_file()
        .expect("validated file")
        .filename
        .clone();
    download_archives(&plan, &dest, progress)?;
    let dependencies = if selected.kind == PackKind::Resource {
        plan.iter()
            .skip(1)
            .rev()
            .filter_map(|dep| dep.version.primary_file())
            .map(|file| file.filename.clone())
            .collect()
    } else {
        Vec::new()
    };
    Ok(InstalledPack {
        file_name,
        used_fallback: selected.other_game,
        kind: selected.kind,
        dependencies,
    })
}

pub(super) fn plan_archives(
    version: modrinth::Version,
    loaders: &[&str],
    game: &str,
    other_game: bool,
    present: &HashSet<String>,
) -> Result<Vec<Planned>> {
    // For a confirmed fallback, resolve its dependencies for the version the file targets.
    let game_version = if game.is_empty() || other_game {
        version
            .game_versions
            .first()
            .map_or(game, String::as_str)
            .to_owned()
    } else {
        game.to_owned()
    };
    let target = Target {
        loaders,
        game_version: &game_version,
        loader_name: &loaders.join("/"),
    };
    let source = resolve::Modrinth {
        loaders,
        game_version: &game_version,
    };
    let project = version.project_id.clone();
    resolve::resolve_root(&project, version, target, present, &source)
}

pub(super) fn download_archives(plan: &[Planned], dest: &Path, progress: Progress) -> Result<()> {
    let mut jobs = plan
        .iter()
        .rev()
        .map(|planned| download_job(&planned.version, dest))
        .collect::<Result<Vec<_>>>()?;
    // Validate every file first. A failed dependency download should not leave a new root.
    let root = jobs.pop().expect("resolved root");
    if !jobs.is_empty() {
        download_all("Downloading", jobs, progress)?;
    }
    download_all("Downloading", vec![root], progress)
}

/// Packs in the instance (files and unpacked folders), by name.
pub fn list(kind: PackKind, game_dir: &Path, game: &str) -> Vec<Pack> {
    let active = active_names(kind, game_dir, game);
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
pub fn remove(kind: PackKind, game_dir: &Path, game: &str, file_name: &str) -> Result<()> {
    let _reservation = reserve_for_edit(game_dir)?;
    let path = pack_path(kind, game_dir, file_name)?;
    update_active(kind, game_dir, game, file_name, false)?;
    if path.is_dir() {
        std::fs::remove_dir_all(&path).map_err(|e| Error::io(&path, e))
    } else {
        std::fs::remove_file(&path).map_err(|e| Error::io(&path, e))
    }
}

/// Switch a pack on or off for the game. Resource packs stack (the newest
/// one switched on wins); only one shader pack is used at a time.
pub fn set_active(
    kind: PackKind,
    game_dir: &Path,
    game: &str,
    file_name: &str,
    on: bool,
) -> Result<()> {
    let _reservation = reserve_for_edit(game_dir)?;
    pack_path(kind, game_dir, file_name)?;
    update_active(kind, game_dir, game, file_name, on)
}

fn reserve_for_edit(game_dir: &Path) -> Result<std::fs::File> {
    if let Some(reservation) = crate::launch::session::try_reserve_dir(game_dir)?
        && !crate::sharing::client::in_use(game_dir)
    {
        return Ok(reservation);
    }
    Err(Error::Other(
        "Minecraft is running from this instance; close it first (it saves its own settings when it quits)"
            .into(),
    ))
}

fn update_active(
    kind: PackKind,
    game_dir: &Path,
    game: &str,
    file_name: &str,
    on: bool,
) -> Result<()> {
    match kind {
        PackKind::Resource => set_resource_packs(game_dir, game, &[file_name], on),
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

fn active_names(kind: PackKind, game_dir: &Path, game: &str) -> Vec<String> {
    match kind {
        PackKind::Resource => resource_packs(&read(&game_dir.join(OPTIONS)))
            .ok()
            .flatten()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|p| {
                if uses_bare_pack_names(game) {
                    (!p.contains('/')).then_some(p)
                } else {
                    p.strip_prefix("file/").map(str::to_owned)
                }
            })
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
fn resource_packs(options: &str) -> Result<Option<Vec<String>>> {
    options
        .lines()
        .rev()
        .find_map(|line| line.strip_prefix(RESOURCE_PACKS_KEY))
        .map(serde_json::from_str)
        .transpose()
        .map_err(Error::from)
}

// The file repository introduced in 1.13 uses file/ IDs. Older releases
// match the saved selection directly against the filename.
fn uses_bare_pack_names(game: &str) -> bool {
    game.strip_prefix("1.")
        .and_then(|version| version.split(['.', '-']).next()?.parse::<u32>().ok())
        .is_some_and(|minor| minor < 13)
}

fn set_resource_packs(game_dir: &Path, game: &str, file_names: &[&str], on: bool) -> Result<()> {
    let path = game_dir.join(OPTIONS);
    let options = match std::fs::read_to_string(&path) {
        Ok(options) => options,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && !on => return Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(Error::io(&path, e)),
    };
    let bare_names = uses_bare_pack_names(game);
    let mut packs = resource_packs(&options)?.unwrap_or_else(|| {
        if bare_names {
            Vec::new()
        } else {
            vec!["vanilla".into()]
        }
    });
    let count = packs.len();
    for &file_name in file_names {
        let entry = format!("file/{file_name}");
        packs.retain(|p| p != &entry && !(bare_names && p == file_name));
        if on {
            // Last in the list is on top.
            packs.push(if bare_names {
                file_name.to_owned()
            } else {
                entry
            });
        }
    }
    if !on && packs.len() == count {
        return Ok(());
    }
    let line = format!("{RESOURCE_PACKS_KEY}{}", serde_json::to_string(&packs)?);
    crate::storage::write_atomic(
        &path,
        replace_or_add(&options, RESOURCE_PACKS_KEY, &line).as_bytes(),
    )
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
    } else if active_names(PackKind::Shader, game_dir, "").contains(&file_name.to_owned()) {
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
    fn selected_pack_targets_the_exact_file_and_content_folder() {
        for (kind, loader, folder) in [
            (PackKind::Resource, "minecraft", "resourcepacks"),
            (PackKind::Shader, "iris", "shaderpacks"),
            (PackKind::Shader, "optifine", "shaderpacks"),
        ] {
            let selected = version("older", "beta", "2023", "1.21.4", loader);
            assert!(!check_selected_version(kind, &selected, "1.21.4", false).unwrap());
            let job = download_job(
                &selected,
                &Path::new("selected-instance").join(kind.folder()),
            )
            .unwrap();
            assert_eq!(
                job.dest,
                Path::new("selected-instance")
                    .join(folder)
                    .join("older.zip")
            );
            assert_eq!(job.url, "https://cdn/older.zip");
            // Another game version needs the player's confirmation, then is flagged.
            assert!(check_selected_version(kind, &selected, "1.20.1", false).is_err());
            assert!(check_selected_version(kind, &selected, "1.20.1", true).unwrap());
            let mut no_file = selected.clone();
            no_file.files.clear();
            assert!(check_selected_version(kind, &no_file, "1.21.4", true).is_err());
        }
        // The wrong pack type is never installed, confirmed or not.
        let canvas = version("canvas", "release", "2023", "1.21.4", "canvas");
        assert!(check_selected_version(PackKind::Shader, &canvas, "1.21.4", true).is_err());
        let iris = version("iris", "release", "2023", "1.21.4", "iris");
        assert!(check_selected_version(PackKind::Resource, &iris, "1.21.4", true).is_err());
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
    fn resource_packs_stack_in_options() {
        let g = game();
        std::fs::write(
            g.path().join(OPTIONS),
            "fov:0.5\nresourcePacks:[\"vanilla\"]\nlang:en_us\n",
        )
        .unwrap();
        set_active(PackKind::Resource, g.path(), "1.21.4", "Faithful.zip", true).unwrap();
        set_active(PackKind::Resource, g.path(), "1.21.4", "Other.zip", true).unwrap();
        let options = read(&g.path().join(OPTIONS));
        assert!(
            options
                .contains("resourcePacks:[\"vanilla\",\"file/Faithful.zip\",\"file/Other.zip\"]")
        );
        assert!(options.contains("fov:0.5") && options.contains("lang:en_us"));
        set_active(
            PackKind::Resource,
            g.path(),
            "1.21.4",
            "Faithful.zip",
            false,
        )
        .unwrap();
        let on: Vec<_> = list(PackKind::Resource, g.path(), "1.21.4")
            .into_iter()
            .filter(|p| p.active)
            .map(|p| p.file_name)
            .collect();
        assert_eq!(on, vec!["Other.zip".to_owned()]);
    }

    #[test]
    fn legacy_pack_toggle_uses_bare_names_and_repairs_a_previous_toggle() {
        let g = game();
        let path = g.path().join(OPTIONS);
        let kind = PackKind::Resource;
        std::fs::write(
            &path,
            "resourcePacks:[\"Faithful.zip\",\"file/Other.zip\"]\n",
        )
        .unwrap();
        assert!(list(kind, g.path(), "1.8.9")[0].active);
        assert!(!list(kind, g.path(), "1.8.9")[1].active);
        set_active(kind, g.path(), "1.8.9", "Other.zip", true).unwrap();
        assert_eq!(
            read(&path),
            "resourcePacks:[\"Faithful.zip\",\"Other.zip\"]\n"
        );
        assert!(list(kind, g.path(), "1.8.9").iter().all(|pack| pack.active));
        set_active(kind, g.path(), "1.8.9", "Faithful.zip", false).unwrap();
        assert_eq!(read(&path), "resourcePacks:[\"Other.zip\"]\n");

        std::fs::remove_file(&path).unwrap();
        set_active(kind, g.path(), "1.12.2", "Faithful.zip", true).unwrap();
        assert_eq!(read(&path), "resourcePacks:[\"Faithful.zip\"]\n");
        std::fs::remove_file(&path).unwrap();
        set_active(kind, g.path(), "1.13.2", "Faithful.zip", true).unwrap();
        assert_eq!(
            read(&path),
            "resourcePacks:[\"vanilla\",\"file/Faithful.zip\"]\n"
        );
    }

    #[test]
    fn resource_pack_edit_preserves_unreadable_or_malformed_options() {
        let g = game();
        let path = g.path().join(OPTIONS);
        for original in [
            b"lang:en_us\nresourcePacks:[\"file/Other.zip\"\n".as_slice(),
            b"lang:en_us\nresourcePacks:[\"file/\xe9t\xe9.zip\"]\n",
        ] {
            std::fs::write(&path, original).unwrap();
            assert!(
                set_active(PackKind::Resource, g.path(), "1.21.4", "Faithful.zip", true).is_err()
            );
            assert_eq!(std::fs::read(&path).unwrap(), original);
        }
    }

    #[test]
    fn cannot_edit_packs_while_a_watched_game_owns_the_folder() {
        let g = game();
        let path = g.path().join(OPTIONS);
        let original = "resourcePacks:[\"vanilla\",\"file/Faithful.zip\"]\n";
        std::fs::write(&path, original).unwrap();
        let reservation = crate::launch::session::try_reserve_dir(g.path())
            .unwrap()
            .unwrap();
        let kind = PackKind::Resource;
        assert!(set_active(kind, g.path(), "1.21.4", "Other.zip", true).is_err());
        assert!(remove(kind, g.path(), "1.21.4", "Faithful.zip").is_err());
        assert_eq!(read(&path), original);
        assert!(g.path().join("resourcepacks/Faithful.zip").is_file());
        drop(reservation);
        set_active(kind, g.path(), "1.21.4", "Other.zip", true).unwrap();
    }

    #[test]
    fn discover_activates_the_whole_pack_stack_or_leaves_options_untouched() {
        let g = game();
        let options = g.path().join(OPTIONS);
        let original = "resourcePacks:[\"vanilla\"]\n";
        std::fs::write(&options, original).unwrap();
        let mut installed = InstalledPack {
            file_name: "Other.zip".into(),
            used_fallback: false,
            kind: PackKind::Resource,
            dependencies: vec!["Faithful.zip".into(), "Missing.zip".into()],
        };
        assert!(installed.activate(g.path(), "1.21.4").is_err());
        assert_eq!(read(&options), original);
        installed.dependencies.pop();
        installed.activate(g.path(), "1.21.4").unwrap();
        assert_eq!(
            read(&options),
            "resourcePacks:[\"vanilla\",\"file/Faithful.zip\",\"file/Other.zip\"]\n"
        );
    }

    #[test]
    fn one_shader_at_a_time_through_iris() {
        let g = game();
        set_active(PackKind::Shader, g.path(), "1.21.4", "BSL v8.2.zip", true).unwrap();
        let props = read(&g.path().join(IRIS_CONFIG));
        assert!(props.contains("shaderPack=BSL v8.2.zip") && props.contains("enableShaders=true"));
        assert!(list(PackKind::Shader, g.path(), "1.21.4")[0].active);
        set_active(PackKind::Shader, g.path(), "1.21.4", "BSL v8.2.zip", false).unwrap();
        assert!(!list(PackKind::Shader, g.path(), "1.21.4")[0].active);
    }

    #[test]
    fn refuses_paths_and_missing_packs() {
        let g = game();
        assert!(
            set_active(
                PackKind::Resource,
                g.path(),
                "1.21.4",
                "../options.txt",
                true
            )
            .is_err()
        );
        assert!(remove(PackKind::Resource, g.path(), "1.21.4", "nope.zip").is_err());
        remove(PackKind::Resource, g.path(), "1.21.4", "Other.zip").unwrap();
        assert_eq!(list(PackKind::Resource, g.path(), "1.21.4").len(), 1);
    }
}
