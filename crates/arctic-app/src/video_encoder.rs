//! Replay videos without FFmpeg: on Windows the game pipes raw frames into
//! `Arctic Launcher.exe --encode-video`, which turns them into an MP4 with
//! Windows' own H.264 encoder (the graphics card's when it has one). Nothing
//! is downloaded.
//!
//! Input: RGBA frames, top row first, `width * height * 4` bytes each, on
//! stdin until it closes.

use std::io::Read;
use std::path::Path;

/// The command-line switch that runs the encoder instead of the launcher.
pub const SWITCH: &str = "--encode-video";

/// Bits per pixel per frame: about CRF 18 x264 for Minecraft's detail.
const BITS_PER_PIXEL: f64 = 0.18;
const MIN_BITRATE: u32 = 2_000_000;
const MAX_BITRATE: u32 = 80_000_000;

/// `--encode-video <width> <height> <fps> <out.mp4>`: runs the encoder and
/// returns the process exit code.
pub fn run(args: &[String]) -> i32 {
    let parsed = (|| {
        let [w, h, fps, out] = args else {
            return None;
        };
        Some((w.parse().ok()?, h.parse().ok()?, fps.parse().ok()?, out))
    })();
    let Some((width, height, fps, out)) = parsed else {
        eprintln!("usage: {SWITCH} <width> <height> <fps> <out.mp4>");
        return 2;
    };
    match encode(std::io::stdin().lock(), width, height, fps, Path::new(out)) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("video export: {e}");
            1
        }
    }
}

/// Can this computer encode H.264 without FFmpeg? (Asked once.)
pub fn available() -> bool {
    static AVAILABLE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *AVAILABLE.get_or_init(mf::has_h264_encoder)
}

/// The video's size: H.264 wants even sides, so an odd last row or column is
/// dropped.
fn even_size(width: u32, height: u32) -> (u32, u32) {
    (width & !1, height & !1)
}

fn bitrate(width: u32, height: u32, fps: u32) -> u32 {
    let bits = f64::from(width) * f64::from(height) * f64::from(fps) * BITS_PER_PIXEL;
    (bits as u32).clamp(MIN_BITRATE, MAX_BITRATE)
}

/// Encode RGBA frames from `input` into an MP4 at `out`.
pub fn encode(
    mut input: impl Read,
    width: u32,
    height: u32,
    fps: u32,
    out: &Path,
) -> Result<(), String> {
    let (w, h) = even_size(width, height);
    if w == 0 || h == 0 || fps == 0 {
        return Err(format!("bad size {width}x{height} at {fps} fps"));
    }
    let mut writer = mf::Writer::open(out, w, h, fps, bitrate(w, h, fps))?;
    let mut rgba = vec![0u8; width as usize * height as usize * 4];
    let mut nv12 = vec![0u8; w as usize * h as usize * 3 / 2];
    let mut frames = 0u64;
    while read_frame(&mut input, &mut rgba)? {
        rgba_to_nv12(&rgba, width as usize, w as usize, h as usize, &mut nv12);
        writer.write(&nv12, frames)?;
        frames += 1;
    }
    if frames == 0 {
        return Err("no frames".into());
    }
    writer.finish()
}

/// Fill `frame`; false at the end of the input (a cut-off frame is dropped).
fn read_frame(input: &mut impl Read, frame: &mut [u8]) -> Result<bool, String> {
    let mut filled = 0;
    while filled < frame.len() {
        match input.read(&mut frame[filled..]) {
            Ok(0) => return Ok(false),
            Ok(n) => filled += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(format!("reading frames: {e}")),
        }
    }
    Ok(true)
}

