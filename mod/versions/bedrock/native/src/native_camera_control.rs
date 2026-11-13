//! Camera-consumer scoped control of the checked native camera.
//! Native player updates and saved camera matrices are never overwritten.
use crate::ecs::{actor_context, component_context, hash};
use crate::native_camera::{RenderBinding, Source};
use crate::world_hooks::{floats, read};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::cell::{Cell, RefCell};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};

type Run = unsafe extern "system" fn(usize, usize, f32);
type Matrix = unsafe extern "system" fn(usize, usize, usize);
static INSTALLED_BINDING: OnceLock<crate::world_hooks::CameraConsumerBinding> = OnceLock::new();
static CONSUMER_ENABLED: AtomicBool = AtomicBool::new(false);
static MATRIX_ENABLED: AtomicBool = AtomicBool::new(false);
const EXPIRED_BEFORE_APPLY: &str = "native camera lease expired before application";
static ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static MATRIX_ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static MATRIX_MATCHES: AtomicU64 = AtomicU64::new(0);
static OBSERVED: Mutex<Option<(String, usize, std::time::Instant)>> = Mutex::new(None);
static CALLS: AtomicU64 = AtomicU64::new(0);
static APPLIED: AtomicU64 = AtomicU64::new(0);
static RESTORED: AtomicU64 = AtomicU64::new(0);
static ROOT: OnceLock<PathBuf> = OnceLock::new();
static PROBE_ENABLED: AtomicBool = AtomicBool::new(false);
static QUEUED: Mutex<Option<(Request, Instant)>> = Mutex::new(None);
static REJECTED: Mutex<Option<Value>> = Mutex::new(None);
static REPORTED: Mutex<Value> = Mutex::new(Value::Null);
static CONTROL: Mutex<Control> = Mutex::new(Control {
    session: String::new(),
    sequence: 0,
    target: None,
    pending: None,
    last: None,
    error: None,
    restoration_failed: false,
    request_id: None,
    expected_camera: None,
    lease: None,
    lease_ms: None,
    lease_expired: false,
    consumer: None,
    consumer_serial: 0,
    captured: Vec::new(),
    ambiguous_frames: Vec::new(),
});
struct Control {
    session: String,
    sequence: u64,
    target: Option<Target>,
    pending: Option<u64>,
    last: Option<Value>,
    error: Option<String>,
    restoration_failed: bool,
    request_id: Option<String>,
    expected_camera: Option<ExpectedCamera>,
    lease: Option<Instant>,
    lease_ms: Option<u32>,
    lease_expired: bool,
    consumer: Option<u64>,
    consumer_serial: u64,
    captured: Vec<(crate::render_bridge::FrameKey, Value)>,
    ambiguous_frames: Vec<crate::render_bridge::FrameKey>,
}
#[derive(Clone, Deserialize, Debug)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Target {
    position: [f32; 3],
    orientation: [f32; 4],
    /// Public command units are degrees; target_bytes converts to radians.
    field_of_view: f32,
}
#[derive(Clone, Deserialize, Serialize, Debug)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ExpectedCamera {
    session: String,
    sequence: u64,
    request_id: Option<String>,
    active: bool,
}
struct CameraSelection {
    target: Target,
    sequence: u64,
    request_id: Option<String>,
    consumer: u64,
}
fn expire_lease(state: &mut Control) {
    if state.lease.is_some_and(|until| until <= Instant::now()) {
        state.target = None;
        state.lease = None;
        state.lease_expired = true;
        state.captured.clear();
    }
    // A worker never restores engine bytes. A completed scope already owns
    // that observation; an in-flight scope must publish its actual restoration.
    if state.lease_expired
        && state.consumer.is_none()
        && state.pending.is_none()
        && state.error.is_none()
        && !state.restoration_failed
        && let Some(last) = state.last.as_mut()
        && last["schema"].as_u64() == Some(1)
        && last["session"].as_str() == Some(state.session.as_str())
        && last["sequence"].as_u64() == Some(state.sequence)
        && last.get("requestId") == Some(&json!(state.request_id))
        && last["op"].as_str() == Some("set")
        && last["active"].as_bool() == Some(true)
        && last["error"].is_null()
        && last["restored"].as_bool() == Some(true)
        && last["playerUnchanged"].as_bool() == Some(true)
    {
        last["op"] = json!("lease_expired");
        last["active"] = json!(false);
        last["leaseActive"] = json!(false);
        last["leaseExpired"] = json!(true);
        last["expiryObservation"] = json!(true);
    }
}
fn valid_request_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    schema: u32,
    sequence: u64,
    session: String,
    op: String,
    target: Option<Target>,
    #[serde(default, rename = "requestId")]
    request_id: Option<String>,
    #[serde(default, rename = "expectedCamera")]
    expected_camera: Option<ExpectedCamera>,
    #[serde(default, rename = "leaseMs")]
    lease_ms: Option<u32>,
}
fn expected_camera(request: &Request, state: &Control) -> Result<(), String> {
    let Some(expected) = request.expected_camera.as_ref() else {
        return Ok(());
    };
    let acknowledged = if expected.sequence == 0 {
        state.last.is_none() && expected.request_id.is_none()
    } else {
        state.last.as_ref().is_some_and(|last| {
            last["schema"].as_u64() == Some(1)
                && last["session"].as_str() == Some(expected.session.as_str())
                && last["sequence"].as_u64() == Some(expected.sequence)
                && last.get("requestId") == Some(&json!(expected.request_id))
                && last["active"].as_bool() == Some(expected.active)
                && last["restored"].as_bool() == Some(true)
                && last["error"].is_null()
                && (!expected.active || last["playerUnchanged"].as_bool() == Some(true))
        })
    };
    if state.session != expected.session
        || state.sequence != expected.sequence
        || state.request_id != expected.request_id
        || state.target.is_some() != expected.active
        || state.pending.is_some()
        || state.error.is_some()
        || state.restoration_failed
        || !acknowledged
        || !expected.active && state.consumer.is_some()
    {
        return Err("native camera ownership changed before the guarded command".into());
    }
    Ok(())
}
fn validate(request: &Request, session: &str, last: u64) -> Result<(), String> {
    if request.schema != 1
        || !(1..=1_000_000_000).contains(&request.sequence)
        || request.sequence <= last
        || request.session != session
        || !matches!(request.op.as_str(), "set" | "release")
        || (request.op == "set") != request.target.is_some()
        || request
            .request_id
            .as_ref()
            .is_some_and(|id| !valid_request_id(id))
        || request
            .lease_ms
            .is_some_and(|lease| !(500..=30_000).contains(&lease) || request.target.is_none())
        || request.expected_camera.is_some()
            && request.target.is_some()
            && request.lease_ms.is_none()
        || request.expected_camera.as_ref().is_some_and(|expected| {
            request.request_id.is_none()
                || expected.session != request.session
                || expected.sequence >= request.sequence
                || expected.sequence > 1_000_000_000
                || expected
                    .request_id
                    .as_ref()
                    .is_some_and(|id| !valid_request_id(id))
                || expected.sequence == 0 && (expected.active || expected.request_id.is_some())
                || expected.active && expected.request_id.is_none()
                || !expected.active && request.op != "set"
        })
    {
        return Err("invalid native camera command/session/sequence".into());
    }
    if let Some(t) = &request.target {
        let norm = t.orientation.iter().map(|v| v * v).sum::<f32>();
        if t.position
            .iter()
            .any(|v| !v.is_finite() || v.abs() > 30_000_000.)
            || t.orientation.iter().any(|v| !v.is_finite())
            || !(0.95..=1.05).contains(&norm)
            || !t.field_of_view.is_finite()
            || !(1. ..=179.).contains(&t.field_of_view)
        {
            return Err("invalid native camera position/quaternion/FOV".into());
        }
    }
    Ok(())
}
pub(crate) fn configure(root: &Path, isolated: bool) {
    let _ = ROOT.set(root.to_path_buf());
    PROBE_ENABLED.store(isolated && crate::native_body::enabled(), Ordering::Release);
}

