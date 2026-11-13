//! Exact-image Timer::advanceTime and opt-in native stepping diagnostics.
//! Client stepping is not yet deterministic integrated-server/replay playback.
use crate::native_scene_layout as scene;
use crate::world_hooks::read;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};

type Advance = unsafe extern "system" fn(usize, f32);
type AdvanceWithSingleTick = unsafe extern "system" fn(usize, f32, u8);
#[derive(Clone, Copy)]
enum AdvanceCall {
    Legacy(Advance),
    SingleTick {
        function: AdvanceWithSingleTick,
        flag: u8,
    },
}
impl AdvanceCall {
    /// Preserve the incoming native flag on every call, including scoped rate
    /// changes and both calls used by the explicit stepping branch. It is not
    /// a request from Arctic to override the engine's normal tick policy.
    unsafe fn invoke(self, timer: usize, preferred: f32) {
        match self {
            Self::Legacy(function) => unsafe { function(timer, preferred) },
            Self::SingleTick { function, flag } => unsafe { function(timer, preferred, flag) },
        }
    }
}
static INSTALLED_BINDING: OnceLock<crate::world_hooks::TimerBinding> = OnceLock::new();
static ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static CALLS: AtomicU64 = AtomicU64::new(0);
static MATCHED: AtomicU64 = AtomicU64::new(0);
static REAL_MATCHED: AtomicU64 = AtomicU64::new(0);
static ROOT: OnceLock<PathBuf> = OnceLock::new();
static PROBE_ENABLED: AtomicBool = AtomicBool::new(false);
static QUEUED: Mutex<Option<(Request, Instant)>> = Mutex::new(None);
static REJECTED: Mutex<Option<Value>> = Mutex::new(None);
static REPORTED: Mutex<Value> = Mutex::new(Value::Null);
static STATE: Mutex<State> = Mutex::new(State::new());

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
struct Sample {
    ticks_per_second: f32,
    time_scale: f32,
    ticks: i32,
    alpha: f32,
    passed_time: f32,
    last_timestep: f32,
    stepping_tick: f32,
}
fn sample(timer: usize) -> Option<Sample> {
    let mut bytes = [0u8; 64];
    read(timer, &mut bytes).ok()?;
    let f = |at| f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
    let s = Sample {
        ticks_per_second: f(0),
        time_scale: f(12),
        ticks: i32::from_le_bytes(bytes[4..8].try_into().unwrap()),
        alpha: f(8),
        passed_time: f(16),
        last_timestep: f(24),
        stepping_tick: f(60),
    };
    ((1. ..=1000.).contains(&s.ticks_per_second)
        && (0..=10).contains(&s.ticks)
        && (0. ..=1.).contains(&s.alpha)
        && s.passed_time.is_finite()
        && s.last_timestep.is_finite()
        && s.stepping_tick.is_finite())
    .then_some(s)
}
struct Binding {
    level: usize,
    timer: usize,
    real_timer: Option<usize>,
    game: usize,
    level_renderer: usize,
    game_renderer: usize,
    session: String,
    at: Instant,
}
struct State {
    binding: Option<Binding>,
    last_sequence: u64,
    frozen: bool,
    playback_rate: Option<f32>,
    rate_pending_sim: bool,
    rate_pending_real: bool,
    rate_restoration_failed: bool,
    freeze_sequence: u64,
    freeze_real: bool,
    pending_real_step: bool,
    pending_ack: Option<Value>,
    pending_since: Option<Instant>,
    pending_world_tick: Option<(usize, u64)>,
    expected_world_tick: Option<(usize, u64)>,
    waiting_for_real: bool,
    lease: Option<Instant>,
    last: Option<Value>,
    partial: f32,
    before: Option<Sample>,
    after: Option<Sample>,
    calculated_steps: u64,
    calculated_real_steps: u64,
    real_before: Option<Sample>,
    real_after: Option<Sample>,
    error: Option<String>,
}
impl State {
    const fn new() -> Self {
        Self {
            binding: None,
            last_sequence: 0,
            frozen: false,
            playback_rate: None,
            rate_pending_sim: false,
            rate_pending_real: false,
            rate_restoration_failed: false,
            freeze_sequence: 0,
            freeze_real: false,
            pending_real_step: false,
            pending_ack: None,
            pending_since: None,
            pending_world_tick: None,
            expected_world_tick: None,
            waiting_for_real: false,
            lease: None,
            last: None,
            partial: 0.,
            before: None,
            after: None,
            calculated_steps: 0,
            calculated_real_steps: 0,
            real_before: None,
            real_after: None,
            error: None,
        }
    }
}
pub(crate) fn configure(root: &Path, isolated: bool) {
    let _ = ROOT.set(root.to_path_buf());
    PROBE_ENABLED.store(isolated && crate::native_body::enabled(), Ordering::Release);
}
pub(crate) fn probe_enabled() -> bool {
    PROBE_ENABLED.load(Ordering::Acquire)
}
pub(crate) fn control_enabled() -> bool {
    ROOT.get().is_some()
}
pub(crate) fn frozen_for_probe() -> bool {
    ROOT.get().is_some() && STATE.try_lock().is_ok_and(|state| state.frozen)
}
pub(crate) fn owned_player_renderer(game_renderer: usize) -> Option<usize> {
    ROOT.get()?;
    let state = STATE.lock().ok()?;
    if !state.frozen {
        return None;
    }
    let binding = state
        .binding
        .as_ref()
        .filter(|b| b.game_renderer == game_renderer && b.live())?;
    crate::world_hooks::pointer(
        binding
            .level_renderer
            .checked_add(scene::level_renderer::PLAYER_OWNER)?,
    )
}
pub(crate) fn bind(
    level: usize,
    timer: usize,
    epoch: u64,
    game: usize,
    level_renderer: usize,
    game_renderer: usize,
    real_timer: Option<usize>,
) {
    if let Ok(mut state) = STATE.lock() {
        let session = format!("native_{}_{}", std::process::id(), epoch);
        if !state.binding.as_ref().is_some_and(|b| {
            b.level == level
                && b.timer == timer
                && b.session == session
                && b.game == game
                && b.level_renderer == level_renderer
                && b.game_renderer == game_renderer
                && b.real_timer == real_timer
        }) {
            state.frozen = false;
            state.playback_rate = None;
            state.rate_pending_sim = false;
            state.rate_pending_real = false;
            state.rate_restoration_failed = false;
            state.freeze_real = false;
            state.pending_real_step = false;
            state.pending_ack = None;
            state.pending_since = None;
            state.pending_world_tick = None;
            state.expected_world_tick = None;
            state.waiting_for_real = false;
            state.lease = None;
            state.last = None;
            state.last_sequence = 0;
            state.freeze_sequence = 0;
            state.calculated_steps = 0;
            state.calculated_real_steps = 0;
            state.real_before = None;
            state.real_after = None;
            state.partial = 0.;
            state.before = None;
            state.after = None;
            state.error = None;
        }
        state.binding = Some(Binding {
            level,
            timer,
            real_timer,
            game,
            level_renderer,
            game_renderer,
            session,
            at: Instant::now(),
        });
    }
}
impl Binding {
    fn live(&self) -> bool {
        self.at.elapsed() < Duration::from_secs(1)
            && self
                .game
                .checked_add(scene::game::RENDERER)
                .and_then(crate::world_hooks::pointer)
                == Some(self.game_renderer)
            && crate::live_client::read_client(self.game, self.level_renderer).is_some_and(|c| {
                c.level == self.level
                    && c.sim_timer == self.timer
                    && c.real_timer == self.real_timer
            })
    }
}
pub(crate) fn render_sample(renderer: usize) -> Option<(String, f64, f32)> {
    render_control(renderer).map(|(s, t, p, _)| (s, t, p))
}
/// Identity of a held, completed native clock sample, copied while collecting
/// a world pass. Neither a queued operation nor a calculated tick qualifies.
pub(crate) fn capture_sample(session: &str, tick: u64, partial: f32) -> Option<Value> {
    ROOT.get()?;
    let state = STATE.try_lock().ok()?;
    let binding = state
        .binding
        .as_ref()
        .filter(|b| b.session == session && b.live())?;
    if !state.frozen
        || state.error.is_some()
        || state.pending_ack.is_some()
        || state.pending_world_tick.is_some()
        || state.waiting_for_real
        || state.pending_real_step
        || !state.lease.is_some_and(|until| until > Instant::now())
        || state.expected_world_tick != Some((binding.level, tick))
        || state.partial.to_bits() != partial.to_bits()
        || crate::world_hooks::client_completed_tick(binding.level) != Some(tick)
        || QUEUED.try_lock().ok()?.is_some()
    {
        return None;
    }
    let timer = sample(binding.timer)?;
    if timer.alpha.to_bits() != partial.to_bits() {
        return None;
    }
    // The current interpolation rate must still describe the last actual
    // completed tick; an in-place TPS change is not the held export sample.
    crate::world_hooks::client_sample_seconds(binding.level, partial, timer.ticks_per_second)?;
    let real = state.freeze_real
        && binding.real_timer.and_then(sample).is_some_and(|r| {
            r.alpha.to_bits() == partial.to_bits()
                && r.ticks_per_second.to_bits() == timer.ticks_per_second.to_bits()
        });
    if state.freeze_real && !real {
        return None;
    }
    let receipt = state.last.as_ref()?;
    if receipt["sequence"].as_u64() != Some(state.last_sequence) || !receipt["error"].is_null() {
        return None;
    }
    let request_id = receipt["requestId"].as_str()?;
    Some(
        json!({"sequence":state.last_sequence,"requestId":request_id,
        "freezeSequence":state.freeze_sequence,"ticks":tick,"partial":partial,
        "ticksPerSecond":timer.ticks_per_second,"realTimerControlled":real}),
    )
}
/// A fully acknowledged held sample for this actual primary renderer. Drop
/// the binding lock before the normal capture admission checks take it again.
pub(crate) fn capture_renderer(renderer: usize) -> Option<Value> {
    let (session, tick, partial) = {
        let state = STATE.try_lock().ok()?;
        let binding = state
            .binding
            .as_ref()
            .filter(|b| b.game_renderer == renderer && b.live())?;
        (
            binding.session.clone(),
            crate::world_hooks::client_completed_tick(binding.level)?,
            state.partial,
        )
    };
    capture_sample(&session, tick, partial)
}
pub(crate) fn render_control(renderer: usize) -> Option<(String, f64, f32, u64)> {
    ROOT.get()?;
    let state = STATE.lock().ok()?;
    if !state.frozen {
        return None;
    }
    let binding = state
        .binding
        .as_ref()
        .filter(|b| b.game_renderer == renderer && b.live())?;
    let timer = sample(binding.timer)?;
    Some((
        binding.session.clone(),
        crate::world_hooks::client_sample_seconds(
            binding.level,
            timer.alpha,
            timer.ticks_per_second,
        )?,
        timer.alpha,
        state.freeze_sequence,
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    schema: u32,
    sequence: u64,
    session: String,
    op: String,
    #[serde(default)]
    partial: f32,
    #[serde(default, rename = "realTimer")]
    real_timer: bool,
    #[serde(default, rename = "requestId")]
    request_id: Option<String>,
    #[serde(default, rename = "leaseMs")]
    lease_ms: Option<u32>,
    #[serde(default)]
    rate: Option<f32>,
    #[serde(default, rename = "expectedClock")]
    expected_clock: Option<ExpectedClock>,
}
#[derive(Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExpectedClock {
    session: String,
    sequence: u64,
    request_id: Option<String>,
    freeze_sequence: u64,
    frozen: bool,
    playback_rate: Option<f32>,
}
impl ExpectedClock {
    fn idle(&self) -> bool {
        !self.frozen && self.playback_rate.is_none()
    }
    fn allows(&self, op: &str) -> bool {
        if self.idle() {
            matches!(op, "freeze" | "rate")
        } else {
            op != "freeze"
        }
    }
    fn valid(&self) -> bool {
        self.sequence <= 1_000_000_000
            && self.freeze_sequence <= self.sequence
            && (!self.frozen || self.freeze_sequence > 0)
            && !(self.frozen && self.playback_rate.is_some())
            && self
                .playback_rate
                .is_none_or(|rate| rate.is_finite() && (0. ..=16.).contains(&rate))
            && (if self.sequence == 0 {
                self.idle() && self.request_id.is_none()
            } else {
                self.idle() || self.request_id.is_some()
            })
            && self.request_id.as_ref().is_none_or(|id| {
                !id.is_empty()
                    && id.len() <= 64
                    && id
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
            })
    }
}
fn expected_clock(request: &Request, state: &State, session: &str) -> Result<(), String> {
    let Some(expected) = request.expected_clock.as_ref() else {
        return Ok(());
    };
    let acknowledged = if expected.sequence == 0 {
        state.last.is_none()
    } else {
        state.last.as_ref().is_some_and(|last| {
            last["sequence"].as_u64() == Some(expected.sequence)
                && match expected.request_id.as_deref() {
                    Some(id) => last["requestId"].as_str() == Some(id),
                    None => last.get("requestId") == Some(&Value::Null),
                }
        })
    };
    let lease = if expected.idle() {
        state.lease.is_none()
    } else {
        state.lease.is_some_and(|until| until > Instant::now())
    };
    if !expected.valid()
        || !expected.allows(&request.op)
        || expected.session != session
        || expected.session != request.session
        || expected.sequence != state.last_sequence
        || expected.freeze_sequence != state.freeze_sequence
        || !acknowledged
        || expected.frozen != state.frozen
        || expected.playback_rate.map(f32::to_bits) != state.playback_rate.map(f32::to_bits)
        || !lease
        || state.pending_ack.is_some()
        || state.pending_world_tick.is_some()
        || state.waiting_for_real
        || state.pending_real_step
        || state.rate_pending_sim
        || state.rate_pending_real
    {
        return Err("native clock state changed before its guarded transition; current controls were left unchanged".into());
    }
    Ok(())
}
pub(crate) fn timer_sample(timer: usize) -> Option<Value> {
    sample(timer).map(|v| json!(v))
}
fn validate(request: &Request, session: &str, frozen: bool) -> Result<(), String> {
    if request.schema != 1
        || !(1..=1_000_000_000).contains(&request.sequence)
        || request.session != session
        || !request.partial.is_finite()
        || !(0. ..1.).contains(&request.partial)
        || !matches!(
            request.op.as_str(),
            "freeze" | "step" | "sample" | "resume" | "renew" | "rate"
        )
        || (request.op == "rate") != request.rate.is_some()
        || request
            .rate
            .is_some_and(|rate| !rate.is_finite() || !(0. ..=16.).contains(&rate))
        || request.op == "rate" && frozen
        || request.op == "rate" && request.partial != 0.
        || matches!(request.op.as_str(), "step" | "sample" | "renew") && !frozen
        || request
            .lease_ms
            .is_some_and(|ms| !(500..=30_000).contains(&ms))
        || request.request_id.as_ref().is_some_and(|id| {
            id.is_empty()
                || id.len() > 64
                || !id
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        })
        || request.expected_clock.as_ref().is_some_and(|expected| {
            !expected.allows(&request.op)
                || !expected.valid()
                || expected.session != request.session
        })
    {
        return Err("invalid native clock command/session/state".into());
    }
    Ok(())
}
fn bounded_read(path: &Path) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|file| file.take(4097).read_to_end(&mut bytes))
        .map_err(|e| e.to_string())?;
    if bytes.len() > 4096 {
        return Err("native clock command exceeds 4 KiB".into());
    }
    Ok(bytes)
}
pub(crate) fn command(bytes: &[u8]) -> Result<(), String> {
    if bytes.is_empty() || bytes.len() > 4096 {
        return Err("native clock command exceeds 4 KiB or is empty".into());
    }
    let request: Request = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    let root = ROOT.get().ok_or("native clock control is unavailable")?;
    if ORIGINAL.load(Ordering::Acquire) == 0 {
        return Err("native timer hook is unavailable".into());
    }
    let mut state = STATE.lock().map_err(|e| e.to_string())?;
    expire(root, &mut state);
    if state.rate_restoration_failed {
        return Err("native rate restoration failed; a fresh world session is required".into());
    }
    let binding = state
        .binding
        .as_ref()
        .filter(|b| b.live())
        .ok_or("no fresh owned client timer")?;
    validate(&request, &binding.session, state.frozen)?;
    expected_clock(&request, &state, &binding.session)?;
    if request.sequence <= state.last_sequence {
        return Err("native clock sequence must increase".into());
    }
    if request.op != "resume" && request.lease_ms.is_none() {
        return Err("native clock control needs an expiring leaseMs (500–30000)".into());
    }
    if state.pending_ack.is_some() {
        return Err("wait for the pending native clock command".into());
    }
    let mut queued = QUEUED.lock().map_err(|e| e.to_string())?;
    if queued.is_some() {
        return Err("a native clock command is already queued".into());
    }
    *queued = Some((request, Instant::now()));
    if let Ok(mut rejected) = REJECTED.lock() {
        *rejected = None;
    }
    Ok(())
}

