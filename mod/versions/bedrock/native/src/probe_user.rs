//! Test-only empty XUser provider. It never calls the real user provider, creates
//! a user handle/token, or changes Store/entitlement APIs. Failed additions use
//! the real GDK XThreading async machinery so callbacks and cleanup complete.
//! Public COM layouts: xodus-gaming/xgameruntime-docs; XAsync ABI: Microsoft's
//! MIT libHttpClient headers. See THIRD_PARTY.md.
use hudhook::windows::core::{GUID, HRESULT};
use std::{
    ffi::c_void,
    sync::{
        OnceLock,
        atomic::{AtomicU32, AtomicU64, Ordering},
    },
};

const NO_DEFAULT_USER: HRESULT = HRESULT(0x89245106u32 as i32);
const USER_NOT_FOUND: HRESULT = HRESULT(0x89245104u32 as i32);
const CANCELED: HRESULT = HRESULT(0x80004004u32 as i32);
const DENIED: HRESULT = HRESULT(0x80070005u32 as i32);
const POINTER: HRESULT = HRESULT(0x80004003u32 as i32);
const NO_INTERFACE: HRESULT = HRESULT(0x80004002u32 as i32);
const THREAD_CLASS: GUID = GUID::from_u128(0x073b7dcb_1fcf_4030_94be_e3c9eb623428);
const USER_IIDS: [GUID; 7] = [
    GUID::from_u128(0x00000000_0000_0000_c000_000000000046),
    GUID::from_u128(0x01acd177_91f9_4763_a38e_ccbb55ce32e0),
    GUID::from_u128(0xeb9bf948_18dc_4d82_bbcc_40e0a809c4c0),
    GUID::from_u128(0x1bf2f8c5_d507_4e52_bb05_f726d0e71161),
    GUID::from_u128(0x079415e3_6727_437f_8e9d_8f8f9b2439f7),
    GUID::from_u128(0x26f3c674_a2fe_44fa_b6c4_a323bc94ff53),
    GUID::from_u128(0x5131d685_4394_4ee6_8c18_bfb5d4aef1ff),
];
static API: OnceLock<super::Api> = OnceLock::new();
static THREADING: OnceLock<Result<Threading, HRESULT>> = OnceLock::new();
static TABLE: OnceLock<Box<[usize; 50]>> = OnceLock::new();
static OBJECT: OnceLock<User> = OnceLock::new();
static DEVICE_TABLE: OnceLock<Box<[usize; 11]>> = OnceLock::new();
static DEVICE_OBJECT: OnceLock<User> = OnceLock::new();
static REFERENCES: AtomicU32 = AtomicU32::new(1);
static REGISTRATION: AtomicU64 = AtomicU64::new(1);
static IDENTITY: u8 = 0;
static CAPACITY_QUERIES: AtomicU64 = AtomicU64::new(0);
static ADDITIONS: AtomicU64 = AtomicU64::new(0);
static RESULTS: AtomicU64 = AtomicU64::new(0);
static CLEANUPS: AtomicU64 = AtomicU64::new(0);
static LAST_OPTIONS: AtomicU32 = AtomicU32::new(0);
static DEVICE_LOOKUPS: AtomicU64 = AtomicU64::new(0);
pub(super) fn status() -> serde_json::Value {
    serde_json::json!({"capacityQueries":CAPACITY_QUERIES.load(Ordering::Relaxed),"additions":ADDITIONS.load(Ordering::Relaxed),"lastOptions":LAST_OPTIONS.load(Ordering::Relaxed),"results":RESULTS.load(Ordering::Relaxed),"cleanups":CLEANUPS.load(Ordering::Relaxed),"deviceLookups":DEVICE_LOOKUPS.load(Ordering::Relaxed)})
}
#[repr(C)]
struct User {
    table: usize,
}

// These HRESULT-only methods have several public signatures. A Win64 assembly
// leaf ignores all argument registers/stack slots and returns one failure;
// it has no Rust parameter values, borrowed outputs, or stack/unwind state.
#[cfg(target_arch = "x86_64")]
core::arch::global_asm!(
    ".global arctic_empty_user_denied",
    "arctic_empty_user_denied:",
    "mov eax, 0x80070005",
    "ret",
);
#[cfg(target_arch = "x86_64")]
unsafe extern "system" {
    fn arctic_empty_user_denied();
}

