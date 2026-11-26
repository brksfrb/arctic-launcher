//! Turns a chosen GIF into the cape format the clients already play: one
//! PNG strip of 2:1 frames stacked vertically, shown at the cape's own speed
//! (8 frames a second). The strip has no place for per-frame delays, so the
//! GIF's timeline is resampled onto that fixed tick: a frame shown for 400 ms
//! becomes four strip frames. The same conversion runs on the server (arctic-server
//! `gif_cape.rs`, limits in its `images.rs`), so what the preview plays is what players get; keep both in step.

use std::io::Cursor;

use crate::cosmetics::{
    DEFAULT_CAPE_FPS as DEFAULT_FPS, MAX_CAPE_FRAMES, MAX_CAPE_STRIP_HEIGHT, MAX_CAPE_WIDTH,
};

/// Largest cape file the server accepts (and the strip it stores).
pub const MAX_CAPE_BYTES: usize = 2 * 1024 * 1024;

/// Widest canvas read (larger GIFs are shrunk to 1024, but not without limit).
const MAX_SOURCE_WIDTH: u32 = 4096;
/// Most pixels decoded in one GIF (canvas × frames): tiny files can inflate hugely.
const MAX_SOURCE_PIXELS: u64 = 256 * 1024 * 1024;
/// Most GIF frames read from one file.
const MAX_SOURCE_FRAMES: usize = 600;
/// GIFs with a delay under 20 ms play at 100 ms in browsers; so do we.
const MIN_DELAY_MS: u32 = 20;
const FALLBACK_DELAY_MS: u32 = 100;
const MIN_WIDTH: u32 = 64;

pub fn is_gif(bytes: &[u8]) -> bool {
    bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a")
}

/// Convert GIF bytes to a cape PNG strip that passes `images::check`.
pub fn to_cape_png(bytes: &[u8]) -> Result<Vec<u8>, String> {
    if bytes.len() > MAX_CAPE_BYTES {
        return Err("image is too large".into());
    }
    let bad = |_| "the GIF is damaged or not a valid GIF".to_owned();
    let delays = read_delays(bytes).map_err(bad)?;
    if delays.is_empty() {
        return Err("the GIF has no frames".into());
    }
    if delays.len() > MAX_SOURCE_FRAMES {
        return Err(format!(
            "the GIF has {} frames; at most {MAX_SOURCE_FRAMES} are accepted",
            delays.len()
        ));
    }
    let (w, h) = dimensions(bytes).map_err(bad)?;
    if w == 0 || h == 0 || w != 2 * h {
        return Err(format!(
            "a {w}×{h} GIF is not a cape: frames must be 2:1 (for example 1024×512 or 512×256)"
        ));
    }
    if w < MIN_WIDTH {
        return Err(format!(
            "a {w}×{h} GIF is too small: capes are at least {MIN_WIDTH}×{}",
            MIN_WIDTH / 2
        ));
    }
    if w > MAX_SOURCE_WIDTH {
        return Err(format!(
            "a {w}×{h} GIF is too large: capes are at most {MAX_CAPE_WIDTH}×{} (GIFs up to {MAX_SOURCE_WIDTH} wide are shrunk)",
            MAX_CAPE_WIDTH / 2
        ));
    }
    if u64::from(w) * u64::from(h) * delays.len() as u64 > MAX_SOURCE_PIXELS {
        return Err(format!(
            "this {w}×{h} GIF has too many frames ({}) for its size; shrink it or cut frames",
            delays.len()
        ));
    }
    // Largest power-of-two width that fits; halve it while the strip is over
    // the byte limit.
    let mut width = floor_pow2(w.min(MAX_CAPE_WIDTH));
    loop {
        let png = render(bytes, &delays, w, h, width)?;
        if png.len() <= MAX_CAPE_BYTES {
            return Ok(png);
        }
        if width <= MIN_WIDTH {
            return Err(format!(
                "this GIF is too detailed to fit in {} MB even at {MIN_WIDTH}×{}",
                MAX_CAPE_BYTES / (1024 * 1024),
                MIN_WIDTH / 2
            ));
        }
        width /= 2;
    }
}

