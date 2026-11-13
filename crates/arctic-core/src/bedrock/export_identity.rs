//! Content-bound indexed-export intent. Receipts identify captured artifacts;
//! they do not certify native packet/terrain/shader or Java feature parity.
use super::{
    capture::EncoderKind,
    offline::CameraPath,
    video_audit::{VideoAudit, hash_valid, open_finished},
};
use crate::{Error, Result, error::IoContext};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs::{File, Metadata, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
pub const MAX_RECEIPT: usize = 2 * 1024 * 1024;
fn fail(message: impl Into<String>) -> Error {
    Error::Other(message.into())
}
fn canceled(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        Err(fail("export identity operation canceled"))
    } else {
        Ok(())
    }
}
fn id_valid(id: &str) -> bool {
    uuid::Uuid::parse_str(id).is_ok_and(|parsed| parsed.to_string() == id)
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecordingSource {
    pub sha256: String,
    pub bytes: u64,
    pub recording_id: Option<String>,
    pub version: String,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExportIntent {
    pub schema: u32,
    pub source: RecordingSource,
    pub camera_path: CameraPath,
    pub camera_path_sha256: String,
    pub replay_origin_ms: u32,
    pub frames: u32,
    pub fps: u32,
    /// Schema 1 did not bind simulation cadence. Never infer a default for it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_ticks_per_second: Option<u32>,
    pub settle_frames: u32,
    pub encoder: EncoderKind,
    /// Schema 3 binds a named indexed encoding request. Older intent has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoding_profile: Option<String>,
    /// Schema 4 binds an explicit inclusive selection when one was requested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_range: Option<super::export_range::ExportRange>,
    pub appearance_manifest_hash: Option<String>,
}
impl ExportIntent {
    pub fn validate(&self) -> Result<()> {
        self.camera_path.validate()?;
        let cadence_valid = matches!(
            (self.schema, self.native_ticks_per_second),
            (1, None) | (2..=4, Some(1..=1000))
        );
        let profile_valid = if self.schema >= 3 {
            self.encoding_profile.as_deref() == Some(self.encoder.indexed_profile())
        } else {
            self.encoding_profile.is_none()
        };
        let range_valid = match self.selected_range {
            Some(range) => {
                self.schema == 4 && range.matches(self.fps, self.frames, self.replay_origin_ms)?
            }
            None => true,
        };
        if !cadence_valid
            || !profile_valid
            || !range_valid
            || !hash_valid(&self.source.sha256)
            || !(22..=2 * 1024 * 1024 * 1024).contains(&self.source.bytes)
            || self
                .source
                .recording_id
                .as_deref()
                .is_some_and(|id| !id_valid(id))
            || self.source.version.len() > 32
            || !(3..=4).contains(&self.source.version.split('.').count())
            || self
                .source
                .version
                .split('.')
                .any(|part| part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()))
            || !(1..=60).contains(&self.fps)
            || self.frames == 0
            || self.frames > self.fps * 7200 + u32::from(self.schema == 4)
            || !(1..=8).contains(&self.settle_frames)
            || self.replay_origin_ms > 86_400_000
            || u64::from(self.replay_origin_ms) * u64::from(self.fps)
                + u64::from(self.frames - 1) * 1000
                > 86_400_000 * u64::from(self.fps)
            || self.camera_path.sha256()? != self.camera_path_sha256
            || self
                .appearance_manifest_hash
                .as_deref()
                .is_some_and(|hash| !hash_valid(hash))
        {
            return Err(fail("invalid indexed export intent"));
        }
        Ok(())
    }
    pub fn sha256(&self) -> Result<String> {
        self.validate()?;
        let bytes = serde_json::to_vec(self)?;
        if bytes.len() > MAX_RECEIPT {
            return Err(fail("export intent exceeds 2 MiB"));
        }
        let mut hash = Sha256::new();
        hash.update(match self.schema {
            1 => b"arctic-bedrock-export-intent-v1\0",
            2 => b"arctic-bedrock-export-intent-v2\0",
            3 => b"arctic-bedrock-export-intent-v3\0",
            _ => b"arctic-bedrock-export-intent-v4\0",
        });
        hash.update(bytes);
        Ok(hex::encode(hash.finalize()))
    }
    /// After intent validation, bind every new-schema frame to the same
    /// observed integral rate, indexed native tick/partial cadence and, for
    /// schemas 3 and 4, the explicit indexed encoding profile. Schema 4 also
    /// binds the optional range through validation and the hashed intent.
    /// Absolute tick origins stay capture-local rather than entering intent.
    pub(super) fn captured_tick_rate(
        &self,
        frame: &Value,
        index: u32,
        first_tick: &mut Option<u64>,
    ) -> Result<()> {
        super::capture::validate_camera_time_frame(frame)?;
        if self.schema >= 3
            && (frame["encoding"]["schema"].as_u64() != Some(2)
                || frame["encoding"]["profile"].as_str() != self.encoding_profile.as_deref())
        {
            return Err(fail(
                "captured encoding profile differs from the export intent",
            ));
        }
        let Some(rate) = self.native_ticks_per_second else {
            return Ok(());
        };
        let world = &frame["world"];
        if frame["requiredControls"]["ticksPerSecond"].as_f64() != Some(f64::from(rate))
            || world["controls"]["clock"]["ticksPerSecond"].as_f64() != Some(f64::from(rate))
        {
            return Err(fail(
                "captured native tick rate differs from the export intent",
            ));
        }
        let tick = world["ticks"]
            .as_u64()
            .filter(|tick| *tick <= 1_000_000_000_000)
            .ok_or_else(|| fail("captured native tick is unavailable"))?;
        let origin = match *first_tick {
            Some(origin) => origin,
            None if index == 0 => {
                *first_tick = Some(tick);
                tick
            }
            _ => return Err(fail("captured native cadence has no first frame")),
        };
        let numerator = u64::from(index) * u64::from(rate);
        let expected_tick = origin.checked_add(numerator / u64::from(self.fps));
        let expected_partial = (numerator % u64::from(self.fps)) as f32 / self.fps as f32;
        if expected_tick != Some(tick)
            || world["partial"]
                .as_f64()
                .filter(|partial| partial.is_finite() && (0.0..1.0).contains(partial))
                .is_none_or(|partial| (partial as f32).to_bits() != expected_partial.to_bits())
        {
            return Err(fail(
                "captured native tick/partial differs from the indexed export cadence",
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExportReceipt {
    pub schema: u32,
    pub capture_id: String,
    pub intent: ExportIntent,
    pub intent_sha256: String,
    pub encoded_file_sha256: String,
    pub input_rgba_sequence_sha256: String,
    pub decoded_nv12_sequence_sha256: String,
    pub frames_manifest_sha256: String,
    pub decoded_audit_sha256: String,
    pub source_unchanged: bool,
    pub helper_source_confirmed: bool,
    pub native_world_verified: bool,
    pub deterministic_video_verified: bool,
}
impl ExportReceipt {
    pub fn from_bytes(bytes: &[u8], audit: &VideoAudit, capture_id: &str) -> Result<Self> {
        if bytes.len() > MAX_RECEIPT {
            return Err(fail("export identity receipt exceeds 2 MiB"));
        }
        let receipt: Self = serde_json::from_slice(bytes)?;
        if receipt.schema != 1
            || receipt.capture_id != capture_id
            || !receipt.source_unchanged
            || !receipt.helper_source_confirmed
            || receipt.native_world_verified
            || receipt.deterministic_video_verified
            || receipt.intent.sha256()? != receipt.intent_sha256
            || receipt.intent.frames != audit.frames
            || receipt.intent.fps != audit.fps
            || receipt.intent.encoder != audit.decoder_backend
            || receipt.encoded_file_sha256 != audit.encoded_file_sha256
            || receipt.input_rgba_sequence_sha256 != audit.input_rgba_sequence_sha256
            || receipt.decoded_nv12_sequence_sha256 != audit.decoded_nv12_sequence_sha256
            || !hash_valid(&receipt.frames_manifest_sha256)
            || !hash_valid(&receipt.decoded_audit_sha256)
        {
            return Err(fail(
                "export identity receipt differs from its intent or decoded audit",
            ));
        }
        Ok(receipt)
    }
}
pub(super) struct ContentFile {
    path: PathBuf,
    file: File,
    before: Metadata,
}
impl ContentFile {
    pub(super) fn open(path: &Path, cancel: &AtomicBool) -> Result<Self> {
        let path = path.canonicalize().at(path)?;
        let file = open_finished(&path, cancel)?;
        let before = file.metadata().at(&path)?;
        if !before.is_file() || before.len() == 0 {
            return Err(fail("export identity source is empty or not a file"));
        }
        Ok(Self { path, file, before })
    }
    pub(super) fn hash(&mut self, max: u64, cancel: &AtomicBool) -> Result<String> {
        use std::io::{Seek, SeekFrom};
        self.file.seek(SeekFrom::Start(0)).at(&self.path)?;
        if self.before.len() > max {
            return Err(fail("export identity file exceeds its size bound"));
        }
        let mut digest = Sha256::new();
        let mut bytes = [0u8; 65536];
        let mut total = 0u64;
        loop {
            canceled(cancel)?;
            let count = self.file.read(&mut bytes).at(&self.path)?;
            if count == 0 {
                break;
            }
            total += count as u64;
            if total > max {
                return Err(fail("export identity file grew beyond its size bound"));
            }
            digest.update(&bytes[..count]);
        }
        if total != self.before.len() {
            return Err(fail("export identity file changed size"));
        }
        self.unchanged()?;
        Ok(hex::encode(digest.finalize()))
    }
    pub(super) fn unchanged(&self) -> Result<()> {
        let handle = self.file.metadata().at(&self.path)?;
        let named = std::fs::metadata(&self.path).at(&self.path)?;
        if [handle, named].iter().any(|after| {
            after.len() != self.before.len() || after.modified().ok() != self.before.modified().ok()
        }) {
            return Err(fail("export identity source changed during capture"));
        }
        Ok(())
    }
}
/// Keep the exact replay file locked through the last captured frame. No
/// process/session is started; the supplied helper reply is correlated by its
/// existing ReplayQueue owner before reaching this binding.
pub(super) struct SourceLease {
    file: ContentFile,
    pub source: RecordingSource,
}
impl SourceLease {
    pub(super) fn bind(helper: &Value, cancel: &AtomicBool) -> Result<Self> {
        let path = PathBuf::from(
            helper["recordingFile"]
                .as_str()
                .ok_or_else(|| fail("helper has no replay source path"))?,
        );
        if !path.is_absolute() {
            return Err(fail("helper replay source path is not absolute"));
        }
        let mut file = ContentFile::open(&path, cancel)?;
        let metadata = super::offline::replay_metadata_file(&mut file.file, &file.path)?;
        let recording_id = match metadata.get("recordingId") {
            None | Some(Value::Null) => None,
            Some(Value::String(id)) if id_valid(id) => Some(id.clone()),
            _ => return Err(fail("recording source has an invalid identity")),
        };
        let source = RecordingSource {
            sha256: file.hash(2 * 1024 * 1024 * 1024, cancel)?,
            bytes: file.before.len(),
            recording_id,
            version: metadata["version"]
                .as_str()
                .ok_or_else(|| fail("recording source has no protocol version"))?
                .into(),
        };
        let lease = Self { file, source };
        lease.confirm(helper)?;
        Ok(lease)
    }
    pub(super) fn confirm(&self, helper: &Value) -> Result<()> {
        let path = PathBuf::from(
            helper["recordingFile"]
                .as_str()
                .ok_or_else(|| fail("helper source path disappeared"))?,
        );
        if !path.is_absolute()
            || path.canonicalize().at(&path)? != self.file.path
            || helper["sourceSha256"].as_str() != Some(self.source.sha256.as_str())
            || match (&self.source.recording_id, helper.get("recordingId")) {
                (None, None | Some(Value::Null)) => false,
                (Some(expected), Some(Value::String(actual))) => expected != actual,
                _ => true,
            }
        {
            return Err(fail(
                "helper source descriptor differs from the locked replay source",
            ));
        }
        self.file.unchanged()
    }
    pub(super) fn unchanged(&self) -> Result<()> {
        self.file.unchanged()
    }
}

/// Check bounded lines from the locked manifest before binding new-schema
/// intent to it. This operates only on saved data, without an engine/decoder.
fn verify_captured_tick_rate(
    frames: &mut ContentFile,
    intent: &ExportIntent,
    cancel: &AtomicBool,
) -> Result<()> {
    use std::io::{BufRead, BufReader, Seek, SeekFrom};
    if intent.native_ticks_per_second.is_none() {
        return Ok(());
    }
    frames.file.seek(SeekFrom::Start(0)).at(&frames.path)?;
    let mut count = 0u32;
    let mut first_tick = None;
    {
        let mut reader = BufReader::new(&mut frames.file);
        loop {
            canceled(cancel)?;
            let mut bytes = Vec::new();
            let limit = 1024 * 1024;
            let size = reader
                .by_ref()
                .take((limit + 1) as u64)
                .read_until(b'\n', &mut bytes)
                .at(&frames.path)?;
            if size == 0 {
                break;
            }
            if size > limit || bytes.last() != Some(&b'\n') || count >= intent.frames {
                return Err(fail(
                    "export tick-rate manifest is oversized, incomplete or has extra frames",
                ));
            }
            let value: Value = serde_json::from_slice(&bytes)?;
            if value["index"].as_u64() != Some(u64::from(count)) {
                return Err(fail("export tick-rate manifest has an unordered frame"));
            }
            intent.captured_tick_rate(&value, count, &mut first_tick)?;
            count += 1;
        }
    }
    if count != intent.frames {
        return Err(fail("export tick-rate manifest is missing frames"));
    }
    frames.unchanged()
}

/// Save only after native capture, cleanup and decoded audit have completed.
/// A source lease must already have been confirmed after the last frame.
pub(super) fn write_receipt(
    video: &Path,
    capture_id: &str,
    intent: ExportIntent,
    audit: &VideoAudit,
    cancel: &AtomicBool,
) -> Result<PathBuf> {
    canceled(cancel)?;
    let intent_sha256 = intent.sha256()?;
    let mut encoded = ContentFile::open(video, cancel)?;
    let mut frames = ContentFile::open(&video.with_extension("frames.jsonl"), cancel)?;
    let mut decoded = ContentFile::open(&video.with_extension("decoded.jsonl"), cancel)?;
    verify_captured_tick_rate(&mut frames, &intent, cancel)?;
    if encoded.hash(u64::MAX, cancel)? != audit.encoded_file_sha256 {
        return Err(fail("MP4 changed before writing export identity"));
    }
    let receipt = ExportReceipt {
        schema: 1,
        capture_id: capture_id.into(),
        intent,
        intent_sha256,
        encoded_file_sha256: audit.encoded_file_sha256.clone(),
        input_rgba_sequence_sha256: audit.input_rgba_sequence_sha256.clone(),
        decoded_nv12_sequence_sha256: audit.decoded_nv12_sequence_sha256.clone(),
        frames_manifest_sha256: frames.hash(u64::from(audit.frames) * 1024 * 1024, cancel)?,
        decoded_audit_sha256: decoded.hash(u64::from(audit.frames) * 4096 + 32768, cancel)?,
        source_unchanged: true,
        helper_source_confirmed: true,
        native_world_verified: false,
        deterministic_video_verified: false,
    };
    let bytes = serde_json::to_vec(&receipt)?;
    if bytes.len() + 1 > MAX_RECEIPT {
        return Err(fail("export identity receipt exceeds 2 MiB"));
    }
    ExportReceipt::from_bytes(&bytes, audit, capture_id)?;
    canceled(cancel)?;
    let path = video.with_extension("export.json");
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .at(&path)?;
    output.write_all(&bytes).at(&path)?;
    output.write_all(b"\n").at(&path)?;
    output.sync_all().at(&path)?;
    encoded.unchanged()?;
    frames.unchanged()?;
    decoded.unchanged()?;
    Ok(path)
}
