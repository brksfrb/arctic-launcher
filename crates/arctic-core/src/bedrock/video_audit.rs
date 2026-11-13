//! Actual decoded NV12 frame receipts after export. This checks one encoded
//! artifact; it does not establish native scene or repeat-run determinism.
use super::capture::EncoderKind;
use crate::{Error, Result, error::IoContext};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

/// Canonical pixel bytes only: no stride padding or transient frame identities.
pub fn frame_sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
fn fail(message: impl Into<String>) -> Error {
    Error::Other(message.into())
}
fn canceled(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        Err(fail("video audit canceled"))
    } else {
        Ok(())
    }
}
pub(super) fn open_finished(path: &Path, cancel: &AtomicBool) -> Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    // A capture worker may have sent completion just before dropping its files.
    // Deny concurrent writers/deletion while reading, but allow decoder readers.
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0x00000001);
    }
    let started = Instant::now();
    loop {
        canceled(cancel)?;
        match options.open(path) {
            Ok(file) => return Ok(file),
            Err(error)
                if error.kind() == std::io::ErrorKind::PermissionDenied
                    && started.elapsed() < Duration::from_secs(1) =>
            {
                thread::sleep(Duration::from_millis(25))
            }
            Err(error) => return Err(Error::io(path, error)),
        }
    }
}
pub(super) fn hash_valid(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
fn line(reader: &mut impl BufRead, limit: usize) -> std::io::Result<Option<String>> {
    let mut bytes = Vec::new();
    let count = reader
        .take((limit + 1) as u64)
        .read_until(b'\n', &mut bytes)?;
    if count == 0 {
        return Ok(None);
    }
    if count > limit || bytes.last() != Some(&b'\n') {
        return Err(std::io::Error::other("oversized or incomplete audit line"));
    }
    String::from_utf8(bytes)
        .map(|s| Some(s.trim().to_owned()))
        .map_err(std::io::Error::other)
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoAudit {
    pub path: PathBuf,
    pub frames: u32,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub input_width: u32,
    pub input_height: u32,
    pub decoder_backend: EncoderKind,
    pub encoded_file_sha256: String,
    pub input_rgba_sequence_sha256: String,
    pub decoded_nv12_sequence_sha256: String,
    pub first_presentation_timestamp: i64,
    pub time_base_numerator: i64,
    pub time_base_denominator: i64,
    pub decoded_frame_count_verified: bool,
    pub presentation_cadence_verified: bool,
    pub deterministic_video_verified: bool,
}
#[path = "video_compare.rs"]
mod comparison;
pub use comparison::{ExportComparison, FrameDifference, compare};
pub struct Request<'a> {
    pub executable: &'a Path,
    pub encoder: EncoderKind,
    pub video: &'a Path,
    pub capture_id: &'a str,
    pub frames: u32,
    pub fps: u32,
}
struct Manifest {
    width: u32,
    height: u32,
    input_width: u32,
    input_height: u32,
    input_hash: String,
}
fn manifest(
    request: &Request<'_>,
    cancel: &AtomicBool,
    progress: &mut impl FnMut(&'static str, u32),
) -> Result<Manifest> {
    let path = request.video.with_extension("frames.jsonl");
    let mut reader = BufReader::new(open_finished(&path, cancel)?);
    let before = reader.get_ref().metadata().at(&path)?;
    if !before.is_file() {
        return Err(fail("capture manifest is not a file"));
    }
    let encoder = match request.encoder {
        EncoderKind::ArcticBuiltin => "arctic_builtin",
        EncoderKind::Ffmpeg => "ffmpeg",
    };
    let mut size = None;
    let mut original = None;
    let mut count = 0u32;
    let mut hash = Sha256::new();
    hash.update(b"arctic-rgba-sequence-v1\0");
    hash.update(request.fps.to_le_bytes());
    hash.update(request.frames.to_le_bytes());
    while let Some(text) = line(&mut reader, 1024 * 1024).at(&path)? {
        canceled(cancel)?;
        if count == request.frames {
            return Err(fail("capture manifest has extra frame rows"));
        }
        let value: Value = serde_json::from_str(&text)?;
        let width = value["width"]
            .as_u64()
            .filter(|n| (2..=3840).contains(n))
            .ok_or_else(|| fail("invalid captured width"))? as u32;
        let height = value["height"]
            .as_u64()
            .filter(|n| (2..=2160).contains(n))
            .ok_or_else(|| fail("invalid captured height"))? as u32;
        let current = ((width + 1) & !1, (height + 1) & !1);
        let profile_valid = match value["encoding"]["schema"].as_u64() {
            Some(1) => value["encoding"].get("profile").is_none(),
            Some(2) => {
                value["encoding"]["profile"].as_str() == Some(request.encoder.indexed_profile())
            }
            _ => false,
        };
        if size.is_some_and(|size| size != current)
            || original.is_some_and(|size| size != (width, height))
            || value["encodedWidth"].as_u64() != Some(u64::from(current.0))
            || value["encodedHeight"].as_u64() != Some(u64::from(current.1))
            || value["captureId"].as_str() != Some(request.capture_id)
            || value["encoder"].as_str() != Some(encoder)
            || value["index"].as_u64() != Some(u64::from(count))
            || !profile_valid
            || value["encoding"]["encoderInputWritten"].as_bool() != Some(true)
            || value["encoding"]["sourcePixelFormat"].as_str() != Some("rgba8")
            || value["encoding"]["sourceRange"].as_str() != Some("full")
            || value["encoding"]["sourceAlpha"].as_str() != Some("discarded")
            || value["encoding"]["requestedOutput"]["matrix"].as_str() != Some("bt709")
            || value["encoding"]["requestedOutput"]["primaries"].as_str() != Some("bt709")
            || value["encoding"]["requestedOutput"]["transfer"].as_str() != Some("bt709")
            || value["encoding"]["requestedOutput"]["range"].as_str() != Some("limited")
            || value["encoding"]["requestedOutput"]["chromaSubsampling"].as_str() != Some("420")
            || value["encoding"]["requestedOutput"]["chromaLocation"].as_str() != Some("center")
            || value["encoding"]["requestedOutput"]["padding"].as_str()
                != Some("black_right_bottom")
            || value["encoding"]["time"]["numerator"].as_u64() != Some(u64::from(count))
            || value["encoding"]["time"]["denominator"].as_u64() != Some(u64::from(request.fps))
            || value["encoding"]["time"]["timescale"].as_u64() != Some(10_000_000)
            || value["encoding"]["time"]["start100ns"].as_u64()
                != Some(u64::from(count) * 10_000_000 / u64::from(request.fps))
            || value["encoding"]["time"]["duration100ns"].as_u64()
                != Some(
                    (u64::from(count) + 1) * 10_000_000 / u64::from(request.fps)
                        - u64::from(count) * 10_000_000 / u64::from(request.fps),
                )
        {
            return Err(fail(
                "capture manifest has inconsistent geometry, identity, encoder, color request or frame timing",
            ));
        }
        let rgba = value["rgbaSha256"]
            .as_str()
            .filter(|s| hash_valid(s))
            .ok_or_else(|| fail("capture manifest has no valid RGBA digest"))?;
        hash.update(count.to_le_bytes());
        hash.update(width.to_le_bytes());
        hash.update(height.to_le_bytes());
        hash.update(hex::decode(rgba).map_err(|e| fail(e.to_string()))?);
        size = Some(current);
        original = Some((width, height));
        count += 1;
        progress("checking_manifest", count);
    }
    if count != request.frames {
        return Err(fail("capture manifest ended before every frame"));
    }
    let after = reader.get_ref().metadata().at(&path)?;
    if before.len() != after.len() || before.modified().ok() != after.modified().ok() {
        return Err(fail("capture manifest changed during audit"));
    }
    let (width, height) = size.ok_or_else(|| fail("empty capture manifest"))?;
    let (input_width, input_height) = original.ok_or_else(|| fail("empty capture manifest"))?;
    Ok(Manifest {
        width,
        height,
        input_width,
        input_height,
        input_hash: hex::encode(hash.finalize()),
    })
}
enum Event {
    Line(String),
    End,
    Error(String),
}
struct Decoder {
    child: Child,
    receiver: Option<Receiver<Event>>,
    reader: Option<JoinHandle<()>>,
    stderr: Option<JoinHandle<String>>,
    terminal: bool,
}
impl Decoder {
    fn start(request: &Request<'_>, width: u32, height: u32) -> Result<Self> {
        let mut command = Command::new(request.executable);
        match request.encoder {
            EncoderKind::ArcticBuiltin => {
                command
                    .arg("--decode-video-frames")
                    .arg(width.to_string())
                    .arg(height.to_string())
                    .arg(request.frames.to_string())
                    .arg(request.video);
            }
            EncoderKind::Ffmpeg => {
                command
                    .args(["-v", "error", "-nostdin", "-noautorotate", "-i"])
                    .arg(request.video)
                    .args([
                        "-map",
                        "0:v:0",
                        "-an",
                        "-sn",
                        "-dn",
                        "-c:v",
                        "rawvideo",
                        "-pix_fmt",
                        "nv12",
                        "-fps_mode",
                        "passthrough",
                        "-f",
                        "framehash",
                        "-hash",
                        "sha256",
                        "-format_version",
                        "2",
                        "pipe:1",
                    ]);
            }
        }
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }
        let mut child = command.spawn().at(request.executable)?;
        let stdout = match child.stdout.take() {
            Some(out) => out,
            None => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(fail("decoder has no hash output"));
            }
        };
        let stderr = child.stderr.take();
        let (send, receive) = mpsc::sync_channel(8);
        let mut decoder = Self {
            child,
            receiver: Some(receive),
            reader: None,
            stderr: None,
            terminal: false,
        };
        decoder.reader = Some(
            thread::Builder::new()
                .name("video-audit-frames".into())
                .spawn(move || {
                    let mut input = BufReader::new(stdout);
                    loop {
                        let event = match line(&mut input, 4096) {
                            Ok(Some(text)) => Event::Line(text),
                            Ok(None) => Event::End,
                            Err(error) => Event::Error(error.to_string()),
                        };
                        let end = !matches!(&event, Event::Line(_));
                        if send.send(event).is_err() || end {
                            break;
                        }
                    }
                })
                .map_err(|e| fail(format!("starting decoder reader: {e}")))?,
        );
        decoder.stderr = Some(
            thread::Builder::new()
                .name("video-audit-errors".into())
                .spawn(move || {
                    let mut tail = Vec::new();
                    if let Some(mut input) = stderr {
                        let mut bytes = [0; 1024];
                        loop {
                            match input.read(&mut bytes) {
                                Ok(0) => break,
                                Ok(n) => {
                                    tail.extend_from_slice(&bytes[..n]);
                                    if tail.len() > 8192 {
                                        tail.drain(..tail.len() - 8192);
                                    }
                                }
                                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {
                                    continue;
                                }
                                Err(_) => break,
                            }
                        }
                    }
                    String::from_utf8_lossy(&tail).into_owned()
                })
                .map_err(|e| fail(format!("starting decoder diagnostics: {e}")))?,
        );
        Ok(decoder)
    }
    fn stop(&mut self) -> String {
        if !self.terminal {
            let _ = self.child.kill();
            let _ = self.child.wait();
            self.terminal = true;
        }
        drop(self.receiver.take());
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        self.stderr
            .take()
            .and_then(|worker| worker.join().ok())
            .unwrap_or_default()
    }
}
impl Drop for Decoder {
    fn drop(&mut self) {
        self.stop();
    }
}
struct Frames {
    count: u32,
    headers: u32,
    format: bool,
    version: bool,
    hash: bool,
    video: bool,
    raw: bool,
    size: bool,
    time_base: Option<(i64, i64)>,
    first: Option<i64>,
    last: Option<i64>,
    digest: Sha256,
}
impl Frames {
    fn new(request: &Request<'_>, size: &Manifest) -> Self {
        let mut digest = Sha256::new();
        digest.update(b"arctic-nv12-sequence-v1\0");
        for value in [size.width, size.height, request.fps, request.frames] {
            digest.update(value.to_le_bytes());
        }
        Self {
            count: 0,
            headers: 0,
            format: false,
            version: false,
            hash: false,
            video: false,
            raw: false,
            size: false,
            time_base: None,
            first: None,
            last: None,
            digest,
        }
    }
    fn accept(
        &mut self,
        text: &str,
        request: &Request<'_>,
        size: &Manifest,
        output: &mut File,
    ) -> Result<()> {
        if text.starts_with('#') {
            self.headers += 1;
            if self.headers > 64 || self.count != 0 {
                return Err(fail("unexpected decoder headers"));
            }
            if let Some(value) = text.strip_prefix("#format:") {
                if self.format || value.trim() != "frame checksums" {
                    return Err(fail("unexpected frame hash format"));
                }
                self.format = true;
            }
            if let Some(value) = text.strip_prefix("#version:") {
                if self.version || value.trim() != "2" {
                    return Err(fail("unsupported frame hash version"));
                }
                self.version = true;
            }
            if let Some(value) = text.strip_prefix("#hash:") {
                if self.hash || !value.trim().eq_ignore_ascii_case("sha256") {
                    return Err(fail("unexpected decoded hash algorithm"));
                }
                self.hash = true;
            }
            if let Some(value) = text.strip_prefix("#media_type 0:") {
                if self.video || value.trim() != "video" {
                    return Err(fail("decoded stream is not video"));
                }
                self.video = true;
            }
            if let Some(value) = text.strip_prefix("#codec_id 0:") {
                if self.raw || value.trim() != "rawvideo" {
                    return Err(fail("decoded packets are not raw video"));
                }
                self.raw = true;
            }
            if let Some(value) = text.strip_prefix("#dimensions 0:") {
                if self.size || value.trim() != format!("{}x{}", size.width, size.height) {
                    return Err(fail("decoded dimensions differ"));
                }
                self.size = true;
            }
            if let Some(value) = text.strip_prefix("#tb 0:") {
                let (a, b) = value
                    .trim()
                    .split_once('/')
                    .ok_or_else(|| fail("invalid decoded timebase"))?;
                let a = a
                    .parse::<i64>()
                    .map_err(|_| fail("invalid decoded timebase"))?;
                let b = b
                    .parse::<i64>()
                    .map_err(|_| fail("invalid decoded timebase"))?;
                if self.time_base.is_some()
                    || !(1..=1_000_000_000).contains(&a)
                    || !(1..=1_000_000_000).contains(&b)
                {
                    return Err(fail("invalid decoded timebase"));
                }
                self.time_base = Some((a, b));
            }
            return Ok(());
        }
        if !(self.format && self.version && self.hash && self.video && self.raw && self.size)
            || self.count == request.frames
        {
            return Err(fail("missing decoded headers or excess frames"));
        }
        let (num, den) = self
            .time_base
            .ok_or_else(|| fail("missing decoded timebase"))?;
        let fields: Vec<_> = text.split(',').map(str::trim).collect();
        if fields.len() < 6 || fields[0] != "0" {
            return Err(fail("invalid decoded frame row"));
        }
        let integer = |field: usize| {
            fields[field]
                .parse::<i64>()
                .ok()
                .filter(|v| v.unsigned_abs() <= 1_000_000_000_000_000)
                .ok_or_else(|| fail("invalid decoded timestamp or length"))
        };
        let _dts = integer(1)?;
        let pts = integer(2)?;
        let duration = integer(3)?;
        let bytes = integer(4)?;
        if duration < 0
            || bytes != i64::from(size.width) * i64::from(size.height) * 3 / 2
            || !hash_valid(fields[5])
            || self.last.is_some_and(|last| pts <= last)
        {
            return Err(fail(
                "invalid decoded NV12 length, digest or timestamp order",
            ));
        }
        let first = *self.first.get_or_insert(pts);
        let delta = i128::from(pts) - i128::from(first);
        let error = (delta * i128::from(num) * 10_000_000 * i128::from(request.fps)
            - i128::from(self.count) * 10_000_000 * i128::from(den))
        .abs();
        if error > 2 * i128::from(den) * i128::from(request.fps) {
            return Err(fail("decoded presentation cadence differs from export FPS"));
        }
        if duration > 0
            && (i128::from(duration) * i128::from(num) * 10_000_000 * i128::from(request.fps)
                - 10_000_000 * i128::from(den))
            .abs()
                > 2 * i128::from(den) * i128::from(request.fps)
        {
            return Err(fail("decoded frame duration differs from export FPS"));
        }
        self.digest.update(self.count.to_le_bytes());
        self.digest
            .update(hex::decode(fields[5]).map_err(|e| fail(e.to_string()))?);
        serde_json::to_writer(
            &mut *output,
            &json!({"frame":self.count,"pts":pts,"duration":duration,"timeBase":{"numerator":num,"denominator":den},
            "nv12Bytes":bytes,"nv12Sha256":fields[5]}),
        )?;
        output.write_all(b"\n").map_err(|e| fail(e.to_string()))?;
        self.count += 1;
        self.last = Some(pts);
        Ok(())
    }
}