/// Queue schema-1 clock JSON. Admission is not a completed world tick.
///
/// # Safety
/// `bytes` must address `length` readable bytes for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn arctic_bedrock_clock_v1(bytes: *const u8, length: usize) -> i32 {
    if bytes.is_null() || length == 0 || length > 4096 {
        return -1;
    }
    std::panic::catch_unwind(|| {
        let bytes = unsafe { std::slice::from_raw_parts(bytes, length) };
        command(bytes).map_or(-1, |_| 0)
    })
    .unwrap_or(-1)
}

fn reject(request: &Request, error: &str) {
    if let Ok(mut rejected) = REJECTED.lock() {
        *rejected = Some(
            json!({"schema":1,"session":request.session,"sequence":request.sequence,
            "requestId":request.request_id,"error":error}),
        );
    }
}
fn release(state: &mut State) {
    state.frozen = false;
    state.playback_rate = None;
    state.rate_pending_sim = false;
    state.rate_pending_real = false;
    state.freeze_real = false;
    state.pending_real_step = false;
    state.pending_world_tick = None;
    state.expected_world_tick = None;
    state.waiting_for_real = false;
    state.lease = None;
}
fn cancel(root: &Path, state: &mut State, error: &str) {
    release(state);
    state.error = Some(error.into());
    let mut response = state
        .pending_ack
        .take()
        .or_else(|| state.last.clone())
        .unwrap_or_else(|| {
            json!({"schema":1,"sequence":state.last_sequence,
            "session":state.binding.as_ref().map(|b|&b.session)})
        });
    response["error"] = json!(error);
    response["worldTickComplete"] = json!(false);
    respond(root, state, response);
}
fn expire(root: &Path, state: &mut State) {
    if state.lease.is_some_and(|until| Instant::now() >= until) {
        cancel(
            root,
            state,
            "native clock lease expired; simulation control released",
        );
    } else if state
        .pending_since
        .is_some_and(|at| at.elapsed() > Duration::from_secs(2))
    {
        cancel(
            root,
            state,
            "native clock command did not complete its timer/world tick; control released",
        );
    } else if (state.frozen || state.playback_rate.is_some())
        && !state.binding.as_ref().is_some_and(|b| b.live())
    {
        cancel(
            root,
            state,
            "native clock ownership changed; simulation control released",
        );
    } else if state.frozen
        && state.expected_world_tick.is_some_and(|(level, wanted)| {
            crate::world_hooks::client_completed_tick(level).is_some_and(|tick| tick > wanted)
        })
    {
        cancel(
            root,
            state,
            "native world advanced beyond the held clock sample; control released",
        );
    }
}
pub(crate) fn service(root: &Path) {
    if let Ok(mut state) = STATE.lock() {
        expire(root, &mut state);
    }
    let pid = std::process::id();
    let request = root.join(format!("arctic-bedrock-{pid}.clock-request.json"));
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
                    json!({"schema":1,"session":identity["session"],"sequence":identity["sequence"],
                    "requestId":identity["requestId"],"error":error}),
                );
            }
        }
    }
    if let Ok(mut queued) = QUEUED.lock()
        && queued
            .as_ref()
            .is_some_and(|(_, at)| at.elapsed() > Duration::from_secs(2))
        && let Some((request, _)) = queued.take()
    {
        reject(
            &request,
            "native clock request expired before its timer consumer",
        );
    }
    let next = status();
    if let Ok(mut previous) = REPORTED.lock()
        && *previous != next
    {
        let report = root.join(format!("arctic-bedrock-{pid}.clock.json"));
        let temporary = report.with_extension("json.tmp");
        if std::fs::write(&temporary, next.to_string()).is_ok()
            && std::fs::rename(temporary, report).is_ok()
        {
            *previous = next;
        }
    }
}

