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
    let good = jar_with_class("example/Library.class");
    let mut bad = good.clone();
    let payload = bad
        .windows(4)
        .position(|b| b == b"\xca\xfe\xba\xbe")
        .unwrap();
    bad[payload] ^= 1;
    prepare_with_library(
        LibraryTarget::Fabric,
        request_repair,
        library,
        failed_download,
        &bad,
        &good,
        true,
    );
}

enum LibraryTarget<'a> {
    Vanilla(&'a str),
    Fabric,
}

fn prepare_with_library(
    target: LibraryTarget,
    request_repair: bool,
    library: &str,
    failed_download: bool,
    existing: &[u8],
    replacement: &[u8],
    checksummed: bool,
) {
    let temp = tempfile::tempdir().unwrap();
    let dirs = DataDirs::new(temp.path());
    dirs.ensure().unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let stopped = Arc::new(AtomicBool::new(false));
    let stop_server = stopped.clone();
    let mut response = replacement.to_vec();
    if failed_download {
        response[0] ^= 1;
    }
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
                    write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        response.len()
                    )
                    .unwrap();
                    stream.write_all(&response).unwrap();
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
    let (version, loader) = match target {
        LibraryTarget::Vanilla(version) => (version, Loader::Vanilla),
        LibraryTarget::Fabric => (
            "1.21.4",
            Loader::Fabric {
                version: "0.19.5".into(),
            },
        ),
    };
    let client = jar_with_class("net/minecraft/client/main/Main.class");
    let mut vanilla = json!({
        "id": version, "mainClass": "net.minecraft.client.main.Main",
        "arguments": {"jvm": ["-cp", "${classpath}"], "game": []},
        "javaVersion": {"component": "java-runtime-delta", "majorVersion": 21},
        "assetIndex": {"id": "test", "url": url, "sha1": sha1(index), "size": index.len()},
        "downloads": {"client": {"url": url, "sha1": sha1(&client), "size": client.len()}},
        "libraries": []
    });
    let loader_id = "fabric-loader-0.19.5-1.21.4";
    let mut library_entry = json!({"name": library, "url": url});
    if matches!(target, LibraryTarget::Vanilla(_)) {
        library_entry = json!({
            "name": library,
            "downloads": {"artifact": {"url": url, "sha1": sha1(replacement), "size": replacement.len()}},
        });
        vanilla["libraries"] = json!([library_entry]);
    } else {
        if checksummed {
            library_entry["sha1"] = json!(sha1(replacement));
            library_entry["size"] = json!(replacement.len());
        }
        let profile = json!({
            "id": loader_id, "inheritsFrom": version,
            "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
            "libraries": [library_entry],
        });
        fs::create_dir_all(dirs.version_dir(loader_id)).unwrap();
        save_json(
            &dirs
                .version_dir(loader_id)
                .join(format!("{loader_id}.json")),
            &profile,
        )
        .unwrap();
    }
    fs::create_dir_all(dirs.version_dir(version)).unwrap();
    let version_json = dirs.version_dir(version).join(format!("{version}.json"));
    save_json(&version_json, &vanilla).unwrap();
    fs::write(
        dirs.version_dir(version).join(format!("{version}.jar")),
        client,
    )
    .unwrap();
    let jar = dirs
        .libraries()
        .join(crate::versions::maven_path(library).unwrap());
    fs::create_dir_all(jar.parent().unwrap()).unwrap();
    fs::write(&jar, existing).unwrap();
    let java = temp.path().join("java.exe");
    fs::write(&java, b"not executed").unwrap();
    let instance = Instance {
        id: "repair-test".into(),
        loader,
        arctic_mod: false,
        performance: false,
        own_settings: true,
        java_path: Some(java),
        ..Instance::vanilla_default()
    };
    let entry = VersionEntry {
        id: version.into(),
        kind: VersionKind::Release,
        url,
        release_time: String::new(),
        sha1: crate::net::sha1_file(&version_json).unwrap(),
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
        if checksummed {
            assert!(matches!(result, Err(Error::Checksum(_))), "{result:?}");
        } else {
            assert!(matches!(result, Err(Error::Other(_))), "{result:?}");
        }
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
        replacement,
        "repair the library before giving its path to Java"
    );
    assert_eq!(
        downloads,
        usize::from(existing != replacement || (request_repair && !checksummed)),
        "fetch only the damaged library, preserving a healthy loader on normal Play"
    );
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

