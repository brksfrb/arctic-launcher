//! Reconcile the Performance set against the dependencies the loader will see.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;

use super::{ModIndex, clear_pending, index_path, record_pending};
use crate::error::IoContext;
use crate::loaders::LoaderKind;
use crate::migrate::{deps, detect};
use crate::mods::{self, resolve::Planned};
use crate::{Error, Progress, Result};

pub(super) const FABRIC_API: &str = "P7dR8mSH";

fn project_for(id: &str) -> &str {
    match id {
        "fabric" | "fabric-api" => FABRIC_API,
        "cloth-config2" => "cloth-config",
        _ => id,
    }
}

/// Requirements from enabled, indexed jars that no enabled jar provides.
/// Use file names for ownership even when a nested jar declared the dependency.
fn missing(mods_dir: &Path, index: &ModIndex, loader: LoaderKind) -> BTreeMap<String, Vec<String>> {
    let (managed, others): (Vec<_>, Vec<_>) = deps::jars(mods_dir).partition(|path| {
        path.file_name()
            .and_then(|f| f.to_str())
            .is_some_and(|file| index.by_file(file).is_some())
    });
    let mut provided = HashSet::new();
    let mut required = BTreeMap::new();
    for path in managed {
        let (ids, needs) = deps::fabric_ids(&path, loader);
        provided.extend(ids);
        if let Some(file) = path.file_name().and_then(|f| f.to_str())
            && !needs.is_empty()
        {
            required.insert(file.to_owned(), needs);
        }
    }
    // Player jars matter only for requirements the managed set does not
    // already cover. In a large pack, stop as soon as we've found them.
    let mut unmet: HashSet<_> = required
        .values()
        .flatten()
        .filter(|id| !provided.contains(*id))
        .collect();
    for path in others {
        if unmet.is_empty() {
            return BTreeMap::new();
        }
        for id in deps::fabric_ids(&path, loader).0 {
            unmet.remove(&id);
        }
    }
    required
        .iter()
        .filter_map(|(file, needs)| {
            let mut absent: Vec<_> = needs
                .iter()
                .filter(|id| unmet.contains(id))
                .cloned()
                .collect();
            absent.sort_unstable();
            (!absent.is_empty()).then(|| (file.clone(), absent))
        })
        .collect()
}

pub(super) fn check(
    game_dir: &Path,
    game_version: &str,
    loader: LoaderKind,
    progress: Progress,
) -> Result<()> {
    let index_file = index_path(game_dir);
    let mods_dir = game_dir.join("mods");
    reconcile(&mods_dir, &index_file, game_version, loader, |id| {
        install_missing(id, game_version, loader, game_dir, progress)
    })
}

fn reconcile(
    mods_dir: &Path,
    index_file: &Path,
    game_version: &str,
    loader: LoaderKind,
    mut fetch: impl FnMut(&str) -> Result<()>,
) -> Result<()> {
    let mut index = ModIndex::load(index_file)?;
    if index.mods.is_empty() || missing(mods_dir, &index, loader).is_empty() {
        return Ok(());
    }

    // Check the full API before fetching or changing anything. The launch path
    // applies the slim API again after the Performance set has been checked.
    mods::slim_api::undo(mods_dir)?;
    let mut attempted = HashSet::new();
    loop {
        let absent = missing(mods_dir, &index, loader);
        if absent.is_empty() {
            return Ok(());
        }

        let mut installed = false;
        for id in absent.values().flatten() {
            if attempted.insert(id.clone()) {
                match fetch(id) {
                    Ok(()) => {
                        installed = true;
                        break;
                    }
                    Err(e) => {
                        log::warn!("performance mods: couldn't add {id} for {game_version}: {e}")
                    }
                }
            }
        }
        if installed {
            index = ModIndex::load(index_file)?;
            continue;
        }

        // Check again after removing these jars: other managed mods may have
        // depended on one of them. Player jars are never changed here.
        for (file, ids) in absent {
            log::warn!(
                "{file} skipped for {game_version}: missing {}",
                ids.join(", ")
            );
            mods::files::delete(mods_dir, &file)?;
            index = index.without_file(&file);
        }
        index.save(index_file)?;
    }
}

fn install_missing(
    id: &str,
    game_version: &str,
    loader: LoaderKind,
    game_dir: &Path,
    progress: Progress,
) -> Result<()> {
    let project = project_for(id);
    let mods_dir = game_dir.join("mods");
    let current = ModIndex::load(&index_path(game_dir))?;
    let existing = mods::installed_projects(&current, &mods_dir);
    let (mut plan, mut failed) =
        mods::plan_many(&[project], game_version, LoaderKind::Fabric, &existing);
    if let Some((_, error)) = failed.pop() {
        return Err(error);
    }
    // An enabled copy that could not satisfy the requirement, or a disabled
    // copy, should not be replaced by a dependency repair.
    if plan
        .iter()
        .any(|p| existing.contains(&p.version.project_id))
    {
        return Err(Error::Other(format!("{project} is already installed")));
    }
    for p in &mut plan {
        p.dependency = true;
    }
    let present: HashSet<_> = deps::jars(&mods_dir)
        .flat_map(|p| deps::fabric_ids(&p, loader).0)
        .collect();
    install_plan(&plan, &current, game_dir, progress, |staging| {
        let providers: Vec<_> = deps::jars(staging).collect();
        if !providers
            .iter()
            .any(|path| deps::fabric_ids(path, loader).0.contains(id))
        {
            return Err(Error::Other(format!(
                "project {project} does not provide {id}"
            )));
        }
        // Resolving a mod id as a slug can find another build of an existing
        // mod, including one the player installed by hand.
        for jar in providers {
            if let Some((mod_id, _)) = detect::mod_identity(&jar)
                && present.contains(&mod_id)
            {
                return Err(Error::Other(format!("{mod_id} is already provided")));
            }
        }
        Ok(())
    })?;
    Ok(())
}