/// Queue one owned camera request. Engine memory is touched only later, inside
/// the checked native consumer; admission is not a rendered-frame receipt.
pub(crate) fn command(bytes: &[u8]) -> Result<(), String> {
    if bytes.is_empty() || bytes.len() > 16 * 1024 {
        return Err("native camera command exceeds 16 KiB or is empty".into());
    }
    let request: Request = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if ROOT.get().is_none()
        || !CONSUMER_ENABLED.load(Ordering::Acquire)
        || !MATRIX_ENABLED.load(Ordering::Acquire)
        || !OBSERVED
            .lock()
            .map_err(|e| e.to_string())?
            .as_ref()
            .is_some_and(|(s, _, at)| {
                s == &request.session && at.elapsed() < Duration::from_secs(1)
            })
    {
        return Err("no fresh owned native render camera for this session".into());
    }
    let mut state = CONTROL.lock().map_err(|e| e.to_string())?;
    expire_lease(&mut state);
    if state.session == request.session && state.restoration_failed {
        return Err(
            "native camera restoration previously failed; a fresh world session is required".into(),
        );
    }
    let last = if state.session == request.session {
        state.sequence
    } else {
        0
    };
    validate(&request, &request.session, last)?;
    let mut queued = QUEUED.lock().map_err(|e| e.to_string())?;
    if queued.is_some() || state.pending.is_some() {
        return Err("wait for the pending native camera command".into());
    }
    expected_camera(&request, &state)?;
    *queued = Some((request, Instant::now()));
    if let Ok(mut rejected) = REJECTED.lock() {
        *rejected = None;
    }
    Ok(())
}
fn acknowledge(value: &Value) {
    if !PROBE_ENABLED.load(Ordering::Acquire) {
        return;
    }
    let Some(root) = ROOT.get() else { return };
    let tmp = root.join("probe-camera-control-result.json.tmp");
    if let Ok(bytes) = serde_json::to_vec(value)
        && std::fs::write(&tmp, bytes).is_ok()
    {
        let _ = std::fs::rename(tmp, root.join("probe-camera-control-result.json"));
    }
}
fn poll(binding: &RenderBinding) -> Option<CameraSelection> {
    let root = ROOT.get()?;
    let mut state = CONTROL.lock().ok()?;
    // Claim the complete consumer, not just its patch on this thread. Another
    // render callback cannot apply/reset a new command before this one restores.
    if state.consumer.is_some() {
        return None;
    }
    if state.session != binding.session {
        state.session = binding.session.clone();
        state.sequence = 0;
        state.target = None;
        state.pending = None;
        state.last = None;
        state.error = None;
        state.restoration_failed = false;
        state.request_id = None;
        state.expected_camera = None;
        state.lease = None;
        state.lease_ms = None;
        state.lease_expired = false;
        state.captured.clear();
        state.ambiguous_frames.clear();
    }
    expire_lease(&mut state);
    let queued = QUEUED.lock().ok()?.take();
    let had_queued = queued.is_some();
    if let Some((request, at)) = queued {
        let result = if at.elapsed() > Duration::from_secs(2) {
            Err("native camera request expired before its render consumer".into())
        } else {
            validate(&request, &binding.session, state.sequence)
        };
        accept(&mut state, request, result);
    }
    let file = root.join("probe-camera-control.json");
    if !had_queued
        && PROBE_ENABLED.load(Ordering::Acquire)
        && let Ok(metadata) = file.metadata()
    {
        let parsed = if metadata.len() <= 16 * 1024 {
            bounded_read(&file)
                .and_then(|v| serde_json::from_slice::<Request>(&v).map_err(|e| e.to_string()))
        } else {
            Err("native camera command exceeds 16 KiB".into())
        };
        let _ = std::fs::remove_file(&file);
        match parsed {
            Ok(request) => {
                let result = validate(&request, &binding.session, state.sequence);
                accept(&mut state, request, result);
            }
            Err(error) => acknowledge(&json!({"schema":1,"error":error})),
        }
    }
    expire_lease(&mut state);
    let target = state.target.clone()?;
    let request_id = state.request_id.clone();
    let consumer = state.consumer_serial.checked_add(1)?;
    state.consumer_serial = consumer;
    state.consumer = Some(consumer);
    Some(CameraSelection {
        target,
        sequence: state.sequence,
        request_id,
        consumer,
    })
}
fn accept(state: &mut Control, request: Request, result: Result<(), String>) {
    let result = result.and_then(|()| {
        if state.restoration_failed {
            Err(
                "native camera restoration previously failed; a fresh world session is required"
                    .into(),
            )
        } else {
            expected_camera(&request, state)
        }
    });
    if let Err(error) = result {
        let value = json!({"schema":1,"sequence":request.sequence,"session":request.session,"requestId":request.request_id,"error":error});
        acknowledge(&value);
        if let Ok(mut rejected) = REJECTED.lock() {
            *rejected = Some(value);
        }
        return;
    }
    state.sequence = request.sequence;
    state.error = None;
    state.target = request.target;
    state.request_id = request.request_id;
    state.expected_camera = request.expected_camera;
    state.lease_ms = request.lease_ms;
    state.lease = request
        .lease_ms
        .map(|ms| Instant::now() + Duration::from_millis(u64::from(ms)));
    state.lease_expired = false;
    state.captured.clear();
    if request.op == "release" {
        state.pending = None;
        // Prior patches are scoped and restored before this consumer starts.
        let mut value = json!({"schema":1,"sequence":request.sequence,"session":request.session,"requestId":state.request_id,
            "op":"release","active":false,"restored":true,"leaseActive":false,"leaseExpired":false,"leaseMs":null});
        if let Some(expected) = state.expected_camera.as_ref() {
            value["expectedCamera"] = json!(expected);
        }
        acknowledge(&value);
        state.last = Some(value);
    } else {
        state.pending = Some(request.sequence);
    }
}
fn bounded_read(path: &Path) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|file| file.take(16 * 1024 + 1).read_to_end(&mut bytes))
        .map_err(|e| e.to_string())?;
    if bytes.len() > 16 * 1024 {
        return Err("native camera command exceeds 16 KiB".into());
    }
    Ok(bytes)
}

