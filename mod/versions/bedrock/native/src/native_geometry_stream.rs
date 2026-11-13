//! Serialized client geometry -> owned asynchronous upload. Completion is only
//! a source of geometry for an exact world scope; it is not a native draw lease.
use crate::{
    avatar_geometry::Frame,
    avatar_gpu_upload::{self, GlintArtwork, Uploaded},
    world_hooks::GeometryInput,
};
use hudhook::windows::Win32::Graphics::Direct3D12::{ID3D12CommandQueue, ID3D12Device};
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicU64, Ordering},
};

struct Completed {
    input: GeometryInput,
    uploaded: Arc<Uploaded>,
}
struct Pending {
    input: GeometryInput,
    ticket: avatar_gpu_upload::Ticket,
}
static READY: OnceLock<Mutex<Option<Arc<Completed>>>> = OnceLock::new();
static REASON: Mutex<Option<String>> = Mutex::new(None);
static GENERATED: AtomicU64 = AtomicU64::new(0);
static STARTED: AtomicU64 = AtomicU64::new(0);
static COMPLETED: AtomicU64 = AtomicU64::new(0);
static STALE: AtomicU64 = AtomicU64::new(0);

fn ready() -> &'static Mutex<Option<Arc<Completed>>> {
    READY.get_or_init(Mutex::default)
}
fn replace(value: Option<Arc<Completed>>) {
    // Dropping GPU/COM owners is outside the publication mutex. Existing draw
    // owners keep their own Arc until their independently observed native fence.
    let old = ready()
        .lock()
        .ok()
        .and_then(|mut slot| std::mem::replace(&mut *slot, value));
    drop(old);
}
fn reason(value: Option<&str>) {
    if let Ok(mut slot) = REASON.lock() {
        *slot = value.map(str::to_owned);
    }
}

#[derive(Default)]
pub(crate) struct Stream {
    pending: Option<Pending>,
}
impl Stream {
    pub(crate) fn reset(&mut self) {
        // Ticket abandonment delegates retirement to the bounded upload pool;
        // it never releases submitted buffers before their upload fence.
        self.pending = None;
        replace(None);
        reason(None);
    }
    pub(crate) fn available(&self) -> bool {
        self.pending.is_none() && crate::native_body::enabled()
    }
    pub(crate) fn reject(&mut self, error: &str) {
        replace(None);
        reason(Some(error));
    }
    pub(crate) fn poll(&mut self) -> Result<(), String> {
        if !crate::native_body::enabled() {
            self.reset();
            return Ok(());
        }
        let stale = ready()
            .try_lock()
            .ok()
            .and_then(|slot| slot.clone())
            .is_some_and(|completed| !completed.input.current());
        if stale {
            replace(None);
        }
        let Some(pending) = self.pending.as_mut() else {
            return Ok(());
        };
        match pending.ticket.take_completed() {
            Ok(None) => Ok(()),
            Ok(Some(uploaded)) => {
                let pending = self.pending.take().expect("upload was pending");
                if !pending.input.current() || !pending.input.matches(uploaded.frame()) {
                    STALE.fetch_add(1, Ordering::Relaxed);
                    reason(Some(
                        "upload completed after its exact producer frame expired",
                    ));
                    return Ok(());
                }
                replace(Some(Arc::new(Completed {
                    input: pending.input,
                    uploaded: Arc::new(uploaded),
                })));
                COMPLETED.fetch_add(1, Ordering::Relaxed);
                reason(None);
                Ok(())
            }
            Err(error) => {
                self.pending = None;
                replace(None);
                reason(Some(&error));
                Err(error)
            }
        }
    }
    pub(crate) fn stage(
        &mut self,
        device: &ID3D12Device,
        queue: &ID3D12CommandQueue,
        input: GeometryInput,
        frame: Frame,
        glint: Option<GlintArtwork>,
    ) -> Result<(), String> {
        if !self.available() {
            return Ok(());
        }
        if !input.current() || !input.matches(&frame) {
            STALE.fetch_add(1, Ordering::Relaxed);
            return Ok(());
        }
        GENERATED.fetch_add(1, Ordering::Relaxed);
        if frame.batches.is_empty() {
            replace(None);
            reason(Some("captured world emitted no replacement geometry"));
            return Ok(());
        }
        // The packet is useful provenance even when the installed live source
        // cannot yet produce Java-equivalent lightmap/fog/material constants.
        // Do not turn that case into white lighting or a successful GPU draw.
        if frame.material.is_none() {
            replace(None);
            reason(Some("Java entity lightmap/fog/material inputs unavailable"));
            return Ok(());
        }
        match avatar_gpu_upload::begin_with_glint(device, queue, frame, glint) {
            Ok(Some(ticket)) => {
                self.pending = Some(Pending { input, ticket });
                STARTED.fetch_add(1, Ordering::Relaxed);
                reason(None);
                Ok(())
            }
            Ok(None) => Ok(()),
            Err(error) => {
                reason(Some(&error));
                Err(error)
            }
        }
    }
}

/// Exact-key borrowing for the native phase adapter. This does not authorize
/// drawing or suppression: that caller still supplies real phase/target owners.
pub(crate) fn completed(
    scope: u64,
    width: u32,
    height: u32,
    reverse_z: bool,
) -> Option<Arc<Uploaded>> {
    if !crate::native_body::enabled() {
        return None;
    }
    let completed = ready().try_lock().ok()?.clone()?;
    if completed.input.scope() != scope
        || completed.input.surface() != (width, height, reverse_z)
        || !completed.input.current()
        || !completed.input.matches(completed.uploaded.frame())
    {
        return None;
    }
    Some(completed.uploaded.clone())
}

pub(crate) fn status() -> serde_json::Value {
    let current = ready().try_lock().ok().and_then(|slot| slot.clone());
    serde_json::json!({"generatedPackets":GENERATED.load(Ordering::Relaxed),
        "uploadsStarted":STARTED.load(Ordering::Relaxed),"uploadsCompleted":COMPLETED.load(Ordering::Relaxed),
        "stalePackets":STALE.load(Ordering::Relaxed),
        "completedWorld":current.as_ref().map(|value|serde_json::json!({
            "session":value.input.session(),"sequence":value.input.sequence(),"scope":value.input.scope(),
            "current":value.input.current(),"surface":value.input.surface()})),
        "reason":REASON.try_lock().ok().and_then(|value|value.clone()),
        "nativeInsertionImplemented":false,"nativeDrawingVerified":false,"fullParity":false})
}