/// Download a batch outside the loader's view and validate it before
/// publishing any files. A failed CDN request can otherwise leave unindexed
/// jars in the live mods directory.
pub(super) fn install_plan(
    plan: &[Planned],
    current: &ModIndex,
    game_dir: &Path,
    progress: Progress,
    validate: impl FnOnce(&Path) -> Result<()>,
) -> Result<Vec<mods::InstalledMod>> {
    if plan.is_empty() {
        return Ok(Vec::new());
    }
    let mods_dir = game_dir.join("mods");
    fs::create_dir_all(&mods_dir).at(&mods_dir)?;
    let staging = tempfile::Builder::new()
        .prefix(".arctic-performance-")
        .tempdir_in(&mods_dir)
        .at(&mods_dir)?;
    let (entries, jobs) = mods::prepare(plan, &mods_dir)?;
    let mut names = HashSet::new();
    let mut downloads = Vec::new();
    for (entry, mut job) in entries.iter().zip(jobs) {
        if !names.insert(&entry.file_name)
            || (mods::files::exists(&mods_dir, &entry.file_name)
                && current
                    .by_file(&entry.file_name)
                    .is_none_or(|m| m.project_id != entry.project_id))
        {
            return Err(Error::Other(format!(
                "mod file already in use: {}",
                entry.file_name
            )));
        }
        if !job.is_satisfied() {
            job.dest = staging.path().join(&entry.file_name);
            downloads.push(job);
        }
    }
    let entries = mods::with_project_info(entries);
    crate::net::download_all("Downloading mods", downloads, progress)?;
    validate(staging.path())?;

    record_pending(game_dir, &entries, current)?;
    let result = (|| {
        for entry in &entries {
            let from = staging.path().join(&entry.file_name);
            if from.is_file() {
                let to = mods_dir.join(&entry.file_name);
                fs::rename(&from, &to).at(&to)?;
            }
        }
        mods::record(current, &entries, &mods_dir)?.save(&index_path(game_dir))
    })();
    clear_pending(game_dir)?;
    result?;
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mods::InstalledMod;
    use serde_json::{Value, json};
    use std::io::Write;

    fn jar(path: &Path, metadata: Value) {
        let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
        zip.start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(metadata.to_string().as_bytes()).unwrap();
        zip.finish().unwrap();
    }

    fn entry(project: &str, file: &str, dependency: bool) -> InstalledMod {
        InstalledMod {
            project_id: project.into(),
            version_id: "v".into(),
            title: project.into(),
            version_number: "1".into(),
            file_name: file.into(),
            icon_url: None,
            dependency,
        }
    }

    #[test]
    fn repairs_entityculling_without_a_modrinth_dependency() {
        let game = tempfile::tempdir().unwrap();
        let mods = game.path().join("mods");
        fs::create_dir(&mods).unwrap();
        let index = index_path(game.path());
        let current = ModIndex::default().with(entry("NNAgCjsB", "culling.jar", false));
        current.save(&index).unwrap();
        jar(
            &mods.join("culling.jar"),
            json!({"id": "entityculling", "version": "1.5.2", "depends": {
                "minecraft": "1.19", "fabricloader": "*", "fabric": "*"
            }, "recommends": {"optional_lib": "*"}}),
        );
        // The player's disabled API doesn't provide anything to the loader.
        jar(&mods.join("old-api.jar.disabled"), json!({"id": "fabric"}));

        reconcile(&mods, &index, "1.19", LoaderKind::Fabric, |id| {
            assert_eq!(id, "fabric");
            assert_eq!(project_for(id), FABRIC_API);
            let file = "fabric-api-0.58.0.jar";
            jar(
                &mods.join(file),
                json!({"id": "fabric", "version": "0.58.0"}),
            );
            current.with(entry(FABRIC_API, file, true)).save(&index)
        })
        .unwrap();
        assert!(mods.join("culling.jar").is_file());
        assert!(mods.join("old-api.jar.disabled").is_file());
        assert!(
            ModIndex::load(&index)
                .unwrap()
                .by_project(FABRIC_API)
                .unwrap()
                .dependency
        );
        reconcile(&mods, &index, "1.19", LoaderKind::Fabric, |_| {
            panic!("the cached set already has Fabric API")
        })
        .unwrap();
    }

    #[test]
    fn skips_unrepairable_managed_jars_and_their_dependents() {
        let game = tempfile::tempdir().unwrap();
        let mods = game.path().join("mods");
        fs::create_dir(&mods).unwrap();
        let index = index_path(game.path());
        ModIndex::default()
            .with(entry("NNAgCjsB", "culling.jar", false))
            .with(entry("extra", "extra.jar", false))
            .save(&index)
            .unwrap();
        jar(
            &mods.join("culling.jar"),
            json!({"id": "entityculling", "depends": {"fabric": "*"}}),
        );
        jar(
            &mods.join("extra.jar"),
            json!({"id": "extra", "depends": {"entityculling": "*"}}),
        );
        jar(
            &mods.join("player.jar"),
            json!({"id": "player", "depends": {"fabric": "*"}}),
        );
        let mut attempted = Vec::new();
        reconcile(&mods, &index, "1.19", LoaderKind::Quilt, |id| {
            attempted.push(id.to_owned());
            Err(Error::Http("offline".into()))
        })
        .unwrap();
        assert_eq!(attempted, ["fabric", "entityculling"]);
        assert!(!mods.join("culling.jar").exists());
        assert!(!mods.join("extra.jar").exists());
        assert!(mods.join("player.jar").is_file());
        assert!(ModIndex::load(&index).unwrap().mods.is_empty());
    }
}
