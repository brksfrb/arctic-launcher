use std::path::{Path, PathBuf};

use super::*;
use crate::instances::{self, Loader, list_custom, load_default};

fn data() -> (tempfile::TempDir, DataDirs) {
    let dir = tempfile::tempdir().unwrap();
    let dirs = DataDirs::new(dir.path().join("data"));
    (dir, dirs)
}

/// The old built-in default as earlier launchers wrote it, with a world in its game folder.
fn legacy(dirs: &DataDirs, name: &str, client: bool) -> PathBuf {
    let dir = dirs.instance_dir(LEGACY_DEFAULT_ID);
    std::fs::create_dir_all(&dir).unwrap();
    let json = serde_json::json!({
        "id": "vanilla", "name": name, "loader": {"type": "vanilla"}, "version": null,
        "max_memory_mb": 6144, "arctic_mod": client, "performance": false, "shaders": true,
    });
    std::fs::write(dir.join(INSTANCE_FILE), json.to_string()).unwrap();
    world(&dir.join("minecraft"))
}

fn world(game: &Path) -> PathBuf {
    let level = game.join("saves/World/level.dat");
    std::fs::create_dir_all(level.parent().unwrap()).unwrap();
    std::fs::write(&level, b"world").unwrap();
    level
}

fn settings(dirs: &DataDirs, last_version: Option<&str>, last_instance: Option<&str>) {
    Settings {
        last_version: last_version.map(Into::into),
        last_instance: last_instance.map(Into::into),
        ..Settings::default()
    }
    .save(dirs)
    .unwrap();
}

fn builtins(dirs: &DataDirs) -> Vec<String> {
    std::fs::read_dir(dirs.instances())
        .unwrap()
        .flatten()
        .filter_map(|e| {
            load_json::<Instance>(&e.path().join(INSTANCE_FILE))
                .ok()
                .flatten()
        })
        .filter(|i| i.builtin)
        .map(|i| i.id)
        .collect()
}

fn fail_at(step: Option<&'static str>) {
    FAIL_AT.with(|f| f.set(step));
}

#[test]
fn a_new_profile_starts_with_the_arctic_instance_only() {
    let (_t, dirs) = data();
    let (arctic, done) = ensure_default(&dirs).unwrap();
    assert_eq!(
        done,
        Some(BuiltinMigration::Created {
            id: ARCTIC_ID.into()
        })
    );
    assert!(arctic.is_default() && arctic.builtin);
    assert_eq!(
        (arctic.id.as_str(), arctic.name.as_str()),
        ("arctic", "Arctic")
    );
    assert_eq!(arctic.version, None, "follows the Play tab");
    assert!(arctic.arctic_mod && arctic.arctic_client() && arctic.performance);
    assert!(!arctic.has_arctic_switch());
    assert!(
        arctic
            .game_dir(&dirs)
            .ends_with("instances/arctic/minecraft")
    );
    assert!(
        !dirs.instance_dir(LEGACY_DEFAULT_ID).exists(),
        "no leftover Vanilla"
    );
    assert!(list_custom(&dirs).unwrap().is_empty());
    // Already there: nothing more.
    assert_eq!(ensure_default(&dirs).unwrap(), (arctic.clone(), None));
    assert_eq!(load_default(&dirs).unwrap(), arctic);
    assert_eq!(builtins(&dirs), vec!["arctic".to_owned()]);
}

#[test]
fn default_with_the_client_on_becomes_arctic_in_place() {
    let (_t, dirs) = data();
    let level = legacy(&dirs, "Vanilla", true);
    crate::playtime::record(&dirs, "vanilla", 3600, 1_000).unwrap();
    let (arctic, done) = ensure_default(&dirs).unwrap();
    assert_eq!(
        done,
        Some(BuiltinMigration::Converted {
            id: "vanilla".into(),
            renamed: true
        })
    );
    assert!(arctic.is_default());
    assert_eq!(arctic.name, "Arctic");
    assert_eq!(arctic.id, "vanilla", "same folder, same id-keyed data");
    assert_eq!(arctic.game_dir(&dirs), level.ancestors().nth(3).unwrap());
    assert_eq!(std::fs::read(&level).unwrap(), b"world");
    // The player's other choices are kept.
    assert_eq!(arctic.max_memory_mb, Some(6144));
    assert!(!arctic.performance && arctic.shaders);
    assert!(
        dirs.instance_dir("vanilla").join(BACKUP_FILE).is_file(),
        "old instance.json kept"
    );
    assert!(!dirs.instance_dir(ARCTIC_ID).exists(), "no second Arctic");
    assert!(list_custom(&dirs).unwrap().is_empty());
    assert!(crate::playtime::Stats::load(&dirs).of("vanilla").is_some());
    // Second start: nothing.
    assert_eq!(ensure_default(&dirs).unwrap(), (arctic, None));
    assert_eq!(builtins(&dirs), vec!["vanilla".to_owned()]);
}

