//! Embeds the generated snowflake icon and version info into the Windows exe.

use std::path::PathBuf;

include!("src/icon_raster.rs");

const ICO_SIZES: [u32; 6] = [16, 24, 32, 48, 64, 256];

fn main() {
    println!("cargo:rerun-if-changed=src/icon_raster.rs");
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR is set by cargo"));
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
