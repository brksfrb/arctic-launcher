//! Resolve and install the projects selected in Discover or in an instance's browser.

use std::sync::mpsc::{self, Receiver};

use arctic_core::instances::Instance;
use arctic_core::loaders::LoaderKind;
use arctic_core::mods::discovery::{self, VersionInfo};
use arctic_core::mods::{self, ProjectHit, ProjectType, datapack, modpack};
use arctic_core::{Progress, ProgressInfo};
use eframe::egui;

use crate::t;
use crate::tasks::{Event, Outcome, Tasks, content_install_error};

pub(crate) struct ContentRequest {
    pub kind: ProjectType,
    pub hits: Vec<ProjectHit>,
    pub version: Option<String>,
    pub bulk: bool,
}

pub(crate) type ContentVersions = Vec<Outcome<Vec<VersionInfo>>>;

impl ContentRequest {
    pub fn fetch_versions(&self, ctx: &egui::Context) -> Receiver<ContentVersions> {
        let ids: Vec<_> = self.hits.iter().map(|hit| hit.project_id.clone()).collect();
        let ctx = ctx.clone();
        let (send, receive) = mpsc::channel();
        std::thread::spawn(move || {
            let versions = ids
                .chunks(6)
                .flat_map(|chunk| {
                    std::thread::scope(|scope| {
                        let requests: Vec<_> = chunk
                            .iter()
                            .map(|id| {
                                scope.spawn(move || {
                                    discovery::project_version_list(id).map_err(|e| e.to_string())
                                })
                            })
                            .collect();
                        requests
                            .into_iter()
                            .map(|request| {
                                request.join().unwrap_or_else(|_| {
                                    Err(t!("instances.project_request_stopped_please_retry").into())
                                })
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            let _ = send.send(versions);
            ctx.request_repaint();
        });
        receive
    }

    pub fn choices(
        &self,
        versions: &ContentVersions,
        target: &Instance,
        game: &str,
    ) -> Vec<Outcome<ContentChoice>> {
        self.choices_with_loader(versions, target.loader.kind(), game)
    }

    fn choices_with_loader(
        &self,
        versions: &ContentVersions,
        loader: Option<LoaderKind>,
        game: &str,
    ) -> Vec<Outcome<ContentChoice>> {
        versions
            .iter()
            .map(|versions| match versions {
                Ok(versions) => choose_version(
                    self.kind,
                    versions,
                    self.version.as_deref(),
                    loader,
                    game,
                    self.bulk,
                ),
                Err(error) => Err(error.clone()),
            })
            .collect()
    }
}

pub(crate) struct ContentChoice {
    pub version: VersionInfo,
    pub other_game: bool,
}

fn choose_version(
    kind: ProjectType,
    versions: &[VersionInfo],
    exact: Option<&str>,
    loader: Option<LoaderKind>,
    game: &str,
    bulk: bool,
) -> Outcome<ContentChoice> {
    let best = |loader, game| discovery::best_version(versions, kind, loader, game);
    let choice = |version, other_game| ContentChoice {
        version,
        other_game,
    };
    let fits = |version: &VersionInfo, game| {
        version.has_install_file(kind) && version.fits(kind, loader, game)
    };
    let allow_other_game = !bulk
        && matches!(
            kind,
            ProjectType::ResourcePack | ProjectType::Shader | ProjectType::DataPack
        );
    let selected = match exact {
        Some(id) => versions.iter().find(|v| v.id == id).and_then(|version| {
            if fits(version, game) {
                Some(choice(version.clone(), false))
            } else if allow_other_game && fits(version, "") {
                Some(choice(version.clone(), true))
            } else {
                None
            }
        }),
        None => best(loader, game).map(|v| choice(v, false)).or_else(|| {
            if allow_other_game {
                best(loader, "").map(|v| choice(v, true))
            } else if kind == ProjectType::Modpack {
                best(None, "").map(|v| choice(v, false))
            } else {
                None
            }
        }),
    };
    selected.ok_or_else(|| t!("instances.hub.no_matching_install").into())
}

pub(crate) struct ContentProgress {
    pub current: usize,
    pub total: usize,
    pub title: String,
    pub share: f32,
}

pub(crate) struct ContentReport {
    pub target: String,
    pub kind: ProjectType,
    pub bulk: bool,
    pub succeeded: usize,
    pub failed: Vec<(String, String)>,
    pub notes: Vec<String>,
    pub shaders_switched_on: bool,
}

pub(crate) struct ContentTarget {
    pub instance: Instance,
    pub game: String,
    pub world: Option<String>,
}

impl Tasks {
    pub(crate) fn install_content(
        &self,
        request: ContentRequest,
        choices: Vec<Outcome<ContentChoice>>,
        target: ContentTarget,
    ) {
        self.run(move |t| {
            let _guard = t.content_lock();
            let total = request.hits.len();
            let mut report = ContentReport {
                target: target.instance.id.clone(),
                kind: request.kind,
                bulk: request.bulk,
                succeeded: 0,
                failed: Vec::new(),
                notes: Vec::new(),
                shaders_switched_on: false,
            };
            for (index, (hit, choice)) in request.hits.into_iter().zip(choices).enumerate() {
                let progress = |p: ProgressInfo| {
                    let share = if p.bytes_total > 0 {
                        (p.bytes_done as f32 / p.bytes_total as f32).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    t.send(Event::ContentProgress(ContentProgress {
                        current: index + 1,
                        total,
                        title: hit.title.clone(),
                        share,
                    }));
                };
                progress(ProgressInfo::stage(""));
                match choice.and_then(|choice| {
                    t.install_content_item(request.kind, &hit, &choice, &target, &progress)
                }) {
                    Ok(pack) => {
                        report.succeeded += 1;
                        if let Some(pack) = pack {
                            report.shaders_switched_on |= pack.shaders_switched_on;
                            if !pack.note.is_empty() && !report.notes.contains(&pack.note) {
                                report.notes.push(pack.note);
                            }
                        }
                    }
                    Err(error) => report.failed.push((hit.title, error)),
                }
            }
            t.send(Event::ContentInstalled(report));
        });
    }

    fn install_content_item(
        &self,
        kind: ProjectType,
        hit: &ProjectHit,
        choice: &ContentChoice,
        target: &ContentTarget,
        progress: Progress,
    ) -> Outcome<Option<crate::pack_tasks::PackDone>> {
        let instance = &target.instance;
        let game = target.game.as_str();
        let version = choice.version.id.as_str();
        let game_dir = instance.game_dir(self.dirs());
        match kind {
            ProjectType::Mod => {
                let loader = instance
                    .loader
                    .kind()
                    .ok_or_else(|| t!("tasks.vanilla_instances_cant_load_mods").to_owned())?;
                let index = mods::index_path(&self.dirs().instance_dir(&instance.id));
                mods::install_version(
                    version,
                    game,
                    loader,
                    &game_dir.join("mods"),
                    &index,
                    progress,
                )
                .map(|_| None)
                .map_err(content_install_error)
            }
            ProjectType::ResourcePack | ProjectType::Shader => {
                let kind = if kind == ProjectType::Shader {
                    mods::packs::PackKind::Shader
                } else {
                    mods::packs::PackKind::Resource
                };
                self.pack_install_blocking(
                    instance,
                    kind,
                    &hit.project_id,
                    game,
                    Some((version, choice.other_game)),
                    Some(progress),
                )
                .map(Some)
                .map_err(content_install_error)
            }
            ProjectType::DataPack => {
                let world = target
                    .world
                    .as_deref()
                    .ok_or_else(|| t!("instances.hub.no_worlds").to_owned())?;
                datapack::install(version, &game_dir, world, game, choice.other_game, progress)
                    .map(|()| None)
                    .map_err(content_install_error)
            }
            ProjectType::Modpack => {
                modpack::install_version(self.dirs(), &hit.project_id, Some(version), progress)
                    .map(|_| None)
                    .map_err(content_install_error)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(id: &str, channel: &str, loader: &str, game: &str, date: &str) -> VersionInfo {
        VersionInfo {
            id: id.into(),
            number: id.into(),
            name: String::new(),
            version_type: channel.into(),
            date: date.into(),
            downloads: 0,
            game_versions: vec![game.into()],
            loaders: vec![loader.into()],
            file_name: Some("mod.jar".into()),
        }
    }

    #[test]
    fn bulk_resolution_prefers_a_stable_fitting_version_and_keeps_individual_failures() {
        let request = ContentRequest {
            kind: ProjectType::Mod,
            hits: ["a", "b", "c"]
                .map(|id| ProjectHit {
                    project_id: id.into(),
                    slug: id.into(),
                    title: id.into(),
                    description: String::new(),
                    author: String::new(),
                    downloads: 0,
                    follows: 0,
                    icon_url: None,
                    featured_gallery: None,
                    categories: vec![],
                    game_versions: vec![],
                })
                .to_vec(),
            version: None,
            bulk: true,
        };
        let versions = vec![
            Ok(vec![
                version("new-beta", "beta", "quilt", "1.21.4", "2026-10-04"),
                version("stable", "release", "fabric", "1.21.4", "2026-10-03"),
                version("older", "release", "quilt", "1.21.4", "2026-10-02"),
                version("other-game", "release", "quilt", "1.20.1", "2026-10-05"),
            ]),
            Ok(vec![version(
                "other-loader",
                "release",
                "forge",
                "1.21.4",
                "2026-10-05",
            )]),
            Err("network failed".into()),
        ];
        let choices = request.choices_with_loader(&versions, Some(LoaderKind::Quilt), "1.21.4");
        assert_eq!(choices[0].as_ref().unwrap().version.id, "stable");
        assert!(choices[1].is_err());
        assert_eq!(choices[2].as_ref().err().unwrap(), "network failed");
    }
}
