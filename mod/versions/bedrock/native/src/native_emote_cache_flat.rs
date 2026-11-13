//! 1.26.60.5 parallel key/matrix vectors. Native addresses live only in the
//! synchronous cache snapshot; persistent history receives opaque witnesses.
//! This reader does not establish native mutation/destruction coverage.
use super::Reads;
use crate::native_emote_cache_history::Witness;
use crate::native_emote_history::Slot;
use crate::native_scene_layout::{self as scene, EmoteCacheStorage};
use std::sync::Arc;

const PRODUCT_VERSION: &str = "1.26.60.5";
pub(super) const KEYS: usize = 0x120;
pub(super) const VALUES: usize = 0x138;
pub(super) const KEY_BYTES: usize = 0x38;
pub(super) const MATRIX_BYTES: usize = 64;
pub(super) const MAX_ENTRIES: usize = 1024;

pub(super) fn selected_family() -> bool {
    scene::PRODUCT_VERSION == PRODUCT_VERSION
        && scene::emote_cache::STORAGE == EmoteCacheStorage::FlatParallelVectors
}
pub(super) struct KeyEntry {
    pub(super) address: usize,
    pub(super) slot: Arc<Slot>,
}
pub(super) fn copy_keys(guards: &mut Reads, keys: usize, count: usize) -> Option<Vec<KeyEntry>> {
    if count > MAX_ENTRIES {
        return None;
    }
    let mut entries = Vec::with_capacity(count);
    let mut previous: Option<Arc<Slot>> = None;
    for index in 0..count {
        let address = keys.checked_add(index.checked_mul(KEY_BYTES)?)?;
        let bytes = guards.copy(address, KEY_BYTES)?;
        let (hash, name) = guards.key_bytes(&bytes)?;
        let slot = Arc::new(Slot {
            hash,
            name,
            group: u32::from(bytes[0x30]),
            palette: u32::from(bytes[0x31]),
        });
        if previous.as_ref().is_some_and(|old| {
            (old.hash, old.name.as_bytes(), old.group, old.palette)
                >= (slot.hash, slot.name.as_bytes(), slot.group, slot.palette)
        }) {
            return None;
        }
        previous = Some(slot.clone());
        entries.push(KeyEntry { address, slot });
    }
    Some(entries)
}

pub(super) struct Value {
    pub(super) address: usize,
    pub(super) native: [f32; 16],
    pub(super) witness: Witness,
}
pub(super) struct Lookup {
    pub(super) guards: Reads,
    /// None is a validated absent key, not a failed or missing read.
    pub(super) value: Option<Value>,
}
fn overlaps(a: usize, n: usize, b: usize, m: usize) -> bool {
    match (a.checked_add(n), b.checked_add(m)) {
        (Some(end), Some(other)) => a < other && b < end,
        _ => true,
    }
}
impl Lookup {
    pub(super) fn retained_bytes(&self) -> usize {
        self.guards.guards.capacity() * std::mem::size_of::<(usize, Vec<u8>)>()
            + self
                .guards
                .guards
                .iter()
                .map(|(_, bytes)| bytes.capacity())
                .sum::<usize>()
    }
    pub(super) fn copy(actor: usize, slot: &Slot) -> Option<Self> {
        // These candidates are not compatible with the approved older family.
        if !selected_family() || slot.group >= 64 || slot.palette >= 256 {
            return None;
        }
        let mut guards = Reads::default();
        let key_header = actor.checked_add(KEYS)?;
        let value_header = actor.checked_add(VALUES)?;
        let (keys, key_count) = guards.vector(key_header, KEY_BYTES, MAX_ENTRIES)?;
        let (values, value_count) = guards.vector(value_header, MATRIX_BYTES, MAX_ENTRIES)?;
        if key_count != value_count {
            return None;
        }
        let key_bytes = key_count.checked_mul(KEY_BYTES)?;
        let value_bytes = value_count.checked_mul(MATRIX_BYTES)?;
        if key_count != 0
            && (overlaps(keys, key_bytes, values, value_bytes)
                || overlaps(keys, key_bytes, key_header, 24)
                || overlaps(keys, key_bytes, value_header, 24)
                || overlaps(values, value_bytes, key_header, 24)
                || overlaps(values, value_bytes, value_header, 24))
        {
            return None;
        }
        let mut value = None;
        for (index, entry) in copy_keys(&mut guards, keys, key_count)?.iter().enumerate() {
            if entry.slot.as_ref() == slot {
                let address = values.checked_add(index.checked_mul(MATRIX_BYTES)?)?;
                let matrix = guards.copy(address, MATRIX_BYTES)?;
                let native: [f32; 16] = std::array::from_fn(|i| {
                    f32::from_le_bytes(matrix[i * 4..i * 4 + 4].try_into().expect("four bytes"))
                });
                if native
                    .iter()
                    .any(|v| !v.is_finite() || v.abs() > 1_000_000.)
                {
                    return None;
                }
                value = Some(Value {
                    address,
                    native,
                    witness: Witness::flat(
                        key_header,
                        value_header,
                        keys,
                        values,
                        entry.address,
                        address,
                    ),
                });
            }
        }
        guards.unchanged()?;
        Some(Self { guards, value })
    }
}
pub(super) fn status() -> serde_json::Value {
    serde_json::json!({"sourceImplemented":true,"productVersion":PRODUCT_VERSION,
        "selectedFamily":scene::PRODUCT_VERSION==PRODUCT_VERSION
            &&scene::emote_cache::STORAGE==EmoteCacheStorage::FlatParallelVectors,
        "keysOffset":KEYS,"valuesOffset":VALUES,"keyStride":KEY_BYTES,"matrixStride":MATRIX_BYTES,
        "maximumEntries":MAX_ENTRIES,"sourcePointerMatchRequired":true,
        "destructionHookSourceImplemented":true,
        "lifetimeHooksMigrated":false,"runtimeVerified":false})
}
