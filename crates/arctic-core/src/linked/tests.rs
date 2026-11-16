use std::path::Path;

use super::apply::{self, Download, Target};
use super::diff::{self, ConfigChange, ModChange};
use super::manifest::{self, ConfigFile, LeftOutReason, LinkedMod, Manifest};
use super::paths::{self, Refused};
use super::*;
use crate::instances::Loader;
use crate::mods::InstalledMod;
use crate::mods::remote::{self, RemoteFile, RemoteVersion};
use crate::net::DownloadJob;

fn fabric() -> Loader {
    Loader::Fabric {
        version: "0.17.2".into(),
    }
}

/// A game folder with a mods folder and an index next to it.
struct Fixture {
    _dir: tempfile::TempDir,
    root: std::path::PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        std::fs::create_dir_all(root.join("game/mods")).unwrap();
        std::fs::create_dir_all(root.join("game/config")).unwrap();
        Self { _dir: dir, root }
    }
    fn game(&self) -> std::path::PathBuf {
        self.root.join("game")
    }
    fn index(&self) -> std::path::PathBuf {
        self.root.join("mods.json")
    }
    fn target(&self) -> (std::path::PathBuf, std::path::PathBuf) {
        (self.game(), self.index())
    }
    /// A Modrinth mod in the folder and the index.
    fn tracked(&self, project: &str, version: &str, file: &str, bytes: &[u8]) {
        std::fs::write(self.game().join("mods").join(file), bytes).unwrap();
        let mut entries = remote::index_entries(&self.index()).unwrap();
        entries.push(InstalledMod {
            project_id: project.into(),
            version_id: version.into(),
            title: project.to_uppercase(),
            version_number: "1.0".into(),
            file_name: file.into(),
            icon_url: None,
            dependency: false,
        });
        remote::save_index(&self.index(), entries).unwrap();
    }
    fn config(&self, path: &str, text: &str) {
        let p = self.game().join(path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
    fn read(&self, rel: &str) -> Option<String> {
        std::fs::read_to_string(self.game().join(rel)).ok()
    }
}

fn hashes(bytes: &[u8]) -> (String, String) {
    use sha2::Digest;
    (
        hex::encode(sha1::Sha1::digest(bytes)),
        hex::encode(sha2::Sha512::digest(bytes)),
    )
}

fn linked(project: &str, version: &str, file: &str, bytes: &[u8]) -> LinkedMod {
    let (sha1, sha512) = hashes(bytes);
    LinkedMod {
        project: project.into(),
        version: version.into(),
        title: project.to_uppercase(),
        file: file.into(),
        size: bytes.len() as u64,
        sha1,
        sha512,
        enabled: true,
        dependency: false,
    }
}

fn manifest(mods: Vec<LinkedMod>, configs: Vec<ConfigFile>) -> Manifest {
    Manifest {
        arctic_link: manifest::FORMAT,
        name: "Our SMP".into(),
        game_version: "1.21.11".into(),
        loader: fabric(),
        mods,
        configs,
    }
}

fn download(m: &LinkedMod) -> Download {
    Download {
        project: m.project.clone(),
        version: m.version.clone(),
        version_number: "2.0".into(),
        url: format!("https://cdn.modrinth.com/data/{}/{}", m.project, m.file),
        file: m.file.clone(),
        size: m.size,
        sha1: m.sha1.clone(),
        sha512: m.sha512.clone(),
    }
}

/// A fetcher that "downloads" these contents by URL.
fn serving(files: Vec<(String, Vec<u8>)>) -> impl Fn(Vec<DownloadJob>) -> crate::Result<()> {
    move |jobs: Vec<DownloadJob>| {
        for job in jobs {
            let bytes = files
                .iter()
                .find(|(u, _)| *u == job.url)
                .map(|(_, b)| b.clone())
                .ok_or_else(|| crate::Error::Other("404".into()))?;
            std::fs::write(&job.dest, bytes).unwrap();
        }
        Ok(())
    }
}

/// Every file under a folder with its contents (to compare before and after).
fn snapshot(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    fn walk(base: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(base, &p, out);
            } else {
                let rel = p.strip_prefix(base).unwrap().to_string_lossy().into_owned();
                out.push((rel, std::fs::read(&p).unwrap()));
            }
        }
    }
    walk(dir, dir, &mut out);
    out.sort();
    out
}