/// Serviced by the existing control worker, never by the native render thread.
pub(crate) fn service(root: &Path) {
    let pid = std::process::id();
    let request = root.join(format!("arctic-bedrock-{pid}.camera-request.json"));
    if request.is_file() {
        let bytes = bounded_read(&request);
        let _ = std::fs::remove_file(&request);
        let result = bytes
            .as_ref()
            .map_err(|e| e.clone())
            .and_then(|v| command(v));
        if let Err(error) = result {
            let identity = bytes
                .ok()
                .and_then(|v| serde_json::from_slice::<Value>(&v).ok())
                .unwrap_or(Value::Null);
            if let Ok(mut rejected) = REJECTED.lock() {
                *rejected = Some(
                    json!({"schema":1,"session":identity["session"],"sequence":identity["sequence"],"requestId":identity["requestId"],"error":error}),
                );
            }
        }
    }
    // A stopped renderer must not apply an old request when it resumes later.
    if let Ok(mut state) = CONTROL.lock() {
        expire_lease(&mut state);
        if let Ok(mut queued) = QUEUED.lock()
            && queued
                .as_ref()
                .is_some_and(|(_, at)| at.elapsed() > Duration::from_secs(2))
            && let Some((request, _)) = queued.take()
            && let Ok(mut rejected) = REJECTED.lock()
        {
            *rejected = Some(
                json!({"schema":1,"session":request.session,"sequence":request.sequence,"requestId":request.request_id,
                "error":"native camera request expired before its render consumer","active":state.target.is_some()}),
            );
        }
    }
    let next = status();
    if let Ok(mut previous) = REPORTED.lock()
        && *previous != next
    {
        let report = root.join(format!("arctic-bedrock-{pid}.camera.json"));
        let temporary = report.with_extension("json.tmp");
        if std::fs::write(&temporary, next.to_string()).is_ok()
            && std::fs::rename(temporary, report).is_ok()
        {
            *previous = next;
        }
    }
}

