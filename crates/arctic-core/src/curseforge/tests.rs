use std::io::Write;
use std::path::Path;

use serde_json::json;

use super::*;

fn make_zip(path: &Path, entries: &[(&str, &str)]) {
    let mut w = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    for (name, body) in entries {
        w.start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        w.write_all(body.as_bytes()).unwrap();
    }
    w.finish().unwrap();
}

fn cf_manifest() -> String {
    json!({
        "minecraft": {"version": "1.20.1", "modLoaders": [{"id": "forge-47.2.0", "primary": true}]},
        "manifestType": "minecraftModpack",
        "manifestVersion": 1,
        "name": "Big Pack",
        "files": [{"projectID": 238222, "fileID": 4000000, "required": true}],
        "overrides": "overrides"
    })
    .to_string()
}

fn refused(r: Result<()>) -> bool {
    matches!(r, Err(Error::Other(m)) if m == "Arctic doesn't support CurseForge. Look for this pack on Modrinth instead.")
}

#[test]
fn curseforge_zip_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let zip = dir.path().join("pack.zip");
    make_zip(
        &zip,
        &[
            ("manifest.json", &cf_manifest()),
            ("modlist.html", "<ul></ul>"),
            ("overrides/options.txt", "x"),
        ],
    );
    assert!(refused(check_file(&zip)));
}

#[test]
fn curseforge_zip_with_nested_manifest_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let zip = dir.path().join("pack.zip");
    make_zip(&zip, &[("Big Pack/manifest.json", &cf_manifest())]);
    assert!(refused(check_file(&zip)));
    let deep = dir.path().join("deep.zip");
    make_zip(&deep, &[("a/b/manifest.json", &cf_manifest())]);
    assert!(check_file(&deep).is_ok());
}

#[test]
fn curseforge_install_is_refused_before_anything_is_made() {
    let dir = tempfile::tempdir().unwrap();
    let zip = dir.path().join("pack.zip");
    make_zip(&zip, &[("manifest.json", &cf_manifest())]);
    let dirs = crate::storage::DataDirs::new(dir.path().join("data"));
    let err = crate::mods::modpack::install_file(&dirs, &zip, &|_| {}).unwrap_err();
    assert!(refused(Err(err)));
    assert!(!dir.path().join("data").join("instances").exists());
}

#[test]
fn curseforge_instance_folder_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let inst = dir.path().join("Big Pack");
    std::fs::create_dir_all(&inst).unwrap();
    std::fs::write(
        inst.join("minecraftinstance.json"),
        json!({"name": "Big Pack", "gameVersion": "1.20.1"}).to_string(),
    )
    .unwrap();
    assert!(refused(check_dir(&inst)));
    let scan = crate::migrate::scan_folder(&inst);
    assert!(scan.instances.is_empty());
    assert!(scan.curseforge_blocked);
    // The folder holding it too.
    let scan = crate::migrate::scan_folder(dir.path());
    assert!(scan.instances.is_empty());
    assert!(scan.curseforge_blocked);
}

#[test]
fn curseforge_links_are_refused() {
    for link in [
        "curseforge://install?addonId=238222&fileId=4000000",
        "CurseForge://install?addonId=1",
        " curseforge:foo",
        "https://www.curseforge.com/minecraft/modpacks/all-the-mods-9",
        "https://legacy.curseforge.com/minecraft/modpacks/x",
        "https://api.curseforge.com/v1/mods/1",
        "https://edge.forgecdn.net/files/1/2/x.zip",
        "https://mediafilez.forgecdn.net/files/1/2/x.zip",
    ] {
        assert!(refused(check_link(link)), "{link}");
    }
    for link in [
        "https://modrinth.com/modpack/fabulously-optimized",
        "fabulously-optimized",
        "https://notcurseforge.com/x",
        "https://example.com/?u=curseforge.com",
        "https://curseforge.com.evil.example/x",
    ] {
        assert!(check_link(link).is_ok(), "{link}");
    }
}

#[test]
fn modrinth_pack_is_not_curseforge() {
    let dir = tempfile::tempdir().unwrap();
    let pack = dir.path().join("p.mrpack");
    // Even one that also carries a CurseForge-looking manifest.
    make_zip(
        &pack,
        &[
            ("modrinth.index.json", "{}"),
            ("manifest.json", &cf_manifest()),
        ],
    );
    assert!(check_file(&pack).is_ok());
    assert!(crate::mods::modpack::is_modrinth_pack(&pack));
}

#[test]
fn plain_jar_and_generic_zips_pass() {
    let dir = tempfile::tempdir().unwrap();
    // A jar that came from CurseForge: its fabric.mod.json, even a manifest.json, doesn't matter.
    let jar = dir.path().join("sodium.jar");
    make_zip(
        &jar,
        &[
            ("fabric.mod.json", "{}"),
            ("META-INF/MANIFEST.MF", "Manifest-Version: 1.0"),
        ],
    );
    assert!(check_file(&jar).is_ok());
    let zip_of_jars = dir.path().join("jars.zip");
    make_zip(&zip_of_jars, &[("a.jar", "x"), ("b.jar", "y")]);
    assert!(check_file(&zip_of_jars).is_ok());
    let other_manifest = dir.path().join("other.zip");
    make_zip(
        &other_manifest,
        &[("manifest.json", r#"{"name":"web app","files":[]}"#)],
    );
    assert!(check_file(&other_manifest).is_ok());
    let not_a_zip = dir.path().join("x.txt");
    std::fs::write(&not_a_zip, "hello").unwrap();
    assert!(check_file(&not_a_zip).is_ok());
    assert!(check_file(&dir.path().join("missing.zip")).is_ok());
}
