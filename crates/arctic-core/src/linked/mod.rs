//! Linked instances: an instance shared with friends by a short code, kept in
//! sync with the owner's copy. Only the mod list (exact Modrinth files,
//! hash-checked) and config files travel; worlds, game settings, key
//! bindings, the server list, accounts and anything else personal never do.
//!
//! The owner gets a code (and a key kept on this PC that lets only them
//! publish). A friend enters the code to make a linked copy. Every change
//! (the first install included) is shown as a diff the friend accepts item by
//! item; updates the owner publishes wait as a pending update until the
//! friend reviews them. Unlinking keeps the instance as it is, as a normal one.
//!
//! The format and the server API are described in `docs/linked-instances.md`.

pub mod apply;
pub mod diff;
pub mod manifest;
pub mod paths;
pub mod server;
#[cfg(test)]
mod tests;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub use apply::{Download, Report};
pub use diff::{ConfigChange, Diff, Item, LocalMod, ModChange, TargetChange};
pub use manifest::{Built, ConfigFile, LeftOut, LeftOutReason, LinkedMod, Manifest};

use crate::instances::{self, Instance, Loader};
use crate::storage::{DataDirs, load_json, save_json};
use crate::{Error, Progress, ProgressInfo, Result};

/// Next to `instance.json`.
const LINK_FILE: &str = "link.json";

/// Which side of a link this PC is on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum Role {
    /// Made the link: may publish. The key is encrypted on disk.
    Owner {
        #[serde(with = "crate::secret::on_disk")]
        key: String,
        #[serde(default = "yes")]
        include_configs: bool,
    },
    /// A linked copy of someone's instance.
    Follower,
}

fn yes() -> bool {
    true
}

/// `instances/<id>/link.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkState {
    /// Clean code (`abcdefgh`).
    pub code: String,
    #[serde(flatten)]
    pub role: Role,
    /// Owner: last published. Follower: last taken.
    pub revision: u64,
    /// The manifest of that revision (follower: as offered, whatever was accepted of it).
    #[serde(default)]
    pub manifest: Option<Manifest>,
    /// Follower: the newest revision seen on the server.
    #[serde(default)]
    pub latest: u64,
    /// Follower: a revision the player put off ("Not now"); no reminder until a newer one.
    #[serde(default)]
    pub dismissed: u64,
    /// The owner stopped sharing (the code no longer works).
    #[serde(default)]
    pub gone: bool,
}

impl LinkState {
    pub fn is_owner(&self) -> bool {
        matches!(self.role, Role::Owner { .. })
    }

    /// A newer revision waits for review.
    pub fn has_update(&self) -> bool {
        !self.is_owner() && self.latest > self.revision
    }

    /// [`Self::has_update`] and not put off.
    pub fn reminds(&self) -> bool {
        self.has_update() && self.latest > self.dismissed
    }

    pub fn pretty_code(&self) -> String {
        crate::sharing::codes::pretty(&self.code)
    }
}

/// The link of an instance, if it has one.
pub fn load(dirs: &DataDirs, instance_id: &str) -> Option<LinkState> {
    let path = dirs.instance_dir(instance_id).join(LINK_FILE);
    match load_json(&path) {
        Ok(state) => state,
        Err(e) => {
            log::warn!("ignoring unreadable {}: {e}", path.display());
            None
        }
    }
}

pub fn save(dirs: &DataDirs, instance_id: &str, state: &LinkState) -> Result<()> {
    save_json(&dirs.instance_dir(instance_id).join(LINK_FILE), state)
}

/// The links of these instances, by instance id.
pub fn all(dirs: &DataDirs, ids: &[&str]) -> HashMap<String, LinkState> {
    ids.iter()
        .filter_map(|id| load(dirs, id).map(|s| ((*id).to_owned(), s)))
        .collect()
}

/// Make it a normal instance again. Its files stay exactly as they are; only the link is
/// forgotten (an owner's code keeps working until [`stop_sharing`]).
pub fn unlink(dirs: &DataDirs, instance_id: &str) -> Result<()> {
    let path = dirs.instance_dir(instance_id).join(LINK_FILE);
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(Error::io(path, e)),
    }
}