/// Audit a finished local export only. No game/session operation occurs here.
/// The caller's existing encoder executable supplies the decoding backend.
pub fn run(
    request: Request<'_>,
    cancel: &AtomicBool,
    mut progress: impl FnMut(&'static str, u32),
) -> Result<VideoAudit> {
    if !(1..=60).contains(&request.fps)
        || request.frames == 0
        || request.frames > request.fps * 7200 + 1
        || !request.video.is_absolute()
    {
        return Err(fail("invalid video audit request"));
    }
    canceled(cancel)?;
    let mut video = open_finished(request.video, cancel)?;
    let before = video.metadata().at(request.video)?;
    if !before.is_file() || before.len() == 0 {
        return Err(fail("encoded video is empty or not a file"));
    }
    progress("checking_manifest", 0);
    let source = manifest(&request, cancel, &mut progress)?;
    let path = request.video.with_extension("decoded.jsonl");
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .at(&path)?;
    serde_json::to_writer(
        &mut output,
        &json!({"schema":1,"pixelFormat":"nv12","width":source.width,"height":source.height,
        "inputWidth":source.input_width,"inputHeight":source.input_height,"fps":request.fps,
        "framesExpected":request.frames,"captureId":request.capture_id,"inputRgbaSequenceSha256":&source.input_hash,"encoder":request.encoder}),
    )?;
    output.write_all(b"\n").at(&path)?;
    progress("checking_video", 0);
    let mut decoder = Decoder::start(&request, source.width, source.height)?;
    let mut frames = Frames::new(&request, &source);
    let decoded = (|| -> Result<()> {
        let mut eof = false;
        let mut last = Instant::now();
        loop {
            canceled(cancel)?;
            if last.elapsed() > Duration::from_secs(60) {
                return Err(fail("video decoder timed out"));
            }
            if eof {
                if let Some(status) = decoder.child.try_wait().map_err(|e| fail(e.to_string()))? {
                    decoder.terminal = true;
                    if !status.success() {
                        return Err(fail(format!("video decoder exited with {status}")));
                    }
                    break;
                }
                thread::sleep(Duration::from_millis(50));
                continue;
            }
            match decoder
                .receiver
                .as_ref()
                .ok_or_else(|| fail("decoder output closed"))?
                .recv_timeout(Duration::from_millis(100))
            {
                Ok(Event::Line(text)) => {
                    last = Instant::now();
                    frames.accept(&text, &request, &source, &mut output)?;
                    progress("checking_video", frames.count);
                }
                Ok(Event::End) => {
                    eof = true;
                    last = Instant::now();
                }
                Ok(Event::Error(error)) => return Err(fail(error)),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(_) => return Err(fail("decoder output ended without EOF")),
            }
        }
        if frames.count != request.frames {
            return Err(fail("decoded video frame count differs from export"));
        }
        Ok(())
    })();
    let diagnostics = decoder.stop();
    decoded.map_err(|error| {
        fail(format!(
            "{error}{}",
            if diagnostics.is_empty() {
                String::new()
            } else {
                format!("; decoder: {diagnostics}")
            }
        ))
    })?;
    progress("checking_file", request.frames);
    let mut hash = Sha256::new();
    let mut bytes = [0; 65536];
    let mut total = 0u64;
    loop {
        canceled(cancel)?;
        let n = video.read(&mut bytes).at(request.video)?;
        if n == 0 {
            break;
        }
        hash.update(&bytes[..n]);
        total += n as u64;
    }
    let after = video.metadata().at(request.video)?;
    if total != before.len()
        || after.len() != before.len()
        || after.modified().ok() != before.modified().ok()
    {
        return Err(fail("encoded video changed during audit"));
    }
    let (num, den) = frames
        .time_base
        .ok_or_else(|| fail("missing decoded timebase"))?;
    let receipt = VideoAudit {
        path: path.clone(),
        frames: frames.count,
        width: source.width,
        height: source.height,
        fps: request.fps,
        input_width: source.input_width,
        input_height: source.input_height,
        decoder_backend: request.encoder,
        encoded_file_sha256: hex::encode(hash.finalize()),
        input_rgba_sequence_sha256: source.input_hash,
        decoded_nv12_sequence_sha256: hex::encode(frames.digest.finalize()),
        first_presentation_timestamp: frames
            .first
            .ok_or_else(|| fail("missing decoded timestamp"))?,
        time_base_numerator: num,
        time_base_denominator: den,
        decoded_frame_count_verified: true,
        presentation_cadence_verified: true,
        deterministic_video_verified: false,
    };
    serde_json::to_writer(&mut output, &json!({"complete":&receipt}))?;
    output.write_all(b"\n").at(&path)?;
    output.sync_all().at(&path)?;
    Ok(receipt)
}
