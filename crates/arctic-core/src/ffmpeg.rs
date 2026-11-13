//! FFmpeg for exporting replay videos: one already on the PATH, or a copy
//! the launcher downloads once when a player first exports (on Windows,
//! gyan.dev's "essentials" build, checked against its published SHA-256;
//! only `ffmpeg.exe` is kept).

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::error::IoContext;
use crate::net::{agent, body_timeout};
use crate::storage::DataDirs;
use crate::{Error, Result};

const BUILDS: &str = "https://www.gyan.dev/ffmpeg/builds";
const CHUNK: usize = 64 * 1024;
/// A sane ceiling for the download (the build is about 115 MB).
const MAX_BYTES: u64 = 400 * 1024 * 1024;
/// About the build's size, for how long the download may take on a slow line.
const EXPECTED_BYTES: u64 = 120 * 1024 * 1024;
const EXE: &str = if cfg!(windows) {
    "ffmpeg.exe"
} else {
    "ffmpeg"
};

/// Where the launcher keeps its own copy.
pub fn tool_path(dirs: &DataDirs) -> PathBuf {
    dirs.root().join("tools").join("ffmpeg").join(EXE)
}

/// Where a binary the player picked in Settings is remembered.
fn chosen_file(dirs: &DataDirs) -> PathBuf {
    dirs.root().join("tools").join("ffmpeg").join("path.txt")
}

/// The FFmpeg the player picked in Settings, if any (and still there).
pub fn chosen(dirs: &DataDirs) -> Option<PathBuf> {
    let text = fs::read_to_string(chosen_file(dirs)).ok()?;
    let path = PathBuf::from(text.trim());
    (path.is_absolute() && path.is_file()).then_some(path)
}

/// Remember (or with `None` forget) the FFmpeg the player picked.
pub fn set_chosen(dirs: &DataDirs, path: Option<&Path>) -> Result<()> {
    let file = chosen_file(dirs);
    match path {
        Some(path) => {
            if let Some(dir) = file.parent() {
                fs::create_dir_all(dir).at(dir)?;
            }
            fs::write(&file, path.to_string_lossy().as_bytes()).at(&file)
        }
        None => match fs::remove_file(&file) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(Error::io(&file, e)),
            _ => Ok(()),
        },
    }
}

/// Is this really FFmpeg? It answers `-version` with a line starting "ffmpeg version".
pub fn check(path: &Path) -> Result<()> {
    let output = std::process::Command::new(path)
        .arg("-version")
        .output()
        .map_err(|e| Error::Other(format!("{} couldn't be run: {e}", path.display())))?;
    if is_ffmpeg_version_output(&output.stdout) {
        Ok(())
    } else {
        Err(Error::Other(format!(
            "{} doesn't look like FFmpeg (it didn't answer `ffmpeg -version`)",
            path.display()
        )))
    }
}

fn is_ffmpeg_version_output(stdout: &[u8]) -> bool {
    String::from_utf8_lossy(stdout)
        .lines()
        .next()
        .is_some_and(|l| l.starts_with("ffmpeg version"))
}

/// How to get FFmpeg where the launcher doesn't download it.
pub fn install_hint() -> &'static str {
    if cfg!(target_os = "macos") {
        "brew install ffmpeg"
    } else {
        "sudo apt install ffmpeg (Debian, Ubuntu) · sudo dnf install ffmpeg (Fedora) · sudo pacman -S ffmpeg (Arch)"
    }
}

/// FFmpeg to use: the one the player picked, else the launcher's copy, else one on the PATH.
pub fn find(dirs: &DataDirs) -> Option<PathBuf> {
    if let Some(picked) = chosen(dirs) {
        return Some(picked);
    }
    let own = tool_path(dirs);
    if own.is_file() {
        return Some(own);
    }
    on_path()
}

fn on_path() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(EXE))
        .find(|p| p.is_file())
}

/// Whether this platform gets a download (elsewhere: the package manager).
pub fn can_download() -> bool {
    cfg!(windows)
}

/// Download and unpack FFmpeg; `progress(done, total)` in bytes.
pub fn install(dirs: &DataDirs, mut progress: impl FnMut(u64, u64)) -> Result<PathBuf> {
    if !can_download() {
        return Err(Error::Other(
            "install FFmpeg with your package manager (like `apt install ffmpeg`)".into(),
        ));
    }
    let version = text(&format!("{BUILDS}/release-version"))?;
    let version = version.trim();
    if !valid_version(version) {
        return Err(Error::Other(format!(
            "unexpected FFmpeg version {version:?}"
        )));
    }
    let url = package_url(version);
    let expected = parse_sha256(&text(&format!("{url}.sha256"))?)
        .ok_or_else(|| Error::Other("FFmpeg checksum missing".into()))?;
    let cache = dirs.cache().join("ffmpeg");
    fs::create_dir_all(&cache).at(&cache)?;
    let zip_path = cache.join(format!("ffmpeg-{version}.zip"));
    let result = download(&url, &zip_path, &expected, &mut progress)
        .and_then(|()| unpack(&zip_path, &tool_path(dirs)));
    let _ = fs::remove_file(&zip_path);
    result?;
    Ok(tool_path(dirs))
}

fn text(url: &str) -> Result<String> {
    Ok(agent().get(url).call()?.body_mut().read_to_string()?)
}

/// Build versions look like "9.0.2".
fn valid_version(v: &str) -> bool {
    !v.is_empty() && v.len() < 20 && v.chars().all(|c| c.is_ascii_digit() || c == '.')
}