pub(super) fn prepare(api: super::Api) -> Result<(), String> {
    API.set(api)
        .map_err(|_| "empty user provider already initialized".into())
}
pub(super) unsafe fn query(iid: *const GUID, out: *mut *mut c_void) -> HRESULT {
    if out.is_null() {
        return POINTER;
    }
    unsafe {
        *out = std::ptr::null_mut();
    }
    if !super::read(iid).is_some_and(|id| USER_IIDS.contains(&id)) {
        return NO_INTERFACE;
    }
    let object = OBJECT.get_or_init(|| User {
        table: table().as_ptr() as usize,
    });
    unsafe {
        *out = (object as *const User).cast_mut().cast();
        add_ref(0);
    }
    HRESULT(0)
}
fn table() -> &'static [usize; 50] {
    TABLE.get_or_init(|| {
        #[cfg(target_arch = "x86_64")]
        let failure = arctic_empty_user_denied as *const () as usize;
        #[cfg(not(target_arch = "x86_64"))]
        let failure = unsupported as *const () as usize;
        let mut table = Box::new([failure; 50]);
        for (slot, method) in [
            (0, query_interface as *const () as usize),
            (1, add_ref as *const () as usize),
            (2, release as *const () as usize),
            (3, duplicate as *const () as usize),
            (4, close as *const () as usize),
            (5, compare as *const () as usize),
            (6, max_users as *const () as usize),
            (7, add_async as *const () as usize),
            (8, add_result as *const () as usize),
            (10, find as *const () as usize),
            (12, find as *const () as usize),
            (33, register as *const () as usize),
            (34, unregister as *const () as usize),
            (36, close as *const () as usize),
            (37, add_by_id as *const () as usize),
            (38, add_result as *const () as usize),
            (42, is_store_user as *const () as usize),
            (47, is_signout_present as *const () as usize),
        ] {
            table[slot] = method;
        }
        table
    })
}
pub(super) unsafe fn query_device(iid: *const GUID, out: *mut *mut c_void) -> HRESULT {
    if out.is_null() {
        return POINTER;
    }
    unsafe { *out = std::ptr::null_mut() };
    let interfaces = [
        USER_IIDS[0],
        GUID::from_u128(0x7d824997_10dc_45ab_86b7_2737767c0bf1),
        GUID::from_u128(0x0cc6a956_e7e1_4fdf_9341_9d5da649ebc8),
    ];
    if !super::read(iid).is_some_and(|id| interfaces.contains(&id)) {
        return NO_INTERFACE;
    }
    let methods = DEVICE_TABLE.get_or_init(|| {
        #[cfg(target_arch = "x86_64")]
        let failure = arctic_empty_user_denied as *const () as usize;
        #[cfg(not(target_arch = "x86_64"))]
        let failure = unsupported as *const () as usize;
        let mut table = Box::new([failure; 11]);
        for (slot, method) in [
            (0, device_query_interface as *const () as usize),
            (1, add_ref as *const () as usize),
            (2, release as *const () as usize),
            (3, device_find as *const () as usize),
            (4, register as *const () as usize),
            (5, unregister as *const () as usize),
            (7, register as *const () as usize),
            (8, unregister as *const () as usize),
        ] {
            table[slot] = method;
        }
        table
    });
    let object = DEVICE_OBJECT.get_or_init(|| User {
        table: methods.as_ptr() as usize,
    });
    unsafe {
        *out = (object as *const User).cast_mut().cast();
        add_ref(0);
    }
    HRESULT(0)
}
unsafe extern "system" fn device_query_interface(
    _: usize,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    unsafe { query_device(iid, out) }
}
unsafe extern "system" fn device_find(
    this: usize,
    _device: *const c_void,
    out: *mut *mut c_void,
) -> HRESULT {
    DEVICE_LOOKUPS.fetch_add(1, Ordering::Relaxed);
    unsafe { find(this, 0, out) }
}
#[cfg(not(target_arch = "x86_64"))]
unsafe extern "system" fn unsupported() -> HRESULT {
    DENIED
}
unsafe extern "system" fn query_interface(
    _: usize,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    unsafe { query(iid, out) }
}
unsafe extern "system" fn add_ref(_: usize) -> u32 {
    REFERENCES
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
            Some(v.saturating_add(1))
        })
        .unwrap()
        .saturating_add(1)
}
unsafe extern "system" fn release(_: usize) -> u32 {
    REFERENCES
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
            Some(v.saturating_sub(1).max(1))
        })
        .unwrap()
        .saturating_sub(1)
        .max(1)
}
unsafe extern "system" fn close(_: usize, _: usize) {}
unsafe extern "system" fn compare(_: usize, a: usize, b: usize) -> i32 {
    match a.cmp(&b) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}
