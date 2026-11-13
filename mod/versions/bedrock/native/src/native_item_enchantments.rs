//! Read-only installed ItemStackBase::isEnchanted predicate and optional list
//! header and bounded raw id/lvl copies. No native call, inventory mutation
//! or inferred Java enchantment/foil component conversion.
use crate::world_hooks::{pointer, read};
use serde::Serialize;
use std::sync::{
    OnceLock,
    atomic::{AtomicU64, Ordering},
};

const COMPOUND_VTABLE: usize = 0xe821430;
const LIST_VTABLE: usize = 0xe7d0200;
const COMPOUND_ID: usize = 0xa45940;
const LIST_ID: usize = 0x254f660;
const SHORT_VTABLE: usize = 0xe821800;
const SHORT_ID: usize = 0x3164e0;
const LOOKUP: usize = 0x2548830;
const MAX_TAGS: usize = 128;
const MAX_DEPTH: usize = 32;
const MAX_LIST_ENTRIES: usize = 256;
const MAX_LIST_CAPACITY: usize = 4096;
const MAX_PAYLOAD_ENTRIES: usize = 64;
const MAX_PAYLOAD_NODES: usize = 512;
static LAYOUT: OnceLock<bool> = OnceLock::new();
static LIST_LAYOUT: OnceLock<bool> = OnceLock::new();
static PAYLOAD_LAYOUT: OnceLock<bool> = OnceLock::new();
static ATTEMPTS: AtomicU64 = AtomicU64::new(0);
static COPIES: AtomicU64 = AtomicU64::new(0);
static LIST_HEADERS: AtomicU64 = AtomicU64::new(0);
static PAYLOADS: AtomicU64 = AtomicU64::new(0);

pub(super) fn status() -> serde_json::Value {
    serde_json::json!({"attempts":ATTEMPTS.load(Ordering::Relaxed),"copies":COPIES.load(Ordering::Relaxed),
        "maxRootTags":MAX_TAGS,"maxSearchDepth":MAX_DEPTH,"maxEnchantmentListEntries":MAX_LIST_ENTRIES,
        "listHeaderCopies":LIST_HEADERS.load(Ordering::Relaxed),"listHeaderLayoutVerified":false,
        "rawEntryCopies":PAYLOADS.load(Ordering::Relaxed),"maxRawEntries":MAX_PAYLOAD_ENTRIES,
        "maxRawEntrySearchNodes":MAX_PAYLOAD_NODES,"rawEntryLayoutVerified":false,
        "javaComponentsComputedInNativeReader":false,"javaComponentProjectionRuntimeVerified":false,"enchantmentLayoutVerified":false,
        "nativeGlintPredicateVerified":false})
}
fn list_layout(base: usize) -> bool {
    // ListTag::toString computes (end-begin)>>3, then reads mType at +0x20.
    // These are separate gates: a failed size layout must not change the
    // already known native list-presence predicate into a false result.
    let mut vector = [0u8; 12];
    let mut kind = [0u8; 4];
    pointer(base + LIST_VTABLE + 0x20) == Some(base + 0x254f670)
        && read(base + 0x254f6a5, &mut vector).is_ok()
        && vector
            == [
                0x48, 0x8b, 0x56, 0x10, 0x48, 0x2b, 0x56, 0x08, 0x48, 0xc1, 0xfa, 0x03,
            ]
        && read(base + 0x254f6c6, &mut kind).is_ok()
        && kind == [0x0f, 0xb6, 0x4e, 0x20]
}