// ------------------------------------------------------------ allow-list

#[test]
fn config_paths_stay_inside_config() {
    assert_eq!(
        paths::check_config_path("config/sodium-options.json"),
        Ok("config/sodium-options.json".into())
    );
    assert!(paths::check_config_path("config/create/client.toml").is_ok());
    for bad in [
        "config/../options.txt",
        "../config/a.json",
        "config/./a.json",
        "/config/a.json",
        "config\\a.json",
        "C:/config/a.json",
        "config//a.json",
        "config/",
        "config",
        "mods/a.json",
        "options.txt",
        "servers.dat",
        "saves/world/level.json",
        "config/.hidden.json",
        "config/con.json",
        "config/a.json.",
        "config/a\u{0}.json",
        "config/a\n.json",
        "config/a/b/c/d/e/f/g/h.json",
    ] {
        assert_eq!(
            paths::check_config_path(bad),
            Err(Refused::OutsideConfig),
            "{bad:?}"
        );
    }
    for not_config in ["config/a.jar", "config/a.exe", "config/a.dat", "config/a"] {
        assert_eq!(
            paths::check_config_path(not_config),
            Err(Refused::NotConfig),
            "{not_config:?}"
        );
    }
    for personal in [
        "config/arctic.json",
        "config/arctic-session.json",
        "config/arctic/hud.json",
        "config/mymod/tokens.json",
        "config/discord-auth.toml",
        "config/amecs-keybindings.json",
        "config/mymod/cache/x.json",
        "config/voicechat/servers.json",
        "config/x/waypoints/a.txt",
    ] {
        assert_eq!(
            paths::check_config_path(personal),
            Err(Refused::Personal),
            "{personal:?}"
        );
    }
}

#[cfg(unix)]
#[test]
fn config_paths_never_go_through_links() {
    let f = Fixture::new();
    let outside = f.root.join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, f.game().join("config/linked")).unwrap();
    assert!(paths::resolve(&f.game(), "config/linked/a.json").is_err());
    assert!(paths::resolve(&f.game(), "config/plain/a.json").is_ok());
}

// --------------------------------------------------------- manifest build

#[test]
fn build_keeps_modrinth_mods_and_drops_local_jars() {
    let f = Fixture::new();
    f.tracked("sodium", "s1", "sodium.jar", b"sodium bytes");
    f.tracked("lithium", "l1", "lithium.jar", b"lithium bytes");
    std::fs::rename(
        f.game().join("mods/lithium.jar"),
        f.game().join("mods/lithium.jar.disabled"),
    )
    .unwrap();
    std::fs::write(f.game().join("mods/my-own.jar"), b"hand made").unwrap();
    std::fs::write(f.game().join("mods/arctic-mod.jar"), b"arctic").unwrap();
    f.config("config/sodium-options.json", "{\"a\":1}");
    f.config("config/arctic.json", "{\"proxy\":\"x\"}");
    f.config("config/journeymap/cache/blob.dat", "x");
    std::fs::write(
        f.game().join("config/huge.json"),
        vec![b'x'; manifest::MAX_CONFIG_FILE as usize + 1],
    )
    .unwrap();
    std::fs::write(f.game().join("options.txt"), "fov:90").unwrap();

    let (game, index) = f.target();
    let built = manifest::build(&manifest::Source {
        name: "Mine",
        game_version: "1.21.11",
        loader: &fabric(),
        game_dir: &game,
        index: &index,
        include_configs: true,
    })
    .unwrap();
    let m = &built.manifest;
    assert_eq!(
        m.mods
            .iter()
            .map(|m| m.project.as_str())
            .collect::<Vec<_>>(),
        ["lithium", "sodium"]
    );
    let sodium = m.mods.iter().find(|m| m.project == "sodium").unwrap();
    assert_eq!(
        (sodium.sha1.clone(), sodium.sha512.clone()),
        hashes(b"sodium bytes")
    );
    assert!(
        !m.mods
            .iter()
            .find(|m| m.project == "lithium")
            .unwrap()
            .enabled
    );
    assert_eq!(
        m.configs
            .iter()
            .map(|c| c.path.as_str())
            .collect::<Vec<_>>(),
        ["config/sodium-options.json"]
    );
    let left = |name: &str| {
        built
            .left_out
            .iter()
            .find(|l| l.name == name)
            .map(|l| l.reason)
    };
    assert_eq!(left("my-own.jar"), Some(LeftOutReason::NotFromModrinth));
    assert_eq!(
        left("arctic-mod.jar"),
        None,
        "the launcher's own jar is never listed"
    );
    assert_eq!(
        left("config/arctic.json"),
        Some(LeftOutReason::Config(Refused::Personal))
    );
    assert_eq!(
        left("config/journeymap/cache/blob.dat"),
        Some(LeftOutReason::Config(Refused::NotConfig))
    );
    assert_eq!(left("config/huge.json"), Some(LeftOutReason::TooBig));
    assert!(
        built
            .left_out
            .iter()
            .all(|l| !l.name.contains("options.txt"))
    );
    let json = m.to_json().unwrap();
    assert!(!json.contains("proxy") && !json.contains("fov"));
    assert_eq!(Manifest::from_json(json.as_bytes()).unwrap(), *m);

    let without = manifest::build(&manifest::Source {
        include_configs: false,
        name: "Mine",
        game_version: "1.21.11",
        loader: &fabric(),
        game_dir: &game,
        index: &index,
    })
    .unwrap();
    assert!(without.manifest.configs.is_empty());
}

