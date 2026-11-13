//! Passive diagnostics for the pinned installed-game test. These observers
//! forward every argument/result and never alter connection or user state.
use hudhook::mh::{MH_CreateHook, MH_EnableHook, MH_STATUS};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
static ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static CALLS: AtomicU64 = AtomicU64::new(0);
static LAST: AtomicU64 = AtomicU64::new(0);
static BASE: AtomicUsize = AtomicUsize::new(0);
static START: AtomicUsize = AtomicUsize::new(0);
static TICK: AtomicUsize = AtomicUsize::new(0);
static STARTS: AtomicU64 = AtomicU64::new(0);
static TICKS: AtomicU64 = AtomicU64::new(0);
static FLAGS: AtomicU64 = AtomicU64::new(u64::MAX);
static START_CALLBACK: AtomicUsize = AtomicUsize::new(0);
static PROGRESS_CALLERS: OnceLock<Vec<usize>> = OnceLock::new();
static CONTROLLER: AtomicUsize = AtomicUsize::new(0);
static GATE: AtomicUsize = AtomicUsize::new(0);
static CONTROLLER_STATE: AtomicU64 = AtomicU64::new(u64::MAX);
static GATE_CALLS: AtomicU64 = AtomicU64::new(0);
static GATE_RESULT: AtomicU64 = AtomicU64::new(u64::MAX);
static GATE_TARGET: AtomicUsize = AtomicUsize::new(0);
static CONNECTIVITY: AtomicUsize = AtomicUsize::new(0);
static CONNECTIVITY_CALLS: AtomicU64 = AtomicU64::new(0);
static CONNECTIVITY_RESULT: AtomicU64 = AtomicU64::new(u64::MAX);
static CONNECTIVITY_HINT: AtomicU64 = AtomicU64::new(u64::MAX);
static CONNECTIVITY_CHANGED: AtomicUsize = AtomicUsize::new(0);
static CONNECTIVITY_CHANGES: AtomicU64 = AtomicU64::new(0);
static CONNECTIVITY_CHANGED_HINT: AtomicU64 = AtomicU64::new(u64::MAX);

fn record_hint(input: *const u8, destination: &AtomicU64) {
    if let Some(bytes) = super::read::<[u8; 16]>(input.cast()) {
        let level = u32::from_ne_bytes(bytes[..4].try_into().unwrap());
        if level <= 4 && bytes[12] <= 1 {
            destination.store(
                u64::from(level) | u64::from(bytes[12]) << 32,
                Ordering::Relaxed,
            );
        }
    }
}

type ConnectivityChanged = unsafe extern "system" fn(usize, *const u8);
unsafe extern "system" fn connectivity_changed(context: usize, input: *const u8) {
    record_hint(input, &CONNECTIVITY_CHANGED_HINT);
    CONNECTIVITY_CHANGES.fetch_add(1, Ordering::Relaxed);
    let original: ConnectivityChanged =
        unsafe { std::mem::transmute(CONNECTIVITY_CHANGED.load(Ordering::Acquire)) };
    unsafe { original(context, input) };
}

type Connectivity = unsafe extern "system" fn(*mut u8) -> i32;
unsafe extern "system" fn connectivity(output: *mut u8) -> i32 {
    let original: Connectivity =
        unsafe { std::mem::transmute(CONNECTIVITY.load(Ordering::Acquire)) };
    let result = unsafe { original(output) };
    CONNECTIVITY_CALLS.fetch_add(1, Ordering::Relaxed);
    CONNECTIVITY_RESULT.store(result as u32 as u64, Ordering::Relaxed);
    // XNetworkingConnectivityHint is three u32 enums and four bool bytes.
    // Copy only after success; never inspect an uninitialized failure output.
    if result >= 0 {
        record_hint(output, &CONNECTIVITY_HINT);
    }
    result
}

unsafe fn install_connectivity(target: usize, changed: bool) -> Result<(), String> {
    let (callback, slot) = if changed {
        (
            connectivity_changed as *const () as usize,
            &CONNECTIVITY_CHANGED,
        )
    } else {
        (connectivity as *const () as usize, &CONNECTIVITY)
    };
    let mut original = std::ptr::null_mut();
    let created = unsafe { MH_CreateHook(target as _, callback as _, &mut original) };
    if created != MH_STATUS::MH_OK {
        return Err(format!("create connectivity observer: {created:?}"));
    }
    slot.store(original as usize, Ordering::Release);
    let enabled = unsafe { MH_EnableHook(target as _) };
    if enabled != MH_STATUS::MH_OK {
        return Err(format!("enable connectivity observer: {enabled:?}"));
    }
    Ok(())
}

