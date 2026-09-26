use std::io::Write;
use std::path::Path;

use serde_json::json;

use super::*;
use crate::loaders::LoaderKind;

/// A jar holding `files` (name, content).
fn jar(path: &Path, files: &[(&str, &str)]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    for (name, text) in files {
        zip.start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(text.as_bytes()).unwrap();
    }
    zip.finish().unwrap();
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn fabric_mod(id: &str, version: &str, mc: &str) -> String {
    json!({"schemaVersion": 1, "id": id, "version": version, "depends": {"minecraft": mc}})
        .to_string()
}

#[test]
fn reads_each_instance_format() {
    let root = tempfile::tempdir().unwrap();
    let r = root.path();
    // Prism / MultiMC
    write(
        &r.join("prism/instance.cfg"),
        "InstanceType=OneSix\nname=Fabric Fun\nOverrideMemory=true\nMaxMemAlloc=6144\n",
    );
    write(
        &r.join("prism/mmc-pack.json"),
        &json!({"components": [{"uid": "net.minecraft", "version": "1.21.4"}, {"uid": "net.fabricmc.fabric-loader", "version": "0.16.10"}]}).to_string(),
    );
    std::fs::create_dir_all(r.join("prism/.minecraft")).unwrap();
    // CurseForge
    write(
        &r.join("cf/minecraftinstance.json"),
        &json!({"name": "Big Pack", "gameVersion": "1.20.1", "baseModLoader": {"name": "forge-47.2.0"}, "isMemoryOverride": true, "allocatedMemory": 8192}).to_string(),
    );
    // ATLauncher
    write(
        &r.join("atl/instance.json"),
        &json!({"id": "1.21.1", "launcher": {"name": "Neo", "loaderVersion": {"type": "NeoForge", "version": "21.1.234"}, "maximumMemory": 4096}}).to_string(),
    );
    std::fs::create_dir_all(r.join("atl/disabledmods")).unwrap();
    // GDLauncher
    write(
        &r.join("gdl/instance.json"),
        &json!({"name": "Quilty", "game_configuration": {"version": {"release": "1.20.4", "modloaders": [{"type": "Quilt", "version": "0.26.0"}]}, "memory": {"min_mb": 1024, "max_mb": 3072}}}).to_string(),
    );
    let scan = scan_folder(r);
    let by = |name: &str| {
        scan.instances
            .iter()
            .find(|f| f.name == name)
            .unwrap_or_else(|| panic!("{name} missing"))
    };
    let prism = by("Fabric Fun");
    assert_eq!(prism.game_version, "1.21.4");
    assert_eq!(
        prism.loader,
        Some(FoundLoader {
            kind: LoaderKind::Fabric,
            version: Some("0.16.10".into())
        })
    );
    assert_eq!(prism.memory_mb, Some(6144));
    assert!(prism.game_dir.ends_with(".minecraft"));
    let cf = by("Big Pack");
    assert_eq!(
        cf.loader.as_ref().unwrap().version.as_deref(),
        Some("47.2.0")
    );
    assert_eq!(cf.memory_mb, Some(8192));
    let atl = by("Neo");
    assert_eq!(atl.loader.as_ref().unwrap().kind, LoaderKind::NeoForge);
    assert!(atl.disabled_mods_dir.is_some());
    let gdl = by("Quilty");
    assert_eq!(gdl.loader.as_ref().unwrap().kind, LoaderKind::Quilt);
    assert!(gdl.game_dir.ends_with("instance"));
}

#[test]
fn tells_what_a_version_folder_runs() {
    let root = tempfile::tempdir().unwrap();
    let versions = root.path().join("versions");
    // Vanilla, with a BOM (TLauncher writes one).
    write(
        &versions.join("1.20.1/1.20.1.json"),
        &format!(
            "\u{feff}{}",
            json!({"id": "1.20.1", "mainClass": "net.minecraft.client.main.Main", "downloads": {"client": {"sha1": "abc"}}})
        ),
    );
    // A merged TLauncher Forge version: no name fields, only libraries.
    write(
        &versions.join("Forge 1.20.1/Forge 1.20.1.json"),
        &json!({"id": "Forge 1.20.1", "mainClass": "cpw.mods.bootstraplauncher.BootstrapLauncher", "downloads": {"client": {"sha1": "abc"}},
                "libraries": [{"name": "net.minecraftforge:fmlloader:1.20.1-47.4.0"}]}).to_string(),
    );
    // NeoForge named only in the extra file.
    write(
        &versions.join("Pack/Pack.json"),
        &json!({"id": "Pack", "mainClass": "x"}).to_string(),
    );
    write(
        &versions.join("Pack/TLauncherAdditional.json"),
        &json!({"additionalFiles": [{"path": "libraries/net/neoforged/neoforge/21.4.157/neoforge-21.4.157-client.jar"},
                                    {"path": "libraries/net/minecraft/client/1.21.4-20241203.161809/client.jar"}]}).to_string(),
    );
    // Official-launcher Fabric: inherits from vanilla.
    write(
        &versions.join("fabric-loader-0.16.10-1.20.1/fabric-loader-0.16.10-1.20.1.json"),
        &json!({"id": "fabric-loader-0.16.10-1.20.1", "inheritsFrom": "1.20.1", "libraries": [{"name": "net.fabricmc:fabric-loader:0.16.10"}]}).to_string(),
    );
    let vanilla = detect::vanilla_by_client(&versions);
    let info = |id: &str| detect::version_info(&versions, id, &vanilla).unwrap();
    let forge = info("Forge 1.20.1");
    assert_eq!(forge.game_version, "1.20.1");
    assert_eq!(forge.loader.unwrap().version.as_deref(), Some("47.4.0"));
    let pack = info("Pack");
    assert_eq!(pack.game_version, "1.21.4");
    assert_eq!(
        pack.loader.unwrap(),
        FoundLoader {
            kind: LoaderKind::NeoForge,
            version: Some("21.4.157".into())
        }
    );
    let fabric = info("fabric-loader-0.16.10-1.20.1");
    assert_eq!(fabric.game_version, "1.20.1");
    assert_eq!(fabric.loader.unwrap().kind, LoaderKind::Fabric);
    assert_eq!(info("1.20.1").loader, None);
}

fn shared_found(dir: &Path, kind: LoaderKind) -> Found {
    Found {
        launcher: Launcher::Official,
        name: "test".into(),
        game_version: "1.21.4".into(),
        loader: Some(FoundLoader {
            kind,
            version: None,
        }),
        game_dir: dir.to_path_buf(),
        mods_dir: None,
        shared: true,
        memory_mb: None,
        extra_jars: Vec::new(),
        disabled_mods_dir: None,
        extra_dirs: Vec::new(),
        into_vanilla: false,
        notes: Vec::new(),
    }
}

#[test]
fn shared_folders_bring_only_fitting_mods_and_no_secrets() {
    let root = tempfile::tempdir().unwrap();
    let g = root.path();
    jar(
        &g.join("mods/fab.jar"),
        &[("fabric.mod.json", &fabric_mod("fab", "1.0", "*"))],
    );
    jar(
        &g.join("mods/frg.jar"),
        &[("META-INF/mods.toml", "[[mods]]\nmodId=\"frg\"\n")],
    );
    // Made for both: stays for either loader.
    jar(
        &g.join("mods/both.jar"),
        &[
            ("fabric.mod.json", &fabric_mod("both", "1.0", "*")),
            ("META-INF/mods.toml", "[[mods]]\nmodId=\"both\"\n"),
        ],
    );
    jar(
        &g.join("mods/optifine.jar"),
        &[("optifine/Config.class", "x")],
    );
    write(&g.join("options.txt"), "fov:0.5");
    write(&g.join("launcher_accounts.json"), "{\"secret\": 1}");
    write(&g.join("saves/World/level.dat"), "x");
    let sizes = measure(&shared_found(g, LoaderKind::Fabric));
    assert_eq!(sizes.counts.get(&Category::Mods), Some(&3));
    assert_eq!(sizes.other_loader, vec!["frg.jar".to_owned()]);
    assert_eq!(
        sizes.counts.get(&Category::Settings),
        Some(&1),
        "only options.txt"
    );
    assert_eq!(
        sizes.counts.get(&Category::Worlds),
        None,
        "worlds come with the Vanilla entry"
    );
}

#[test]
fn own_folders_skip_launcher_files_and_secrets() {
    let root = tempfile::tempdir().unwrap();
    let g = root.path();
    for f in [
        "options.txt",
        "servers.dat",
        "instance.cfg",
        "mmc-pack.json",
        "usercache.json",
        "my_token_cache.json",
        "latest.log",
    ] {
        write(&g.join(f), "x");
    }
    write(&g.join("config/mod.toml"), "a=1");
    write(&g.join("logs/latest.log"), "x");
    write(&g.join("saves/W/level.dat"), "x");
    write(&g.join("saves/W/session.lock"), "x");
    let mut f = shared_found(g, LoaderKind::Fabric);
    f.shared = false;
    let sizes = measure(&f);
    // options.txt, servers.dat, config/
    assert_eq!(sizes.counts.get(&Category::Settings), Some(&3));
    assert_eq!(sizes.counts.get(&Category::Worlds), Some(&1));
    assert_eq!(
        sizes.bytes.get(&Category::Worlds),
        Some(&1),
        "session.lock stays behind"
    );
}

#[test]
fn keeps_the_newest_of_duplicate_mods() {
    let root = tempfile::tempdir().unwrap();
    let a = root.path().join("fabric-api-0.140.0.jar");
    let b = root.path().join("fabric-api-0.141.3.jar");
    let c = root.path().join("other.jar");
    jar(
        &a,
        &[(
            "fabric.mod.json",
            &fabric_mod("fabric-api", "0.140.0+1.21.11", "*"),
        )],
    );
    jar(
        &b,
        &[(
            "fabric.mod.json",
            &fabric_mod("fabric-api", "0.141.3+1.21.11", "*"),
        )],
    );
    jar(&c, &[("fabric.mod.json", &fabric_mod("other", "1", "*"))]);
    let kept = detect::newest_only(vec![
        (a, "a".into()),
        (b.clone(), "b".into()),
        (c.clone(), "c".into()),
    ]);
    let paths: Vec<_> = kept.into_iter().map(|(p, _)| p).collect();
    assert_eq!(paths, vec![b, c]);
}

#[test]
fn reads_the_loaders_own_description_of_a_jar() {
    let root = tempfile::tempdir().unwrap();
    let j = root.path().join("multi.jar");
    jar(
        &j,
        &[
            ("fabric.mod.json", &fabric_mod("multi", "1", ">=1.21.8")),
            (
                "META-INF/mods.toml",
                "[[dependencies.multi]]\nmodId=\"minecraft\"\nversionRange=\"[1.21,1.22)\"\n",
            ),
        ],
    );
    assert_eq!(
        ranges::game_fits(&j, "1.21.4", LoaderKind::Fabric),
        Some(false)
    );
    assert_eq!(
        ranges::game_fits(&j, "1.21.4", LoaderKind::Forge),
        Some(true)
    );
}

#[test]
fn finds_missing_required_mods_counting_bundled_ones() {
    let root = tempfile::tempdir().unwrap();
    let mods = root.path();
    // `a` needs `lib` (bundled inside `a`) and `gone` (nowhere).
    let mut nested = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut nested));
        zip.start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(fabric_mod("lib", "1", "*").as_bytes())
            .unwrap();
        zip.finish().unwrap();
    }
    let a = json!({"schemaVersion": 1, "id": "a", "version": "1", "depends": {"lib": "*", "gone": "*", "fabricloader": "*", "minecraft": "*"},
                   "jars": [{"file": "META-INF/jars/lib.jar"}]}).to_string();
    {
        let mut zip = zip::ZipWriter::new(std::fs::File::create(mods.join("a.jar")).unwrap());
        zip.start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(a.as_bytes()).unwrap();
        zip.start_file(
            "META-INF/jars/lib.jar",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(&nested).unwrap();
        zip.finish().unwrap();
    }
    // A Forge mod whose `fabric.mod.json` requirements don't count on Forge.
    jar(
        &mods.join("forge.jar"),
        &[
            (
                "fabric.mod.json",
                &json!({"schemaVersion": 1, "id": "f", "version": "1", "depends": {"fabric": "*"}})
                    .to_string(),
            ),
            (
                "META-INF/mods.toml",
                "[[mods]]\nmodId=\"f\"\n[[dependencies.f]]\nmodId=\"needed\"\nmandatory=true\nside=\"BOTH\"\n[[dependencies.f]]\nmodId=\"serveronly\"\nmandatory=true\nside=\"SERVER\"\n",
            ),
        ],
    );
    let fabric: Vec<String> = deps::missing(mods, LoaderKind::Fabric)
        .into_iter()
        .map(|m| m.id)
        .collect();
    assert!(
        fabric.contains(&"gone".to_owned()) && !fabric.contains(&"lib".to_owned()),
        "{fabric:?}"
    );
    let forge: Vec<String> = deps::missing(mods, LoaderKind::Forge)
        .into_iter()
        .map(|m| m.id)
        .collect();
    assert_eq!(forge, vec!["needed".to_owned()]);
}

