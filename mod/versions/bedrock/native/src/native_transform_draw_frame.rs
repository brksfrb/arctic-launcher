//! Native frame handoff while the original model draw owns its component.
//! Never infer a clock from an increment or reuse earlier phase receipts.
use super::{
    ACTORS, ActorScope, Global, TransformObservation, actor_observation, observation_serial,
    pointer, word,
};
use std::{
    cell::Cell,
    marker::PhantomData,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};

static HANDOFFS: AtomicU64 = AtomicU64::new(0);
static REJECTED: AtomicU64 = AtomicU64::new(0);
thread_local! {static REJECTION_EPOCH:Cell<u64>=const{Cell::new(1)};}
pub(super) fn rejection_epoch() -> u64 {
    REJECTION_EPOCH.try_with(Cell::get).unwrap_or(0)
}
fn reject() {
    REJECTED.fetch_add(1, Ordering::Relaxed);
    // This independent cell remains available when an ACTORS borrow fails.
    // Nested scopes cannot clear a failure and revive their older ancestor.
    let _ = REJECTION_EPOCH.try_with(|epoch| {
        let old = epoch.get();
        epoch.set(if old == 0 {
            0
        } else {
            old.checked_add(1).unwrap_or(0)
        });
    });
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct Call {
    data: usize,
    model: usize,
    component: usize,
}
impl Call {
    pub(super) fn new(data: usize, model: usize, component: usize) -> Self {
        Self {
            data,
            model,
            component,
        }
    }
    fn valid(self, entry: &ActorScope) -> Option<()> {
        if pointer(
            entry
                .actor
                .checked_add(crate::native_scene_layout::actor::ANIMATION_COMPONENT)?,
        )? != self.component
            || pointer(self.data)? != entry.actor
            || word(self.data.checked_add(8)?)? != 0
            || pointer(self.data.checked_add(0x38)?)? != self.component
            || pointer(self.component.checked_add(0x88)?)? != entry.context
            || pointer(self.component.checked_add(0xc0)?)? != entry.actor
            || pointer(self.component.checked_add(0xa8)?)? != self.model
        {
            return None;
        }
        let mut ui = [0u8; 1];
        super::read(self.data.checked_add(0x2e)?, &mut ui).ok()?;
        (ui == [0]).then_some(())
    }
}
struct Pending {
    scope: u64,
    observation: u64,
    old_frame: u64,
    frame: u64,
    call: Call,
    global: Global,
}
fn pending() -> Option<Option<Pending>> {
    ACTORS
        .try_with(|active| {
            let active = active.try_borrow().ok()?;
            let entry = active.last()?;
            if entry.ambiguous || crate::world_hooks::current_pass() != Some(entry.scope) {
                return None;
            }
            let Some(global) = entry.observed.as_ref() else {
                return Some(None);
            };
            if global.observation != TransformObservation::DrawEntry {
                return Some(None);
            }
            let epoch = rejection_epoch();
            if epoch == 0 || entry.rejection_epoch != epoch {
                return None;
            }
            let component = pointer(
                entry
                    .actor
                    .checked_add(crate::native_scene_layout::actor::ANIMATION_COMPONENT)?,
            )?;
            if pointer(component.checked_add(0x88)?)? != entry.context
                || pointer(component.checked_add(0xc0)?)? != entry.actor
            {
                return None;
            }
            if let Some(call) = entry.draw_call {
                call.valid(entry)?;
            }
            let frame = word(component.checked_add(0x368)?)?;
            if frame == global.animation_frame {
                return Some(None);
            }
            // Changes are admitted only while the typed draw callback owns its
            // original invocation. The numeric frame need not increase.
            let call = entry.draw_call?;
            call.valid(entry)?;
            let scale = super::floats::<1>(component.checked_add(0x198)?, 16.)?[0];
            if global.draw.as_ref()?.model_scale.to_bits() != scale.to_bits() {
                return None;
            }
            Some(Some(Pending {
                scope: entry.id,
                observation: entry.observation,
                old_frame: global.animation_frame,
                frame,
                call,
                global: global.clone(),
            }))
        })
        .ok()
        .flatten()
}
pub(super) fn synchronize() -> bool {
    let Some(pending) = pending() else {
        reject();
        return false;
    };
    let Some(pending) = pending else {
        return true;
    };
    let accepted = (|| {
        let context = ACTORS
            .try_with(|active| {
                let active = active.try_borrow().ok()?;
                let entry = active.last()?;
                (entry.id == pending.scope).then_some(entry.context)
            })
            .ok()
            .flatten()?;
        let mut fresh = actor_observation(pending.call.data, context)?;
        if fresh.observation != TransformObservation::DrawEntry
            || fresh.animation_frame != pending.frame
            || fresh.actor_id != pending.global.actor_id
            || fresh.world_scope != pending.global.world_scope
        {
            return None;
        }
        fresh.draw = pending.global.draw.clone();
        fresh.draw.as_ref()?;
        let next = observation_serial()?;
        let retired = ACTORS
            .try_with(|active| {
                let mut active = active.try_borrow_mut().ok()?;
                let entry = active.last_mut()?;
                if entry.id != pending.scope
                    || entry.observation != pending.observation
                    || entry.ambiguous
                    || entry.rejection_epoch != rejection_epoch()
                    || entry.draw_call != Some(pending.call)
                    || entry.scope != fresh.world_scope
                    || crate::world_hooks::current_pass() != Some(entry.scope)
                    || entry.observed.as_ref()?.animation_frame != pending.old_frame
                {
                    return None;
                }
                pending.call.valid(entry)?;
                if word(pending.call.component.checked_add(0x368)?)? != pending.frame {
                    return None;
                }
                let scale = super::floats::<1>(pending.call.component.checked_add(0x198)?, 16.)?[0];
                if fresh.draw.as_ref()?.model_scale.to_bits() != scale.to_bits() {
                    return None;
                }
                let reference = entry.skin_reference.take();
                let old = entry.observed.replace(fresh);
                entry.observation = next;
                // Invalidation poison and expression serials are preserved. A
                // native frame change cannot erase an earlier scope failure or
                // make a completed setup serial available for reuse.
                entry.palette_writes.advance_frame();
                entry.frame_uploads.advance_frame();
                entry.expression_bindings.invalidate();
                entry.attachment_samples.advance_frame();
                Some((old, reference))
            })
            .ok()
            .flatten()?;
        // Any owned shader/reference state retires outside the TLS borrow.
        drop(retired);
        HANDOFFS.fetch_add(1, Ordering::Relaxed);
        Some(())
    })()
    .is_some();
    if !accepted {
        reject();
    }
    accepted
}
pub(super) struct Guard {
    scope: Option<u64>,
    _thread: PhantomData<Rc<()>>,
}
impl Guard {
    pub(super) fn new() -> Self {
        Self {
            scope: None,
            _thread: PhantomData,
        }
    }
    pub(super) fn bind(&mut self, id: u64, slot: &mut Option<Call>, call: Call) -> Option<()> {
        if self.scope.is_some() || slot.is_some() {
            return None;
        }
        self.scope = Some(id);
        *slot = Some(call);
        Some(())
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        let Some(id) = self.scope else {
            return;
        };
        let accepted = std::panic::catch_unwind(synchronize).unwrap_or(false);
        let cleared = ACTORS
            .try_with(|active| {
                let mut active = active.try_borrow_mut().ok()?;
                let top = active.last().map(|entry| entry.id);
                let entry = active.iter_mut().find(|entry| entry.id == id)?;
                if !accepted || top != Some(id) || entry.draw_call.is_none() {
                    entry.ambiguous = true;
                }
                entry.draw_call = None;
                Some(())
            })
            .ok()
            .flatten()
            .is_some();
        if !accepted || !cleared {
            reject();
            if !cleared {
                crate::native_emote_cache_history::invalidate();
            }
        }
    }
}
pub(super) fn status() -> serde_json::Value {
    serde_json::json!({"sourceImplemented":true,"nativeFrameHandoffs":HANDOFFS.load(Ordering::Relaxed),
        "rejected":REJECTED.load(Ordering::Relaxed),"frameMustIncrease":false,
        "priorFrameReceiptsRetired":true,"priorFailurePoisonPreserved":true,
        "failedFrameHandoffCannotRecoverThroughFrameABA":true,
        "observationVersionNeverArchived":true,"runtimeVerified":false})
}
