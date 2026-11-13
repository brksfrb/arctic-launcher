//! Compare two finished exports using their saved decoded audits and locked
//! video/manifest files. No decoder, encoder, helper or game is started here.
//! Agreement of these artifacts is not proof of native-world or Java parity.
use super::{
    EncoderKind, Request, VideoAudit, canceled, fail, hash_valid, line, manifest, open_finished,
};
use crate::bedrock::export_identity::{ExportIntent, ExportReceipt, MAX_RECEIPT};
use crate::{Result, error::IoContext};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, Metadata},
    io::{BufReader, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameDifference {
    pub index: u32,
    pub fields: Vec<&'static str>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportComparison {
    pub schema: u32,
    pub first_video: PathBuf,
    pub second_video: PathBuf,
    pub first_capture_id: String,
    pub second_capture_id: String,
    pub first_audit: VideoAudit,
    pub second_audit: VideoAudit,
    pub first_intent_sha256: Option<String>,
    pub second_intent_sha256: Option<String>,
    pub recording_source_identical: Option<bool>,
    pub input_intent_identical: Option<bool>,
    pub intent_differences: Vec<&'static str>,
    pub frames: u32,
    pub fps: u32,
    pub input_rgba_identical: bool,
    pub decoded_nv12_identical: bool,
    pub presentation_timing_identical: bool,
    pub observed_timeline_identical: bool,
    pub encoding_request_identical: bool,
    pub encoded_files_identical: bool,
    pub first_difference: Option<FrameDifference>,
    pub saved_audits_validated: bool,
    // Export logs do not yet bind every source packet/resource/shader and
    // arbitrary caller-created receipts are not trusted game attestation.
    pub same_recording_verified: bool,
    pub native_world_verified: bool,
    pub deterministic_video_verified: bool,
}
impl ExportComparison {
    /// Observed frames/timing agree. Container bytes may legitimately differ;
    /// this does not promote the broader determinism or native-world flags.
    pub fn matches(&self) -> bool {
        self.input_rgba_identical
            && self.decoded_nv12_identical
            && self.presentation_timing_identical
            && self.observed_timeline_identical
            && self.encoding_request_identical
            && self.input_intent_identical != Some(false)
    }
}
struct Locked {
    path: PathBuf,
    file: File,
    before: Metadata,
}
impl Locked {
    fn open(path: PathBuf, cancel: &AtomicBool) -> Result<Self> {
        let path = canonical(&path)?;
        let file = open_finished(&path, cancel)?;
        let before = file.metadata().at(&path)?;
        if !before.is_file() || before.len() == 0 {
            return Err(fail("comparison input is empty or not a file"));
        }
        Ok(Self { path, file, before })
    }
    fn unchanged(&self) -> Result<()> {
        let handle = self.file.metadata().at(&self.path)?;
        let named = std::fs::metadata(&self.path).at(&self.path)?;
        if [handle, named].iter().any(|after| {
            after.len() != self.before.len() || after.modified().ok() != self.before.modified().ok()
        }) {
            return Err(fail(format!(
                "comparison input changed: {}",
                self.path.display()
            )));
        }
        Ok(())
    }
    fn rewind(&mut self) -> Result<()> {
        self.file.seek(SeekFrom::Start(0)).at(&self.path)?;
        Ok(())
    }
    fn digest(&mut self, cancel: &AtomicBool) -> Result<String> {
        self.rewind()?;
        let mut hash = Sha256::new();
        let mut buffer = [0u8; 65536];
        let mut total = 0u64;
        loop {
            canceled(cancel)?;
            let count = self.file.read(&mut buffer).at(&self.path)?;
            if count == 0 {
                break;
            }
            hash.update(&buffer[..count]);
            total += count as u64;
        }
        if total != self.before.len() {
            return Err(fail("comparison file changed length while hashing"));
        }
        self.unchanged()?;
        Ok(hex::encode(hash.finalize()))
    }
}
struct Export {
    video: Locked,
    decoded: Locked,
    frames: Locked,
    id: String,
    audit: VideoAudit,
    receipt_file: Option<Locked>,
    receipt: Option<ExportReceipt>,
}
fn integer(value: &Value, key: &str, min: u64, max: u64) -> Result<u64> {
    value[key]
        .as_u64()
        .filter(|v| (min..=max).contains(v))
        .ok_or_else(|| fail(format!("invalid comparison field {key}")))
}
fn signed(value: &Value, key: &str) -> Result<i64> {
    value[key]
        .as_i64()
        .filter(|v| v.unsigned_abs() <= 1_000_000_000_000_000)
        .ok_or_else(|| fail(format!("invalid comparison timestamp {key}")))
}
fn row(input: &mut impl std::io::BufRead, path: &Path, limit: usize) -> Result<Value> {
    let text = line(input, limit)
        .at(path)?
        .ok_or_else(|| fail(format!("incomplete comparison input: {}", path.display())))?;
    Ok(serde_json::from_str(&text)?)
}
fn canonical(path: &Path) -> Result<PathBuf> {
    path.canonicalize().at(path)
}
impl Export {
    fn open(
        video: &Path,
        cancel: &AtomicBool,
        progress: &mut impl FnMut(&'static str, u32),
    ) -> Result<Self> {
        let video = Locked::open(video.to_path_buf(), cancel)?;
        let mut decoded = Locked::open(video.path.with_extension("decoded.jsonl"), cancel)?;
        let frames = Locked::open(video.path.with_extension("frames.jsonl"), cancel)?;
        let mut input = BufReader::new(&mut decoded.file);
        let header = row(&mut input, &decoded.path, 16384)?;
        let fps = integer(&header, "fps", 1, 60)? as u32;
        let count = integer(&header, "framesExpected", 1, u64::from(fps) * 7200 + 1)? as u32;
        let width = integer(&header, "width", 2, 3840)? as u32;
        let height = integer(&header, "height", 2, 2160)? as u32;
        let input_width = integer(&header, "inputWidth", 2, 3840)? as u32;
        let input_height = integer(&header, "inputHeight", 2, 2160)? as u32;
        let id = header["captureId"]
            .as_str()
            .filter(|s| {
                !s.is_empty()
                    && s.len() <= 64
                    && s.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            })
            .ok_or_else(|| fail("invalid comparison capture identity"))?
            .to_owned();
        if header["schema"] != 1
            || header["pixelFormat"] != "nv12"
            || (width, height) != ((input_width + 1) & !1, (input_height + 1) & !1)
        {
            return Err(fail("unsupported decoded audit schema or geometry"));
        }
        let encoder: EncoderKind = serde_json::from_value(header["encoder"].clone())?;
        let mut digest = Sha256::new();
        digest.update(b"arctic-nv12-sequence-v1\0");
        for value in [width, height, fps, count] {
            digest.update(value.to_le_bytes());
        }
        let mut first = None;
        let mut last = None;
        let mut time_base = None;
        for index in 0..count {
            canceled(cancel)?;
            let value = row(&mut input, &decoded.path, 4096)?;
            if integer(&value, "frame", 0, u64::from(count))? != u64::from(index)
                || integer(&value, "nv12Bytes", 1, 3840 * 2160 * 3 / 2)?
                    != u64::from(width) * u64::from(height) * 3 / 2
            {
                return Err(fail(
                    "decoded comparison frame index or pixel length differs",
                ));
            }
            let pts = signed(&value, "pts")?;
            let duration = signed(&value, "duration")?;
            let num = integer(&value["timeBase"], "numerator", 1, 1_000_000_000)? as i64;
            let den = integer(&value["timeBase"], "denominator", 1, 1_000_000_000)? as i64;
            if duration < 0
                || last.is_some_and(|last| pts <= last)
                || time_base.is_some_and(|old| old != (num, den))
            {
                return Err(fail(
                    "decoded comparison timestamps run backwards or change timebase",
                ));
            }
            let start = *first.get_or_insert(pts);
            let delta = i128::from(pts) - i128::from(start);
            let error = (delta * i128::from(num) * 10_000_000 * i128::from(fps)
                - i128::from(index) * 10_000_000 * i128::from(den))
            .abs();
            let tolerance = 2 * i128::from(den) * i128::from(fps);
            if error > tolerance
                || duration > 0
                    && (i128::from(duration) * i128::from(num) * 10_000_000 * i128::from(fps)
                        - 10_000_000 * i128::from(den))
                    .abs()
                        > tolerance
            {
                return Err(fail(
                    "decoded comparison presentation cadence differs from export FPS",
                ));
            }
            let hash = value["nv12Sha256"]
                .as_str()
                .filter(|s| hash_valid(s))
                .ok_or_else(|| fail("invalid decoded comparison digest"))?;
            digest.update(index.to_le_bytes());
            digest.update(hex::decode(hash).map_err(|e| fail(e.to_string()))?);
            last = Some(pts);
            time_base = Some((num, den));
            progress("checking_saved_frames", index + 1);
        }
        let footer = row(&mut input, &decoded.path, 16384)?;
        let audit: VideoAudit = serde_json::from_value(footer["complete"].clone())?;
        if footer.as_object().is_none_or(|o| o.len() != 1)
            || line(&mut input, 4096).at(&decoded.path)?.is_some()
        {
            return Err(fail(
                "decoded comparison audit has extra rows or no complete footer",
            ));
        }
        if (
            audit.frames,
            audit.fps,
            audit.width,
            audit.height,
            audit.input_width,
            audit.input_height,
        ) != (count, fps, width, height, input_width, input_height)
            || audit.decoder_backend != encoder
            || !audit.decoded_frame_count_verified
            || !audit.presentation_cadence_verified
            || audit.deterministic_video_verified
            || Some(audit.first_presentation_timestamp) != first
            || Some((audit.time_base_numerator, audit.time_base_denominator)) != time_base
            || audit.decoded_nv12_sequence_sha256 != hex::encode(digest.finalize())
            || !hash_valid(&audit.encoded_file_sha256)
            || header["inputRgbaSequenceSha256"].as_str()
                != Some(audit.input_rgba_sequence_sha256.as_str())
            || !hash_valid(&audit.input_rgba_sequence_sha256)
            || canonical(&audit.path)? != decoded.path
        {
            return Err(fail(
                "decoded comparison footer differs from its rows or header",
            ));
        }
        drop(input);
        decoded.unchanged()?;
        // Existing per-artifact validation checks every RGBA row, encoder
        // color request, geometry and rational encoder timestamp again.
        let request = Request {
            executable: &video.path,
            encoder,
            video: &video.path,
            capture_id: &id,
            frames: count,
            fps,
        };
        let source = manifest(&request, cancel, progress)?;
        if source.input_hash != audit.input_rgba_sequence_sha256
            || (
                source.width,
                source.height,
                source.input_width,
                source.input_height,
            ) != (width, height, input_width, input_height)
        {
            return Err(fail("RGBA manifest differs from saved decoded audit"));
        }
        let mut result = Self {
            video,
            decoded,
            frames,
            id,
            audit,
            receipt_file: None,
            receipt: None,
        };
        result.check_video(cancel)?;
        result.load_receipt(cancel)?;
        result.unchanged()?;
        Ok(result)
    }
    fn check_video(&mut self, cancel: &AtomicBool) -> Result<()> {
        let mut digest = Sha256::new();
        let mut buffer = [0u8; 65536];
        let mut bytes = 0u64;
        loop {
            canceled(cancel)?;
            let count = self.video.file.read(&mut buffer).at(&self.video.path)?;
            if count == 0 {
                break;
            }
            digest.update(&buffer[..count]);
            bytes += count as u64;
        }
        if bytes != self.video.before.len()
            || hex::encode(digest.finalize()) != self.audit.encoded_file_sha256
        {
            return Err(fail("MP4 differs from its saved decoded audit"));
        }
        Ok(())
    }
    fn load_receipt(&mut self, cancel: &AtomicBool) -> Result<()> {
        let path = self.video.path.with_extension("export.json");
        match std::fs::symlink_metadata(&path) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(crate::Error::io(&path, error)),
        }
        let mut file = Locked::open(path, cancel)?;
        if file.before.len() > MAX_RECEIPT as u64 {
            return Err(fail("export identity receipt exceeds 2 MiB"));
        }
        let mut bytes = Vec::new();
        (&mut file.file)
            .take(MAX_RECEIPT as u64 + 1)
            .read_to_end(&mut bytes)
            .at(&file.path)?;
        let receipt = ExportReceipt::from_bytes(&bytes, &self.audit, &self.id)?;
        if self.frames.digest(cancel)? != receipt.frames_manifest_sha256
            || self.decoded.digest(cancel)? != receipt.decoded_audit_sha256
        {
            return Err(fail(
                "export receipt does not bind these exact manifest and decoded audit files",
            ));
        }
        file.unchanged()?;
        self.receipt_file = Some(file);
        self.receipt = Some(receipt);
        Ok(())
    }
    fn unchanged(&self) -> Result<()> {
        self.video.unchanged()?;
        self.decoded.unchanged()?;
        self.frames.unchanged()?;
        if let Some(file) = &self.receipt_file {
            file.unchanged()?;
        }
        Ok(())
    }
}
fn intent_frame(
    value: &Value,
    intent: &ExportIntent,
    index: u32,
    first_tick: &mut Option<u64>,
) -> Result<()> {
    intent.captured_tick_rate(value, index, first_tick)?;
    let packet = &value["requiredControls"]["packetDispatch"];
    let ms = integer(packet, "timeMs", 0, 86_400_000)?;
    let rem = integer(packet, "timeRemainder", 0, 999)?;
    let den = integer(packet, "timeDenominator", 1, 1000)?;
    let expected =
        u64::from(intent.replay_origin_ms) * u64::from(intent.fps) + u64::from(index) * 1000;
    if rem >= den
        || (ms * den + rem) * u64::from(intent.fps) != expected * den
        || value["requiredControls"]["cosmeticClockMs"].as_u64() != Some(ms)
        || value["world"]["cleanView"].as_bool() != Some(true)
    {
        return Err(fail(
            "captured frame time/view differs from the bound export intent",
        ));
    }
    let appearance = &value["requiredControls"]["appearance"];
    if match &intent.appearance_manifest_hash {
        Some(hash) => {
            appearance["manifestHash"].as_str() != Some(hash.as_str())
                || appearance["timeMs"].as_u64() != Some(ms)
        }
        None => !appearance.is_null(),
    } {
        return Err(fail(
            "captured appearance differs from the bound export intent",
        ));
    }
    if intent.settle_frames > 1
        && value["settled"]["frames"]
            .as_u64()
            .is_none_or(|n| n < u64::from(intent.settle_frames))
    {
        return Err(fail(
            "captured frame has no settling proof for the bound export intent",
        ));
    }
    Ok(())
}
// Compare only stable observed intent. Native sequence numbers, process/session
// identities and absolute tick origins differ between valid capture runs.
struct Timeline {
    first_tick: u64,
    last_tick: u64,
    last_partial: f32,
    replay_numerator: u64,
    replay_denominator: u64,
}
fn timeline(value: &Value, state: &mut Option<Timeline>, index: u32, fps: u32) -> Result<Value> {
    super::super::capture::validate_camera_time_frame(value)?;
    let world = &value["world"];
    if world["schema"] != 1 {
        return Err(fail("comparison frame has no native world tag"));
    }
    let tick = integer(world, "ticks", 0, 1_000_000_000_000)?;
    let partial = world["partial"]
        .as_f64()
        .filter(|v| v.is_finite() && (0. ..1.).contains(v))
        .ok_or_else(|| fail("invalid comparison partial tick"))? as f32;
    let camera = world["camera"]
        .as_array()
        .filter(|a| a.len() == 6)
        .ok_or_else(|| fail("comparison has no native camera pose"))?;
    let mut pose = [0u64; 6];
    for (index, value) in camera.iter().enumerate() {
        let number = value
            .as_f64()
            .filter(|n| n.is_finite() && n.abs() <= 32_000_000.)
            .ok_or_else(|| fail("invalid comparison camera component"))?;
        pose[index] = number.to_bits();
    }
    let packet = &value["requiredControls"]["packetDispatch"];
    let ms = integer(packet, "timeMs", 0, 86_400_000)?;
    let rem = integer(packet, "timeRemainder", 0, 999)?;
    let den = integer(packet, "timeDenominator", 1, 1000)?;
    if rem >= den || ms == 86_400_000 && rem != 0 {
        return Err(fail("invalid comparison replay time"));
    }
    let numerator = ms * den + rem;
    let state = state.get_or_insert(Timeline {
        first_tick: tick,
        last_tick: tick,
        last_partial: partial,
        replay_numerator: numerator,
        replay_denominator: den,
    });
    if tick < state.last_tick || tick == state.last_tick && partial < state.last_partial {
        return Err(fail("comparison native tick/partial time runs backwards"));
    }
    let elapsed = i128::from(numerator) * i128::from(state.replay_denominator)
        - i128::from(state.replay_numerator) * i128::from(den);
    if elapsed * i128::from(fps)
        != i128::from(index) * 1000 * i128::from(den) * i128::from(state.replay_denominator)
    {
        return Err(fail(
            "comparison replay clock differs from the indexed frame cadence",
        ));
    }
    let relative = tick - state.first_tick;
    state.last_tick = tick;
    state.last_partial = partial;
    if !(1. ..=179.).contains(&camera[5].as_f64().unwrap_or(f64::NAN)) {
        return Err(fail("invalid comparison camera FOV"));
    }
    let cosmetic = &value["requiredControls"]["cosmeticClockMs"];
    if !cosmetic.is_null() && cosmetic.as_u64() != Some(ms) {
        return Err(fail("comparison cosmetic clock differs from replay time"));
    }
    let clean = world["cleanView"]
        .as_bool()
        .ok_or_else(|| fail("comparison has no clean-view state"))?;
    let appearance = &value["requiredControls"]["appearance"];
    if !appearance.is_null()
        && (!appearance.is_object()
            || !appearance["manifestHash"].as_str().is_some_and(hash_valid)
            || appearance["timeMs"].as_u64() != Some(ms))
    {
        return Err(fail("invalid comparison appearance intent"));
    }
    Ok(
        json!({"tick":relative,"partialBits":partial.to_bits(),"cameraBits":pose,"cleanView":clean,
        "replayTime":[ms,rem,den],"cosmeticClockMs":value["requiredControls"]["cosmeticClockMs"],"appearance":appearance,
        "cameraTimeRequired":value["requiredControls"]["requireCameraTime"].as_bool().unwrap_or(false)}),
    )
}

/// Read-only comparison of two distinct finished local exports. The decoded
/// audit rows are revalidated, not regenerated. No process launch occurs.
pub fn compare(
    first: &Path,
    second: &Path,
    cancel: &AtomicBool,
    mut progress: impl FnMut(&'static str, u32),
) -> Result<ExportComparison> {
    if canonical(first)? == canonical(second)? {
        return Err(fail("comparison requires two different export files"));
    }
    let mut a = Export::open(first, cancel, &mut progress)?;
    let mut b = Export::open(second, cancel, &mut progress)?;
    if a.id == b.id {
        return Err(fail("comparison requires two distinct capture identities"));
    }
    if (
        a.audit.frames,
        a.audit.fps,
        a.audit.width,
        a.audit.height,
        a.audit.input_width,
        a.audit.input_height,
    ) != (
        b.audit.frames,
        b.audit.fps,
        b.audit.width,
        b.audit.height,
        b.audit.input_width,
        b.audit.input_height,
    ) {
        return Err(fail(
            "comparison exports need matching frame counts, FPS and dimensions",
        ));
    }
    let mut intent_differences = Vec::new();
    let (recording_source_identical, input_intent_identical) = match (&a.receipt, &b.receipt) {
        (Some(left), Some(right)) => {
            let x = &left.intent;
            let y = &right.intent;
            if x.source != y.source {
                intent_differences.push("recordingSource");
            }
            if x.camera_path_sha256 != y.camera_path_sha256 {
                intent_differences.push("cameraPath");
            }
            if (x.replay_origin_ms, x.frames, x.fps) != (y.replay_origin_ms, y.frames, y.fps) {
                intent_differences.push("frameRange");
            }
            if x.selected_range != y.selected_range {
                intent_differences.push("selectedRange");
            }
            if x.appearance_manifest_hash != y.appearance_manifest_hash {
                intent_differences.push("appearance");
            }
            if x.encoder != y.encoder {
                intent_differences.push("encoder");
            }
            if x.encoding_profile != y.encoding_profile {
                intent_differences.push("encodingProfile");
            }
            if x.settle_frames != y.settle_frames {
                intent_differences.push("settling");
            }
            if x.native_ticks_per_second != y.native_ticks_per_second {
                intent_differences.push("nativeTickRate");
            }
            if x.schema != y.schema {
                intent_differences.push("intentSchema");
            }
            (
                Some(x.source == y.source),
                Some(left.intent_sha256 == right.intent_sha256),
            )
        }
        _ => (None, None),
    };
    a.frames.rewind()?;
    b.frames.rewind()?;
    a.decoded.rewind()?;
    b.decoded.rewind()?;
    let mut af = BufReader::new(&mut a.frames.file);
    let mut bf = BufReader::new(&mut b.frames.file);
    let mut ad = BufReader::new(&mut a.decoded.file);
    let mut bd = BufReader::new(&mut b.decoded.file);
    row(&mut ad, &a.decoded.path, 16384)?;
    row(&mut bd, &b.decoded.path, 16384)?;
    let mut input_equal = true;
    let mut decoded_equal = true;
    let mut timing_equal = true;
    let mut timeline_equal = true;
    let mut encoding_equal = true;
    let mut difference = None;
    let mut atick = None;
    let mut btick = None;
    let mut a_native_origin = None;
    let mut b_native_origin = None;
    for index in 0..a.audit.frames {
        canceled(cancel)?;
        let fa = row(&mut af, &a.frames.path, 1024 * 1024)?;
        let fb = row(&mut bf, &b.frames.path, 1024 * 1024)?;
        if let Some(receipt) = &a.receipt {
            intent_frame(&fa, &receipt.intent, index, &mut a_native_origin)?;
        }
        if let Some(receipt) = &b.receipt {
            intent_frame(&fb, &receipt.intent, index, &mut b_native_origin)?;
        }
        let da = row(&mut ad, &a.decoded.path, 4096)?;
        let db = row(&mut bd, &b.decoded.path, 4096)?;
        let mut fields = Vec::new();
        if fa["rgbaSha256"] != fb["rgbaSha256"] {
            input_equal = false;
            fields.push("inputRgba");
        }
        if da["nv12Sha256"] != db["nv12Sha256"] {
            decoded_equal = false;
            fields.push("decodedNv12");
        }
        let relative = |value: &Value, audit: &VideoAudit| -> Result<(i128, i128, i128)> {
            Ok((
                (i128::from(signed(value, "pts")?)
                    - i128::from(audit.first_presentation_timestamp))
                    * i128::from(audit.time_base_numerator),
                i128::from(signed(value, "duration")?) * i128::from(audit.time_base_numerator),
                i128::from(audit.time_base_denominator),
            ))
        };
        let (ap, au, az) = relative(&da, &a.audit)?;
        let (bp, bu, bz) = relative(&db, &b.audit)?;
        if ap * bz != bp * az || au * bz != bu * az {
            timing_equal = false;
            fields.push("presentationTiming");
        }
        if timeline(&fa, &mut atick, index, a.audit.fps)?
            != timeline(&fb, &mut btick, index, b.audit.fps)?
        {
            timeline_equal = false;
            fields.push("observedTimeline");
        }
        if fa["encoding"] != fb["encoding"] || a.audit.decoder_backend != b.audit.decoder_backend {
            encoding_equal = false;
            fields.push("encodingRequest");
        }
        if difference.is_none() && !fields.is_empty() {
            difference = Some(FrameDifference { index, fields });
        }
        progress("comparing_exports", index + 1);
    }
    drop((af, bf, ad, bd));
    a.unchanged()?;
    b.unchanged()?;
    Ok(ExportComparison {
        schema: 1,
        first_video: a.video.path,
        second_video: b.video.path,
        first_capture_id: a.id,
        second_capture_id: b.id,
        first_intent_sha256: a.receipt.as_ref().map(|r| r.intent_sha256.clone()),
        second_intent_sha256: b.receipt.as_ref().map(|r| r.intent_sha256.clone()),
        recording_source_identical,
        input_intent_identical,
        intent_differences,
        frames: a.audit.frames,
        fps: a.audit.fps,
        input_rgba_identical: input_equal,
        decoded_nv12_identical: decoded_equal,
        presentation_timing_identical: timing_equal,
        observed_timeline_identical: timeline_equal,
        encoding_request_identical: encoding_equal,
        encoded_files_identical: a.audit.encoded_file_sha256 == b.audit.encoded_file_sha256,
        first_difference: difference,
        saved_audits_validated: true,
        same_recording_verified: false,
        native_world_verified: false,
        deterministic_video_verified: false,
        first_audit: a.audit,
        second_audit: b.audit,
    })
}