type Controller = unsafe extern "system" fn(usize) -> u32;
unsafe extern "system" fn controller(this: usize) -> u32 {
    let _ = std::panic::catch_unwind(|| {
        let initiated = super::read::<u8>(this.checked_add(0xd70)? as *const _)?;
        let has_ticked = super::read::<u8>(this.checked_add(0xf18)? as *const _)?;
        let handler = crate::world_hooks::pointer(this.checked_add(0xda0)?)?;
        let table = crate::world_hooks::pointer(handler)?;
        let base = BASE.load(Ordering::Acquire);
        if initiated <= 1 && has_ticked <= 1 && table.checked_sub(base)? == 0xe7ba660 {
            CONTROLLER_STATE.store(
                u64::from(initiated) | u64::from(has_ticked) << 1,
                Ordering::Relaxed,
            );
        }
        Some(())
    });
    let original: Controller = unsafe { std::mem::transmute(CONTROLLER.load(Ordering::Acquire)) };
    unsafe { original(this) }
}
type Gate = unsafe extern "system" fn(usize) -> u8;
unsafe extern "system" fn gate(this: usize) -> u8 {
    let _ = std::panic::catch_unwind(|| {
        let target = crate::world_hooks::pointer(this.checked_add(0x58)?)
            .and_then(crate::world_hooks::pointer)
            .and_then(|table| crate::world_hooks::pointer(table.checked_add(0x598)?))?;
        let rva = target.checked_sub(BASE.load(Ordering::Acquire))?;
        if (0x1000..0xe5a7000).contains(&rva) {
            GATE_TARGET.store(rva, Ordering::Relaxed);
        }
        Some(())
    });
    let original: Gate = unsafe { std::mem::transmute(GATE.load(Ordering::Acquire)) };
    let result = unsafe { original(this) };
    GATE_CALLS.fetch_add(1, Ordering::Relaxed);
    GATE_RESULT.store(u64::from(result), Ordering::Relaxed);
    result
}

