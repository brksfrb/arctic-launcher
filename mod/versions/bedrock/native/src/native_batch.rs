//! Join an owned native batch packet to actual D3D12 root CBV/CBF bindings.
//! No native packet, stack field, graphics binding or return value is changed.
use crate::native_scene_layout::{frame_view, native_batch as layout, producer_frame};
use crate::render_bridge::FrameKey;
use crate::world_hooks::{pointer, read};
use hudhook::mh::{MH_CreateHook, MH_EnableHook, MH_STATUS};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::{
    OnceLock,
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};

type Reset = unsafe extern "system" fn(usize, usize, u8);
type ResetFrame = unsafe extern "system" fn(usize, usize);
type Fill = unsafe extern "system" fn(usize, usize, usize, usize) -> u32;
static INSTALL: OnceLock<Result<(), String>> = OnceLock::new();
static ENABLED: AtomicBool = AtomicBool::new(false);
static RESET: AtomicUsize = AtomicUsize::new(0);
static FILL: AtomicUsize = AtomicUsize::new(0);
static RESET_CALLS: AtomicU64 = AtomicU64::new(0);
static MATCHED_RESETS: AtomicU64 = AtomicU64::new(0);
static FILL_CALLS: AtomicU64 = AtomicU64::new(0);
static PACKETS: AtomicU64 = AtomicU64::new(0);
static WORLD_BINDINGS: AtomicU64 = AtomicU64::new(0);
static OTHER_BINDINGS: AtomicU64 = AtomicU64::new(0);
static CONFLICTS: AtomicU64 = AtomicU64::new(0);
static SCOPED_DRAWS: AtomicU64 = AtomicU64::new(0);
static STEP: AtomicUsize = AtomicUsize::new(0);

