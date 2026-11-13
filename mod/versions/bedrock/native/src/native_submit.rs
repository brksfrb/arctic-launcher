//! Scoped ownership of native skin/cape submissions and typed replacement plans.
//! The exact-image encoder selects both its hook and RenderItem reader layout.
use crate::native_scene_layout::encoder_submit as layout;
use crate::render_bridge::FrameKey;
use crate::world_hooks::{pointer, read};
use serde_json::{Value, json};
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};

#[path = "native_submit_profile.rs"]
mod profile;
static INSTALL: OnceLock<Result<(), String>> = OnceLock::new();
static INSTALLED: AtomicBool = AtomicBool::new(false);
const MAX_PACKETS: usize = 2048;
// RCX encoder, RDX/R8/R9 u16 handles; depth/flags at entry RSP+0x28/+0x30.
type Submit = unsafe extern "system" fn(usize, u16, u16, u16, u32, u8);
static ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static CALLS: AtomicU64 = AtomicU64::new(0);
static ACTOR_CALLS: AtomicU64 = AtomicU64::new(0);
static OWNED: AtomicU64 = AtomicU64::new(0);
static REJECTED: AtomicU64 = AtomicU64::new(0);
static GPU_MATCHES: AtomicU64 = AtomicU64::new(0);
static PACKETS: Mutex<Vec<Packet>> = Mutex::new(Vec::new());
static LAST: Mutex<Option<Value>> = Mutex::new(None);
static ACTOR_SCOPE: AtomicU64 = AtomicU64::new(0);
static WORLD_CALLS: AtomicU64 = AtomicU64::new(0);
static WORLD_CALLERS: Mutex<Vec<Value>> = Mutex::new(Vec::new());
static WORLD_PROGRAMS: std::sync::LazyLock<Mutex<HashSet<(u16, u16)>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashSet::new()));