// ------------------------------------------------------------------ owner

/// The manifest of an instance now, checked against Modrinth (jars added by hand are looked up
/// by hash first, so the Modrinth ones among them count). Network.
pub fn build_checked(dirs: &DataDirs, instance: &Instance, include_configs: bool) -> Result<Built> {
    let version = instance
        .version
        .as_deref()
        .ok_or_else(|| Error::Other("pick a Minecraft version for this instance first".into()))?;
    let game_dir = instance.game_dir(dirs);
    let index = crate::mods::index_path(&dirs.instance_dir(&instance.id));
    if let Some(kind) = instance.loader.kind()
        && game_dir.join("mods").is_dir()
        && let Err(e) = crate::mods::recognize(&game_dir.join("mods"), &index, version, kind)
    {
        log::info!("linked: could not look up hand-added mods: {e}");
    }
    let built = manifest::build(&manifest::Source {
        name: &instance.name,
        game_version: version,
        loader: &instance.loader,
        game_dir: &game_dir,
        index: &index,
        include_configs,
    })?;
    let ids: Vec<&str> = built
        .manifest
        .mods
        .iter()
        .map(|m| m.version.as_str())
        .collect();
    let versions = crate::mods::remote::versions_by_id(&ids)?;
    let built = manifest::verify(built, &versions);
    built.manifest.clone().check()?;
    Ok(built)
}

/// Share an instance: store its manifest under a new code. `token`: an Arctic session, if any.
pub fn share(
    dirs: &DataDirs,
    base: &str,
    token: Option<&str>,
    instance: &Instance,
    include_configs: bool,
) -> Result<(LinkState, Built)> {
    if load(dirs, &instance.id).is_some_and(|s| !s.is_owner()) {
        return Err(Error::Other(
            "this is a linked copy of someone else's instance; unlink it first to share it".into(),
        ));
    }
    let built = build_checked(dirs, instance, include_configs)?;
    let created = server::create(base, token, &built.manifest)?;
    let state = LinkState {
        code: created.code,
        role: Role::Owner {
            key: created.key,
            include_configs,
        },
        revision: created.revision,
        manifest: Some(built.manifest.clone()),
        latest: created.revision,
        dismissed: 0,
        gone: false,
    };
    save(dirs, &instance.id, &state)?;
    Ok((state, built))
}

/// Publish the owner's instance as it is now.
pub fn publish(
    dirs: &DataDirs,
    base: &str,
    instance: &Instance,
    include_configs: bool,
) -> Result<(LinkState, Built)> {
    let mut state = load(dirs, &instance.id)
        .filter(LinkState::is_owner)
        .ok_or_else(|| Error::Other("this instance isn't shared from this PC".into()))?;
    let Role::Owner { key, .. } = &state.role else {
        unreachable!("checked above");
    };
    let key = key.clone();
    if key.is_empty() {
        return Err(Error::Other(
            "the key of this link can't be read on this PC (it was made on another one)".into(),
        ));
    }
    let built = build_checked(dirs, instance, include_configs)?;
    let revision = server::publish(base, &state.code, &key, state.revision, &built.manifest)?;
    state.revision = revision;
    state.latest = revision;
    state.manifest = Some(built.manifest.clone());
    state.role = Role::Owner {
        key,
        include_configs,
    };
    save(dirs, &instance.id, &state)?;
    Ok((state, built))
}

/// Stop sharing: delete the link on the server and make the instance a normal one.
pub fn stop_sharing(dirs: &DataDirs, base: &str, instance_id: &str) -> Result<()> {
    if let Some(LinkState {
        code,
        role: Role::Owner { key, .. },
        ..
    }) = load(dirs, instance_id)
        && !key.is_empty()
    {
        server::delete(base, &code, &key)?;
    }
    unlink(dirs, instance_id)
}

