//! Background jobs for resource packs and shaders.

use arctic_core::instances::Instance;
use arctic_core::mods::packs::{self, PackKind};
use arctic_core::mods::{self, SearchQuery};
use arctic_core::{Error, ProgressInfo, Result};

use crate::tasks::{Event, ProgressSnapshot, Tasks};

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
        self.run(move |t| {
            let result = t
                .pack_install_blocking(&instance, kind, &project, &game)
                .map_err(|e| e.to_string());
            t.send(Event::PackInstalled(instance.id.clone(), project, result));
        });
    }

    fn pack_install_blocking(
        &self,
        instance: &Instance,
        kind: PackKind,
        project: &str,
        game: &str,
    ) -> Result<PackDone> {
        let progress = |p: ProgressInfo| self.send(Event::ModProgress(ProgressSnapshot::from(p)));
        let game_dir = instance.game_dir(self.dirs());
        let mut note = String::new();
        let mut shaders_switched_on = false;
        if kind == PackKind::Shader {
            match instance.loader.kind() {
                // The Vanilla instance gets Iris through its shader switch.
                None => {
                    shaders_switched_on = !instance.shaders;
                    if shaders_switched_on {
                        note = "Shaders are on for this instance; Iris comes with the next start."
                            .into();
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
                            Error::Other(format!(
                                "shaders need Iris, which couldn't be installed: {e}"
                            ))
                        })?;
                        note = "Iris was added so shaders work.".into();
                    }
                }
            }
        }
        let file = packs::install(kind, project, game, &game_dir, &progress)?;
        if let Err(e) = packs::set_active(kind, &game_dir, &file, true) {
            note = format!("Installed but not switched on: {e}");
        }
        Ok(PackDone {
            file,
            note,
            shaders_switched_on,
        })
    }
}
