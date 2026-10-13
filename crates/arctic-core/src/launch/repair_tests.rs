//! Exercise the real two-pass preparation with local metadata and a local download server.
//! No Java process, user data or public network service is used.

use super::*;
use crate::instances::Loader;
use crate::storage::save_json;
use crate::versions::VersionKind;
use serde_json::json;
use sha1::{Digest, Sha1};
use std::io::{BufRead, BufReader, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

fn sha1(bytes: &[u8]) -> String {
    hex::encode(Sha1::digest(bytes))
}

fn prepare_with_damaged_library(request_repair: bool, library: &str, failed_download: bool) {
    let temp = tempfile::tempdir().unwrap();
    let dirs = DataDirs::new(temp.path());
    dirs.ensure().unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let stopped = Arc::new(AtomicBool::new(false));
    let stop_server = stopped.clone();
    let server = std::thread::spawn(move || {
        let mut downloads = 0;
        while !stop_server.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    stream.set_nonblocking(false).unwrap();
                    stream
                        .set_read_timeout(Some(Duration::from_secs(2)))
                        .unwrap();
                    let mut reader = BufReader::new(&mut stream);
                    loop {
                        let mut line = String::new();
                        if reader.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
                            break;
                        }
                    }
                    let body = if failed_download { "wrong" } else { "right" };
                    write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\n{body}"
                    )
                    .unwrap();
                    downloads += 1;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(e) => panic!("local server: {e}"),
            }
        }
        downloads
    });

    let index = br#"{"objects":{}}"#;
    fs::create_dir_all(dirs.assets().join("indexes")).unwrap();
    fs::write(dirs.assets().join("indexes/test.json"), index).unwrap();
    let vanilla = json!({
        "id": "1.21.4", "mainClass": "net.minecraft.client.main.Main",
        "arguments": {"jvm": ["-cp", "${classpath}"], "game": []},
        "javaVersion": {"component": "java-runtime-delta", "majorVersion": 21},
        "assetIndex": {"id": "test", "url": url, "sha1": sha1(index), "size": index.len()},
        "downloads": {"client": {"url": url, "sha1": sha1(b"right"), "size": 5}},
        "libraries": []
    });
    let loader_id = "fabric-loader-0.19.5-1.21.4";
    let profile = json!({
        "id": loader_id, "inheritsFrom": "1.21.4",
        "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
        "libraries": [{"name": library, "url": url, "sha1": sha1(b"right"), "size": 5}]
    });
    for (id, metadata) in [("1.21.4", &vanilla), (loader_id, &profile)] {
        fs::create_dir_all(dirs.version_dir(id)).unwrap();
        save_json(&dirs.version_dir(id).join(format!("{id}.json")), metadata).unwrap();
        fs::write(dirs.version_dir(id).join(format!("{id}.jar")), b"right").unwrap();
    }
    let jar = dirs
        .libraries()
        .join(crate::versions::maven_path(library).unwrap());
    fs::create_dir_all(jar.parent().unwrap()).unwrap();
    fs::write(&jar, b"wrong").unwrap(); // Correct size, wrong content.
    let java = temp.path().join("java.exe");
    fs::write(&java, b"not executed").unwrap();
    let instance = Instance {
        id: "repair-test".into(),
        loader: Loader::Fabric {
            version: "0.19.5".into(),
        },
        arctic_mod: false,
        performance: false,
        own_settings: true,
        java_path: Some(java),
        ..Instance::vanilla_default()
    };
    let entry = VersionEntry {
        id: "1.21.4".into(),
        kind: VersionKind::Release,
        url,
        release_time: String::new(),
        sha1: crate::net::sha1_file(&dirs.version_dir("1.21.4").join("1.21.4.json")).unwrap(),
    };
    let account = crate::auth::offline::create("RepairTest").unwrap();
    let settings = Settings::default();
    if request_repair {
        files::request_repair(&dirs);
    }
    let result = prepare(
        &LaunchRequest {
            dirs: &dirs,
            version: &entry,
            instance: &instance,
            account: &account,
            settings: &settings,
            bridge: None,
            copy: 0,
            quick_play: None,
        },
        &|_| {},
    );
    stopped.store(true, Ordering::Relaxed);
    let downloads = server.join().unwrap();
    if failed_download {
        assert!(matches!(result, Err(Error::Checksum(_))), "{result:?}");
        assert!(downloads > 0);
        assert!(!jar.exists(), "do not keep the failed replacement");
        assert!(
            dirs.cache().join("repair-libraries").is_file(),
            "a failed preparation must retain its repair request"
        );
        return;
    }
    let plan = result.unwrap();
    assert_eq!(
        fs::read(&jar).unwrap(),
        b"right",
        "loader library must be repaired after the vanilla pass"
    );
    assert_eq!(downloads, 1, "fetch only the damaged library");
    assert!(
        plan.args
            .iter()
            .any(|arg| arg.contains(jar.to_str().unwrap()))
    );
    assert!(
        !dirs.cache().join("repair-libraries").exists(),
        "clear repair only after full preparation succeeds"
    );
}

#[test]
fn a_requested_repair_reaches_loader_libraries_after_vanilla() {
    // Not a proactively checked bootstrap library: this requires the repair request to survive.
    prepare_with_damaged_library(true, "example:loader-dependency:1", false);
}

#[test]
fn damaged_mixin_is_repaired_on_play_even_without_a_previous_crash_request() {
    prepare_with_damaged_library(false, "net.fabricmc:sponge-mixin:0.17.4+mixin.0.8.7", false);
}

#[test]
fn a_failed_loader_download_keeps_repair_pending() {
    prepare_with_damaged_library(true, "example:loader-dependency:1", true);
}