#[test]
fn a_name_the_player_chose_is_kept() {
    let (_t, dirs) = data();
    let level = legacy(&dirs, "My Survival", true);
    let (arctic, done) = ensure_default(&dirs).unwrap();
    assert_eq!(
        done,
        Some(BuiltinMigration::Converted {
            id: "vanilla".into(),
            renamed: false
        })
    );
    assert!(arctic.is_default());
    assert_eq!(arctic.name, "My Survival");
    assert!(level.is_file());
    assert_eq!(ensure_default(&dirs).unwrap(), (arctic, None));
    // Found under "arctic" too (CLI, shortcuts).
    assert_eq!(instances::find(&dirs, "arctic").unwrap().id, "vanilla");
}

#[test]
fn pure_vanilla_default_is_kept_and_arctic_is_added() {
    let (_t, dirs) = data();
    let level = legacy(&dirs, "Vanilla", false);
    settings(&dirs, Some("1.21.4"), None);
    let (arctic, done) = ensure_default(&dirs).unwrap();
    assert_eq!(
        done,
        Some(BuiltinMigration::Added {
            id: "arctic".into(),
            vanilla: "vanilla".into()
        })
    );
    assert!(arctic.is_default() && arctic.arctic_client());
    assert_eq!(
        (arctic.id.as_str(), arctic.name.as_str()),
        ("arctic", "Arctic")
    );
    assert!(arctic.performance, "same defaults as a new Arctic instance");

    let custom = list_custom(&dirs).unwrap();
    assert_eq!(custom.len(), 1);
    let vanilla = &custom[0];
    assert_eq!(
        (vanilla.id.as_str(), vanilla.name.as_str()),
        ("vanilla", "Vanilla")
    );
    assert!(!vanilla.is_default());
    assert_eq!(vanilla.loader, Loader::Vanilla);
    assert_eq!(
        vanilla.version.as_deref(),
        Some("1.21.4"),
        "pinned to what it played"
    );
    assert!(!vanilla.arctic_client() && !vanilla.has_arctic_switch());
    assert_eq!(vanilla.max_memory_mb, Some(6144));
    assert_eq!(vanilla.game_dir(&dirs), level.ancestors().nth(3).unwrap());
    assert_eq!(std::fs::read(&level).unwrap(), b"world", "its worlds stay");
    assert!(!arctic.game_dir(&dirs).join("saves").exists());
    // Still the selected instance on the Play tab.
    let s = Settings::load(&dirs).unwrap();
    assert_eq!(s.last_instance.as_deref(), Some("vanilla"));
    assert_eq!(s.last_version.as_deref(), Some("1.21.4"));

    // Second start: no second Arctic, nothing renamed.
    assert_eq!(ensure_default(&dirs).unwrap(), (arctic, None));
    assert_eq!(list_custom(&dirs).unwrap(), custom);
    assert_eq!(builtins(&dirs), vec!["arctic".to_owned()]);
    // The leftover Vanilla can go like any other instance; Arctic can't.
    assert!(instances::remove(&dirs, "arctic").is_err());
    instances::remove(&dirs, "vanilla").unwrap();
    assert_eq!(load_default(&dirs).unwrap().id, "arctic");
    assert!(list_custom(&dirs).unwrap().is_empty());
}

#[test]
fn another_selected_instance_stays_selected() {
    let (_t, dirs) = data();
    legacy(&dirs, "Vanilla", false);
    settings(&dirs, Some("1.8.9"), Some("pvp-pack"));
    ensure_default(&dirs).unwrap();
    assert_eq!(
        Settings::load(&dirs).unwrap().last_instance.as_deref(),
        Some("pvp-pack")
    );
}