#[test]
fn verify_drops_jars_that_are_not_modrinths_file() {
    let good = linked("sodium", "s1", "sodium.jar", b"real");
    let changed = linked("lithium", "l1", "lithium.jar", b"patched");
    let remote_for = |m: &LinkedMod, bytes: &[u8]| {
        let (sha1, sha512) = hashes(bytes);
        RemoteVersion {
            project_id: m.project.clone(),
            version_id: m.version.clone(),
            game_versions: vec!["1.21.11".into()],
            loaders: vec!["fabric".into()],
            files: vec![RemoteFile {
                project_id: m.project.clone(),
                version_id: m.version.clone(),
                version_number: "1".into(),
                file_name: m.file.clone(),
                url: "https://cdn.modrinth.com/x.jar".into(),
                sha1: Some(sha1),
                sha512: Some(sha512),
                size: Some(1),
            }],
        }
    };
    let versions = vec![
        remote_for(&good, b"real"),
        remote_for(&changed, b"original"),
    ];
    let built = manifest::verify(
        manifest::Built {
            manifest: manifest(vec![good.clone(), changed], Vec::new()),
            left_out: Vec::new(),
        },
        &versions,
    );
    assert_eq!(built.manifest.mods, vec![good]);
    assert_eq!(built.left_out[0].name, "lithium.jar");
    assert_eq!(built.left_out[0].reason, LeftOutReason::DiffersFromModrinth);
}

#[test]
fn manifests_with_bad_entries_are_refused() {
    let ok = manifest(
        vec![linked("sodium", "s1", "sodium.jar", b"x")],
        vec![ConfigFile::new("config/a.json".into(), b"{}")],
    );
    assert!(Manifest::from_json(ok.to_json().unwrap().as_bytes()).is_ok());

    let mut traversal = ok.clone();
    traversal.configs[0].path = "config/../../evil.json".into();
    let err = Manifest::from_json(serde_json::to_string(&traversal).unwrap().as_bytes());
    assert!(err.is_err());

    let mut outside = ok.clone();
    outside.configs[0].path = "options.txt".into();
    assert!(Manifest::from_json(serde_json::to_string(&outside).unwrap().as_bytes()).is_err());

    let mut evil_jar = ok.clone();
    evil_jar.mods[0].file = "../../evil.jar".into();
    assert!(Manifest::from_json(serde_json::to_string(&evil_jar).unwrap().as_bytes()).is_err());

    let mut bad_hash = ok.clone();
    bad_hash.mods[0].sha512 = "zz".into();
    assert!(Manifest::from_json(serde_json::to_string(&bad_hash).unwrap().as_bytes()).is_err());

    let mut tampered = ok.clone();
    tampered.configs[0].data =
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"{\"x\":1}");
    assert!(Manifest::from_json(serde_json::to_string(&tampered).unwrap().as_bytes()).is_err());

    let mut twice = ok.clone();
    twice.mods.push(twice.mods[0].clone());
    assert!(Manifest::from_json(serde_json::to_string(&twice).unwrap().as_bytes()).is_err());

    let mut newer: serde_json::Value = serde_json::from_str(&ok.to_json().unwrap()).unwrap();
    newer["arctic_link"] = serde_json::json!(manifest::FORMAT + 1);
    let err = Manifest::from_json(newer.to_string().as_bytes()).unwrap_err();
    assert!(err.to_string().contains("newer Arctic"), "{err}");
}

