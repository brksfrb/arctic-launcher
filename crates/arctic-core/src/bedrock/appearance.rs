//! Checked PNGs for unsigned Bedrock login skins. Only pixel data crosses into
//! the helper; it never opens arbitrary asset paths or account stores.
use std::io::{Cursor, Read};
use std::path::Path;

use base64::Engine;
use serde::Serialize;

use crate::error::IoContext;
use crate::{Error, Result};

#[derive(Debug, Clone, Serialize)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub data: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Appearance {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skin: Option<Image>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cape: Option<Image>,
    pub slim: bool,
}

impl Appearance {
    pub fn read(skin: Option<&Path>, cape: Option<&Path>, slim: bool) -> Result<Option<Self>> {
        if slim && skin.is_none() {
            return Err(Error::Other("--slim needs a custom skin PNG".into()));
        }
        if skin.is_none() && cape.is_none() {
            return Ok(None);
        }
        Ok(Some(Self {
            skin: skin.map(|p| read_image(p, false)).transpose()?,
            cape: cape.map(|p| read_image(p, true)).transpose()?,
            slim,
        }))
    }
}

fn read_image(path: &Path, cape: bool) -> Result<Image> {
    const MAX_FILE: u64 = 1024 * 1024;
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .at(path)?
        .take(MAX_FILE + 1)
        .read_to_end(&mut bytes)
        .at(path)?;
    if bytes.len() as u64 > MAX_FILE {
        return Err(Error::Other("appearance PNG exceeds 1 MiB".into()));
    }
    decode(&bytes, cape)
}

fn decode(bytes: &[u8], cape: bool) -> Result<Image> {
    let bad = |e: String| {
        Error::Other(format!(
            "invalid Bedrock {} PNG: {e}",
            if cape { "cape" } else { "skin" }
        ))
    };
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_limits(png::Limits {
        bytes: 8 * 1024 * 1024,
    });
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|e| bad(e.to_string()))?;
    let (width, height) = (reader.info().width, reader.info().height);
    let valid = if cape {
        (64..=1024).contains(&width) && width.is_power_of_two() && height == width / 2
    } else {
        width == 64 && matches!(height, 32 | 64)
    };
    if !valid {
        return Err(bad(if cape {
            "use a static 2:1 cape, power-of-two width 64–1024; animated cape playback needs the native renderer".into()
        } else {
            "use a 64×64 or 64×32 skin".into()
        }));
    }
    if reader.info().animation_control.is_some() {
        return Err(bad("APNG is not supported".into()));
    }
    let mut decoded = vec![
        0;
        reader
            .output_buffer_size()
            .ok_or_else(|| bad("image too large".into()))?
    ];
    let info = reader
        .next_frame(&mut decoded)
        .map_err(|e| bad(e.to_string()))?;
    let channels = info.color_type.samples();
    // Bedrock's cape texture is 64×32. Reduce HD Arctic capes without changing UVs.
    let (out_w, mut out_h) = if cape { (64, 32) } else { (width, height) };
    let mut rgba = Vec::with_capacity((out_w * out_h * 4) as usize);
    for y in 0..out_h {
        for x in 0..out_w {
            let at = (((y * height / out_h) * width + x * width / out_w) as usize) * channels;
            let p = &decoded[at..at + channels];
            match info.color_type {
                png::ColorType::Rgba => rgba.extend_from_slice(p),
                png::ColorType::Rgb => rgba.extend_from_slice(&[p[0], p[1], p[2], 255]),
                png::ColorType::Grayscale => rgba.extend_from_slice(&[p[0], p[0], p[0], 255]),
                png::ColorType::GrayscaleAlpha => rgba.extend_from_slice(&[p[0], p[0], p[0], p[1]]),
                _ => return Err(bad("unsupported color format".into())),
            }
        }
    }
    if !cape {
        rgba.resize(64 * 64 * 4, 0);
        rgba = crate::skins::normalize_rgba(rgba, height == 32).rgba;
        out_h = 64;
    }
    Ok(Image {
        width: out_w,
        height: out_h,
        data: base64::prelude::BASE64_STANDARD.encode(rgba),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32, color: png::ColorType, data: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut e = png::Encoder::new(&mut bytes, w, h);
            e.set_color(color);
            e.write_header().unwrap().write_image_data(data).unwrap();
        }
        bytes
    }

    #[test]
    fn skin_pixels_use_the_same_opacity_and_legacy_rules_as_java() {
        let bytes = png(
            64,
            64,
            png::ColorType::Rgba,
            &[13, 27, 191, 91].repeat(64 * 64),
        );
        let skin = decode(&bytes, false).unwrap();
        let data = base64::prelude::BASE64_STANDARD.decode(skin.data).unwrap();
        assert_eq!(
            &data[(8 * 64 + 8) * 4..(8 * 64 + 8) * 4 + 4],
            &[13, 27, 191, 255]
        );
        assert_eq!(
            &data[(8 * 64 + 40) * 4..(8 * 64 + 40) * 4 + 4],
            &[13, 27, 191, 91]
        );
        let gray = png(64, 32, png::ColorType::Grayscale, &vec![79; 64 * 32]);
        let skin = decode(&gray, false).unwrap();
        assert_eq!((skin.width, skin.height), (64, 64));
        let data = base64::prelude::BASE64_STANDARD.decode(skin.data).unwrap();
        assert_eq!(
            &data[(52 * 64 + 20) * 4..(52 * 64 + 20) * 4 + 4],
            &[79, 79, 79, 255]
        );
        assert_eq!(data[(8 * 64 + 40) * 4 + 3], 0); // Solid legacy hat is cleared.
    }

    #[test]
    fn reduces_hd_cape_and_refuses_invalid_or_animated_layouts() {
        let bytes = png(128, 64, png::ColorType::Rgb, &[1, 2, 3].repeat(128 * 64));
        let cape = decode(&bytes, true).unwrap();
        assert_eq!((cape.width, cape.height), (64, 32));
        assert_eq!(
            base64::prelude::BASE64_STANDARD.decode(cape.data).unwrap(),
            [1, 2, 3, 255].repeat(64 * 32)
        );
        assert!(decode(&bytes, false).is_err());
        assert!(
            decode(
                &png(64, 64, png::ColorType::Rgba, &vec![0; 64 * 64 * 4]),
                true
            )
            .is_err()
        );
    }
}
