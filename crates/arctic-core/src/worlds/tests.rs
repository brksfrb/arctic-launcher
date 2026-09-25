use std::io::Write;

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
    trash(&world, saves.path()).unwrap();
    assert!(list(saves.path()).is_empty());
    assert_eq!(
        fs::read_dir(saves.path().join(".trash")).unwrap().count(),
        1
    );
}

#[test]
fn folder_names_are_safe() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(free_name(dir.path(), "a/b:c"), "a_b_c");
    assert_eq!(free_name(dir.path(), "  ..  "), "World");
}