#[test]
fn a_failure_part_way_keeps_the_data_and_the_next_start_finishes() {
    for step in ["settings", "pin", "create"] {
        let (_t, dirs) = data();
        let level = legacy(&dirs, "Vanilla", false);
        settings(&dirs, Some("1.20.1"), None);
        fail_at(Some(step));
        assert!(ensure_default(&dirs).is_err(), "{step}");
        fail_at(None);
        assert_eq!(std::fs::read(&level).unwrap(), b"world", "{step}");
        assert!(builtins(&dirs).is_empty(), "{step}");
        assert!(!dirs.instance_dir(ARCTIC_ID).exists(), "{step}: tidied up");

        let (arctic, done) = ensure_default(&dirs).unwrap();
        assert!(
            matches!(done, Some(BuiltinMigration::Added { .. })),
            "{step}"
        );
        assert_eq!(arctic.id, "arctic");
        let custom = list_custom(&dirs).unwrap();
        assert_eq!(custom.len(), 1, "{step}");
        assert_eq!(custom[0].version.as_deref(), Some("1.20.1"), "{step}");
        assert!(!custom[0].arctic_client());
        assert_eq!(
            Settings::load(&dirs).unwrap().last_instance.as_deref(),
            Some("vanilla"),
            "{step}"
        );
        assert_eq!(ensure_default(&dirs).unwrap().1, None);
        assert_eq!(builtins(&dirs), vec!["arctic".to_owned()]);
    }

    let (_t, dirs) = data();
    let level = legacy(&dirs, "Vanilla", true);
    fail_at(Some("convert"));
    assert!(ensure_default(&dirs).is_err());
    fail_at(None);
    assert!(level.is_file() && builtins(&dirs).is_empty());
    let raw: Instance = load_json(&dirs.instance_dir("vanilla").join(INSTANCE_FILE))
        .unwrap()
        .unwrap();
    assert!(raw.arctic_mod && raw.name == "Vanilla", "left as it was");
    let (arctic, _) = ensure_default(&dirs).unwrap();
    assert_eq!(
        (arctic.id.as_str(), arctic.name.as_str()),
        ("vanilla", "Arctic")
    );
}

#[test]
fn an_unreadable_default_is_not_replaced() {
    let (_t, dirs) = data();
    let level = legacy(&dirs, "Vanilla", true);
    std::fs::write(dirs.instance_dir("vanilla").join(INSTANCE_FILE), b"{oops").unwrap();
    // No second instance is made over it.
    assert!(ensure_default(&dirs).is_err());
    assert!(!dirs.instance_dir(ARCTIC_ID).exists());
    assert!(level.is_file());
}

#[test]
fn an_old_game_folder_without_instance_file_becomes_arctic() {
    let (_t, dirs) = data();
    let level = world(&dirs.instance_dir("vanilla").join("minecraft"));
    let (arctic, _) = ensure_default(&dirs).unwrap();
    assert_eq!(
        (arctic.id.as_str(), arctic.name.as_str()),
        ("vanilla", "Arctic")
    );
    assert_eq!(arctic.game_dir(&dirs), level.ancestors().nth(3).unwrap());
    assert!(!dirs.instance_dir(ARCTIC_ID).exists());
}

#[test]
fn a_players_own_arctic_folder_is_left_alone() {
    let (_t, dirs) = data();
    let theirs = Instance {
        id: "arctic".into(),
        name: "Arctic".into(),
        loader: Loader::Fabric {
            version: "0.16.9".into(),
        },
        version: Some("1.21.4".into()),
        builtin: false,
        ..Instance::arctic_default()
    };
    theirs.save(&dirs).unwrap();
    legacy(&dirs, "Vanilla", false);
    let (arctic, _) = ensure_default(&dirs).unwrap();
    assert_eq!(arctic.id, "arctic-2");
    assert_eq!(ensure_default(&dirs).unwrap(), (arctic, None));
    assert_eq!(builtins(&dirs), vec!["arctic-2".to_owned()]);
    let ids: Vec<String> = list_custom(&dirs)
        .unwrap()
        .into_iter()
        .map(|i| i.id)
        .collect();
    assert_eq!(ids, vec!["arctic".to_owned(), "vanilla".to_owned()]);
}

