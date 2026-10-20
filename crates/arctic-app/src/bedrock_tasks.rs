//! Background jobs for the Bedrock tab: finding the game and its data,
//! the version list, installing a version, worlds, packs and servers.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use arctic_core::bedrock::packs::{self, Pack};
use arctic_core::bedrock::servers::{self, BedrockStatus, Server};
use arctic_core::bedrock::versions::{self, Catalog, Version};
use arctic_core::bedrock::worlds::{self, World};
use arctic_core::bedrock::{self, Edition, Folders, Install, Player};
use arctic_core::{ProgressInfo, Result};

use crate::tasks::{Event, Outcome, ProgressSnapshot, Tasks};

/// What's on this computer for one edition.
#[derive(Debug, Clone, Default)]
pub struct BedrockScan {
    pub install: Option<Install>,
    pub running: bool,
    /// Gaming Services is installed (needed to install a version).
    pub gaming_services: bool,
    pub folders: Folders,
}

/// A Bedrock job's result, for the Bedrock tab.
pub enum BedrockEvent {
    Scanned(Edition, BedrockScan),
    Catalog(Outcome<Catalog>),
    InstallProgress(ProgressSnapshot),
    /// A version install finished (edition, version, result).
    Installed(Edition, String, Outcome<()>),
    Worlds(PathBuf, Vec<World>),
    Packs(PathBuf, Vec<Pack>),
    Servers(PathBuf, Vec<Server>),
    /// (address, status)
    ServerStatus(String, Outcome<BedrockStatus>),
    /// A world, pack or server job finished (what to say).
    Done(Outcome<String>),
}

impl Tasks {
    fn bedrock(&self, event: BedrockEvent) {
        self.send(Event::Bedrock(event));
    }

    pub fn bedrock_scan(&self, edition: Edition) {
        self.run(move |t| {
            let install = bedrock::find(edition);
            let running = install.is_some() && bedrock::running(edition);
            t.bedrock(BedrockEvent::Scanned(
                edition,
                BedrockScan {
                    install,
                    running,
                    gaming_services: bedrock::gaming_services(),
                    folders: bedrock::folders(edition),
                },
            ));
        });
    }

    pub fn bedrock_launch(&self, edition: Edition) {
        self.run(move |t| {
            let result = bedrock::launch(edition)
                .map(|()| format!("Starting {}", edition.name()))
                .map_err(|e| e.to_string());
            t.bedrock(BedrockEvent::Done(result));
            // It shows as running once its window is up.
            std::thread::sleep(std::time::Duration::from_secs(4));
            t.bedrock_scan(edition);
        });
    }

    pub fn bedrock_catalog(&self) {
        self.run(|t| {
            let result = versions::catalog(&t.dirs().cache()).map_err(|e| e.to_string());
            t.bedrock(BedrockEvent::Catalog(result));
        });
    }

    pub fn bedrock_install(&self, version: Version, cancel: Arc<AtomicBool>) {
        self.run(move |t| {
            let progress = |p: ProgressInfo| {
                t.bedrock(BedrockEvent::InstallProgress(ProgressSnapshot::from(p)));
            };
            let result = versions::install(&version, &cancel, &progress).map_err(|e| e.to_string());
            t.bedrock(BedrockEvent::Installed(
                version.edition,
                version.id.clone(),
                result,
            ));
            t.bedrock_scan(version.edition);
        });
    }

    pub fn bedrock_worlds(&self, player: Player) {
        self.run(move |t| {
            let list = worlds::list(&player);
            t.bedrock(BedrockEvent::Worlds(player.com_mojang, list));
        });
    }

    pub fn bedrock_packs(&self, shared: Player) {
        self.run(move |t| {
            let list = packs::list(&shared);
            t.bedrock(BedrockEvent::Packs(shared.com_mojang, list));
        });
    }

    pub fn bedrock_servers(&self, player: Player) {
        self.run(move |t| {
            let list = servers::list(&player);
            let addresses: Vec<String> = list.iter().map(Server::address).collect();
            t.bedrock(BedrockEvent::Servers(player.com_mojang, list));
            servers::ping_each(&addresses, |i, result| {
                t.bedrock(BedrockEvent::ServerStatus(
                    addresses[i].clone(),
                    result.map_err(|e| e.to_string()),
                ));
            });
        });
    }

    pub fn bedrock_ping(&self, address: String) {
        self.run(move |t| {
            let result = servers::ping(&address).map_err(|e| e.to_string());
            t.bedrock(BedrockEvent::ServerStatus(address, result));
        });
    }

