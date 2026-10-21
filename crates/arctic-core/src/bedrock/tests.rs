use std::fs;
use std::io::Write;
use std::path::Path;

use super::*;

fn player(dir: &Path) -> Player {
    Player {
        id: "123".into(),
        com_mojang: dir.to_path_buf(),
    }
}

fn make_world(dir: &Path, name: &str) {
    fs::create_dir_all(dir.join("db")).unwrap();
    fs::write(dir.join("level.dat"), [8, 0, 0, 0, 0, 0, 0, 0]).unwrap();
    fs::write(dir.join("levelname.txt"), name).unwrap();
    fs::write(dir.join("db").join("CURRENT"), "MANIFEST-000001").unwrap();
}

fn zip_file(path: &Path, files: &[(&str, &[u8])]) {
    let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
    for (name, data) in files {
        zip.start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(data).unwrap();
    }
    zip.finish().unwrap();
}

#[test]
fn finds_shared_and_account_folders() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Minecraft Bedrock");
    for user in ["Shared", "2535400000000001", "2535400000000002"] {
        fs::create_dir_all(root.join("Users").join(user).join("games/com.mojang")).unwrap();
    }
    // Played most recently: its worlds folder changed last.
    fs::create_dir_all(root.join("Users/2535400000000002/games/com.mojang/minecraftWorlds"))
        .unwrap();
    let legacy = dir.path().join("LocalState/games/com.mojang");
    fs::create_dir_all(&legacy).unwrap();

    let found = folders_in(Some(&root), Some(&legacy));
    let ids: Vec<&str> = found.players.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, ["2535400000000002", "2535400000000001", "UWP"]);
    assert_eq!(found.packs.as_ref().unwrap().id, "Shared");
    assert_eq!(found.players[0].label(), "Account …0002");
    assert_eq!(found.player(Some("UWP")).unwrap().id, "UWP");
    assert_eq!(found.player(Some("gone")).unwrap().id, "2535400000000002");

    let none = folders_in(Some(&dir.path().join("missing")), None);
    assert!(none.players.is_empty() && none.packs.is_none() && none.root.is_none());
}

#[test]
fn worlds_export_and_import_as_mcworld() {
    let dir = tempfile::tempdir().unwrap();
    let p = player(&dir.path().join("com.mojang"));
    make_world(&p.worlds_dir().join("AbCdEfGhIjKl"), "My World");
    let worlds = worlds::list(&p);
    assert_eq!(worlds.len(), 1);
    assert_eq!(worlds[0].name, "My World");
    assert!(worlds[0].size > 0);

    let file = dir.path().join("out/My World.mcworld");
    worlds::export(&worlds[0], &file).unwrap();
    let names: Vec<String> = archive::open(&file)
        .unwrap()
        .file_names()
        .map(str::to_owned)
        .collect();
    assert!(names.contains(&"level.dat".to_owned()));
    assert!(names.contains(&"db/CURRENT".to_owned()));

    assert_eq!(worlds::import(&file, &p).unwrap(), "My World");
    assert_eq!(worlds::list(&p).len(), 2);

    let backup = worlds::backup(&worlds[0], &dir.path().join("backups")).unwrap();
    assert!(backup.extension().is_some_and(|e| e == "mcworld"));

    worlds::rename(&worlds[0], "Renamed").unwrap();
    worlds::trash(&worlds[0], &p).unwrap();
    let left: Vec<String> = worlds::list(&p).into_iter().map(|w| w.name).collect();
    assert_eq!(left, ["My World"]);
    assert!(p.trash_dir().join("worlds").read_dir().unwrap().count() == 1);
}

#[test]
fn a_java_world_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let p = player(&dir.path().join("com.mojang"));
    let file = dir.path().join("java.zip");
    zip_file(
        &file,
        &[("World/level.dat", b"x"), ("World/region/r.0.0.mca", b"x")],
    );
    assert!(worlds::import(&file, &p).is_err());
    assert!(worlds::list(&p).is_empty());
}

#[test]
fn packs_import_from_mcpack_and_mcaddon() {
    let dir = tempfile::tempdir().unwrap();
    let p = player(&dir.path().join("com.mojang"));
    let uuid = "11111111-2222-3333-4444-555555555555";
    let rp = packs::test_manifest("resources", "pack.name", uuid);
    let lang = "pack.name=Shiny Textures §a\npack.description=Makes it shine\n";
    let mcpack = dir.path().join("shiny.mcpack");
    zip_file(
        &mcpack,
        &[
            ("manifest.json", rp.as_bytes()),
            ("texts/en_US.lang", lang.as_bytes()),
            ("subpacks/low/manifest.json", b"{}"),
        ],
    );
    assert_eq!(packs::import(&mcpack, &p).unwrap(), ["Shiny Textures"]);
    let list = packs::list(&p);
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].kind, packs::PackKind::Resource);
    assert_eq!(list[0].description, "Makes it shine");
    assert_eq!(list[0].version, "1.2.0");

    // The same pack again replaces it; an addon brings a behavior pack along.
    let bp = packs::test_manifest("data", "Mobs+", "99999999-2222-3333-4444-555555555555");
    let addon = dir.path().join("mobs.mcaddon");
    zip_file(
        &addon,
        &[
            ("Mobs RP/manifest.json", rp.as_bytes()),
            ("Mobs RP/texts/en_US.lang", lang.as_bytes()),
            ("Mobs BP/manifest.json", bp.as_bytes()),
        ],
    );
    let mut names = packs::import(&addon, &p).unwrap();
    names.sort();
    assert_eq!(names, ["Mobs+", "Shiny Textures"]);
    let list = packs::list(&p);
    assert_eq!(list.len(), 2);
    assert_eq!(list[1].kind, packs::PackKind::Behavior);

    packs::trash(&list[1], &p).unwrap();
    assert_eq!(packs::list(&p).len(), 1);

    let template = dir.path().join("template.mctemplate");
    zip_file(
        &template,
        &[(
            "manifest.json",
            packs::test_manifest("world_template", "T", uuid).as_bytes(),
        )],
    );
    assert!(
        packs::import(&template, &p)
            .unwrap_err()
            .to_string()
            .contains("template")
    );
}