fn floor_pow2(n: u32) -> u32 {
    1 << (31 - n.leading_zeros())
}

fn options() -> gif::DecodeOptions {
    let mut o = gif::DecodeOptions::new();
    o.set_color_output(gif::ColorOutput::RGBA);
    o.set_memory_limit(gif::MemoryLimit::Bytes(
        std::num::NonZeroU64::new(64 * 1024 * 1024).expect("nonzero"),
    ));
    o
}

fn dimensions(bytes: &[u8]) -> Result<(u32, u32), gif::DecodingError> {
    let d = options().read_info(Cursor::new(bytes))?;
    Ok((u32::from(d.width()), u32::from(d.height())))
}

/// Each frame's delay in milliseconds, without decoding pixels.
fn read_delays(bytes: &[u8]) -> Result<Vec<u32>, gif::DecodingError> {
    let mut d = options().read_info(Cursor::new(bytes))?;
    let mut out = Vec::new();
    while let Some(f) = d.next_frame_info()? {
        let ms = u32::from(f.delay) * 10;
        out.push(if ms < MIN_DELAY_MS {
            FALLBACK_DELAY_MS
        } else {
            ms
        });
        if out.len() > MAX_SOURCE_FRAMES {
            break;
        }
    }
    Ok(out)
}

/// Which source frame each strip frame shows: the timeline is cut into
/// 125 ms ticks (the cape's 8 fps), or into `max_frames` equal steps when it
/// is longer than that (the motion is kept, a little faster).
pub fn plan(delays: &[u32], max_frames: u32) -> Vec<usize> {
    let total: u64 = delays.iter().map(|&d| u64::from(d)).sum();
    let tick = 1000 / u64::from(DEFAULT_FPS);
    let ticks = ((total + tick / 2) / tick).clamp(1, u64::from(max_frames));
    let mut out = Vec::with_capacity(ticks as usize);
    for i in 0..ticks {
        // Middle of this tick on the GIF's timeline.
        let at = (2 * i + 1) * total / (2 * ticks);
        let mut end = 0u64;
        let mut chosen = delays.len() - 1;
        for (n, &d) in delays.iter().enumerate() {
            end += u64::from(d);
            if at < end {
                chosen = n;
                break;
            }
        }
        out.push(chosen);
    }
    out
}

fn render(bytes: &[u8], delays: &[u32], w: u32, h: u32, width: u32) -> Result<Vec<u8>, String> {
    let fh = width / 2;
    let max_frames = MAX_CAPE_FRAMES.min(MAX_CAPE_STRIP_HEIGHT / fh);
    let picks = plan(delays, max_frames);
    let mut needed = vec![false; delays.len()];
    for &p in &picks {
        needed[p] = true;
    }
    let bad = |_| "the GIF is damaged or not a valid GIF".to_owned();
    let mut d = options().read_info(Cursor::new(bytes)).map_err(bad)?;
    let (cw, ch) = (w as usize, h as usize);
    let mut canvas = vec![0u8; cw * ch * 4];
    let mut shots: Vec<Option<Vec<u8>>> = vec![None; delays.len()];
    let mut n = 0;
    while let Some(f) = d.read_next_frame().map_err(bad)? {
        let before = (f.dispose == gif::DisposalMethod::Previous).then(|| canvas.clone());
        let (fx, fy, fw, fh_) = (
            usize::from(f.left),
            usize::from(f.top),
            usize::from(f.width),
            usize::from(f.height),
        );
        for y in 0..fh_.min(ch.saturating_sub(fy)) {
            for x in 0..fw.min(cw.saturating_sub(fx)) {
                let s = (y * fw + x) * 4;
                if f.buffer[s + 3] != 0 {
                    let t = ((fy + y) * cw + fx + x) * 4;
                    canvas[t..t + 4].copy_from_slice(&f.buffer[s..s + 4]);
                }
            }
        }
        if needed.get(n).copied().unwrap_or(false) {
            shots[n] = Some(shrink(&canvas, cw, ch, width as usize, fh as usize));
        }
        match f.dispose {
            gif::DisposalMethod::Background if fx < cw => {
                for y in 0..fh_.min(ch.saturating_sub(fy)) {
                    let t = ((fy + y) * cw + fx) * 4;
                    let len = fw.min(cw - fx) * 4;
                    canvas[t..t + len].fill(0);
                }
            }
            gif::DisposalMethod::Previous => canvas = before.unwrap_or(canvas),
            _ => {}
        }
        n += 1;
    }
    let mut strip = Vec::with_capacity(picks.len() * (width * fh * 4) as usize);
    for p in &picks {
        strip.extend_from_slice(shots[*p].as_ref().ok_or("the GIF is damaged")?);
    }
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, width, fh * picks.len() as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_compression(png::Compression::High);
    let mut writer = enc.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(&strip).map_err(|e| e.to_string())?;
    drop(writer);
    Ok(out)
}