#[test]
fn server_answers_parse() {
    let m = manifest(vec![linked("sodium", "s1", "sodium.jar", b"x")], Vec::new());
    let body =
        serde_json::json!({"code": "abcdefgh", "revision": 7, "updated": 100, "manifest": m});
    let fetched = server::parse_fetched(body.to_string().as_bytes()).unwrap();
    assert_eq!((fetched.revision, fetched.updated), (7, 100));
    assert_eq!(fetched.manifest, m);
    assert!(server::parse_fetched(b"{\"revision\":1}").is_err());
}

// ------------------------------------------------------------------- diff

#[test]
fn diff_lists_adds_updates_removals_and_configs() {
    let f = Fixture::new();
    f.tracked("sodium", "s1", "sodium-1.jar", b"sodium 1");
    f.tracked("lithium", "l1", "lithium.jar", b"lithium");
    f.tracked("mine", "m1", "mine.jar", b"my own pick");
    f.tracked("old", "o1", "old.jar", b"owner dropped");
    f.config("config/same.json", "{}");
    f.config("config/changed.json", "{\"v\":1}");
    f.config("config/dropped.json", "{\"d\":1}");
    f.config("config/dropped-edited.json", "{\"mine\":1}");

    let previous = manifest(
        vec![
            linked("sodium", "s1", "sodium-1.jar", b"sodium 1"),
            linked("lithium", "l1", "lithium.jar", b"lithium"),
            linked("old", "o1", "old.jar", b"owner dropped"),
        ],
        vec![
            ConfigFile::new("config/dropped.json".into(), b"{\"d\":1}"),
            ConfigFile::new("config/dropped-edited.json".into(), b"{\"d\":1}"),
        ],
    );
    let wanted = manifest(
        vec![
            linked("sodium", "s2", "sodium-2.jar", b"sodium 2"),
            linked("lithium", "l1", "lithium.jar", b"lithium"),
            linked("iris", "i1", "iris.jar", b"iris"),
        ],
        vec![
            ConfigFile::new("config/same.json".into(), b"{}"),
            ConfigFile::new("config/changed.json".into(), b"{\"v\":2}"),
            ConfigFile::new("config/new/a.toml".into(), b"a = 1"),
        ],
    );
    let (game, index) = f.target();
    let d = diff::compute(
        &wanted,
        3,
        Some(&previous),
        Some("1.21.11"),
        &fabric(),
        &game,
        &index,
    )
    .unwrap();
    assert_eq!(d.revision, 3);
    assert!(d.target.is_none());
    let find = |title: &str| d.mods.iter().find(|m| m.change.title() == title).unwrap();
    assert!(matches!(find("IRIS").change, ModChange::Add(_)) && find("IRIS").accept);
    assert!(matches!(&find("SODIUM").change, ModChange::Update { to, .. } if to.version == "s2"));
    assert!(matches!(
        find("OLD").change,
        ModChange::Remove {
            from_link: true,
            ..
        }
    ));
    assert!(
        find("OLD").accept,
        "the owner's removed mod goes by default"
    );
    assert!(matches!(
        find("MINE").change,
        ModChange::Remove {
            from_link: false,
            ..
        }
    ));
    assert!(
        !find("MINE").accept,
        "a mod the friend added stays unless they tick it"
    );
    assert_eq!(d.mods.len(), 4, "lithium is the same: not listed");

    let configs: Vec<(&str, bool, &str)> = d
        .configs
        .iter()
        .map(|c| {
            let kind = match &c.change {
                ConfigChange::Add(_) => "add",
                ConfigChange::Change(_) => "change",
                ConfigChange::Remove { .. } => "remove",
            };
            (c.change.path(), c.accept, kind)
        })
        .collect();
    assert_eq!(
        configs,
        [
            ("config/changed.json", true, "change"),
            ("config/dropped-edited.json", false, "remove"),
            ("config/dropped.json", true, "remove"),
            ("config/new/a.toml", true, "add"),
        ]
    );
}

