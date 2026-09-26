//! Compare one stream vs parallel ranges for a big file:
//! `cargo run --release -p arctic-core --example download_bench`
use std::time::Instant;

use arctic_core::net::{DownloadJob, download_all};

fn main() {
    let manifest: serde_json::Value = arctic_core::net::get_json(
        "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json",
    )
    .unwrap();
    let id = manifest["latest"]["release"].as_str().unwrap().to_owned();
    let entry = manifest["versions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["id"] == id.as_str())
        .unwrap();
    let version: serde_json::Value =
        arctic_core::net::get_json(entry["url"].as_str().unwrap()).unwrap();
    let client = &version["downloads"]["client"];
    let (url, sha1, size) = (
        client["url"].as_str().unwrap().to_owned(),
        client["sha1"].as_str().unwrap().to_owned(),
        client["size"].as_u64().unwrap(),
    );
    let dir = std::env::temp_dir().join("arctic-bench");
    let _ = std::fs::remove_dir_all(&dir);
    for (label, known_size) in [("single stream", None), ("parallel ranges", Some(size))] {
        let dest = dir.join(format!("{}.jar", label.replace(' ', "-")));
        let job = DownloadJob {
            url: url.clone(),
            dest: dest.clone(),
            sha1: Some(sha1.clone()),
            size: known_size,
            lzma: None,
        };
        let t = Instant::now();
        download_all("bench", vec![job], &|_| {}).unwrap();
        let secs = t.elapsed().as_secs_f64();
        println!(
            "{id} client ({:.1} MB), {label}: {secs:.2}s, {:.1} MB/s",
            size as f64 / 1e6,
            size as f64 / 1e6 / secs
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}
