//! Read-only replay-marker correlation around the pinned latency dispatcher.
//! A callback return is not proof that earlier chunk work or terrain completed.
use crate::world_hooks::{pointer, read};
use hudhook::windows::Win32::System::Threading::GetCurrentThreadId;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::{
    Mutex,
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};
use std::{
    collections::HashSet,
    io::Read,
    path::Path,
    time::{Duration, Instant},
};

const DISPATCH: usize = 0x1733ce0;
const DISPATCH_TABLE: usize = 0xe7bfea0;
const DISPATCH_INSTANCE: usize = 0x1185b5b0;
const PACKET_TABLE: usize = 0xe7b0fd0;
const PACKET_ID: usize = 0x3c40700;
type Handle = unsafe extern "system" fn(usize, usize, usize, usize);
static ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static INSTALLED: AtomicBool = AtomicBool::new(false);
static CALLS: AtomicU64 = AtomicU64::new(0);
static FORWARDED: AtomicU64 = AtomicU64::new(0);
static OBSERVED: AtomicU64 = AtomicU64::new(0);
static MAIN_THREAD: AtomicU64 = AtomicU64::new(0);
static REJECTED: AtomicU64 = AtomicU64::new(0);
static CLIENT: Mutex<Option<ClientThread>> = Mutex::new(None);
static HISTORY: Mutex<Vec<Observation>> = Mutex::new(Vec::new());
static GATE: Mutex<Gate> = Mutex::new(Gate {
    session: String::new(),
    sequence: 0,
    pending: None,
    completed: None,
    last: None,
    rejected: None,
    used: None,
});
static REPORTED: Mutex<(Value, Option<Instant>)> = Mutex::new((Value::Null, None));

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    schema: u32,
    session: String,
    sequence: u64,
    request_id: String,
    marker: String,
    generation: u64,
    replay_sequence: u64,
    time_ms: u32,
    #[serde(default)]
    time_remainder: u32,
    #[serde(default = "unit_denominator")]
    time_denominator: u32,
    #[serde(default = "client_tick_completion")]
    completion: String,
    #[serde(default = "marker_timeout")]
    timeout_ms: u32,
}
fn marker_timeout() -> u32 {
    15000
}
fn unit_denominator() -> u32 {
    1
}
fn client_tick_completion() -> String {
    "client_tick".into()
}
struct Gate {
    session: String,
    sequence: u64,
    pending: Option<Pending>,
    completed: Option<Completed>,
    last: Option<Value>,
    rejected: Option<Value>,
    // Never reuse a marker across world transitions in this process. An old
    // peer's delayed packet must not satisfy an arm in a later native world.
    used: Option<HashSet<String>>,
}
struct Pending {
    request: Request,
    at: Instant,
    after: u64,
    dispatch: Option<Dispatch>,
}
struct Dispatch {
    observation: u64,
    tick: u64,
    client: ClientThread,
}
struct Completed {
    client: ClientThread,
    tick: u64,
    receipt: Value,
    at: Instant,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct HistoryAnchor {
    pub recording_id: String,
    pub sequence: u64,
    pub request_id: String,
    pub marker: String,
    pub generation: u64,
    pub replay_sequence: u64,
}
impl HistoryAnchor {
    pub fn valid(&self) -> bool {
        self.recording_id.len() == 36
            && self.recording_id.bytes().enumerate().all(|(i, b)| {
                if [8, 13, 18, 23].contains(&i) {
                    b == b'-'
                } else {
                    b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
                }
            })
            && valid_token(&self.request_id)
            && [self.sequence, self.generation, self.replay_sequence]
                .iter()
                .all(|n| (1..=1_000_000_000).contains(n))
            && self.marker.len() == 16
            && self.marker.starts_with("37c1")
            && self
                .marker
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }
}
pub(crate) fn history_anchor(key: &HistoryAnchor, session: &str) -> Result<Value, String> {
    if !key.valid() {
        return Err("invalid recording anchor identity".into());
    }
    let client = client_context().ok_or("recording anchor lost its primary client")?;
    let mut gate = GATE.lock().map_err(|e| e.to_string())?;
    reconcile(&mut gate);
    let completed = gate
        .completed
        .as_ref()
        .ok_or("recording anchor has no completed client tick")?;
    let value = &completed.receipt;
    if completed.at.elapsed() > Duration::from_secs(3)
        || completed.client != client
        || gate.session != session
        || value["session"].as_str() != Some(session)
        || value["sequence"].as_u64() != Some(key.sequence)
        || value["requestId"].as_str() != Some(key.request_id.as_str())
        || value["marker"].as_str() != Some(key.marker.as_str())
        || value["generation"].as_u64() != Some(key.generation)
        || value["replaySequence"].as_u64() != Some(key.replay_sequence)
        || value["stage"].as_str() != Some("client_tick_completed")
        || !value["clientTickElapsedMs"]
            .as_f64()
            .is_some_and(|n| n.is_finite() && n >= 0.)
    {
        return Err("recording anchor does not match the owned native receipt".into());
    }
    let mut result = value.clone();
    result["recordingId"] = json!(key.recording_id);
    result["alignment"] = json!("marker_post_tick");
    Ok(result)
}
fn valid_token(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}
fn receipt(request: &Request, stage: &str) -> Value {
    json!({"schema":1,"session":request.session,"sequence":request.sequence,"requestId":request.request_id,
        "marker":request.marker,"generation":request.generation,"replaySequence":request.replay_sequence,
        "timeMs":request.time_ms,"timeRemainder":request.time_remainder,"timeDenominator":request.time_denominator,
        "completion":request.completion,"stage":stage,"packetApplicationVerified":false,
        "nativeWorldVerified":false,"terrainReady":false,"deterministicVideoReady":false})
}
fn reconcile(gate: &mut Gate) {
    let session = client_context().map(|c| format!("native_{}_{}", std::process::id(), c.epoch));
    // A failed try-lock or a temporarily stale player-directory sample is
    // inconclusive. Retire only a positively observed new session, or timeout.
    let changed = session.as_ref().is_some_and(|s| *s != gate.session);
    let expired = gate
        .pending
        .as_ref()
        .is_some_and(|p| p.at.elapsed() > Duration::from_millis(u64::from(p.request.timeout_ms)));
    if changed || expired {
        if let Some(pending) = gate.pending.take() {
            let mut value = receipt(&pending.request, "failed");
            value["error"] = json!(if changed {
                "native packet-marker world session changed or was lost"
            } else {
                "native packet marker did not complete its dispatch/client tick before timeout"
            });
            gate.last = Some(value);
        }
        if changed {
            gate.completed = None;
            gate.sequence = 0;
            gate.session = session.unwrap_or_default();
        }
    }
}
pub(crate) fn arm(bytes: &[u8]) -> Result<(), String> {
    if bytes.is_empty() || bytes.len() > 4096 {
        return Err("packet arm exceeds 4 KiB or is empty".into());
    }
    let request: Request = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    let client =
        client_context().ok_or("packet arm needs a live primary-client tick/thread binding")?;
    let session = format!("native_{}_{}", std::process::id(), client.epoch);
    if !INSTALLED.load(Ordering::Acquire)
        || request.schema != 1
        || request.session != session
        || !valid_token(&request.request_id)
        || !(1..=1_000_000_000).contains(&request.sequence)
        || !(1..=1_000_000_000).contains(&request.generation)
        || !(1..=1_000_000_000).contains(&request.replay_sequence)
        || request.time_ms > 86_400_000
        || !(500..=30000).contains(&request.timeout_ms)
        || !(1..=1000).contains(&request.time_denominator)
        || request.time_remainder >= request.time_denominator
        || request.time_ms == 86_400_000 && request.time_remainder != 0
        || !matches!(request.completion.as_str(), "dispatch" | "client_tick")
        || request.marker.len() != 16
        || !request.marker.starts_with("37c1")
        || !request
            .marker
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    {
        return Err(
            "invalid packet-marker session, sequence, identity, timestamp or timeout".into(),
        );
    }
    let mut gate = GATE.lock().map_err(|e| e.to_string())?;
    reconcile(&mut gate);
    if request.session != gate.session
        || request.sequence <= gate.sequence
        || gate.pending.is_some()
    {
        return Err("packet arm needs an increasing sequence with no pending marker".into());
    }
    let used = gate.used.get_or_insert_with(HashSet::new);
    if used.len() >= 1_000_000 || !used.insert(request.marker.clone()) {
        return Err(
            "packet marker was already used or the process marker limit was reached".into(),
        );
    }
    gate.sequence = request.sequence;
    gate.completed = None;
    gate.rejected = None;
    gate.last = Some(receipt(&request, "armed"));
    gate.pending = Some(Pending {
        request,
        at: Instant::now(),
        after: OBSERVED.load(Ordering::Acquire),
        dispatch: None,
    });
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct ClientThread {
    level: usize,
    epoch: u64,
    thread: u32,
}

/// Called only after the actual primary level callback/cape tick completes.
/// Keep no actor, network peer or callback object alive between invocations.
pub(crate) fn completed_client_tick(level: usize, tick: u64) {
    let Some((current, epoch)) = crate::players::session().filter(|s| s.0 == level) else {
        return;
    };
    let next = ClientThread {
        level: current,
        epoch,
        thread: unsafe { GetCurrentThreadId() },
    };
    if let Ok(mut client) = CLIENT.lock() {
        *client = Some(next);
    }
    if let Ok(mut gate) = GATE.lock() {
        reconcile(&mut gate);
        let ready = gate
            .pending
            .as_ref()
            .and_then(|p| p.dispatch.as_ref())
            .is_some_and(|d| d.client == next && tick > d.tick);
        if ready {
            let pending = gate.pending.take().expect("matched pending dispatch");
            let dispatch = pending.dispatch.expect("matched dispatch");
            let mut value = receipt(&pending.request, "client_tick_completed");
            value["dispatcherReturned"] = json!(true);
            value["primaryClientThread"] = json!(true);
            value["postClientTickCompleted"] = json!(true);
            value["observationSequence"] = json!(dispatch.observation);
            value["dispatcherTick"] = json!(dispatch.tick);
            value["clientTick"] = json!(tick);
            value["clientTickElapsedMs"] =
                json!(crate::world_hooks::completed_tick_clock(level, tick));
            value["presentationClock"] = crate::native_presentation_clock::sample()
                .map(|sample| sample.value())
                .unwrap_or(Value::Null);
            gate.completed = Some(Completed {
                client: next,
                tick,
                receipt: value.clone(),
                at: Instant::now(),
            });
            gate.last = Some(value);
        }
    }
}
fn client_context() -> Option<ClientThread> {
    let client = *CLIENT.try_lock().ok()?;
    let client = client?;
    (crate::players::session()? == (client.level, client.epoch)).then_some(client)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Observation {
    sequence: u64,
    /// Hex preserves the exact uint64 payload across JSON/JavaScript callers.
    timestamp: String,
    needs_response: bool,
    thread: u32,
    client_session: Option<String>,
    primary_client_thread: bool,
    client_tick: Option<u64>,
    dispatcher_returned: bool,
    packet_application_verified: bool,
    terrain_ready: bool,
}
struct Incoming {
    timestamp: u64,
    needs_response: bool,
    client: Option<ClientThread>,
}

fn observe_marker(observation: &Observation, client: Option<ClientThread>) {
    let Some(client) =
        client.filter(|_| observation.primary_client_thread && observation.needs_response)
    else {
        return;
    };
    let Some(tick) = observation.client_tick else {
        return;
    };
    let Ok(mut gate) = GATE.lock() else {
        return;
    };
    reconcile(&mut gate);
    let value = if let Some(pending) = gate.pending.as_mut().filter(|p| {
        p.request.marker == observation.timestamp
            && p.request.session == format!("native_{}_{}", std::process::id(), client.epoch)
            && observation.sequence > p.after
            && p.dispatch.is_none()
    }) {
        pending.dispatch = Some(Dispatch {
            observation: observation.sequence,
            tick,
            client,
        });
        let mut value = receipt(&pending.request, "dispatch_returned");
        value["dispatcherReturned"] = json!(true);
        value["primaryClientThread"] = json!(true);
        value["observationSequence"] = json!(observation.sequence);
        value["dispatcherTick"] = json!(tick);
        value["postClientTickCompleted"] = json!(false);
        Some((value, pending.request.completion == "dispatch"))
    } else {
        None
    };
    if let Some((value, dispatch_only)) = value {
        if dispatch_only {
            gate.pending = None;
            gate.completed = Some(Completed {
                client,
                tick,
                receipt: value.clone(),
                at: Instant::now(),
            });
        }
        gate.last = Some(value);
    }
}

fn incoming(instance: usize, packet_reference: usize) -> Option<Incoming> {
    let base = crate::world_hooks::image_base()?;
    // The inspected factory stores this exact singleton at Packet+0x20.
    // Its virtual slot 1 is the thunk we observe; no native virtual is called.
    if instance != base.checked_add(DISPATCH_INSTANCE)?
        || pointer(instance)? != base.checked_add(DISPATCH_TABLE)?
        || pointer(base.checked_add(DISPATCH_TABLE + 8)?)? != base.checked_add(DISPATCH)?
    {
        return None;
    }
    let mut shared = [0u8; 16];
    read(packet_reference, &mut shared).ok()?;
    let packet = usize::from_le_bytes(shared[..8].try_into().ok()?);
    let control = usize::from_le_bytes(shared[8..].try_into().ok()?);
    if packet < 0x10000
        || control < 0x10000
        || pointer(packet)? != base.checked_add(PACKET_TABLE)?
        || pointer(packet.checked_add(0x20)?)? != instance
        || pointer(base.checked_add(PACKET_TABLE + 8)?)? != base.checked_add(PACKET_ID)?
    {
        return None;
    }
    let mut counts = [0u8; 8];
    read(control.checked_add(8)?, &mut counts).ok()?;
    if i32::from_le_bytes(counts[..4].try_into().ok()?) <= 0
        || i32::from_le_bytes(counts[4..].try_into().ok()?) <= 0
    {
        return None;
    }
    let mut payload = [0u8; 9];
    read(packet.checked_add(0x30)?, &mut payload).ok()?;
    if payload[8] > 1 {
        return None;
    }
    let mut after = [0u8; 16];
    read(packet_reference, &mut after).ok()?;
    if after != shared {
        return None;
    }
    Some(Incoming {
        timestamp: u64::from_le_bytes(payload[..8].try_into().ok()?),
        needs_response: payload[8] != 0,
        client: client_context(),
    })
}

unsafe extern "system" fn handle(instance: usize, network: usize, callback: usize, packet: usize) {
    CALLS.fetch_add(1, Ordering::Relaxed);
    // Copy before the original: the shared_ptr reference may be consumed or
    // reassigned by native code. Rust never retains or adjusts its reference.
    let before = std::panic::catch_unwind(|| incoming(instance, packet))
        .ok()
        .flatten();
    unsafe {
        let original: Handle = std::mem::transmute(ORIGINAL.load(Ordering::Acquire));
        original(instance, network, callback, packet);
    }
    FORWARDED.fetch_add(1, Ordering::Relaxed);
    let _ = std::panic::catch_unwind(|| {
        let Some(before) = before else {
            REJECTED.fetch_add(1, Ordering::Relaxed);
            return;
        };
        let client = before.client.filter(|c| client_context() == Some(*c));
        let thread = unsafe { GetCurrentThreadId() };
        let primary = client.is_some_and(|c| c.thread == thread);
        let sequence = OBSERVED.fetch_add(1, Ordering::Relaxed) + 1;
        if primary {
            MAIN_THREAD.fetch_add(1, Ordering::Relaxed);
        }
        let observation = Observation {
            sequence,
            timestamp: format!("{:016x}", before.timestamp),
            needs_response: before.needs_response,
            thread,
            client_session: client.map(|c| format!("native_{}_{}", std::process::id(), c.epoch)),
            primary_client_thread: primary,
            client_tick: client.and_then(|c| crate::world_hooks::client_completed_tick(c.level)),
            dispatcher_returned: true,
            packet_application_verified: false,
            terrain_ready: false,
        };
        observe_marker(&observation, client);
        if let Ok(mut history) = HISTORY.try_lock() {
            if history.len() == 8 {
                history.remove(0);
            }
            history.push(observation);
        }
    });
}

/// Copy a completed marker only into a world at or after its actual post-marker
/// tick. This is dispatch/tick correlation, never a terrain/application claim.
pub(crate) fn capture_sample(session: &str, tick: u64) -> Option<Value> {
    let gate = GATE.try_lock().ok()?;
    let completed = gate.completed.as_ref()?;
    let tick_matches = if completed.receipt["stage"].as_str() == Some("dispatch_returned") {
        tick == completed.tick
    } else {
        tick >= completed.tick
    };
    (gate.session == session && client_context() == Some(completed.client) && tick_matches)
        .then(|| completed.receipt.clone())
}

pub(crate) fn service(root: &Path) {
    let pid = std::process::id();
    let path = root.join(format!("arctic-bedrock-{pid}.packet-request.json"));
    if path.is_file() {
        let mut bytes = Vec::new();
        let result = std::fs::File::open(&path)
            .and_then(|f| f.take(4097).read_to_end(&mut bytes))
            .map_err(|e| e.to_string())
            .and_then(|_| arm(&bytes));
        let _ = std::fs::remove_file(&path);
        if let Err(error) = result
            && let Ok(mut gate) = GATE.lock()
        {
            let value = serde_json::from_slice::<Value>(&bytes).unwrap_or(Value::Null);
            gate.rejected = Some(
                json!({"session":value["session"],"sequence":value["sequence"],
                "requestId":value["requestId"],"marker":value["marker"],"error":error}),
            );
        }
    }
    if let Ok(mut gate) = GATE.lock() {
        reconcile(&mut gate);
    }
    let next = status();
    // Refresh unchanged receipts as well: a stopped worker's old "contextLive"
    // report must not remain acceptable to a later host-side wait.
    if let Ok(mut previous) = REPORTED.lock()
        && (previous.0 != next
            || previous
                .1
                .is_none_or(|at| at.elapsed() >= Duration::from_millis(500)))
    {
        let report = root.join(format!("arctic-bedrock-{pid}.packet.json"));
        let temporary = report.with_extension("json.tmp");
        if std::fs::write(&temporary, next.to_string()).is_ok()
            && std::fs::rename(temporary, report).is_ok()
        {
            *previous = (next, Some(Instant::now()));
        }
    }
}

/// Arm one bounded marker; success is admission, not a completed dispatcher.
/// # Safety
/// `bytes` must address `length` readable bytes for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn arctic_bedrock_packet_arm_v1(bytes: *const u8, length: usize) -> i32 {
    if bytes.is_null() || length == 0 || length > 4096 {
        return -1;
    }
    std::panic::catch_unwind(|| {
        arm(unsafe { std::slice::from_raw_parts(bytes, length) }).map_or(-1, |_| 0)
    })
    .unwrap_or(-1)
}

pub(crate) unsafe fn install() -> Result<(), String> {
    let base = crate::world_hooks::image_base().ok_or("packet image profile unavailable")?;
    let mut code = [0u8; 16];
    read(base + DISPATCH, &mut code)?;
    // RCX <- callback (R8), R8 <- shared_ptr.get() (R9), then callback virtual
    // at +0x2b0. The remaining native thunk performs its original CFG tailcall.
    if code
        != [
            0x4c, 0x89, 0xc1, 0x4d, 0x8b, 0x01, 0x48, 0x8b, 0x01, 0x48, 0x8b, 0x80, 0xb0, 0x02,
            0x00, 0x00,
        ]
    {
        return Err("latency dispatcher prologue differs from exact profile".into());
    }
    let mut id = [0u8; 6];
    read(base + PACKET_ID, &mut id)?;
    if id != [0xb8, 0x73, 0x00, 0x00, 0x00, 0xc3]
        || pointer(base + DISPATCH_INSTANCE) != Some(base + DISPATCH_TABLE)
        || pointer(base + DISPATCH_TABLE + 8) != Some(base + DISPATCH)
        || pointer(base + PACKET_TABLE + 8) != Some(base + PACKET_ID)
    {
        return Err("latency packet/dispatcher tables differ from exact profile".into());
    }
    use hudhook::mh::{MH_CreateHook, MH_EnableHook, MH_STATUS};
    let mut original = std::ptr::null_mut();
    let created = unsafe {
        MH_CreateHook(
            (base + DISPATCH) as _,
            handle as *const () as _,
            &mut original,
        )
    };
    if created != MH_STATUS::MH_OK {
        return Err(format!("create latency dispatcher observer: {created:?}"));
    }
    ORIGINAL.store(original as usize, Ordering::Release);
    let enabled = unsafe { MH_EnableHook((base + DISPATCH) as _) };
    if enabled != MH_STATUS::MH_OK {
        return Err(format!("enable latency dispatcher observer: {enabled:?}"));
    }
    INSTALLED.store(true, Ordering::Release);
    Ok(())
}

pub(crate) fn status() -> Value {
    let context = client_context();
    let gate = GATE.lock().ok();
    json!({"installed":INSTALLED.load(Ordering::Acquire),"dispatcherRva":DISPATCH,
        "calls":CALLS.load(Ordering::Relaxed),"forwarded":FORWARDED.load(Ordering::Relaxed),
        "observed":OBSERVED.load(Ordering::Relaxed),"primaryThreadReturns":MAIN_THREAD.load(Ordering::Relaxed),
        "rejected":REJECTED.load(Ordering::Relaxed),"recent":HISTORY.try_lock().ok().map(|h|json!(*h)),
        "session":gate.as_ref().map(|g|g.session.clone()),"sequence":gate.as_ref().map(|g|g.sequence),
        "usedMarkers":gate.as_ref().map(|g|g.used.as_ref().map_or(0,|used|used.len())),"last":gate.as_ref().and_then(|g|g.last.clone()),
        "contextLive":context.is_some_and(|c| gate.as_ref().is_some_and(|g|
            g.session == format!("native_{}_{}",std::process::id(),c.epoch))),
        "pending":gate.as_ref().and_then(|g|g.pending.as_ref().map(|p|json!({"request":p.request,
            "dispatchReturned":p.dispatch.is_some(),"observedAfter":p.after}))),
        "requestRejected":gate.as_ref().and_then(|g|g.rejected.clone()),
        "packetApplicationVerified":false,"terrainReady":false,"deterministicVideoReady":false})
}