#[test]
fn only_loaders_have_the_arctic_switch() {
    let (_t, dirs) = data();
    let vanilla = instances::create(&dirs, "Plain", "1.21.4", Loader::Vanilla).unwrap();
    assert!(!vanilla.arctic_mod && !vanilla.arctic_client() && !vanilla.has_arctic_switch());
    // A switch left on in an old file doesn't make it Fabric.
    let mut old = vanilla.clone();
    old.arctic_mod = true;
    std::fs::write(
        dirs.instance_dir(&old.id).join(INSTANCE_FILE),
        serde_json::to_string(&old).unwrap(),
    )
    .unwrap();
    assert!(!instances::find(&dirs, "Plain").unwrap().arctic_mod);

    let fabric = instances::create(
        &dirs,
        "Mods",
        "1.21.4",
        Loader::Fabric {
            version: "0.16.9".into(),
        },
    )
    .unwrap();
    assert!(fabric.arctic_mod && fabric.has_arctic_switch());
    let off = Instance {
        arctic_mod: false,
        ..fabric
    };
    off.save(&dirs).unwrap();
    assert!(!instances::find(&dirs, "Mods").unwrap().arctic_client());

    let arctic = load_default(&dirs).unwrap();
    let mut forced = arctic.clone();
    forced.arctic_mod = false;
    forced.version = Some("1.8.9".into());
    forced.save(&dirs).unwrap();
    assert_eq!(
        load_default(&dirs).unwrap(),
        arctic,
        "always on, follows Play"
    );
    assert_eq!(arctic.kind_label(), "Arctic");
}

#[test]
fn the_arctic_instance_cannot_be_copied_or_reused_by_name() {
    let (_t, dirs) = data();
    let arctic = load_default(&dirs).unwrap();
    assert!(instances::duplicate(&dirs, &arctic, false).is_err());
    assert_eq!(instances::free_name(&dirs, "Arctic").unwrap(), "Arctic (2)");
    assert_eq!(instances::free_name(&dirs, "Vanilla").unwrap(), "Vanilla");
    // A player's instance named "Arctic" gets its own folder id.
    let own = instances::create(&dirs, "Arctic", "1.21.4", Loader::Vanilla).unwrap();
    assert_ne!(own.id, "arctic");
    assert_eq!(load_default(&dirs).unwrap().id, "arctic");
}

#[test]
fn the_stand_in_after_a_failed_move_keeps_the_players_worlds() {
    // Client on: the stand-in is the old default's folder, so its worlds show and play.
    let (_t, dirs) = data();
    let level = legacy(&dirs, "Vanilla", true);
    fail_at(Some("convert"));
    assert!(ensure_default(&dirs).is_err());
    fail_at(None);
    let stand_in = fallback_default(&dirs);
    assert!(stand_in.is_default());
    assert_eq!(
        (stand_in.id.as_str(), stand_in.name.as_str()),
        ("vanilla", "Arctic")
    );
    assert_eq!(stand_in.game_dir(&dirs), level.ancestors().nth(3).unwrap());
    assert!(builtins(&dirs).is_empty(), "nothing written");
    // Saving it (a setting changed) does what the move would have.
    stand_in.save(&dirs).unwrap();
    assert_eq!(ensure_default(&dirs).unwrap(), (stand_in, None));
    assert!(!dirs.instance_dir(ARCTIC_ID).exists());

    // Client off: a stand-in in `arctic`; what was played there is adopted by the next move.
    let (_t, dirs) = data();
    let level = legacy(&dirs, "Vanilla", false);
    settings(&dirs, Some("1.21.4"), None);
    fail_at(Some("settings"));
    assert!(ensure_default(&dirs).is_err());
    fail_at(None);
    let stand_in = fallback_default(&dirs);
    assert_eq!(stand_in.id, "arctic");
    let played = world(&stand_in.game_dir(&dirs));
    let (arctic, _) = ensure_default(&dirs).unwrap();
    assert_eq!(arctic.id, "arctic", "the stand-in's folder, not arctic-2");
    assert!(played.is_file() && level.is_file());
    assert_eq!(list_custom(&dirs).unwrap()[0].id, "vanilla");
}

#[test]
fn other_instances_can_be_removed_while_the_move_fails() {
    let (_t, dirs) = data();
    let mine = instances::create(&dirs, "Mine", "1.21.4", Loader::Vanilla).unwrap();
    legacy(&dirs, "Vanilla", true);
    std::fs::write(dirs.instance_dir("vanilla").join(INSTANCE_FILE), b"{oops").unwrap();
    assert!(ensure_default(&dirs).is_err());
    instances::remove(&dirs, &mine.id).unwrap();
}