#[test]
fn a_version_change_must_be_accepted_before_mods_apply() {
    let f = Fixture::new();
    f.tracked("old", "o1", "old.jar", b"o");
    let mut wanted = manifest(vec![linked("iris", "i1", "iris.jar", b"iris")], Vec::new());
    wanted.game_version = "26.3".into();
    let (game, index) = f.target();
    let mut d = diff::compute(&wanted, 1, None, Some("1.21.11"), &fabric(), &game, &index).unwrap();
    let t = d.target.as_ref().unwrap();
    assert_eq!(
        (
            t.change.from_version.as_deref(),
            t.change.to_version.as_str()
        ),
        (Some("1.21.11"), "26.3")
    );
    d.mods.iter_mut().for_each(|m| m.accept = true);
    d.target.as_mut().unwrap().accept = false;
    assert!(d.mods_blocked());
    let sel = d.selected();
    assert!(sel.target.is_none());
    assert!(
        sel.mods
            .iter()
            .all(|m| matches!(m.change, ModChange::Remove { .. })),
        "only removals go without the version change"
    );
}

#[test]
fn owner_changes_are_listed() {
    let old = manifest(
        vec![
            linked("a", "a1", "a.jar", b"a"),
            linked("b", "b1", "b.jar", b"b"),
        ],
        vec![ConfigFile::new("config/x.json".into(), b"1")],
    );
    let mut new = manifest(
        vec![
            linked("a", "a2", "a2.jar", b"a2"),
            linked("c", "c1", "c.jar", b"c"),
        ],
        vec![ConfigFile::new("config/x.json".into(), b"2")],
    );
    new.game_version = "26.3".into();
    let c = changes(&old, &new);
    assert_eq!(c.added, ["C"]);
    assert_eq!(c.removed, ["B"]);
    assert_eq!(c.updated, ["A"]);
    assert_eq!(c.configs, ["config/x.json"]);
    assert!(c.target.is_some());
    assert!(changes(&old, &old).is_empty());
}

// ------------------------------------------------------------------ apply

/// The instance of the apply tests, and a diff that adds, updates, removes and changes configs.
type Served = Vec<(String, Vec<u8>)>;

fn prepared() -> (Fixture, diff::Diff, Vec<Download>, Served) {
    let f = Fixture::new();
    f.tracked("sodium", "s1", "sodium-1.jar", b"sodium 1");
    f.tracked("old", "o1", "old.jar", b"old");
    f.config("config/changed.json", "{\"v\":1}");
    let previous = manifest(vec![linked("old", "o1", "old.jar", b"old")], Vec::new());
    let iris = linked("iris", "i1", "iris.jar", b"iris bytes");
    let sodium = linked("sodium", "s2", "sodium-2.jar", b"sodium 2");
    let wanted = manifest(
        vec![iris.clone(), sodium.clone()],
        vec![
            ConfigFile::new("config/changed.json".into(), b"{\"v\":2}"),
            ConfigFile::new("config/new/a.toml".into(), b"a = 1"),
        ],
    );
    let (game, index) = f.target();
    let d = diff::compute(
        &wanted,
        2,
        Some(&previous),
        Some("1.21.11"),
        &fabric(),
        &game,
        &index,
    )
    .unwrap();
    let downloads = vec![download(&iris), download(&sodium)];
    let served = vec![
        (downloads[0].url.clone(), b"iris bytes".to_vec()),
        (downloads[1].url.clone(), b"sodium 2".to_vec()),
    ];
    (f, d, downloads, served)
}

#[test]
fn applying_an_approved_diff_changes_exactly_that() {
    let (f, d, downloads, served) = prepared();
    let (game, index) = f.target();
    let fetch = serving(served);
    let mut committed = false;
    let report = apply::apply(
        &Target {
            game_dir: &game,
            index: &index,
        },
        &d,
        &downloads,
        &fetch,
        &mut || {
            committed = true;
            Ok(())
        },
    )
    .unwrap();
    assert!(committed);
    assert_eq!(
        (report.added, report.updated, report.removed, report.configs),
        (1, 1, 1, 2)
    );
    assert_eq!(f.read("mods/iris.jar").as_deref(), Some("iris bytes"));
    assert_eq!(f.read("mods/sodium-2.jar").as_deref(), Some("sodium 2"));
    assert!(f.read("mods/sodium-1.jar").is_none());
    assert!(f.read("mods/old.jar").is_none());
    assert_eq!(f.read("config/changed.json").as_deref(), Some("{\"v\":2}"));
    assert_eq!(f.read("config/new/a.toml").as_deref(), Some("a = 1"));
    let mut tracked: Vec<(String, String)> = remote::index_entries(&index)
        .unwrap()
        .into_iter()
        .map(|m| (m.project_id, m.version_id))
        .collect();
    tracked.sort();
    assert_eq!(
        tracked,
        [("iris".into(), "i1".into()), ("sodium".into(), "s2".into())]
    );
    assert!(
        !game.join(".arctic-link").exists(),
        "work folders are cleaned up"
    );
}

