//! Owned dimension identity from the pinned actor's shared dimension object.
//! This copies metadata only; it does not lease terrain/render resources.
use crate::world_hooks::{pointer, read};
use std::sync::atomic::{AtomicBool, Ordering};

static READY: AtomicBool = AtomicBool::new(false);
const ACCESSORS: &[(usize, &[u8])] = &[
    (
        0x24c7c90,
        &[
            0x48, 0x8b, 0x81, 0xc8, 0x01, 0, 0, 0x48, 0x8d, 0x48, 0x48, 0x48, 0x85, 0xc0, 0x48,
            0x8d, 0x05, 0xb3, 0xff, 0x32, 0x0f, 0x48, 0x0f, 0x45, 0xc1, 0xc3,
        ],
    ),
    (
        0x24c7cb0,
        &[
            0x48, 0x8b, 0x81, 0xc8, 0x01, 0, 0, 0x48, 0x85, 0xc0, 0x74, 0x04, 0x8b, 0x40, 0x78,
            0xc3, 0x31, 0xc0, 0xc3,
        ],
    ),
    (0x24c8000, &[0x48, 0x8b, 0x81, 0xc8, 0x01, 0, 0, 0xc3]),
];
pub(crate) fn configure(base: usize) -> Result<(), String> {
    READY.store(false, Ordering::Release);
    // Installed accessors agree on actor+1c8. The string accessor uses +48,
    // not the older public SDK's +20; the ID accessor reads dimension+78.
    for &(rva, expected) in ACCESSORS {
        let address = base
            .checked_add(rva)
            .ok_or("dimension accessor address overflow")?;
        let mut actual = vec![0; expected.len()];
        read(address, &mut actual)?;
        if actual.as_slice() != expected {
            return Err("dimension accessor differs from the pinned image".into());
        }
    }
    READY.store(true, Ordering::Release);
    Ok(())
}
fn alive(control: usize) -> Option<()> {
    let mut counts = [0; 8];
    read(control.checked_add(8)?, &mut counts).ok()?;
    let uses = u32::from_le_bytes(counts[..4].try_into().ok()?);
    let weak = u32::from_le_bytes(counts[4..].try_into().ok()?);
    ((1..=1_000_000).contains(&uses) && (1..=1_000_000).contains(&weak)).then_some(())
}
pub(crate) fn sample(actor: usize) -> Option<&'static str> {
    if !READY.load(Ordering::Acquire) {
        return None;
    }
    let registry = crate::ecs::registry_key(actor)?;
    let level = pointer(actor.checked_add(0x1d8)?)?;
    let slot = actor.checked_add(0x1c8)?;
    let mut shared = [0; 16];
    read(slot, &mut shared).ok()?;
    let dimension = usize::try_from(u64::from_le_bytes(shared[..8].try_into().ok()?)).ok()?;
    let control = usize::try_from(u64::from_le_bytes(shared[8..].try_into().ok()?)).ok()?;
    if dimension == 0 || control == 0 {
        return None;
    }
    alive(control)?;
    let name_at = dimension.checked_add(0x48)?;
    let id_at = dimension.checked_add(0x78)?;
    let mut header = [0; 32];
    read(name_at, &mut header).ok()?;
    // All accepted names fit the SSO buffer; never follow an unnecessary heap
    // name pointer. Header rechecks then also cover their actual characters.
    if u64::from_le_bytes(header[24..32].try_into().ok()?) >= 16 {
        return None;
    }
    let name = crate::players::string_header(&header, 16)?;
    let mut id = [0; 4];
    read(id_at, &mut id).ok()?;
    let canonical = match (name.as_str(), i32::from_le_bytes(id)) {
        ("Overworld", 0) => "overworld",
        ("Nether", 1) => "the_nether",
        ("TheEnd", 2) => "the_end",
        _ => return None,
    };
    let mut shared_after = [0; 16];
    let mut header_after = [0; 32];
    let mut id_after = [0; 4];
    read(slot, &mut shared_after).ok()?;
    read(name_at, &mut header_after).ok()?;
    read(id_at, &mut id_after).ok()?;
    alive(control)?;
    if shared != shared_after
        || header != header_after
        || id != id_after
        || crate::ecs::registry_key(actor)? != registry
        || pointer(actor.checked_add(0x1d8)?)? != level
    {
        return None;
    }
    Some(canonical)
}
