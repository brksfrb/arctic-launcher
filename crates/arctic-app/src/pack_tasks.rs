//! Background jobs for resource packs and shaders.

use arctic_core::instances::Instance;
use arctic_core::mods::packs::{self, PackKind};
use arctic_core::mods::{self, SearchQuery};
use arctic_core::{Error, ProgressInfo, Result};

use crate::tasks::{Event, ProgressSnapshot, Tasks};
use crate::{t, tf};
/// A finished pack install.
#[derive(Debug, Clone)]
pub struct PackDone {
    pub file: String,
    /// What else happened ("Iris added so shaders work").
    pub note: String,
    /// The Vanilla instance's shader switch was turned on.
    pub shaders_switched_on: bool,
}

impl Tasks {
    pub fn pack_search(&self, request: u64, query: SearchQuery) {
        self.run(move |t| {
            let result = mods::search(&query).map_err(|e| e.to_string());
            t.send(Event::PackSearch(request, result));
        });
    }

    /// Install a pack and switch it on; shaders bring Iris (Oculus on Forge).
    pub fn pack_install(&self, instance: Instance, kind: PackKind, project: String, game: String) {
        self.pack_install_selected(instance, kind, project, game, None);
    }

    pub fn pack_install_version(
        &self,
        instance: Instance,
        kind: PackKind,
        project: String,
        game: String,
        version_id: String,
        allow_other_game: bool,
    ) {
        let chosen = Some((version_id, allow_other_game));
        self.pack_install_selected(instance, kind, project, game, chosen);
    }

    fn pack_install_selected(
        &self,
        instance: Instance,
        kind: PackKind,
        project: String,
        game: String,
        chosen: Option<(String, bool)>,
    ) {
        self.run(move |t| {
            let chosen = chosen.as_ref().map(|(id, allow)| (id.as_str(), *allow));
            let result = t
                .pack_install_blocking(&instance, kind, &project, &game, chosen)
                .map_err(crate::tasks::content_install_error);
            t.send(Event::PackInstalled(instance.id.clone(), project, result));
        });
    }

    fn pack_install_blocking(
        &self,
        instance: &Instance,
        kind: PackKind,
        project: &str,
        game: &str,
        chosen: Option<(&str, bool)>,
    ) -> Result<PackDone> {
        let progress = |p: ProgressInfo| self.send(Event::ModProgress(ProgressSnapshot::from(p)));
        let game_dir = instance.game_dir(self.dirs());
        // A chosen version is checked before Iris is added for it.
        let selected = match chosen {
            Some((id, allow_other_game)) => {
                progress(ProgressInfo::stage("Finding a version"));
                Some(packs::selected_version(kind, id, game, allow_other_game)?)
            }
            None => None,
        };
        let mut note = String::new();
        let mut shaders_switched_on = false;
        if kind == PackKind::Shader {
            match instance.loader.kind() {
                // The Vanilla instance gets Iris through its shader switch.
                None => {
                    shaders_switched_on = !instance.shaders;
                    if shaders_switched_on {
                        note = t!("tasks.shaders_are_on_for_this_instance_iris_comes").into();
                    }
                }
                Some(loader) => {
                    let index = mods::index_path(&self.dirs().instance_dir(&instance.id));
                    let shader_mod = packs::shader_mod(loader);
                    let have = mods::list(&game_dir.join("mods"), &index)?.iter().any(|m| {
                        m.tracked
                            .as_ref()
                            .is_some_and(|t| t.project_id == shader_mod)
                    });
                    if !have {
                        mods::install(
                            shader_mod,
                            game,
                            loader,
                            &game_dir.join("mods"),
                            &index,
                            &progress,
                        )
                        .map_err(|e| {
                            Error::Other(tf!(
                                "tasks.shaders_need_iris_which_couldnt_be_installed",
                                e = e
                            ))
                        })?;
                        note = t!("tasks.iris_was_added_so_shaders_work").into();
                    }
                }
            }
        }
        let installed = match &selected {
            Some(selected) => packs::install_selected(selected, &game_dir, &progress)?,
            None => packs::install(kind, project, game, &game_dir, &progress)?,
        };
        let file = installed.file_name;
        if let Err(e) = packs::set_active(kind, &game_dir, &file, true) {
            note = tf!("tasks.installed_but_not_switched_on", e = e);
        }
        if installed.used_fallback {
            if !note.is_empty() {
                note.push(' ');
            }
            note.push_str(if selected.is_some() {
                t!("tasks.chosen_file_is_tagged_for_another")
            } else {
                t!("tasks.installed_the_newest_file_tagged_for_another")
            });
        }
        Ok(PackDone {
            file,
            note,
            shaders_switched_on,
        })
    }
}
