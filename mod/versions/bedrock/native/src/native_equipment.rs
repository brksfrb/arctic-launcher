//! Owned read-only equipment snapshots. No inventory calls or engine mutations.
use crate::world_hooks::{pointer, read};
use serde::Serialize;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
#[path = "native_item_enchantments.rs"]
mod enchantments;
pub use enchantments::Entry as EnchantmentEntry;
static ATTEMPTS: AtomicU64 = AtomicU64::new(0);
static COPIES: AtomicU64 = AtomicU64::new(0);
static STEP: AtomicU32 = AtomicU32::new(0);
static GETTER_RVA: AtomicU64 = AtomicU64::new(0);
static HAND_GETTER_RVA: AtomicU64 = AtomicU64::new(0);
static STATIC_ICON_FRAMES: AtomicU64 = AtomicU64::new(0);
static BASE_GLINT_COPIES: AtomicU64 = AtomicU64::new(0);
pub(crate) fn status() -> serde_json::Value {
    serde_json::json!({"attempts":ATTEMPTS.load(Ordering::Relaxed),"copies":COPIES.load(Ordering::Relaxed),"step":STEP.load(Ordering::Relaxed),
        "getterRva":GETTER_RVA.load(Ordering::Relaxed),"handGetterRva":HAND_GETTER_RVA.load(Ordering::Relaxed),
        "staticIconFrames":STATIC_ICON_FRAMES.load(Ordering::Relaxed),"enchantments":enchantments::status(),
        "baseGlintCopies":BASE_GLINT_COPIES.load(Ordering::Relaxed),"baseGlintLayoutVerified":false,
        "equipmentLayoutVerified":false,"iconKeyLayoutVerified":false,"iconFrameLayoutVerified":false})
}

const GET_ITEM: usize = 0x25a5100;
const EMPTY_ITEM: usize = 0x11d65d50;
const STRIDE: usize = 0x98;

#[derive(Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Slot {
    pub empty: bool,
    /// Copied canonical ID, never a retained Item/ItemStack pointer. Unknown
    /// layout/name stays absent even when slot occupancy was readable.
    pub item: Option<String>,
    /// Zero for a canonical empty stack; unknown for unreadable validity/count.
    pub count: Option<u8>,
    /// Optional observed Item::mIconName for the item atlas. No ID-to-key guess.
    pub icon_key: Option<String>,
    /// Optional resolved base-layer index, only for the inspected static Item
    /// getter with its component/trim/alternate-atlas branches ruled out.
    pub icon_frame: Option<u16>,
    /// Installed has-enchantment-list predicate. Unknown is distinct from
    /// false; this does not include an Item's forced glint or Java overrides.
    pub enchanted: Option<bool>,
    /// Structurally checked native ench ListTag header, including size zero.
    /// This is not a Java ENCHANTMENTS component or a payload-validity claim.
    pub enchantment_list_size: Option<u16>,
    pub enchantment_list_element_type: Option<u8>,
    /// Bounded raw short fields in list order. Null means the entry copy was
    /// unavailable, not an empty list or a validated Java component map.
    pub enchantment_entries: Option<Vec<EnchantmentEntry>>,
    /// Item's forced bit, only when its selected virtual isGlint is the
    /// inspected base implementation. Overrides/unreadable state stay unknown.
    pub forced_glint: Option<bool>,
    /// Result of that checked native base predicate, distinct from Java's
    /// hasFoil overrides and from a receipt that a glint pass was rendered.
    pub native_glint: Option<bool>,
}
#[derive(Serialize)]
pub struct Equipment {
    /// Native armor order (chest is index 1). Missing group means unknown.
    pub armor: Option<[Slot; 4]>,
    /// Native hand-container order. Do not infer a selected inventory slot.
    pub hands: Option<[Slot; 2]>,
    #[serde(skip)]
    pub chest: Option<Slot>,
}

