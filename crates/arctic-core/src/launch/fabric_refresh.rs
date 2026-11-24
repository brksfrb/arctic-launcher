//! Cache Fabric loader choices per game version. Publish a background update
//! only after its profile and libraries are ready for Play.

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant, SystemTime};

use serde::{Deserialize, Serialize};

use crate::instances::{Instance, Loader};
use crate::loaders::{self, LoaderKind, fabric_like, util};
use crate::net::{download_before, get_json_before};
use crate::storage::{DataDirs, load_json, save_json};
use crate::versions::RuleEnv;
use crate::{Error, Result, arctic_mod};

use super::files;

const RECHECK: Duration = Duration::from_secs(6 * 60 * 60);
const PREFETCH_TIMEOUT: Duration = Duration::from_secs(5);
static CACHE_WRITE: Mutex<()> = Mutex::new(());

#[derive(Deserialize, Serialize)]
struct CachedLoader {
    version: String,
    prepared: bool,
    checked_at: SystemTime,
}

impl CachedLoader {
    fn recent(&self) -> bool {
        self.checked_at.elapsed().is_ok_and(|age| age < RECHECK)
    }
}

fn cache_path(dirs: &DataDirs, game: &str) -> std::path::PathBuf {
    dirs.meta()
        .join("fabric-loader")
        .join(format!("game-{}.json", util::encode_segment(game)))
}

fn read_cache(dirs: &DataDirs, game: &str) -> Option<CachedLoader> {
    if let Ok(Some(cached)) = load_json(&cache_path(dirs, game)) {
        return Some(cached);
    }
    // PR #198 originally used one version map and its modification time.
    let legacy = dirs.meta().join("arctic-client-loader.json");
    let mut versions: HashMap<String, String> = load_json(&legacy).ok().flatten()?;
    Some(CachedLoader {
        version: versions.remove(game)?,
        prepared: false,
        checked_at: std::fs::metadata(legacy)
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH),
    })
}

fn remember(dirs: &DataDirs, game: &str, cached: CachedLoader) -> Result<()> {
    let _write = CACHE_WRITE.lock().unwrap_or_else(PoisonError::into_inner);
    // A foreground lookup can finish while the worker is installing its result.
    // Only a fully staged result may supersede an already staged choice.
    if cached.prepared || !read_cache(dirs, game).is_some_and(|current| current.prepared) {
        save_json(&cache_path(dirs, game), &cached)?;
    }
    Ok(())
}

fn lookup(dirs: &DataDirs, game: &str, fallback: Option<CachedLoader>) -> Option<String> {
    match loaders::default_loader_version(LoaderKind::Fabric, game) {
        Ok(version) => {
            let _ = remember(
                dirs,
                game,
                CachedLoader {
                    version: version.clone(),
                    prepared: false,
                    checked_at: SystemTime::now(),
                },
            );
            Some(version)
        }
        Err(_) => read_cache(dirs, game)
            .or(fallback)
            .map(|cached| cached.version),
    }
}

pub(crate) fn client_loader(dirs: &DataDirs, game: &str) -> Option<String> {
    match read_cache(dirs, game) {
        Some(cached) if cached.recent() => Some(cached.version),
        cached => lookup(dirs, game, cached),
    }
}

pub(super) fn automatic_loader(
    dirs: &DataDirs,
    game: &str,
    usable: impl Fn(&str) -> bool,
) -> Option<String> {
    match read_cache(dirs, game) {
        Some(cached) if usable(&cached.version) => Some(cached.version),
        cached => lookup(dirs, game, cached),
    }
}

pub(super) fn fabric_loader(
    dirs: &DataDirs,
    instance: &Instance,
    game: &str,
    known: &str,
) -> String {
    read_cache(dirs, game)
        .filter(|cached| {
            cached.prepared
                && util::compare_versions(&cached.version, known).is_gt()
                && (!instance.arctic_mod || arctic_mod::loader_fits(game, &cached.version))
        })
        .map_or_else(|| known.to_owned(), |cached| cached.version)
}

pub(super) fn vanilla_uses_fabric(instance: &Instance, game: &str) -> bool {
    (instance.arctic_mod && arctic_mod::supports(game)) || instance.performance || instance.shaders
}