/// What changed between two manifests, in words for the owner's "publish" list.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Changes {
    pub target: Option<(String, String)>,
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub updated: Vec<String>,
    pub configs: Vec<String>,
}

impl Changes {
    pub fn is_empty(&self) -> bool {
        self.target.is_none()
            && self.added.is_empty()
            && self.removed.is_empty()
            && self.updated.is_empty()
            && self.configs.is_empty()
    }
}

pub fn changes(old: &Manifest, new: &Manifest) -> Changes {
    let label = |m: &Manifest| format!("{} {}", m.loader.label(), m.game_version);
    let mut out = Changes {
        target: (old.game_version != new.game_version || old.loader != new.loader)
            .then(|| (label(old), label(new))),
        ..Changes::default()
    };
    for m in &new.mods {
        match old.mods.iter().find(|o| o.project == m.project) {
            None => out.added.push(m.title.clone()),
            Some(o) if o.version != m.version || o.sha512 != m.sha512 => {
                out.updated.push(m.title.clone())
            }
            Some(_) => {}
        }
    }
    for o in &old.mods {
        if !new.mods.iter().any(|m| m.project == o.project) {
            out.removed.push(o.title.clone());
        }
    }
    for c in &new.configs {
        if !old
            .configs
            .iter()
            .any(|o| o.path == c.path && o.sha256 == c.sha256)
        {
            out.configs.push(c.path.clone());
        }
    }
    for o in &old.configs {
        if !new.configs.iter().any(|c| c.path == o.path) {
            out.configs.push(o.path.clone());
        }
    }
    out
}

// --------------------------------------------------------------- follower

/// Ask the server whether the owner published something newer; saves what it finds.
pub fn check(dirs: &DataDirs, base: &str, instance_id: &str) -> Result<LinkState> {
    let mut state =
        load(dirs, instance_id).ok_or_else(|| Error::Other("this instance isn't linked".into()))?;
    if state.is_owner() {
        return Ok(state);
    }
    match server::head(base, &state.code) {
        Ok(revision) => {
            state.latest = revision;
            state.gone = false;
        }
        Err(e) if e.to_string().contains("no link has that code") => state.gone = true,
        Err(e) => return Err(e),
    }
    save(dirs, instance_id, &state)?;
    Ok(state)
}

/// The owner's newest manifest and what taking it would change here.
pub fn review(dirs: &DataDirs, base: &str, instance: &Instance) -> Result<(server::Fetched, Diff)> {
    let state = load(dirs, &instance.id)
        .filter(|s| !s.is_owner())
        .ok_or_else(|| Error::Other("this instance isn't a linked copy".into()))?;
    let fetched = server::fetch(base, &state.code)?;
    let diff = diff::compute(
        &fetched.manifest,
        fetched.revision,
        state.manifest.as_ref(),
        instance.version.as_deref(),
        &instance.loader,
        &instance.game_dir(dirs),
        &crate::mods::index_path(&dirs.instance_dir(&instance.id)),
    )?;
    Ok((fetched, diff))
}

/// A friend's link, fetched by its code, for a new linked copy: the diff is everything in it.
pub fn preview(base: &str, code: &str) -> Result<(server::Fetched, Diff)> {
    let fetched = server::fetch(base, code)?;
    let diff = fresh_diff(&fetched.manifest, fetched.revision);
    Ok((fetched, diff))
}

/// Taking `manifest` into an empty instance.
pub fn fresh_diff(manifest: &Manifest, revision: u64) -> Diff {
    let mut diff = diff::additions(manifest.mods.clone());
    diff.revision = revision;
    diff.configs = manifest
        .configs
        .iter()
        .map(|c| Item {
            change: ConfigChange::Add(c.clone()),
            accept: true,
        })
        .collect();
    diff
}

/// Downloads with progress (the real fetcher for [`apply::apply`]).
pub fn downloader(
    progress: Progress<'_>,
) -> impl Fn(Vec<crate::net::DownloadJob>) -> Result<()> + '_ {
    move |jobs| crate::net::download_all("Downloading mods", jobs, progress)
}