pub(crate) fn equipment(actor: usize) -> Option<Equipment> {
    ATTEMPTS.fetch_add(1, Ordering::Relaxed);
    STEP.store(0, Ordering::Relaxed);
    let component =
        crate::ecs::component(actor, crate::ecs::hash("ActorEquipmentComponent"), 16).ok()??;
    STEP.store(1, Ordering::Relaxed);
    let mut header = [0u8; 16];
    read(component, &mut header).ok()?;
    let hand = usize::from_le_bytes(header[..8].try_into().ok()?);
    let container = usize::from_le_bytes(header[8..].try_into().ok()?);
    let base = crate::world_hooks::image_base()?;
    let mut empty = [0u8; 40];
    read(base.checked_add(EMPTY_ITEM)?, &mut empty).ok()?;
    let armor = container_layout(container, base, &GETTER_RVA)
        .and_then(|(getter, offset)| read_slots_at::<4>(container, getter, offset, 4, 6, &empty));
    // Preserve independently checked chest observation if another armor slot
    // could not be copied. A missing full group never means the rest is empty.
    let chest = armor.as_ref().map(|slots| slots[1].clone()).or_else(|| {
        let (getter, offset) = container_layout(container, base, &GETTER_RVA)?;
        read_chest_state_at(container, getter, offset, &empty)
    });
    let hands = container_layout(hand, base, &HAND_GETTER_RVA)
        .and_then(|(getter, offset)| read_slots_at::<2>(hand, getter, offset, 2, 2, &empty));
    let mut after = [0u8; 16];
    read(component, &mut after).ok()?;
    if header != after
        || crate::ecs::component(actor, crate::ecs::hash("ActorEquipmentComponent"), 16).ok()??
            != component
    {
        return None;
    }
    COPIES.fetch_add(1, Ordering::Relaxed);
    STEP.store(3, Ordering::Relaxed);
    Some(Equipment {
        armor,
        hands,
        chest,
    })
}

fn container_layout(
    container: usize,
    base: usize,
    diagnostic: &AtomicU64,
) -> Option<(usize, usize)> {
    let table = pointer(container)?;
    let getter = pointer(table.checked_add(7 * 8)?)?;
    let rva = getter.checked_sub(base)?;
    diagnostic.store(rva as u64, Ordering::Relaxed);
    let offset = match rva {
        GET_ITEM => 0x198,
        0x19df320 | 0x19df740 => 0x1a0,
        _ => return None,
    };
    Some((getter, offset))
}

fn slot(stack: &[u8; 40], empty_reference: &[u8; 40]) -> Slot {
    let empty = empty_reference[34] == 0 && stack[..36] == empty_reference[..36];
    let identity = if empty { None } else { item_identity(stack) };
    let (item, icon_key, icon_frame, forced_glint) = match identity {
        Some(identity) => (
            Some(identity.id),
            identity.icon_key,
            identity.icon_frame,
            identity.forced_glint,
        ),
        None => (None, None, None, None),
    };
    let enchantments = if empty {
        None
    } else {
        enchantments::observe_stack(stack)
    };
    let enchanted = if empty {
        stack[16..24].iter().all(|byte| *byte == 0).then_some(false)
    } else {
        enchantments.as_ref().map(|state| state.present)
    };
    let enchantment_list_size = enchantments.as_ref().and_then(|state| state.list_size);
    let enchantment_list_element_type = enchantments.as_ref().and_then(|state| state.element_type);
    let enchantment_entries = enchantments.and_then(|state| state.entries);
    let native_glint = if empty {
        Some(false)
    } else {
        forced_glint.and_then(|forced| if forced { Some(true) } else { enchanted })
    };
    Slot {
        empty,
        item,
        icon_key,
        icon_frame,
        enchanted,
        enchantment_list_size,
        enchantment_list_element_type,
        enchantment_entries,
        forced_glint,
        native_glint,
        count: if empty {
            Some(0)
        } else {
            (stack[34] > 0 && stack[35] == 1).then_some(stack[34])
        },
    }
}