fn games<'a>(instances: &'a [Instance], default_game: Option<&'a str>) -> Vec<&'a str> {
    let mut seen = HashSet::new();
    instances
        .iter()
        .filter_map(|instance| {
            let game = if instance.is_default() {
                default_game
            } else {
                instance.version.as_deref()
            }?;
            match instance.loader {
                Loader::Fabric { .. } => Some(game),
                Loader::Vanilla if vanilla_uses_fabric(instance, game) => Some(game),
                _ => None,
            }
        })
        .filter(|game| seen.insert(*game))
        .collect()
}

/// Warm the Fabric versions the launcher can start right now. Call once on a
/// background thread with the active profile's instances and Play selection.
pub fn prefetch_fabric_loaders(
    dirs: &DataDirs,
    instances: &[Instance],
    default_game: Option<&str>,
) {
    for game in games(instances, default_game) {
        let deadline = Instant::now() + PREFETCH_TIMEOUT;
        let result = refresh(
            dirs,
            game,
            || fabric_like::latest_fabric_before(game, deadline),
            |version| stage(dirs, game, version, deadline),
        );
        if let Err(err) = result {
            log::debug!("Fabric preload for {game}: {err}");
        }
    }
}

fn refresh(
    dirs: &DataDirs,
    game: &str,
    latest: impl FnOnce() -> Result<Option<String>>,
    stage: impl FnOnce(&str) -> Result<()>,
) -> Result<()> {
    let mut cached = match read_cache(dirs, game) {
        Some(cached) if cached.recent() && cached.prepared => return Ok(()),
        Some(cached) if cached.recent() => cached,
        _ => {
            let Some(version) = latest()? else {
                return Ok(());
            };
            CachedLoader {
                version,
                prepared: false,
                checked_at: SystemTime::now(),
            }
        }
    };
    stage(&cached.version)?;
    cached.prepared = true;
    remember(dirs, game, cached)
}

