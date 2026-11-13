//! Copied default GameType for the pinned client-level implementation.
//! This observes metadata; it neither calls a virtual getter nor owns a level,
//! changes a game mode, or establishes a native render-resource lease.
use crate::world_hooks::{pointer, read};
use std::sync::atomic::{AtomicUsize, Ordering};

const VTABLE: usize = 0xe8e9380;
const GETTER: usize = 0x2241430;
const SLOT: usize = 0x460;
const GETTER_BYTES: &[u8] = &[
    0x48, 0x8b, 0x81, 0xf0, 0x0b, 0, 0, 0x8b, 0x80, 0x28, 0x01, 0, 0, 0xc3,
];
static BASE: AtomicUsize = AtomicUsize::new(0);

/// Called only after world_hooks validates the full pinned executable hash.
/// A mismatch disables this optional fallback without disabling direct modes.
pub(crate) fn configure(base: usize) -> Result<(), String> {
    BASE.store(0, Ordering::Release);
    let target = base
        .checked_add(GETTER)
        .ok_or("game-mode getter address overflow")?;
    let mut bytes = [0u8; 14];
    read(target, &mut bytes)?;
    let slot = base
        .checked_add(VTABLE)
        .and_then(|v| v.checked_add(SLOT))
        .ok_or("game-mode vtable address overflow")?;
    if bytes.as_slice() != GETTER_BYTES || pointer(slot) != Some(target) {
        return Err("client-level game-mode getter differs from the pinned image".into());
    }
    BASE.store(base, Ordering::Release);
    Ok(())
}
fn alive(control: usize) -> Option<()> {
    let mut bytes = [0u8; 8];
    read(control.checked_add(8)?, &mut bytes).ok()?;
    let uses = u32::from_le_bytes(bytes[..4].try_into().ok()?);
    let weak = u32::from_le_bytes(bytes[4..].try_into().ok()?);
    ((1..=1_000_000).contains(&uses) && (1..=1_000_000).contains(&weak)).then_some(())
}
fn copy(level: usize, base: usize) -> Option<([u8; 16], [u8; 4])> {
    let vtable = base.checked_add(VTABLE)?;
    if pointer(level) != Some(vtable) {
        return None;
    }
    // Base constructor copies a shared reference to level+BF0/BF8. The
    // verified getter loads [level+BF0]->128 as one signed GameType integer.
    let slot = level.checked_add(0xbf0)?;
    let mut shared = [0u8; 16];
    read(slot, &mut shared).ok()?;
    let data = usize::try_from(u64::from_le_bytes(shared[..8].try_into().ok()?)).ok()?;
    let control = usize::try_from(u64::from_le_bytes(shared[8..].try_into().ok()?)).ok()?;
    if data < 0x10000 || control < 0x10000 {
        return None;
    }
    alive(control)?;
    let mut mode = [0u8; 4];
    read(data.checked_add(0x128)?, &mut mode).ok()?;
    let mut after = [0u8; 16];
    read(slot, &mut after).ok()?;
    alive(control)?;
    if shared != after || pointer(level) != Some(vtable) {
        return None;
    }
    Some((shared, mode))
}
/// ActorGameTypeComponent::Default is resolved only against this actor's
/// unchanged client level. The enclosing component reader also revalidates
/// actor context, registry generation and raw mode around this operation.
pub(crate) fn spectator(actor: usize) -> Option<bool> {
    let base = BASE.load(Ordering::Acquire);
    if base == 0 {
        return None;
    }
    let link = actor.checked_add(0x1d8)?;
    let level = pointer(link)?;
    let before = copy(level, base)?;
    let after = copy(level, base)?;
    if before != after || pointer(link) != Some(level) || BASE.load(Ordering::Acquire) != base {
        return None;
    }
    match i32::from_le_bytes(before.1) {
        0..=2 => Some(false),
        6 => Some(true),
        _ => None,
    }
}
pub(crate) fn status() -> serde_json::Value {
    serde_json::json!({"profileMatched":BASE.load(Ordering::Acquire)!=0,
        "virtualGetterInvoked":false,"modeChanged":false,"liveDefaultModeReadVerified":false})
}