fn read_slots_at<const N: usize>(
    container: usize,
    getter: usize,
    offset: usize,
    min: usize,
    max: usize,
    empty: &[u8; 40],
) -> Option<[Slot; N]> {
    let table = pointer(container)?;
    if pointer(table.checked_add(7 * 8)?)? != getter {
        return None;
    }
    let mut vector = [0u8; 24];
    read(container.checked_add(offset)?, &mut vector).ok()?;
    let word = |at| usize::from_le_bytes(vector[at..at + 8].try_into().unwrap());
    let (start, end, capacity) = (word(0), word(8), word(16));
    let length = end.checked_sub(start)?;
    if start < 0x10000
        || !length.is_multiple_of(STRIDE)
        || !(min..=max).contains(&(length / STRIDE))
        || N > length / STRIDE
        || capacity < end
        || capacity.checked_sub(start)? > STRIDE * 64
    {
        return None;
    }
    let mut headers = [[0u8; 40]; N];
    for (i, header) in headers.iter_mut().enumerate() {
        let address = start.checked_add(i.checked_mul(STRIDE)?)?;
        read(address, header).ok()?;
        if pointer(address)? == 0 {
            return None;
        }
    }
    let slots: Vec<Slot> = headers.iter().map(|header| slot(header, empty)).collect();
    for (i, header) in headers.iter().enumerate() {
        let mut after = [0u8; 40];
        read(start.checked_add(i.checked_mul(STRIDE)?)?, &mut after).ok()?;
        // Recheck optional names as well as pointer/validity/count headers.
        if *header != after || slot(&after, empty) != slots[i] {
            return None;
        }
    }
    let mut after = [0u8; 24];
    read(container.checked_add(offset)?, &mut after).ok()?;
    if vector != after
        || pointer(container)? != table
        || pointer(table.checked_add(7 * 8)?)? != getter
    {
        return None;
    }
    slots.try_into().ok()
}

#[cfg(test)]
fn read_chest(container: usize, getter: usize) -> Option<bool> {
    let mut empty = [0u8; 40];
    empty[..8].copy_from_slice(&pointer(container)?.to_le_bytes());
    read_chest_at(container, getter, 0x198, &empty)
}
#[cfg(test)]
fn read_chest_at(
    container: usize,
    getter: usize,
    offset: usize,
    empty_reference: &[u8; 40],
) -> Option<bool> {
    read_chest_state_at(container, getter, offset, empty_reference).map(|slot| slot.empty)
}
fn read_chest_state_at(
    container: usize,
    getter: usize,
    offset: usize,
    empty_reference: &[u8; 40],
) -> Option<Slot> {
    let table = pointer(container)?;
    // The pinned getItem at 0x25A5100 bounds-checks the vector at +0x198/+0x1A0
    // and indexes 0x98-byte ItemStacks. Slot 1 is the native chest slot.
    if pointer(table.checked_add(7 * 8)?)? != getter {
        return None;
    }
    let mut vector = [0u8; 24];
    read(container.checked_add(offset)?, &mut vector).ok()?;
    let word = |at| usize::from_le_bytes(vector[at..at + 8].try_into().unwrap());
    let start = word(0);
    let end = word(8);
    let capacity = word(16);
    let length = end.checked_sub(start)?;
    if start < 0x10000
        || !length.is_multiple_of(STRIDE)
        || !(4..=6).contains(&(length / STRIDE))
        || capacity < end
        || capacity.checked_sub(start)? > STRIDE * 64
    {
        return None;
    }
    let address = start.checked_add(STRIDE)?;
    let mut stack = [0u8; 40];
    read(address, &mut stack).ok()?;
    if pointer(address)? == 0 {
        return None;
    }
    // Both inspected getters return the same canonical EMPTY_ITEM fallback.
    // Compare the actual empty visual state, including its validity byte,
    // rather than assuming that an empty ItemStack was zero-initialized.
    let result = slot(&stack, empty_reference);
    let mut after = [0; 40];
    read(address, &mut after).ok()?;
    let mut vector_after = [0; 24];
    read(container.checked_add(offset)?, &mut vector_after).ok()?;
    (stack == after
        && vector == vector_after
        && pointer(container)? == table
        && slot(&after, empty_reference) == result)
        .then_some(result)
}