#[test]
fn converts_labymod_and_lunar_huds() {
    let root = tempfile::tempdir().unwrap();
    let configs = root.path().join("configs");
    write(
        &configs.join("labymod/hud/default.json"),
        &json!({"configs": {
            "fps": {"enabled": true, "areaIdentifier": "TOP_LEFT", "x": 2.0, "y": 2.0, "scale": 1.0},
            "click_test": {"enabled": true, "areaIdentifier": "TOP_LEFT", "x": 2.0, "y": 2.0},
            "ping": {"enabled": true, "areaIdentifier": "TOP_LEFT", "x": 0.0, "y": 0.0},
            "speed": {"enabled": true, "areaIdentifier": "TOP_RIGHT", "x": -4.0, "y": 10.0, "scale": 9.0},
            "helmet": {"enabled": true, "dropzoneId": "item_top_left"},
            "spotify": {"enabled": true}
        }}).to_string(),
    );
    write(
        &configs.join("labymod/settings.json"),
        &json!({"ingame": {"zoom": {"zoomKey": "LEFT_ALT"}}}).to_string(),
    );
    let mut scan = Scan::default();
    hud::labymod(&configs, &mut scan);
    let c = &scan.clients[0];
    let h = &c.settings.values["hud"];
    assert_eq!(h["fps"]["placed"], true);
    assert_eq!(h["cps"]["placed"], false, "same spot as fps");
    assert_eq!(h["ping"]["placed"], false, "never moved");
    assert_eq!(
        (h["speed"]["ax"].as_i64(), h["speed"]["dx"].as_i64()),
        (Some(2), Some(4))
    );
    assert_eq!(h["speed"]["scale"], 2.5);
    assert_eq!(h["armor"]["enabled"], true);
    assert_eq!(h["target"]["enabled"], false);
    assert_eq!(c.settings.values["zoomKey"], "key.keyboard.left.alt");
    assert!(c.notes[0].contains("spotify"));

    let lunar = root.path().join("lunar");
    write(
        &lunar.join("profile_manager.json"),
        &json!([{"name": "Default"}, {"name": "../escape"}]).to_string(),
    );
    write(
        &lunar.join("Default/mods.json"),
        &json!({"FPS": {"enabled": true, "x": 10.0, "y": 6.0, "position": "topLeft"},
                "COORDINATES": {"x": 10.0, "y": 31.0, "position": "topLeft"},
                "ARMORSTATUS": {"options": {}},
                "CLOCK": {"enabled": false}})
        .to_string(),
    );
    let mut scan = Scan::default();
    hud::lunar(&lunar, &mut scan);
    assert_eq!(scan.clients.len(), 1);
    let c = &scan.clients[0];
    assert_eq!(c.guesses(), vec!["coords".to_owned()]);
    let chosen = c.with_choices(&["armor".into()]);
    assert_eq!(chosen.values["hud"]["armor"]["enabled"], true);
    assert_eq!(chosen.values["hud"]["coords"]["enabled"], false);
    assert_eq!(chosen.values["hud"]["clock"]["enabled"], false);
}
