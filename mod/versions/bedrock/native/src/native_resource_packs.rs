//! Exact-image, read-only full-stack observation. No engine getters, filesystem
//! discovery, constructors or resource loaders are invoked by the observer.
use crate::world_hooks::{image_base, read};
use hudhook::mh::{MH_CreateHook, MH_EnableHook, MH_STATUS};
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

const CLIENT_TABLE: usize = 0xe9731b0;
const CLIENT_RESOURCES: usize = 0x5dbdfb0;
const MANAGER_TABLE: usize = 0xe7b7f10;
const STACK_TABLE: usize = 0xe7c84b0;
const DIRECTORY_TABLE: usize = 0xe7c9f40;
const COMPOSE: usize = 0x14c8d20;
const STRIDE: usize = 0x128;
type Compose = unsafe extern "system" fn(usize);
static ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static INSTALLED: AtomicBool = AtomicBool::new(false);
static ACTIVE: AtomicUsize = AtomicUsize::new(0);
static REVISION: AtomicU64 = AtomicU64::new(0);
static ATTEMPTS: AtomicU64 = AtomicU64::new(0);
static COPIES: AtomicU64 = AtomicU64::new(0);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Snapshot {
    pub revision: String,
    pub packs: Vec<Vec<String>>,
    pub error: Option<String>,
    #[serde(skip)]
    epoch: u64,
}
impl Snapshot {
    pub(crate) fn current(&self) -> bool {
        INSTALLED.load(Ordering::Acquire)
            && ACTIVE.load(Ordering::Acquire) == 0
            && REVISION.load(Ordering::Acquire) == self.epoch
    }
    pub(crate) fn heap_bytes(&self) -> usize {
        self.revision.capacity()
            + self.error.as_ref().map_or(0, String::capacity)
            + self.packs.capacity() * std::mem::size_of::<Vec<String>>()
            + self
                .packs
                .iter()
                .map(|roots| {
                    roots.capacity() * std::mem::size_of::<String>()
                        + roots.iter().map(String::capacity).sum::<usize>()
                })
                .sum::<usize>()
    }
    pub(crate) fn publish(&self) -> serde_json::Value {
        // Selected subpack/revision bytes may change without composing a new
        // vector. Re-observe the current primary context before publishing an
        // older GPU-matched frame; history retains no engine pointers.
        let live = crate::world_hooks::current_resource_stack();
        if self.current()
            && live
                .as_ref()
                .is_some_and(|s| s.current() && s.revision == self.revision)
        {
            serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
        } else {
            let epoch = REVISION.load(Ordering::Acquire);
            let error =
                live.as_ref()
                    .and_then(|s| s.error.as_deref())
                    .unwrap_or(if live.is_none() {
                        "primary resource context is unavailable"
                    } else {
                        "resource stack changed after this buffered world draw"
                    });
            let mut hash = Sha256::new();
            hash.update(epoch.to_le_bytes());
            hash.update(error.as_bytes());
            if let Some(live) = &live {
                hash.update(live.revision.as_bytes());
            }
            json!({"revision":crate::native_hash::hex(&hash.finalize()),"packs":[],"error":error})
        }
    }
}
pub(crate) fn status() -> serde_json::Value {
    json!({"observerInstalled":INSTALLED.load(Ordering::Acquire),"revision":REVISION.load(Ordering::Acquire),
        "restacksInProgress":ACTIVE.load(Ordering::Acquire),"attempts":ATTEMPTS.load(Ordering::Relaxed),
        "copies":COPIES.load(Ordering::Relaxed),"activeEngineStackVerified":false})
}