#[test]
fn a_bad_hash_changes_nothing() {
    let (f, d, downloads, mut served) = prepared();
    served[1].1 = b"sodium 2 but tampered".to_vec();
    let (game, index) = f.target();
    let before = snapshot(&f.root);
    let fetch = serving(served);
    let err = apply::apply(
        &Target {
            game_dir: &game,
            index: &index,
        },
        &d,
        &downloads,
        &fetch,
        &mut || panic!("never committed"),
    )
    .unwrap_err();
    assert!(matches!(err, crate::Error::Checksum(_)), "{err}");
    assert_eq!(snapshot(&f.root), before);
}

#[test]
fn a_failed_download_changes_nothing() {
    let (f, d, downloads, mut served) = prepared();
    served.pop();
    let (game, index) = f.target();
    let before = snapshot(&f.root);
    let fetch = serving(served);
    assert!(
        apply::apply(
            &Target {
                game_dir: &game,
                index: &index
            },
            &d,
            &downloads,
            &fetch,
            &mut || Ok(()),
        )
        .is_err()
    );
    assert_eq!(snapshot(&f.root), before);
}

#[test]
fn a_failure_halfway_is_rolled_back() {
    // Fail at every step in turn: the instance is always as it was.
    for fail_at in 0..8 {
        let (f, d, downloads, served) = prepared();
        let (game, index) = f.target();
        let before = snapshot(&f.root);
        let fetch = serving(served);
        let mut n = 0;
        let result = apply::apply_with(
            &Target {
                game_dir: &game,
                index: &index,
            },
            &d,
            &downloads,
            &fetch,
            &mut || Ok(()),
            &mut |_| {
                n += 1;
                if n > fail_at {
                    Err(crate::Error::Other("disk full".into()))
                } else {
                    Ok(())
                }
            },
        );
        assert!(result.is_err(), "step {fail_at}");
        assert_eq!(snapshot(&f.root), before, "step {fail_at}");
    }
}

#[test]
fn a_failed_commit_is_rolled_back() {
    let (f, d, downloads, served) = prepared();
    let (game, index) = f.target();
    let before = snapshot(&f.root);
    let fetch = serving(served);
    let err = apply::apply(
        &Target {
            game_dir: &game,
            index: &index,
        },
        &d,
        &downloads,
        &fetch,
        &mut || Err(crate::Error::Other("could not save link.json".into())),
    )
    .unwrap_err();
    assert!(err.to_string().contains("link.json"));
    assert_eq!(snapshot(&f.root), before);
}

#[test]
fn only_accepted_items_are_applied() {
    let (f, mut d, downloads, served) = prepared();
    for m in &mut d.mods {
        m.accept = matches!(m.change, ModChange::Add(_));
    }
    for c in &mut d.configs {
        c.accept = false;
    }
    let (game, index) = f.target();
    let fetch = serving(served);
    let report = apply::apply(
        &Target {
            game_dir: &game,
            index: &index,
        },
        &d,
        &downloads[..1],
        &fetch,
        &mut || Ok(()),
    )
    .unwrap();
    assert_eq!(
        (report.added, report.updated, report.removed, report.configs),
        (1, 0, 0, 0)
    );
    assert_eq!(f.read("mods/sodium-1.jar").as_deref(), Some("sodium 1"));
    assert_eq!(f.read("mods/old.jar").as_deref(), Some("old"));
    assert_eq!(f.read("config/changed.json").as_deref(), Some("{\"v\":1}"));
}

