//! Opt-in development native client. This currently ports shared client menus
//! and renderer/input integration; it does not provide Java feature parity.

use std::path::{Path, PathBuf};

use super::Edition;
use crate::error::IoContext;
use crate::{Error, Result};

/// Absolute native render-camera target. Orientation is the engine's XYZW
/// quaternion; callers must use an observed native basis rather than assuming
/// Java's camera axes. This changes the scoped view, not the player's pose.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeCameraTarget {
    pub position: [f32; 3],
    pub orientation: [f32; 4],
    /// Vertical FOV in degrees, with the same 1..=179 range as saved paths.
    pub field_of_view: f32,
}

impl NativeCameraTarget {
    #[cfg(windows)]
    fn validate(&self) -> Result<()> {
        let norm: f32 = self.orientation.iter().map(|v| v * v).sum();
        if self
            .position
            .iter()
            .any(|v| !v.is_finite() || v.abs() > 30_000_000.0)
            || self.orientation.iter().any(|v| !v.is_finite())
            || !(0.95..=1.05).contains(&norm)
            || !self.field_of_view.is_finite()
            || !(1.0..=179.0).contains(&self.field_of_view)
        {
            return Err(Error::Other(
                "invalid native camera position/quaternion/FOV".into(),
            ));
        }
        Ok(())
    }
}

/// An acknowledged native camera identity, separate from a requested path or
/// pose. Automatic export transitions compare it again at the native consumer.
#[cfg(windows)]
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CameraOwner {
    session: String,
    sequence: u64,
    request_id: Option<String>,
    active: bool,
}
#[cfg(windows)]
impl CameraOwner {
    fn request_id(value: &serde_json::Value) -> Result<Option<String>> {
        match value {
            serde_json::Value::Null => Ok(None),
            serde_json::Value::String(id)
                if !id.is_empty()
                    && id.len() <= 64
                    && id
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-') =>
            {
                Ok(Some(id.clone()))
            }
            _ => Err(Error::Other(
                "native camera has no valid acknowledged request identity".into(),
            )),
        }
    }
    fn from_receipt(session: &str, receipt: &serde_json::Value) -> Result<Self> {
        let invalid = || {
            Error::Other(
                "owned camera follow-up needs an acknowledged active camera identity".into(),
            )
        };
        let sequence = receipt["sequence"]
            .as_u64()
            .filter(|s| (1..=1_000_000_000).contains(s))
            .ok_or_else(invalid)?;
        let request_id = Self::request_id(&receipt["requestId"])?;
        if receipt["schema"].as_u64() != Some(1)
            || receipt["session"].as_str() != Some(session)
            || receipt["op"].as_str() != Some("set")
            || receipt["active"].as_bool() != Some(true)
            || receipt["restored"].as_bool() != Some(true)
            || receipt["playerUnchanged"].as_bool() != Some(true)
            || !receipt["error"].is_null()
            || request_id.is_none()
        {
            return Err(invalid());
        }
        Ok(Self {
            session: session.into(),
            sequence,
            request_id,
            active: true,
        })
    }
    fn from_idle_status(session: &str, status: &serde_json::Value) -> Result<Self> {
        let invalid = || {
            Error::Other(
                "native camera acquisition needs the same observed idle camera identity".into(),
            )
        };
        if status["idleAcquisitionGuardImplemented"].as_bool() != Some(true)
            || status["boundedLeaseImplemented"].as_bool() != Some(true)
        {
            return Err(Error::Other("native camera acquisition needs a bundle with camera ownership guards and bounded leases".into()));
        }
        let sequence = status["sequence"]
            .as_u64()
            .filter(|s| *s < 1_000_000_000)
            .ok_or_else(invalid)?;
        let request_id = Self::request_id(status.get("requestId").ok_or_else(invalid)?)?;
        let acknowledged = if sequence == 0 {
            request_id.is_none() && status["last"].is_null()
        } else {
            let last = &status["last"];
            last["schema"].as_u64() == Some(1)
                && last["session"].as_str() == Some(session)
                && last["sequence"].as_u64() == Some(sequence)
                && last.get("requestId") == Some(&serde_json::json!(request_id))
                && (last["op"].as_str() == Some("release")
                    || last["op"].as_str() == Some("lease_expired")
                        && last["expiryObservation"].as_bool() == Some(true)
                        && last["leaseExpired"].as_bool() == Some(true)
                        && status["leaseExpired"].as_bool() == Some(true)
                        && last["playerUnchanged"].as_bool() == Some(true))
                && last["active"].as_bool() == Some(false)
                && last["restored"].as_bool() == Some(true)
                && last["error"].is_null()
        };
        if status["session"].as_str() != Some(session)
            || !acknowledged
            || status["installed"].as_bool() != Some(true)
            || status["matrixInstalled"].as_bool() != Some(true)
            || status["controlEnabled"].as_bool() != Some(true)
            || status["bindingReady"].as_bool() != Some(true)
            || status["active"].as_bool() != Some(false)
            || !status["pending"].is_null()
            || !status["queued"].is_null()
            || status["consumerActive"].as_bool() != Some(false)
            || !status["leaseRemainingMs"].is_null()
            || !status["error"].is_null()
            || status["restorationFailed"].as_bool() != Some(false)
        {
            return Err(invalid());
        }
        Ok(Self {
            session: session.into(),
            sequence,
            request_id,
            active: false,
        })
    }
    fn already_expired(&self, status: &serde_json::Value) -> bool {
        status["last"]["op"].as_str() == Some("lease_expired")
            && status["last"]["playerUnchanged"].as_bool() == Some(true)
            && Self::from_idle_status(&self.session, status).is_ok_and(|idle| {
                idle.sequence == self.sequence && idle.request_id == self.request_id
            })
    }
}
#[cfg(windows)]
enum CameraGuard {
    Manual,
    Idle,
    Owned(CameraOwner),
}
#[cfg(windows)]
fn recent_camera_status(report: &Path) -> Result<serde_json::Value> {
    use std::{io::Read, time::Duration};
    let file = std::fs::File::open(report).at(report)?;
    if !file
        .metadata()
        .and_then(|m| m.modified())
        .is_ok_and(|at| at.elapsed().is_ok_and(|age| age < Duration::from_secs(2)))
    {
        return Err(Error::Other(
            "native camera control needs a fresh report".into(),
        ));
    }
    let mut data = Vec::new();
    file.take(16 * 1024 + 1).read_to_end(&mut data).at(report)?;
    if data.len() > 16 * 1024 {
        return Err(Error::Other("native camera report exceeds 16 KiB".into()));
    }
    Ok(serde_json::from_slice(&data)?)
}

