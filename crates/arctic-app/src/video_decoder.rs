//! Read-only software decoding for replay export audits. Runs as a separate
//! hidden child, before launcher initialization, and emits bounded frame hashes.
use std::{
    io::{self, Write},
    path::Path,
};

pub const SWITCH: &str = "--decode-video-frames";
pub fn run(args: &[String]) -> i32 {
    let result = (|| -> Result<(), String> {
        let [width, height, limit, path] = args else {
            return Err("expected width height frame-count local-video-file".into());
        };
        let parse = |value: &String| {
            value
                .parse::<u32>()
                .map_err(|_| "invalid decoder geometry or count".to_owned())
        };
        let (width, height, limit) = (parse(width)?, parse(height)?, parse(limit)?);
        if width < 2
            || height < 2
            || width > 3840
            || height > 2160
            || width % 2 != 0
            || height % 2 != 0
            || limit == 0
            || limit > 432_000
            || !Path::new(path).is_absolute()
            || !Path::new(path).is_file()
        {
            return Err("invalid bounded local video audit".into());
        }
        decode(
            Path::new(path),
            width,
            height,
            limit,
            &mut io::stdout().lock(),
        )
    })();
    match result {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("video decode audit: {error}");
            1
        }
    }
}
#[cfg(not(windows))]
fn decode(_: &Path, _: u32, _: u32, _: u32, _: &mut impl Write) -> Result<(), String> {
    Err("built-in video decoding requires Windows".into())
}

#[cfg(windows)]
fn decode(
    path: &Path,
    width: u32,
    height: u32,
    limit: u32,
    output: &mut impl Write,
) -> Result<(), String> {
    use windows::{
        Win32::{
            Media::MediaFoundation::*,
            System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize},
        },
        core::HSTRING,
    };
    struct Runtime;
    impl Drop for Runtime {
        fn drop(&mut self) {
            unsafe {
                let _ = MFShutdown();
                CoUninitialize();
            }
        }
    }
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED)
            .ok()
            .map_err(|e| e.to_string())?;
        if let Err(error) = MFStartup(MF_VERSION, MFSTARTUP_FULL) {
            CoUninitialize();
            return Err(error.to_string());
        }
        let _runtime = Runtime;
        let mut attributes = None;
        MFCreateAttributes(&mut attributes, 2).map_err(|e| e.to_string())?;
        let attributes = attributes.ok_or("missing decoder attributes")?;
        attributes
            .SetUINT32(&MF_SOURCE_READER_DISABLE_DXVA, 1)
            .map_err(|e| e.to_string())?;
        attributes
            .SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 0)
            .map_err(|e| e.to_string())?;
        let reader = MFCreateSourceReaderFromURL(&HSTRING::from(path.as_os_str()), &attributes)
            .map_err(|e| e.to_string())?;
        let stream = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
        reader
            .SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)
            .map_err(|e| e.to_string())?;
        reader
            .SetStreamSelection(stream, true)
            .map_err(|e| e.to_string())?;
        let target = MFCreateMediaType().map_err(|e| e.to_string())?;
        target
            .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
            .map_err(|e| e.to_string())?;
        target
            .SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)
            .map_err(|e| e.to_string())?;
        reader
            .SetCurrentMediaType(stream, None, &target)
            .map_err(|e| e.to_string())?;
        let validate = || -> Result<IMFMediaType, String> {
            let media = reader
                .GetCurrentMediaType(stream)
                .map_err(|e| e.to_string())?;
            if media.GetGUID(&MF_MT_SUBTYPE).map_err(|e| e.to_string())? != MFVideoFormat_NV12 {
                return Err("decoded format is not NV12".into());
            }
            decoded_layout(&media, width, height)?;
            Ok(media)
        };
        validate()?;
        writeln!(output,"#format: frame checksums\n#version: 2\n#hash: SHA256\n#tb 0: 1/10000000\n#media_type 0: video\n#codec_id 0: rawvideo\n#dimensions 0: {width}x{height}")
            .map_err(|e|e.to_string())?;
        output.flush().map_err(|e| e.to_string())?;
        let mut count = 0u32;
        let mut empty = 0u32;
        loop {
            let mut flags = 0u32;
            let mut time = 0i64;
            let mut sample = None;
            reader
                .ReadSample(
                    stream,
                    0,
                    None,
                    Some(&mut flags),
                    Some(&mut time),
                    Some(&mut sample),
                )
                .map_err(|e| e.to_string())?;
            if flags & MF_SOURCE_READERF_ERROR.0 as u32 != 0 {
                return Err("video decoder reported a stream error".into());
            }
            // A decoder may resolve its first output type on the first sample.
            let media = validate()?;
            if let Some(sample) = sample {
                if count == limit {
                    return Err("decoded video contains extra frames".into());
                }
                let pixels = canonical_nv12(&sample, &media, width, height)?;
                let duration = sample.GetSampleDuration().unwrap_or(0);
                if duration < 0 {
                    return Err("negative decoded frame duration".into());
                }
                let hash = arctic_core::bedrock::video_audit::frame_sha256(&pixels);
                writeln!(
                    output,
                    "0, {time}, {time}, {duration}, {}, {hash}",
                    pixels.len()
                )
                .map_err(|e| e.to_string())?;
                output.flush().map_err(|e| e.to_string())?;
                count += 1;
                empty = 0;
            } else {
                empty += 1;
                if empty > 1024 {
                    return Err("video decoder produced no frames".into());
                }
            }
            if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                break;
            }
        }
        if count != limit {
            return Err(format!("decoded {count} frames, expected {limit}"));
        }
        Ok(())
    }
}