/// Latite's public ItemStackBase/Item/HashedString declarations describe these
/// fields. Require three consistent canonical name views and stable copies;
/// this optional identity observation never authorizes native suppression.
struct ItemIdentity {
    id: String,
    icon_key: Option<String>,
    icon_frame: Option<u16>,
    forced_glint: Option<bool>,
}
fn base_forced_glint(item: usize, prefix: &[u8; 0x150]) -> Option<bool> {
    let base = crate::world_hooks::image_base()?;
    let table = pointer(item)?;
    if table != usize::from_le_bytes(prefix[..8].try_into().ok()?)
        || pointer(table.checked_add(0x28 * 8)?)? != base.checked_add(0x24cda00)?
    {
        return None;
    }
    // Actual base Item::isGlint: userData from its stack argument, followed
    // by (enchantment-list result OR item+152) & 1. Never call the virtual.
    let mut stack_load = [0u8; 9];
    let mut tail = [0u8; 8];
    read(base.checked_add(0x24cda17)?, &mut stack_load).ok()?;
    read(base.checked_add(0x24cda60)?, &mut tail).ok()?;
    if stack_load != [0x48, 0x8b, 0x4a, 0x10, 0x48, 0x85, 0xc9, 0x74, 0x3e]
        || tail != [0x0a, 0x86, 0x52, 0x01, 0, 0, 0x24, 0x01]
    {
        return None;
    }
    let mut before = [0u8; 1];
    let mut after = [0u8; 1];
    read(item.checked_add(0x152)?, &mut before).ok()?;
    read(item.checked_add(0x152)?, &mut after).ok()?;
    if before != after
        || pointer(item)? != table
        || pointer(table.checked_add(0x28 * 8)?)? != base.checked_add(0x24cda00)?
    {
        return None;
    }
    BASE_GLINT_COPIES.fetch_add(1, Ordering::Relaxed);
    Some(before[0] & 1 != 0)
}
fn static_icon_frame(item: usize, prefix: &[u8; 0x150]) -> Option<u16> {
    let base = crate::world_hooks::image_base()?;
    let table = pointer(item)?;
    if table != usize::from_le_bytes(prefix[..8].try_into().ok()?) {
        return None;
    }
    // The pinned base getIconInfo ignores stack/animation arguments when its
    // component getter and isHumanoidArmor both return false. Only its ordinary
    // item-icon branch is supported; the alternate atlas branch is not guessed.
    if pointer(table.checked_add(0x75 * 8)?)? != base.checked_add(0x24c2530)?
        || pointer(table.checked_add(0x18 * 8)?)? != base.checked_add(0x76c20)?
        || pointer(table.checked_add(10 * 8)?)? != base.checked_add(0x76c20)?
        || u64::from_le_bytes(prefix[0x98..0xa0].try_into().ok()?) != 0
    {
        return None;
    }
    let mut code = [0u8; 13];
    read(base.checked_add(0x24c2530)?, &mut code).ok()?;
    if code
        != [
            0x55, 0x41, 0x56, 0x56, 0x57, 0x53, 0x48, 0x81, 0xec, 0xd0, 0, 0, 0,
        ]
    {
        return None;
    }
    let mut zero = [0u8; 3];
    read(base.checked_add(0x76c20)?, &mut zero).ok()?;
    if zero != [0x33, 0xc0, 0xc3] && zero != [0x31, 0xc0, 0xc3] {
        return None;
    }
    if pointer(item)? != table {
        return None;
    }
    u16::try_from(i32::from_le_bytes(prefix[0x58..0x5c].try_into().ok()?)).ok()
}
fn item_identity(stack: &[u8; 40]) -> Option<ItemIdentity> {
    if stack[34] == 0 || stack[35] != 1 {
        return None;
    }
    let holder = usize::from_le_bytes(stack[8..16].try_into().ok()?);
    let item = pointer(holder)?;
    pointer(item)?;
    let mut before = [0u8; 0x150];
    read(item, &mut before).ok()?;
    let name = crate::players::string_header(before[0xd8..0xf8].try_into().ok()?, 128)?;
    let namespace = crate::players::string_header(before[0x100..0x120].try_into().ok()?, 64)?;
    let full = crate::players::string_header(before[0x128..0x148].try_into().ok()?, 193)?;
    // The public Item prefix has the same atlas/name/namespace/full-name
    // anchors as this observer. Icon metadata is optional and rechecked, never
    // sufficient to classify an unknown item or select a resource-pack stack.
    let atlas = crate::players::string_header(before[0x10..0x30].try_into().ok()?, 128);
    let icon = crate::players::string_header(before[0x68..0x88].try_into().ok()?, 193);
    let segment = |value: &str, path: bool| {
        value.bytes().all(|b| {
            b.is_ascii_lowercase()
                || b.is_ascii_digit()
                || matches!(b, b'_' | b'-' | b'.')
                || path && b == b'/'
        })
    };
    if namespace.is_empty()
        || name.is_empty()
        || !segment(&namespace, false)
        || !segment(&name, true)
        || full != format!("{namespace}:{name}")
    {
        return None;
    }
    // Copy referenced string bytes again too: matching headers alone cannot
    // detect an in-place mutation of a long std::string's allocation.
    if crate::players::string(item.checked_add(0xd8)?, 128)? != name
        || crate::players::string(item.checked_add(0x100)?, 64)? != namespace
        || crate::players::string(item.checked_add(0x128)?, 193)? != full
    {
        return None;
    }
    let atlas_address = item.checked_add(0x10)?;
    let icon_address = item.checked_add(0x68)?;
    let icon_key = icon.filter(|icon| {
        atlas.as_deref() == Some("atlas.items")
            && !icon.is_empty()
            && icon
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b':' | b'/'))
            && crate::players::string(atlas_address, 128).as_deref() == Some("atlas.items")
            && crate::players::string(icon_address, 193).as_deref() == Some(icon.as_str())
    });
    let icon_frame = icon_key
        .as_ref()
        .and_then(|_| static_icon_frame(item, &before));
    let forced_glint = base_forced_glint(item, &before);
    let mut after = [0u8; 0x150];
    read(item, &mut after).ok()?;
    if before != after || pointer(holder)? != item {
        return None;
    }
    if icon_frame.is_some() {
        STATIC_ICON_FRAMES.fetch_add(1, Ordering::Relaxed);
    }
    Some(ItemIdentity {
        id: full,
        icon_key,
        icon_frame,
        forced_glint,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chest_requires_known_getter_bounded_stable_vector_and_empty_item() {
        let mut table = [0usize; 8];
        table[7] = 0x12345678;
        let mut container = [0u8; 0x1b0];
        container[..8].copy_from_slice(&(table.as_ptr() as usize).to_le_bytes());
        let mut items = vec![0u8; STRIDE * 4];
        items[STRIDE..STRIDE + 8].copy_from_slice(&(table.as_ptr() as usize).to_le_bytes());
        let start = items.as_ptr() as usize;
        for (at, value) in [
            (0x198, start),
            (0x1a0, start + items.len()),
            (0x1a8, start + items.len()),
        ] {
            container[at..at + 8].copy_from_slice(&value.to_le_bytes());
        }
        let sample = |items: &[u8]| {
            std::hint::black_box(items);
            std::hint::black_box(&container);
            read_chest(container.as_ptr() as usize, 0x12345678)
        };
        assert_eq!(sample(&items), Some(true));
        for at in [8, 24, 34, 35] {
            items[STRIDE + at] = 1;
            assert_eq!(sample(&items), Some(false));
            items[STRIDE + at] = 0;
        }
        let mut canonical = [0u8; 40];
        canonical[..8].copy_from_slice(&(table.as_ptr() as usize).to_le_bytes());
        canonical[35] = 1;
        items[STRIDE + 35] = 1;
        std::hint::black_box(&items);
        assert_eq!(
            read_chest_at(container.as_ptr() as usize, 0x12345678, 0x198, &canonical),
            Some(true)
        );
        canonical[34] = 1;
        items[STRIDE + 34] = 1;
        std::hint::black_box(&items);
        assert_eq!(
            read_chest_at(container.as_ptr() as usize, 0x12345678, 0x198, &canonical),
            Some(false)
        );
        assert_eq!(read_chest(container.as_ptr() as usize, 0xdeadbeef), None);
        assert_eq!(read_chest(1, 0x12345678), None);
    }
}