fn stage(dirs: &DataDirs, game: &str, version: &str, deadline: Instant) -> Result<()> {
    let kind = LoaderKind::Fabric;
    let (profile, raw) = fabric_like::profile(dirs, kind, version, game, |url| {
        get_json_before(url, deadline)
    })?;
    if profile.id != fabric_like::profile_id_for(kind, version, game) {
        return Err(Error::Other(format!(
            "unexpected Fabric profile id: {}",
            profile.id
        )));
    }
    // An unmerged profile contains only the loader's own libraries.
    let libraries = files::resolve_libraries(dirs, &profile, &RuleEnv::for_version(&profile));
    for job in libraries.jobs {
        download_before(job, deadline)?;
    }
    if !libraries
        .main_class
        .is_some_and(|(jar, entry)| files::jar_has_main_class(&jar, &entry))
    {
        return Err(Error::Other("Fabric loader main class is missing".into()));
    }
    if let Some(raw) = raw {
        util::save_profile(dirs, &raw)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    use serde_json::json;

    use super::*;
    use crate::launch::repair_tests::jar_with_class;

    #[test]
    fn only_installed_fabric_and_arctic_instances_supply_games() {
        let default = Instance::vanilla_default();
        let mut fabric = default.clone();
        fabric.id = "fabric".into();
        fabric.version = Some("26.3".into());
        fabric.loader = Loader::Fabric {
            version: "0.20.1".into(),
        };
        let mut quilt = fabric.clone();
        quilt.loader = Loader::Quilt {
            version: "1".into(),
        };
        quilt.version = Some("1.21.1".into());
        let mut pure = default.clone();
        pure.id = "pure".into();
        pure.version = Some("1.20.1".into());
        pure.arctic_mod = false;
        pure.performance = false;
        assert_eq!(
            games(
                &[default, fabric.clone(), fabric, quilt, pure],
                Some("1.8.9")
            ),
            ["1.8.9", "26.3"]
        );
    }

    #[test]
    fn stale_loader_remains_available_until_everything_is_staged() {
        let temp = tempfile::tempdir().unwrap();
        let dirs = DataDirs::new(temp.path());
        let instance = Instance::vanilla_default();
        let game = "26.3";
        let old = "0.19.0";
        let new = "0.20.1";
        let legacy = dirs.meta().join("arctic-client-loader.json");
        save_json(&legacy, &HashMap::from([(game, old)])).unwrap();
        let stamp = std::time::SystemTime::now() - RECHECK - Duration::from_secs(60);
        std::fs::OpenOptions::new()
            .write(true)
            .open(&legacy)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(stamp))
            .unwrap();
        let known = || automatic_loader(&dirs, game, |_| true).unwrap();
        let ask = || Ok(Some(new.to_owned()));
        assert!(
            refresh(&dirs, game, ask, |_| {
                assert_eq!(known(), old);
                Err(Error::Other("offline".into()))
            })
            .is_err()
        );
        assert_eq!(known(), old);
        assert_eq!(fabric_loader(&dirs, &instance, game, old), old);

        refresh(&dirs, game, ask, |version| {
            assert_eq!(version, new);
            assert_eq!(known(), old);
            Ok(())
        })
        .unwrap();
        assert_eq!(known(), new);
        assert_eq!(fabric_loader(&dirs, &instance, game, old), new);
        assert_eq!(fabric_loader(&dirs, &instance, game, "0.21.0"), "0.21.0");
        refresh(
            &dirs,
            game,
            || panic!("refetched a fresh check"),
            |_| panic!("staged twice"),
        )
        .unwrap();
    }

    #[test]
    fn prefetch_installs_the_profile_library_before_publishing() {
        let temp = tempfile::tempdir().unwrap();
        let dirs = DataDirs::new(temp.path());
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let jar = jar_with_class("net/fabricmc/loader/impl/launch/knot/KnotClient.class");
        let artifact = "net.fabricmc:fabric-loader:0.20.1";
        let dest = dirs
            .libraries()
            .join(crate::versions::maven_path(artifact).unwrap());
        let game = "26.3";
        let version = "0.20.1";
        let id = fabric_like::profile_id_for(LoaderKind::Fabric, version, game);
        util::save_profile(
            &dirs,
            &json!({
                "id": id, "inheritsFrom": game,
                "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
                "libraries": [{"name": artifact, "url": url, "size": jar.len()}]
            }),
        )
        .unwrap();

        // A normal launch can have a .part file open at the same time.
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        let foreground_part = crate::net::part_path(&dest);
        std::fs::write(&foreground_part, b"foreground download").unwrap();

        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(&mut socket);
            let mut headers = String::new();
            while reader.read_line(&mut headers).unwrap() != 0 {
                if headers.ends_with("\r\n\r\n") {
                    break;
                }
            }
            write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                jar.len()
            )
            .unwrap();
            socket.write_all(&jar).unwrap();
        });
        refresh(
            &dirs,
            game,
            || Ok(Some(version.into())),
            |loader| stage(&dirs, game, loader, Instant::now() + PREFETCH_TIMEOUT),
        )
        .unwrap();
        server.join().unwrap();
        assert!(dest.is_file());
        assert_eq!(
            std::fs::read(foreground_part).unwrap(),
            b"foreground download"
        );
        assert!(read_cache(&dirs, game).unwrap().prepared);
    }

    #[test]
    fn freshness_is_per_game_and_recent_legacy_answers_are_reused() {
        let temp = tempfile::tempdir().unwrap();
        let dirs = DataDirs::new(temp.path());
        save_json(
            &dirs.meta().join("arctic-client-loader.json"),
            &HashMap::from([("26.3", "0.20.1")]),
        )
        .unwrap();
        refresh(
            &dirs,
            "26.3",
            || panic!("refetched a fresh existing check"),
            |version| {
                assert_eq!(version, "0.20.1");
                Ok(())
            },
        )
        .unwrap();
        refresh(
            &dirs,
            "1.8.9",
            || Ok(Some("0.19.0".into())),
            |version| {
                assert_eq!(version, "0.19.0");
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(
            automatic_loader(&dirs, "26.3", |_| true).as_deref(),
            Some("0.20.1")
        );
        assert_eq!(
            automatic_loader(&dirs, "1.8.9", |_| true).as_deref(),
            Some("0.19.0")
        );
    }
}