/// Area-average RGBA (alpha-weighted) from `sw`×`sh` to `dw`×`dh`.
fn shrink(src: &[u8], sw: usize, sh: usize, dw: usize, dh: usize) -> Vec<u8> {
    if sw == dw && sh == dh {
        return src.to_vec();
    }
    let mut out = vec![0u8; dw * dh * 4];
    for dy in 0..dh {
        let (y0, y1) = (
            dy * sh / dh,
            ((dy + 1) * sh).div_ceil(dh).max(dy * sh / dh + 1),
        );
        for dx in 0..dw {
            let (x0, x1) = (
                dx * sw / dw,
                ((dx + 1) * sw).div_ceil(dw).max(dx * sw / dw + 1),
            );
            let (mut r, mut g, mut b, mut a, mut cnt) = (0u64, 0u64, 0u64, 0u64, 0u64);
            for y in y0..y1.min(sh) {
                for x in x0..x1.min(sw) {
                    let p = &src[(y * sw + x) * 4..][..4];
                    let al = u64::from(p[3]);
                    r += u64::from(p[0]) * al;
                    g += u64::from(p[1]) * al;
                    b += u64::from(p[2]) * al;
                    a += al;
                    cnt += 1;
                }
            }
            if let (Some(r), Some(g), Some(b)) =
                (r.checked_div(a), g.checked_div(a), b.checked_div(a))
            {
                let o = (dy * dw + dx) * 4;
                out[o] = r as u8;
                out[o + 1] = g as u8;
                out[o + 2] = b as u8;
                out[o + 3] = (a / cnt) as u8;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A GIF of `w`×`h` frames, each a flat color, with delays in 1/100 s.
    fn make_gif(w: u16, h: u16, frames: &[([u8; 3], u16)]) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut enc = gif::Encoder::new(&mut out, w, h, &[]).unwrap();
            for (rgb, delay) in frames {
                let mut f = gif::Frame::from_rgb(w, h, &rgb.repeat(w as usize * h as usize));
                f.delay = *delay;
                enc.write_frame(&f).unwrap();
            }
        }
        out
    }

    fn decode(png_bytes: &[u8]) -> (u32, u32, Vec<u8>) {
        let mut r = png::Decoder::new(Cursor::new(png_bytes))
            .read_info()
            .unwrap();
        let mut buf = vec![0; r.output_buffer_size().unwrap()];
        let i = r.next_frame(&mut buf).unwrap();
        (i.width, i.height, buf)
    }

    /// Red of the strip frame `n` (first pixel).
    fn red_of(buf: &[u8], w: u32, n: u32) -> u8 {
        buf[((n * (w / 2)) * w * 4) as usize]
    }

    #[test]
    fn per_frame_delays_become_repeated_strip_frames() {
        // 500 ms red then 250 ms blue: 4 ticks of red, 2 of blue.
        let gif = make_gif(128, 64, &[([255, 0, 0], 50), ([0, 0, 255], 25)]);
        let png = to_cape_png(&gif).unwrap();
        assert!(crate::cosmetics::is_cape_png(&png));
        let (w, h, buf) = decode(&png);
        assert_eq!((w, h), (128, 64 * 6));
        let reds: Vec<u8> = (0..6).map(|n| red_of(&buf, w, n)).collect();
        assert_eq!(reds, [255, 255, 255, 255, 0, 0]);
    }

    #[test]
    fn single_frame_is_a_still_cape() {
        let png = to_cape_png(&make_gif(64, 32, &[([9, 9, 9], 0)])).unwrap();
        assert_eq!(decode(&png).1, 32);
    }

    #[test]
    fn oversized_gif_is_shrunk() {
        let png = to_cape_png(&make_gif(2048, 1024, &[([1, 2, 3], 12), ([3, 2, 1], 12)])).unwrap();
        let (w, h, _) = decode(&png);
        assert_eq!(w, 1024);
        assert!(h % 512 == 0 && h <= MAX_CAPE_STRIP_HEIGHT);
        assert!(crate::cosmetics::is_cape_png(&png));
        // Not a power of two: 600 wide becomes 512.
        let png = to_cape_png(&make_gif(600, 300, &[([1, 2, 3], 12)])).unwrap();
        assert_eq!(decode(&png).0, 512);
    }

    #[test]
    fn long_gif_is_thinned_keeping_order_and_proportion() {
        // 80 frames at 100 ms: first half red, second half blue.
        let frames: Vec<_> = (0..80)
            .map(|i| (if i < 40 { [255, 0, 0] } else { [0, 0, 255] }, 10))
            .collect();
        let png = to_cape_png(&make_gif(128, 64, &frames)).unwrap();
        let (w, h, buf) = decode(&png);
        let n = h / 64;
        assert_eq!(n, MAX_CAPE_FRAMES);
        let reds: Vec<u8> = (0..n).map(|i| red_of(&buf, w, i)).collect();
        assert_eq!(reds.iter().filter(|&&r| r == 255).count(), 16);
        assert!(reds[..16].iter().all(|&r| r == 255));
        assert!(reds[16..].iter().all(|&r| r == 0));
    }

    #[test]
    fn frames_cap_follows_strip_height() {
        let frames = vec![([0, 0, 0], 10); 60];
        let png = to_cape_png(&make_gif(1024, 512, &frames)).unwrap();
        let (w, h, _) = decode(&png);
        assert_eq!(w, 1024);
        assert!(h <= MAX_CAPE_STRIP_HEIGHT);
    }

    #[test]
    fn offscreen_frames_with_background_disposal_do_not_panic() {
        let mut out = Vec::new();
        {
            let mut enc = gif::Encoder::new(&mut out, 64, 32, &[]).unwrap();
            let mut f = gif::Frame::from_rgb(8, 8, &[7; 8 * 8 * 3]);
            f.left = 200;
            f.top = 30;
            f.dispose = gif::DisposalMethod::Background;
            enc.write_frame(&f).unwrap();
        }
        let _ = to_cape_png(&out);
    }

    #[test]
    fn bad_files_are_rejected_clearly() {
        assert!(to_cape_png(b"GIF89a").is_err());
        assert!(to_cape_png(b"").is_err());
        assert!(to_cape_png(b"not a gif at all").is_err());
        let e = to_cape_png(&make_gif(100, 100, &[([0, 0, 0], 10)])).unwrap_err();
        assert!(e.contains("2:1"), "{e}");
        let e = to_cape_png(&make_gif(32, 16, &[([0, 0, 0], 10)])).unwrap_err();
        assert!(e.contains("too small"), "{e}");
        let e = to_cape_png(&make_gif(8192, 4096, &[([0, 0, 0], 10)])).unwrap_err();
        assert!(e.contains("too large"), "{e}");
        let mut cut = make_gif(128, 64, &[([5, 5, 5], 10), ([6, 6, 6], 10)]);
        cut.truncate(cut.len() / 2);
        assert!(to_cape_png(&cut).is_err());
        // Over the byte limit before any decoding.
        let many = vec![([0, 0, 0], 10); 130];
        let e = to_cape_png(&make_gif(2048, 1024, &many)).unwrap_err();
        assert!(e.contains("too many frames"), "{e}");
        let mut big = make_gif(64, 32, &[([0, 0, 0], 10)]);
        big.resize(MAX_CAPE_BYTES + 1, 0);
        assert_eq!(to_cape_png(&big).unwrap_err(), "image is too large");
    }
}