/// RGBA (rows of `stride` pixels) to NV12 (`w` x `h`, both even): BT.709,
/// limited range, the colors players expect from HD video.
fn rgba_to_nv12(rgba: &[u8], stride: usize, w: usize, h: usize, nv12: &mut [u8]) {
    let (luma, chroma) = nv12.split_at_mut(w * h);
    let px = |x: usize, y: usize| {
        let i = (y * stride + x) * 4;
        (
            i32::from(rgba[i]),
            i32::from(rgba[i + 1]),
            i32::from(rgba[i + 2]),
        )
    };
    for y in (0..h).step_by(2) {
        for x in (0..w).step_by(2) {
            let (mut rs, mut gs, mut bs) = (0, 0, 0);
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let (r, g, b) = px(x + dx, y + dy);
                luma[(y + dy) * w + x + dx] = ((47 * r + 157 * g + 16 * b + 128) >> 8) as u8 + 16;
                rs += r;
                gs += g;
                bs += b;
            }
            // Averages of the 2x2 block, scaled by 4 (hence >> 10).
            let u = ((-26 * rs - 86 * gs + 112 * bs + 512) >> 10) + 128;
            let v = ((112 * rs - 102 * gs - 10 * bs + 512) >> 10) + 128;
            let at = (y / 2) * w + x;
            chroma[at] = u.clamp(0, 255) as u8;
            chroma[at + 1] = v.clamp(0, 255) as u8;
        }
    }
}

#[cfg(windows)]
mod mf {
    use std::path::Path;