#[derive(Clone, Serialize, PartialEq, Eq)]
pub struct Entry {
    /// Full signed ShortTag value, before the installed parser narrows id to
    /// its low byte. Null means a coherently absent/non-short field.
    pub id: Option<i16>,
    pub level: Option<i16>,
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct State {
    pub present: bool,
    pub list_size: Option<u16>,
    pub element_type: Option<u8>,
    pub entries: Option<Vec<Entry>>,
}
impl State {
    fn absent() -> Self {
        Self {
            present: false,
            list_size: None,
            element_type: None,
            entries: None,
        }
    }
}

/// The map variant copy already contains the complete ListTag header. These
/// checks are independent of the optional payload reader. Empty lists retain
/// a real size of zero rather than becoming an absent tag.
fn list_header(node: &[u8; 0x70]) -> Option<(u16, u8)> {
    let start = word(node, 0x48)?;
    let end = word(node, 0x50)?;
    let capacity = word(node, 0x58)?;
    let kind = node[0x60];
    if kind > 11 {
        return None;
    }
    if start == 0 {
        return (end == 0 && capacity == 0).then_some((0, kind));
    }
    address(start)?;
    address(end)?;
    address(capacity)?;
    let bytes = end.checked_sub(start)?;
    let reserved = capacity.checked_sub(start)?;
    if start % 8 != 0
        || !bytes.is_multiple_of(8)
        || !reserved.is_multiple_of(8)
        || end > capacity
        || bytes / 8 > MAX_LIST_ENTRIES
        || reserved / 8 > MAX_LIST_CAPACITY
        || bytes != 0 && kind == 0
    {
        return None;
    }
    Some(((bytes / 8) as u16, kind))
}
fn layout(base: usize) -> bool {
    let mut code = [0u8; 16];
    if read(base + LOOKUP, &mut code).is_err()
        || code
            != [
                0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x83, 0xec,
                0x20, 0x48,
            ]
        || pointer(base + COMPOUND_VTABLE + 0x28) != Some(base + COMPOUND_ID)
        || pointer(base + LIST_VTABLE + 0x28) != Some(base + LIST_ID)
    {
        return false;
    }
    let mut compound = [0u8; 3];
    let mut list = [0u8; 3];
    let mut predicate = [0u8; 9];
    read(base + COMPOUND_ID, &mut compound).is_ok()
        && compound == [0xb0, 10, 0xc3]
        && read(base + LIST_ID, &mut list).is_ok()
        && list == [0xb0, 9, 0xc3]
        && read(base + 0x2488463, &mut predicate).is_ok()
        && predicate == [0x48, 0x8b, 0x49, 0x10, 0x48, 0x85, 0xc9, 0x74, 0x3e]
}
fn word(bytes: &[u8], offset: usize) -> Option<usize> {
    Some(usize::from_le_bytes(
        bytes.get(offset..offset + 8)?.try_into().ok()?,
    ))
}
fn address(value: usize) -> Option<usize> {
    (0x10000..0x0000_8000_0000_0000)
        .contains(&value)
        .then_some(value)
}

#[derive(PartialEq, Eq)]
struct CompoundObservation {
    found: bool,
    root: [u8; 24],
    sentinel: [u8; 32],
    path: Vec<(usize, [u8; 0x70], String)>,
}
impl CompoundObservation {
    fn node(&self) -> Option<&[u8; 0x70]> {
        if self.found {
            self.path.last().map(|(_, bytes, _)| bytes)
        } else {
            None
        }
    }
    fn compatible(&self, other: &Self) -> bool {
        self.root == other.root
            && self.sentinel == other.sentinel
            && self.path.iter().all(|(at, bytes, key)| {
                other
                    .path
                    .iter()
                    .find(|(candidate, _, _)| candidate == at)
                    .is_none_or(|(_, value, name)| value == bytes && name == key)
            })
    }
}
#[derive(PartialEq, Eq)]
struct PayloadObservation {
    pointers: Vec<u8>,
    fields: Vec<(CompoundObservation, CompoundObservation)>,
    entries: Vec<Entry>,
}
#[derive(PartialEq, Eq)]
struct Observation {
    state: State,
    compound: CompoundObservation,
    payload: Option<PayloadObservation>,
}
fn tag_id(base: usize, node: &[u8; 0x70]) -> Option<u8> {
    let index = node[0x68];
    if index > 11 {
        return None;
    }
    let table = word(node, 0x40)?;
    if !(base + 0xe5a7000..base + 0x117da000).contains(&table) {
        return None;
    }
    let getter = pointer(table.checked_add(0x28)?)?;
    if !(base + 0x1000..base + 0xe5a7000 - 6).contains(&getter) {
        return None;
    }
    let mut code = [0u8; 6];
    read(getter, &mut code).ok()?;
    let literal = code[..3] == [0xb0, index, 0xc3]
        || code == [0xb8, index, 0, 0, 0, 0xc3]
        || index == 0 && (code[..3] == [0x31, 0xc0, 0xc3] || code[..3] == [0x33, 0xc0, 0xc3]);
    if !literal || index == 9 && (table != base + LIST_VTABLE || getter != base + LIST_ID) {
        return None;
    }
    Some(index)
}
fn lookup(
    base: usize,
    tag: usize,
    wanted: &[u8],
    node_limit: usize,
) -> Option<CompoundObservation> {
    let mut root = [0u8; 24];
    read(tag, &mut root).ok()?;
    if word(&root, 0)? != base + COMPOUND_VTABLE {
        return None;
    }
    let head = address(word(&root, 8)?)?;
    let count = word(&root, 16)?;
    if count > MAX_TAGS {
        return None;
    }
    let mut sentinel = [0u8; 32];
    read(head, &mut sentinel).ok()?;
    if sentinel[0x18] != 1 || sentinel[0x19] != 1 {
        return None;
    }
    let mut node = address(word(&sentinel, 8)?)?;
    if count == 0 {
        if (0..3).any(|i| word(&sentinel, i * 8) != Some(head)) {
            return None;
        }
        return Some(CompoundObservation {
            found: false,
            root,
            sentinel,
            path: Vec::new(),
        });
    }
    if node == head || word(&sentinel, 0) == Some(head) || word(&sentinel, 16) == Some(head) {
        return None;
    }
    let mut path: Vec<(usize, [u8; 0x70], String)> = Vec::new();
    let mut parent = head;
    let mut lower: Option<String> = None;
    let mut upper: Option<String> = None;
    while node != head {
        if path.len() >= MAX_DEPTH
            || path.len() >= node_limit
            || path.len() >= count
            || path.iter().any(|(visited, _, _)| *visited == node)
        {
            return None;
        }
        let mut bytes = [0u8; 0x70];
        read(node, &mut bytes).ok()?;
        if bytes[0x19] != 0 || bytes[0x18] > 1 || word(&bytes, 8) != Some(parent) {
            return None;
        }
        let key = crate::players::string_header(bytes[0x20..0x40].try_into().ok()?, 256)?;
        if lower
            .as_ref()
            .is_some_and(|value| key.as_bytes() <= value.as_bytes())
            || upper
                .as_ref()
                .is_some_and(|value| key.as_bytes() >= value.as_bytes())
        {
            return None;
        }
        let order = key.as_bytes().cmp(wanted);
        let next = match order {
            std::cmp::Ordering::Less => {
                lower = Some(key.clone());
                address(word(&bytes, 16)?)?
            }
            std::cmp::Ordering::Greater => {
                upper = Some(key.clone());
                address(word(&bytes, 0)?)?
            }
            std::cmp::Ordering::Equal => {
                path.push((node, bytes, key));
                return Some(CompoundObservation {
                    found: true,
                    root,
                    sentinel,
                    path,
                });
            }
        };
        path.push((node, bytes, key));
        parent = node;
        node = next;
    }
    Some(CompoundObservation {
        found: false,
        root,
        sentinel,
        path,
    })
}

fn payload_layout(base: usize) -> bool {
    let mut getter = [0u8; 3];
    let mut filter = [0u8; 14];
    let mut load = [0u8; 4];
    let mut id_key = [0u8; 3];
    let mut level_key = [0u8; 4];
    let mut id_read = [0u8; 4];
    let mut level_read = [0u8; 5];
    pointer(base+SHORT_VTABLE+0x28)==Some(base+SHORT_ID)
        &&read(base+SHORT_ID,&mut getter).is_ok()&&getter==[0xb0,2,0xc3]
        // CompoundTag's typed short lookup returns node+0x40 only for index 2.
        &&read(base+0x2548b08,&mut filter).is_ok()
        &&filter==[0x48,0x8d,0x4b,0x40,0x31,0xc0,0x80,0x7b,0x68,2,0x48,0x0f,0x44,0xc1]
        // ShortTag::copy reads the 16-bit value from object+8.
        &&read(base+0x2555acb,&mut load).is_ok()&&load==[0x0f,0xb7,0x4f,8]
        &&read(base+0xec87fda,&mut id_key).is_ok()&&id_key==*b"id\0"
        &&read(base+0xec3762f,&mut level_key).is_ok()&&level_key==*b"lvl\0"
        // ItemEnchants::_fromList uses id's low byte and a signed short level.
        &&read(base+0x519b611,&mut id_read).is_ok()&&id_read==[0x0f,0xb6,0x70,8]
        &&read(base+0x519b647,&mut level_read).is_ok()&&level_read==[0x48,0x0f,0xbf,0x78,8]
}
fn short(base: usize, field: &CompoundObservation) -> Option<Option<i16>> {
    let Some(node) = field.node() else {
        return Some(None);
    };
    if tag_id(base, node)? != 2 {
        return Some(None);
    }
    if word(node, 0x40)? != base + SHORT_VTABLE {
        return None;
    }
    Some(Some(i16::from_le_bytes(node[0x48..0x4a].try_into().ok()?)))
}
fn observe_list(base: usize, node: &[u8; 0x70], size: u16, kind: u8) -> Option<PayloadObservation> {
    let size = usize::from(size);
    if size > MAX_PAYLOAD_ENTRIES || size != 0 && kind != 10 {
        return None;
    }
    let mut pointers = vec![0u8; size.checked_mul(8)?];
    if size != 0 {
        read(word(node, 0x48)?, &mut pointers).ok()?;
    }
    let mut fields = Vec::new();
    let mut entries = Vec::new();
    let mut seen = Vec::new();
    let mut budget = MAX_PAYLOAD_NODES;
    for i in 0..size {
        let entry = address(word(&pointers, i * 8)?)?;
        if entry % 8 != 0 || seen.contains(&entry) {
            return None;
        }
        seen.push(entry);
        let id = lookup(base, entry, b"id", budget)?;
        budget = budget.checked_sub(id.path.len())?;
        let level = lookup(base, entry, b"lvl", budget)?;
        budget = budget.checked_sub(level.path.len())?;
        if !id.compatible(&level) {
            return None;
        }
        entries.push(Entry {
            id: short(base, &id)?,
            level: short(base, &level)?,
        });
        fields.push((id, level));
    }
    Some(PayloadObservation {
        pointers,
        fields,
        entries,
    })
}
fn observe(base: usize, tag: usize) -> Option<Observation> {
    let compound = lookup(base, tag, b"ench", MAX_DEPTH)?;
    let mut state = State::absent();
    let mut payload = None;
    if let Some(node) = compound.node()
        && tag_id(base, node)? == 9
    {
        state.present = true;
        let header = if *LIST_LAYOUT.get_or_init(|| list_layout(base)) {
            list_header(node)
        } else {
            None
        };
        if let Some((size, kind)) = header {
            state.list_size = Some(size);
            state.element_type = Some(kind);
            if *PAYLOAD_LAYOUT.get_or_init(|| payload_layout(base)) {
                payload = observe_list(base, node, size, kind);
                state.entries = payload.as_ref().map(|copy| copy.entries.clone());
            }
        }
    }
    Some(Observation {
        state,
        compound,
        payload,
    })
}

pub(super) fn observe_stack(stack: &[u8; 40]) -> Option<State> {
    ATTEMPTS.fetch_add(1, Ordering::Relaxed);
    let base = crate::world_hooks::image_base()?;
    if !*LAYOUT.get_or_init(|| layout(base)) || stack[34] == 0 || stack[35] != 1 {
        return None;
    }
    let tag = word(stack, 16)?;
    let state = if tag == 0 {
        State::absent()
    } else {
        let tag = address(tag)?;
        let first = observe(base, tag)?;
        let second = observe(base, tag)?;
        if first != second {
            return None;
        }
        first.state
    };
    if state.list_size.is_some() {
        LIST_HEADERS.fetch_add(1, Ordering::Relaxed);
    }
    if state.entries.is_some() {
        PAYLOADS.fetch_add(1, Ordering::Relaxed);
    }
    COPIES.fetch_add(1, Ordering::Relaxed);
    Some(state)
}
