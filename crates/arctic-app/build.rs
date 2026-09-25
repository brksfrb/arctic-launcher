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
    // A missing resource compiler must not break the build; the exe just
    // keeps the default icon.
    if let Err(e) = res.compile() {
        println!("cargo:warning=could not embed exe icon: {e}");
    }
}
