//! One native strong reference to the exact inspected MaterialPtr info class.
//! Callback-thread scope only. This does not pin its replaceable unique_ptr,
//! shader descriptors, render phase, scene targets or a GPU submission.
use crate::{native_scene_layout::material as layout, world_hooks::read};
use serde_json::{Value, json};
use std::{
    marker::PhantomData,
    rc::Rc,
    sync::atomic::{AtomicI32, AtomicU64, Ordering},
};

type Release = unsafe extern "system" fn(usize);
static RETAINED: AtomicU64 = AtomicU64::new(0);
static RELEASED: AtomicU64 = AtomicU64::new(0);
static DESTROYED: AtomicU64 = AtomicU64::new(0);
static DELETED: AtomicU64 = AtomicU64::new(0);
static FAULTS: AtomicU64 = AtomicU64::new(0);

pub(super) struct Owner {
    base: usize,
    object: usize,
    control: usize,
    destroy: Release,
    delete: Release,
    // Native object destruction has no established cross-thread contract.
    // Even an Arc containing this owner cannot be sent to a GPU retire worker.
    _thread: PhantomData<Rc<()>>,
}
fn word(address: usize) -> Option<usize> {
    let mut bytes = [0u8; 8];
    read(address, &mut bytes).ok()?;
    usize::try_from(u64::from_le_bytes(bytes)).ok()
}
impl Owner {
    /// # Safety
    /// This exact native constructor/submission callback owns a live strong
    /// MaterialPtr pair for the duration of acquisition. Readable bytes or a
    /// positive observed count alone do not meet this contract. The approved
    /// image/profile and concrete control class must already be installed.
    pub(super) unsafe fn retain(base: usize, object: usize, control: usize) -> Option<Self> {
        if FAULTS.load(Ordering::Acquire) != 0
            || crate::world_hooks::image_base() != Some(base)
            || !(0x10000..0x0000_8000_0000_0000).contains(&control)
            || !control.is_multiple_of(8)
            || control.checked_add(layout::INFO_IN_CONTROL)? != object
        {
            return None;
        }
        let table = base.checked_add(layout::INFO_CONTROL_VTABLE)?;
        if word(control)? != table {
            return None;
        }
        for (slot, rva) in [
            layout::INFO_DESTROY_RVA,
            layout::INFO_DELETE_RVA,
            layout::INFO_DELETING_DTOR_RVA,
        ]
        .into_iter()
        .enumerate()
        {
            if word(table.checked_add(slot * 8)?)? != base.checked_add(rva)? {
                return None;
            }
        }
        // SAFETY: caller owns this concrete live control, whose aligned native
        // strong/weak LONG fields use interlocked operations at +8/+12.
        let strong = unsafe { AtomicI32::from_ptr(control.checked_add(8)? as *mut i32) };
        let weak = unsafe { AtomicI32::from_ptr(control.checked_add(12)? as *mut i32) };
        if weak.load(Ordering::SeqCst) <= 0 {
            return None;
        }
        let destroy: Release =
            unsafe { std::mem::transmute(base.checked_add(layout::INFO_DESTROY_RVA)?) };
        let delete: Release =
            unsafe { std::mem::transmute(base.checked_add(layout::INFO_DELETE_RVA)?) };
        let mut count = strong.load(Ordering::SeqCst);
        for _ in 0..32 {
            if !(1..i32::MAX - 1).contains(&count) {
                return None;
            }
            match strong.compare_exchange_weak(count, count + 1, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => {
                    RETAINED.fetch_add(1, Ordering::Relaxed);
                    let result = Self {
                        base,
                        object,
                        control,
                        destroy,
                        delete,
                        _thread: PhantomData,
                    };
                    // A retained info object may still have been replaced in
                    // its owning pair. The caller also rechecks that pair.
                    return result.current().then_some(result);
                }
                Err(next) => count = next,
            }
        }
        None
    }
    pub(super) fn current(&self) -> bool {
        if FAULTS.load(Ordering::Acquire) != 0
            || crate::world_hooks::image_base() != Some(self.base)
            || word(self.control) != self.base.checked_add(layout::INFO_CONTROL_VTABLE)
            || self.control.checked_add(layout::INFO_IN_CONTROL) != Some(self.object)
        {
            return false;
        }
        // SAFETY: this Owner holds one native strong reference until Drop.
        let strong = unsafe { AtomicI32::from_ptr((self.control + 8) as *mut i32) };
        let weak = unsafe { AtomicI32::from_ptr((self.control + 12) as *mut i32) };
        strong.load(Ordering::SeqCst) > 0 && weak.load(Ordering::SeqCst) > 0
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        // SAFETY: acquisition owns exactly one strong count. This !Send/!Sync
        // value is released in its native callback thread. The pinned module's
        // exact virtual ABI destroys info at last strong, then deletes the
        // control after dropping its implicit weak count at last weak.
        let strong = unsafe { AtomicI32::from_ptr((self.control + 8) as *mut i32) };
        let before = strong.fetch_sub(1, Ordering::SeqCst);
        RELEASED.fetch_add(1, Ordering::Relaxed);
        if before > 1 {
            return;
        }
        if before != 1 {
            FAULTS.fetch_add(1, Ordering::AcqRel);
            return;
        }
        unsafe { (self.destroy)(self.control) };
        DESTROYED.fetch_add(1, Ordering::Relaxed);
        let weak = unsafe { AtomicI32::from_ptr((self.control + 12) as *mut i32) };
        match weak.fetch_sub(1, Ordering::SeqCst) {
            1 => {
                unsafe { (self.delete)(self.control) };
                DELETED.fetch_add(1, Ordering::Relaxed);
            }
            value if value > 1 => {}
            _ => {
                FAULTS.fetch_add(1, Ordering::AcqRel);
            }
        }
    }
}
pub(super) fn status() -> Value {
    json!({"infoRetentions":RETAINED.load(Ordering::Relaxed),"infoReleases":RELEASED.load(Ordering::Relaxed),
        "infoDestructions":DESTROYED.load(Ordering::Relaxed),"controlDeletions":DELETED.load(Ordering::Relaxed),
        "invalidReleaseCounts":FAULTS.load(Ordering::Relaxed),"callbackThreadOnly":true,
        "maximumRetainAttempts":32,
        "nestedRenderMaterialPinned":false,"sceneLeaseImplemented":false,"runtimeVerified":false})
}
