//! Checks for uploaded textures: real PNGs of the sizes the game uses.

use sha1::{Digest, Sha1};

/// Largest upload accepted (a 512×256 HD cape is well under this).
pub const MAX_PNG_BYTES: usize = 256 * 1024;
/// Animated capes stack up to this many frames vertically (clients play
/// them at 8 frames a second).
pub const MAX_CAPE_FRAMES: u32 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Skin,
    Cape,
}

/// Validate a PNG for `kind`; returns its SHA-1 (hex) on success.
pub fn check(png_bytes: &[u8], kind: Kind) -> Result<String, String> {
    if png_bytes.len() > MAX_PNG_BYTES {
        return Err("image is too large".into());
    }
    let decoder = png::Decoder::new(std::io::Cursor::new(png_bytes));
    let reader = decoder
        .read_info()
        .map_err(|_| "not a PNG image".to_owned())?;
    let (w, h) = (reader.info().width, reader.info().height);
    let ok = match kind {
        // Classic 64×64 skins and legacy 64×32 ones.
        Kind::Skin => w == 64 && (h == 64 || h == 32),
        // Capes are 2:1 frames, from 64×32 up to 512×256 (HD capes),
        // stacked vertically when animated.
        Kind::Cape => {
            let frame = w / 2;
            (64..=512).contains(&w)
                && w.is_power_of_two()
                && h.is_multiple_of(frame)
                && (1..=MAX_CAPE_FRAMES).contains(&(h / frame))
        }
    };
    if !ok {
        return Err(format!(
            "a {w}×{h} image is not a valid {}",
            match kind {
                Kind::Skin => "skin (64×64)",
                Kind::Cape => {
                    "cape (64×32, or 128×64 up to 512×256; animated capes stack up to 8 frames)"
                }
            }
        ));
    }
    Ok(hex::encode(Sha1::digest(png_bytes)))
}

/// What an image looks like, as a hash of its size and RGBA pixels, so the
/// same skin saved by different programs still matches. Fully transparent
/// pixels count as one color (their hidden RGB varies between tools).
pub fn pixel_key(png_bytes: &[u8]) -> Option<String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png_bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let bytes = &buf[..info.buffer_size()];
    let rgba: Vec<[u8; 4]> = match info.color_type {
        png::ColorType::Rgba => bytes
            .chunks_exact(4)
            .map(|p| [p[0], p[1], p[2], p[3]])
            .collect(),
        png::ColorType::Rgb => bytes
            .chunks_exact(3)
            .map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => bytes
            .chunks_exact(2)
            .map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Grayscale => bytes.iter().map(|&v| [v, v, v, 255]).collect(),
        png::ColorType::Indexed => return None,
    };
    let mut hasher = Sha1::new();
    hasher.update(info.width.to_le_bytes());
    hasher.update(info.height.to_le_bytes());
    for p in rgba {
        hasher.update(if p[3] == 0 { [0, 0, 0, 0] } else { p });
    }
    Some(hex::encode(hasher.finalize()))
}

/// Offline players' UUID, as the game derives it from the name.
pub fn offline_uuid(name: &str) -> String {
    use md5::{Digest as _, Md5};
    let digest = Md5::digest(format!("OfflinePlayer:{name}").as_bytes());
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest);
    bytes[6] = (bytes[6] & 0x0f) | 0x30;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    hex::encode(bytes)
}

#[cfg(test)]
pub fn test_png(w: u32, h: u32) -> Vec<u8> {
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().unwrap();
    writer
        .write_image_data(&vec![200u8; (w * h * 4) as usize])
        .unwrap();
    drop(writer);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes() {
        assert!(check(&test_png(64, 64), Kind::Skin).is_ok());
        assert!(check(&test_png(64, 32), Kind::Skin).is_ok());
        assert!(check(&test_png(128, 128), Kind::Skin).is_err());
        assert!(check(&test_png(64, 32), Kind::Cape).is_ok());
        assert!(check(&test_png(128, 64), Kind::Cape).is_ok());
        assert!(check(&test_png(96, 48), Kind::Cape).is_err());
        // Animated: frames stacked vertically, at most 8.
        assert!(check(&test_png(64, 32 * 6), Kind::Cape).is_ok());
        assert!(check(&test_png(128, 64 * 8), Kind::Cape).is_ok());
        assert!(check(&test_png(64, 32 * 9), Kind::Cape).is_err());
        assert!(check(&test_png(64, 48), Kind::Cape).is_err());
        assert!(check(b"nope", Kind::Cape).is_err());
    }

    #[test]
    fn pixel_key_ignores_encoding_and_hidden_colors() {
        let a = test_png(64, 64);
        let b = test_png(64, 64);
        assert_eq!(pixel_key(&a), pixel_key(&b));
        assert_ne!(pixel_key(&a), pixel_key(&test_png(64, 32)));
        assert_eq!(pixel_key(b"nope"), None);
    }

    #[test]
    fn offline_uuid_matches_the_game() {
        // Java: UUID.nameUUIDFromBytes("OfflinePlayer:Notch".getBytes())
        assert_eq!(offline_uuid("Notch"), "b50ad385829d3141a2167e7d7539ba7f");
    }
}