#[test]
fn downloads_must_match_modrinths_file() {
    let m = linked("sodium", "s1", "sodium.jar", b"real");
    let (sha1, sha512) = hashes(b"real");
    let version = |url: &str, sha512: &str, project: &str| RemoteVersion {
        project_id: project.into(),
        version_id: "s1".into(),
        game_versions: vec![],
        loaders: vec![],
        files: vec![RemoteFile {
            project_id: project.into(),
            version_id: "s1".into(),
            version_number: "1.0".into(),
            file_name: "sodium.jar".into(),
            url: url.into(),
            sha1: Some(sha1.clone()),
            sha512: Some(sha512.into()),
            size: Some(4),
        }],
    };
    let cdn = "https://cdn.modrinth.com/data/x/sodium.jar";
    let plan = apply::plan_downloads(&[&m], &[version(cdn, &sha512, "sodium")]).unwrap();
    assert_eq!(plan[0].url, cdn);
    assert!(apply::plan_downloads(&[&m], &[version(cdn, &"0".repeat(128), "sodium")]).is_err());
    assert!(
        apply::plan_downloads(
            &[&m],
            &[version("https://evil.example/x.jar", &sha512, "sodium")]
        )
        .is_err()
    );
    assert!(apply::plan_downloads(&[&m], &[version(cdn, &sha512, "other")]).is_err());
    assert!(apply::plan_downloads(&[&m], &[]).is_err());
}

// ------------------------------------------------------------ link state

#[test]
fn unlink_keeps_the_files() {
    let dir = tempfile::tempdir().unwrap();
    let dirs = crate::storage::DataDirs::new(dir.path());
    let inst = crate::instances::create(&dirs, "Linked", "1.21.11", fabric()).unwrap();
    let game = inst.game_dir(&dirs);
    std::fs::create_dir_all(game.join("mods")).unwrap();
    std::fs::create_dir_all(game.join("config")).unwrap();
    std::fs::write(game.join("mods/a.jar"), b"a").unwrap();
    std::fs::write(game.join("config/a.json"), b"{}").unwrap();
    let state = LinkState {
        code: "abcdefgh".into(),
        role: Role::Follower,
        revision: 2,
        manifest: Some(manifest(Vec::new(), Vec::new())),
        latest: 3,
        dismissed: 0,
        gone: false,
    };
    save(&dirs, &inst.id, &state).unwrap();
    let loaded = load(&dirs, &inst.id).unwrap();
    assert_eq!(loaded, state);
    assert!(loaded.has_update() && loaded.reminds());
    dismiss(&dirs, &inst.id).unwrap();
    let loaded = load(&dirs, &inst.id).unwrap();
    assert!(loaded.has_update() && !loaded.reminds());

    unlink(&dirs, &inst.id).unwrap();
    assert!(load(&dirs, &inst.id).is_none());
    assert_eq!(std::fs::read(game.join("mods/a.jar")).unwrap(), b"a");
    assert_eq!(std::fs::read(game.join("config/a.json")).unwrap(), b"{}");
    assert!(
        crate::instances::find(&dirs, &inst.id).is_ok(),
        "still an instance"
    );
    unlink(&dirs, &inst.id).unwrap();
}

#[test]
fn a_fresh_copy_takes_everything() {
    let m = manifest(
        vec![linked("a", "a1", "a.jar", b"a")],
        vec![ConfigFile::new("config/a.json".into(), b"{}")],
    );
    let d = fresh_diff(&m, 4);
    assert_eq!((d.revision, d.mods.len(), d.configs.len()), (4, 1, 1));
    assert!(d.mods.iter().all(|i| i.accept) && d.configs.iter().all(|i| i.accept));
    assert!(!apply::needs_game_closed(&diff::additions(m.mods.clone())));
    assert!(apply::needs_game_closed(&d));
}

#[test]
fn an_owner_key_is_kept_encrypted() {
    let dir = tempfile::tempdir().unwrap();
    let dirs = crate::storage::DataDirs::new(dir.path());
    let state = LinkState {
        code: "abcdefgh".into(),
        role: Role::Owner {
            key: "k".repeat(43),
            include_configs: false,
        },
        revision: 1,
        manifest: None,
        latest: 1,
        dismissed: 0,
        gone: false,
    };
    save(&dirs, "x", &state).unwrap();
    let text = std::fs::read_to_string(dirs.instance_dir("x").join("link.json")).unwrap();
    assert!(
        !text.contains(&"k".repeat(43)),
        "the key isn't stored in plain text"
    );
    assert!(text.contains("\"role\": \"owner\""));
    assert_eq!(load(&dirs, "x").unwrap(), state);
    assert!(state.is_owner() && !state.has_update());
}
