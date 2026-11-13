//! Process-local isolation for the opt-in, pre-entry installed-game probe.
//! This is test instrumentation, not a license/entitlement implementation.
//! Source-profile file opens are redirected to a fresh profile. Network-device
//! creation/IO and GDK user acquisition fail normally; no user/token is created.
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use hudhook::mh::{MH_CreateHook, MH_EnableHook, MH_Initialize, MH_STATUS};
use hudhook::windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use hudhook::windows::core::{GUID, HRESULT, s, w};
use serde_json::{Value, json};
#[path = "probe_errors.rs"]
mod errors;
#[path = "probe_join.rs"]
mod join;
#[path = "probe_network.rs"]
mod network;
#[path = "probe_user.rs"]
mod user;

/// The caller has already checked the exact installed image fingerprint.
pub(crate) unsafe fn observe_errors(base: usize) -> Result<(), String> {
    if STATE.get().is_some() {
        unsafe { errors::install(base) }?;
        unsafe { join::install(base) }?;
    }
    Ok(())
}

const DENIED: i32 = 0xc0000022u32 as i32;
const USER_CLASS: GUID = GUID::from_u128(0x01acd177_91f9_4763_a38e_ccbb55ce32e0);
const USER_DEVICE_CLASS: GUID = GUID::from_u128(0x7d824997_10dc_45ab_86b7_2737767c0bf1);
static STATE: OnceLock<Isolation> = OnceLock::new();
static ORIGINALS: OnceLock<Originals> = OnceLock::new();
static REDIRECTS: AtomicU64 = AtomicU64::new(0);
static NETWORK_DENIALS: AtomicU64 = AtomicU64::new(0);
static NETWORK_LAST_IO: AtomicU64 = AtomicU64::new(0);
static NETWORK_METADATA: AtomicU64 = AtomicU64::new(0);
static USER_QUERIES: AtomicU64 = AtomicU64::new(0);
static USER_DEVICE_QUERIES: AtomicU64 = AtomicU64::new(0);
static USER_DENIALS: AtomicU64 = AtomicU64::new(0);
static SNAPSHOT: AtomicU64 = AtomicU64::new(0);
static PATH_DENIALS: AtomicU64 = AtomicU64::new(0);
static NAME_FAILURES: AtomicU64 = AtomicU64::new(0);
static ROOT_QUERY: AtomicU64 = AtomicU64::new(0);

/// This sidecar is created only by inspect-initialization.ps1 -IsolatedBoot.
/// Ordinary bundles do not enable process instrumentation.
pub(crate) fn optional(directory: &Path) -> Result<bool, String> {
    let path = directory.join("arctic-offline-probe.json");
    if !path.exists() {
        return Ok(false);
    }
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Config {
        schema: u32,
        profile: PathBuf,
        mode: String,
        seed_probe: Option<PathBuf>,
        #[serde(default)]
        empty_user_provider: bool,
        #[serde(default)]
        headless_present: bool,
        #[serde(default)]
        auto_world: bool,
        #[serde(default)]
        body_probe: bool,
        #[serde(default)]
        lan_record_probe: bool,
        #[serde(default)]
        replay_probe: bool,
    }
    let config: Config = serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    if config.body_probe && !config.auto_world {
        return Err("body probe requires automatic isolated world entry".into());
    }
    if config.lan_record_probe && !config.auto_world {
        return Err("LAN recording probe requires automatic isolated world entry".into());
    }
    if config.replay_probe && (!config.auto_world || config.lan_record_probe || config.body_probe) {
        return Err("native replay probe requires separate automatic isolated world entry".into());
    }
    if config.auto_world
        && (config.seed_probe.is_none()
            || !config.empty_user_provider
            || config.mode != "loopback-and-deny-user-provider")
    {
        return Err(
            "automatic world probe requires a seeded loopback/empty-user configuration".into(),
        );
    }
    if config.schema != 1
        || !matches!(
            config.mode.as_str(),
            "deny-network-and-user-provider" | "loopback-and-deny-user-provider"
        )
        || config.profile.canonicalize().map_err(|e| e.to_string())?
            != directory
                .join("game-profile")
                .canonicalize()
                .map_err(|e| e.to_string())?
    {
        return Err("invalid isolated-probe configuration".into());
    }
    if std::fs::read_dir(&config.profile)
        .map_err(|e| e.to_string())?
        .next()
        .is_some()
    {
        return Err("isolated game profile must start empty".into());
    }
    let source = std::env::var_os("USERPROFILE").ok_or("source profile unavailable")?;
    install_mode(
        &config.profile,
        &PathBuf::from(source),
        config.mode == "loopback-and-deny-user-provider",
        config.empty_user_provider,
    )?;
    if config.lan_record_probe {
        network::enable_lan_bind_translation();
    }
    if config.replay_probe {
        network::enable_replay_discovery();
        join::configure(directory)?;
    }
    if let Some(seed) = config.seed_probe {
        seed_profile(directory, &seed, &config.profile)?;
    }
    let options = config.profile.join(
        "AppData/Roaming/Minecraft Bedrock/Users/Shared/games/com.mojang/minecraftpe/options.txt",
    );
    std::fs::create_dir_all(options.parent().ok_or("missing test options parent")?)
        .map_err(|e| e.to_string())?;
    let mut text = std::fs::read_to_string(&options).unwrap_or_default();
    if text.len() > 1024 * 1024 {
        return Err("test options exceed limit".into());
    }
    for (key, value) in [
        ("mp_username", "ArcticTest"),
        ("game_thirdperson", "0"),
        ("gfx_viewdistance", "128"),
        ("gfx_max_framerate", "30"),
        ("gfx_fullscreen", "0"),
        ("graphics_mode", "0"),
        ("gfx_msaa", "1"),
        ("audio_main", "0"),
        ("audio_music", "0"),
        ("audio_sound", "0"),
        ("audio_texttospeech", "0"),
        (
            "mp_server_visible",
            if config.lan_record_probe { "1" } else { "0" },
        ),
        ("pause_feature_enabled", "0"),
        ("pause_menu_on_focus_lost", "0"),
    ] {
        let field = format!("{key}:");
        text = text
            .lines()
            .filter(|line| !line.starts_with(&field))
            .map(|line| format!("{line}\n"))
            .collect::<String>();
        text.push_str(&format!("{key}:{value}\n"));
    }
    std::fs::write(options, text).map_err(|e| e.to_string())?;
    if config.headless_present {
        hudhook::hooks::dx12::set_present_result_filter(headless_present)
            .map_err(|_| "headless presentation adapter already installed")?;
    }
    Ok(true)
}