#[derive(Clone)]
pub(crate) enum Replacement {
    Body(crate::native_body::Replacement),
    Cape(crate::native_capes::Replacement),
    HideCape(crate::native_capes::Hidden),
    Elytra(crate::native_elytra::Replacement),
}
impl Replacement {
    pub(crate) fn alpha(&self) -> f32 {
        match self {
            Self::Body(body) => body.suppression_alpha,
            Self::Cape(_) | Self::HideCape(_) | Self::Elytra(_) => 0.0001,
        }
    }
    pub(crate) fn method(&self) -> &str {
        match self {
            Self::Body(body) => &body.suppression_method,
            Self::Cape(_) | Self::HideCape(_) | Self::Elytra(_) => "bones",
        }
    }
}
#[derive(Clone)]
pub(crate) enum MeshRole {
    Skin,
    Cape {
        animation_frame: u64,
        native_cape: Option<String>,
    },
    Elytra {
        animation_frame: u64,
    },
    Other,
}
#[derive(Clone)]
pub(crate) struct Tag {
    pub scope: u64,
    pub actor: String,
    pub geometry: String,
    /// Selected native hierarchy/texture role, independent of replacement.
    pub role: MeshRole,
    pub replacement: Option<Replacement>,
}
struct Packet {
    frame: FrameKey,
    index: u32,
    tag: Tag,
    at: Instant,
    seen: bool,
}
thread_local! {
    static ACTIVE:RefCell<Vec<Option<Tag>>>=const{RefCell::new(Vec::new())};
    static SKIPPED:Cell<usize>=const{Cell::new(0)};
}
pub(crate) struct Guard {
    pushed: bool,
    _shader_scope: crate::native_shader_consumer::Scope,
}
impl Drop for Guard {
    fn drop(&mut self) {
        if self.pushed {
            ACTIVE.with(|s| {
                s.borrow_mut().pop();
            });
        } else {
            SKIPPED.with(|s| s.set(s.get() - 1));
        }
    }
}
pub(crate) fn begin(tag: Option<Tag>) -> Guard {
    if let Some(tag) = tag.as_ref() {
        ACTOR_SCOPE.store(tag.scope, Ordering::Relaxed);
    }
    let pushed = ACTIVE.with(|s| {
        let mut s = s.borrow_mut();
        if s.len() >= 8 {
            return false;
        }
        s.push(tag);
        true
    });
    if !pushed {
        SKIPPED.with(|s| s.set(s.get() + 1));
    }
    Guard {
        pushed,
        _shader_scope: crate::native_shader_consumer::begin_scope(),
    }
}
pub(crate) fn active() -> Option<Tag> {
    if SKIPPED.with(Cell::get) != 0 {
        return None;
    }
    ACTIVE.with(|s| s.borrow().last().cloned().flatten())
}
fn integer(address: usize) -> Option<u32> {
    let mut bytes = [0; 4];
    read(address, &mut bytes).ok()?;
    Some(u32::from_le_bytes(bytes))
}
#[derive(Clone, Copy)]
struct Header {
    frame: FrameKey,
    items: u32,
    submitted: u32,
}
fn header(encoder: usize) -> Option<Header> {
    let frame = crate::render_bridge::producer_frame()?;
    if pointer(encoder.checked_add(layout::ENCODER_FRAME)?)? != frame.address {
        return None;
    }
    let items = integer(frame.address.checked_add(layout::ITEMS_COUNT)?)?;
    let submitted = integer(encoder.checked_add(layout::ENCODER_SUBMITTED)?)?;
    if usize::try_from(items).ok()? >= layout::ITEM_CAPACITY
        || pointer(encoder.checked_add(layout::ENCODER_FRAME)?)? != frame.address
    {
        return None;
    }
    Some(Header {
        frame,
        items,
        submitted,
    })
}
fn allocated(before: Header, after: Header) -> Option<u32> {
    // Another encoder can reserve an item concurrently. A counter interval is
    // usable only if this call submitted exactly the sole allocated item.
    (before.frame == after.frame
        && before.items.checked_add(1) == Some(after.items)
        && before.submitted.checked_add(1) == Some(after.submitted))
    .then_some(before.items)
}
fn remember(frame: FrameKey, index: u32, tag: Tag, view: u16, program: u16) -> Option<()> {
    let slot = usize::try_from(index).ok()?;
    if slot >= layout::ITEM_CAPACITY {
        return None;
    }
    let address = frame
        .address
        .checked_add(layout::ITEMS_BASE)?
        .checked_add(slot.checked_mul(layout::ITEM_STRIDE)?)?;
    let indices = integer(address.checked_add(layout::ITEM_INDICES)?)?;
    let vertices = integer(address.checked_add(layout::ITEM_VERTICES)?)?;
    if indices == 0 && vertices == 0 {
        return None;
    }
    let mut packets = PACKETS.lock().ok()?;
    packets.retain(|p| p.at.elapsed() < Duration::from_millis(500));
    if packets.len() >= MAX_PACKETS || packets.iter().any(|p| p.frame == frame && p.index == index)
    {
        return None;
    }
    let sample = json!({"worldScope":tag.scope,"actorId":tag.actor,"geometry":tag.geometry,
        "frameNumber":frame.number,"itemIndex":index,"view":view,"program":program,
        "indices":indices,"vertices":vertices,"gpuMatched":false});
    packets.push(Packet {
        frame,
        index,
        tag,
        at: Instant::now(),
        seen: false,
    });
    if let Ok(mut last) = LAST.lock() {
        *last = Some(sample);
    }
    OWNED.fetch_add(1, Ordering::Relaxed);
    Some(())
}
/// A pointer match is checked against the actual backend Frame, not a copy of
/// the most recent draw or a constant-buffer address that other actors may use.
pub(crate) fn observe_packet(frame: FrameKey, draw: usize, scope: Option<u64>) {
    let Some(offset) = draw.checked_sub(frame.address) else {
        return;
    };
    let Some(offset) = offset.checked_sub(layout::ITEMS_BASE) else {
        return;
    };
    if offset % layout::ITEM_STRIDE != 0 || offset / layout::ITEM_STRIDE >= layout::ITEM_CAPACITY {
        return;
    }
    let Ok(mut packets) = PACKETS.lock() else {
        return;
    };
    let Some(packet) = packets.iter_mut().find(|p| {
        p.frame == frame
            && p.index as usize == offset / layout::ITEM_STRIDE
            && p.at.elapsed() < Duration::from_millis(500)
    }) else {
        return;
    };
    if packet.seen || scope != Some(packet.tag.scope) {
        return;
    }
    packet.seen = true;
    GPU_MATCHES.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut last) = LAST.lock() {
        *last = Some(json!({"worldScope":packet.tag.scope,
        "actorId":packet.tag.actor,"geometry":packet.tag.geometry,"frameNumber":frame.number,
        "itemIndex":packet.index,"gpuMatched":true}));
    }
}
unsafe extern "system" fn submit(
    encoder: usize,
    view: u16,
    program: u16,
    occlusion: u16,
    depth: u32,
    flags: u8,
) {
    CALLS.fetch_add(1, Ordering::Relaxed);
    if !INSTALLED.load(Ordering::Acquire) {
        let original: Submit = unsafe { std::mem::transmute(ORIGINAL.load(Ordering::Acquire)) };
        unsafe { original(encoder, view, program, occlusion, depth, flags) };
        return;
    }
    if crate::world_hooks::current_pass().is_some() && ACTOR_SCOPE.load(Ordering::Relaxed) != 0 {
        WORLD_CALLS.fetch_add(1, Ordering::Relaxed);
        // Walk each view/program stack at most once. Repeated draw calls must
        // not add unbounded stack-walk work to the real renderer.
        let sample = WORLD_PROGRAMS
            .try_lock()
            .is_ok_and(|mut programs| programs.len() < 32 && programs.insert((view, program)));
        if sample && let Ok(mut samples) = WORLD_CALLERS.try_lock() {
            let callers = crate::world_hooks::game_callers();
            if !callers.is_empty() {
                samples.push(json!({"view":view,"program":program,"callers":callers}));
            }
        }
    }
    let tag = active();
    let before = tag.as_ref().and_then(|_| header(encoder));
    if tag.is_some() {
        ACTOR_CALLS.fetch_add(1, Ordering::Relaxed);
    }
    let original: Submit = unsafe { std::mem::transmute(ORIGINAL.load(Ordering::Acquire)) };
    unsafe { original(encoder, view, program, occlusion, depth, flags) };
    if let Some(tag) = tag {
        let result = std::panic::catch_unwind(|| {
            let before = before?;
            let after = header(encoder)?;
            remember(after.frame, allocated(before, after)?, tag, view, program)
        });
        if !matches!(result, Ok(Some(()))) {
            REJECTED.fetch_add(1, Ordering::Relaxed);
        }
    }
}
pub(crate) fn install(base: usize) -> Result<(), String> {
    crate::render_bridge::install_producer(base)?;
    let binding = profile::binding(base)?;
    INSTALL
        .get_or_init(|| {
            let target = base
                .checked_add(binding.rva)
                .ok_or("skin submission address overflow")?;
            use hudhook::mh::{MH_CreateHook, MH_EnableHook, MH_STATUS};
            let mut original = std::ptr::null_mut();
            let status =
                unsafe { MH_CreateHook(target as _, submit as *const () as _, &mut original) };
            if status != MH_STATUS::MH_OK {
                return Err(format!("create skin submission observer: {status:?}"));
            }
            ORIGINAL.store(original as usize, Ordering::Release);
            let status = unsafe { MH_EnableHook(target as _) };
            if status != MH_STATUS::MH_OK {
                return Err(format!("enable skin submission observer: {status:?}"));
            }
            INSTALLED.store(true, Ordering::Release);
            Ok(())
        })
        .clone()
}
pub(crate) fn status() -> Value {
    json!({"installed":INSTALLED.load(Ordering::Acquire),"hookRva":profile::current().map(|binding|binding.rva),
        "encoderAndItemReaderSourceImplemented":true,
        "calls":CALLS.load(Ordering::Relaxed),"actorCalls":ACTOR_CALLS.load(Ordering::Relaxed),
        "ownedItems":OWNED.load(Ordering::Relaxed),"rejected":REJECTED.load(Ordering::Relaxed),
        "gpuMatches":GPU_MATCHES.load(Ordering::Relaxed),"last":LAST.lock().ok().and_then(|v|v.clone()),
        "worldCalls":WORLD_CALLS.load(Ordering::Relaxed),"worldCallers":WORLD_CALLERS.lock().ok().map(|v|v.clone()),
        "drawsChanged":false})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn concurrent_reused_or_dropped_reservations_have_no_owner() {
        let frame = FrameKey {
            address: 0x10000,
            number: 17,
        };
        let before = Header {
            frame,
            items: 31,
            submitted: 8,
        };
        let after = Header {
            frame,
            items: 32,
            submitted: 9,
        };
        assert_eq!(allocated(before, after), Some(31));
        assert_eq!(allocated(before, Header { items: 33, ..after }), None);
        assert_eq!(
            allocated(
                before,
                Header {
                    submitted: 8,
                    ..after
                }
            ),
            None
        );
        assert_eq!(
            allocated(
                before,
                Header {
                    frame: FrameKey {
                        number: 18,
                        ..frame
                    },
                    ..after
                }
            ),
            None
        );
        let _outer = begin(Some(Tag {
            scope: 1,
            actor: "actor".into(),
            geometry: "default".into(),
            role: MeshRole::Other,
            replacement: None,
        }));
        assert!(active().is_some());
        {
            let _inner = begin(None);
            assert!(active().is_none());
        }
        assert_eq!(active().unwrap().scope, 1);
    }
    #[test]
    fn gpu_packet_requires_exact_frame_draw_address_and_owned_scope() {
        let index = 1;
        let offset = layout::ITEMS_BASE + index * layout::ITEM_STRIDE;
        let mut bytes = vec![0u8; offset + layout::ITEM_STRIDE];
        bytes[offset + layout::ITEM_INDICES..offset + layout::ITEM_INDICES + 4]
            .copy_from_slice(&36u32.to_le_bytes());
        bytes[offset + layout::ITEM_VERTICES..offset + layout::ITEM_VERTICES + 4]
            .copy_from_slice(&24u32.to_le_bytes());
        let frame = FrameKey {
            address: bytes.as_ptr() as usize,
            number: 3,
        };
        let tag = Tag {
            scope: 19,
            actor: "actor".into(),
            geometry: "default".into(),
            role: MeshRole::Other,
            replacement: None,
        };
        assert!(remember(frame, index as u32, tag, 7, 8).is_some());
        let draw = frame.address + offset;
        let seen = || {
            PACKETS
                .lock()
                .unwrap()
                .iter()
                .find(|p| p.frame == frame && p.index == 1)
                .unwrap()
                .seen
        };
        observe_packet(frame, draw, None);
        observe_packet(frame, draw, Some(20));
        observe_packet(FrameKey { number: 4, ..frame }, draw, Some(19));
        observe_packet(frame, draw + 8, Some(19));
        assert!(!seen());
        observe_packet(frame, draw, Some(19));
        assert!(seen());
        PACKETS.lock().unwrap().retain(|p| p.frame != frame);
    }
    #[test]
    fn submit_forwards_view_handles_depth_and_discard_flags_unchanged() {
        static ARGS: Mutex<[usize; 6]> = Mutex::new([0; 6]);
        unsafe extern "system" fn original(a: usize, b: u16, c: u16, d: u16, e: u32, f: u8) {
            *ARGS.lock().unwrap() = [
                a, b as usize, c as usize, d as usize, e as usize, f as usize,
            ];
        }
        ORIGINAL.store(original as *const () as usize, Ordering::Release);
        unsafe { submit(0x11223344, 255, 0xabcd, 0xffff, 0x87654321, 0xff) };
        assert_eq!(
            *ARGS.lock().unwrap(),
            [0x11223344, 255, 0xabcd, 0xffff, 0x87654321, 0xff]
        );
    }
}
