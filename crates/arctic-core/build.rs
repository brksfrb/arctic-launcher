//! Embeds the Arctic Client: `mod/dist/arctic-client.pack` holds a jar for
//! every build target in `mod/targets.json` (see `mod/build.py`), and each
//! target is looked up by every Minecraft version it covers.

use std::fmt::Write as _;
use std::path::Path;

fn main() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mod_dir = manifest.join("../../mod");
    let targets_path = mod_dir.join("targets.json");
    let pack_path = mod_dir.join("dist/arctic-client.pack");
    println!("cargo:rerun-if-changed={}", targets_path.display());
    println!("cargo:rerun-if-changed={}", pack_path.display());
    let text = std::fs::read_to_string(&targets_path).expect("mod/targets.json");
    let targets: serde_json::Value = serde_json::from_str(&text).expect("valid targets.json");
    let packed = packed_builds(&pack_path);
    let pack_bytes = std::fs::read(&pack_path).unwrap_or_default();

    // Debug builds skip a target that isn't packed yet (a version being
    // added); release builds must have every one.
    let release = std::env::var("PROFILE").as_deref() == Ok("release");
    let mut code = String::from(
        "/// (Minecraft version, build target) for every version the client covers.\n\
         const COVERS: &[(&str, &str)] = &[\n",
    );
    for target in targets.as_array().expect("a list of targets") {
        let build = target["build"].as_str().expect("build version");
        if !packed.iter().any(|(b, _)| b == build) {
            assert!(
                !release,
                "Arctic Client {build} isn't in {} (run python mod/build.py)",
                pack_path.display()
            );
            println!("cargo:warning=skipping Arctic Client {build}: it isn't packed");
            continue;
        }
        for covered in target["covers"].as_array().expect("covers") {
            let covered = covered.as_str().expect("version");
            writeln!(code, "    ({covered:?}, {build:?}),").unwrap();
        }
    }
    code.push_str("];\n");
    // Read once here, so a launch needn't unpack anything to check the loader.
    code.push_str(
        "/// (build target, oldest Fabric Loader its fabric.mod.json accepts).\n\
         const MIN_LOADER: &[(&str, &str)] = &[\n",
    );
    for (build, loader) in &packed {
        if let Some(loader) = loader {
            writeln!(code, "    ({build:?}, {loader:?}),").unwrap();
        }
    }
    code.push_str("];\n");
    // Names this pack: an instance's jar stamped with it is already current.
    writeln!(code, "const PACK_ID: u64 = {};", fnv1a(&pack_bytes)).unwrap();
    match pack_path.canonicalize() {
        Ok(path) => writeln!(
            code,
            "const PACK: &[u8] = include_bytes!({:?});",
            path.display().to_string()
        ),
        Err(_) => writeln!(code, "const PACK: &[u8] = &[];"),
    }
    .unwrap();
    let out = Path::new(&std::env::var("OUT_DIR").unwrap()).join("arctic_jars.rs");
    std::fs::write(out, code).unwrap();
}

/// FNV-1a: a stable 64-bit hash (std's hashers aren't guaranteed stable).
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// The build targets inside the pack (none if there's no pack yet), each with
/// the oldest Fabric Loader its `fabric.mod.json` asks for.
fn packed_builds(path: &Path) -> Vec<(String, Option<String>)> {
    let Ok(bytes) = std::fs::read(path) else {
        return Vec::new();
    };
    let mut raw = Vec::new();
    lzma_rs::lzma_decompress(&mut std::io::Cursor::new(bytes), &mut raw)
        .expect("mod/dist/arctic-client.pack is damaged (run python mod/build.py --pack)");
    let magic = b"ARCTICPACK1\n";
    assert!(raw.starts_with(magic), "not an Arctic Client pack");
    let at = magic.len();
    let len = u32::from_le_bytes(raw[at..at + 4].try_into().unwrap()) as usize;
    let index: serde_json::Value =
        serde_json::from_slice(&raw[at + 4..at + 4 + len]).expect("pack index");
    let sizes: Vec<usize> = index["sizes"]
        .as_array()
        .map(|a| a.iter().map(|v| v.as_u64().unwrap_or(0) as usize).collect())
        .unwrap_or_default();
    let data = &raw[at + 4 + len..];
    let file = |n: usize| {
        let start: usize = sizes[..n].iter().sum();
        &data[start..start + sizes[n]]
    };
    let Some(jars) = index["jars"].as_object() else {
        return Vec::new();
    };
    jars.iter()
        .map(|(build, entries)| {
            let loader = entries
                .as_array()
                .into_iter()
                .flatten()
                .find(|e| e[0] == "fabric.mod.json")
                .and_then(|e| e[1].as_u64())
                .and_then(|n| serde_json::from_slice::<serde_json::Value>(file(n as usize)).ok())
                .and_then(|json| {
                    json.pointer("/depends/fabricloader")
                        .and_then(|v| v.as_str())
                        .map(|r| r.trim_start_matches(">=").trim().to_owned())
                });
            (build.clone(), loader)
        })
        .collect()
}