/// Queue schema-1 native camera JSON. Returns 0 for admission, -1 on rejection.
/// Completion, restoration and player-pose checks are in the camera report.
///
/// # Safety
/// `bytes` must address `length` readable bytes for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn arctic_bedrock_camera_v1(bytes: *const u8, length: usize) -> i32 {
    if bytes.is_null() || length == 0 || length > 16 * 1024 {
        return -1;
    }
    std::panic::catch_unwind(|| {
        let bytes = unsafe { std::slice::from_raw_parts(bytes, length) };
        command(bytes).map_or(-1, |_| 0)
    })
    .unwrap_or(-1)
}
fn write(address: usize, bytes: &[u8]) -> bool {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut core::ffi::c_void;
        fn WriteProcessMemory(
            p: *mut core::ffi::c_void,
            address: *mut core::ffi::c_void,
            bytes: *const u8,
            size: usize,
            count: *mut usize,
        ) -> i32;
    }
    let mut count = 0;
    unsafe {
        WriteProcessMemory(
            GetCurrentProcess(),
            address as _,
            bytes.as_ptr(),
            bytes.len(),
            &mut count,
        ) != 0
            && count == bytes.len()
    }
}
#[derive(Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct ActorPose {
    vector: [f32; 9],
    rotation: [f32; 4],
    body_yaw: [f32; 2],
    head_yaw: [f32; 2],
}
fn actor_pose(actor: usize) -> Option<ActorPose> {
    let c = actor_context(actor).ok()?;
    let get = |name, stride| component_context(c, hash(name), stride).ok().flatten();
    Some(ActorPose {
        vector: floats(get("StateVectorComponent", 36)?, 32_000_000.)?,
        rotation: floats(get("ActorRotationComponent", 16)?, 360_000.)?,
        body_yaw: floats(get("MobBodyRotationComponent", 8)?, 360_000.)?,
        head_yaw: floats(get("ActorHeadRotationComponent", 8)?, 360_000.)?,
    })
}
fn target_bytes(before: [u8; 44], target: &Target) -> [u8; 44] {
    let mut after = before;
    let norm = target.orientation.iter().map(|v| v * v).sum::<f32>().sqrt();
    for (i, value) in target
        .orientation
        .iter()
        .map(|v| v / norm)
        .chain(target.position)
        .enumerate()
    {
        after[i * 4..i * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
    after[32..36].copy_from_slice(&target.field_of_view.to_radians().to_le_bytes());
    after
}
struct Patch {
    source: Source,
    before: [u8; 44],
    after: [u8; 44],
    restored: bool,
}
impl Patch {
    fn restore(&mut self) -> bool {
        if self.restored {
            return true;
        }
        let Some(component) = self.source.component() else {
            return false;
        };
        let address = component + 0x30;
        if !write(address, &self.before) {
            return false;
        }
        let mut check = [0; 44];
        self.restored = read(address, &mut check).is_ok() && check == self.before;
        if self.restored {
            RESTORED.fetch_add(1, Ordering::Relaxed);
        }
        self.restored
    }
}
impl Drop for Patch {
    fn drop(&mut self) {
        self.restore();
    }
}
thread_local! {
    static PATCH:RefCell<Option<Patch>>=const {RefCell::new(None)};
    static ABORTED:Cell<bool>=const {Cell::new(false)};
    static CONSUMER:RefCell<Option<RenderBinding>>=const {RefCell::new(None)};
    static MATRIX_HIT:Cell<bool>=const {Cell::new(false)};
}
fn apply(source: Source, target: &Target) -> Option<Patch> {
    let component = source.component()?;
    // Require all native layout values to be readable, not just writable memory.
    let values = floats::<11>(component + 0x30, 32_000_000.)?;
    if !(0.1..=10.).contains(&values[7])
        || !(0.0001..=10.).contains(&values[9])
        || values[10] <= values[9]
    {
        return None;
    }
    let mut before = [0; 44];
    read(component + 0x30, &mut before).ok()?;
    let after = target_bytes(before, target);
    let mut patch = Patch {
        source,
        before,
        after,
        restored: false,
    };
    if !write(component + 0x30, &after) {
        if !patch.restore() {
            block_failed_restoration();
        }
        return None;
    }
    let mut check = [0; 44];
    if read(component + 0x30, &mut check).is_err() || check != after {
        if !patch.restore() {
            block_failed_restoration();
        }
        return None;
    }
    Some(patch)
}
fn block_failed_restoration() {
    if let Ok(mut state) = CONTROL.lock() {
        state.restoration_failed = true;
        state.target = None;
        state.lease = None;
        state.error =
            Some("native camera restoration failed; a fresh world session is required".into());
    }
}
struct Frame {
    binding: RenderBinding,
    before: Option<ActorPose>,
    producer: Option<crate::render_bridge::FrameKey>,
    selection: CameraSelection,
    had_patch: bool,
    failure: Option<&'static str>,
}
impl Drop for Frame {
    fn drop(&mut self) {
        let patch = PATCH.with(|p| p.borrow_mut().take());
        let after = actor_pose(self.binding.actor);
        let mut applied_camera = None;
        let mut restored = if let Some(mut patch) = patch {
            applied_camera = Some(
                patch
                    .after
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|b| f32::from_le_bytes(*b))
                    .collect::<Vec<_>>(),
            );
            patch.restore()
        } else {
            !self.had_patch
        };
        let mut error = if let Some(failure) = self.failure {
            Some(failure)
        } else if !MATRIX_HIT.get() {
            Some("controlled camera was not consumed by the native matrix calculation")
        } else if ABORTED.get() {
            Some("camera updater ran inside the controlled render frame")
        } else if !restored {
            Some("native camera restoration failed")
        } else if self.before.is_none() || after != self.before {
            Some("native player pose changed during controlled rendering")
        } else {
            None
        };
        if let Ok(mut state) = CONTROL.lock() {
            if state.session != self.binding.session
                || state.sequence != self.selection.sequence
                || state.request_id != self.selection.request_id
                || state.consumer != Some(self.selection.consumer)
            {
                return;
            }
            state.consumer = None;
            expire_lease(&mut state);
            restored &= !state.restoration_failed;
            state.restoration_failed |= !restored;
            if state.lease_expired
                && !self.had_patch
                && self.failure == Some(EXPIRED_BEFORE_APPLY)
                && restored
                && self.before.is_some()
                && after == self.before
            {
                // No patch was attempted. This can establish an inactive
                // expiry observation, never a consumed-camera frame receipt.
                error = None;
            }
            if let Some(error) = error {
                state.error = Some(error.into());
                state.target = None;
                state.lease = None;
            }
            let expired = state.lease_expired && error.is_none() && restored;
            let mut value = json!({"schema":1,"sequence":state.sequence,"session":self.binding.session,"requestId":state.request_id,"op":"set",
                "active":state.target.is_some(),"restored":restored,"playerUnchanged":self.before.is_some()&&after==self.before,
                "leaseMs":state.lease_ms,"leaseActive":state.lease.is_some_and(|until|until>Instant::now()),"leaseExpired":state.lease_expired,
                "playerBefore":self.before,"playerAfter":after,"cameraEntity":self.binding.source.entity(),"appliedCamera":applied_camera,"error":error,
                "nativeCameraControlVerified":false,"deterministicVideoVerified":false});
            if let Some(expected) = state.expected_camera.as_ref() {
                value["expectedCamera"] = json!(expected);
            }
            if expired {
                value["op"] = json!("lease_expired");
                value["expiryObservation"] = json!(true);
            }
            if error.is_none()
                && state.target.is_some()
                && let Some(key) = self
                    .producer
                    .filter(|key| Some(*key) == crate::render_bridge::producer_frame())
            {
                let summary = json!({"sequence":state.sequence,"requestId":state.request_id,
                    "producerFrame":key.number,"cameraEntity":self.binding.source.entity(),
                    "matrixConsumed":true,"restored":true,"playerUnchanged":true});
                let previous = state.captured.iter().position(|(old, _)| *old == key);
                let conflict = previous.is_some_and(|i| state.captured[i].1 != summary);
                if conflict {
                    state.captured.retain(|(old, _)| *old != key);
                    if !state.ambiguous_frames.contains(&key) {
                        if state.ambiguous_frames.len() == 8 {
                            state.ambiguous_frames.remove(0);
                        }
                        state.ambiguous_frames.push(key);
                    }
                } else if !state.ambiguous_frames.contains(&key) && previous.is_none() {
                    if state.captured.len() == 8 {
                        state.captured.remove(0);
                    }
                    state.captured.push((key, summary));
                }
            }
            if let Some(pending) = state.pending.take() {
                value["sequence"] = json!(pending);
                if expired && let Ok(mut rejected) = REJECTED.lock() {
                    *rejected = Some(
                        json!({"schema":1,"sequence":pending,"session":state.session,"requestId":state.request_id,
                        "error":"native camera lease expired before its acknowledgement"}),
                    );
                }
                acknowledge(&value);
            }
            state.last = Some(value);
        }
    }
}
fn begin(binding: RenderBinding) -> Option<Frame> {
    ROOT.get()?;
    if PATCH.with(|p| p.borrow().is_some()) {
        return None;
    }
    let component = binding.source.component()?;
    if !OBSERVED.lock().ok()?.as_ref().is_some_and(|(s, c, at)| {
        s == &binding.session && *c == component && at.elapsed() < std::time::Duration::from_secs(1)
    }) {
        return None;
    }
    let selection = poll(&binding)?;
    // Create the restoration/claim guard before any fallible observation or
    // patch. Early returns use the same identity-bound failure path as a frame.
    let mut frame = Frame {
        binding,
        before: None,
        producer: None,
        selection,
        had_patch: false,
        failure: None,
    };
    frame.producer = crate::render_bridge::producer_frame();
    frame.before = actor_pose(frame.binding.actor);
    if frame.before.is_none() {
        frame.failure = Some("native player pose unavailable for camera control");
        return None;
    }
    let live = CONTROL.lock().ok().is_some_and(|mut state| {
        expire_lease(&mut state);
        state.consumer == Some(frame.selection.consumer)
            && state.sequence == frame.selection.sequence
            && state.request_id == frame.selection.request_id
            && state.target.is_some()
            && state.error.is_none()
            && !state.restoration_failed
    });
    if !live {
        frame.failure = Some(EXPIRED_BEFORE_APPLY);
        return None;
    }
    let Some(patch) = apply(frame.binding.source, &frame.selection.target) else {
        frame.failure = Some("owned native camera patch/readback failed");
        return None;
    };
    frame.had_patch = true;
    PATCH.with(|p| *p.borrow_mut() = Some(patch));
    ABORTED.set(false);
    MATRIX_HIT.set(false);
    APPLIED.fetch_add(1, Ordering::Relaxed);
    Some(frame)
}
/// A restored camera consumer in this exact native producer frame. A global
/// "last command succeeded" receipt cannot bind a later world presentation.
pub(crate) fn capture_sample(
    session: &str,
    producer: crate::render_bridge::FrameKey,
) -> Option<Value> {
    let mut state = CONTROL.try_lock().ok()?;
    expire_lease(&mut state);
    if state.session != session
        || state.target.is_none()
        || state.pending.is_some()
        || state.error.is_some()
        || state.restoration_failed
        || state.ambiguous_frames.contains(&producer)
        || QUEUED.try_lock().ok()?.is_some()
    {
        return None;
    }
    let (key, receipt) = state.captured.iter().find(|(key, _)| *key == producer)?;
    (*key == producer
        && receipt["sequence"].as_u64() == Some(state.sequence)
        && receipt["requestId"].as_str() == state.request_id.as_deref())
    .then(|| receipt.clone())
}
pub(crate) fn before_camera_update(component: usize) {
    // An unexpected simulation update must receive the native values, never
    // the replay view. Abort the render override before forwarding the update.
    let patch = PATCH.with(|p| {
        let mut p = p.borrow_mut();
        if p.as_ref()
            .is_some_and(|p| p.source.component() == Some(component))
        {
            p.take()
        } else {
            None
        }
    });
    if let Some(mut patch) = patch {
        ABORTED.set(true);
        patch.restore();
    }
}
struct ConsumerScope(Option<RenderBinding>);
impl Drop for ConsumerScope {
    fn drop(&mut self) {
        CONSUMER.with(|c| *c.borrow_mut() = self.0.take());
    }
}
unsafe extern "system" fn run(this: usize, camera: usize, partial: f32) {
    CALLS.fetch_add(1, Ordering::Relaxed);
    let binding = ROOT
        .get()
        .and_then(|_| crate::native_camera::consumer_binding(this));
    let scope = ConsumerScope(CONSUMER.with(|c| c.replace(binding.clone())));
    let frame = binding.and_then(|b| std::panic::catch_unwind(|| begin(b)).ok().flatten());
    let original: Run = unsafe { std::mem::transmute(ORIGINAL.load(Ordering::Acquire)) };
    unsafe { original(this, camera, partial) };
    let _ = std::panic::catch_unwind(|| {
        drop(frame);
        drop(scope);
    });
}
fn observe_matrix(input: usize) {
    CONSUMER.with(|c| {
        let c = c.borrow();
        let Some(binding) = c.as_ref() else { return };
        if binding.source.component() == Some(input) {
            MATRIX_MATCHES.fetch_add(1, Ordering::Relaxed);
            if let Ok(mut observed) = OBSERVED.lock() {
                *observed = Some((binding.session.clone(), input, std::time::Instant::now()));
            }
            MATRIX_HIT.set(true);
        } else if let Some(component) = binding.source.component() {
            before_camera_update(component);
        }
    });
}
unsafe extern "system" fn matrix(output: usize, input: usize, transform: usize) {
    let _ = std::panic::catch_unwind(|| observe_matrix(input));
    let original: Matrix = unsafe { std::mem::transmute(MATRIX_ORIGINAL.load(Ordering::Acquire)) };
    unsafe { original(output, input, transform) };
}
pub(crate) unsafe fn install() -> Result<(), String> {
    let base = crate::world_hooks::image_base().ok_or("camera render image unavailable")?;
    let binding = crate::world_hooks::camera_consumer_binding()?;
    let consumer = base
        .checked_add(binding.consumer_rva)
        .ok_or("camera consumer address overflow")?;
    let matrix_target = base
        .checked_add(binding.matrix_rva)
        .ok_or("camera matrix address overflow")?;
    let mut code = [0; 16];
    read(consumer, &mut code)?;
    let mut expected = vec![
        0x41, 0x57, 0x41, 0x56, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x81, 0xec,
    ];
    expected.extend_from_slice(&binding.consumer_stack_bytes.to_le_bytes());
    if code.as_slice() != expected.as_slice() {
        return Err("native camera consumer prologue differs".into());
    }
    let mut code = [0; 12];
    read(matrix_target, &mut code)?;
    if code
        != [
            0x41, 0x56, 0x56, 0x57, 0x53, 0x48, 0x81, 0xec, 0x58, 0x01, 0, 0,
        ]
    {
        return Err("native camera matrix helper prologue differs".into());
    }
    // Validate both functions before creating either hook. The matrix observer
    // must be active before a consumer can acknowledge a controlled frame.
    INSTALLED_BINDING
        .set(binding)
        .map_err(|_| "camera consumer installation already attempted")?;
    use hudhook::mh::{MH_CreateHook, MH_DisableHook, MH_EnableHook, MH_STATUS};
    let mut original = std::ptr::null_mut();
    let result = unsafe { MH_CreateHook(consumer as _, run as *const () as _, &mut original) };
    if result != MH_STATUS::MH_OK {
        return Err(format!("create camera render hook: {result:?}"));
    }
    ORIGINAL.store(original as usize, Ordering::Release);
    let mut original = std::ptr::null_mut();
    let result =
        unsafe { MH_CreateHook(matrix_target as _, matrix as *const () as _, &mut original) };
    if result != MH_STATUS::MH_OK {
        return Err(format!("create camera matrix hook: {result:?}"));
    }
    MATRIX_ORIGINAL.store(original as usize, Ordering::Release);
    let result = unsafe { MH_EnableHook(matrix_target as _) };
    if result != MH_STATUS::MH_OK {
        return Err(format!("enable camera matrix hook: {result:?}"));
    }
    MATRIX_ENABLED.store(true, Ordering::Release);
    let result = unsafe { MH_EnableHook(consumer as _) };
    if result != MH_STATUS::MH_OK {
        let rollback = unsafe { MH_DisableHook(matrix_target as _) };
        if matches!(rollback, MH_STATUS::MH_OK | MH_STATUS::MH_ERROR_DISABLED) {
            MATRIX_ENABLED.store(false, Ordering::Release);
        }
        // Retain originals for callbacks already in flight during rollback.
        return Err(format!(
            "enable camera render hook: {result:?}; disable matrix rollback: {rollback:?}"
        ));
    }
    CONSUMER_ENABLED.store(true, Ordering::Release);
    Ok(())
}
pub(crate) fn status() -> Value {
    let mut state = CONTROL.lock().ok();
    if let Some(state) = state.as_mut() {
        expire_lease(state);
    }
    let observed = OBSERVED.lock().ok();
    let session = observed
        .as_ref()
        .and_then(|o| o.as_ref())
        .filter(|(_, _, at)| at.elapsed() < Duration::from_secs(1))
        .map(|(s, _, _)| s.clone());
    json!({"installed":CONSUMER_ENABLED.load(Ordering::Acquire),"controlEnabled":ROOT.get().is_some(),
        "hookRva":INSTALLED_BINDING.get().map(|b|b.consumer_rva),
        "matrixHookRva":INSTALLED_BINDING.get().map(|b|b.matrix_rva),
        "matrixInstalled":MATRIX_ENABLED.load(Ordering::Acquire),
        "idleAcquisitionGuardImplemented":true,"ownedFollowupGuardImplemented":true,"ownedReleaseGuardImplemented":true,
        "boundedLeaseImplemented":true,
        "probeEnabled":PROBE_ENABLED.load(Ordering::Acquire),"session":session,"calls":CALLS.load(Ordering::Relaxed),
        "bindingReady":state.as_ref().is_some_and(|s|session.as_deref()==Some(s.session.as_str())),
        "sequence":state.as_ref().map(|s|s.sequence),"requestId":state.as_ref().and_then(|s|s.request_id.clone()),
        "pending":state.as_ref().and_then(|s|s.pending),
        "consumerActive":state.as_ref().is_some_and(|s|s.consumer.is_some()),
        "leaseMs":state.as_ref().and_then(|s|s.lease_ms),"leaseExpired":state.as_ref().is_some_and(|s|s.lease_expired),
        "leaseRemainingMs":state.as_ref().and_then(|s|s.lease.map(|until|until.saturating_duration_since(Instant::now()).as_millis() as u64)),
        "applied":APPLIED.load(Ordering::Relaxed),"restored":RESTORED.load(Ordering::Relaxed),
        "nativeMatrixInputMatches":MATRIX_MATCHES.load(Ordering::Relaxed),
        "active":state.as_ref().is_some_and(|s|s.target.is_some()),"error":state.as_ref().and_then(|s|s.error.clone()),
        "restorationFailed":state.as_ref().is_some_and(|s|s.restoration_failed),
        "queued":QUEUED.lock().ok().and_then(|q|q.as_ref().map(|(r,_)|json!({"session":r.session,"sequence":r.sequence,"requestId":r.request_id}))),
        "requestRejected":REJECTED.lock().ok().and_then(|v|v.clone()),
        "last":state.as_ref().and_then(|s|s.last.clone()),"nativeCameraControlVerified":false})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn camera_patch_restores_native_bytes_on_drop_and_rejects_expired_owners() {
        fn point(bytes: &mut [u8], offset: usize, value: usize) {
            bytes[offset..offset + 8].copy_from_slice(&(value as u64).to_le_bytes());
        }
        let mut camera = [0u8; 0x120];
        for (i, v) in [0f32, 0., 0., 1., 1., 2., 3., 1.9, 1.15, 0.025, 2500.]
            .into_iter()
            .enumerate()
        {
            camera[0x30 + i * 4..0x34 + i * 4].copy_from_slice(&v.to_le_bytes());
        }
        let fixture = crate::ecs::Fixture::new(&[("MinecraftCamera::CameraComponent", &camera)]);
        let context = actor_context(fixture.address()).unwrap();
        let component = component_context(context, hash("MinecraftCamera::CameraComponent"), 0x120)
            .unwrap()
            .unwrap();
        let mut owner = vec![0u8; 0x164];
        read(context.registry, &mut owner[0x30..0xb0]).unwrap();
        let mut page = vec![0xffu8; 2048 * 4];
        page[28..32].copy_from_slice(&0x40003u32.to_le_bytes());
        let sparse = [page.as_ptr() as usize];
        point(&mut owner, 0x110, sparse.as_ptr() as usize);
        point(&mut owner, 0x118, sparse.as_ptr() as usize + 8);
        point(&mut owner, 0x150, 4);
        let source = Source::fixture(owner.as_ptr() as usize, context.entity, component);
        let target = Target {
            position: [10., 20., 30.],
            orientation: [0., 1., 0., 0.],
            field_of_view: 50.,
        };
        let before = floats::<11>(component + 0x30, 32_000_000.).unwrap();
        {
            let _patch = apply(source, &target).unwrap();
            let after = floats::<11>(component + 0x30, 32_000_000.).unwrap();
            assert_eq!(&after[4..7], &[10., 20., 30.]);
            assert_eq!(after[7], before[7]);
            assert_eq!(&after[9..], &before[9..]);
        }
        assert_eq!(floats::<11>(component + 0x30, 32_000_000.).unwrap(), before);
        point(&mut owner, 0x150, 3);
        std::hint::black_box(&owner);
        assert!(apply(source, &target).is_none());
        assert_eq!(floats::<11>(component + 0x30, 32_000_000.).unwrap(), before);
    }
    #[test]
    fn camera_requests_require_session_sequence_and_bounded_valid_native_values() {
        let mut request = Request {
            schema: 1,
            sequence: 2,
            session: "current".into(),
            op: "set".into(),
            request_id: None,
            expected_camera: None,
            lease_ms: None,
            target: Some(Target {
                position: [1., 2., 3.],
                orientation: [0., 0., 0., 1.],
                field_of_view: 60.,
            }),
        };
        assert!(validate(&request, "current", 1).is_ok());
        assert!(validate(&request, "expired", 1).is_err());
        assert!(validate(&request, "current", 2).is_err());
        request.target.as_mut().unwrap().orientation = [0.; 4];
        assert!(validate(&request, "current", 1).is_err());
        request.target = None;
        request.op = "release".into();
        assert!(validate(&request, "current", 1).is_ok());
    }
    #[test]
    fn camera_bytes_preserve_native_aspect_and_clipping_and_convert_degrees() {
        let mut before = [0; 44];
        for (i, v) in [0f32, 0., 0., 1., 1., 2., 3., 1.9, 1.15, 0.025, 2500.]
            .into_iter()
            .enumerate()
        {
            before[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        let target = Target {
            position: [10., 20., 30.],
            orientation: [0., 1., 0., 0.],
            field_of_view: 50.,
        };
        let after = target_bytes(before, &target);
        assert_eq!(&after[28..32], &before[28..32]);
        assert_eq!(&after[36..44], &before[36..44]);
        assert_eq!(
            f32::from_le_bytes(after[32..36].try_into().unwrap()),
            50f32.to_radians()
        );
        assert_eq!(f32::from_le_bytes(after[24..28].try_into().unwrap()), 30.);
    }
    #[test]
    fn native_consumer_detour_preserves_both_native_pointers_once() {
        static ARGS: Mutex<Vec<(usize, usize)>> = Mutex::new(Vec::new());
        #[inline(never)]
        unsafe extern "system" fn host(this: usize, camera: usize, _partial: f32) {
            ARGS.lock().unwrap().push((this, camera));
        }
        use hudhook::mh::{MH_CreateHook, MH_DisableHook, MH_EnableHook, MH_Initialize, MH_STATUS};
        assert!(matches!(
            unsafe { MH_Initialize() },
            MH_STATUS::MH_OK | MH_STATUS::MH_ERROR_ALREADY_INITIALIZED
        ));
        let at = host as *const () as usize;
        let mut trampoline = std::ptr::null_mut();
        assert_eq!(
            unsafe { MH_CreateHook(at as _, run as *const () as _, &mut trampoline) },
            MH_STATUS::MH_OK
        );
        ORIGINAL.store(trampoline as usize, Ordering::Release);
        assert_eq!(unsafe { MH_EnableHook(at as _) }, MH_STATUS::MH_OK);
        let call: Run = std::hint::black_box(host);
        unsafe { call(0x123456789, 0x234567891, 0.25) };
        assert_eq!(unsafe { MH_DisableHook(at as _) }, MH_STATUS::MH_OK);
        ORIGINAL.store(at, Ordering::Release);
        assert_eq!(*ARGS.lock().unwrap(), vec![(0x123456789, 0x234567891)]);
        assert!(CONSUMER.with(|c| c.borrow().is_none()));
    }
    #[test]
    fn native_matrix_detour_preserves_output_input_and_transform_once() {
        static ARGS: Mutex<Vec<[usize; 3]>> = Mutex::new(Vec::new());
        #[inline(never)]
        unsafe extern "system" fn host(output: usize, input: usize, transform: usize) {
            ARGS.lock().unwrap().push([output, input, transform]);
        }
        use hudhook::mh::{MH_CreateHook, MH_DisableHook, MH_EnableHook, MH_Initialize, MH_STATUS};
        assert!(matches!(
            unsafe { MH_Initialize() },
            MH_STATUS::MH_OK | MH_STATUS::MH_ERROR_ALREADY_INITIALIZED
        ));
        let at = host as *const () as usize;
        let mut trampoline = std::ptr::null_mut();
        assert_eq!(
            unsafe { MH_CreateHook(at as _, matrix as *const () as _, &mut trampoline) },
            MH_STATUS::MH_OK
        );
        MATRIX_ORIGINAL.store(trampoline as usize, Ordering::Release);
        assert_eq!(unsafe { MH_EnableHook(at as _) }, MH_STATUS::MH_OK);
        let call: Matrix = std::hint::black_box(host);
        unsafe { call(0x123456789, 0x234567891, 0x345678912) };
        assert_eq!(unsafe { MH_DisableHook(at as _) }, MH_STATUS::MH_OK);
        MATRIX_ORIGINAL.store(at, Ordering::Release);
        assert_eq!(
            *ARGS.lock().unwrap(),
            vec![[0x123456789, 0x234567891, 0x345678912]]
        );
    }
}
