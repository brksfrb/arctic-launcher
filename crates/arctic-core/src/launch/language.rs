//! Pre-1.6 StringTranslate opens the selected language directly from the
//! client jar and does not handle a missing stream. Shared modern options
//! (en_us), or a language added after that release, otherwise crash startup.

use std::fs;
use std::path::Path;

use crate::error::IoContext;
use crate::{Error, Result};

pub(super) fn prepare(client: &Path, game_dir: &Path) -> Result<()> {
    let options = game_dir.join("options.txt");
    let bytes = match fs::read(&options) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(Error::io(&options, e)),
    };
    if !bytes
        .split(|&byte| byte == b'\n')
        .any(|line| line.starts_with(b"lang:"))
    {
        return Ok(());
    }
    let mut jar = zip::ZipArchive::new(fs::File::open(client).at(client)?)
        .map_err(|e| Error::Other(format!("could not read {}: {e}", client.display())))?;
    // Detect the actual resource layout, including snapshots and loader profiles.
    // Resource-pack-era games handle missing languages themselves.
    if jar.by_name("lang/en_US.lang").is_err() {
        return Ok(());
    }
    let languages: Vec<String> = jar
        .file_names()
        .filter_map(|name| name.strip_prefix("lang/")?.strip_suffix(".lang"))
        .map(str::to_owned)
        .collect();
    let mut changed = false;
    let mut repaired = Vec::with_capacity(bytes.len());
    // Options use the game's platform encoding. Only replace ASCII language
    // values; preserve all unrelated bytes, including original line endings.
    for line in bytes.split_inclusive(|&byte| byte == b'\n') {
        let (body, ending) = if let Some(body) = line.strip_suffix(b"\r\n") {
            (body, b"\r\n".as_slice())
        } else if let Some(body) = line.strip_suffix(b"\n") {
            (body, b"\n".as_slice())
        } else {
            (line, b"".as_slice())
        };
        if let Some(selected) = body.strip_prefix(b"lang:") {
            let available = languages
                .iter()
                .find(|code| code.as_bytes().eq_ignore_ascii_case(selected.trim_ascii()))
                .map(String::as_str)
                .unwrap_or("en_US");
            changed |= selected != available.as_bytes();
            repaired.extend_from_slice(b"lang:");
            repaired.extend_from_slice(available.as_bytes());
            repaired.extend_from_slice(ending);
        } else {
            repaired.extend_from_slice(line);
        }
    }
    if changed {
        log::info!("adjusted language to the installed legacy client's language files");
        fs::write(&options, repaired).at(&options)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn repairs_shared_and_existing_options_using_the_client_resources() {
        let dir = tempfile::tempdir().unwrap();
        let client = dir.path().join("client.jar");
        let mut jar = zip::ZipWriter::new(fs::File::create(&client).unwrap());
        for name in ["lang/en_US.lang", "lang/pt_BR.lang", "lang/de_DE.lang"] {
            jar.start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            jar.write_all(b"menu.singleplayer=Singleplayer\n").unwrap();
        }
        jar.finish().unwrap();
        let options = dir.path().join("options.txt");
        for (selected, expected) in [
            ("en_us", "en_US"),
            ("pt_br", "pt_BR"),
            ("de_DE", "de_DE"),
            ("xx_zz", "en_US"),
            ("../../missing", "en_US"),
            ("", "en_US"),
        ] {
            fs::write(
                &options,
                format!("fov:0.5\nlang:{selected}\nkey_key.jump:57\n"),
            )
            .unwrap();
            prepare(&client, dir.path()).unwrap();
            let want = format!("fov:0.5\nlang:{expected}\nkey_key.jump:57\n");
            assert_eq!(fs::read_to_string(&options).unwrap(), want);
            prepare(&client, dir.path()).unwrap();
            assert_eq!(fs::read_to_string(&options).unwrap(), want);
        }
    }

    #[test]
    fn resource_pack_era_and_missing_options_are_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let client = dir.path().join("client.jar");
        prepare(&client, dir.path()).unwrap();
        let mut jar = zip::ZipWriter::new(fs::File::create(&client).unwrap());
        jar.start_file(
            "assets/minecraft/lang/en_US.lang",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        jar.finish().unwrap();
        let options = dir.path().join("options.txt");
        prepare(&client, dir.path()).unwrap();
        let text = b"lang:custom_pack_language\r\nresourcePacks:[\"\xe9t\xe9.zip\"]\r\nfov:0.5\r\n";
        fs::write(&options, text).unwrap();
        prepare(&client, dir.path()).unwrap();
        assert_eq!(fs::read(options).unwrap(), text);
    }

    #[test]
    fn legacy_repairs_preserve_non_utf8_bytes_and_line_endings() {
        let dir = tempfile::tempdir().unwrap();
        let client = dir.path().join("client.jar");
        let mut jar = zip::ZipWriter::new(fs::File::create(&client).unwrap());
        jar.start_file("lang/en_US.lang", zip::write::SimpleFileOptions::default())
            .unwrap();
        jar.finish().unwrap();
        let options = dir.path().join("options.txt");
        prepare(&client, dir.path()).unwrap();
        assert!(!options.exists());
        for ending in [b"\n".as_slice(), b"\r\n".as_slice(), b"".as_slice()] {
            let prefix = b"texturePack:\xe9t\xe9.zip\r\n";
            let original = [prefix.as_slice(), b"lang:en_us", ending].concat();
            let expected = [prefix.as_slice(), b"lang:en_US", ending].concat();
            fs::write(&options, original).unwrap();
            prepare(&client, dir.path()).unwrap();
            assert_eq!(fs::read(&options).unwrap(), expected);
            prepare(&client, dir.path()).unwrap();
            assert_eq!(fs::read(&options).unwrap(), expected);
        }
    }
}