struct Context {
    frame: Option<FrameKey>,
    batch: usize,
    renderer: usize,
    stack: Option<usize>,
    bindings: HashMap<(usize, u64, u64), Option<u64>>,
    views: HashMap<u16, Option<u64>>,
    overflow: bool,
}
thread_local! {
    static ACTIVE:RefCell<Vec<Context>>=const{RefCell::new(Vec::new())};
    static SKIPPED:Cell<usize>=const{Cell::new(0)};
}
pub(crate) struct Guard(bool);
impl Drop for Guard {
    fn drop(&mut self) {
        if self.0 {
            let _ = ACTIVE.try_with(|s| {
                s.borrow_mut().pop();
            });
        } else {
            let _ = SKIPPED.try_with(|s| s.set(s.get() - 1));
        }
    }
}
pub(crate) fn begin(frame: Option<FrameKey>, batch: usize) -> Guard {
    begin_with_renderer(frame, batch, 0)
}
pub(crate) fn begin_with_renderer(frame: Option<FrameKey>, batch: usize, renderer: usize) -> Guard {
    let pushed = ACTIVE.with(|s| {
        let mut s = s.borrow_mut();
        if s.len() >= 8 {
            return false;
        }
        s.push(Context {
            frame,
            batch,
            renderer,
            stack: None,
            bindings: HashMap::new(),
            views: HashMap::new(),
            overflow: false,
        });
        true
    });
    if !pushed {
        SKIPPED.with(|s| s.set(s.get() + 1));
    }
    Guard(pushed)
}
fn with_context<T>(run: impl FnOnce(&mut Context) -> Option<T>) -> Option<T> {
    if SKIPPED.try_with(Cell::get).ok()? != 0 {
        return None;
    }
    ACTIVE
        .try_with(|s| run(s.try_borrow_mut().ok()?.last_mut()?))
        .ok()
        .flatten()
}
pub(crate) fn framebuffer_frame(renderer: usize) -> Option<FrameKey> {
    with_context(|context| {
        if renderer == 0 || context.renderer != renderer || context.overflow {
            return None;
        }
        let frame = context.frame?;
        (count(frame.address.checked_add(producer_frame::FRAME_NUMBER)?)? == frame.number)
            .then_some(frame)
    })
}
/// The actual backend submission currently enclosing this thread, with its
/// frame-number guard still intact; never a globally most-recent frame.
pub(crate) fn current_frame() -> Option<FrameKey> {
    with_context(|context| {
        if context.overflow {
            return None;
        }
        let frame = context.frame?;
        (count(frame.address.checked_add(producer_frame::FRAME_NUMBER)?)? == frame.number)
            .then_some(frame)
    })
}
fn word(address: usize) -> Option<u64> {
    let mut b = [0u8; 8];
    read(address, &mut b).ok()?;
    Some(u64::from_le_bytes(b))
}
fn count(address: usize) -> Option<u32> {
    let mut b = [0u8; 4];
    read(address, &mut b).ok()?;
    Some(u32::from_le_bytes(b))
}
fn insert(context: &mut Context, key: (usize, u64, u64), scope: Option<u64>) {
    if context.overflow {
        return;
    }
    if let Some(previous) = context.bindings.get_mut(&key) {
        if *previous != scope {
            *previous = None;
            CONFLICTS.fetch_add(1, Ordering::Relaxed);
        }
    } else if context.bindings.len() < 4096 {
        context.bindings.insert(key, scope);
    } else {
        context.bindings.clear();
        context.overflow = true;
    }
}
fn observe_packet(command: usize, output: usize, draw: usize) -> Option<()> {
    STEP.store(0, Ordering::Relaxed);
    with_context(|context| {
        let frame = context.frame?;
        if count(frame.address.checked_add(producer_frame::FRAME_NUMBER)?)? != frame.number {
            return None;
        }
        STEP.store(1, Ordering::Relaxed);
        let stack = context.stack?;
        STEP.store(2, Ordering::Relaxed);
        let index = u16::try_from(word(stack.checked_add(layout::VIEW_IN_STACK)?)?).ok()?;
        if usize::from(index) >= frame_view::COUNT {
            return None;
        }
        STEP.store(3, Ordering::Relaxed);
        let mut handle = [0u8; 2];
        read(draw.checked_add(layout::DRAW_INDEX_HANDLE)?, &mut handle).ok()?;
        let indexed = u16::from_le_bytes(handle) != u16::MAX;
        let (count_offset, array_offset, stride, cb_offset) = if indexed {
            (
                layout::INDEXED_COUNT,
                layout::INDEXED_ARRAY,
                layout::INDEXED_STRIDE,
                layout::INDEXED_CBV,
            )
        } else {
            (
                layout::NON_INDEXED_COUNT,
                layout::NON_INDEXED_ARRAY,
                layout::NON_INDEXED_STRIDE,
                layout::NON_INDEXED_CBV,
            )
        };
        let size = count(context.batch.checked_add(count_offset)?)? as usize;
        if size == 0 || size > 65536 {
            return None;
        }
        let array = pointer(context.batch.checked_add(array_offset)?)?;
        if output != array.checked_add((size - 1).checked_mul(stride)?)? {
            return None;
        }
        STEP.store(4, Ordering::Relaxed);
        PACKETS.fetch_add(1, Ordering::Relaxed);
        let cbv = word(output.checked_add(cb_offset)?)?;
        let cbf = word(output.checked_add(cb_offset + 8)?)?;
        if cbv == 0 || cbf == 0 || !cbv.is_multiple_of(256) || !cbf.is_multiple_of(256) {
            return None;
        }
        STEP.store(5, Ordering::Relaxed);
        let list = pointer(command.checked_add(layout::COMMAND_LIST)?)?;
        let mut after_handle = [0u8; 2];
        read(
            draw.checked_add(layout::DRAW_INDEX_HANDLE)?,
            &mut after_handle,
        )
        .ok()?;
        if after_handle != handle
            || word(stack.checked_add(layout::VIEW_IN_STACK)?)? != u64::from(index)
            || count(frame.address.checked_add(producer_frame::FRAME_NUMBER)?)? != frame.number
            || count(context.batch.checked_add(count_offset)?)? as usize != size
            || pointer(context.batch.checked_add(array_offset)?)? != array
            || word(output.checked_add(cb_offset)?)? != cbv
            || word(output.checked_add(cb_offset + 8)?)? != cbf
            || pointer(command.checked_add(layout::COMMAND_LIST)?)? != list
        {
            return None;
        }
        let scope = *context
            .views
            .entry(index)
            .or_insert_with(|| crate::render_bridge::scope_for_view(frame, index));
        if scope.is_some() {
            WORLD_BINDINGS.fetch_add(1, Ordering::Relaxed);
        } else {
            OTHER_BINDINGS.fetch_add(1, Ordering::Relaxed);
        }
        crate::native_submit::observe_packet(frame, draw, scope);
        insert(context, (list, cbv, cbf), scope);
        STEP.store(6, Ordering::Relaxed);
        Some(())
    })
}
pub(crate) fn draw_scope(list: usize, cbv: Option<u64>, cbf: Option<u64>) -> Option<u64> {
    let scope = binding_scope(list, cbv, cbf)?;
    SCOPED_DRAWS.fetch_add(1, Ordering::Relaxed);
    Some(scope)
}
pub(crate) fn binding_scope(list: usize, cbv: Option<u64>, cbf: Option<u64>) -> Option<u64> {
    with_context(|context| {
        if context.overflow {
            return None;
        }
        let frame = context.frame?;
        if count(frame.address.checked_add(producer_frame::FRAME_NUMBER)?)? != frame.number {
            return None;
        }
        let scope = context
            .bindings
            .get(&(list, cbv?, cbf?))
            .copied()
            .flatten()?;
        if !crate::world_hooks::has_snapshot(scope) {
            return None;
        }
        Some(scope)
    })
}
fn observe_reset(this: usize, frame: usize) {
    if !ENABLED.load(Ordering::Acquire) {
        return;
    }
    let _ = std::panic::catch_unwind(|| {
        let binding = crate::render_bridge::current_backend_binding()?;
        if crate::world_hooks::game_callers().first().copied() != Some(binding.reset_return_rva) {
            return None;
        }
        with_context(|context| {
            context.stack = None;
            context.bindings.clear();
            context.views.clear();
            let expected = context.frame?;
            if context.overflow
                || expected.address != frame
                || count(frame.checked_add(producer_frame::FRAME_NUMBER)?)? != expected.number
            {
                return None;
            }
            // This reset object's placement comes from the admitted backend
            // E8 caller. The derived stack pointer stays in its enclosing call.
            context.stack = this.checked_sub(layout::RESET_IN_STACK);
            MATCHED_RESETS.fetch_add(1, Ordering::Relaxed);
            Some(())
        })
    });
}
unsafe extern "system" fn reset(this: usize, frame: usize, stereo: u8) {
    let original: Reset = unsafe { std::mem::transmute(RESET.load(Ordering::Acquire)) };
    unsafe { original(this, frame, stereo) };
    RESET_CALLS.fetch_add(1, Ordering::Relaxed);
    observe_reset(this, frame);
}
unsafe extern "system" fn reset_frame(this: usize, frame: usize) {
    let original: ResetFrame = unsafe { std::mem::transmute(RESET.load(Ordering::Acquire)) };
    unsafe { original(this, frame) };
    RESET_CALLS.fetch_add(1, Ordering::Relaxed);
    observe_reset(this, frame);
}
unsafe extern "system" fn fill(command: usize, output: usize, draw: usize, vertices: usize) -> u32 {
    let original: Fill = unsafe { std::mem::transmute(FILL.load(Ordering::Acquire)) };
    let result = unsafe { original(command, output, draw, vertices) };
    FILL_CALLS.fetch_add(1, Ordering::Relaxed);
    if ENABLED.load(Ordering::Acquire) {
        let _ = std::panic::catch_unwind(|| observe_packet(command, output, draw));
    }
    result
}
unsafe fn install_targets(targets: [usize; 2], reset_callback: usize) -> Result<(), String> {
    for ((target, callback), slot) in targets
        .into_iter()
        .zip([reset_callback, fill as *const () as usize])
        .zip([&RESET, &FILL])
    {
        let mut original = std::ptr::null_mut();
        let status = unsafe { MH_CreateHook(target as _, callback as _, &mut original) };
        if status != MH_STATUS::MH_OK {
            return Err(format!("create batch observer: {status:?}"));
        }
        slot.store(original as usize, Ordering::Release);
    }
    for target in targets {
        let status = unsafe { MH_EnableHook(target as _) };
        if status != MH_STATUS::MH_OK {
            return Err(format!("enable batch observer: {status:?}"));
        }
    }
    ENABLED.store(true, Ordering::Release);
    Ok(())
}
pub(crate) unsafe fn install(base: usize) -> Result<(), String> {
    crate::render_bridge::install_producer(base)?;
    let binding = crate::render_bridge::backend_binding(base)?;
    INSTALL
        .get_or_init(|| {
            let targets = [
                base.checked_add(binding.reset_rva)
                    .ok_or("batch reset address overflow")?,
                base.checked_add(binding.fill_rva)
                    .ok_or("batch fill address overflow")?,
            ];
            let callback = match binding.reset_abi {
                crate::render_bridge::BatchResetAbi::FrameStereo3 => reset as *const () as usize,
                crate::render_bridge::BatchResetAbi::Frame2 => reset_frame as *const () as usize,
            };
            unsafe { install_targets(targets, callback) }
        })
        .clone()
}
pub(crate) fn status() -> serde_json::Value {
    serde_json::json!({"installed":ENABLED.load(Ordering::Acquire),"exactFamilyBatchSourceImplemented":true,
        "resetRva":crate::render_bridge::current_backend_binding().map(|binding|binding.reset_rva),
        "fillRva":crate::render_bridge::current_backend_binding().map(|binding|binding.fill_rva),
        "resetAbi":crate::render_bridge::current_backend_binding().map(|binding|match binding.reset_abi{
            crate::render_bridge::BatchResetAbi::FrameStereo3=>"frameStereo3",
            crate::render_bridge::BatchResetAbi::Frame2=>"frame2"}),
        "resetCalls":RESET_CALLS.load(Ordering::Relaxed),"matchedResets":MATCHED_RESETS.load(Ordering::Relaxed),
        "fillCalls":FILL_CALLS.load(Ordering::Relaxed),"ownedPackets":PACKETS.load(Ordering::Relaxed),
        "worldBindings":WORLD_BINDINGS.load(Ordering::Relaxed),"otherBindings":OTHER_BINDINGS.load(Ordering::Relaxed),
        "conflicts":CONFLICTS.load(Ordering::Relaxed),"scopedDraws":SCOPED_DRAWS.load(Ordering::Relaxed),"step":STEP.load(Ordering::Relaxed)})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn packet_reader_requires_the_actual_last_owned_slot_and_records_no_guessed_scope() {
        let mut frame = vec![0u8; producer_frame::FRAME_NUMBER + 4];
        frame[producer_frame::FRAME_NUMBER..producer_frame::FRAME_NUMBER + 4]
            .copy_from_slice(&7u32.to_le_bytes());
        let mut batch = vec![0u8; 0xa8];
        let mut packet = vec![0u8; layout::INDEXED_STRIDE];
        let mut command = vec![0u8; layout::COMMAND_LIST + 8];
        let mut stack = vec![0u8; layout::VIEW_IN_STACK + 8];
        let mut draw = [0u8; 0x78];
        let set = |b: &mut [u8], at: usize, value: u64| {
            b[at..at + 8].copy_from_slice(&value.to_le_bytes())
        };
        set(&mut packet, layout::INDEXED_CBV, 256);
        set(&mut packet, layout::INDEXED_CBV + 8, 512);
        batch[layout::INDEXED_COUNT..layout::INDEXED_COUNT + 4]
            .copy_from_slice(&1u32.to_le_bytes());
        set(&mut batch, layout::INDEXED_ARRAY, packet.as_ptr() as u64);
        set(&mut command, layout::COMMAND_LIST, 0x123450);
        set(&mut stack, layout::VIEW_IN_STACK, 3);
        draw[layout::DRAW_INDEX_HANDLE..layout::DRAW_INDEX_HANDLE + 2]
            .copy_from_slice(&1u16.to_le_bytes());
        let _guard = begin(
            Some(FrameKey {
                address: frame.as_ptr() as usize,
                number: 7,
            }),
            batch.as_ptr() as usize,
        );
        with_context(|context| {
            context.stack = Some(stack.as_ptr() as usize);
            Some(())
        });
        assert!(
            observe_packet(
                command.as_ptr() as usize,
                packet.as_ptr() as usize,
                draw.as_ptr() as usize
            )
            .is_some()
        );
        with_context(|context| {
            assert_eq!(context.bindings[&(0x123450, 256, 512)], None);
            Some(())
        });
        assert!(
            observe_packet(
                command.as_ptr() as usize,
                packet.as_ptr() as usize + 8,
                draw.as_ptr() as usize
            )
            .is_none()
        );
        assert_eq!(draw_scope(0x123450, Some(256), Some(512)), None);
    }
    #[inline(never)]
    unsafe extern "system" fn host_reset(output: usize, frame: usize, flag: u8) {
        unsafe { *(output as *mut [usize; 2]) = std::hint::black_box([frame, usize::from(flag)]) };
    }
    #[inline(never)]
    unsafe extern "system" fn host_fill(a: usize, b: usize, c: usize, output: usize) -> u32 {
        unsafe { *(output as *mut [usize; 3]) = std::hint::black_box([a, b, c]) };
        std::hint::black_box(0x81234567)
    }
    #[test]
    fn batch_detours_preserve_flags_outputs_and_full_u32_return() {
        let initialized = unsafe { hudhook::mh::MH_Initialize() };
        assert!(matches!(
            initialized,
            MH_STATUS::MH_OK | MH_STATUS::MH_ERROR_ALREADY_INITIALIZED
        ));
        unsafe {
            install_targets(
                [
                    host_reset as *const () as usize,
                    host_fill as *const () as usize,
                ],
                super::reset as *const () as usize,
            )
            .unwrap()
        };
        let reset: Reset = std::hint::black_box(host_reset);
        let fill: Fill = std::hint::black_box(host_fill);
        let mut output = [0usize; 3];
        unsafe { reset(output.as_mut_ptr() as usize, 0x1122334455667788, 0xa5) };
        assert_eq!(&output[..2], &[0x1122334455667788, 0xa5]);
        assert_eq!(
            unsafe { fill(1, 2, 3, output.as_mut_ptr() as usize) },
            0x81234567
        );
        assert_eq!(output, [1, 2, 3]);
    }
    #[test]
    fn shared_binding_between_world_and_other_views_stays_rejected() {
        let _guard = begin(None, 0);
        with_context(|context| {
            insert(context, (1, 256, 512), Some(7));
            insert(context, (1, 256, 512), None);
            insert(context, (1, 256, 512), Some(7));
            assert_eq!(context.bindings[&(1, 256, 512)], None);
            Some(())
        });
        assert_eq!(draw_scope(1, Some(256), Some(512)), None);
    }
}