/// Apply the accepted part of `diff` (from [`review`]) to a linked copy; the link then stands at
/// the fetched revision. All or nothing.
pub fn take(
    dirs: &DataDirs,
    instance: &Instance,
    fetched: &server::Fetched,
    diff: &Diff,
    progress: Progress,
) -> Result<Report> {
    let mut state = load(dirs, &instance.id)
        .filter(|s| !s.is_owner())
        .ok_or_else(|| Error::Other("this instance isn't a linked copy".into()))?;
    let selected = diff.selected();
    progress(ProgressInfo::stage("Looking up mods"));
    let downloads = apply::plan_from_modrinth(&selected)?;
    let game_dir = instance.game_dir(dirs);
    let index = crate::mods::index_path(&dirs.instance_dir(&instance.id));
    let mut updated = instance.clone();
    if let Some(t) = &selected.target {
        updated.version = Some(t.change.to_version.clone());
        updated.loader = arctic_ready(dirs, &t.change.to_loader, &t.change.to_version);
    }
    state.revision = fetched.revision;
    state.latest = state.latest.max(fetched.revision);
    state.manifest = Some(fetched.manifest.clone());
    let fetch = downloader(progress);
    let instance_before = instance.clone();
    apply::apply(
        &apply::Target {
            game_dir: &game_dir,
            index: &index,
        },
        &selected,
        &downloads,
        &fetch,
        &mut || {
            if selected.target.is_some() {
                updated.save(dirs)?;
            }
            if let Err(e) = save(dirs, &instance.id, &state) {
                // Leave instance.json as it was too.
                let _ = instance_before.save(dirs);
                return Err(e);
            }
            Ok(())
        },
    )
}

/// Make a new linked copy from a previewed link and apply the accepted part of its diff. A
/// failure removes the half-made instance again.
pub fn follow(
    dirs: &DataDirs,
    code: &str,
    fetched: &server::Fetched,
    diff: &Diff,
    progress: Progress,
) -> Result<(Instance, Report)> {
    let code = crate::sharing::codes::clean(code)
        .ok_or_else(|| Error::Other("that's not a link code".into()))?;
    let m = &fetched.manifest;
    let selected = diff.selected();
    progress(ProgressInfo::stage("Looking up mods"));
    let downloads = apply::plan_from_modrinth(&selected)?;
    let name = instances::free_name(dirs, &m.name)?;
    let created = instances::create(
        dirs,
        &name,
        &m.game_version,
        arctic_ready(dirs, &m.loader, &m.game_version),
    )?;
    // Its own settings: the profile's shared ones would mix with the owner's configs.
    let instance = Instance {
        own_settings: true,
        ..created
    };
    let state = LinkState {
        code,
        role: Role::Follower,
        revision: fetched.revision,
        manifest: Some(m.clone()),
        latest: fetched.revision,
        dismissed: 0,
        gone: false,
    };
    let game_dir = instance.game_dir(dirs);
    let index = crate::mods::index_path(&dirs.instance_dir(&instance.id));
    let fetch = downloader(progress);
    let result = apply::apply(
        &apply::Target {
            game_dir: &game_dir,
            index: &index,
        },
        &selected,
        &downloads,
        &fetch,
        &mut || {
            instance.save(dirs)?;
            save(dirs, &instance.id, &state)
        },
    );
    match result {
        Ok(report) => Ok((instance, report)),
        Err(e) => {
            let _ = instances::remove(dirs, &instance.id);
            Err(e)
        }
    }
}

/// Put off a pending update (no reminder until the owner publishes again).
pub fn dismiss(dirs: &DataDirs, instance_id: &str) -> Result<()> {
    let mut state =
        load(dirs, instance_id).ok_or_else(|| Error::Other("this instance isn't linked".into()))?;
    state.dismissed = state.latest;
    save(dirs, instance_id, &state)
}