static HEADLESS_OCCLUSIONS: AtomicU64 = AtomicU64::new(0);
fn headless_present(_: u32, result: HRESULT) -> HRESULT {
    // Isolated test only: keep the game pump running on an unshown desktop.
    // Actual GPU rendering/readback still runs, and DX12 diagnostics retain the
    // real occlusion HRESULT. Never replace device errors or other status codes.
    if result.0 == 0x087a0001 {
        HEADLESS_OCCLUSIONS.fetch_add(1, Ordering::Relaxed);
        HRESULT(0)
    } else {
        result
    }
}

fn seed_profile(directory: &Path, seed: &Path, target: &Path) -> Result<(), String> {
    let seed = seed.canonicalize().map_err(|e| e.to_string())?;
    if seed == directory.canonicalize().map_err(|e| e.to_string())?
        || seed.parent()
            != directory
                .canonicalize()
                .map_err(|e| e.to_string())?
                .parent()
        || !seed
            .file_name()
            .is_some_and(|s| s.to_string_lossy().starts_with("early-initialization-"))
    {
        return Err("seed must be a previous sibling probe directory".into());
    }
    let bytes = std::fs::read(seed.join("report.json")).map_err(|e| e.to_string())?;
    if bytes.len() > 1024 * 1024 {
        return Err("seed report exceeds limit".into());
    }
    let report: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let isolation = &report["target"]["engineObserverReport"]["probeIsolation"];
    let profile = seed
        .join("game-profile")
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if report["gameEntryPointResumed"] != true
        || !matches!(
            isolation["networkMode"].as_str(),
            Some("deny-all" | "loopback-only")
        )
        || !matches!(
            isolation["userMode"].as_str(),
            Some("deny-provider" | "empty-provider")
        )
        || isolation["licenseChanged"] != false
        || isolation["profile"]
            .as_str()
            .map(PathBuf::from)
            .and_then(|p| p.canonicalize().ok())
            .as_ref()
            != Some(&profile)
    {
        return Err("seed lacks a completed isolated offline probe report".into());
    }
    use hudhook::windows::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    if let Some(pid) = report["target"]["pid"]
        .as_u64()
        .and_then(|v| u32::try_from(v).ok())
    {
        unsafe {
            match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
                Ok(handle) => {
                    let mut exit = 259;
                    let result = GetExitCodeProcess(handle, &mut exit);
                    let _ = hudhook::windows::Win32::Foundation::CloseHandle(handle);
                    if result.is_err() || exit == 259 {
                        return Err("seed probe process is still live or unverifiable".into());
                    }
                }
                Err(error) if error.code() == HRESULT::from_win32(87) => {}
                Err(error) => {
                    return Err(format!("seed probe process cannot be verified: {error}"));
                }
            }
        }
    } else {
        return Err("seed report has no owned process ID".into());
    }
    let mut budget = (0usize, 0u64);
    for relative in [
        "AppData/Roaming/Minecraft Bedrock",
        "AppData/Local/D3DSCache",
        "AppData/Local/NVIDIA/DXCache",
        "AppData/LocalLow/NVIDIA/DXCache",
        "AppData/LocalLow/Intel/ShaderCache",
    ] {
        let source = profile.join(relative);
        if source.is_dir() {
            copy_tree(&source, &target.join(relative), 0, &mut budget)?;
        }
    }
    Ok(())
}
fn copy_tree(
    source: &Path,
    target: &Path,
    depth: usize,
    budget: &mut (usize, u64),
) -> Result<(), String> {
    use std::os::windows::fs::MetadataExt;
    if depth > 16 {
        return Err("test seed nesting limit exceeded".into());
    }
    let metadata = std::fs::symlink_metadata(source).map_err(|e| e.to_string())?;
    if metadata.file_attributes() & 0x400 != 0 {
        return Err("reparse points are not permitted in test seeds".into());
    }
    if metadata.is_dir() {
        std::fs::create_dir_all(target).map_err(|e| e.to_string())?;
        for entry in std::fs::read_dir(source).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            copy_tree(
                &entry.path(),
                &target.join(entry.file_name()),
                depth + 1,
                budget,
            )?;
        }
    } else if metadata.is_file() {
        budget.0 += 1;
        budget.1 = budget
            .1
            .checked_add(metadata.len())
            .ok_or("seed size overflow")?;
        if budget.0 > 16384 || budget.1 > 512 * 1024 * 1024 {
            return Err("test seed exceeds bounded copy budget".into());
        }
        std::fs::copy(source, target).map_err(|e| e.to_string())?;
    } else {
        return Err("unsupported test seed file type".into());
    }
    Ok(())
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Unicode {
    length: u16,
    maximum: u16,
    buffer: *const u16,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Attributes {
    length: u32,
    root: *mut c_void,
    name: *const Unicode,
    flags: u32,
    security: *mut c_void,
    qos: *mut c_void,
}
type Create = unsafe extern "system" fn(
    *mut *mut c_void,
    u32,
    *const Attributes,
    *mut c_void,
    *const i64,
    u32,
    u32,
    u32,
    u32,
    *mut c_void,
    u32,
) -> i32;
type Open = unsafe extern "system" fn(
    *mut *mut c_void,
    u32,
    *const Attributes,
    *mut c_void,
    u32,
    u32,
) -> i32;
type Query = unsafe extern "system" fn(*const Attributes, *mut c_void) -> i32;
type Delete = unsafe extern "system" fn(*const Attributes) -> i32;
type SetInfo = unsafe extern "system" fn(*mut c_void, *mut c_void, *const c_void, u32, u32) -> i32;
type Io = unsafe extern "system" fn(
    *mut c_void,
    *mut c_void,
    *mut c_void,
    *mut c_void,
    *mut c_void,
    u32,
    *mut c_void,
    u32,
    *mut c_void,
    u32,
) -> i32;
type Api = unsafe extern "system" fn(*const GUID, *const GUID, *mut *mut c_void) -> HRESULT;
struct Originals {
    create: Create,
    open: Open,
    query: Query,
    query_full: Query,
    delete: Delete,
    set_info: SetInfo,
    io: Io,
    api: Api,
}
struct Isolation {
    source: Vec<Vec<u16>>,
    target: Vec<u16>,
    root: PathBuf,
    at: Instant,
    loopback: bool,
    empty_user_provider: bool,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn QueryDosDeviceW(name: *const u16, target: *mut u16, size: u32) -> u32;
    fn GetFinalPathNameByHandleW(handle: *mut c_void, path: *mut u16, size: u32, flags: u32)
    -> u32;
    fn SetEnvironmentVariableW(name: *const u16, value: *const u16) -> i32;
}
#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtQueryObject(
        handle: *mut c_void,
        class: u32,
        information: *mut c_void,
        length: u32,
        returned: *mut u32,
    ) -> i32;
    fn NtQueryVolumeInformationFile(
        handle: *mut c_void,
        status: *mut usize,
        information: *mut u32,
        length: u32,
        class: u32,
    ) -> i32;
}