#[cfg(windows)]
fn canonical_nv12(
    sample: &windows::Win32::Media::MediaFoundation::IMFSample,
    media: &windows::Win32::Media::MediaFoundation::IMFMediaType,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, String> {
    use windows::{Win32::Media::MediaFoundation::*, core::Interface};
    let (coded_width, coded_height, x, y) = decoded_layout(media, width, height)?;
    let expected = coded_width as usize * coded_height as usize * 3 / 2;
    unsafe {
        if sample.GetTotalLength().map_err(|e| e.to_string())? > 64 * 1024 * 1024 {
            return Err("decoded sample exceeds buffer bound".into());
        }
        let buffer = sample
            .ConvertToContiguousBuffer()
            .map_err(|e| e.to_string())?;
        let mut output = vec![0; expected];
        if let Ok(planar) = buffer.cast::<IMF2DBuffer>() {
            if planar.GetContiguousLength().map_err(|e| e.to_string())? as usize != expected {
                return Err("unexpected contiguous NV12 length".into());
            }
            planar
                .ContiguousCopyTo(&mut output)
                .map_err(|e| e.to_string())?;
        } else {
            let mut data = std::ptr::null_mut();
            let mut length = 0;
            buffer
                .Lock(&mut data, None, Some(&mut length))
                .map_err(|e| e.to_string())?;
            struct Unlock<'a>(&'a IMFMediaBuffer);
            impl Drop for Unlock<'_> {
                fn drop(&mut self) {
                    unsafe {
                        let _ = self.0.Unlock();
                    }
                }
            }
            let _unlock = Unlock(&buffer);
            if data.is_null() || length == 0 || length > 64 * 1024 * 1024 {
                return Err("invalid locked NV12 data".into());
            }
            let stride = media
                .GetUINT32(&MF_MT_DEFAULT_STRIDE)
                .ok()
                .map(|v| v as i32);
            let pitch = if length as usize == expected {
                coded_width as usize
            } else {
                stride
                    .filter(|value| {
                        *value >= coded_width as i32 && *value <= 16384 && *value % 2 == 0
                    })
                    .ok_or("missing or invalid padded NV12 stride")? as usize
            };
            let rows = coded_height as usize * 3 / 2;
            if pitch.checked_mul(rows) != Some(length as usize) {
                return Err("NV12 stride does not cover the decoded buffer".into());
            }
            let source = std::slice::from_raw_parts(data, length as usize);
            for row in 0..rows {
                output[row * coded_width as usize..(row + 1) * coded_width as usize]
                    .copy_from_slice(&source[row * pitch..row * pitch + coded_width as usize]);
            }
        }
        if (coded_width, coded_height, x, y) == (width, height, 0, 0) {
            return Ok(output);
        }
        let mut cropped = vec![0; width as usize * height as usize * 3 / 2];
        for row in 0..height as usize {
            let at = (row + y as usize) * coded_width as usize + x as usize;
            cropped[row * width as usize..(row + 1) * width as usize]
                .copy_from_slice(&output[at..at + width as usize]);
        }
        let source_uv = coded_width as usize * coded_height as usize;
        let target_uv = width as usize * height as usize;
        for row in 0..height as usize / 2 {
            let at = source_uv + (row + y as usize / 2) * coded_width as usize + x as usize;
            cropped[target_uv + row * width as usize..target_uv + (row + 1) * width as usize]
                .copy_from_slice(&output[at..at + width as usize]);
        }
        Ok(cropped)
    }
}

#[cfg(windows)]
fn decoded_layout(
    media: &windows::Win32::Media::MediaFoundation::IMFMediaType,
    width: u32,
    height: u32,
) -> Result<(u32, u32, u32, u32), String> {
    use windows::Win32::Media::MediaFoundation::*;
    unsafe {
        let size = media
            .GetUINT64(&MF_MT_FRAME_SIZE)
            .map_err(|e| e.to_string())?;
        let (w, h) = ((size >> 32) as u32, size as u32);
        if w < 2
            || h < 2
            || w > 4096
            || h > 2304
            || w % 2 != 0
            || h % 2 != 0
            || media.GetUINT32(&MF_MT_PAN_SCAN_ENABLED).unwrap_or(0) != 0
        {
            return Err("unsupported decoded canvas or pan-scan".into());
        }
        for key in [MF_MT_MINIMUM_DISPLAY_APERTURE, MF_MT_GEOMETRIC_APERTURE] {
            if let Ok(length) = media.GetBlobSize(&key) {
                if length != 16 {
                    return Err("invalid decoded display aperture size".into());
                }
                let mut area = [0u8; 16];
                media
                    .GetBlob(&key, &mut area, None)
                    .map_err(|e| e.to_string())?;
                let x = i16::from_le_bytes([area[2], area[3]]);
                let y = i16::from_le_bytes([area[6], area[7]]);
                let aw =
                    i32::from_le_bytes(area[8..12].try_into().map_err(|_| "invalid aperture")?);
                let ah =
                    i32::from_le_bytes(area[12..16].try_into().map_err(|_| "invalid aperture")?);
                if area[0..2] != [0, 0]
                    || area[4..6] != [0, 0]
                    || x < 0
                    || y < 0
                    || x % 2 != 0
                    || y % 2 != 0
                    || aw != width as i32
                    || ah != height as i32
                    || u32::from(x as u16) + width > w
                    || u32::from(y as u16) + height > h
                {
                    return Err("decoded aperture differs from captured pixels".into());
                }
                return Ok((w, h, x as u32, y as u32));
            }
        }
        if (w, h) != (width, height) {
            return Err("decoded canvas differs without a declared display aperture".into());
        }
        Ok((w, h, 0, 0))
    }
}