/// A Fabric Loader older than the Arctic Client needs gets the one it needs.
fn arctic_ready(dirs: &DataDirs, loader: &Loader, game: &str) -> Loader {
    match loader {
        Loader::Fabric { version } if !crate::arctic_mod::loader_fits(game, version) => {
            crate::arctic_mod::client_loader(dirs, game)
                .filter(|v| crate::arctic_mod::loader_fits(game, v))
                .map_or_else(|| loader.clone(), |version| Loader::Fabric { version })
        }
        _ => loader.clone(),
    }
}

// ----------------------------------------------------------- missing mods

/// A mod a server needs, as the game reported it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct WantedMod {
    pub id: String,
    #[serde(default)]
    pub version: Option<String>,
}

/// What looking up a server's missing mods found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MissingLookup {
    /// Everything found on Modrinth, as a diff of mods to add.
    pub diff: Option<Diff>,
    /// Ids Modrinth has nothing for (for this version and loader).
    pub not_found: Vec<String>,
    /// Ids already in the instance.
    pub installed: Vec<String>,
}

/// Look up the mods a server said were missing, for `instance`. Nothing is installed: the
/// result goes to the diff screen like a link update.
pub fn find_missing(
    dirs: &DataDirs,
    instance: &Instance,
    wanted: &[WantedMod],
) -> Result<MissingLookup> {
    let version = instance
        .version
        .as_deref()
        .ok_or_else(|| Error::Other("this instance follows the Play tab's version".into()))?;
    let loader = instance.loader.kind().ok_or_else(|| {
        Error::Other("this instance has no mod loader, so it can't take mods".into())
    })?;
    let game_dir = instance.game_dir(dirs);
    let index = crate::mods::index_path(&dirs.instance_dir(&instance.id));
    let local = diff::local_mods(&game_dir, &index)?;
    let mut out = MissingLookup::default();
    let mut adds: Vec<LinkedMod> = Vec::new();
    for w in wanted.iter().take(64) {
        let Some(id) = crate::mods::remote::clean_mod_id(&w.id) else {
            continue;
        };
        let found =
            crate::mods::remote::find_for_mod_id(&id, w.version.as_deref(), version, loader)?;
        let Some((title, file)) = found else {
            out.not_found.push(id);
            continue;
        };
        if local.iter().any(|l| l.project == file.project_id) {
            out.installed.push(title);
            continue;
        }
        let (Some(sha1), Some(sha512), Some(size)) = (file.sha1, file.sha512, file.size) else {
            out.not_found.push(id);
            continue;
        };
        if adds.iter().any(|a| a.project == file.project_id) {
            continue;
        }
        adds.push(LinkedMod {
            project: file.project_id,
            version: file.version_id,
            title,
            file: file.file_name,
            size,
            sha1: sha1.to_ascii_lowercase(),
            sha512: sha512.to_ascii_lowercase(),
            enabled: true,
            dependency: false,
        });
    }
    if !adds.is_empty() {
        out.diff = Some(diff::additions(adds));
    }
    Ok(out)
}

/// Install the accepted mods of a missing-mods diff (adds only, so the game may be running).
pub fn install_missing(
    dirs: &DataDirs,
    instance: &Instance,
    diff: &Diff,
    progress: Progress,
) -> Result<Report> {
    let selected = diff.selected();
    if selected
        .mods
        .iter()
        .any(|m| !matches!(m.change, ModChange::Add(_)))
        || selected.target.is_some()
        || !selected.configs.is_empty()
    {
        return Err(Error::Other(
            "only new mods can be installed this way".into(),
        ));
    }
    progress(ProgressInfo::stage("Looking up mods"));
    let downloads = apply::plan_from_modrinth(&selected)?;
    let game_dir = instance.game_dir(dirs);
    let index = crate::mods::index_path(&dirs.instance_dir(&instance.id));
    let fetch = downloader(progress);
    apply::apply(
        &apply::Target {
            game_dir: &game_dir,
            index: &index,
        },
        &selected,
        &downloads,
        &fetch,
        &mut || Ok(()),
    )
}