fn root_name(handle: *mut c_void) -> Option<Vec<u16>> {
    let mut full = vec![0u16; 32768];
    let n = unsafe { GetFinalPathNameByHandleW(handle, full.as_mut_ptr(), full.len() as u32, 2) }
        as usize;
    if n > 0 && n < full.len() {
        full.truncate(n);
        return Some(full);
    }
    // A named-pipe instance can have no ObjectNameInformation at all. Query
    // its actual device type: only FILE_DEVICE_NAMED_PIPE is admitted here,
    // never AFD/network devices or an unresolved filesystem root. This label
    // is used for guard classification; the original attributes are forwarded.
    let mut io_status = [0usize; 2];
    let mut device = [0u32; 2];
    if unsafe {
        NtQueryVolumeInformationFile(handle, io_status.as_mut_ptr(), device.as_mut_ptr(), 8, 4)
    } >= 0
        && device[0] == 0x11
    {
        return Some(wide("\\Device\\NamedPipe"));
    }
    // Native root handles also name objects such as NamedPipe directories.
    // GetFinalPathNameByHandleW only resolves filesystem handles; rejecting all
    // other roots prevents normal anonymous-pipe and child-process creation.
    // ObjectNameInformation resolves the canonical NT root without opening it.
    kernel_name(handle)
}
// Device IO classification must not call GetFinalPathNameByHandleW: resolving
// a device as a filesystem path can itself submit IOCTLs and reenter the guard.
fn kernel_name(handle: *mut c_void) -> Option<Vec<u16>> {
    let mut required = 0u32;
    let status = unsafe { NtQueryObject(handle, 1, std::ptr::null_mut(), 0, &mut required) };
    ROOT_QUERY.store(
        (status as u32 as u64) << 32 | required as u64,
        Ordering::Relaxed,
    );
    if !(size_of::<Unicode>() as u32..=65536).contains(&required) {
        return None;
    }
    let mut storage = vec![0usize; (required as usize).div_ceil(size_of::<usize>())];
    let status = unsafe {
        NtQueryObject(
            handle,
            1,
            storage.as_mut_ptr().cast(),
            required,
            &mut required,
        )
    };
    ROOT_QUERY.store(
        (status as u32 as u64) << 32 | required as u64,
        Ordering::Relaxed,
    );
    if status < 0 {
        return None;
    }
    let name = read(storage.as_ptr().cast::<Unicode>())?;
    let start = storage.as_ptr() as usize;
    let end = start.checked_add(storage.len() * size_of::<usize>())?;
    let source = name.buffer as usize;
    if name.length == 0
        || name.length % 2 != 0
        || name.length > name.maximum
        || source < start
        || source.checked_add(name.length as usize)? > end
    {
        return None;
    }
    let mut path = vec![0u16; name.length as usize / 2];
    let bytes = unsafe {
        std::slice::from_raw_parts_mut(path.as_mut_ptr().cast::<u8>(), name.length as usize)
    };
    crate::world_hooks::read(source, bytes).ok()?;
    Some(path)
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}
fn terminated(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn prefix(path: &[u16], root: &[u16]) -> bool {
    path.len() >= root.len()
        && path[..root.len()].iter().zip(root).all(|(a, b)| {
            let lower = |c: u16| {
                if (b'A' as u16..=b'Z' as u16).contains(&c) {
                    c + 32
                } else {
                    c
                }
            };
            lower(*a) == lower(*b)
        })
        && (path.len() == root.len() || path[root.len()] == b'\\' as u16)
}
fn read<T: Copy>(pointer: *const T) -> Option<T> {
    let mut value = std::mem::MaybeUninit::<T>::uninit();
    let bytes =
        unsafe { std::slice::from_raw_parts_mut(value.as_mut_ptr().cast::<u8>(), size_of::<T>()) };
    crate::world_hooks::read(pointer as usize, bytes).ok()?;
    Some(unsafe { value.assume_init() })
}
fn object_name(a: &Attributes) -> Option<Vec<u16>> {
    let Some(name) = read(a.name) else {
        NAME_FAILURES.fetch_or(1, Ordering::Relaxed);
        return None;
    };
    // Native file inputs use Length; MaximumLength may be zero for a borrowed
    // constant name (including Windows' anonymous-pipe implementation). Check
    // the actual bounded readable span rather than rejecting that valid input.
    if name.length % 2 != 0 || name.length > 65532 {
        NAME_FAILURES.fetch_or(2, Ordering::Relaxed);
        return None;
    }
    let mut path = vec![0u16; name.length as usize / 2];
    let bytes = unsafe {
        std::slice::from_raw_parts_mut(path.as_mut_ptr().cast::<u8>(), name.length as usize)
    };
    if !bytes.is_empty() && crate::world_hooks::read(name.buffer as usize, bytes).is_err() {
        NAME_FAILURES.fetch_or(4, Ordering::Relaxed);
        return None;
    }
    if !a.root.is_null() {
        // VOLUME_NAME_NT gives the canonical NT volume prefix, including for
        // source-profile directories opened before this instrumentation.
        let Some(mut full) = root_name(a.root) else {
            NAME_FAILURES.fetch_or(8, Ordering::Relaxed);
            return None;
        };
        if !path.is_empty() {
            full.push(b'\\' as u16);
            full.extend(path);
        }
        path = full;
    }
    Some(path)
}
fn with_attributes(
    original: *const Attributes,
    call: impl FnOnce(*const Attributes) -> i32,
) -> i32 {
    // Refuse malformed/unresolved names instead of bypassing the guard. No
    // pointers borrowed from a caller survive this synchronous API invocation.
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let Some(mut a) = read(original) else {
            return DENIED;
        };
        if a.length as usize != size_of::<Attributes>() {
            return DENIED;
        }
        let Some(path) = object_name(&a) else {
            PATH_DENIALS.fetch_add(1, Ordering::Relaxed);
            return DENIED;
        };
        // Socket endpoints are NT device objects. Blocking creation plus all
        // FILE_DEVICE_NETWORK IOCTLs also covers ConnectEx/overlapped Winsock,
        // without modifying machine firewall settings or another process.
        if prefix(&path, &wide("\\Device\\Afd")) && !STATE.get().is_some_and(|s| s.loopback)
            || prefix(&path, &wide("\\Device\\Tcp"))
            || prefix(&path, &wide("\\Device\\Udp"))
        {
            NETWORK_DENIALS.fetch_add(1, Ordering::Relaxed);
            return DENIED;
        }
        let Some(state) = STATE.get() else {
            return DENIED;
        };
        if let Some(root) = state.source.iter().find(|root| prefix(&path, root)) {
            let mut mapped = state.target.clone();
            mapped.extend_from_slice(&path[root.len()..]);
            let Ok(length) = u16::try_from(mapped.len() * 2) else {
                return DENIED;
            };
            let name = Unicode {
                length,
                maximum: length,
                buffer: mapped.as_ptr(),
            };
            a.root = std::ptr::null_mut();
            a.name = &name;
            REDIRECTS.fetch_add(1, Ordering::Relaxed);
            call(&a)
        } else {
            call(original)
        }
    }))
    .unwrap_or(DENIED)
}
unsafe extern "system" fn create(
    out: *mut *mut c_void,
    access: u32,
    a: *const Attributes,
    io: *mut c_void,
    size: *const i64,
    attrs: u32,
    share: u32,
    disposition: u32,
    options: u32,
    ea: *mut c_void,
    ea_size: u32,
) -> i32 {
    with_attributes(a, |a| unsafe {
        (ORIGINALS.get().unwrap().create)(
            out,
            access,
            a,
            io,
            size,
            attrs,
            share,
            disposition,
            options,
            ea,
            ea_size,
        )
    })
}
unsafe extern "system" fn open(
    out: *mut *mut c_void,
    access: u32,
    a: *const Attributes,
    io: *mut c_void,
    share: u32,
    options: u32,
) -> i32 {
    with_attributes(a, |a| unsafe {
        (ORIGINALS.get().unwrap().open)(out, access, a, io, share, options)
    })
}
unsafe extern "system" fn query(a: *const Attributes, out: *mut c_void) -> i32 {
    with_attributes(a, |a| unsafe { (ORIGINALS.get().unwrap().query)(a, out) })
}
unsafe extern "system" fn query_full(a: *const Attributes, out: *mut c_void) -> i32 {
    with_attributes(a, |a| unsafe {
        (ORIGINALS.get().unwrap().query_full)(a, out)
    })
}
unsafe extern "system" fn delete(a: *const Attributes) -> i32 {
    with_attributes(a, |a| unsafe { (ORIGINALS.get().unwrap().delete)(a) })
}
unsafe extern "system" fn set_info(
    file: *mut c_void,
    status: *mut c_void,
    input: *const c_void,
    length: u32,
    class: u32,
) -> i32 {
    // Rename/link structures (including Ex and bypass-access-check variants)
    // share x64 RootDirectory/+8, FileNameLength/+16, FileName/+20. Route their
    // destination too: a redirected source handle must not rename into the
    // original profile through an absolute destination path.
    if !matches!(class, 10 | 11 | 56 | 57 | 65 | 66 | 72 | 73) {
        return unsafe { (ORIGINALS.get().unwrap().set_info)(file, status, input, length, class) };
    }
    if !(20..=65536).contains(&length) {
        return DENIED;
    }
    let mut bytes = vec![0u8; length as usize];
    if crate::world_hooks::read(input as usize, &mut bytes).is_err() {
        return DENIED;
    }
    let name_length = u32::from_ne_bytes(bytes[16..20].try_into().unwrap()) as usize;
    if !name_length.is_multiple_of(2) || name_length > 65532 || 20 + name_length > bytes.len() {
        return DENIED;
    }
    let mut name: Vec<u16> = bytes[20..20 + name_length]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|v| u16::from_ne_bytes([v[0], v[1]]))
        .collect();
    let root = usize::from_ne_bytes(bytes[8..16].try_into().unwrap()) as *mut c_void;
    // Null-root relative rename names are relative to the source file's parent,
    // which is already isolated by its opened file handle.
    if root.is_null() && name.first() != Some(&(b'\\' as u16)) {
        return unsafe { (ORIGINALS.get().unwrap().set_info)(file, status, input, length, class) };
    }
    let unicode = Unicode {
        length: name_length as u16,
        maximum: name_length as u16,
        buffer: name.as_mut_ptr(),
    };
    let attributes = Attributes {
        length: size_of::<Attributes>() as u32,
        root,
        name: &unicode,
        flags: 0x40,
        security: std::ptr::null_mut(),
        qos: std::ptr::null_mut(),
    };
    with_attributes(&attributes, |mapped| {
        let Some(a) = read(mapped) else {
            return DENIED;
        };
        let Some(u) = read(a.name) else {
            return DENIED;
        };
        let mut output = bytes[..20].to_vec();
        output.resize(20 + u.length as usize, 0);
        output[8..16].copy_from_slice(&(a.root as usize).to_ne_bytes());
        output[16..20].copy_from_slice(&(u.length as u32).to_ne_bytes());
        if crate::world_hooks::read(u.buffer as usize, &mut output[20..]).is_err() {
            return DENIED;
        }
        unsafe {
            (ORIGINALS.get().unwrap().set_info)(
                file,
                status,
                output.as_ptr().cast(),
                output.len() as u32,
                class,
            )
        }
    })
}
unsafe extern "system" fn io(
    file: *mut c_void,
    event: *mut c_void,
    apc: *mut c_void,
    context: *mut c_void,
    status: *mut c_void,
    code: u32,
    input: *mut c_void,
    input_size: u32,
    output: *mut c_void,
    output_size: u32,
) -> i32 {
    let local_metadata = STATE.get().is_some_and(|s| s.loopback)
        && matches!(code, 0x0012_0007 | 0x0012_000f | 0x0012_001b)
        && kernel_name(file).is_some_and(|name| name == wide("\\Device\\Nsi"));
    if local_metadata {
        // NSI get-parameter/get-all/enumeration read existing local state. It is not
        // an AFD operation and cannot authorize a socket send or connection.
        NETWORK_METADATA.fetch_add(1, Ordering::Relaxed);
    }
    if (code >> 16 == 0x12 || code & 0xffff_f000 == 0x0001_2000)
        && !local_metadata
        && !(STATE.get().is_some_and(|s| s.loopback)
            && network::allow_io(file, code, input, input_size))
    {
        NETWORK_LAST_IO.store(code as u64, Ordering::Relaxed);
        NETWORK_DENIALS.fetch_add(1, Ordering::Relaxed);
        return DENIED;
    }
    unsafe {
        (ORIGINALS.get().unwrap().io)(
            file,
            event,
            apc,
            context,
            status,
            code,
            input,
            input_size,
            output,
            output_size,
        )
    }
}
unsafe extern "system" fn api(
    class: *const GUID,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    let class_id = read(class);
    if !matches!(class_id, Some(USER_CLASS | USER_DEVICE_CLASS)) {
        return unsafe { (ORIGINALS.get().unwrap().api)(class, iid, out) };
    }
    // Deny the complete user provider before calling GDK. This includes future
    // interface versions, remote-connect flows and silent default-user lookup.
    // Normal platform/storage/license providers still receive their real call.
    USER_QUERIES.fetch_add(1, Ordering::Relaxed);
    if class_id == Some(USER_DEVICE_CLASS) {
        // Device association can also return user handles. Never query that
        // real provider while testing an isolated unsigned identity.
        USER_DEVICE_QUERIES.fetch_add(1, Ordering::Relaxed);
        if STATE.get().is_some_and(|s| s.empty_user_provider) {
            return unsafe { user::query_device(iid, out) };
        }
    } else if STATE.get().is_some_and(|s| s.empty_user_provider) {
        return unsafe { user::query(iid, out) };
    }
    USER_DENIALS.fetch_add(1, Ordering::Relaxed);
    if !out.is_null() {
        unsafe {
            *out = std::ptr::null_mut();
        }
    }
    HRESULT(0x80070005u32 as i32)
}

