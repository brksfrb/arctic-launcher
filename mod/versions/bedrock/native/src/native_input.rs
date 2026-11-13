//! Read-only processed local movement input for the exact executable profile.
//! This observes the game's input component, not physical keys or velocity.
use crate::world_hooks::read;
use serde::Serialize;

const COMPONENT: u32 = 0x018b1887;
const STRIDE: usize = 100;

#[derive(Serialize)]
pub struct Movement {
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub jump: bool,
    pub sneak: bool,
    pub analog: [f32; 2],
}

/// The pinned native accessors at 0x28348f0/0x28349d0 index this EnTT payload
/// with `imul ..., 0x64`. Jump reads +0x60 bit 4; sneak reads input-state bit 0.
/// The movement predicate at 0x2834b90 checks input-state direction bits 13–16
/// and both processed movement axes at +0x24/+0x28. No engine code is called.
pub(crate) fn movement(actor: usize) -> Option<Movement> {
    crate::world_hooks::image_base()?;
    let context = crate::ecs::actor_context(actor).ok()?;
    if !crate::ecs::contains_context(context, crate::ecs::hash("LocalPlayerComponent")).ok()? {
        return None;
    }
    let component = crate::ecs::component_context(context, COMPONENT, STRIDE).ok()??;
    let mut before = [0u8; STRIDE];
    read(component, &mut before).ok()?;
    let input = u32::from_le_bytes(before[..4].try_into().ok()?);
    let flags = u16::from_le_bytes(before[96..98].try_into().ok()?);
    let analog = [
        f32::from_le_bytes(before[36..40].try_into().ok()?),
        f32::from_le_bytes(before[40..44].try_into().ok()?),
    ];
    // Do not interpret unused bitfield padding as additional input flags.
    if before[98] > 1 || before[99] > 1 || analog.iter().any(|v| !v.is_finite() || v.abs() > 1.001)
    {
        return None;
    }
    let mut after = [0u8; STRIDE];
    read(component, &mut after).ok()?;
    if before != after
        || crate::ecs::actor_context(actor).ok()? != context
        || crate::ecs::component_context(context, COMPONENT, STRIDE).ok()?? != component
    {
        return None;
    }
    let bit = |index: u32| input & (1u32 << index) != 0;
    Some(Movement {
        forward: bit(13) || bit(9) || bit(10),
        backward: bit(14) || bit(11) || bit(12),
        left: bit(15) || bit(9) || bit(11),
        right: bit(16) || bit(10) || bit(12),
        jump: bit(7) || flags & (1 << 4) != 0,
        sneak: bit(0) || bit(1) || flags & 1 != 0,
        analog,
    })
}
