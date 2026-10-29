//! OptiFine capes: OptiFine keeps one public PNG per player name on its cape server. They are read
//! here so a player can wear theirs as an Arctic cape (the game mod shows other players' the same way).

use crate::{Error, Result};

const URL: &str = "http://s.optifine.net/capes/";
/// OptiFine's own cape layout (46 × 22, or a multiple), which sits in the corner of Minecraft's 64 × 32.
const OWN_W: u32 = 46;
const OWN_H: u32 = 22;
const MC_W: u32 = 64;
const MC_H: u32 = 32;
/// The largest cape file read.
const MAX_BYTES: u64 = 1024 * 1024;
const MAX_NAME: usize = 16;

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// The OptiFine cape of the player `name` as a Minecraft-layout PNG, or `None` when there is none.
pub fn fetch(name: &str) -> Result<Option<Vec<u8>>> {
    if !valid_name(name) {
        return Ok(None);
    }
    let mut resp = crate::net::agent()
        .get(&format!("{URL}{name}.png"))
        .config()
        .http_status_as_error(false)
        .build()
        .call()?;
    let status = resp.status().as_u16();
    if status == 404 {
        return Ok(None);
    }
    if status != 200 {
        return Err(Error::Other(format!("OptiFine's cape server answered {status}")));
    }
    let bytes = resp
        .body_mut()
        .with_config()
        .limit(MAX_BYTES)
        .read_to_vec()?;
    layout(&bytes).map(Some)
}

/// OptiFine's 46 × 22 layout moved into the corner of a 64 × 32 texture (any multiple of it); other
/// sizes are kept as they are.
pub fn layout(png: &[u8]) -> Result<Vec<u8>> {
    let bad = |e: &dyn std::fmt::Display| Error::Other(format!("not a cape image: {e}"));
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|e| bad(&e))?;
    let info = reader.info();
    let (w, h) = (info.width, info.height);
    let scale = w / OWN_W;
    if scale < 1 || w != OWN_W * scale || h != OWN_H * scale {
        return Ok(png.to_vec());
    }
    let mut buf = vec![0; reader.output_buffer_size().unwrap_or(0)];
    let frame = reader.next_frame(&mut buf).map_err(|e| bad(&e))?;
    let channels = frame.color_type.samples();
    if !(3..=4).contains(&channels) || buf.len() < w as usize * h as usize * channels {
        return Err(bad(&"unsupported colour format"));
    }
    let (out_w, out_h) = ((MC_W * scale) as usize, (MC_H * scale) as usize);
    let mut rgba = vec![0u8; out_w * out_h * 4];
    for y in 0..h as usize {
        for x in 0..w as usize {
            let src = (y * w as usize + x) * channels;
            let dst = (y * out_w + x) * 4;
            rgba[dst..dst + 3].copy_from_slice(&buf[src..src + 3]);
            rgba[dst + 3] = if channels == 4 { buf[src + 3] } else { 255 };
        }
    }
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, out_w as u32, out_h as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|e| bad(&e))?;
        writer.write_image_data(&rgba).map_err(|e| bad(&e))?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgba_png(w: u32, h: u32, fill: [u8; 4]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, w, h);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let data: Vec<u8> = (0..w * h).flat_map(|_| fill).collect();
        encoder.write_header().unwrap().write_image_data(&data).unwrap();
        out
    }

    fn size(png: &[u8]) -> (u32, u32) {
        let r = png::Decoder::new(std::io::Cursor::new(png)).read_info().unwrap();
        (r.info().width, r.info().height)
    }

    #[test]
    fn the_own_layout_moves_into_a_minecraft_sized_texture() {
        let out = layout(&rgba_png(46, 22, [9, 8, 7, 255])).unwrap();
        assert_eq!(size(&out), (64, 32));
        let out = layout(&rgba_png(92, 44, [9, 8, 7, 255])).unwrap();
        assert_eq!(size(&out), (128, 64));
    }

    #[test]
    fn other_sizes_are_kept() {
        let png = rgba_png(64, 32, [1, 2, 3, 255]);
        assert_eq!(layout(&png).unwrap(), png);
    }

    #[test]
    fn only_plain_names_are_asked() {
        assert!(valid_name("Some_Name1"));
        assert!(!valid_name(""));
        assert!(!valid_name("../x"));
        assert!(!valid_name("a name"));
        assert!(!valid_name("seventeen_chars_xx"));
    }
}