/// Set or release the already-loaded adapter's scoped camera override. This
/// never launches Minecraft. Success requires a matching consumer receipt with
/// restored component bytes (and, for a set, an unchanged native player pose).
/// It is not a terrain-ready, presented-pixel or deterministic export fence.
#[cfg(windows)]
pub fn camera_control(
    edition: Edition,
    bundle: &Path,
    session: &str,
    sequence: u64,
    target: Option<NativeCameraTarget>,
) -> Result<serde_json::Value> {
    camera_command(
        edition,
        bundle,
        session,
        sequence,
        target,
        CameraGuard::Manual,
    )
}

/// Acquire only the freshly observed idle override with a 30-second wall-time
/// lease; never replace another active controller. This never launches the game.
#[cfg(windows)]
pub fn camera_control_from_idle(
    edition: Edition,
    bundle: &Path,
    session: &str,
    sequence: u64,
    target: NativeCameraTarget,
) -> Result<serde_json::Value> {
    camera_command(
        edition,
        bundle,
        session,
        sequence,
        Some(target),
        CameraGuard::Idle,
    )
}
/// Replace/release only this acknowledged identity. Replacements renew a
/// 30-second lease; cleanup may observe the same identity already expired.
#[cfg(windows)]
pub fn camera_control_owned(
    edition: Edition,
    bundle: &Path,
    session: &str,
    sequence: u64,
    target: Option<NativeCameraTarget>,
    receipt: &serde_json::Value,
) -> Result<serde_json::Value> {
    let owner = CameraOwner::from_receipt(session, receipt)?;
    camera_command(
        edition,
        bundle,
        session,
        sequence,
        target,
        CameraGuard::Owned(owner),
    )
}
#[cfg(windows)]
fn camera_command(
    edition: Edition,
    bundle: &Path,
    session: &str,
    sequence: u64,
    target: Option<NativeCameraTarget>,
    guard: CameraGuard,
) -> Result<serde_json::Value> {
    use std::io::{Read, Write};
    use std::time::{Duration, Instant};
    if session.is_empty()
        || session.len() > 64
        || !session
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        || !(1..=1_000_000_000).contains(&sequence)
    {
        return Err(Error::Other(
            "native camera needs a live session and increasing sequence".into(),
        ));
    }
    if let Some(target) = target {
        target.validate()?;
    }
    let bundle = validate_bundle(bundle)?;
    let installed =
        super::find(edition).ok_or_else(|| Error::Other("Bedrock isn't installed".into()))?;
    let pid =
        super::package::process_id(edition.package_family(), &installed.path).ok_or_else(|| {
            Error::Other("Bedrock must already be running with the native adapter loaded".into())
        })?;
    let marker = bundle.join(format!("arctic-bedrock-{pid}.ready"));
    if !std::fs::read_to_string(&marker).is_ok_and(|value| value.trim() == "native-preview") {
        return Err(Error::Other(
            "this native bundle is not loaded into the running game".into(),
        ));
    }
    let report = bundle.join(format!("arctic-bedrock-{pid}.camera.json"));
    let owner = match guard {
        CameraGuard::Manual => None,
        CameraGuard::Owned(owner) => Some(owner),
        CameraGuard::Idle => Some(CameraOwner::from_idle_status(
            session,
            &recent_camera_status(&report)?,
        )?),
    };
    if owner
        .as_ref()
        .is_some_and(|owner| sequence <= owner.sequence)
    {
        return Err(Error::Other(
            "owned camera transition must follow its acknowledged sequence".into(),
        ));
    }
    if target.is_none()
        && let Some(owner) = owner.as_ref()
        && let Ok(mut status) = recent_camera_status(&report)
        && owner.already_expired(&status)
    {
        // An observation, not a new release acknowledgement. Do not clear any
        // controller that acquired after this same-identity expired snapshot.
        status["releaseSkipped"] = serde_json::json!("owned_camera_lease_expired");
        return Ok(status);
    }
    let op = if target.is_some() { "set" } else { "release" };
    static REQUEST_SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let request_id = format!(
        "camera-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        REQUEST_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    let mut value = serde_json::json!({"schema":1,"session":session,
        "sequence":sequence,"requestId":request_id,"op":op,"target":target});
    let expected_camera = owner.as_ref().map(serde_json::to_value).transpose()?;
    if let Some(expected) = expected_camera.as_ref() {
        value["expectedCamera"] = expected.clone();
    }
    let leased = owner.is_some() && target.is_some();
    if leased {
        value["leaseMs"] = serde_json::json!(30_000);
    }
    let bytes = serde_json::to_vec(&value)?;
    let request = bundle.join(format!("arctic-bedrock-{pid}.camera-request.json"));
    let temporary = bundle.join(format!(
        ".camera-{}-{}.tmp",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let publish = (|| -> Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .at(&temporary)?;
        file.write_all(&bytes).at(&temporary)?;
        drop(file);
        std::fs::hard_link(&temporary, &request).at(&request)?;
        Ok(())
    })();
    let _ = std::fs::remove_file(&temporary);
    publish?;
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(5) {
        let mut data = Vec::new();
        if std::fs::File::open(&report)
            .and_then(|file| file.take(16 * 1024 + 1).read_to_end(&mut data))
            .is_ok()
            && data.len() <= 16 * 1024
            && let Ok(status) = serde_json::from_slice::<serde_json::Value>(&data)
        {
            for key in ["requestRejected", "last"] {
                let receipt = &status[key];
                if receipt["session"].as_str() != Some(session)
                    || receipt["sequence"].as_u64() != Some(sequence)
                    || receipt["requestId"].as_str() != Some(request_id.as_str())
                {
                    continue;
                }
                if let Some(error) = receipt["error"].as_str() {
                    return Err(Error::Other(error.into()));
                }
                if key == "last"
                    && receipt["op"].as_str() == Some(op)
                    && expected_camera
                        .as_ref()
                        .is_some_and(|expected| receipt.get("expectedCamera") != Some(expected))
                {
                    return Err(Error::Other(
                        "native camera acknowledgement did not retain its expected ownership guard"
                            .into(),
                    ));
                }
                if key == "last"
                    && receipt["op"].as_str() == Some(op)
                    && receipt["active"].as_bool() == Some(target.is_some())
                    && receipt["restored"].as_bool() == Some(true)
                    && (!leased
                        || receipt["leaseMs"].as_u64() == Some(30_000)
                            && receipt["leaseActive"].as_bool() == Some(true)
                            && receipt["leaseExpired"].as_bool() == Some(false))
                    && (target.is_none() || receipt["playerUnchanged"].as_bool() == Some(true))
                {
                    return Ok(receipt.clone());
                }
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    // Withdraw only our still-unconsumed mailbox entry. A worker that has
    // already admitted it expires its queue after two seconds independently.
    if std::fs::read(&request).is_ok_and(|data| data == bytes) {
        let _ = std::fs::remove_file(&request);
    }
    Err(Error::Other(format!(
        "native camera consumer did not acknowledge this request; inspect {}",
        report.display()
    )))
}

#[cfg(not(windows))]
pub fn camera_control(
    _: Edition,
    _: &Path,
    _: &str,
    _: u64,
    _: Option<NativeCameraTarget>,
) -> Result<serde_json::Value> {
    Err(Error::Other(
        "native camera control requires Windows".into(),
    ))
}
#[cfg(not(windows))]
pub fn camera_control_from_idle(
    _: Edition,
    _: &Path,
    _: &str,
    _: u64,
    _: NativeCameraTarget,
) -> Result<serde_json::Value> {
    Err(Error::Other(
        "native camera control requires Windows".into(),
    ))
}
#[cfg(not(windows))]
pub fn camera_control_owned(
    _: Edition,
    _: &Path,
    _: &str,
    _: u64,
    _: Option<NativeCameraTarget>,
    _: &serde_json::Value,
) -> Result<serde_json::Value> {
    Err(Error::Other(
        "native camera control requires Windows".into(),
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeClockOperation {
    Freeze,
    Step,
    Sample,
    Resume,
    Renew,
    Rate,
}

/// One native client/real-timer operation. Step waits for an actual primary
/// level tick, but never proves packet application, terrain readiness or pixels.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeClockControl {
    pub op: NativeClockOperation,
    #[serde(default)]
    pub partial: f32,
    #[serde(default)]
    pub real_timer: bool,
    #[serde(default = "clock_lease")]
    pub lease_ms: u32,
    /// Only the normal-playback Rate operation accepts this multiplier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate: Option<f32>,
}
fn clock_lease() -> u32 {
    5000
}

/// Exact observed idle or acknowledged active state for automatic clock
/// transitions. Compare it again inside the native timer consumer.
#[cfg(windows)]
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ClockOwner {
    session: String,
    sequence: u64,
    request_id: Option<String>,
    freeze_sequence: u64,
    frozen: bool,
    playback_rate: Option<f32>,
}
#[cfg(windows)]
impl ClockOwner {
    fn from_receipt(session: &str, receipt: &serde_json::Value) -> Result<Self> {
        let invalid = || {
            Error::Other(
                "owned clock transition needs an acknowledged active clock identity".into(),
            )
        };
        let sequence = receipt["sequence"]
            .as_u64()
            .filter(|value| (1..=1_000_000_000).contains(value))
            .ok_or_else(invalid)?;
        let request_id = receipt["requestId"]
            .as_str()
            .filter(|value| {
                !value.is_empty()
                    && value.len() <= 64
                    && value
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
            })
            .ok_or_else(invalid)?
            .to_owned();
        let frozen = receipt["frozen"].as_bool().ok_or_else(invalid)?;
        let freeze_sequence = receipt["freezeSequence"]
            .as_u64()
            .filter(|value| *value <= 1_000_000_000)
            .ok_or_else(invalid)?;
        let playback_rate = if receipt["playbackRate"].is_null() {
            None
        } else {
            Some(
                receipt["playbackRate"]
                    .as_f64()
                    .filter(|value| value.is_finite() && (0. ..=16.).contains(value))
                    .ok_or_else(invalid)? as f32,
            )
        };
        if receipt["schema"].as_u64() != Some(1)
            || receipt["session"].as_str() != Some(session)
            || !receipt["error"].is_null()
            || receipt["leaseActive"].as_bool() != Some(true)
            || frozen == playback_rate.is_some()
            || freeze_sequence > sequence
            || frozen && freeze_sequence == 0
            || playback_rate.is_some()
                && (receipt["rateApplied"].as_bool() != Some(true)
                    || receipt["rateRestorationFailed"].as_bool() != Some(false))
        {
            return Err(invalid());
        }
        Ok(Self {
            session: session.into(),
            sequence,
            request_id: Some(request_id),
            freeze_sequence,
            frozen,
            playback_rate,
        })
    }
    #[cfg(windows)]
    fn from_idle_status(session: &str, status: &serde_json::Value) -> Result<Self> {
        let invalid = || {
            Error::Other(
                "native clock acquisition needs the same observed idle clock identity".into(),
            )
        };
        if status["idleAcquisitionGuardImplemented"].as_bool() != Some(true) {
            return Err(Error::Other(
                "native clock acquisition needs a bundle with idle-state ownership guards".into(),
            ));
        }
        let sequence = status["sequence"]
            .as_u64()
            .filter(|sequence| *sequence < 1_000_000_000)
            .ok_or_else(invalid)?;
        let freeze_sequence = status["freezeSequence"]
            .as_u64()
            .filter(|generation| *generation <= sequence)
            .ok_or_else(invalid)?;
        let request_id = if sequence == 0 {
            None
        } else {
            match status["last"].get("requestId") {
                Some(serde_json::Value::Null) => None,
                Some(serde_json::Value::String(value))
                    if !value.is_empty()
                        && value.len() <= 64
                        && value
                            .bytes()
                            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-') =>
                {
                    Some(value.clone())
                }
                _ => return Err(invalid()),
            }
        };
        let owner = Self {
            session: session.into(),
            sequence,
            request_id,
            freeze_sequence,
            frozen: false,
            playback_rate: None,
        };
        if !owner.already_idle(status) {
            return Err(invalid());
        }
        Ok(owner)
    }
    #[cfg(windows)]
    fn already_idle(&self, status: &serde_json::Value) -> bool {
        let acknowledged = if self.sequence == 0 {
            self.request_id.is_none() && status["last"].is_null()
        } else {
            status["last"]["sequence"].as_u64() == Some(self.sequence)
                && match self.request_id.as_deref() {
                    Some(id) => status["last"]["requestId"].as_str() == Some(id),
                    None => status["last"].get("requestId") == Some(&serde_json::Value::Null),
                }
        };
        status["session"].as_str() == Some(self.session.as_str())
            && status["sequence"].as_u64() == Some(self.sequence)
            && acknowledged
            && status["leaseRemainingMs"].is_null()
            && status["freezeSequence"].as_u64() == Some(self.freeze_sequence)
            && status["bindingReady"].as_bool() == Some(true)
            && status["installed"].as_bool() == Some(true)
            && status["controlEnabled"].as_bool() == Some(true)
            && status["frozen"].as_bool() == Some(false)
            && status["playbackRate"].is_null()
            && status["queued"].is_null()
            && status["pendingWorldTick"].is_null()
            && status["waitingForRealTimer"].as_bool() == Some(false)
            && status["ratePendingSim"].as_bool() == Some(false)
            && status["ratePendingReal"].as_bool() == Some(false)
            && status["rateRestorationFailed"].as_bool() == Some(false)
    }
}

/// Control an already-loaded native timer. No game launch or account action.
/// The lease expires independently in the adapter if the caller goes away.
#[cfg(windows)]
pub fn clock_control(
    edition: Edition,
    bundle: &Path,
    session: &str,
    sequence: u64,
    control: NativeClockControl,
) -> Result<serde_json::Value> {
    clock_command(edition, bundle, session, sequence, control, None)
}

/// Release only a successful acknowledged held/rate state. Native lease
/// expiry can already have made that same identity idle; that returns the
/// actual status with an explicit skip label, not a synthetic command ack.
#[cfg(windows)]
pub fn resume_owned_clock(
    edition: Edition,
    bundle: &Path,
    session: &str,
    sequence: u64,
    receipt: &serde_json::Value,
) -> Result<serde_json::Value> {
    clock_control_owned(
        edition,
        bundle,
        session,
        sequence,
        NativeClockControl {
            op: NativeClockOperation::Resume,
            partial: 0.,
            real_timer: true,
            lease_ms: 5000,
            rate: None,
        },
        receipt,
    )
}

/// Apply a follow-up only while the exact acknowledged held/rate state is
/// still owned. Initial acquisition remains a separate operation.
#[cfg(windows)]
pub fn clock_control_owned(
    edition: Edition,
    bundle: &Path,
    session: &str,
    sequence: u64,
    control: NativeClockControl,
    receipt: &serde_json::Value,
) -> Result<serde_json::Value> {
    let owner = ClockOwner::from_receipt(session, receipt)?;
    if sequence <= owner.sequence || control.op == NativeClockOperation::Freeze {
        return Err(Error::Other(
            "owned clock follow-up must follow its acknowledgement and cannot acquire a new freeze"
                .into(),
        ));
    }
    clock_command(edition, bundle, session, sequence, control, Some(&owner))
}

/// Compare the initial idle observation inside the native consumer before
/// acquiring a freeze or rate. A report becoming stale cannot replace a clock
/// acquired by another controller in the meantime.
#[cfg(windows)]
pub fn clock_control_from_idle(
    edition: Edition,
    bundle: &Path,
    session: &str,
    sequence: u64,
    control: NativeClockControl,
    status: &serde_json::Value,
) -> Result<serde_json::Value> {
    let owner = ClockOwner::from_idle_status(session, status)?;
    if sequence <= owner.sequence
        || !matches!(
            control.op,
            NativeClockOperation::Freeze | NativeClockOperation::Rate
        )
    {
        return Err(Error::Other(
            "native clock acquisition needs an increasing freeze/rate command".into(),
        ));
    }
    clock_command(edition, bundle, session, sequence, control, Some(&owner))
}

/// Leased normal-playback simulation rate, including zero for pause. This
/// waits for scoped multiplier application/restoration on the selected native
/// timers, not packet, terrain or presented-frame completion. Resume releases
/// the rate through clock_control; renew it with another owned rate command.
#[cfg(windows)]
pub fn playback_clock_control(
    edition: Edition,
    bundle: &Path,
    session: &str,
    sequence: u64,
    rate: f32,
    real_timer: bool,
    lease_ms: u32,
) -> Result<serde_json::Value> {
    clock_command(
        edition,
        bundle,
        session,
        sequence,
        NativeClockControl {
            op: NativeClockOperation::Rate,
            partial: 0.0,
            real_timer,
            lease_ms,
            rate: Some(rate),
        },
        None,
    )
}

#[cfg(windows)]
fn clock_command(
    edition: Edition,
    bundle: &Path,
    session: &str,
    sequence: u64,
    control: NativeClockControl,
    owner: Option<&ClockOwner>,
) -> Result<serde_json::Value> {
    use std::io::{Read, Write};
    use std::time::{Duration, Instant};
    let rate = control.rate;
    if session.is_empty()
        || session.len() > 64
        || !session
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        || !(1..=1_000_000_000).contains(&sequence)
        || !control.partial.is_finite()
        || !(0.0..1.0).contains(&control.partial)
        || !(500..=30_000).contains(&control.lease_ms)
        || (control.op == NativeClockOperation::Rate) != rate.is_some()
        || control.op == NativeClockOperation::Rate && control.partial != 0.0
        || rate.is_some_and(|rate| !rate.is_finite() || !(0.0..=16.0).contains(&rate))
    {
        return Err(Error::Other("native clock needs a live session, increasing sequence, partial [0,1) and lease 500–30000 ms".into()));
    }
    let bundle = validate_bundle(bundle)?;
    let installed =
        super::find(edition).ok_or_else(|| Error::Other("Bedrock isn't installed".into()))?;
    let pid =
        super::package::process_id(edition.package_family(), &installed.path).ok_or_else(|| {
            Error::Other("Bedrock must already be running with the native adapter loaded".into())
        })?;
    let marker = bundle.join(format!("arctic-bedrock-{pid}.ready"));
    if !std::fs::read_to_string(&marker).is_ok_and(|v| v.trim() == "native-preview") {
        return Err(Error::Other(
            "this native bundle is not loaded into the running game".into(),
        ));
    }
    let report = bundle.join(format!("arctic-bedrock-{pid}.clock.json"));
    if let Some(owner) = owner.filter(|_| control.op == NativeClockOperation::Resume)
        && let Ok(file) = std::fs::File::open(&report)
        && file
            .metadata()
            .and_then(|m| m.modified())
            .is_ok_and(|at| at.elapsed().is_ok_and(|age| age < Duration::from_secs(2)))
    {
        let mut bytes = Vec::new();
        file.take(16 * 1024 + 1)
            .read_to_end(&mut bytes)
            .at(&report)?;
        if bytes.len() <= 16 * 1024
            && let Ok(mut status) = serde_json::from_slice::<serde_json::Value>(&bytes)
            && owner.already_idle(&status)
        {
            status["releaseSkipped"] = serde_json::json!("owned_clock_already_idle");
            return Ok(status);
        }
    }
    static REQUEST_SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let request_id = format!(
        "clock-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        REQUEST_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    let mut value = serde_json::to_value(control)?;
    value["schema"] = serde_json::json!(1);
    value["session"] = serde_json::json!(session);
    value["sequence"] = serde_json::json!(sequence);
    value["requestId"] = serde_json::json!(request_id);
    let expected_clock = owner.map(serde_json::to_value).transpose()?;
    if let Some(expected) = expected_clock.as_ref() {
        value["expectedClock"] = expected.clone();
    }
    let bytes = serde_json::to_vec(&value)?;
    let request = bundle.join(format!("arctic-bedrock-{pid}.clock-request.json"));
    let temporary = bundle.join(format!(".{request_id}.tmp"));
    let publish = (|| -> Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .at(&temporary)?;
        file.write_all(&bytes).at(&temporary)?;
        drop(file);
        std::fs::hard_link(&temporary, &request).at(&request)?;
        Ok(())
    })();
    let _ = std::fs::remove_file(&temporary);
    publish?;
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(5) {
        let mut bytes = Vec::new();
        if std::fs::File::open(&report)
            .and_then(|file| file.take(16 * 1024 + 1).read_to_end(&mut bytes))
            .is_ok()
            && bytes.len() <= 16 * 1024
            && let Ok(status) = serde_json::from_slice::<serde_json::Value>(&bytes)
        {
            for key in ["requestRejected", "last"] {
                let receipt = &status[key];
                if receipt["session"].as_str() != Some(session)
                    || receipt["sequence"].as_u64() != Some(sequence)
                    || receipt["requestId"].as_str() != Some(request_id.as_str())
                {
                    continue;
                }
                if let Some(error) = receipt["error"].as_str() {
                    return Err(Error::Other(error.into()));
                }
                if key == "last"
                    && receipt["op"] == value["op"]
                    && expected_clock
                        .as_ref()
                        .is_some_and(|expected| receipt.get("expectedClock") != Some(expected))
                {
                    return Err(Error::Other(
                        "native clock acknowledgement did not retain its expected ownership guard"
                            .into(),
                    ));
                }
                let scaling = control.op == NativeClockOperation::Rate;
                let frozen = !matches!(
                    control.op,
                    NativeClockOperation::Resume | NativeClockOperation::Rate
                );
                let fraction_matches = |sample: &serde_json::Value| {
                    sample["alpha"]
                        .as_f64()
                        .is_some_and(|v| (v as f32).to_bits() == control.partial.to_bits())
                };
                if key == "last"
                    && receipt["op"] == value["op"]
                    && receipt["frozen"].as_bool() == Some(frozen)
                    && (!(frozen || scaling) || receipt["leaseActive"].as_bool() == Some(true))
                    && (!scaling
                        || receipt["rateApplied"].as_bool() == Some(true)
                            && receipt["rateRestorationFailed"].as_bool() == Some(false)
                            && receipt["playbackRate"].as_f64().zip(rate).is_some_and(
                                |(actual, wanted)| (actual as f32).to_bits() == wanted.to_bits(),
                            ))
                    && (!frozen || fraction_matches(&receipt["timer"]))
                    && (!(frozen || scaling)
                        || !control.real_timer
                        || receipt["realTimerControlled"].as_bool() == Some(true))
                    && (!frozen || !control.real_timer || fraction_matches(&receipt["realTimer"]))
                    && (control.op != NativeClockOperation::Step
                        || receipt["worldTickComplete"].as_bool() == Some(true)
                            && receipt["completedTick"].as_u64().is_some())
                {
                    return Ok(receipt.clone());
                }
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    if std::fs::read(&request).is_ok_and(|data| data == bytes) {
        let _ = std::fs::remove_file(&request);
    }
    Err(Error::Other(format!(
        "native timer/world tick did not acknowledge this command; inspect {}",
        report.display()
    )))
}

#[cfg(not(windows))]
pub fn clock_control(
    _: Edition,
    _: &Path,
    _: &str,
    _: u64,
    _: NativeClockControl,
) -> Result<serde_json::Value> {
    Err(Error::Other("native clock control requires Windows".into()))
}
#[cfg(not(windows))]
pub fn resume_owned_clock(
    _: Edition,
    _: &Path,
    _: &str,
    _: u64,
    _: &serde_json::Value,
) -> Result<serde_json::Value> {
    Err(Error::Other(
        "owned native clock release requires Windows".into(),
    ))
}
#[cfg(not(windows))]
pub fn clock_control_owned(
    _: Edition,
    _: &Path,
    _: &str,
    _: u64,
    _: NativeClockControl,
    _: &serde_json::Value,
) -> Result<serde_json::Value> {
    Err(Error::Other(
        "owned native clock control requires Windows".into(),
    ))
}
#[cfg(not(windows))]
pub fn clock_control_from_idle(
    _: Edition,
    _: &Path,
    _: &str,
    _: u64,
    _: NativeClockControl,
    _: &serde_json::Value,
) -> Result<serde_json::Value> {
    Err(Error::Other(
        "guarded native clock acquisition requires Windows".into(),
    ))
}

#[cfg(not(windows))]
pub fn playback_clock_control(
    _: Edition,
    _: &Path,
    _: &str,
    _: u64,
    _: f32,
    _: bool,
    _: u32,
) -> Result<serde_json::Value> {
    Err(Error::Other(
        "native playback clock control requires Windows".into(),
    ))
}

/// Validate a built bundle before starting or modifying a game process.
pub fn validate_bundle(bundle: &Path) -> Result<PathBuf> {
    let bundle = bundle.canonicalize().at(bundle)?;
    for name in [
        "arctic-bedrock.json",
        "arctic-bedrock.jar",
        "arctic_bedrock.dll",
        "arctic-bedrock-loader.exe",
    ] {
        let path = bundle.join(name);
        if !path.is_file() {
            return Err(Error::Other(format!(
                "native client bundle is missing {}",
                path.display()
            )));
        }
    }
    let path = bundle.join("arctic-bedrock.json");
    let config: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).at(&path)?)?;
    let object = config
        .as_object()
        .ok_or_else(|| Error::Other("native client config must be an object".into()))?;
    for key in object.keys() {
        if ![
            "java_home",
            "java_reference_jar",
            "config_dir",
            "game_version",
            "renderer",
            "offline_name",
        ]
        .contains(&key.as_str())
        {
            return Err(Error::Other(format!("unknown native client option: {key}")));
        }
    }
    if config["game_version"]
        .as_str()
        .is_none_or(|s| s.trim().is_empty())
    {
        return Err(Error::Other(
            "native client bundle needs a game_version".into(),
        ));
    }
    if object
        .get("renderer")
        .is_some_and(|r| !matches!(r.as_str(), Some("dx11" | "dx12")))
    {
        return Err(Error::Other("native renderer must be dx11 or dx12".into()));
    }
    if let Some(name) = object.get("offline_name") {
        let valid = name.as_str().is_some_and(|n| {
            (3..=16).contains(&n.len()) && n.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
        });
        if !valid {
            return Err(Error::Other(
                "offline_name needs 3–16 letters, digits or underscores".into(),
            ));
        }
    }
    let java = config["java_home"]
        .as_str()
        .map(Path::new)
        .ok_or_else(|| Error::Other("native client bundle has no Java home".into()))?;
    if let Some(reference) = object
        .get("java_reference_jar")
        .filter(|value| !value.is_null())
    {
        let valid = reference.as_str().is_some_and(|value| {
            let path = Path::new(value);
            path.is_absolute() && path.is_file()
        });
        if !valid {
            return Err(Error::Other(
                "java_reference_jar must be an absolute path to a local file".into(),
            ));
        }
    }
    let client_config = config["config_dir"]
        .as_str()
        .map(Path::new)
        .ok_or_else(|| Error::Other("native client bundle has no config directory".into()))?;
    if !java.is_absolute()
        || !client_config.is_absolute()
        || !java.join("bin/server/jvm.dll").is_file()
    {
        return Err(Error::Other("native client needs an absolute Java home with bin/server/jvm.dll and an absolute config directory".into()));
    }
    Ok(bundle)
}

/// Start Bedrock and attach only to the selected edition's process. Success
/// requires a first-render handshake from the native DLL.
pub fn launch(edition: Edition, bundle: &Path) -> Result<()> {
    if !super::supported() || !cfg!(target_arch = "x86_64") {
        return Err(Error::Other(
            "the native Bedrock client requires Windows x64".into(),
        ));
    }
    let bundle = validate_bundle(bundle)?;
    #[cfg(windows)]
    {
        use std::time::{Duration, Instant};
        let installed = super::find(edition)
            .ok_or_else(|| Error::Other(format!("{} isn't installed", edition.name())))?;
        super::launch(edition)?;
        let started = Instant::now();
        let pid = loop {
            if let Some(pid) = super::package::process_id(edition.package_family(), &installed.path)
            {
                break pid;
            }
            if started.elapsed() > Duration::from_secs(30) {
                return Err(Error::Other(
                    "Bedrock didn't expose its process within 30 seconds".into(),
                ));
            }
            std::thread::sleep(Duration::from_millis(250));
        };
        let ready = bundle.join(format!("arctic-bedrock-{pid}.ready"));
        // A PID can be reused after a previous run. Only accept a new marker.
        if ready.exists() {
            std::fs::remove_file(&ready).at(&ready)?;
        }
        let output = std::process::Command::new(bundle.join("arctic-bedrock-loader.exe"))
            .arg(pid.to_string())
            .arg(&bundle)
            .output()
            .at(&bundle)?;
        if !output.status.success() {
            return Err(Error::Other(format!(
                "native client loader failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        let started = Instant::now();
        loop {
            if std::fs::read_to_string(&ready).is_ok_and(|s| s.trim() == "native-preview") {
                return Ok(());
            }
            if started.elapsed() > Duration::from_secs(30) {
                return Err(Error::Other(format!(
                    "native client didn't confirm rendering; see {}",
                    bundle.join(format!("arctic-bedrock-{pid}.log")).display()
                )));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (edition, bundle);
        unreachable!()
    }
}

/// Record real presented frames from an already-loaded native adapter. This
/// does not start a game, select an account or synthesize a replay world.
#[cfg(windows)]
pub fn record_video(
    edition: Edition,
    bundle: &Path,
    ffmpeg: &Path,
    output: &Path,
    seconds: u32,
    fps: u32,
) -> Result<PathBuf> {
    record_video_with_encoder(
        edition,
        bundle,
        ffmpeg,
        super::capture::EncoderKind::Ffmpeg,
        output,
        seconds,
        fps,
    )
}
#[cfg(windows)]
pub fn record_video_with_encoder(
    edition: Edition,
    bundle: &Path,
    ffmpeg: &Path,
    encoder: super::capture::EncoderKind,
    output: &Path,
    seconds: u32,
    fps: u32,
) -> Result<PathBuf> {
    use std::io::Write;
    use std::time::{Duration, Instant};
    if !(1..=7200).contains(&seconds) || !(1..=60).contains(&fps) {
        return Err(Error::Other(
            "video duration must be 1–7200 seconds and FPS 1–60".into(),
        ));
    }
    let bundle = validate_bundle(bundle)?;
    let ffmpeg = ffmpeg.canonicalize().at(ffmpeg)?;
    let output = std::path::absolute(output).at(output)?;
    if !ffmpeg.is_file()
        || output.exists()
        || output
            .extension()
            .is_none_or(|extension| !extension.eq_ignore_ascii_case("mp4"))
        || !output.parent().is_some_and(Path::is_dir)
    {
        return Err(Error::Other(
            "choose an encoder executable and a new .mp4 in an existing directory".into(),
        ));
    }
    let pid = {
        let installed =
            super::find(edition).ok_or_else(|| Error::Other("Bedrock isn't installed".into()))?;
        super::package::process_id(edition.package_family(), &installed.path).ok_or_else(|| {
            Error::Other("Bedrock must already be running with the native adapter loaded".into())
        })?
    };
    let marker = bundle.join(format!("arctic-bedrock-{pid}.ready"));
    if !std::fs::read_to_string(&marker).is_ok_and(|value| value.trim() == "native-preview") {
        return Err(Error::Other(
            "load this native bundle into the running game before recording".into(),
        ));
    }
    let request = bundle.join(format!("arctic-bedrock-{pid}.capture-request.json"));
    let temporary = bundle.join(format!(
        ".capture-{}-{}.tmp",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let mut command = serde_json::json!({"op":"start","ffmpeg":ffmpeg,"output":output,"fps":fps,"frames":seconds*fps});
    if !encoder.is_ffmpeg() {
        command["encoder"] = serde_json::to_value(encoder)?;
    }
    let data = serde_json::to_vec(&command)?;
    let publish = (|| -> Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .at(&temporary)?;
        file.write_all(&data).at(&temporary)?;
        drop(file);
        // Atomic exclusive publication: the helper never reads a partial request
        // and an unconsumed command from another caller cannot be replaced.
        std::fs::hard_link(&temporary, &request).at(&request)?;
        Ok(())
    })();
    let _ = std::fs::remove_file(&temporary);
    publish?;
    let report = bundle.join(format!("arctic-bedrock-{pid}.capture.json"));
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(seconds as u64 + 25) {
        if let Ok(bytes) = std::fs::read(&report)
            && let Ok(status) = serde_json::from_slice::<serde_json::Value>(&bytes)
        {
            if status["requestOutput"] == serde_json::json!(output)
                && let Some(error) = status["requestError"].as_str()
            {
                return Err(Error::Other(error.into()));
            }
            if status["output"] != serde_json::json!(output) {
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
            match status["state"].as_str() {
                Some("complete") => {
                    if status["frames"].as_u64() != Some((seconds * fps) as u64) {
                        return Err(Error::Other(
                            "capture ended before its requested frame count".into(),
                        ));
                    }
                    return Ok(output);
                }
                Some("error") => {
                    return Err(Error::Other(
                        status["message"]
                            .as_str()
                            .unwrap_or("native video capture failed")
                            .into(),
                    ));
                }
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(Error::Other(format!(
        "video capture did not finish; inspect {}",
        report.display()
    )))
}

#[cfg(not(windows))]
pub fn record_video(_: Edition, _: &Path, _: &Path, _: &Path, _: u32, _: u32) -> Result<PathBuf> {
    Err(Error::Other("native video capture requires Windows".into()))
}
#[cfg(not(windows))]
pub fn record_video_with_encoder(
    _: Edition,
    _: &Path,
    _: &Path,
    _: super::capture::EncoderKind,
    _: &Path,
    _: u32,
    _: u32,
) -> Result<PathBuf> {
    Err(Error::Other("native video capture requires Windows".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_an_incomplete_bundle_before_launching_the_game() {
        let dir = tempfile::tempdir().unwrap();
        assert!(
            validate_bundle(dir.path())
                .unwrap_err()
                .to_string()
                .contains("missing")
        );
    }

    #[test]
    fn rejects_invalid_native_options_before_starting_a_game() {
        let dir = tempfile::tempdir().unwrap();
        let java = dir.path().join("java");
        std::fs::create_dir_all(java.join("bin/server")).unwrap();
        std::fs::write(java.join("bin/server/jvm.dll"), []).unwrap();
        for name in [
            "arctic-bedrock.jar",
            "arctic_bedrock.dll",
            "arctic-bedrock-loader.exe",
        ] {
            std::fs::write(dir.path().join(name), []).unwrap();
        }
        let valid = serde_json::json!({"java_home":java,"config_dir":dir.path().join("config"),
            "game_version":"1.26.52","offline_name":"ArcticGuest","renderer":"dx12"});
        let config = dir.path().join("arctic-bedrock.json");
        std::fs::write(&config, serde_json::to_vec(&valid).unwrap()).unwrap();
        assert!(validate_bundle(dir.path()).is_ok());
        for (key, value) in [
            ("offline_name", serde_json::json!("../account")),
            ("renderer", serde_json::json!("vulkan")),
            ("game_version", serde_json::json!("")),
            ("access_token", serde_json::json!("must-not-be-accepted")),
        ] {
            let mut invalid = valid.clone();
            invalid[key] = value;
            std::fs::write(&config, serde_json::to_vec(&invalid).unwrap()).unwrap();
            assert!(validate_bundle(dir.path()).is_err(), "accepted {key}");
        }
    }
}
