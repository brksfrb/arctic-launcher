//! Embeds the translations (`locales/`), and on Windows the generated snowflake icon and
//! version info, into the exe.

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use xz2::stream::{LzmaOptions, Stream};
use xz2::write::XzEncoder;

include!("src/icon_raster.rs");

const ICO_SIZES: [u32; 6] = [16, 24, 32, 48, 64, 256];

fn main() {
    println!("cargo:rerun-if-changed=src/icon_raster.rs");
    println!("cargo:rerun-if-changed=build.rs");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR is set by cargo"));
    embed_locales(&out_dir);
    embed_fonts(&out_dir);
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let ico = out_dir.join("arctic.ico");
    if let Err(e) = std::fs::write(&ico, ico_bytes(&ICO_SIZES, icon_frame)) {
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

fn icon_frame(size: u32) -> Vec<u8> {
    if size != 256 {
        return bmp_entry(size);
    }
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, size, size);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("write icon PNG header");
    writer
        .write_image_data(&app_icon_rgba(size))
        .expect("write icon PNG pixels");
    writer.finish().expect("finish icon PNG");
    bytes
}

/// Compress each language independently. Keep file boundaries for parsing and validation.
fn embed_locales(out_dir: &Path) {
    println!("cargo:rerun-if-changed=locales");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("locales");
    let mut languages = directory_paths(&root);
    languages.sort();
    let mut source = String::from("const LOCALES: &[EmbeddedLocale] = &[\n");
    for language in languages {
        if !language.is_dir() {
            continue;
        }
        let mut files = directory_paths(&language);
        files.retain(|p| p.extension().is_some_and(|ext| ext == "json"));
        if files.is_empty() {
            continue;
        }
        files.sort();
        let code = language.file_name().unwrap().to_str().unwrap();
        let blob = format!("locale-{code}.lzma");
        writeln!(
            source,
            "    EmbeddedLocale {{ code: {code:?}, bytes: include_bytes!({blob:?}), files: &["
        )
        .unwrap();
        let mut plain = Vec::new();
        for file in files {
            let name = file.file_name().unwrap().to_str().unwrap();
            let json = std::fs::read_to_string(&file)
                .unwrap_or_else(|error| panic!("read {}: {error}", file.display()));
            plain.extend_from_slice(json.as_bytes());
            writeln!(source, "        ({name:?}, {}),", plain.len()).unwrap();
        }
        source.push_str("    ] },\n");

        compress(&out_dir.join(blob), &plain);
    }
    source.push_str("];\n");
    std::fs::write(out_dir.join("locales.rs"), source).expect("write locales.rs into OUT_DIR");
}

fn embed_fonts(out_dir: &Path) {
    println!("cargo:rerun-if-changed=assets/fonts");
    for path in directory_paths(Path::new("assets/fonts")) {
        if !matches!(
            path.extension().and_then(|ext| ext.to_str()),
            Some("ttf" | "otf")
        ) {
            continue;
        }
        let name = path.file_name().unwrap().to_str().unwrap();
        let bytes =
            std::fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        compress(&out_dir.join(format!("{name}.lzma")), &bytes);
    }
}

fn compress(path: &Path, plain: &[u8]) {
    // A match-finding encoder. The existing lzma-rs runtime decoder reads
    // this format, but lzma-rs's own encoder writes only literals.
    let mut options = LzmaOptions::new_preset(6).expect("valid LZMA preset");
    options.dict_size(512 * 1024);
    let stream = Stream::new_lzma_encoder(&options).expect("initialize LZMA encoder");
    let mut encoder = XzEncoder::new_stream(Vec::new(), stream);
    encoder.write_all(plain).expect("compress embedded asset");
    std::fs::write(path, encoder.finish().expect("finish embedded asset"))
        .unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
}

fn directory_paths(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("read {}: {error}", dir.display()))
        .map(|entry| entry.expect("read locale directory entry").path())
        .collect()
}