fn next_request(root: &Path, state: &State) -> Option<Request> {
    if state.pending_ack.is_some() {
        return None;
    }
    if let Some((request, at)) = QUEUED.lock().ok()?.take() {
        if at.elapsed() > Duration::from_secs(2) {
            reject(
                &request,
                "native clock request expired before its timer consumer",
            );
            return None;
        }
        return Some(request);
    }
    if !probe_enabled() {
        return None;
    }
    bounded_read(&root.join("probe-clock-control.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Request>(&bytes).ok())
        .filter(|r| r.sequence > state.last_sequence)
}
fn write(address: usize, bytes: &[u8]) -> bool {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut core::ffi::c_void;
        fn WriteProcessMemory(
            process: *mut core::ffi::c_void,
            address: *mut core::ffi::c_void,
            buffer: *const u8,
            size: usize,
            written: *mut usize,
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
struct Restore {
    address: usize,
    bytes: [u8; 4],
}
impl Drop for Restore {
    fn drop(&mut self) {
        write(self.address, &self.bytes);
    }
}
fn override_step(
    timer: usize,
    original: AdvanceCall,
    preferred: f32,
    partial: f32,
    step: bool,
) -> bool {
    let Some(before) = sample(timer) else {
        return false;
    };
    if before.stepping_tick >= 0. || !partial.is_finite() || !(0. ..1.).contains(&partial) {
        return false;
    }
    let restore = Restore {
        address: timer + 0x3c,
        bytes: before.stepping_tick.to_le_bytes(),
    };
    if !write(restore.address, &partial.to_le_bytes()) {
        return false;
    }
    // The engine's sub-tick stepping branch sets ticks=0, alpha/passedTime to
    // the requested fraction and lastTimestep=0, without consulting wall time.
    unsafe { original.invoke(timer, preferred) };
    if step {
        if !write(restore.address, &(1. + partial).to_le_bytes()) {
            return false;
        }
        // Its >=1 branch emits exactly one tick, retaining the sampled alpha.
        unsafe { original.invoke(timer, preferred) };
    }
    let valid = sample(timer).is_some_and(|s| {
        s.ticks == i32::from(step)
            && s.alpha.to_bits() == partial.to_bits()
            && s.passed_time.to_bits() == partial.to_bits()
            && s.last_timestep == 0.
    });
    drop(restore);
    valid
        && sample(timer)
            .is_some_and(|s| s.stepping_tick.to_bits() == before.stepping_tick.to_bits())
}
fn acknowledge(root: &Path, value: Value) {
    if !probe_enabled() {
        return;
    }
    let temp = root.join("probe-clock-control-result.json.tmp");
    if std::fs::write(&temp, value.to_string()).is_ok() {
        let _ = std::fs::rename(temp, root.join("probe-clock-control-result.json"));
    }
}
fn respond(root: &Path, state: &mut State, mut response: Value) {
    response["frozen"] = json!(state.frozen);
    response["freezeSequence"] = json!(state.freeze_sequence);
    response["timer"] = json!(state.after);
    response["realTimer"] = json!(state.real_after);
    response["realTimerControlled"] =
        json!((state.frozen || state.playback_rate.is_some()) && state.freeze_real);
    response["playbackRate"] = json!(state.playback_rate);
    response["rateApplied"] =
        json!(state.playback_rate.is_some() && !state.rate_pending_sim && !state.rate_pending_real);
    response["rateRestorationFailed"] = json!(state.rate_restoration_failed);
    response["calculatedSteps"] = json!(state.calculated_steps);
    response["calculatedRealSteps"] = json!(state.calculated_real_steps);
    response["nativeSimulationVerified"] = json!(false);
    response["leaseActive"] = json!(state.lease.is_some());
    response["heldTick"] = json!(state.expected_world_tick.map(|(_, tick)| tick));
    response["partial"] = json!(state.partial);
    state.last = Some(response.clone());
    state.pending_since = None;
    acknowledge(root, response);
}
fn finish_pending(root: &Path, state: &mut State) {
    if state.waiting_for_real
        || state.pending_world_tick.is_some()
        || state.rate_pending_sim
        || state.rate_pending_real
    {
        return;
    }
    if let Some(mut response) = state.pending_ack.take() {
        if state.error.is_some() {
            response["error"] = json!(state.error);
        }
        respond(root, state, response);
    }
}
/// Called only after the primary level's real tick and cape simulation finish.
/// Packet catch-up and calculated timer output do not satisfy this fence.
pub(crate) fn completed_tick(level: usize, tick: u64) {
    let Some(root) = ROOT.get() else {
        return;
    };
    let Ok(mut state) = STATE.lock() else {
        return;
    };
    expire(root, &mut state);
    if state.frozen
        && state
            .expected_world_tick
            .is_some_and(|(expected_level, expected_tick)| {
                level == expected_level && tick > expected_tick
            })
    {
        cancel(
            root,
            &mut state,
            "native world advanced without a requested clock step",
        );
        return;
    }
    let Some((expected_level, expected_tick)) = state.pending_world_tick else {
        return;
    };
    if level != expected_level || tick < expected_tick {
        return;
    }
    if tick != expected_tick {
        cancel(
            root,
            &mut state,
            "native world advanced beyond the requested single tick",
        );
        return;
    }
    state.pending_world_tick = None;
    if let Some(response) = state.pending_ack.as_mut() {
        response["completedTick"] = json!(tick);
        response["worldTickComplete"] = json!(true);
    }
    finish_pending(root, &mut state);
}
fn control(
    timer: usize,
    original: AdvanceCall,
    preferred: f32,
    rate_applied: Option<RateApplication>,
) {
    let Some(root) = ROOT.get() else { return };
    let Ok(mut state) = STATE.lock() else { return };
    expire(root, &mut state);
    if state.binding.as_ref().is_some_and(|b| {
        (b.timer == timer || b.real_timer == Some(timer))
            && b.at.elapsed() >= Duration::from_secs(1)
    }) {
        cancel(
            root,
            &mut state,
            "native timer binding expired; stepping released",
        );
        return;
    }
    let Some((is_sim, real_alias, has_real, session)) = state
        .binding
        .as_ref()
        .filter(|b| (b.timer == timer || b.real_timer == Some(timer)) && b.live())
        .map(|b| {
            (
                b.timer == timer,
                b.real_timer == Some(b.timer),
                b.real_timer.is_some(),
                b.session.clone(),
            )
        })
    else {
        return;
    };
    // A command is acknowledged only after its selected original timer calls
    // used the multiplier and restored the exact prior native field. The tag
    // prevents an old call from acknowledging a newer command/world session.
    if let Some(applied) = rate_applied
        && applied.sequence == state.last_sequence
        && applied.session == session
        && state
            .playback_rate
            .is_some_and(|rate| rate.to_bits() == applied.rate.to_bits())
    {
        if is_sim {
            state.rate_pending_sim = false;
        }
        if !is_sim || real_alias {
            state.rate_pending_real = false;
        }
    }
    if !is_sim {
        REAL_MATCHED.fetch_add(1, Ordering::Relaxed);
        state.real_before = sample(timer);
        if state.frozen && state.freeze_real {
            let step = state.pending_real_step;
            if override_step(timer, original, preferred, state.partial, step) {
                if step {
                    state.calculated_real_steps = state.calculated_real_steps.saturating_add(1);
                }
            } else {
                cancel(
                    root,
                    &mut state,
                    "native real-timer stepping outputs/ownership did not match",
                );
            }
            state.pending_real_step = false;
        }
        state.real_after = sample(timer);
        state.waiting_for_real = false;
        finish_pending(root, &mut state);
        return;
    }
    MATCHED.fetch_add(1, Ordering::Relaxed);
    state.before = sample(timer);
    let request = next_request(root, &state);
    let mut step = false;
    let mut response = None;
    let mut wait_real = false;
    if let Some(request) = request {
        let matching_rates = state
            .binding
            .as_ref()
            .and_then(|b| b.real_timer)
            .and_then(sample)
            .zip(state.before)
            .is_some_and(|(real, sim)| {
                real.ticks_per_second.to_bits() == sim.ticks_per_second.to_bits()
            });
        let current_tick = state.binding.as_ref().and_then(|b| {
            crate::world_hooks::client_completed_tick(b.level).map(|tick| (b.level, tick))
        });
        let completed =
            current_tick.and_then(|(level, tick)| tick.checked_add(1).map(|next| (level, next)));
        let validation = validate(&request, &session, state.frozen)
            .and_then(|()| expected_clock(&request, &state, &session))
            .and_then(|()| {
                if request.sequence <= state.last_sequence
                    || request.real_timer && !has_real
                    || request.real_timer && request.op != "resume" && !matching_rates
                    || matches!(request.op.as_str(), "step" | "sample" | "renew")
                        && request.real_timer != state.freeze_real
                    || request.op == "renew" && request.partial.to_bits() != state.partial.to_bits()
                    || request.op == "step" && completed.is_none()
                    || request.op != "resume" && current_tick.is_none()
                    || request.op == "freeze" && state.playback_rate.is_some()
                    || state.rate_restoration_failed
                {
                    Err("native real-timer binding or control mode differs".into())
                } else {
                    Ok(())
                }
            });
        if let Err(error) = validation {
            // Rejection is not this timer owner's acknowledgement. Preserve
            // its current last receipt, lease and pending state.
            reject(&request, &error);
        } else {
            state.error = None;
            state.last_sequence = request.sequence;
            if request.op == "freeze" {
                state.freeze_sequence = request.sequence;
            }
            if request.op != "renew" {
                state.playback_rate = request.rate;
                state.rate_pending_sim = request.op == "rate";
                state.rate_pending_real = request.op == "rate" && request.real_timer && !real_alias;
                state.frozen = !matches!(request.op.as_str(), "resume" | "rate");
                state.freeze_real =
                    (state.frozen || state.playback_rate.is_some()) && request.real_timer;
                state.partial = request.partial;
            }
            state.lease = if state.frozen || state.playback_rate.is_some() {
                request
                    .lease_ms
                    .map(|ms| Instant::now() + Duration::from_millis(u64::from(ms)))
            } else {
                None
            };
            step = request.op == "step";
            state.pending_world_tick = if step { completed } else { None };
            state.expected_world_tick = if step {
                completed
            } else if state.frozen {
                current_tick
            } else {
                None
            };
            state.pending_real_step = step && state.freeze_real && !real_alias;
            wait_real = request.op != "renew" && request.real_timer && !real_alias;
            response = Some(
                json!({"schema":1,"sequence":request.sequence,"session":session,"requestId":request.request_id,"op":request.op,"worldTickComplete":false}),
            );
            if let Some(expected) = request.expected_clock.as_ref() {
                response.as_mut().expect("accepted command receipt")["expectedClock"] =
                    json!(expected);
            }
        }
    }
    if state.frozen {
        if override_step(timer, original, preferred, state.partial, step) {
            if step {
                state.calculated_steps = state.calculated_steps.saturating_add(1);
            }
        } else {
            if state.pending_ack.is_some() {
                cancel(
                    root,
                    &mut state,
                    "native stepping outputs/ownership did not match",
                );
            } else {
                release(&mut state);
            }
            wait_real = false;
            state.error = Some("native stepping outputs/ownership did not match".into());
            if let Some(r) = response.as_mut() {
                r["error"] = json!(state.error);
            }
        }
    }
    state.after = sample(timer);
    if real_alias {
        REAL_MATCHED.fetch_add(1, Ordering::Relaxed);
        state.real_before = state.before;
        state.real_after = state.after;
        if step && state.freeze_real {
            state.calculated_real_steps = state.calculated_real_steps.saturating_add(1);
        }
    }
    if let Some(response) = response {
        if wait_real
            || state.pending_world_tick.is_some()
            || state.rate_pending_sim
            || state.rate_pending_real
        {
            state.waiting_for_real = wait_real;
            state.pending_ack = Some(response);
            state.pending_since = Some(Instant::now());
        } else {
            respond(root, &mut state, response);
        }
    }
    finish_pending(root, &mut state);
}
struct RateApplication {
    sequence: u64,
    session: String,
    rate: f32,
}
/// The pinned advanceTime reads timeScale at +0xc, multiplies seconds by it
/// and TPS, and never writes that field. This scoped override preserves the
/// engine's wall-clock bookkeeping and existing game multiplier. It does not
/// remove the engine's ten-tick cap or establish replay/terrain readiness.
fn advance_rate(timer: usize, original: AdvanceCall, preferred: f32) -> Option<RateApplication> {
    let selected = (|| {
        let state = STATE.lock().ok()?;
        if state.frozen
            || state.rate_restoration_failed
            || !state.lease.is_some_and(|until| until > Instant::now())
        {
            return None;
        }
        let rate = state.playback_rate?;
        let binding = state.binding.as_ref().filter(|binding| {
            binding.live()
                && (binding.timer == timer
                    || state.freeze_real && binding.real_timer == Some(timer))
        })?;
        Some(RateApplication {
            sequence: state.last_sequence,
            session: binding.session.clone(),
            rate,
        })
    })();
    let Some(selected) = selected else {
        unsafe { original.invoke(timer, preferred) };
        return None;
    };
    let mut restoration_failed = false;
    let attempt = (|| {
        let before = sample(timer)?;
        if !before.time_scale.is_finite()
            || !(0. ..=16.).contains(&before.time_scale)
            || before.stepping_tick >= 0.
        {
            return None;
        }
        let scaled = before.time_scale * selected.rate;
        let address = timer.checked_add(0xc)?;
        let restore = Restore {
            address,
            bytes: before.time_scale.to_le_bytes(),
        };
        if !write(address, &scaled.to_le_bytes()) {
            restoration_failed = !restore_rate(restore);
            return None;
        }
        Some((restore, scaled))
    })();
    let Some((restore, scaled)) = attempt else {
        unsafe { original.invoke(timer, preferred) };
        rate_failure(
            &selected,
            "native playback multiplier could not be scoped to its timer",
            restoration_failed,
        );
        return None;
    };
    unsafe { original.invoke(timer, preferred) };
    let applied =
        sample(timer).is_some_and(|sample| sample.time_scale.to_bits() == scaled.to_bits());
    let restored = restore_rate(restore);
    if !applied || !restored {
        rate_failure(
            &selected,
            "native playback multiplier application/restoration did not match",
            !restored,
        );
        return None;
    }
    Some(selected)
}
fn restore_rate(restore: Restore) -> bool {
    let address = restore.address;
    let expected = restore.bytes;
    drop(restore);
    let mut actual = [0u8; 4];
    read(address, &mut actual).is_ok() && actual == expected
}
fn rate_failure(applied: &RateApplication, error: &str, restoration_failed: bool) {
    let Some(root) = ROOT.get() else {
        return;
    };
    let Ok(mut state) = STATE.lock() else {
        return;
    };
    if state.last_sequence == applied.sequence
        && state
            .binding
            .as_ref()
            .is_some_and(|binding| binding.session == applied.session)
    {
        state.rate_restoration_failed |= restoration_failed;
        cancel(root, &mut state, error);
    }
}
unsafe extern "system" fn advance(timer: usize, preferred: f32) {
    let original: Advance = unsafe { std::mem::transmute(ORIGINAL.load(Ordering::Acquire)) };
    advance_with(timer, preferred, AdvanceCall::Legacy(original));
}
unsafe extern "system" fn advance_single_tick(timer: usize, preferred: f32, flag: u8) {
    let function: AdvanceWithSingleTick =
        unsafe { std::mem::transmute(ORIGINAL.load(Ordering::Acquire)) };
    advance_with(timer, preferred, AdvanceCall::SingleTick { function, flag });
}
fn advance_with(timer: usize, preferred: f32, original: AdvanceCall) {
    CALLS.fetch_add(1, Ordering::Relaxed);
    // Always update the real native time bookkeeping first. Frozen frames do
    // not accumulate a wall-clock catch-up burst on resume.
    let rate_applied = advance_rate(timer, original, preferred);
    let _ = std::panic::catch_unwind(|| control(timer, original, preferred, rate_applied));
}
pub(crate) unsafe fn install() -> Result<(), String> {
    let base = crate::world_hooks::image_base().ok_or("timer image profile unavailable")?;
    let binding = crate::world_hooks::timer_binding()?;
    let target = base
        .checked_add(binding.rva)
        .ok_or("timer target address overflow")?;
    use crate::world_hooks::TimerAdvanceAbi;
    let (callback, prefix): (usize, &[u8]) = match binding.abi {
        TimerAdvanceAbi::TimerFloat => (
            advance as *const () as usize,
            &[
                0x56, 0x48, 0x83, 0xec, 0x30, 0x0f, 0x29, 0x74, 0x24, 0x20, 0x0f, 0x28, 0xf1, 0x48,
                0x89, 0xce,
            ],
        ),
        TimerAdvanceAbi::TimerFloatSingleTick => (
            advance_single_tick as *const () as usize,
            &[
                0x56, 0x53, 0x48, 0x83, 0xec, 0x38, 0x0f, 0x29, 0x74, 0x24, 0x20, 0x0f, 0x28, 0xf1,
                0x48, 0x89, 0xce,
            ],
        ),
    };
    let mut code = vec![0u8; prefix.len()];
    read(target, &mut code)?;
    if code.as_slice() != prefix {
        return Err("native timer prologue differs".into());
    }
    use hudhook::mh::{MH_CreateHook, MH_EnableHook, MH_STATUS};
    let mut original = std::ptr::null_mut();
    let result = unsafe { MH_CreateHook(target as _, callback as _, &mut original) };
    if result != MH_STATUS::MH_OK {
        return Err(format!("create timer hook: {result:?}"));
    }
    ORIGINAL.store(original as usize, Ordering::Release);
    let _ = INSTALLED_BINDING.set(binding);
    let result = unsafe { MH_EnableHook(target as _) };
    if result != MH_STATUS::MH_OK {
        return Err(format!("enable timer hook: {result:?}"));
    }
    Ok(())
}
pub(crate) fn status() -> Value {
    let state = STATE.lock().ok();
    json!({"installed":ORIGINAL.load(Ordering::Acquire)!=0,
        "hookRva":INSTALLED_BINDING.get().map(|binding|binding.rva),
        "advanceAbi":INSTALLED_BINDING.get().map(|binding|match binding.abi {
            crate::world_hooks::TimerAdvanceAbi::TimerFloat=>"timerFloat",
            crate::world_hooks::TimerAdvanceAbi::TimerFloatSingleTick=>"timerFloatSingleTick",
        }),"nativeSingleTickFlagForwardingImplemented":true,"nativeSingleTickFlagVerified":false,
        "calls":CALLS.load(Ordering::Relaxed),
        "matchedTimerCalls":MATCHED.load(Ordering::Relaxed),"probeEnabled":probe_enabled(),"controlEnabled":control_enabled(),
        "session":state.as_ref().and_then(|s|s.binding.as_ref().map(|b|b.session.clone())),"sequence":state.as_ref().map(|s|s.last_sequence),
        "bindingReady":state.as_ref().is_some_and(|s|s.binding.as_ref().is_some_and(|b|b.live())),
        "frozen":state.as_ref().is_some_and(|s|s.frozen),"freezeSequence":state.as_ref().map(|s|s.freeze_sequence),"before":state.as_ref().and_then(|s|s.before),
        "playbackRate":state.as_ref().and_then(|s|s.playback_rate),
        "ratePendingSim":state.as_ref().is_some_and(|s|s.rate_pending_sim),"ratePendingReal":state.as_ref().is_some_and(|s|s.rate_pending_real),
        "rateRestorationFailed":state.as_ref().is_some_and(|s|s.rate_restoration_failed),"nativePlaybackRateVerified":false,
        "after":state.as_ref().and_then(|s|s.after),"calculatedSteps":state.as_ref().map(|s|s.calculated_steps),
        "matchedRealTimerCalls":REAL_MATCHED.load(Ordering::Relaxed),"realBefore":state.as_ref().and_then(|s|s.real_before),
        "realAfter":state.as_ref().and_then(|s|s.real_after),"freezeRealTimer":state.as_ref().is_some_and(|s|s.freeze_real),
        "calculatedRealSteps":state.as_ref().map(|s|s.calculated_real_steps),"waitingForRealTimer":state.as_ref().is_some_and(|s|s.waiting_for_real),
        "pendingWorldTick":state.as_ref().and_then(|s|s.pending_world_tick.map(|(_, tick)|tick)),
        "expectedWorldTick":state.as_ref().and_then(|s|s.expected_world_tick.map(|(_, tick)|tick)),
        "leaseRemainingMs":state.as_ref().and_then(|s|s.lease.map(|until|until.saturating_duration_since(Instant::now()).as_millis())),
        "last":state.as_ref().and_then(|s|s.last.clone()),
        "queued":QUEUED.lock().ok().and_then(|q|q.as_ref().map(|(r,_)|json!({"session":r.session,"sequence":r.sequence,"requestId":r.request_id}))),
        "requestRejected":REJECTED.lock().ok().and_then(|r|r.clone()),
        "ownedReleaseGuardImplemented":true,
        "ownedFollowupGuardImplemented":true,
        "idleAcquisitionGuardImplemented":true,
        "error":state.as_ref().and_then(|s|s.error.clone()),"nativeSimulationVerified":false,"deterministicVideoReady":false})
}

#[cfg(test)]
mod tests {
    use super::*;
    unsafe extern "system" fn engine(timer: usize, _preferred: f32) {
        let timer = timer as *mut f32;
        let at = unsafe { *timer.add(15) };
        if at >= 1. {
            unsafe {
                *(timer.add(1) as *mut i32) = 1;
                *timer.add(15) = at - 1.;
            }
        } else if at >= 0. {
            unsafe {
                *(timer.add(1) as *mut i32) = 0;
                *timer.add(2) = at;
                *timer.add(4) = at;
                *timer.add(6) = 0.;
            }
        }
    }
    #[test]
    fn native_stepping_preserves_owner_and_sets_exact_tick_and_fraction() {
        let mut timer = [0f32; 32];
        timer[0] = 20.;
        timer[15] = -1.;
        let address = timer.as_mut_ptr() as usize;
        assert!(override_step(
            address,
            AdvanceCall::Legacy(engine),
            0.,
            0.25,
            false
        ));
        assert_eq!(sample(address).unwrap().ticks, 0);
        assert_eq!(timer[2], 0.25);
        assert_eq!(timer[15], -1.);
        assert!(override_step(
            address,
            AdvanceCall::Legacy(engine),
            0.,
            0.75,
            true
        ));
        assert_eq!(sample(address).unwrap().ticks, 1);
        assert_eq!(timer[2], 0.75);
        assert_eq!(timer[15], -1.);
        for partial in [1., -0.1, f32::NAN] {
            assert!(!override_step(
                address,
                AdvanceCall::Legacy(engine),
                0.,
                partial,
                true
            ));
        }
        timer[15] = 0.;
        std::hint::black_box(&timer);
        assert!(!override_step(
            address,
            AdvanceCall::Legacy(engine),
            0.,
            0.,
            true
        ));
    }
    #[test]
    fn control_rejects_wrong_session_unfrozen_steps_and_unknown_operations() {
        let mut request = Request {
            schema: 1,
            sequence: 1,
            session: "live".into(),
            op: "freeze".into(),
            partial: 0.25,
            real_timer: false,
            request_id: None,
            lease_ms: None,
            rate: None,
            expected_clock: None,
        };
        assert!(validate(&request, "live", false).is_ok());
        assert!(validate(&request, "old", false).is_err());
        request.op = "step".into();
        assert!(validate(&request, "live", false).is_err());
        assert!(validate(&request, "live", true).is_ok());
        request.op = "arbitrary".into();
        assert!(validate(&request, "live", true).is_err());
    }
    #[test]
    fn real_detour_preserves_float_argument_bits_and_void_abi() {
        static RECEIVED: AtomicUsize = AtomicUsize::new(0);
        #[inline(never)]
        unsafe extern "system" fn host(_timer: usize, preferred: f32) {
            RECEIVED.store(preferred.to_bits() as usize, Ordering::Release);
        }
        use hudhook::mh::{MH_CreateHook, MH_DisableHook, MH_EnableHook, MH_Initialize, MH_STATUS};
        assert!(matches!(
            unsafe { MH_Initialize() },
            MH_STATUS::MH_OK | MH_STATUS::MH_ERROR_ALREADY_INITIALIZED
        ));
        let target = host as *const () as usize;
        let mut trampoline = std::ptr::null_mut();
        assert_eq!(
            unsafe { MH_CreateHook(target as _, advance as *const () as _, &mut trampoline) },
            MH_STATUS::MH_OK
        );
        ORIGINAL.store(trampoline as usize, Ordering::Release);
        assert_eq!(unsafe { MH_EnableHook(target as _) }, MH_STATUS::MH_OK);
        let call: Advance = std::hint::black_box(host);
        let value = f32::from_bits(0x7fc12345);
        unsafe { call(1, value) };
        assert_eq!(unsafe { MH_DisableHook(target as _) }, MH_STATUS::MH_OK);
        ORIGINAL.store(target, Ordering::Release);
        assert_eq!(RECEIVED.load(Ordering::Acquire), value.to_bits() as usize);
    }
}
