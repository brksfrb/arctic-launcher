use std::io::Write;

use super::trash::{note_path, trash_at};
use super::*;

fn make_world(saves: &Path, folder: &str, name: &str) -> PathBuf {
    let dir = saves.join(folder);
    fs::create_dir_all(dir.join("region")).unwrap();
    fs::write(dir.join("level.dat"), nbt::fake_level_dat(name)).unwrap();
    fs::write(dir.join("region").join("r.0.0.mca"), vec![7u8; 1000]).unwrap();
    fs::write(dir.join("session.lock"), b"lock").unwrap();
    dir
}

#[test]
fn lists_worlds_with_names_and_sizes() {
    let saves = tempfile::tempdir().unwrap();
    make_world(saves.path(), "world1", "Survival");
    fs::create_dir_all(saves.path().join("not-a-world")).unwrap();
    let worlds = list(saves.path());
    assert_eq!(worlds.len(), 1);
    assert_eq!(worlds[0].name, "Survival");
    assert!(worlds[0].size >= 1000);
    assert!(list(&saves.path().join("missing")).is_empty());
}

#[test]
fn imports_folders_without_the_lock_and_renames_duplicates() {
    let src = tempfile::tempdir().unwrap();
    let dst = tempfile::tempdir().unwrap();
    let world = make_world(src.path(), "Castle", "Castle");
    assert_eq!(import_folder(&world, dst.path()).unwrap(), "Castle");
    assert_eq!(import_folder(&world, dst.path()).unwrap(), "Castle (2)");
    let copied = dst.path().join("Castle");
    assert!(copied.join("region").join("r.0.0.mca").is_file());
    assert!(!copied.join("session.lock").exists());
    assert!(import_folder(src.path(), dst.path()).is_err());
}

#[test]
fn backs_up_and_reimports_zips() {
    let saves = tempfile::tempdir().unwrap();
    let backups = tempfile::tempdir().unwrap();
    make_world(saves.path(), "Base", "Base");
    let world = list(saves.path()).remove(0);
    let zip_path = backup(&world, backups.path()).unwrap();
    assert!(zip_path.is_file());

    let dst = tempfile::tempdir().unwrap();
    let name = import_zip(&zip_path, dst.path()).unwrap();
    assert_eq!(name, "Base");
    let imported = list(dst.path());
    assert_eq!(imported.len(), 1);
    assert_eq!(imported[0].name, "Base");
    assert!(dst.path().join("Base/region/r.0.0.mca").is_file());
}

#[test]
fn zips_with_level_dat_at_the_root_work() {
    let dir = tempfile::tempdir().unwrap();
    let zip_path = dir.path().join("Skyblock.zip");
    let mut zip = zip::ZipWriter::new(fs::File::create(&zip_path).unwrap());
    let opts = zip::write::SimpleFileOptions::default();
    zip.start_file("level.dat", opts).unwrap();
    zip.write_all(&nbt::fake_level_dat("Sky")).unwrap();
    zip.start_file("../evil.txt", opts).unwrap();
    zip.write_all(b"nope").unwrap();
    zip.finish().unwrap();

    let saves = dir.path().join("saves");
    fs::create_dir_all(&saves).unwrap();
    assert_eq!(import_zip(&zip_path, &saves).unwrap(), "Skyblock");
    assert!(saves.join("Skyblock/level.dat").is_file());
    assert!(!dir.path().join("evil.txt").exists());
}

#[test]
fn zip_without_world_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let zip_path = dir.path().join("x.zip");
    let mut zip = zip::ZipWriter::new(fs::File::create(&zip_path).unwrap());
    zip.start_file("readme.txt", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.finish().unwrap();
    assert!(import_zip(&zip_path, dir.path()).is_err());
}

#[test]
fn trash_keeps_the_world() {
    let saves = tempfile::tempdir().unwrap();
    make_world(saves.path(), "Old", "Old");
    let world = list(saves.path()).remove(0);
    trash(&world, saves.path(), "x").unwrap();
    assert!(list(saves.path()).is_empty());
    assert_eq!(list_trash(saves.path()).len(), 1);
}