/// Install only while a test host/owned Minecraft entry thread is held. No
/// source-profile files are copied. The caller must create an empty test root
/// outside that source profile and must terminate its owned process on failure.
pub fn install(root: &Path, source: &Path) -> Result<(), String> {
    install_mode(root, source, false, false)
}
/// Opt-in isolated test transport: only loopback destinations are permitted.
pub fn install_loopback(root: &Path, source: &Path) -> Result<(), String> {
    install_mode(root, source, true, false)
}
/// Test-only LAN mode: translate interface binds into private loopback binds.
/// Destination checks still reject every non-loopback connect/send operation.
pub fn install_lan_loopback(root: &Path, source: &Path) -> Result<(), String> {
    install_loopback(root, source)?;
    network::enable_lan_bind_translation();
    Ok(())
}
/// Test-only replay discovery redirects the exact game discovery broadcasts to a second
/// loopback interface. Other broadcast/external destinations remain denied.
pub fn install_replay_loopback(root: &Path, source: &Path) -> Result<(), String> {
    install_lan_loopback(root, source)?;
    network::enable_replay_discovery();
    Ok(())
}
/// Test-only empty user provider; real GDK async callbacks finish without ever
/// acquiring the real user provider or creating verified user handles/tokens.
pub fn install_empty_user(root: &Path, source: &Path, loopback: bool) -> Result<(), String> {
    install_mode(root, source, loopback, true)
}
fn install_mode(
    root: &Path,
    source: &Path,
    loopback: bool,
    empty_user_provider: bool,
) -> Result<(), String> {
    if STATE.get().is_some() {
        return Err("probe isolation already initialized".into());
    }
    if !root.is_absolute() || !source.is_absolute() {
        return Err("probe profile paths must be absolute".into());
    }
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let source = source.canonicalize().map_err(|e| e.to_string())?;
    if root.starts_with(&source) || source.starts_with(&root) {
        return Err("probe/source profiles must be disjoint".into());
    }
    let trim = |p: &Path| {
        p.to_string_lossy()
            .trim_start_matches("\\\\?\\")
            .trim_end_matches('\\')
            .to_owned()
    };
    let root_dos = trim(&root);
    let source_dos = trim(&source);
    let target = wide(&format!("\\??\\{root_dos}"));
    let mut aliases = vec![
        wide(&format!("\\??\\{source_dos}")),
        wide(&format!("\\DosDevices\\{source_dos}")),
    ];
    if source_dos.as_bytes().get(1) != Some(&b':') {
        return Err("source profile must use a local drive".into());
    }
    let mut volume = vec![0u16; 32768];
    let drive = terminated(&source_dos[..2]);
    let n = unsafe { QueryDosDeviceW(drive.as_ptr(), volume.as_mut_ptr(), volume.len() as u32) };
    if n == 0 {
        return Err("source-profile NT volume unavailable".into());
    }
    volume.truncate(
        volume
            .iter()
            .position(|v| *v == 0)
            .ok_or("unterminated NT volume")?,
    );
    volume.extend(wide(&source_dos[2..]));
    aliases.push(volume);
    for directory in [
        "AppData/Local",
        "AppData/Roaming",
        "AppData/LocalLow",
        "Documents",
        "Saved Games",
        "Temp",
    ] {
        std::fs::create_dir_all(root.join(directory)).map_err(|e| e.to_string())?;
    }
    unsafe {
        let initialized = MH_Initialize();
        if !matches!(
            initialized,
            MH_STATUS::MH_OK | MH_STATUS::MH_ERROR_ALREADY_INITIALIZED
        ) {
            return Err(format!("probe MinHook initialization: {initialized:?}"));
        }
        let nt = LoadLibraryW(w!("ntdll.dll")).map_err(|e| e.to_string())?;
        let gdk = LoadLibraryW(w!("xgameruntime.dll")).map_err(|e| e.to_string())?;
        let exports = [
            (nt, s!("NtCreateFile"), create as *const () as usize),
            (nt, s!("NtOpenFile"), open as *const () as usize),
            (nt, s!("NtQueryAttributesFile"), query as *const () as usize),
            (
                nt,
                s!("NtQueryFullAttributesFile"),
                query_full as *const () as usize,
            ),
            (nt, s!("NtDeviceIoControlFile"), io as *const () as usize),
            (gdk, s!("QueryApiImpl"), api as *const () as usize),
            (nt, s!("NtDeleteFile"), delete as *const () as usize),
            (
                nt,
                s!("NtSetInformationFile"),
                set_info as *const () as usize,
            ),
        ];
        let mut targets = Vec::new();
        let mut originals = Vec::new();
        for (module, name, callback) in exports {
            let target =
                GetProcAddress(module, name).ok_or("probe API unavailable")? as *const () as usize;
            let mut original = std::ptr::null_mut();
            let result = MH_CreateHook(target as _, callback as _, &mut original);
            if result != MH_STATUS::MH_OK {
                return Err(format!("create probe guard: {result:?}"));
            }
            targets.push(target);
            originals.push(original as usize);
        }
        ORIGINALS
            .set(Originals {
                create: std::mem::transmute::<usize, Create>(originals[0]),
                open: std::mem::transmute::<usize, Open>(originals[1]),
                query: std::mem::transmute::<usize, Query>(originals[2]),
                query_full: std::mem::transmute::<usize, Query>(originals[3]),
                io: std::mem::transmute::<usize, Io>(originals[4]),
                api: std::mem::transmute::<usize, Api>(originals[5]),
                delete: std::mem::transmute::<usize, Delete>(originals[6]),
                set_info: std::mem::transmute::<usize, SetInfo>(originals[7]),
            })
            .map_err(|_| "probe trampolines already published")?;
        if empty_user_provider {
            user::prepare(ORIGINALS.get().unwrap().api)?;
        }
        STATE
            .set(Isolation {
                source: aliases,
                target,
                root: root.clone(),
                at: Instant::now(),
                loopback,
                empty_user_provider,
            })
            .map_err(|_| "probe state already published")?;
        if loopback {
            targets.extend(network::prepare()?);
        }
        for target in targets {
            let result = MH_EnableHook(target as _);
            if result != MH_STATUS::MH_OK {
                return Err(format!("enable probe guard: {result:?}"));
            }
        }
        for (key, value) in [
            ("USERPROFILE", root_dos.clone()),
            ("APPDATA", format!("{root_dos}\\AppData\\Roaming")),
            ("LOCALAPPDATA", format!("{root_dos}\\AppData\\Local")),
            ("TEMP", format!("{root_dos}\\Temp")),
            ("TMP", format!("{root_dos}\\Temp")),
        ] {
            if SetEnvironmentVariableW(terminated(key).as_ptr(), terminated(&value).as_ptr()) == 0 {
                return Err("couldn't set isolated process environment".into());
            }
        }
    }
    Ok(())
}
pub fn status() -> Value {
    match STATE.get() {
        None => Value::Null,
        Some(state) => {
            let mut value = json!({"profile":state.root,"profileRedirects":REDIRECTS.load(Ordering::Relaxed),"networkDenied":NETWORK_DENIALS.load(Ordering::Relaxed),"localNetworkQueries":NETWORK_METADATA.load(Ordering::Relaxed),"connectionErrors":errors::status(),"networkExtensions":network::status(),"lastDeniedNetworkIo":NETWORK_LAST_IO.load(Ordering::Relaxed),"userQueries":USER_QUERIES.load(Ordering::Relaxed),"userDeviceQueries":USER_DEVICE_QUERIES.load(Ordering::Relaxed),"emptyProvider":if state.empty_user_provider {user::status()}else{Value::Null},"userAcquisitionsDenied":USER_DENIALS.load(Ordering::Relaxed),"pathDenied":PATH_DENIALS.load(Ordering::Relaxed),"nameFailureMask":NAME_FAILURES.load(Ordering::Relaxed),"rootQuery":ROOT_QUERY.load(Ordering::Relaxed),"networkMode":if state.loopback {"loopback-only"}else{"deny-all"},"userMode":if state.empty_user_provider {"empty-provider"}else{"deny-provider"},"licenseChanged":false});
            value["headlessOcclusionAdjustments"] =
                json!(HEADLESS_OCCLUSIONS.load(Ordering::Relaxed));
            value["nativeJoin"] = join::status();
            value["replayDiscovery"] = network::replay_discovery_status();
            value
        }
    }
}

