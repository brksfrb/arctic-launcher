//! Serialized commands for Java's owned appearance archive/renderer state.
//! These never bypass native body/cape ownership or suppression gates.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    io::Read,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, Instant},
};

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Request {
    schema: u32,
    pub session: String,
    sequence: u64,
    request_id: String,
    pub op: String,
    pub path: Option<PathBuf>,
    pub time_ms: Option<u32>,
    pub owner: Option<String>,
    pub anchor: Option<crate::native_packet::HistoryAnchor>,
}
#[derive(Default)]
struct State {
    session: String,
    sequence: u64,
    queued: Option<(Request, Instant)>,
    executing: Option<Request>,
    last: Option<Value>,
    rejected: Option<Value>,
    history: Vec<Value>,
}
static STATE: Mutex<State> = Mutex::new(State {
    session: String::new(),
    sequence: 0,
    queued: None,
    executing: None,
    last: None,
    rejected: None,
    history: Vec::new(),
});
static REPORTED: Mutex<(Value, Option<Instant>)> = Mutex::new((Value::Null, None));
fn token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}
fn session() -> Option<String> {
    crate::world_hooks::image_base()?;
    let (_, epoch) = crate::players::session()?;
    Some(format!("native_{}_{}", std::process::id(), epoch))
}
fn receipt(request: &Request, stage: &str, error: Option<&str>) -> Value {
    json!({"schema":1,"session":request.session,"sequence":request.sequence,"requestId":request.request_id,"op":request.op,
        "stage":stage,"error":error,"owner":request.owner,"nativeWorldVerified":false,"deterministicVideoVerified":false})
}
fn terminal(state: &mut State, value: Value) {
    if state.history.len() >= 32 {
        state.history.remove(0);
    }
    state.history.push(value.clone());
    state.last = Some(value);
    while state.history.len() > 1
        && state
            .history
            .iter()
            .map(|value| value.to_string().len())
            .sum::<usize>()
            > 16 * 1024
    {
        state.history.remove(0);
    }
}
fn reconcile(state: &mut State) {
    if let Some(current) = session()
        && current != state.session
    {
        if let Some((request, _)) = state.queued.take() {
            terminal(
                state,
                receipt(
                    &request,
                    "failed",
                    Some("appearance command world changed before execution"),
                ),
            );
        }
        state.session = current;
        state.sequence = 0;
    }
    if state
        .queued
        .as_ref()
        .is_some_and(|(_, at)| at.elapsed() > Duration::from_secs(2))
        && let Some((request, _)) = state.queued.take()
    {
        terminal(
            state,
            receipt(
                &request,
                "failed",
                Some("appearance command expired before its client-thread consumer"),
            ),
        );
    }
}
fn admit(bytes: &[u8]) -> Result<(), String> {
    if bytes.is_empty() || bytes.len() > 16 * 1024 {
        return Err("appearance command exceeds 16 KiB or is empty".into());
    }
    let request: Request = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if request
        .anchor
        .as_ref()
        .is_some_and(|a| !a.valid() || request.op != "history_begin")
    {
        return Err("recording anchor requires a valid history begin operation".into());
    }
    if request.schema != 1
        || !token(&request.session)
        || !token(&request.request_id)
        || !(1..=1_000_000_000).contains(&request.sequence)
        || session().as_deref() != Some(request.session.as_str())
        || request.owner.as_ref().is_some_and(|owner| !token(owner))
    {
        return Err(
            "appearance command needs the current native world and increasing identity".into(),
        );
    }
    match request.op.as_str() {
        "history_begin" => {
            let path = request
                .path
                .as_deref()
                .ok_or("appearance history output is missing")?;
            if request.owner.is_none()
                || !request.time_ms.is_some_and(|time| time <= 86_400_000)
                || !path.is_absolute()
                || !path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().ends_with(".arctic-looks.zip"))
                || path.exists()
                || !path.parent().is_some_and(|p| p.is_dir())
            {
                return Err(
                    "appearance history needs an owner, bounded time and new absolute output"
                        .into(),
                );
            }
        }
        "history_finish" | "history_cancel"
            if request.owner.is_some() && request.path.is_none() && request.time_ms.is_none() => {}
        "save" | "load" => {
            let path = request
                .path
                .as_deref()
                .ok_or("appearance file is missing")?;
            if request.time_ms.is_some()
                || request.op == "save" && request.owner.is_some()
                || !path.is_absolute()
                || !path
                    .file_name()
                    .is_some_and(|p| p.to_string_lossy().ends_with(".arctic-looks.zip"))
                || request.op == "load"
                    && (!path.is_file()
                        || path.metadata().map_err(|e| e.to_string())?.len() > 258 * 1024 * 1024)
                || request.op == "save"
                    && (path.exists() || !path.parent().is_some_and(|p| p.is_dir()))
            {
                return Err(
                    "appearance needs an existing snapshot input or a new absolute snapshot output"
                        .into(),
                );
            }
        }
        "time" if request.path.is_none() && request.time_ms.is_some_and(|t| t <= 86_400_000) => {}
        "clear" if request.path.is_none() && request.time_ms.is_none() => {}
        "state"
            if request.path.is_none() && request.time_ms.is_none() && request.owner.is_none() => {}
        _ => return Err("invalid appearance operation or fields".into()),
    }
    let mut state = STATE.lock().map_err(|e| e.to_string())?;
    reconcile(&mut state);
    if state.session != request.session
        || request.sequence <= state.sequence
        || state.queued.is_some()
        || state.executing.is_some()
    {
        return Err("appearance control needs an increasing sequence and an idle consumer".into());
    }
    state.sequence = request.sequence;
    state.last = Some(receipt(&request, "queued", None));
    state.queued = Some((request, Instant::now()));
    Ok(())
}
/// Called on the serialized Java render/client thread, outside the state lock.
pub(crate) fn consume<F>(apply: F)
where
    F: FnOnce(&Request) -> Result<Value, String>,
{
    let request = {
        let Ok(mut state) = STATE.try_lock() else {
            return;
        };
        reconcile(&mut state);
        if state.executing.is_some() {
            return;
        }
        let Some((request, _)) = state.queued.take() else {
            return;
        };
        if session().as_deref() != Some(request.session.as_str()) {
            terminal(
                &mut state,
                receipt(
                    &request,
                    "failed",
                    Some("appearance consumer lost its original world"),
                ),
            );
            return;
        }
        state.executing = Some(request.clone());
        state.last = Some(receipt(&request, "executing", None));
        request
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| apply(&request)))
        .unwrap_or_else(|_| Err("appearance client-thread consumer panicked".into()));
    if let Ok(mut state) = STATE.lock() {
        state.executing = None;
        let result = if session().as_deref() != Some(request.session.as_str()) {
            Err("appearance world changed during execution".into())
        } else {
            result
        };
        let completed = match result {
            Ok(value) => {
                let mut completed = receipt(&request, "completed", None);
                completed["state"] = value;
                completed
            }
            Err(error) => receipt(&request, "failed", Some(&error)),
        };
        terminal(&mut state, completed);
    }
}
fn status() -> Value {
    let Ok(mut state) = STATE.lock() else {
        return json!({"error":"appearance state is busy"});
    };
    reconcile(&mut state);
    json!({"schema":1,"session":state.session,"sequence":state.sequence,"contextLive":session().as_deref()==Some(state.session.as_str()),
        "queued":state.queued.as_ref().map(|(r,_)|receipt(r,"queued",None)),"executing":state.executing.as_ref().map(|r|receipt(r,"executing",None)),
        "last":state.last,"requestRejected":state.rejected,"history":state.history,"nativeWorldVerified":false,"deterministicVideoVerified":false})
}
pub(crate) fn service(root: &Path) {
    let pid = std::process::id();
    let request = root.join(format!("arctic-bedrock-{pid}.appearance-request.json"));
    if request.is_file() {
        let mut bytes = Vec::new();
        let result = std::fs::File::open(&request)
            .and_then(|f| f.take(16 * 1024 + 1).read_to_end(&mut bytes))
            .map_err(|e| e.to_string())
            .and_then(|_| admit(&bytes));
        let _ = std::fs::remove_file(&request);
        if let Err(error) = result {
            let identity = serde_json::from_slice::<Value>(&bytes).unwrap_or(Value::Null);
            if let Ok(mut state) = STATE.lock() {
                state.rejected = Some(
                    json!({"session":identity["session"],"sequence":identity["sequence"],"requestId":identity["requestId"],"error":error}),
                );
            }
        }
    }
    let mut value = status();
    // A recording association appears in successful state receipts. Bound the
    // published history to the host's existing 64 KiB reader, including paths.
    while value.to_string().len() > 64 * 1024 {
        let Some(history) = value["history"].as_array_mut().filter(|h| !h.is_empty()) else {
            break;
        };
        history.remove(0);
    }
    if value.to_string().len() > 64 * 1024 {
        for path in [
            "/last/error",
            "/requestRejected/error",
            "/last/state/history/error",
            "/last/state/history/path",
            "/last/state/appearance/lastCapePhysicsError",
            "/last/state/appearance/lastWingSampleError",
            "/last/state/appearance/lastBodySampleError",
            "/last/state/appearance/lastAttachmentSampleError",
        ] {
            if let Some(field) = value.pointer_mut(path)
                && let Some(message) = field.as_str().filter(|s| s.len() > 2048)
            {
                let mut end = 2048;
                while !message.is_char_boundary(end) {
                    end -= 1;
                }
                *field = json!(format!("{}… [truncated in report]", &message[..end]));
            }
        }
    }
    if let Ok(mut previous) = REPORTED.lock()
        && (previous.0 != value
            || previous
                .1
                .is_none_or(|at| at.elapsed() >= Duration::from_millis(500)))
    {
        let report = root.join(format!("arctic-bedrock-{pid}.appearance.json"));
        let temporary = report.with_extension("json.tmp");
        if std::fs::write(&temporary, value.to_string()).is_ok()
            && std::fs::rename(temporary, report).is_ok()
        {
            *previous = (value, Some(Instant::now()));
        }
    }
}
/// # Safety
/// `bytes` must identify `length` readable bytes for this call. Admission is not
/// completion; the PID-scoped appearance report carries the client result.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn arctic_bedrock_appearance_v1(bytes: *const u8, length: usize) -> i32 {
    if bytes.is_null() || length == 0 || length > 16 * 1024 {
        return -1;
    }
    let mut owned = vec![0; length];
    if crate::world_hooks::read(bytes as usize, &mut owned).is_err() {
        return -1;
    }
    match admit(&owned) {
        Ok(()) => 0,
        Err(_) => -2,
    }
}