#[test]
fn version_numbers() {
    assert_eq!(versions::from_package_version("1.26.5203.0"), "1.26.52.3");
    assert_eq!(versions::from_package_version("1.21.12004.0"), "1.21.120.4");
    assert_eq!(versions::normalize("1.26.52.03").unwrap(), "1.26.52.3");
    assert!(versions::normalize("Release").is_none());
    assert_eq!(versions::display("1.26.52.3", Edition::Release), "1.26.52");
    assert_eq!(
        versions::display("1.26.70.20", Edition::Preview),
        "1.26.70.20"
    );
    assert!(versions::compare("1.26.100.1", "1.26.52.3").is_gt());
}

#[test]
fn reads_both_version_lists() {
    let main = r#"{"file_version":1,
      "previewVersions":[{"version":"Preview 1.26.70.20","urls":["http://assets1.xboxlive.com/x/Microsoft.MinecraftWindowsBeta_1.26.7020.0_x64__8wekyb3d8bbwe.msixvc"],"timestamp":1,"md5":"a"}],
      "releaseVersions":[
        {"version":"Release 1.21.120.04","urls":["http://assets1.xboxlive.cn/a.msixvc","http://assets1.xboxlive.com/a.msixvc","http://evil.example/a.msixvc"],"timestamp":1,"md5":"b"},
        {"version":"Release 1.26.52.03","urls":["http://assets2.xboxlive.com/b.msixvc"],"timestamp":2,"md5":"c"},
        {"version":"Release 1.26.51.01","urls":["http://evil.example/c.msixvc"],"timestamp":2,"md5":"d"}]}"#;
    let c = versions::parse_main(main).unwrap();
    let ids: Vec<&str> = c.release.iter().map(|v| v.id.as_str()).collect();
    assert_eq!(ids, ["1.26.52.3", "1.21.120.4"]);
    assert_eq!(
        c.release[1].urls,
        [
            "http://assets1.xboxlive.com/a.msixvc",
            "http://assets1.xboxlive.cn/a.msixvc"
        ]
    );
    assert_eq!(c.latest(Edition::Preview).unwrap().id, "1.26.70.20");
    assert_eq!(c.find(Edition::Release, "1.26.52").unwrap().id, "1.26.52.3");
    assert_eq!(
        c.find(Edition::Release, "1.21.120.04").unwrap().id,
        "1.21.120.4"
    );
    assert!(c.find(Edition::Release, "1.26").is_none());

    let fallback = r#"{"release":{"1.21.120.4":["http://assets1.xboxlive.com/a.msixvc"],"1.26.52.3":["http://assets1.xboxlive.com/b.msixvc"]},"preview":{}}"#;
    let c = versions::parse_fallback(fallback).unwrap();
    assert_eq!(c.release[0].id, "1.26.52.3");
    assert!(c.preview.is_empty());
}

#[test]
fn reads_the_older_builds_and_adds_only_what_is_missing() {
    let text = r#"[["1.20.81.01","48f0d81e-0cc1-4e97-ab85-c9d3d24e848f","0","x64"],
        ["1.21.120.04","6f2ca544-e31c-4916-b888-bb2815be4c84","0","x64"],
        ["1.21.0.3","0052663e-3323-4fc9-9ead-84567336093a","1","x64"],
        ["1.20.50.3","58c5f0cd-09d7-4e99-a6b6-c3829fd62ac9","0","x86"],
        ["1.19.0.5","not a guid","0","x64"],
        ["1.21.120.20","c1872dc3-ddf8-4a91-844b-8dca1b681324","2","x64"]]"#;
    let legacy = versions::parse_legacy(text).unwrap();
    let ids: Vec<&str> = legacy.iter().map(|v| v.id.as_str()).collect();
    assert_eq!(ids, ["1.20.81.1", "1.21.120.4", "1.21.120.20"]);
    assert!(legacy.iter().all(|v| v.is_legacy() && v.urls.is_empty()));
    assert_eq!(legacy[2].edition, Edition::Preview);

    let main = r#"{"releaseVersions":[{"version":"Release 1.21.120.04","urls":["http://assets1.xboxlive.com/a.msixvc"]}]}"#;
    let mut c = versions::parse_main(main).unwrap();
    c.add_legacy(legacy);
    let ids: Vec<&str> = c.release.iter().map(|v| v.id.as_str()).collect();
    assert_eq!(ids, ["1.21.120.4", "1.20.81.1"]);
    assert!(!c.release[0].is_legacy(), "the GDK build wins a tie");
    assert!(c.release[1].is_legacy());
    assert_eq!(c.preview[0].id, "1.21.120.20");
    assert!(versions::parse_legacy("[]").is_err());
}