struct Guard {
    address: usize,
    bytes: Vec<u8>,
}
struct Copy {
    base: usize,
    guards: Vec<Guard>,
    bytes: usize,
    roots: Vec<String>,
    packs: Vec<Vec<String>>,
    seen: HashSet<String>,
    paths: usize,
    root_count: usize,
    identity: Sha256,
}
fn add(address: usize, offset: usize) -> Result<usize, String> {
    address
        .checked_add(offset)
        .ok_or_else(|| "resource address overflow".into())
}
fn word(bytes: &[u8], at: usize) -> usize {
    u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap()) as usize
}
fn address(value: usize) -> Result<usize, String> {
    (0x10000..0x0000_8000_0000_0000)
        .contains(&value)
        .then_some(value)
        .ok_or_else(|| "invalid resource pointer".into())
}
impl Copy {
    fn bytes(&mut self, at: usize, len: usize) -> Result<Vec<u8>, String> {
        address(at)?;
        add(at, len)?;
        if len > 64 * 1024 || self.guards.len() >= 2048 || self.bytes + len > 256 * 1024 {
            return Err("resource observation copy budget exhausted".into());
        }
        let mut bytes = vec![0; len];
        read(at, &mut bytes)?;
        self.bytes += len;
        self.guards.push(Guard {
            address: at,
            bytes: bytes.clone(),
        });
        Ok(bytes)
    }
    fn pointer(&mut self, at: usize) -> Result<usize, String> {
        let bytes = self.bytes(at, 8)?;
        address(word(&bytes, 0))
    }
    fn text(&mut self, at: usize) -> Result<String, String> {
        let header = self.bytes(at, 32)?;
        let len = word(&header, 16);
        let cap = word(&header, 24);
        if len == 0 || len > 4096 || cap < len || cap > 1024 * 1024 {
            return Err("invalid resolved pack path string".into());
        }
        let bytes = if cap < 16 {
            header[..len].to_vec()
        } else {
            self.bytes(address(word(&header, 0))?, len)?
        };
        let value = String::from_utf8(bytes).map_err(|_| "pack path is not UTF-8")?;
        if value.chars().any(char::is_control) {
            return Err("control character in pack path".into());
        }
        Ok(value)
    }
    fn vector(&mut self, at: usize, stride: usize, max: usize) -> Result<Vec<u8>, String> {
        let header = self.bytes(at, 24)?;
        let first = word(&header, 0);
        let end = word(&header, 8);
        let cap = word(&header, 16);
        if first == 0 && end == 0 && cap == 0 {
            return Ok(Vec::new());
        }
        address(first)?;
        if end < first
            || cap < end
            || !(end - first).is_multiple_of(stride)
            || !(cap - first).is_multiple_of(stride)
            || (end - first) / stride > max
            || (cap - first) / stride > 4096
        {
            return Err("invalid resource vector".into());
        }
        self.bytes(first, end - first)
    }
    fn manager(&mut self, client: usize) -> Result<usize, String> {
        if self.pointer(client)? != add(self.base, CLIENT_TABLE)? {
            return Err("unrecognized resource client table".into());
        }
        let client_table = add(self.base, CLIENT_TABLE)?;
        if self.pointer(add(client_table, 0x300)?)? != add(self.base, CLIENT_RESOURCES)? {
            return Err("resource client delegation changed".into());
        }
        let game = self.pointer(add(client, 0x1a8)?)?;
        let table = self.pointer(game)?;
        let mut getter = self.pointer(add(table, 0x260)?)?;
        for _ in 0..3 {
            if !(add(self.base, 0x1000)?..add(self.base, 0xe5a7000)?).contains(&getter) {
                return Err("resource getter lies outside the pinned image".into());
            }
            let code = self.bytes(getter, 16)?;
            if code[0] == 0xe9 {
                let displacement = i32::from_le_bytes(code[1..5].try_into().unwrap());
                getter = add(getter, 5)?
                    .checked_add_signed(displacement as isize)
                    .ok_or("resource thunk overflow")?;
                continue;
            }
            // Interpret only a plain field-address/pointer getter. Never call
            // a virtual method or infer an offset from a public header alone.
            let offset = if (code[..3] == [0x48, 0x8b, 0x81] || code[..3] == [0x48, 0x8d, 0x81])
                && code[7] == 0xc3
            {
                u32::from_le_bytes(code[3..7].try_into().unwrap()) as usize
            } else if (code[..3] == [0x48, 0x8b, 0x41] || code[..3] == [0x48, 0x8d, 0x41])
                && code[4] == 0xc3
                && code[3] < 0x80
            {
                code[3] as usize
            } else {
                return Err("resource manager getter is not a recognized field access".into());
            };
            if offset > 0x10000 {
                return Err("resource manager field exceeds its layout bound".into());
            }
            let field = add(game, offset)?;
            let manager = if code[1] == 0x8d {
                field
            } else {
                self.pointer(field)?
            };
            if self.pointer(manager)? != add(self.base, MANAGER_TABLE)? {
                return Err("unrecognized resource manager table".into());
            }
            self.identity.update((manager as u64).to_le_bytes());
            return Ok(manager);
        }
        Err("resource getter thunk depth exceeded".into())
    }
    fn directory(&mut self, strategy: usize) -> Result<(), String> {
        if self.pointer(strategy)? != add(self.base, DIRECTORY_TABLE)? {
            return Err(
                "active stack contains an unsupported ZIP, encrypted or custom pack strategy"
                    .into(),
            );
        }
        // The pinned Directory::getAsset prefixes requests with this resolved
        // path. Do not map ResourceFileSystem aliases or guess an install root.
        let root = self.text(add(strategy, 0x70)?)?;
        if !Path::new(&root).is_absolute() {
            return Err("engine directory pack path is not absolute".into());
        }
        self.identity.update((strategy as u64).to_le_bytes());
        self.identity.update(root.as_bytes());
        self.identity.update([0u8]);
        let key = root.replace('/', "\\");
        if self.seen.insert(key) {
            if self.root_count >= 64 || self.paths + root.len() > 64 * 1024 {
                return Err("ordered pack roots exceed their budget".into());
            }
            self.paths += root.len();
            self.root_count += 1;
            self.roots.push(root);
        }
        Ok(())
    }
    fn pack(&mut self, wrapper: usize, subpack: i32, depth: usize) -> Result<(), String> {
        if depth > 1 {
            return Err("resource subpack nesting exceeds the inspected layout".into());
        }
        let inner = self.pointer(wrapper)?;
        let prefix = self.bytes(inner, 0x50)?;
        self.identity.update((wrapper as u64).to_le_bytes());
        self.identity.update((inner as u64).to_le_bytes());
        if subpack >= 0 {
            let children = self.vector(add(inner, 0x38)?, 16, 64)?;
            let index = subpack as usize;
            if index < children.len() / 16 {
                let selected = word(&children, index * 16);
                if selected != 0 {
                    self.pack(address(selected)?, -1, depth + 1)?;
                }
            }
        }
        // ResourcePack::getResource tries the selected child, its own optional
        // subpack access strategy, and then the base Pack strategy, in order.
        let overlay = word(&prefix, 0x18);
        if overlay != 0 {
            self.directory(address(overlay)?)?;
        }
        let pack = address(word(&prefix, 8))?;
        let strategy = self.pointer(add(pack, 8)?)?;
        self.directory(strategy)
    }
    fn stack(&mut self, client: usize) -> Result<(), String> {
        let manager = self.manager(client)?;
        let flags = self.bytes(add(manager, 0xe8)?, 4)?;
        if flags.iter().any(|b| *b > 1) || flags[0] != 0 || flags[1] != 0 {
            return Err("resource stack is initializing or pending a restack".into());
        }
        let stack = self.pointer(add(manager, 0xb8)?)?;
        if self.pointer(stack)? != add(self.base, STACK_TABLE)? {
            return Err("unrecognized full resource stack table".into());
        }
        let packs = self.vector(add(stack, 8)?, STRIDE, 64)?;
        if packs.is_empty() {
            return Err("active resource stack is empty".into());
        }
        self.identity.update((stack as u64).to_le_bytes());
        for entry in packs.chunks_exact(STRIDE) {
            // PackStats at +0x110 changes while assets load. It must not turn
            // every successful load into a new resource-cache generation.
            self.identity.update(&entry[0x100..0x110]);
            self.identity.update(&entry[0x120..0x125]);
            let pack = address(word(entry, 0x100))?;
            let subpack = i32::from_le_bytes(entry[0x120..0x124].try_into().unwrap());
            self.pack(pack, subpack, 0)?;
            if self.roots.is_empty() {
                return Err("active pack instance has no readable directory roots".into());
            }
            self.packs.push(std::mem::take(&mut self.roots));
            self.seen.clear();
            self.identity.update([0xffu8]);
        }
        // Re-copy referenced bytes as well as pointer/vector/string headers.
        // A restack or in-place long-string mutation invalidates the whole copy.
        for guard in &self.guards {
            let mut after = vec![0; guard.bytes.len()];
            read(guard.address, &mut after)?;
            if after != guard.bytes {
                return Err("resource stack changed during observation".into());
            }
        }
        Ok(())
    }
}
pub(crate) fn sample(client: usize) -> Snapshot {
    ATTEMPTS.fetch_add(1, Ordering::Relaxed);
    let epoch = REVISION.load(Ordering::Acquire);
    let mut copy = Copy {
        base: image_base().unwrap_or(0),
        guards: Vec::new(),
        bytes: 0,
        roots: Vec::new(),
        packs: Vec::new(),
        seen: HashSet::new(),
        paths: 0,
        root_count: 0,
        identity: Sha256::new(),
    };
    copy.identity.update(epoch.to_le_bytes());
    let result = if copy.base == 0 || !INSTALLED.load(Ordering::Acquire) {
        Err("native resource observer is unavailable".into())
    } else if ACTIVE.load(Ordering::Acquire) != 0 {
        Err("resource restack is in progress".into())
    } else {
        copy.stack(client)
    };
    let result = result.and_then(|()| {
        if ACTIVE.load(Ordering::Acquire) == 0 && REVISION.load(Ordering::Acquire) == epoch {
            Ok(())
        } else {
            Err("resource stack revision changed during observation".into())
        }
    });
    let error = result.err();
    if let Some(error) = &error {
        copy.packs.clear();
        copy.identity.update(error.as_bytes());
    } else {
        COPIES.fetch_add(1, Ordering::Relaxed);
    }
    Snapshot {
        revision: crate::native_hash::hex(&copy.identity.finalize()),
        packs: copy.packs,
        error,
        epoch,
    }
}
unsafe extern "system" fn compose_hook(manager: usize) {
    ACTIVE.fetch_add(1, Ordering::AcqRel);
    REVISION.fetch_add(1, Ordering::AcqRel);
    let original: Compose = unsafe { std::mem::transmute(ORIGINAL.load(Ordering::Acquire)) };
    unsafe { original(manager) };
    REVISION.fetch_add(1, Ordering::Release);
    ACTIVE.fetch_sub(1, Ordering::Release);
}
pub(crate) unsafe fn install() -> Result<(), String> {
    if INSTALLED.load(Ordering::Acquire) {
        return Ok(());
    }
    let base = image_base().ok_or("resource observer needs a matched image")?;
    let target = add(base, COMPOSE)?;
    let mut trampoline = std::ptr::null_mut();
    let result =
        unsafe { MH_CreateHook(target as _, compose_hook as *const () as _, &mut trampoline) };
    if result != MH_STATUS::MH_OK {
        return Err(format!("create resource restack observer: {result:?}"));
    }
    ORIGINAL.store(trampoline as usize, Ordering::Release);
    let result = unsafe { MH_EnableHook(target as _) };
    if result != MH_STATUS::MH_OK {
        return Err(format!("enable resource restack observer: {result:?}"));
    }
    INSTALLED.store(true, Ordering::Release);
    Ok(())
}
