//! Embeds the translations (`locales/`), and on Windows the generated snowflake icon and
//! version info, into the exe.

use std::path::{Path, PathBuf};

include!("src/icon_raster.rs");

const ICO_SIZES: [u32; 6] = [16, 24, 32, 48, 64, 256];

fn main() {
    println!("cargo:rerun-if-changed=src/icon_raster.rs");
    println!("cargo:rerun-if-changed=build.rs");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR is set by cargo"));
    embed_locales(&out_dir);
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let ico = out_dir.join("arctic.ico");
    if let Err(e) = std::fs::write(&ico, ico_bytes(&ICO_SIZES)) {
        println!("cargo:warning=could not write icon: {e}");
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon(&ico.to_string_lossy());
    res.set("ProductName", "Arctic Launcher");
    res.set("FileDescription", "Arctic Launcher");
    res.set("LegalCopyright", "GPL-3.0-or-later");
    // A missing or broken resource compiler used to only warn, which silently
    // shipped icon-less exes. Fail the build instead; ARCTIC_ALLOW_NO_ICON=1
    // opts back into the warning.
    println!("cargo:rerun-if-env-changed=ARCTIC_ALLOW_NO_ICON");
    if let Err(e) = res.compile() {
        if std::env::var("ARCTIC_ALLOW_NO_ICON").as_deref() == Ok("1") {
            println!("cargo:warning=could not embed exe icon: {e}");
        } else {
            panic!(
                "could not embed exe icon: {e}. Make sure a resource compiler works \
                 (llvm-rc in PATH and an existing TMPDIR for cargo-xwin builds), or set \
                 ARCTIC_ALLOW_NO_ICON=1 to build without an icon."
            );
        }
    }
}

/// Write `locales.rs`: `LOCALE_FILES`, the (language, file, contents) of every
/// `locales/<language>/<file>.json`, each pulled in with `include_str!`. Adding a language is
/// adding a folder; nothing else lists them.
fn embed_locales(out_dir: &Path) {
    println!("cargo:rerun-if-changed=locales");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("locales");
    let mut entries = Vec::new();
    let languages = std::fs::read_dir(&root)
        .map(|dir| dir.flatten().collect::<Vec<_>>())
        .unwrap_or_default();
    for language in languages {
        if !language.path().is_dir() {
            continue;
        }
        let code = language.file_name().to_string_lossy().into_owned();
        let mut files: Vec<PathBuf> = std::fs::read_dir(language.path())
            .map(|dir| dir.flatten().map(|e| e.path()).collect())
            .unwrap_or_default();
        files.retain(|p| p.extension().is_some_and(|ext| ext == "json"));
        files.sort();
        for file in files {
            let name = file
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let path = file.to_string_lossy().into_owned();
            entries.push(format!(
                "    ({code:?}, {name:?}, include_str!({path:?})),\n"
            ));
        }
    }
    entries.sort();
    let source = format!(
        "/// (language, file, contents) of every locales/<language>/<file>.json.\n\
         pub(crate) const LOCALE_FILES: &[(&str, &str, &str)] = &[\n{}];\n",
        entries.concat()
    );
    std::fs::write(out_dir.join("locales.rs"), source).expect("write locales.rs into OUT_DIR");
}