fn package_url(version: &str) -> String {
    format!("{BUILDS}/packages/ffmpeg-{version}-essentials_build.zip")
}

/// The hash from a `.sha256` file ("<64 hex>" or "<64 hex>  name").
fn parse_sha256(file: &str) -> Option<String> {
    let first = file.split_whitespace().next()?.to_ascii_lowercase();
    (first.len() == 64 && first.chars().all(|c| c.is_ascii_hexdigit())).then_some(first)
}

fn download(
    url: &str,
    to: &Path,
    expected: &str,
    progress: &mut impl FnMut(u64, u64),
) -> Result<()> {
    let mut resp = agent()
        .get(url)
        .config()
        .timeout_recv_body(Some(body_timeout(Some(EXPECTED_BYTES))))
        .build()
        .call()?;
    let total = resp
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    let mut reader = resp.body_mut().with_config().limit(MAX_BYTES).reader();
    let mut file = fs::File::create(to).at(to)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; CHUNK];
    let mut done = 0u64;
    loop {
        let n = reader.read(&mut buf).at(to)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        file.write_all(&buf[..n]).at(to)?;
        done += n as u64;
        progress(done, total.max(done));
    }
    drop(file);
    if hex::encode(hasher.finalize()) != expected {
        return Err(Error::Checksum("FFmpeg".into()));
    }
    Ok(())
}

/// Copy `…/bin/ffmpeg.exe` out of the build (nothing else is kept).
fn unpack(zip_path: &Path, to: &Path) -> Result<()> {
    let file = fs::File::open(zip_path).at(zip_path)?;
    let mut zip =
        zip::ZipArchive::new(file).map_err(|e| Error::Other(format!("FFmpeg zip: {e}")))?;
    let names: Vec<String> = zip.file_names().map(str::to_owned).collect();
    let entry =
        pick_exe(&names).ok_or_else(|| Error::Other("no ffmpeg.exe in the build".into()))?;
    let mut src = zip
        .by_name(&entry)
        .map_err(|e| Error::Other(format!("FFmpeg zip: {e}")))?;
    let dir = to.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(dir).at(dir)?;
    let part = to.with_extension("part");
    let mut out = fs::File::create(&part).at(&part)?;
    std::io::copy(&mut src, &mut out).at(&part)?;
    drop(out);
    fs::rename(&part, to).at(to)?;
    Ok(())
}

fn pick_exe(names: &[String]) -> Option<String> {
    names
        .iter()
        .find(|n| n.ends_with("/bin/ffmpeg.exe") && !n.contains(".."))
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picked_binary_is_remembered_and_wins_over_the_path() {
        let dir = tempfile::tempdir().unwrap();
        let dirs = DataDirs::new(dir.path().join("data"));
        assert_eq!(chosen(&dirs), None);
        let fake = dir.path().join("my-ffmpeg");
        set_chosen(&dirs, Some(&fake)).unwrap();
        assert_eq!(chosen(&dirs), None, "not there yet, so not offered");
        fs::write(&fake, b"").unwrap();
        assert_eq!(chosen(&dirs).as_deref(), Some(fake.as_path()));
        assert_eq!(find(&dirs).as_deref(), Some(fake.as_path()));
        set_chosen(&dirs, None).unwrap();
        assert_eq!(chosen(&dirs), None);
        set_chosen(&dirs, None).unwrap();
    }

    #[test]
    fn only_a_real_ffmpeg_answers_version() {
        assert!(is_ffmpeg_version_output(
            b"ffmpeg version 7.1 Copyright (c) 2000-2024 the FFmpeg developers\nbuilt with gcc"
        ));
        assert!(!is_ffmpeg_version_output(b"usage: something else"));
        assert!(!is_ffmpeg_version_output(b""));
        let dir = tempfile::tempdir().unwrap();
        assert!(check(&dir.path().join("missing")).is_err());
    }

    #[test]
    fn versions_are_plain_numbers() {
        assert!(valid_version("9.0.2"));
        assert!(!valid_version(""));
        assert!(!valid_version("9.0.2/../x"));
        assert!(!valid_version("<html>"));
    }

    #[test]
    fn package_urls_follow_the_version() {
        assert_eq!(
            package_url("9.0.2"),
            "https://www.gyan.dev/ffmpeg/builds/packages/ffmpeg-9.0.2-essentials_build.zip"
        );
    }

    #[test]
    fn reads_checksum_files() {
        let h = "60f467265b1e312373dbcd92200c2618a74850f98d3d078e94296bb3fa2047ba";
        assert_eq!(parse_sha256(h).as_deref(), Some(h));
        assert_eq!(
            parse_sha256(&format!("{}  ffmpeg.zip\n", h.to_uppercase())).as_deref(),
            Some(h)
        );
        assert_eq!(parse_sha256("not a hash"), None);
        assert_eq!(parse_sha256(""), None);
    }

    #[test]
    fn keeps_only_the_ffmpeg_executable() {
        let names = vec![
            "ffmpeg-9.0.2-essentials_build/bin/ffplay.exe".to_owned(),
            "ffmpeg-9.0.2-essentials_build/bin/ffmpeg.exe".to_owned(),
            "ffmpeg-9.0.2-essentials_build/doc/ffmpeg.html".to_owned(),
        ];
        assert_eq!(
            pick_exe(&names).as_deref(),
            Some("ffmpeg-9.0.2-essentials_build/bin/ffmpeg.exe")
        );
        assert_eq!(pick_exe(&["../bin/ffmpeg.exe".to_owned()]), None);
    }
}
