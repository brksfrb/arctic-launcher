//! Background jobs for instance worlds (copies can be large).

use std::path::PathBuf;

use arctic_core::worlds::{self, World};

use crate::tasks::{Event, Tasks};
use crate::{t, tf};
impl Tasks {
    /// Copy world folders into `saves`.
    pub fn import_worlds(&self, instance_id: String, worlds: Vec<PathBuf>, saves: PathBuf) {
        self.run(move |t| {
            let mut imported = 0;
            let mut error = None;
            for world in &worlds {
                match worlds::import_folder(world, &saves) {
                    Ok(_) => imported += 1,
                    Err(e) => error = Some(e.to_string()),
                }
            }
            let result = match (imported, error) {
                (0, Some(e)) => Err(e),
                (n, _) => Ok(match n {
                    1 => t!("tasks.imported_1_world").to_owned(),
                    n => tf!("tasks.imported_worlds", n_value = n),
                }),
            };
            t.send(Event::WorldsDone(instance_id, result));
        });
    }

    /// Ask for a .zip and unpack the world in it into `saves`.
    pub fn import_world_zip(&self, instance_id: String, saves: PathBuf) {
        self.run(move |t| {
            let picked = rfd::FileDialog::new()
                .set_title(t!("tasks.choose_a_world_archive"))
                .add_filter(t!("tasks.world_archive"), &["zip"])
                .pick_file();
            t.note_if_picker_missing(&picked);
            let result = match picked {
                None => Ok(t!("tasks.no_file_chosen").to_owned()),
                Some(zip) => worlds::import_zip(&zip, &saves)
                    .map(|name| tf!("tasks.imported", name = name))
                    .map_err(|e| e.to_string()),
            };
            t.send(Event::WorldsDone(instance_id, result));
        });
    }

    /// Zip a world into the instance's backups folder.
    pub fn backup_world(&self, instance_id: String, world: World, backups: PathBuf) {
        self.run(move |t| {
            let result = worlds::backup(&world, &backups)
                .map(|path| {
                    let _ = open::that_detached(&backups);
                    tf!(
                        "tasks.backed_up_to",
                        name = world.name,
                        value = path
                            .file_name()
                            .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
                    )
                })
                .map_err(|e| e.to_string());
            t.send(Event::WorldsDone(instance_id, result));
        });
    }
}
