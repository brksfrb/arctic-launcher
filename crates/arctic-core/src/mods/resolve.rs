//! Required-dependency resolution, independent of the network.
//!
//! The resolver talks to a [`VersionSource`]; production uses Modrinth,
//! tests use an in-memory table.

use std::collections::{HashSet, VecDeque};

use super::modrinth::{self, Version};
use crate::loaders::LoaderKind;
use crate::{Error, Result};

/// Where versions come from.
pub(super) trait VersionSource {
    /// Versions of `project_id` (may be pre-filtered by the source).
    fn project_versions(&self, project_id: &str) -> Result<Vec<Version>>;
    /// One specific version.
    fn version(&self, version_id: &str) -> Result<Version>;
}

/// The live Modrinth API, filtered to one loader and game version.
pub(super) struct Modrinth<'a> {
    pub loaders: &'a [&'a str],
    pub game_version: &'a str,
}

impl VersionSource for Modrinth<'_> {
    fn project_versions(&self, project_id: &str) -> Result<Vec<Version>> {
        modrinth::project_versions(project_id, self.loaders, self.game_version)
    }

    fn version(&self, version_id: &str) -> Result<Version> {
        modrinth::version(version_id)
    }
}

/// What the instance runs.
#[derive(Debug, Clone, Copy)]
pub(super) struct Target<'a> {
    pub loaders: &'a [&'a str],
    pub game_version: &'a str,
    pub loader: LoaderKind,
}

/// A version chosen for installation.
#[derive(Debug, Clone)]
pub(super) struct Planned {
    pub version: Version,
    /// Pulled in as someone else's required dependency.
    pub dependency: bool,
}

/// A required dependency still to be looked up.
enum Pending {
    Project(String),
    Version { id: String, project: Option<String> },
}

/// Resolve `root` plus, transitively, every required dependency whose
/// project is not in `installed`. The root is always part of the plan (it
/// may be an update). Each project appears at most once, which also breaks
/// dependency cycles. Nothing is downloaded here.
pub(super) fn resolve(
    root: &str,
    target: Target,
    installed: &HashSet<String>,
    source: &dyn VersionSource,
) -> Result<Vec<Planned>> {
    let root_version = newest(root, target, source)?;
    let mut seen: HashSet<String> =
        HashSet::from([root.to_string(), root_version.project_id.clone()]);
    let mut queue: VecDeque<Pending> = VecDeque::new();
    enqueue_deps(&root_version, &mut queue);
    let mut plan = vec![Planned {
        version: root_version,
        dependency: false,
    }];

    while let Some(pending) = queue.pop_front() {
        let Some(version) = lookup(pending, target, installed, &seen, source)? else {
            continue;
        };
        if installed.contains(&version.project_id) || !seen.insert(version.project_id.clone()) {
            continue;
        }
        enqueue_deps(&version, &mut queue);
        plan.push(Planned {
            version,
            dependency: true,
        });
    }
    Ok(plan)
}

/// Fetch the version a pending dependency points at, or `None` when its
/// project is already handled.
fn lookup(
    pending: Pending,
    target: Target,
    installed: &HashSet<String>,
    seen: &HashSet<String>,
    source: &dyn VersionSource,
) -> Result<Option<Version>> {
    let skip = |project: &str| installed.contains(project) || seen.contains(project);
    match pending {
        Pending::Project(project) if skip(&project) => Ok(None),
        Pending::Project(project) => newest(&project, target, source).map(Some),
        Pending::Version {
            project: Some(p), ..
        } if skip(&p) => Ok(None),
        Pending::Version { id, .. } => {
            let pinned = source.version(&id)?;
            if skip(&pinned.project_id) {
                return Ok(None);
            }
            // Pins are sometimes stale; fall back to the newest version that
            // actually runs on this instance.
            if pinned.is_compatible(target.loaders, target.game_version)
                && pinned.primary_file().is_some()
            {
                Ok(Some(pinned))
            } else {
                newest(&pinned.project_id, target, source).map(Some)
            }
        }
    }
}

fn enqueue_deps(version: &Version, queue: &mut VecDeque<Pending>) {
    for dep in version
        .dependencies
        .iter()
        .filter(|d| d.dependency_type == "required")
    {
        match (&dep.version_id, &dep.project_id) {
            (Some(id), project) => queue.push_back(Pending::Version {
                id: id.clone(),
                project: project.clone(),
            }),
            (None, Some(project)) => queue.push_back(Pending::Project(project.clone())),
            (None, None) => log::warn!(
                "{} {} has a required dependency outside Modrinth; skipping",
                version.project_id,
                version.version_number
            ),
        }
    }
}