pub(crate) fn needs_frame() -> bool {
    let number = SNAPSHOT.load(Ordering::Acquire);
    STATE.get().is_some_and(|s| {
        number < 12 && s.at.elapsed() >= Duration::from_secs(12 + number * 30)
            || s.root.join("probe-snapshot.request").is_file()
    }) && number < 256
}
pub(crate) fn needs_tagged_frame(marker: Option<u64>) -> bool {
    needs_frame()
        || (marker.is_some() && STATE.get().is_some() && SNAPSHOT.load(Ordering::Acquire) < 256)
}
pub(crate) fn frame(width: u32, height: u32, rgba: &[u8], marker: Option<u64>) {
    if !needs_tagged_frame(marker) || rgba.len() != width as usize * height as usize * 4 {
        return;
    }
    if let Some(state) = STATE.get() {
        let _ = std::fs::remove_file(state.root.join("probe-snapshot.request"));
        let number = SNAPSHOT.fetch_add(1, Ordering::AcqRel);
        // Keep only sixteen raw frames, while sequence numbers remain unique
        // for the controller. Old metadata is removed before its slot changes.
        if number >= 16 {
            let _ =
                std::fs::remove_file(state.root.join(format!("probe-frame-{}.json", number - 16)));
        }
        let name = format!("probe-frame-{}.rgba", number % 16);
        let path = state.root.join(&name);
        if std::fs::write(&path, rgba).is_ok() {
            let _=std::fs::write(state.root.join(format!("probe-frame-{number}.json")),json!({"width":width,"height":height,"format":"RGBA8","file":name,"worldRequestSequence":marker,"worldVerified":false}).to_string());
        }
    }
}

#[cfg(test)]
mod presentation_tests {
    use super::*;
    #[test]
    fn headless_adapter_preserves_device_errors_and_other_status_codes() {
        for code in [0, 1, 0x087a0002, 0x887a0005u32 as i32, 0x887a0007u32 as i32] {
            assert_eq!(headless_present(0, HRESULT(code)), HRESULT(code));
            assert_eq!(headless_present(1, HRESULT(code)), HRESULT(code));
        }
        assert_eq!(headless_present(0, HRESULT(0x087a0001)), HRESULT(0));
        assert_eq!(headless_present(1, HRESULT(0x087a0001)), HRESULT(0));
    }
}
