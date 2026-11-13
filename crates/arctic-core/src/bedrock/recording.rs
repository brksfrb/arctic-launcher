//! Pair an existing unsigned packet take with native appearance observation.
//! Marker clock correlation is not packet-application, terrain or video proof.
use super::{Edition, offline::OfflineConnector};
use crate::{Error, Result, error::IoContext};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(windows)]
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    path::{Path, PathBuf},
};
#[cfg(windows)]
use std::{
    io::Write,
    time::{Duration, Instant},
};
const MAX_PACKET_FILE: u64 = 2 * 1024 * 1024 * 1024;
const MAX_LOOKS_FILE: u64 = 258 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PairLease {
    pub bundle: PathBuf,
    pub source: PathBuf,
    pub appearance: PathBuf,
    pub native_session: String,
    pub recording_id: String,
    pub owner: String,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairOutcome {
    pub source: PathBuf,
    pub appearance: PathBuf,
    pub association: PathBuf,
    pub recording_id: String,
    pub first_time_ms: u32,
    pub end_time_ms: u32,
    pub alignment_verified: bool,
    pub native_world_verified: bool,
    pub deterministic_video_verified: bool,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Association {
    format: u32,
    recording_id: String,
    source_file: String,
    source_sha256: String,
    appearance_file: String,
    appearance_sha256: String,
    appearance_manifest_hash: String,
    alignment: String,
}
pub struct LinkedAppearance {
    pub path: PathBuf,
    pub manifest_hash: String,
    pub recording_id: String,
}
fn sibling(path: &Path, suffix: &str) -> Result<PathBuf> {
    let mut name = path
        .file_name()
        .ok_or_else(|| Error::Other("recording has no filename".into()))?
        .to_os_string();
    name.push(suffix);
    Ok(path.with_file_name(name))
}
fn id_valid(value: &str) -> bool {
    uuid::Uuid::parse_str(value).is_ok_and(|id| id.to_string() == value)
}
fn file_name(path: &Path) -> Result<String> {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(str::to_owned)
        .ok_or_else(|| Error::Other("recording filename is not Unicode".into()))
}
fn digest_file(path: &Path, max: u64) -> Result<String> {
    let mut file = std::fs::File::open(path).at(path)?;
    let length = file.metadata().at(path)?.len();
    if length == 0 || length > max {
        return Err(Error::Other(
            "recording exceeds its file bound or is empty".into(),
        ));
    }
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    let mut total = 0u64;
    loop {
        let n = file.read(&mut buffer).at(path)?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > max {
            return Err(Error::Other("recording grew beyond its file limit".into()));
        }
        hash.update(&buffer[..n]);
    }
    if total != length {
        return Err(Error::Other("recording changed size while hashing".into()));
    }
    Ok(hex::encode(hash.finalize()))
}
fn packet_identity(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path).at(path)?;
    let mut header = [0u8; 13];
    file.read_exact(&mut header).at(path)?;
    if &header[..9] != b"ARCTICBR\x01" {
        return Err(Error::Other(
            "not an Arctic Bedrock packet recording".into(),
        ));
    }
    let length = u32::from_be_bytes(header[9..13].try_into().unwrap()) as usize;
    if length == 0 || length > 4096 {
        return Err(Error::Other(
            "invalid packet recording metadata length".into(),
        ));
    }
    let mut bytes = vec![0; length];
    file.read_exact(&mut bytes).at(path)?;
    let value: Value = serde_json::from_slice(&bytes)?;
    if value["format"].as_u64() != Some(1) || value["edition"].as_str() != Some("bedrock") {
        return Err(Error::Other(
            "invalid packet recording format/edition".into(),
        ));
    }
    value["recordingId"]
        .as_str()
        .filter(|id| id_valid(id))
        .map(str::to_owned)
        .ok_or_else(|| Error::Other("packet recording has no canonical recording ID".into()))
}
fn appearance_manifest(path: &Path) -> Result<(Value, String)> {
    let file = std::fs::File::open(path).at(path)?;
    if file.metadata().at(path)?.len() > MAX_LOOKS_FILE {
        return Err(Error::Other("appearance archive exceeds 258 MiB".into()));
    }
    let mut zip =
        zip::ZipArchive::new(file).map_err(|e| Error::Other(format!("appearance archive: {e}")))?;
    if zip.len() > 4097 {
        return Err(Error::Other(
            "appearance archive has excessive entries".into(),
        ));
    }
    let entry = zip
        .by_name("manifest.json")
        .map_err(|e| Error::Other(format!("appearance manifest: {e}")))?;
    if entry.size() == 0 || entry.size() > 1024 * 1024 {
        return Err(Error::Other(
            "appearance manifest exceeds 1 MiB or is empty".into(),
        ));
    }
    let mut bytes = Vec::new();
    entry
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .at(path)?;
    if bytes.len() > 1024 * 1024 {
        return Err(Error::Other("appearance manifest exceeds 1 MiB".into()));
    }
    Ok((
        serde_json::from_slice(&bytes)?,
        hex::encode(Sha256::digest(&bytes)),
    ))
}
fn companion(source: &Path) -> Result<PathBuf> {
    sibling(source, ".arctic-looks.zip")
}
fn link_file(source: &Path) -> Result<PathBuf> {
    sibling(source, ".appearance.json")
}

#[cfg(windows)]
struct Native {
    edition: Edition,
    bundle: PathBuf,
    session: String,
    pid: u32,
}
#[cfg(windows)]
impl Native {
    fn attach(edition: Edition, bundle: &Path, expected: Option<&str>) -> Result<Self> {
        let bundle = super::native::validate_bundle(bundle)?;
        let installed =
            super::find(edition).ok_or_else(|| Error::Other("Bedrock isn't installed".into()))?;
        let pid = super::package::process_id(&installed.path, edition.package_family())
            .ok_or_else(|| {
                Error::Other("paired recording needs the already-running native client".into())
            })?;
        if !std::fs::read_to_string(bundle.join(format!("arctic-bedrock-{pid}.ready")))
            .is_ok_and(|v| v.trim() == "native-preview")
        {
            return Err(Error::Other("this native bundle is not loaded".into()));
        }
        let status = report(&bundle.join(format!("arctic-bedrock-{pid}.appearance.json")))?;
        let prefix = format!("native_{pid}_");
        let session = status["session"]
            .as_str()
            .filter(|s| {
                s.strip_prefix(prefix.as_str())
                    .and_then(|e| e.parse::<u64>().ok())
                    .is_some_and(|n| n > 0)
            })
            .ok_or_else(|| Error::Other("native appearance world is unavailable".into()))?
            .to_owned();
        if expected.is_some_and(|s| s != session) || status["contextLive"].as_bool() != Some(true) {
            return Err(Error::Other(
                "paired recording lost its native world".into(),
            ));
        }
        Ok(Self {
            edition,
            bundle,
            session,
            pid,
        })
    }
    fn control(&self, op: super::appearance_control::AppearanceControl) -> Result<Value> {
        let status = report(
            &self
                .bundle
                .join(format!("arctic-bedrock-{}.appearance.json", self.pid)),
        )?;
        if status["session"].as_str() != Some(self.session.as_str())
            || !status["queued"].is_null()
            || !status["executing"].is_null()
        {
            return Err(Error::Other(
                "appearance consumer changed world or is busy".into(),
            ));
        }
        let sequence = status["sequence"]
            .as_u64()
            .and_then(|n| n.checked_add(1))
            .filter(|n| *n <= 1_000_000_000)
            .ok_or_else(|| Error::Other("appearance sequence exhausted".into()))?;
        super::appearance_control::control(self.edition, &self.bundle, &self.session, sequence, op)
    }
}
#[cfg(windows)]
fn report(path: &Path) -> Result<Value> {
    let file = std::fs::File::open(path).at(path)?;
    if !file
        .metadata()
        .at(path)?
        .modified()
        .at(path)?
        .elapsed()
        .is_ok_and(|age| age < Duration::from_secs(2))
    {
        return Err(Error::Other("native recording report is stale".into()));
    }
    let mut bytes = Vec::new();
    file.take(64 * 1024 + 1).read_to_end(&mut bytes).at(path)?;
    if bytes.len() > 64 * 1024 {
        return Err(Error::Other(
            "native recording report exceeds 64 KiB".into(),
        ));
    }
    Ok(serde_json::from_slice(&bytes)?)
}

/// Worker-only. Uses the existing helper, initialized viewer and loaded adapter.
/// Does not create a connection, launch Minecraft or authenticate an account.
/// `retained` is filled before native begin. On an ambiguous failure it remains
/// owned until finish/cancel acknowledges recovery; callers must preserve it.
#[cfg(windows)]
pub fn begin(
    session: &mut OfflineConnector,
    edition: Edition,
    bundle: &Path,
    retained: &mut Option<PairLease>,
) -> Result<()> {
    use super::{
        appearance_control::{AppearanceControl, RecordingAnchor},
        offline::ReplayStep,
        packet::{self, PacketArm},
    };
    if retained.is_some() {
        return Err(Error::Other(
            "a paired recording lease is already owned".into(),
        ));
    }
    let state = session
        .request_recording_control(&json!({"op":"recording_state"}), Duration::from_secs(10))?
        .state;
    if state["recordings"].as_u64() != Some(1)
        || state["initialized"].as_bool() != Some(true)
        || state["failed"].as_bool() != Some(false)
    {
        return Err(Error::Other(
            "paired recording needs one initialized live relay recording".into(),
        ));
    }
    let id = state["recordingId"]
        .as_str()
        .filter(|id| id_valid(id))
        .ok_or_else(|| Error::Other("relay recording ID is unavailable".into()))?
        .to_owned();
    let source = PathBuf::from(
        state["file"]
            .as_str()
            .ok_or_else(|| Error::Other("relay recording path is unavailable".into()))?,
    );
    if !source.is_absolute()
        || source
            .extension()
            .is_none_or(|e| !e.eq_ignore_ascii_case("abr"))
    {
        return Err(Error::Other(
            "paired recording requires an absolute .abr path".into(),
        ));
    }
    let source = source.canonicalize().at(&source)?;
    if packet_identity(&source)? != id {
        return Err(Error::Other(
            "relay file differs from its recording identity".into(),
        ));
    }
    let appearance = companion(&source)?;
    if appearance.exists() || link_file(&source)?.exists() {
        return Err(Error::Other(
            "appearance companion already exists; choose a new packet recording".into(),
        ));
    }
    let native = Native::attach(edition, bundle, None)?;
    let appearance_state = native.control(AppearanceControl::State)?;
    if appearance_state["state"]["loaded"].as_bool() != Some(false)
        || matches!(
            appearance_state["state"]["history"]["stage"].as_str(),
            Some("recording" | "finishing")
        )
    {
        return Err(Error::Other(
            "another appearance recording/playback owns the native client".into(),
        ));
    }
    let status = report(
        &native
            .bundle
            .join(format!("arctic-bedrock-{}.packet.json", native.pid)),
    )?;
    if status["installed"].as_bool() != Some(true)
        || status["contextLive"].as_bool() != Some(true)
        || status["session"].as_str() != Some(native.session.as_str())
        || !status["pending"].is_null()
    {
        return Err(Error::Other(
            "native packet marker binding is unavailable or busy".into(),
        ));
    }
    let sequence = status["sequence"]
        .as_u64()
        .and_then(|n| n.checked_add(1))
        .ok_or_else(|| Error::Other("packet marker sequence unavailable".into()))?;
    let replay_sequence = state["sequence"]
        .as_u64()
        .and_then(|n| n.checked_add(1))
        .ok_or_else(|| Error::Other("relay marker sequence unavailable".into()))?;
    let request = PacketArm::new(
        &native.session,
        sequence,
        &ReplayStep {
            generation: 1,
            sequence: replay_sequence,
            time_ms: 0,
        },
    )?;
    packet::arm(edition, &native.bundle, &request)?;
    let queued=session.request_recording_control(&json!({"op":"recording_marker","recordingId":id,"generation":1,"sequence":replay_sequence,"marker":request.marker}),Duration::from_secs(10))?.result;
    if queued["recordingId"].as_str() != Some(id.as_str())
        || queued["generation"].as_u64() != Some(1)
        || queued["marker"].as_str() != Some(request.marker.as_str())
        || queued["sequence"].as_u64() != Some(replay_sequence)
        || queued["markerQueued"].as_bool() != Some(true)
    {
        return Err(Error::Other(
            "relay did not acknowledge the paired marker".into(),
        ));
    }
    let time = queued["timeMs"]
        .as_u64()
        .filter(|n| *n <= 86_400_000)
        .ok_or_else(|| Error::Other("relay marker has no bounded time".into()))?
        as u32;
    let receipt = packet::wait(edition, &native.bundle, &request)?;
    let tick = receipt["clientTick"]
        .as_u64()
        .ok_or_else(|| Error::Other("native marker has no completed tick".into()))?;
    let started = Instant::now();
    loop {
        let state = native.control(AppearanceControl::State)?;
        if state["state"]["worldTick"]
            .as_u64()
            .is_some_and(|t| t >= tick)
        {
            break;
        }
        if started.elapsed() > Duration::from_secs(2) {
            return Err(Error::Other(
                "matched native world frames did not reach the recording anchor".into(),
            ));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let owner = format!("take-{}", uuid::Uuid::new_v4().simple());
    let anchor = RecordingAnchor {
        recording_id: id.clone(),
        sequence,
        request_id: request.request_id,
        marker: request.marker,
        generation: 1,
        replay_sequence,
    };
    // Retain recovery authority before a request whose acknowledgement can
    // time out after the native consumer has already committed the take.
    *retained = Some(PairLease {
        bundle: native.bundle.clone(),
        source,
        appearance: appearance.clone(),
        native_session: native.session.clone(),
        recording_id: id,
        owner: owner.clone(),
    });
    if let Err(error) = native.control(AppearanceControl::HistoryBegin {
        path: appearance.clone(),
        time_ms: time,
        owner: owner.clone(),
        anchor: Some(anchor),
    }) {
        let cleanup = native.control(AppearanceControl::HistoryCancel {
            owner: owner.clone(),
        });
        if cleanup.is_ok() {
            *retained = None;
        }
        return Err(Error::Other(format!(
            "appearance begin: {error}; owned cleanup: {}; owner {owner}",
            match cleanup {
                Ok(_) => "cancelled".into(),
                Err(e) => format!("not acknowledged ({e})"),
            }
        )));
    }
    Ok(())
}
#[cfg(not(windows))]
pub fn begin(
    _: &mut OfflineConnector,
    _: Edition,
    _: &Path,
    _: &mut Option<PairLease>,
) -> Result<()> {
    Err(Error::Other(
        "paired native recording requires Windows".into(),
    ))
}

#[cfg(windows)]
pub fn finish(
    session: &mut OfflineConnector,
    edition: Edition,
    lease: &PairLease,
) -> Result<PairOutcome> {
    use super::appearance_control::AppearanceControl;
    if !id_valid(&lease.recording_id)
        || companion(&lease.source)? != lease.appearance
        || packet_identity(&lease.source)? != lease.recording_id
    {
        return Err(Error::Other(
            "paired recording lease no longer identifies its source".into(),
        ));
    }
    let native = Native::attach(edition, &lease.bundle, Some(&lease.native_session))?;
    native.control(AppearanceControl::HistoryFinish {
        owner: lease.owner.clone(),
    })?;
    // Stop packet capture while the independent appearance writer flushes.
    // Its disk/encoding time must not extend the recorded world timeline.
    let result = session
        .request_recording_control(
            &json!({"op":"recording_finish","recordingId":lease.recording_id}),
            Duration::from_secs(30),
        )?
        .result;
    if result["recordingId"].as_str() != Some(lease.recording_id.as_str())
        || result["finalized"].as_bool() != Some(true)
    {
        return Err(Error::Other(
            "packet recording did not finalize its owned source".into(),
        ));
    }
    let started = Instant::now();
    loop {
        let reply = native.control(AppearanceControl::State)?;
        let history = &reply["state"]["history"];
        if history["owner"].as_str() != Some(lease.owner.as_str())
            || history["source"]["recordingId"].as_str() != Some(lease.recording_id.as_str())
        {
            return Err(Error::Other(
                "appearance history changed recording or owner".into(),
            ));
        }
        match history["stage"].as_str() {
            Some("complete") => break,
            Some("finishing") => {}
            _ => {
                return Err(Error::Other(format!(
                    "appearance writer failed: {}",
                    history["error"]
                )));
            }
        }
        if started.elapsed() > Duration::from_secs(60) {
            return Err(Error::Other("appearance writer is still finishing; retry with the same lease before stopping the world".into()));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    if std::fs::metadata(&lease.source).at(&lease.source)?.len()
        != result["bytes"].as_u64().unwrap_or_default()
    {
        return Err(Error::Other(
            "finalized packet recording changed size".into(),
        ));
    }
    let (manifest, manifest_hash) = appearance_manifest(&lease.appearance)?;
    if manifest["format"].as_u64() != Some(2)
        || manifest["source"]["recordingId"].as_str() != Some(lease.recording_id.as_str())
        || manifest["source"]["session"].as_str() != Some(lease.native_session.as_str())
        || manifest["source"]["alignment"].as_str() != Some("marker_post_tick")
    {
        return Err(Error::Other(
            "appearance file has a different recording association".into(),
        ));
    }
    let first = manifest["timeMs"]
        .as_u64()
        .filter(|n| *n <= 86_400_000)
        .ok_or_else(|| Error::Other("appearance history has no bounded origin".into()))?
        as u32;
    let end = manifest["durationMs"]
        .as_u64()
        .filter(|n| *n >= u64::from(first) && *n <= 86_400_000)
        .ok_or_else(|| Error::Other("appearance history has no bounded end".into()))?
        as u32;
    let source_hash = digest_file(&lease.source, MAX_PACKET_FILE)?;
    if result["sourceSha256"].as_str() != Some(source_hash.as_str()) {
        return Err(Error::Other(
            "packet path differs from the file finalized by the helper".into(),
        ));
    }
    let association = Association {
        format: 1,
        recording_id: lease.recording_id.clone(),
        source_file: file_name(&lease.source)?,
        source_sha256: source_hash,
        appearance_file: file_name(&lease.appearance)?,
        appearance_sha256: digest_file(&lease.appearance, MAX_LOOKS_FILE)?,
        appearance_manifest_hash: manifest_hash,
        alignment: "marker_post_tick".into(),
    };
    let path = link_file(&lease.source)?;
    let bytes = serde_json::to_vec_pretty(&association)?;
    if path.exists() {
        automatic_appearance(&json!({"recordingId":lease.recording_id,"recordingFile":lease.source,"sourceSha256":association.source_sha256}))?
            .filter(|p|p.path==lease.appearance).ok_or_else(||Error::Other("existing appearance association differs from this take".into()))?;
        return Ok(PairOutcome {
            source: lease.source.clone(),
            appearance: lease.appearance.clone(),
            association: path,
            recording_id: lease.recording_id.clone(),
            first_time_ms: first,
            end_time_ms: end,
            alignment_verified: false,
            native_world_verified: false,
            deterministic_video_verified: false,
        });
    }
    let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4().simple()));
    let mut created = false;
    let publish = (|| -> Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .at(&temporary)?;
        created = true;
        file.write_all(&bytes).at(&temporary)?;
        file.sync_all().at(&temporary)?;
        drop(file);
        std::fs::hard_link(&temporary, &path).at(&path)?;
        Ok(())
    })();
    if created {
        let _ = std::fs::remove_file(&temporary);
    }
    publish?;
    Ok(PairOutcome {
        source: lease.source.clone(),
        appearance: lease.appearance.clone(),
        association: path,
        recording_id: lease.recording_id.clone(),
        first_time_ms: first,
        end_time_ms: end,
        alignment_verified: false,
        native_world_verified: false,
        deterministic_video_verified: false,
    })
}
#[cfg(not(windows))]
pub fn finish(_: &mut OfflineConnector, _: Edition, _: &PairLease) -> Result<PairOutcome> {
    Err(Error::Other(
        "paired native recording requires Windows".into(),
    ))
}
#[cfg(windows)]
pub fn cancel(edition: Edition, lease: &PairLease) -> Result<()> {
    use super::appearance_control::AppearanceControl;
    let native = Native::attach(edition, &lease.bundle, None)?;
    let state = native.control(AppearanceControl::State)?;
    let observed = &state["state"];
    if observed["session"].as_str() != Some(native.session.as_str())
        || !observed.is_object()
        || observed.get("history").is_none()
    {
        return Err(Error::Other(
            "native cleanup returned no matching appearance state".into(),
        ));
    }
    // A failed begin may never have reached Java. A fresh serialized State
    // receipt proves our owner absent; never cancel another owner's take.
    let history = &observed["history"];
    if !history.is_null() && (!history.is_object() || history["owner"].as_str().is_none()) {
        return Err(Error::Other(
            "native cleanup returned malformed history ownership".into(),
        ));
    }
    if history["owner"].as_str() != Some(lease.owner.as_str()) {
        return Ok(());
    }
    // WorldState cancels the previous take on an epoch change. Confirm its
    // terminal Java status instead of requiring a dead epoch to become live.
    if matches!(history["stage"].as_str(), Some("complete" | "failed")) {
        return Ok(());
    }
    if native.session != lease.native_session
        || history["session"].as_str() != Some(lease.native_session.as_str())
    {
        return Err(Error::Other(
            "owned appearance history has not acknowledged its world ending".into(),
        ));
    }
    native.control(AppearanceControl::HistoryCancel {
        owner: lease.owner.clone(),
    })?;
    Ok(())
}
#[cfg(not(windows))]
pub fn cancel(_: Edition, _: &PairLease) -> Result<()> {
    Err(Error::Other(
        "paired native recording requires Windows".into(),
    ))
}

/// Discover only an adjacent, exact-content companion. A copied/mutated source,
/// changed archive, foreign recording ID or escaping filename is rejected.
pub fn has_companion(state: &Value) -> Result<bool> {
    if !state["recordingId"].as_str().is_some_and(id_valid) {
        return Ok(false);
    }
    let source = PathBuf::from(
        state["recordingFile"]
            .as_str()
            .ok_or_else(|| Error::Other("replay has no recording source path".into()))?,
    );
    if !source.is_absolute() {
        return Err(Error::Other("replay source path is not absolute".into()));
    }
    let source = source.canonicalize().at(&source)?;
    let link = link_file(&source)?;
    match std::fs::symlink_metadata(&link) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(Error::io(&link, e)),
    }
}
pub fn automatic_appearance(state: &Value) -> Result<Option<LinkedAppearance>> {
    let Some(id) = state["recordingId"].as_str().filter(|id| id_valid(id)) else {
        return Ok(None);
    };
    let source =
        PathBuf::from(state["recordingFile"].as_str().ok_or_else(|| {
            Error::Other("replay has no source path for its recording ID".into())
        })?);
    if !source.is_absolute() {
        return Err(Error::Other("replay source path is not absolute".into()));
    }
    let source = source.canonicalize().at(&source)?;
    let link = link_file(&source)?;
    match std::fs::symlink_metadata(&link) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(Error::io(&link, error)),
    }
    let mut bytes = Vec::new();
    std::fs::File::open(&link)
        .at(&link)?
        .take(16 * 1024 + 1)
        .read_to_end(&mut bytes)
        .at(&link)?;
    if bytes.len() > 16 * 1024 {
        return Err(Error::Other("appearance association exceeds 16 KiB".into()));
    }
    let data: Association = serde_json::from_slice(&bytes)?;
    if data.format != 1
        || data.recording_id != id
        || data.source_file != file_name(&source)?
        || data.alignment != "marker_post_tick"
        || state["sourceSha256"].as_str() != Some(data.source_sha256.as_str())
        || data.appearance_file.contains('/')
        || data.appearance_file.contains('\\')
        || data.appearance_file != file_name(&companion(&source)?)?
        || packet_identity(&source)? != id
        || digest_file(&source, MAX_PACKET_FILE)? != data.source_sha256
    {
        return Err(Error::Other(
            "appearance companion does not match the exact replay source".into(),
        ));
    }
    let appearance = source.with_file_name(&data.appearance_file);
    let appearance = appearance.canonicalize().at(&appearance)?;
    if appearance.parent() != source.parent() {
        return Err(Error::Other(
            "appearance companion escapes the source directory".into(),
        ));
    }
    if digest_file(&appearance, MAX_LOOKS_FILE)? != data.appearance_sha256 {
        return Err(Error::Other("appearance companion bytes changed".into()));
    }
    let (manifest, hash) = appearance_manifest(&appearance)?;
    if hash != data.appearance_manifest_hash
        || manifest["format"].as_u64() != Some(2)
        || manifest["source"]["recordingId"].as_str() != Some(id)
        || manifest["source"]["alignment"].as_str() != Some("marker_post_tick")
    {
        return Err(Error::Other(
            "appearance companion manifest has a foreign recording identity".into(),
        ));
    }
    Ok(Some(LinkedAppearance {
        path: appearance,
        manifest_hash: hash,
        recording_id: id.into(),
    }))
}