#[test]
fn same_size_jar_with_a_damaged_zip_end_is_replaced_on_play() {
    let good = jar_with_class("org/apache/logging/log4j/core/Logger.class");
    let mut broken = good.clone();
    let end_record = broken.len() - 22;
    assert_eq!(&broken[end_record..end_record + 4], b"PK\x05\x06");
    broken[end_record..].fill(0);
    for target in [
        LibraryTarget::Vanilla("1.8.9"),
        LibraryTarget::Vanilla("1.21.1"),
        LibraryTarget::Fabric,
    ] {
        prepare_with_library(
            target,
            false,
            "org.apache.logging.log4j:log4j-core:2.22.1",
            false,
            &broken,
            &good,
            true,
        );
    }
}

fn jar_with_class(entry: &str) -> Vec<u8> {
    let mut jar = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    jar.start_file(
        entry,
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
    )
    .unwrap();
    jar.write_all(b"\xca\xfe\xba\xbe test class").unwrap();
    jar.finish().unwrap().into_inner()
}

#[test]
fn damaged_unhashed_loader_is_replaced_without_a_repair_request() {
    let good = jar_with_class("net/fabricmc/loader/impl/launch/knot/KnotClient.class");
    let wrong_class = jar_with_class("some/other/Class.class");
    let mut bad_crc = good.clone();
    let at = bad_crc
        .windows(4)
        .position(|b| b == b"\xca\xfe\xba\xbe")
        .unwrap();
    bad_crc[at] ^= 1;
    for broken in [
        b"".as_slice(),
        b"not a jar".as_slice(),
        &good[..good.len() / 2],
        &wrong_class,
        &bad_crc,
    ] {
        prepare_with_library(
            LibraryTarget::Fabric,
            false,
            "net.fabricmc:fabric-loader:0.20.1",
            false,
            broken,
            &good,
            false,
        );
    }
}

#[test]
fn a_healthy_unhashed_loader_is_not_downloaded_again() {
    let good = jar_with_class("net/fabricmc/loader/impl/launch/knot/KnotClient.class");
    prepare_with_library(
        LibraryTarget::Fabric,
        false,
        "net.fabricmc:fabric-loader:0.20.1",
        false,
        &good,
        &good,
        false,
    );
}

#[test]
fn a_checksummed_loader_is_repaired_without_a_repair_request() {
    let good = jar_with_class("net/fabricmc/loader/impl/launch/knot/KnotClient.class");
    let broken = vec![0; good.len()];
    prepare_with_library(
        LibraryTarget::Fabric,
        false,
        "net.fabricmc:fabric-loader:0.20.1",
        false,
        &broken,
        &good,
        true,
    );
}

#[test]
fn an_invalid_unhashed_replacement_stops_launch_and_keeps_repair_pending() {
    let good = jar_with_class("net/fabricmc/loader/impl/launch/knot/KnotClient.class");
    prepare_with_library(
        LibraryTarget::Fabric,
        true,
        "net.fabricmc:fabric-loader:0.20.1",
        true,
        b"broken",
        &good,
        false,
    );
}

#[test]
fn main_class_not_found_requests_repair_and_explains_next_play() {
    let temp = tempfile::tempdir().unwrap();
    let dirs = DataDirs::new(temp.path());
    let error =
        "Error: Could not find or load main class net.fabricmc.loader.impl.launch.knot.KnotClient";
    let cause = "Caused by: java.lang.ClassNotFoundException: net.fabricmc.loader.impl.launch.knot.KnotClient";
    for output in [
        error.to_owned(),
        cause.to_owned(),
        format!("{error}\n{cause}"),
    ] {
        let diagnosis =
            crate::crash_conflicts::diagnose_exit_in(&output, 1, &temp.path().join("mods"))
                .unwrap();
        assert_eq!(
            diagnosis.title,
            "A game file was damaged (the Fabric loader)"
        );
        assert!(
            diagnosis
                .detail
                .contains("Arctic will repair it on the next Play.")
        );
        assert!(diagnosis.mods.is_empty());
        assert!(diagnosis.repair);
        // The exit handler uses this flag to enqueue the existing repair request.
        if diagnosis.repair {
            files::request_repair(&dirs);
        }
        assert!(dirs.cache().join("repair-libraries").is_file());
        files::LibraryRepair::new(&dirs).unwrap().finish();
        assert!(!dirs.cache().join("repair-libraries").exists());
    }
}
