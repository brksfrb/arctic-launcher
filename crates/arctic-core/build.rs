//! Embeds the Arctic Client jars listed in `mod/targets.json`: one jar per
//! build target, looked up by every Minecraft version it covers.

use std::fmt::Write as _;
use std::path::Path;

fn main() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mod_dir = manifest.join("../../mod");
    let targets_path = mod_dir.join("targets.json");
    println!("cargo:rerun-if-changed={}", targets_path.display());
    let text = std::fs::read_to_string(&targets_path).expect("mod/targets.json");
    let targets: serde_json::Value = serde_json::from_str(&text).expect("valid targets.json");

    let mut code =
        String::from("/// (Minecraft version, jar) for every version the client covers.\n");
    code.push_str("const JARS: &[(&str, &[u8])] = &[\n");
    for target in targets.as_array().expect("a list of targets") {
        let build = target["build"].as_str().expect("build version");
        let jar = mod_dir.join("dist").join(format!("arctic-mod-{build}.jar"));
        println!("cargo:rerun-if-changed={}", jar.display());
        assert!(
            jar.exists(),
            "missing {} (run python mod/build.py)",
            jar.display()
        );
        for covered in target["covers"].as_array().expect("covers") {
            let covered = covered.as_str().expect("version");
            writeln!(code, "    ({covered:?}, JAR_{}),", ident(build)).unwrap();
        }
    }
    code.push_str("];\n");
    for target in targets.as_array().unwrap() {
        let build = target["build"].as_str().unwrap();
        let path = mod_dir
            .join("dist")
            .join(format!("arctic-mod-{build}.jar"))
            .canonicalize()
            .unwrap();
        writeln!(
            code,
            "const JAR_{}: &[u8] = include_bytes!({:?});",
            ident(build),
            path.display().to_string()
        )
        .unwrap();
    }
    let out = Path::new(&std::env::var("OUT_DIR").unwrap()).join("arctic_jars.rs");
    std::fs::write(out, code).unwrap();
}

fn ident(version: &str) -> String {
    version.replace('.', "_")
}
