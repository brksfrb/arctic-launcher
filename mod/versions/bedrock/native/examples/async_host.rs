//! Verify real GDK async completion under the isolated no-user/network guard.
//! No real user provider is acquired and no sign-in or entitlement API is called.
#![cfg_attr(windows, windows_subsystem = "windows")]
#[cfg(windows)]
fn main() {
    let Some(path) = std::env::args_os().nth(1) else {
        return;
    };
    let path = std::path::PathBuf::from(path);
    let result = run(&path).unwrap_or_else(|e| {
        serde_json::json!({"error":e.to_string(),"isolation":arctic_bedrock::probe_isolation::status()})
    });
    let _ = std::fs::write(path.join("async-report.json"), result.to_string());
}
#[cfg(not(windows))]
fn main() {}

#[cfg(windows)]
fn run(root: &std::path::Path) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    use hudhook::windows::{
        Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW},
        core::{GUID, HRESULT, s, w},
    };
    use std::{
        ffi::c_void,
        sync::atomic::{AtomicU32, Ordering},
    };
    #[repr(C)]
    struct AsyncBlock {
        queue: *mut c_void,
        context: *mut c_void,
        callback: Option<unsafe extern "system" fn(*mut AsyncBlock)>,
        internal: [usize; 4],
    }
    static CALLBACKS: AtomicU32 = AtomicU32::new(0);
    static CLEANUPS: AtomicU32 = AtomicU32::new(0);
    static IDENTITY: u8 = 0;
    unsafe extern "system" fn callback(_: *mut AsyncBlock) {
        CALLBACKS.fetch_add(1, Ordering::Relaxed);
    }
    unsafe extern "system" fn provider(op: u32, _: *const c_void) -> HRESULT {
        if op == 4 {
            CLEANUPS.fetch_add(1, Ordering::Relaxed);
        }
        HRESULT(0)
    }
    let source = root.join("source");
    let profile = root.join("profile");
    std::fs::create_dir(&source)?;
    std::fs::create_dir(&profile)?;
    let empty_user = std::env::args_os()
        .nth(2)
        .is_some_and(|v| v == "--empty-user");
    let threadpool = std::env::args_os().any(|v| v == "--threadpool");
    let cancel_ui = std::env::args_os().any(|v| v == "--cancel-ui");
    if empty_user {
        let loopback = std::env::args_os().any(|v| v == "--loopback");
        arctic_bedrock::probe_isolation::install_empty_user(&profile, &source, loopback)?;
    } else {
        arctic_bedrock::probe_isolation::install(&profile, &source)?;
    }
    type Api = unsafe extern "system" fn(*const GUID, *const GUID, *mut *mut c_void) -> HRESULT;
    unsafe {
        let gdk = LoadLibraryW(w!("xgameruntime.dll"))?;
        let api: Api = std::mem::transmute(
            GetProcAddress(gdk, s!("QueryApiImpl")).ok_or("GDK query export missing")?,
        );
        let network = if std::env::args_os().any(|v| v == "--network") {
            // Exact SDK/runtime version arguments observed in the pinned game's
            // XGameRuntimeInitialize wrapper. No game code or user API is called.
            type Initialize = unsafe extern "system" fn(u64, u64, u64, *const c_void) -> HRESULT;
            let initialize: Initialize = std::mem::transmute(
                GetProcAddress(gdk, s!("InitializeApiImplEx2"))
                    .ok_or("GDK initialization export missing")?,
            );
            // The explicit initializer supplies mode 0 to the wrapper, which
            // adds bit 2. Lazy QueryApiImpl initialization supplies mode 1 and
            // returns NOT_INITIALIZED in a fresh standalone process.
            let initialized = initialize(0x633600002712, 0x65f400000ff4, 2, std::ptr::null());
            initialized
                .ok()
                .map_err(|e| format!("GDK runtime initialization failed: {e}"))?;
            let class = GUID::from_u128(0x37e56907_2f10_41e8_b72f_36edb185331a);
            let version = GUID::from_u128(0xbf2346b2_39af_4658_b5ea_44713c7e83b3);
            let mut network = std::ptr::null_mut();
            let queried = api(&class, &version, &mut network);
            queried
                .ok()
                .map_err(|e| format!("GDK networking query failed: {e}"))?;
            if network.is_null() {
                return Err(format!(
                    "initialized networking provider is null: {initialized:?}, {queried:?}"
                )
                .into());
            }
            #[repr(C)]
            #[derive(Default)]
            struct Hint {
                level: u32,
                cost: u32,
                interface_type: u32,
                initialized: u8,
                near_limit: u8,
                over_limit: u8,
                roaming: u8,
            }
            type Get = unsafe extern "system" fn(*mut c_void, *mut Hint) -> HRESULT;
            type Release = unsafe extern "system" fn(*mut c_void) -> u32;
            let table = *(network as *const *const usize);
            let get: Get = std::mem::transmute(*table.add(15));
            let release: Release = std::mem::transmute(*table.add(2));
            let mut hint = Hint::default();
            let result = get(network, &mut hint);
            release(network);
            result
                .ok()
                .map_err(|e| format!("GDK connectivity hint failed: {e}"))?;
            Some(
                serde_json::json!({"initialized":hint.initialized,"level":hint.level,"result":result.0}),
            )
        } else {
            None
        };
        let class = GUID::from_u128(0x073b7dcb_1fcf_4030_94be_e3c9eb623428);
        let mut object = std::ptr::null_mut();
        api(&class, &class, &mut object).ok()?;
        if object.is_null() {
            return Err("GDK threading provider is null".into());
        }
        let table = *(object as *const *const usize);
        type Create = unsafe extern "system" fn(*mut c_void, u32, u32, *mut *mut c_void) -> HRESULT;
        type Begin = unsafe extern "system" fn(
            *mut c_void,
            *mut AsyncBlock,
            *mut c_void,
            *const u8,
            *const u8,
            unsafe extern "system" fn(u32, *const c_void) -> HRESULT,
        ) -> HRESULT;
        type Complete = unsafe extern "system" fn(*mut c_void, *mut AsyncBlock, HRESULT, usize);
        type Status = unsafe extern "system" fn(*mut c_void, *mut AsyncBlock, bool) -> HRESULT;
        type Result = unsafe extern "system" fn(
            *mut c_void,
            *mut AsyncBlock,
            *const u8,
            usize,
            *mut c_void,
            *mut usize,
        ) -> HRESULT;
        type Dispatch = unsafe extern "system" fn(*mut c_void, *mut c_void, u32, u32) -> bool;
        type Close = unsafe extern "system" fn(*mut c_void, *mut c_void);
        type Release = unsafe extern "system" fn(*mut c_void) -> u32;
        let create: Create = std::mem::transmute(*table.add(12));
        let begin: Begin = std::mem::transmute(*table.add(7));
        let complete: Complete = std::mem::transmute(*table.add(10));
        let status: Status = std::mem::transmute(*table.add(3));
        let result: Result = std::mem::transmute(*table.add(11));
        let dispatch: Dispatch = std::mem::transmute(*table.add(16));
        let close: Close = std::mem::transmute(*table.add(17));
        let release: Release = std::mem::transmute(*table.add(2));
        let mut queue = std::ptr::null_mut();
        let dispatch_mode = u32::from(threadpool);
        create(object, dispatch_mode, dispatch_mode, &mut queue).ok()?;
        let mut block = AsyncBlock {
            queue,
            context: std::ptr::null_mut(),
            callback: Some(callback),
            internal: [0; 4],
        };
        let mut user = std::ptr::null_mut();
        if empty_user {
            let class = GUID::from_u128(0x01acd177_91f9_4763_a38e_ccbb55ce32e0);
            api(&class, &class, &mut user).ok()?;
            if user.is_null() {
                return Err("empty user provider is null".into());
            }
            let methods = *(user as *const *const usize);
            type Add = unsafe extern "system" fn(*mut c_void, u32, *mut AsyncBlock) -> HRESULT;
            type MaxUsers = unsafe extern "system" fn(*mut c_void, *mut u32) -> HRESULT;
            type Find = unsafe extern "system" fn(*mut c_void, u64, *mut *mut c_void) -> HRESULT;
            let capacity: MaxUsers = std::mem::transmute(*methods.add(6));
            let find: Find = std::mem::transmute(*methods.add(12));
            let mut handle = std::ptr::dangling_mut::<c_void>();
            let mut max_users = 0;
            capacity(user, &mut max_users).ok()?;
            if max_users != 1
                || find(user, 0, &mut handle) != HRESULT(0x89245104u32 as i32)
                || !handle.is_null()
            {
                return Err("empty provider returned an invalid capacity or a user".into());
            }
            // Exercise the separate device-association factory as well. It
            // must expose an empty set instead of falling through to real users.
            let class = GUID::from_u128(0x7d824997_10dc_45ab_86b7_2737767c0bf1);
            let version = GUID::from_u128(0x0cc6a956_e7e1_4fdf_9341_9d5da649ebc8);
            let mut device = std::ptr::null_mut();
            api(&class, &version, &mut device).ok()?;
            if device.is_null() {
                return Err("empty device provider is null".into());
            }
            let device_methods = *(device as *const *const usize);
            type DeviceFind =
                unsafe extern "system" fn(*mut c_void, *const u8, *mut *mut c_void) -> HRESULT;
            type Register = unsafe extern "system" fn(
                *mut c_void,
                *mut c_void,
                *mut c_void,
                unsafe extern "system" fn(*mut c_void, *const c_void),
                *mut u64,
            ) -> HRESULT;
            type Unregister = unsafe extern "system" fn(*mut c_void, u64, bool) -> bool;
            unsafe extern "system" fn device_changed(_: *mut c_void, _: *const c_void) {
                CALLBACKS.fetch_add(1000, Ordering::Relaxed);
            }
            let find_device: DeviceFind = std::mem::transmute(*device_methods.add(3));
            let register: Register = std::mem::transmute(*device_methods.add(4));
            let unregister: Unregister = std::mem::transmute(*device_methods.add(5));
            handle = std::ptr::dangling_mut::<c_void>();
            if find_device(device, [0u8; 32].as_ptr(), &mut handle) != HRESULT(0x89245104u32 as i32)
                || !handle.is_null()
            {
                return Err("empty device provider returned a user".into());
            }
            let mut token = 0;
            register(
                device,
                queue,
                std::ptr::null_mut(),
                device_changed,
                &mut token,
            )
            .ok()?;
            if token == 0 || !unregister(device, token, true) {
                return Err("device change registration did not complete".into());
            }
            let device_release: Release = std::mem::transmute(*device_methods.add(2));
            device_release(device);
            let add: Add = std::mem::transmute(*methods.add(7));
            add(user, if cancel_ui { 4 } else { 1 }, &mut block).ok()?;
        } else {
            begin(
                object,
                &mut block,
                std::ptr::null_mut(),
                &IDENTITY,
                c"Arctic signed-out completion test".as_ptr().cast(),
                provider,
            )
            .ok()?;
        }
        let failure = HRESULT(if empty_user && cancel_ui {
            0x80004004u32
        } else {
            0x89245106u32
        } as i32);
        if !empty_user {
            complete(object, &mut block, failure, 0);
        }
        let observed = status(object, &mut block, false);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while CALLBACKS.load(Ordering::Acquire) == 0 && std::time::Instant::now() < deadline {
            if !threadpool {
                dispatch(object, queue, 1, 0);
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let outcome = if empty_user {
            type UserResult = unsafe extern "system" fn(
                *mut c_void,
                *mut AsyncBlock,
                *mut *mut c_void,
            ) -> HRESULT;
            let methods = *(user as *const *const usize);
            let add_result: UserResult = std::mem::transmute(*methods.add(8));
            let mut handle = std::ptr::dangling_mut::<c_void>();
            let outcome = add_result(user, &mut block, &mut handle);
            let user_release: Release = std::mem::transmute(*methods.add(2));
            user_release(user);
            if !handle.is_null() {
                return Err("empty provider returned a user handle".into());
            }
            outcome
        } else {
            result(
                object,
                &mut block,
                &IDENTITY,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        close(object, queue);
        release(object);
        if observed != failure || outcome != failure || CALLBACKS.load(Ordering::Acquire) != 1 {
            return Err(format!(
                "async completion mismatch: {observed:?}, {outcome:?}, callbacks {}",
                CALLBACKS.load(Ordering::Acquire)
            )
            .into());
        }
        Ok(
            serde_json::json!({"passed":true,"network":network,"emptyUserProvider":empty_user,"threadpool":threadpool,"status":observed.0,"result":outcome.0,"callbacks":CALLBACKS.load(Ordering::Acquire),"cleanups":CLEANUPS.load(Ordering::Acquire),"isolation":arctic_bedrock::probe_isolation::status()}),
        )
    }
}