    use windows::Win32::Media::MediaFoundation::*;
    use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoTaskMemFree};
    use windows::core::{GUID, HSTRING};

    /// 100-nanosecond units per second (Media Foundation's clock).
    const TICKS: i64 = 10_000_000;

    fn start() -> Result<(), String> {
        unsafe {
            // Fine if COM was already set up on this thread.
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            MFStartup(MF_VERSION, MFSTARTUP_FULL).map_err(|e| format!("Media Foundation: {e}"))
        }
    }

    pub fn has_h264_encoder() -> bool {
        if start().is_err() {
            return false;
        }
        let output = MFT_REGISTER_TYPE_INFO {
            guidMajorType: MFMediaType_Video,
            guidSubtype: MFVideoFormat_H264,
        };
        let flags = MFT_ENUM_FLAG(
            MFT_ENUM_FLAG_SYNCMFT.0
                | MFT_ENUM_FLAG_ASYNCMFT.0
                | MFT_ENUM_FLAG_HARDWARE.0
                | MFT_ENUM_FLAG_SORTANDFILTER.0,
        );
        let mut found: *mut Option<IMFActivate> = std::ptr::null_mut();
        let mut count = 0u32;
        let ok = unsafe {
            MFTEnumEx(
                MFT_CATEGORY_VIDEO_ENCODER,
                flags,
                None,
                Some(&output),
                &mut found,
                &mut count,
            )
        }
        .is_ok();
        if !found.is_null() {
            unsafe {
                for i in 0..count as usize {
                    std::ptr::drop_in_place(found.add(i));
                }
                CoTaskMemFree(Some(found.cast()));
            }
        }
        ok && count > 0
    }

    pub struct Writer {
        writer: IMFSinkWriter,
        stream: u32,
        fps: u32,
    }

    impl Writer {
        pub fn open(out: &Path, w: u32, h: u32, fps: u32, bitrate: u32) -> Result<Self, String> {
            start()?;
            // The graphics card's encoder when there is one; if it won't
            // take this video, Windows' own.
            let hardware = std::env::var_os("ARCTIC_SOFTWARE_ENCODER").is_none();
            let first = if hardware {
                Self::try_open(out, w, h, fps, bitrate, true)
            } else {
                Err("hardware encoder off".into())
            };
            first.or_else(|_| Self::try_open(out, w, h, fps, bitrate, false))
        }

        fn try_open(
            out: &Path,
            w: u32,
            h: u32,
            fps: u32,
            bitrate: u32,
            hardware: bool,
        ) -> Result<Self, String> {
            let err = |what: &str| {
                let what = what.to_owned();
                move |e: windows::core::Error| format!("{what}: {e}")
            };
            unsafe {
                let mut attrs = None;
                MFCreateAttributes(&mut attrs, 2).map_err(err("attributes"))?;
                let attrs = attrs.ok_or("no attributes")?;
                attrs
                    .SetUINT32(
                        &MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS,
                        u32::from(hardware),
                    )
                    .map_err(err("attributes"))?;
                // Frames come as fast as the game draws them; no pacing.
                attrs
                    .SetUINT32(&MF_SINK_WRITER_DISABLE_THROTTLING, 1)
                    .map_err(err("attributes"))?;
                let _ = std::fs::remove_file(out);
                let url = HSTRING::from(out.as_os_str());
                let writer =
                    MFCreateSinkWriterFromURL(&url, None, &attrs).map_err(err("output file"))?;

                let target = video_type(&MFVideoFormat_H264, w, h, fps).map_err(err("H.264"))?;
                target
                    .SetUINT32(&MF_MT_AVG_BITRATE, bitrate)
                    .map_err(err("H.264"))?;
                target
                    .SetUINT32(&MF_MT_MPEG2_PROFILE, eAVEncH264VProfile_High.0 as u32)
                    .map_err(err("H.264"))?;
                let stream = writer.AddStream(&target).map_err(err("H.264 stream"))?;
                let input = video_type(&MFVideoFormat_NV12, w, h, fps).map_err(err("frames"))?;
                writer
                    .SetInputMediaType(stream, &input, None)
                    .map_err(err("no H.264 encoder takes these frames"))?;
                writer.BeginWriting().map_err(err("starting"))?;
                Ok(Self {
                    writer,
                    stream,
                    fps,
                })
            }
        }

        /// Frame number `index`, as NV12.
        pub fn write(&mut self, nv12: &[u8], index: u64) -> Result<(), String> {
            let fps = i64::from(self.fps);
            let at = index as i64 * TICKS / fps;
            let next = (index as i64 + 1) * TICKS / fps;
            unsafe {
                let len = nv12.len() as u32;
                let buffer = MFCreateMemoryBuffer(len).map_err(|e| format!("buffer: {e}"))?;
                let mut data: *mut u8 = std::ptr::null_mut();
                buffer
                    .Lock(&mut data, None, None)
                    .map_err(|e| format!("buffer: {e}"))?;
                std::ptr::copy_nonoverlapping(nv12.as_ptr(), data, nv12.len());
                buffer.Unlock().map_err(|e| format!("buffer: {e}"))?;
                buffer
                    .SetCurrentLength(len)
                    .map_err(|e| format!("buffer: {e}"))?;
                let sample = MFCreateSample().map_err(|e| format!("sample: {e}"))?;
                sample
                    .AddBuffer(&buffer)
                    .map_err(|e| format!("sample: {e}"))?;
                sample
                    .SetSampleTime(at)
                    .map_err(|e| format!("sample: {e}"))?;
                sample
                    .SetSampleDuration(next - at)
                    .map_err(|e| format!("sample: {e}"))?;
                self.writer
                    .WriteSample(self.stream, &sample)
                    .map_err(|e| format!("encoding: {e}"))
            }
        }

        pub fn finish(self) -> Result<(), String> {
            unsafe { self.writer.Finalize() }.map_err(|e| format!("finishing the file: {e}"))
        }
    }

    /// A progressive video type with BT.709 limited-range colors.
    unsafe fn video_type(
        subtype: &GUID,
        w: u32,
        h: u32,
        fps: u32,
    ) -> windows::core::Result<IMFMediaType> {
        unsafe {
            let t = MFCreateMediaType()?;
            t.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
            t.SetGUID(&MF_MT_SUBTYPE, subtype)?;
            t.SetUINT64(&MF_MT_FRAME_SIZE, (u64::from(w) << 32) | u64::from(h))?;
            t.SetUINT64(&MF_MT_FRAME_RATE, (u64::from(fps) << 32) | 1)?;
            t.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, (1 << 32) | 1)?;
            t.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
            t.SetUINT32(&MF_MT_YUV_MATRIX, MFVideoTransferMatrix_BT709.0 as u32)?;
            t.SetUINT32(&MF_MT_VIDEO_PRIMARIES, MFVideoPrimaries_BT709.0 as u32)?;
            t.SetUINT32(&MF_MT_TRANSFER_FUNCTION, MFVideoTransFunc_709.0 as u32)?;
            t.SetUINT32(&MF_MT_VIDEO_NOMINAL_RANGE, MFNominalRange_16_235.0 as u32)?;
            Ok(t)
        }
    }
}