/// Newest compatible version of a project, or a readable error.
fn newest(project_id: &str, target: Target, source: &dyn VersionSource) -> Result<Version> {
    let versions = source.project_versions(project_id)?;
    modrinth::pick_version(&versions, target.loaders, target.game_version)
        .cloned()
        .ok_or_else(|| {
            Error::Other(format!(
                "{project_id} has no version for {} {}",
                target.loader.label(),
                target.game_version
            ))
        })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::json;

    use super::*;

    /// In-memory Modrinth: project id -> versions.
    struct Fake {
        projects: HashMap<String, Vec<Version>>,
    }

    impl Fake {
        fn new(versions: Vec<serde_json::Value>) -> Self {
            let mut projects: HashMap<String, Vec<Version>> = HashMap::new();
            for v in versions {
                let v: Version = serde_json::from_value(v).unwrap();
                projects.entry(v.project_id.clone()).or_default().push(v);
            }
            Self { projects }
        }
    }

    impl VersionSource for Fake {
        fn project_versions(&self, project_id: &str) -> Result<Vec<Version>> {
            Ok(self.projects.get(project_id).cloned().unwrap_or_default())
        }
        fn version(&self, version_id: &str) -> Result<Version> {
            self.projects
                .values()
                .flatten()
                .find(|v| v.id == version_id)
                .cloned()
                .ok_or_else(|| Error::Other(format!("no {version_id}")))
        }
    }

    fn ver(id: &str, project: &str, game: &str, deps: serde_json::Value) -> serde_json::Value {
        json!({
            "id": id, "project_id": project, "version_number": "1.0",
            "version_type": "release", "date_published": "2024-01-01T00:00:00Z",
            "game_versions": [game], "loaders": ["fabric"],
            "files": [{"url": format!("https://cdn/{id}.jar"), "filename": format!("{id}.jar"),
                       "primary": true, "size": 1, "hashes": {"sha1": "00"}}],
            "dependencies": deps,
        })
    }

    fn req_project(p: &str) -> serde_json::Value {
        json!({"project_id": p, "version_id": null, "dependency_type": "required"})
    }

    const TARGET: Target = Target {
        loaders: &["fabric"],
        game_version: "1.21.4",
        loader: LoaderKind::Fabric,
    };

    fn ids(plan: &[Planned]) -> Vec<(&str, bool)> {
        plan.iter()
            .map(|p| (p.version.id.as_str(), p.dependency))
            .collect()
    }

    #[test]
    fn resolves_required_deps_transitively_and_skips_optional() {
        let fake = Fake::new(vec![
            ver(
                "a1",
                "A",
                "1.21.4",
                json!([req_project("B"),
                {"project_id": "C", "dependency_type": "optional"}]),
            ),
            ver("b1", "B", "1.21.4", json!([req_project("D")])),
            ver("c1", "C", "1.21.4", json!([])),
            ver("d1", "D", "1.21.4", json!([])),
        ]);
        let plan = resolve("A", TARGET, &HashSet::new(), &fake).unwrap();
        assert_eq!(ids(&plan), [("a1", false), ("b1", true), ("d1", true)]);
    }

    #[test]
    fn cycles_and_already_installed_projects_are_skipped() {
        let fake = Fake::new(vec![
            ver(
                "a1",
                "A",
                "1.21.4",
                json!([req_project("B"), req_project("C")]),
            ),
            ver("b1", "B", "1.21.4", json!([req_project("A")])),
            ver("c1", "C", "1.21.4", json!([])),
        ]);
        let installed = HashSet::from(["C".to_string()]);
        let plan = resolve("A", TARGET, &installed, &fake).unwrap();
        assert_eq!(ids(&plan), [("a1", false), ("b1", true)]);
    }

    #[test]
    fn pinned_version_used_when_compatible_else_newest() {
        let fake = Fake::new(vec![
            ver(
                "a1",
                "A",
                "1.21.4",
                json!([
                {"version_id": "b-old", "project_id": "B", "dependency_type": "required"},
                {"version_id": "c-pin", "dependency_type": "required"}]),
            ),
            ver("b-old", "B", "1.20.1", json!([])),
            ver("b-new", "B", "1.21.4", json!([])),
            ver("c-pin", "C", "1.21.4", json!([])),
        ]);
        let plan = resolve("A", TARGET, &HashSet::new(), &fake).unwrap();
        assert_eq!(
            ids(&plan),
            [("a1", false), ("b-new", true), ("c-pin", true)]
        );
    }

    #[test]
    fn missing_compatible_version_is_a_readable_error() {
        let fake = Fake::new(vec![ver("a1", "A", "1.20.1", json!([]))]);
        let err = resolve("A", TARGET, &HashSet::new(), &fake).unwrap_err();
        assert_eq!(err.to_string(), "A has no version for Fabric 1.21.4");
    }
}