    /// Ask for `.mcworld` files and import them for `player`.
    pub fn bedrock_import_worlds(&self, player: Player) {
        self.run(move |t| {
            let picked = rfd::FileDialog::new()
                .set_title("Choose Bedrock worlds")
                .add_filter("Bedrock world", &["mcworld", "zip"])
                .pick_files();
            t.note_if_picker_missing(&picked);
            let Some(files) = picked else {
                return;
            };
            let result = each(&files, |f| worlds::import(f, &player), "world");
            t.bedrock(BedrockEvent::Done(result));
            t.bedrock_worlds(player);
        });
    }

    /// Import dropped files: worlds, packs or add-ons, by their extension.
    pub fn bedrock_import_files(&self, files: Vec<PathBuf>, player: Player, shared: Player) {
        self.run(move |t| {
            let mut said = Vec::new();
            let mut error = None;
            for file in &files {
                let ext = file
                    .extension()
                    .map(|e| e.to_string_lossy().to_ascii_lowercase())
                    .unwrap_or_default();
                let result = if ext == "mcworld" || file.join("level.dat").is_file() {
                    worlds::import(file, &player).map(|name| vec![name])
                } else {
                    packs::import(file, &shared)
                };
                match result {
                    Ok(names) => said.extend(names),
                    Err(e) => error = Some(e.to_string()),
                }
            }
            let result = match (said.len(), error) {
                (0, Some(e)) => Err(e),
                (0, None) => Ok("Nothing to import".to_owned()),
                (1, _) => Ok(format!("Imported {}", said[0])),
                (n, _) => Ok(format!("Imported {n} items")),
            };
            t.bedrock(BedrockEvent::Done(result));
            t.bedrock_worlds(player);
            t.bedrock_packs(shared);
        });
    }

    /// Save a world as a `.mcworld` where the player picks.
    pub fn bedrock_export_world(&self, world: World) {
        self.run(move |t| {
            let picked = rfd::FileDialog::new()
                .set_title("Export world")
                .set_file_name(format!("{}.mcworld", world.name))
                .add_filter("Bedrock world", &["mcworld"])
                .save_file();
            t.note_if_picker_missing(&picked);
            let Some(to) = picked else {
                return;
            };
            let result = worlds::export(&world, &to)
                .map(|()| format!("Exported {}", world.name))
                .map_err(|e| e.to_string());
            t.bedrock(BedrockEvent::Done(result));
        });
    }

    pub fn bedrock_backup_world(&self, world: World, backups: PathBuf) {
        self.run(move |t| {
            let result = worlds::backup(&world, &backups)
                .map(|_| {
                    let _ = open::that_detached(&backups);
                    format!("Backed up {}", world.name)
                })
                .map_err(|e| e.to_string());
            t.bedrock(BedrockEvent::Done(result));
        });
    }

    /// Ask for `.mcpack` / `.mcaddon` files and import them.
    pub fn bedrock_import_packs(&self, shared: Player) {
        self.run(move |t| {
            let picked = rfd::FileDialog::new()
                .set_title("Choose packs or add-ons")
                .add_filter("Bedrock pack", &["mcpack", "mcaddon", "zip"])
                .pick_files();
            t.note_if_picker_missing(&picked);
            let Some(files) = picked else {
                return;
            };
            let result = (|| -> Result<Vec<String>> {
                let mut names = Vec::new();
                for f in &files {
                    names.extend(packs::import(f, &shared)?);
                }
                Ok(names)
            })()
            .map(|names| match names.as_slice() {
                [one] => format!("Imported {one}"),
                many => format!("Imported {} packs", many.len()),
            })
            .map_err(|e| e.to_string());
            t.bedrock(BedrockEvent::Done(result));
            t.bedrock_packs(shared);
        });
    }
}

/// Run `job` for each file; say how it went.
fn each(
    files: &[PathBuf],
    job: impl Fn(&std::path::Path) -> Result<String>,
    what: &str,
) -> Outcome<String> {
    let mut done = Vec::new();
    let mut error = None;
    for f in files {
        match job(f) {
            Ok(name) => done.push(name),
            Err(e) => error = Some(e.to_string()),
        }
    }
    match (done.as_slice(), error) {
        ([], Some(e)) => Err(e),
        ([one], _) => Ok(format!("Imported {one}")),
        (many, _) => Ok(format!("Imported {} {what}s", many.len())),
    }
}