#[cfg(not(windows))]
mod mf {
    use std::path::Path;

    pub fn has_h264_encoder() -> bool {
        false
    }

    pub struct Writer;

    impl Writer {
        pub fn open(_: &Path, _: u32, _: u32, _: u32, _: u32) -> Result<Self, String> {
            Err("the built-in encoder is Windows-only; use FFmpeg".into())
        }

        pub fn write(&mut self, _: &[u8], _: u64) -> Result<(), String> {
            Ok(())
        }

        pub fn finish(self) -> Result<(), String> {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn odd_sides_lose_a_row_or_column() {
        assert_eq!(even_size(1921, 1080), (1920, 1080));
        assert_eq!(even_size(855, 481), (854, 480));
    }

    #[test]
    fn bitrate_grows_with_the_picture_within_limits() {
        assert_eq!(bitrate(1920, 1080, 60), 22_394_880);
        assert_eq!(bitrate(16, 16, 1), MIN_BITRATE);
        assert_eq!(bitrate(7680, 4320, 120), MAX_BITRATE);
    }

    #[test]
    fn colors_convert_to_video_levels() {
        // 2x2 of white, then 2x2 of black (a 4x2 picture, stride 5 with an
        // odd column that's dropped).
        let mut rgba = Vec::new();
        for _ in 0..2 {
            for x in 0..5 {
                let c = if x < 2 { 255 } else { 0 };
                rgba.extend_from_slice(&[c, c, c, 255]);
            }
        }
        let mut nv12 = vec![0; 4 * 2 * 3 / 2];
        rgba_to_nv12(&rgba, 5, 4, 2, &mut nv12);
        assert_eq!(&nv12[..8], &[235, 235, 16, 16, 235, 235, 16, 16]);
        // Grays have no color: U and V sit at the middle.
        assert_eq!(&nv12[8..], &[128, 128, 128, 128]);
    }

    #[test]
    fn red_leans_to_v() {
        let rgba = [255, 0, 0, 255].repeat(4);
        let mut nv12 = vec![0; 6];
        rgba_to_nv12(&rgba, 2, 2, 2, &mut nv12);
        assert!((60..=66).contains(&nv12[0]), "Y {}", nv12[0]);
        assert!(nv12[4] < 110, "U {}", nv12[4]);
        assert!(nv12[5] > 230, "V {}", nv12[5]);
    }

    #[test]
    fn a_cut_off_frame_ends_the_input() {
        let mut input: &[u8] = &[1, 2, 3, 4, 5, 6];
        let mut frame = [0u8; 4];
        assert!(read_frame(&mut input, &mut frame).unwrap());
        assert!(!read_frame(&mut input, &mut frame).unwrap());
    }

    #[cfg(windows)]
    #[test]
    fn encodes_an_mp4() {
        if !available() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("test.mp4");
        let (w, h, fps) = (321u32, 181u32, 30u32);
        let mut frames = Vec::new();
        for i in 0..45u32 {
            for y in 0..h {
                for x in 0..w {
                    frames.extend_from_slice(&[(x + i * 4) as u8, y as u8, (i * 5) as u8, 255]);
                }
            }
        }
        encode(frames.as_slice(), w, h, fps, &out).unwrap();
        let mp4 = std::fs::read(&out).unwrap();
        assert!(mp4.len() > 1000, "{} bytes", mp4.len());
        assert_eq!(&mp4[4..8], b"ftyp");
    }
}