unsafe extern "system" fn max_users(_: usize, capacity: *mut u32) -> HRESULT {
    if capacity.is_null() {
        return POINTER;
    }
    CAPACITY_QUERIES.fetch_add(1, Ordering::Relaxed);
    unsafe {
        *capacity = 1;
    } // capacity, never a count of signed-in users
    HRESULT(0)
}
unsafe extern "system" fn is_store_user(_: usize, _: usize) -> bool {
    false
}
unsafe extern "system" fn is_signout_present(_: usize) -> bool {
    false
}
unsafe extern "system" fn duplicate(_: usize, _: usize, out: *mut *mut c_void) -> HRESULT {
    if out.is_null() {
        return POINTER;
    }
    unsafe {
        *out = std::ptr::null_mut();
    }
    super::USER_DENIALS.fetch_add(1, Ordering::Relaxed);
    DENIED
}
unsafe extern "system" fn find(_: usize, _: u64, out: *mut *mut c_void) -> HRESULT {
    if out.is_null() {
        return POINTER;
    }
    unsafe {
        *out = std::ptr::null_mut();
    }
    super::USER_DENIALS.fetch_add(1, Ordering::Relaxed);
    USER_NOT_FOUND
}
unsafe extern "system" fn register(
    _: usize,
    _queue: usize,
    _context: usize,
    _callback: usize,
    token: *mut u64,
) -> HRESULT {
    if token.is_null() {
        return POINTER;
    }
    let value = REGISTRATION.fetch_add(1, Ordering::Relaxed);
    if value > 65535 {
        return DENIED;
    }
    // No users can ever be acquired in this provider, so there are no change
    // notifications or retained callbacks/contexts to drain on unregister.
    unsafe {
        *token = value;
    }
    HRESULT(0)
}
unsafe extern "system" fn unregister(_: usize, token: u64, _wait: bool) -> bool {
    token > 0 && token < REGISTRATION.load(Ordering::Relaxed)
}
type Begin = unsafe extern "system" fn(
    usize,
    *mut c_void,
    *mut c_void,
    *const u8,
    *const u8,
    unsafe extern "system" fn(u32, *const c_void) -> HRESULT,
) -> HRESULT;
type Complete = unsafe extern "system" fn(usize, *mut c_void, HRESULT, usize);
type GetResult = unsafe extern "system" fn(
    usize,
    *mut c_void,
    *const u8,
    usize,
    *mut c_void,
    *mut usize,
) -> HRESULT;
struct Threading {
    object: usize,
    begin: Begin,
    complete: Complete,
    result: GetResult,
}
fn threading() -> Result<&'static Threading, HRESULT> {
    THREADING
        .get_or_init(|| {
            let api = API.get().ok_or(DENIED)?;
            let mut object = std::ptr::null_mut();
            let result = unsafe { api(&THREAD_CLASS, &THREAD_CLASS, &mut object) };
            if result.0 < 0 {
                return Err(result);
            }
            let table = crate::world_hooks::pointer(object as usize).ok_or(POINTER)?;
            let method = |slot: usize| crate::world_hooks::pointer(table + slot * 8).ok_or(POINTER);
            // One non-user GDK threading reference is retained for this owned test
            // process, keeping queued completion machinery alive until shutdown.
            Ok(Threading {
                object: object as usize,
                begin: unsafe { std::mem::transmute::<usize, Begin>(method(7)?) },
                complete: unsafe { std::mem::transmute::<usize, Complete>(method(10)?) },
                result: unsafe { std::mem::transmute::<usize, GetResult>(method(11)?) },
            })
        })
        .as_ref()
        .map_err(|e| *e)
}
unsafe extern "system" fn provider(op: u32, _: *const c_void) -> HRESULT {
    if op == 4 {
        CLEANUPS.fetch_add(1, Ordering::Relaxed);
    }
    if matches!(op, 0 | 3 | 4) {
        HRESULT(0)
    } else {
        DENIED
    }
}
fn start(block: *mut c_void, failure: HRESULT) -> HRESULT {
    if block.is_null() {
        return POINTER;
    }
    super::USER_DENIALS.fetch_add(1, Ordering::Relaxed);
    ADDITIONS.fetch_add(1, Ordering::Relaxed);
    let threading = match threading() {
        Ok(v) => v,
        Err(e) => return e,
    };
    unsafe {
        let result = (threading.begin)(
            threading.object,
            block,
            std::ptr::null_mut(),
            &IDENTITY,
            c"Arctic isolated signed-out user".as_ptr().cast(),
            provider,
        );
        if result.0 < 0 {
            return result;
        }
        (threading.complete)(threading.object, block, failure, 0);
    }
    HRESULT(0)
}
unsafe extern "system" fn add_async(_: usize, options: u32, block: *mut c_void) -> HRESULT {
    LAST_OPTIONS.store(options, Ordering::Relaxed);
    // The headless test has no sign-in UI. Silent lookup finds no default;
    // requests that would show account selection finish as canceled instead.
    start(
        block,
        if options & 1 != 0 {
            NO_DEFAULT_USER
        } else {
            CANCELED
        },
    )
}
unsafe extern "system" fn add_by_id(_: usize, _id: u64, block: *mut c_void) -> HRESULT {
    start(block, CANCELED)
}
unsafe extern "system" fn add_result(
    _: usize,
    block: *mut c_void,
    out: *mut *mut c_void,
) -> HRESULT {
    RESULTS.fetch_add(1, Ordering::Relaxed);
    if out.is_null() {
        return POINTER;
    }
    unsafe {
        *out = std::ptr::null_mut();
    }
    let threading = match threading() {
        Ok(v) => v,
        Err(e) => return e,
    };
    unsafe {
        (threading.result)(
            threading.object,
            block,
            &IDENTITY,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    }
}