fn capture_progress_callers(base: usize) -> Vec<usize> {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn RtlCaptureStackBackTrace(
            skip: u32,
            count: u32,
            frames: *mut usize,
            hash: *mut u32,
        ) -> u16;
    }
    let mut frames = [0usize; 32];
    let count = unsafe {
        RtlCaptureStackBackTrace(
            0,
            frames.len() as u32,
            frames.as_mut_ptr(),
            std::ptr::null_mut(),
        )
    };
    frames[..usize::from(count).min(frames.len())]
        .iter()
        .filter_map(|pc| pc.checked_sub(base))
        .filter(|rva| (0x1000..0xe5a7000).contains(rva))
        .collect()
}
type Progress = unsafe extern "system" fn(usize, usize);
unsafe extern "system" fn start(handler: usize, screen: usize) {
    STARTS.fetch_add(1, Ordering::Relaxed);
    let _ = std::panic::catch_unwind(|| {
        let callback = crate::world_hooks::pointer(handler.checked_add(0x68)?)?;
        let table = crate::world_hooks::pointer(callback)?;
        let invoke = crate::world_hooks::pointer(table.checked_add(16)?)?;
        let base = BASE.load(Ordering::Acquire);
        let rva = invoke.checked_sub(base)?;
        if base != 0 && (0x1000..0xe5a7000).contains(&rva) {
            START_CALLBACK.store(rva, Ordering::Relaxed);
        }
        Some(())
    });
    let original: Progress = unsafe { std::mem::transmute(START.load(Ordering::Acquire)) };
    unsafe { original(handler, screen) };
}
unsafe extern "system" fn tick(handler: usize, screen: usize) {
    TICKS.fetch_add(1, Ordering::Relaxed);
    let _ = std::panic::catch_unwind(|| {
        let base = BASE.load(Ordering::Acquire);
        if base != 0 {
            PROGRESS_CALLERS.get_or_init(|| capture_progress_callers(base));
        }
        // Sample while the argument is live, before the original can close its
        // progress screen. These offsets are corroborated by this build's code.
        let state = super::read::<[u8; 3]>(handler.checked_add(0x10)? as *const _)?;
        let kind = super::read::<u32>(handler.checked_add(0x28)? as *const _)?;
        if state.iter().all(|v| *v <= 1) && kind <= 4 {
            FLAGS.store(
                state[0] as u64
                    | (state[1] as u64) << 1
                    | (state[2] as u64) << 2
                    | (kind as u64) << 8,
                Ordering::Relaxed,
            );
        }
        if let Some(invoke) = crate::world_hooks::pointer(handler.checked_add(0x68)?)
            .and_then(crate::world_hooks::pointer)
            .and_then(|table| crate::world_hooks::pointer(table.checked_add(16)?))
            && let Some(rva) = invoke.checked_sub(BASE.load(Ordering::Acquire))
            && BASE.load(Ordering::Acquire) != 0
            && (0x1000..0xe5a7000).contains(&rva)
        {
            START_CALLBACK.store(rva, Ordering::Relaxed);
        }
        Some(())
    });
    let original: Progress = unsafe { std::mem::transmute(TICK.load(Ordering::Acquire)) };
    unsafe { original(handler, screen) };
}
type Message = unsafe extern "system" fn(usize, usize, i32) -> usize;
unsafe extern "system" fn message(category: usize, output: usize, error: i32) -> usize {
    CALLS.fetch_add(1, Ordering::Relaxed);
    LAST.store(error as u32 as u64, Ordering::Relaxed);
    let original: Message = unsafe { std::mem::transmute(ORIGINAL.load(Ordering::Acquire)) };
    unsafe { original(category, output, error) }
}
/// Called only after world_hooks has validated the complete installed text hash.
pub(super) unsafe fn install(base: usize) -> Result<(), String> {
    let target = base
        .checked_add(0xd547d0)
        .ok_or("error observer address overflow")?;
    BASE.store(base, Ordering::Release);
    unsafe {
        install_target(target)?;
        install_connectivity(
            base.checked_add(0xe0235dc)
                .ok_or("connectivity address overflow")?,
            false,
        )?;
        install_connectivity(
            base.checked_add(0x8ee60)
                .ok_or("connectivity callback address overflow")?,
            true,
        )?;
        install_progress([
            base.checked_add(0x1574400)
                .ok_or("progress address overflow")?,
            base.checked_add(0x1574430)
                .ok_or("progress address overflow")?,
        ])?;
        install_startup([
            base.checked_add(0x4fcdb10)
                .ok_or("startup observer address overflow")?,
            base.checked_add(0x4938850)
                .ok_or("startup observer address overflow")?,
        ])
    }
}
unsafe fn install_startup(targets: [usize; 2]) -> Result<(), String> {
    for ((target, callback), slot) in targets
        .into_iter()
        .zip([controller as *const () as usize, gate as *const () as usize])
        .zip([&CONTROLLER, &GATE])
    {
        let mut original = std::ptr::null_mut();
        let created = unsafe { MH_CreateHook(target as _, callback as _, &mut original) };
        if created != MH_STATUS::MH_OK {
            return Err(format!("create startup observer: {created:?}"));
        }
        slot.store(original as usize, Ordering::Release);
    }
    for target in targets {
        let enabled = unsafe { MH_EnableHook(target as _) };
        if enabled != MH_STATUS::MH_OK {
            return Err(format!("enable startup observer: {enabled:?}"));
        }
    }
    Ok(())
}
unsafe fn install_progress(targets: [usize; 2]) -> Result<(), String> {
    for ((target, callback), slot) in targets
        .into_iter()
        .zip([start as *const () as usize, tick as *const () as usize])
        .zip([&START, &TICK])
    {
        let mut original = std::ptr::null_mut();
        let result = unsafe { MH_CreateHook(target as _, callback as _, &mut original) };
        if result != MH_STATUS::MH_OK {
            return Err(format!("create progress observer: {result:?}"));
        }
        slot.store(original as usize, Ordering::Release);
    }
    for target in targets {
        let result = unsafe { MH_EnableHook(target as _) };
        if result != MH_STATUS::MH_OK {
            return Err(format!("enable progress observer: {result:?}"));
        }
    }
    Ok(())
}
unsafe fn install_target(target: usize) -> Result<(), String> {
    // The inspected std::error_category::message implementation preserves RDX
    // as its std::string output, formats R8D, and returns that output in RAX.
    let mut original = std::ptr::null_mut();
    let result = unsafe { MH_CreateHook(target as _, message as *const () as _, &mut original) };
    if result != MH_STATUS::MH_OK {
        return Err(format!("create connection error observer: {result:?}"));
    }
    ORIGINAL.store(original as usize, Ordering::Release);
    let result = unsafe { MH_EnableHook(target as _) };
    if result != MH_STATUS::MH_OK {
        return Err(format!("enable connection error observer: {result:?}"));
    }
    Ok(())
}
pub(super) fn status() -> serde_json::Value {
    let flags = FLAGS.load(Ordering::Relaxed);
    let progress = if flags == u64::MAX {
        serde_json::Value::Null
    } else {
        serde_json::json!({"connected":flags&1!=0,"handover":flags&2!=0,"initialized":flags&4!=0,"kind":flags>>8})
    };
    let controller = CONTROLLER_STATE.load(Ordering::Relaxed);
    let gate = GATE_RESULT.load(Ordering::Relaxed);
    let hint = CONNECTIVITY_HINT.load(Ordering::Relaxed);
    let hint_result = CONNECTIVITY_RESULT.load(Ordering::Relaxed);
    let changed_hint = CONNECTIVITY_CHANGED_HINT.load(Ordering::Relaxed);
    serde_json::json!({"installed":ORIGINAL.load(Ordering::Acquire)!=0,"messageCalls":CALLS.load(Ordering::Relaxed),"lastSessionError":LAST.load(Ordering::Relaxed) as u32 as i32,"progressStarts":STARTS.load(Ordering::Relaxed),"progressTicks":TICKS.load(Ordering::Relaxed),"startCallbackRva":START_CALLBACK.load(Ordering::Relaxed),"progressCallers":PROGRESS_CALLERS.get(),"progress":progress,
        "connectivity":{"calls":CONNECTIVITY_CALLS.load(Ordering::Relaxed),"result":if hint_result==u64::MAX {None}else{Some(hint_result as u32 as i32)},"hint":if hint==u64::MAX {serde_json::Value::Null}else{serde_json::json!({"level":hint as u32,"initialized":hint>>32!=0})},"changes":CONNECTIVITY_CHANGES.load(Ordering::Relaxed),"changedHint":if changed_hint==u64::MAX {serde_json::Value::Null}else{serde_json::json!({"level":changed_hint as u32,"initialized":changed_hint>>32!=0})}},
        "controller":if controller==u64::MAX {serde_json::Value::Null}else{serde_json::json!({"initiated":controller&1!=0,"hasTicked":controller&2!=0})},
        "startupGate":{"calls":GATE_CALLS.load(Ordering::Relaxed),"result":if gate==u64::MAX {None}else{Some(gate)},"targetRva":GATE_TARGET.load(Ordering::Relaxed)}})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[inline(never)]
    unsafe extern "system" fn host_connectivity(output: *mut u8) -> i32 {
        if output.is_null() {
            return std::hint::black_box(0x80070005u32 as i32);
        }
        unsafe {
            std::ptr::copy_nonoverlapping(
                [3u8, 0, 0, 0, 1, 0, 0, 0, 71, 0, 0, 0, 1, 0, 1, 0].as_ptr(),
                output,
                16,
            )
        };
        std::hint::black_box(0)
    }
    #[inline(never)]
    unsafe extern "system" fn host_connectivity_changed(context: usize, input: *const u8) {
        unsafe { *(context as *mut usize) = std::hint::black_box(input as usize) };
    }
    #[test]
    fn connectivity_observer_preserves_hint_bytes_and_failed_output() {
        let init = unsafe { hudhook::mh::MH_Initialize() };
        assert!(matches!(
            init,
            MH_STATUS::MH_OK | MH_STATUS::MH_ERROR_ALREADY_INITIALIZED
        ));
        unsafe { install_connectivity(host_connectivity as *const () as usize, false).unwrap() };
        let call: Connectivity = std::hint::black_box(host_connectivity);
        let mut output = [0xabu8; 16];
        assert_eq!(unsafe { call(output.as_mut_ptr()) }, 0);
        assert_eq!(output, [3, 0, 0, 0, 1, 0, 0, 0, 71, 0, 0, 0, 1, 0, 1, 0]);
        assert_eq!(CONNECTIVITY_HINT.load(Ordering::Acquire), 0x100000003);
        assert_eq!(unsafe { call(std::ptr::null_mut()) }, 0x80070005u32 as i32);
        assert_eq!(CONNECTIVITY_HINT.load(Ordering::Acquire), 0x100000003);
        assert_eq!(CONNECTIVITY_CALLS.load(Ordering::Acquire), 2);
        unsafe {
            install_connectivity(host_connectivity_changed as *const () as usize, true).unwrap()
        };
        let changed: ConnectivityChanged = std::hint::black_box(host_connectivity_changed);
        let mut passed_input = 0usize;
        unsafe { changed((&mut passed_input as *mut usize) as usize, output.as_ptr()) };
        assert_eq!(passed_input, output.as_ptr() as usize);
        assert_eq!(
            CONNECTIVITY_CHANGED_HINT.load(Ordering::Acquire),
            0x100000003
        );
        assert_eq!(CONNECTIVITY_CHANGES.load(Ordering::Acquire), 1);
        assert_eq!(output, [3, 0, 0, 0, 1, 0, 0, 0, 71, 0, 0, 0, 1, 0, 1, 0]);
    }
    #[inline(never)]
    unsafe extern "system" fn host_controller(this: usize) -> u32 {
        std::hint::black_box(this as u32).wrapping_add(0x12345678)
    }
    #[inline(never)]
    unsafe extern "system" fn host_gate(this: usize) -> u8 {
        (std::hint::black_box(this) & 1) as u8
    }
    #[test]
    fn startup_observers_preserve_bool_and_full_dirty_flag_returns() {
        let init = unsafe { hudhook::mh::MH_Initialize() };
        assert!(matches!(
            init,
            MH_STATUS::MH_OK | MH_STATUS::MH_ERROR_ALREADY_INITIALIZED
        ));
        unsafe {
            install_startup([
                host_controller as *const () as usize,
                host_gate as *const () as usize,
            ])
            .unwrap()
        };
        let controller: Controller = std::hint::black_box(host_controller);
        let gate: Gate = std::hint::black_box(host_gate);
        assert_eq!(unsafe { controller(0) }, 0x12345678);
        assert_eq!(unsafe { controller(1) }, 0x12345679);
        assert_eq!(unsafe { gate(0) }, 0);
        assert_eq!(unsafe { gate(1) }, 1);
        assert_eq!(GATE_CALLS.load(Ordering::Acquire), 2);
        assert_eq!(GATE_RESULT.load(Ordering::Acquire), 1);
    }
    #[inline(never)]
    unsafe extern "system" fn host_start(handler: usize, output: usize) {
        unsafe { *(output as *mut usize) = std::hint::black_box(handler) ^ 1 };
    }
    #[inline(never)]
    unsafe extern "system" fn host_tick(handler: usize, output: usize) {
        unsafe { *(output as *mut usize) = std::hint::black_box(handler) ^ 2 };
    }
    #[test]
    fn progress_observers_forward_both_arguments_and_reject_invalid_flags() {
        let init = unsafe { hudhook::mh::MH_Initialize() };
        assert!(matches!(
            init,
            MH_STATUS::MH_OK | MH_STATUS::MH_ERROR_ALREADY_INITIALIZED
        ));
        unsafe {
            install_progress([
                host_start as *const () as usize,
                host_tick as *const () as usize,
            ])
            .unwrap()
        };
        let mut handler = [0u8; 0x70];
        handler[0x10] = 1;
        handler[0x12] = 1;
        handler[0x28..0x2c].copy_from_slice(&2u32.to_ne_bytes());
        let address = handler.as_ptr() as usize;
        let mut output = 0usize;
        let start: Progress = std::hint::black_box(host_start);
        let tick: Progress = std::hint::black_box(host_tick);
        unsafe { start(address, (&mut output as *mut usize) as usize) };
        assert_eq!(output, address ^ 1);
        unsafe { tick(address, (&mut output as *mut usize) as usize) };
        assert_eq!(output, address ^ 2);
        assert_eq!(FLAGS.load(Ordering::Acquire), 0x205);
        handler[0x11] = 9;
        unsafe {
            tick(
                handler.as_ptr() as usize,
                (&mut output as *mut usize) as usize,
            )
        };
        assert_eq!(FLAGS.load(Ordering::Acquire), 0x205);
        assert_eq!(STARTS.load(Ordering::Acquire), 1);
        assert_eq!(TICKS.load(Ordering::Acquire), 2);
    }
    #[inline(never)]
    unsafe extern "system" fn host(category: usize, output: usize, error: i32) -> usize {
        unsafe { *(output as *mut (usize, i32)) = (std::hint::black_box(category), error) };
        std::hint::black_box(output)
    }
    #[test]
    fn error_observer_preserves_native_output_arguments_and_return_value() {
        let init = unsafe { hudhook::mh::MH_Initialize() };
        assert!(matches!(
            init,
            MH_STATUS::MH_OK | MH_STATUS::MH_ERROR_ALREADY_INITIALIZED
        ));
        unsafe { install_target(host as *const () as usize).unwrap() };
        let mut observed = (0usize, 0i32);
        let output = (&mut observed as *mut (usize, i32)) as usize;
        let call: Message = std::hint::black_box(host);
        assert_eq!(unsafe { call(0x1234, output, -2193) }, output);
        assert_eq!(observed, (0x1234, -2193));
        assert_eq!(CALLS.load(Ordering::Acquire), 1);
        assert_eq!(LAST.load(Ordering::Acquire) as u32 as i32, -2193);
    }
}