#[test]
fn folder_names_are_safe() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(free_name(dir.path(), "a/b:c"), "a_b_c");
    assert_eq!(free_name(dir.path(), "  ..  "), "World");
}

#[test]
fn a_newer_game_version_backs_up_the_recent_worlds_first() {
    let game = tempfile::tempdir().unwrap();
    let saves = game.path().join("saves");
    make_world(&saves, "Castle", "Castle");
    // First run in this folder: nothing to compare with, nothing backed up.
    assert_eq!(backup_before_version_change(game.path(), "1.21.4"), 0);
    // The same version again, or an older one: nothing.
    assert_eq!(backup_before_version_change(game.path(), "1.21.4"), 0);
    assert_eq!(backup_before_version_change(game.path(), "1.20.1"), 0);
    // Going up from 1.20.1 to 1.21.4 backs up the worlds.
    assert_eq!(backup_before_version_change(game.path(), "1.21.4"), 1);
    let backups = game.path().join(BACKUPS_DIR);
    let wait = std::time::Instant::now();
    while std::fs::read_dir(&backups).map_or(0, |d| d.count()) == 0 {
        assert!(wait.elapsed().as_secs() < 10, "the backup never appeared");
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

fn find(saves: &Path, folder: &str) -> World {
    read_world(&saves.join(folder)).unwrap()
}

#[test]
fn trash_moves_the_world_with_a_note_and_restore_brings_it_back() {
    let saves = tempfile::tempdir().unwrap();
    make_world(saves.path(), "Castle", "Castle Name");
    trash_at(&find(saves.path(), "Castle"), saves.path(), "inst1", 1_000).unwrap();
    assert!(!saves.path().join("Castle").exists());
    assert!(list(saves.path()).is_empty());
    let items = list_trash(saves.path());
    assert_eq!(items.len(), 1);
    let item = &items[0];
    assert_eq!(item.folder, "Castle");
    assert_eq!(item.name, "Castle Name");
    assert_eq!(item.instance, "inst1");
    assert_eq!(item.deleted_at, 1_000);
    assert_eq!(item.original_path, saves.path().join("Castle"));
    assert!(item.dir.join("region").join("r.0.0.mca").is_file());

    assert_eq!(restore(item, saves.path()).unwrap(), "Castle");
    assert!(saves.path().join("Castle").join("level.dat").is_file());
    assert!(list_trash(saves.path()).is_empty());
    assert!(!note_path(&item.dir).exists());
}

#[test]
fn trashing_the_same_folder_twice_in_a_second_keeps_both() {
    let saves = tempfile::tempdir().unwrap();
    for _ in 0..2 {
        make_world(saves.path(), "W", "W");
        trash_at(&find(saves.path(), "W"), saves.path(), "i", 5).unwrap();
    }
    assert_eq!(list_trash(saves.path()).len(), 2);
}

#[test]
fn restore_never_overwrites_a_world_that_took_the_name() {
    let saves = tempfile::tempdir().unwrap();
    make_world(saves.path(), "Castle", "Old");
    trash_at(&find(saves.path(), "Castle"), saves.path(), "i", 1).unwrap();
    let newer = make_world(saves.path(), "Castle", "New");
    let item = list_trash(saves.path()).remove(0);
    assert_eq!(restore(&item, saves.path()).unwrap(), "Castle (2)");
    assert_eq!(find(saves.path(), "Castle").name, "New");
    assert_eq!(find(saves.path(), "Castle (2)").name, "Old");
    assert!(newer.join("level.dat").is_file());
}

#[test]
fn purge_deletes_only_what_is_older_than_14_days() {
    let saves = tempfile::tempdir().unwrap();
    let day = 86_400;
    let now = 100 * day;
    for (folder, at) in [
        ("old", now - 15 * day),
        ("edge", now - 14 * day),
        ("fresh", now - day),
    ] {
        make_world(saves.path(), folder, folder);
        trash_at(&find(saves.path(), folder), saves.path(), "i", at).unwrap();
    }
    // Unknown age (no note, no timestamp in the name): kept.
    let odd = saves.path().join(".trash").join("mystery");
    fs::create_dir_all(&odd).unwrap();
    assert_eq!(purge_expired(saves.path(), now), 1);
    let left: Vec<String> = list_trash(saves.path())
        .into_iter()
        .map(|t| t.folder)
        .collect();
    assert!(left.contains(&"edge".to_owned()) && left.contains(&"fresh".to_owned()));
    assert!(!left.contains(&"old".to_owned()));
    assert!(odd.is_dir());
    assert!(
        !saves
            .path()
            .join(".trash")
            .join("old-8500000.json")
            .exists()
    );
}

#[test]
fn delete_forever_and_foreign_paths() {
    let saves = tempfile::tempdir().unwrap();
    let world = make_world(saves.path(), "A", "A");
    trash_at(&find(saves.path(), "A"), saves.path(), "i", 1).unwrap();
    let item = list_trash(saves.path()).remove(0);
    // A folder outside the trash is refused, and nothing is touched.
    let outside = Trashed {
        dir: saves.path().join("B"),
        ..item.clone()
    };
    let b = make_world(saves.path(), "B", "B");
    assert!(delete_forever(&outside, saves.path()).is_err());
    assert!(restore(&outside, saves.path()).is_err());
    assert!(b.join("level.dat").is_file());
    delete_forever(&item, saves.path()).unwrap();
    assert!(list_trash(saves.path()).is_empty());
    assert!(!world.exists());
}

#[test]
fn legacy_trash_entries_without_a_note_are_listed_and_restorable() {
    let saves = tempfile::tempdir().unwrap();
    let dir = make_world(&saves.path().join(".trash"), "Old-1234", "Legacy");
    assert!(dir.is_dir());
    let item = list_trash(saves.path()).remove(0);
    assert_eq!((item.folder.as_str(), item.deleted_at), ("Old", 1234));
    assert_eq!(item.name, "Legacy");
    assert_eq!(restore(&item, saves.path()).unwrap(), "Old");
}

#[test]
fn favorites_persist_and_sort_first() {
    let dir = tempfile::tempdir().unwrap();
    let file = favorites::favorites_file(dir.path());
    let mut fav = Favorites::load(&file);
    assert!(fav.is_empty());
    assert!(fav.toggle("b", &file).unwrap());
    assert!(Favorites::load(&file).contains("b"));
    let mk = |f: &str| World {
        folder: f.into(),
        name: f.into(),
        path: PathBuf::new(),
        last_played: None,
        size: 0,
        icon: None,
    };
    let mut ws = vec![mk("a"), mk("b"), mk("c")];
    sort_favorites_first(&mut ws, &fav);
    let order: Vec<&str> = ws.iter().map(|w| w.folder.as_str()).collect();
    assert_eq!(order, ["b", "a", "c"]);
    fav.rename("b", "b (2)", &file).unwrap();
    assert!(Favorites::load(&file).contains("b (2)"));
    assert!(!fav.toggle("b (2)", &file).unwrap());
    assert!(Favorites::load(&file).is_empty());
}

#[test]
fn a_favorite_world_cannot_be_trashed_until_unstarred() {
    let saves = tempfile::tempdir().unwrap();
    make_world(saves.path(), "Keep", "Keep");
    let file = saves.path().join("fav.json");
    let mut favs = Favorites::load(&file);
    favs.toggle("Keep", &file).unwrap();
    let world = find(saves.path(), "Keep");
    assert!(trash_unless_favorite(&world, saves.path(), "x", &favs).is_err());
    assert!(saves.path().join("Keep").is_dir());
    assert!(list_trash(saves.path()).is_empty());
    favs.toggle("Keep", &file).unwrap();
    trash_unless_favorite(&world, saves.path(), "x", &favs).unwrap();
    assert!(!saves.path().join("Keep").exists());
    assert_eq!(list_trash(saves.path()).len(), 1);
}
