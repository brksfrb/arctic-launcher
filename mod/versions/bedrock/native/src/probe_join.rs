//! Opt-in installed replay connection through the engine's own LAN join method.
//! Requests execute only inside a fresh native facet-binding callback, on the
//! same thread that just bound the LAN list. No engine object pointer is cached.
use crate::world_hooks::read;
use hudhook::mh::{MH_CreateHook, MH_EnableHook, MH_STATUS};
use hudhook::windows::Win32::System::Threading::GetCurrentThreadId;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicU64, AtomicUsize, Ordering},
};
use std::time::Instant;

const JOIN_BIND: usize = 0xbbf50e0;
const LAN_BIND: usize = 0xbbc0d00;
const LAN_JOIN: usize = 0xbbf7030;
const WORLD_SIZE: usize = 0xc0;
type Bind = unsafe extern "system" fn(usize, usize);
type Join = unsafe extern "system" fn(usize, *const NativeString);
static DIRECTORY: OnceLock<PathBuf> = OnceLock::new();
static BASE: AtomicUsize = AtomicUsize::new(0);
static JOIN_ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static LAN_ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static JOIN_BINDS: AtomicU64 = AtomicU64::new(0);
static LAN_BINDS: AtomicU64 = AtomicU64::new(0);
static SEQUENCE: AtomicU64 = AtomicU64::new(0);
static INVOKED: AtomicU64 = AtomicU64::new(0);
static LAN: Mutex<Option<LanSample>> = Mutex::new(None);
static LAST: Mutex<Option<Value>> = Mutex::new(None);
static LAST_COMMAND: Mutex<Option<Value>> = Mutex::new(None);
static CONTEXT_STEP: AtomicUsize = AtomicUsize::new(0);
static CONTEXT_MAX_STEP: AtomicUsize = AtomicUsize::new(0);
fn step(value: usize) {
    CONTEXT_STEP.store(value, Ordering::Relaxed);
    CONTEXT_MAX_STEP.fetch_max(value, Ordering::Relaxed);
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct World {
    id: String,
    name: String,
    owner_name: String,
    address: String,
    port: u32,
}
struct LanSample {
    thread: u32,
    observed: Instant,
    worlds: Vec<World>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    sequence: u64,
    address: String,
    port: u32,
}

// MSVC x64 basic_string<char>, used only as a const reference. The native
// method copies its input; Rust retains the backing bytes throughout the call.
#[repr(C)]
struct NativeString {
    storage: [u8; 16],
    length: usize,
    capacity: usize,
}
impl NativeString {
    fn new(bytes: &[u8], backing: &mut Vec<u8>) -> Self {
        let mut value = Self {
            storage: [0; 16],
            length: bytes.len(),
            capacity: 15,
        };
        if bytes.len() < 16 {
            value.storage[..bytes.len()].copy_from_slice(bytes);
        } else {
            backing.extend_from_slice(bytes);
            backing.push(0);
            value.storage[..8].copy_from_slice(&(backing.as_ptr() as usize).to_ne_bytes());
            value.capacity = bytes.len();
        }
        value
    }
}
fn bytes<const N: usize>(at: usize) -> Option<[u8; N]> {
    let mut value = [0; N];
    read(at, &mut value).ok()?;
    Some(value)
}
fn word(at: usize) -> Option<usize> {
    Some(usize::from_ne_bytes(bytes(at)?))
}
fn text(at: usize, bound: usize) -> Option<String> {
    let header = bytes::<32>(at)?;
    let length = usize::from_ne_bytes(header[16..24].try_into().ok()?);
    let capacity = usize::from_ne_bytes(header[24..32].try_into().ok()?);
    if length > bound || length > capacity || !(15..=1024 * 1024).contains(&capacity) {
        return None;
    }
    let content = if capacity == 15 {
        header[..length].to_vec()
    } else {
        if capacity < 16 {
            return None;
        }
        let address = usize::from_ne_bytes(header[..8].try_into().ok()?);
        let mut buffer = vec![0; length];
        read(address, &mut buffer).ok()?;
        buffer
    };
    if bytes::<32>(at)? != header {
        return None;
    }
    String::from_utf8(content).ok()
}
fn vector(header: [usize; 3]) -> Option<(usize, usize)> {
    let [begin, end, capacity] = header;
    if begin == 0 {
        return (end == 0 && capacity == 0).then_some((0, 0));
    }
    if begin % 8 != 0 || end < begin || capacity < end {
        return None;
    }
    let used = end.checked_sub(begin)?;
    let reserved = capacity.checked_sub(begin)?;
    if used % WORLD_SIZE != 0 || reserved % WORLD_SIZE != 0 || reserved > WORLD_SIZE * 128 {
        return None;
    }
    Some((begin, used / WORLD_SIZE))
}
fn worlds(context: usize) -> Option<Vec<World>> {
    let at = context.checked_add(0x20)?;
    let raw = bytes::<24>(at)?;
    let header =
        std::array::from_fn(|i| usize::from_ne_bytes(raw[i * 8..i * 8 + 8].try_into().unwrap()));
    let (begin, count) = vector(header)?;
    let mut output = Vec::new();
    for i in 0..count {
        let item = begin.checked_add(i * WORLD_SIZE)?;
        let address = text(item.checked_add(0x60)?, 128)?;
        if address != "127.0.0.2" {
            continue;
        }
        let port = u32::from_ne_bytes(bytes(item.checked_add(0x80)?)?);
        if port != 7551 {
            continue;
        }
        let name = text(item.checked_add(0x20)?, 512)?;
        let owner_name = text(item.checked_add(0x40)?, 512)?;
        let id = text(item, 512)?;
        if id.is_empty()
            || id.chars().any(char::is_control)
            || name != "Offline playback"
            || owner_name != "Arctic Bedrock Replay"
        {
            continue;
        }
        output.push(World {
            id,
            name,
            owner_name,
            address,
            port,
        });
    }
    (bytes::<24>(at)? == raw).then_some(output)
}
pub(super) fn configure(directory: &Path) -> Result<(), String> {
    DIRECTORY
        .set(directory.to_path_buf())
        .map_err(|_| "native replay join already configured".into())
}
unsafe extern "system" fn lan_bind(binder: usize, context: usize) {
    let original: Bind = unsafe { std::mem::transmute(LAN_ORIGINAL.load(Ordering::Acquire)) };
    unsafe { original(binder, context) };
    LAN_BINDS.fetch_add(1, Ordering::Relaxed);
    let _ = std::panic::catch_unwind(|| {
        let sample = if context == 0 {
            None
        } else {
            worlds(context).map(|worlds| LanSample {
                thread: unsafe { GetCurrentThreadId() },
                observed: Instant::now(),
                worlds,
            })
        };
        if let Ok(mut last) = LAN.lock() {
            *last = sample;
        }
    });
}
unsafe extern "system" fn join_bind(binder: usize, context: usize) {
    let original: Bind = unsafe { std::mem::transmute(JOIN_ORIGINAL.load(Ordering::Acquire)) };
    unsafe { original(binder, context) };
    JOIN_BINDS.fetch_add(1, Ordering::Relaxed);
    let _ = std::panic::catch_unwind(|| try_join(context));
}
fn try_join(context: usize) -> Option<()> {
    let directory = DIRECTORY.get()?;
    if context == 0 {
        return None;
    }
    step(1);
    let state = word(context.checked_add(0x118)?)?;
    let control = word(context.checked_add(0x120)?)?;
    let backend = word(context.checked_add(0x18)?)?;
    step(2);
    let _ = bytes::<32>(backend)?;
    let counts = bytes::<8>(control.checked_add(8)?)?;
    step(3);
    if counts
        .as_chunks::<4>()
        .0
        .iter()
        .any(|v| !(1..=1_000_000).contains(&u32::from_ne_bytes(*v)))
    {
        return None;
    }
    let task = u32::from_ne_bytes(bytes(state.checked_add(0x34)?)?);
    step(4);
    if task > 3 {
        return None;
    }
    let thread = unsafe { GetCurrentThreadId() };
    if let Ok(mut last) = LAST.lock() {
        *last = Some(json!({"contextObserved":true,"thread":thread,"lanTaskState":task}));
    }
    if task == 1 {
        return None;
    }
    step(5);
    let mut source = Vec::new();
    std::fs::File::open(directory.join("replay-native.request.json"))
        .ok()?
        .take(1025)
        .read_to_end(&mut source)
        .ok()?;
    if source.len() > 1024 {
        return None;
    }
    let request: Request = serde_json::from_slice(&source).ok()?;
    if request.sequence == 0
        || request.sequence > 20
        || request.sequence <= SEQUENCE.load(Ordering::Acquire)
        || request.address != "127.0.0.2"
        || request.port != 7551
    {
        return None;
    }
    step(6);
    let target = {
        let lan = LAN.lock().ok()?;
        let lan = lan.as_ref()?;
        step(7);
        if lan.thread != thread || lan.observed.elapsed().as_secs_f64() > 5. {
            return None;
        }
        if lan.worlds.len() != 1 {
            return None;
        }
        step(8);
        lan.worlds[0].clone()
    };
    let prior = SEQUENCE.load(Ordering::Acquire);
    if prior >= request.sequence {
        return None;
    }
    SEQUENCE
        .compare_exchange(prior, request.sequence, Ordering::AcqRel, Ordering::Acquire)
        .ok()?;
    let mut backing = Vec::new();
    let id = NativeString::new(target.id.as_bytes(), &mut backing);
    let call: Join =
        unsafe { std::mem::transmute(BASE.load(Ordering::Acquire).checked_add(LAN_JOIN)?) };
    if let Ok(mut last) = LAST_COMMAND.lock() {
        *last = Some(
            json!({"sequence":request.sequence,"returned":false,"thread":thread,"world":target}),
        );
    }
    // Context belongs to the current binding callback; the ID is copied from a
    // fresh native LAN list, never fabricated or taken from a server-list file.
    unsafe { call(context, &id) };
    step(9);
    INVOKED.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut last) = LAST_COMMAND.lock() {
        *last = Some(
            json!({"sequence":request.sequence,"invoked":true,"returned":true,
        "thread":thread,"world":target,"nativeWorldVerified":false}),
        );
    }
    Some(())
}
/// Called only after the complete installed executable hash has matched.
pub(super) unsafe fn install(base: usize) -> Result<(), String> {
    if DIRECTORY.get().is_none() {
        return Ok(());
    }
    BASE.store(base, Ordering::Release);
    for (rva, hook, slot) in [
        (JOIN_BIND, join_bind as *const () as usize, &JOIN_ORIGINAL),
        (LAN_BIND, lan_bind as *const () as usize, &LAN_ORIGINAL),
    ] {
        let target = base
            .checked_add(rva)
            .ok_or("native join address overflow")?;
        let mut original = std::ptr::null_mut();
        let result = unsafe { MH_CreateHook(target as _, hook as _, &mut original) };
        if result != MH_STATUS::MH_OK {
            return Err(format!("create native join observer: {result:?}"));
        }
        slot.store(original as usize, Ordering::Release);
        let result = unsafe { MH_EnableHook(target as _) };
        if result != MH_STATUS::MH_OK {
            return Err(format!("enable native join observer: {result:?}"));
        }
    }
    Ok(())
}
pub(super) fn status() -> Value {
    json!({"enabled":DIRECTORY.get().is_some(),"joinBindings":JOIN_BINDS.load(Ordering::Relaxed),
        "lanBindings":LAN_BINDS.load(Ordering::Relaxed),"invoked":INVOKED.load(Ordering::Relaxed),
        "sequence":SEQUENCE.load(Ordering::Relaxed),"last":LAST.lock().ok().and_then(|v|v.clone()),
        "lastInvocation":LAST_COMMAND.lock().ok().and_then(|v|v.clone()),
        "contextStep":CONTEXT_STEP.load(Ordering::Relaxed),"contextMaxStep":CONTEXT_MAX_STEP.load(Ordering::Relaxed),
        "lanWorlds":LAN.lock().ok().and_then(|v|v.as_ref().map(|v|v.worlds.clone())),
        "nativeWorldVerified":false})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_string_handles_both_storage_forms_without_transferring_ownership() {
        for input in [
            "123",
            "a-native-world-identifier-longer-than-inline-storage",
        ] {
            let mut backing = Vec::new();
            let value = NativeString::new(input.as_bytes(), &mut backing);
            assert_eq!(std::mem::size_of::<NativeString>(), 32);
            assert_eq!(
                text(&value as *const _ as usize, 512).as_deref(),
                Some(input)
            );
        }
    }
    #[test]
    fn lan_vector_rejects_truncation_capacity_overflow_and_oversized_native_lists() {
        assert_eq!(vector([0, 0, 0]), Some((0, 0)));
        assert_eq!(vector([0x1000, 0x1180, 0x1240]), Some((0x1000, 2)));
        for value in [
            [0, 1, 1],
            [0x1000, 0x1001, 0x10c0],
            [0x1000, 0x1180, 0x10c0],
            [usize::MAX - 7, 8, 8],
            [0x1000, 0x1000, 0x1000 + 129 * WORLD_SIZE],
        ] {
            assert!(vector(value).is_none(), "{value:?}");
        }
    }
}
