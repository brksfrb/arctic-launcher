//! Whole-function admission for the native world, actor and completed-tick hooks.
use crate::native_scene_layout::{self as scene, level_tick as layout};
use crate::world_hooks::{image_base, read, read_profile_function};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    sync::OnceLock,
};

const CATALOG: &str = include_str!("../inspection/engine-entry-bindings.json");
static BINDING: OnceLock<Result<(usize, usize, [usize; 3]), String>> = OnceLock::new();

fn number(value: &Value) -> Result<usize, String> {
    value
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| "invalid engine entry binding number".into())
}
fn resolve(base: usize, image_size: usize) -> Result<(usize, usize, [usize; 3]), String> {
    let catalog: Value = serde_json::from_str(CATALOG).map_err(|error| error.to_string())?;
    if catalog["schema"] != 1 {
        return Err("engine entry schema differs".into());
    }
    let profiles = catalog["profiles"]
        .as_array()
        .ok_or("engine entry profiles absent")?;
    let mut selected = profiles
        .iter()
        .filter(|p| p["productVersion"].as_str() == Some(scene::PRODUCT_VERSION));
    let profile = selected
        .next()
        .ok_or("engine entry scene family unavailable")?;
    let world_stack = number(&profile["worldStackBytes"])?;
    if selected.next().is_some()
        || number(&profile["imageSize"])? != image_size
        || profile["worldAbi"].as_str() != Some("rendererScreenFrame3Void")
        || profile["actorAbi"].as_str()
            != Some("dispatcherContextActorTargetPositionRotationByte7Void")
        || profile["tickAbi"].as_str() != Some("level1Void")
        || !matches!(
            (scene::PRODUCT_VERSION, world_stack),
            ("1.26.5203.0", 0x248) | ("1.26.60.5", 0x1f8)
        )
        || number(&profile["actorStackBytes"])? != 0xe8
        || number(&profile["actorFlagEntryOffset"])? != 0x38
        || number(&profile["levelVtableRva"])? != layout::VTABLE
        || number(&profile["tickSlot"])? != layout::SLOT
    {
        return Err("engine entry ABI or scene family differs".into());
    }
    let functions = profile["functions"]
        .as_object()
        .filter(|functions| functions.len() == 5)
        .ok_or("engine entry function inventory differs")?;
    let mut roles = HashMap::new();
    let mut starts = HashSet::new();
    let mut total = 0usize;
    for role in ["world", "worldCaller", "actor", "tick", "levelConstructor"] {
        let function = functions.get(role).ok_or("engine entry function absent")?;
        let rva = number(&function["rva"])?;
        let length = number(&function["bytes"])?;
        total = total
            .checked_add(length)
            .filter(|bytes| *bytes <= 32 * 1024)
            .ok_or("engine entry code budget exceeded")?;
        if !starts.insert(rva) {
            return Err("engine entry function roles repeat".into());
        }
        let expected = function["sha256"]
            .as_str()
            .filter(|hash| hash.len() == 64)
            .ok_or("engine entry function digest absent")?;
        if crate::native_hash::hex(&Sha256::digest(&read_profile_function(rva, length)?))
            != expected
        {
            return Err(format!("engine entry {role} code differs"));
        }
        roles.insert(role, (rva, length));
    }
    let (caller, caller_bytes) = roles["worldCaller"];
    let caller_end = caller
        .checked_add(caller_bytes)
        .ok_or("world caller extent overflow")?;
    let calls = profile["worldCalls"]
        .as_array()
        .filter(|calls| calls.len() == 2)
        .ok_or("world call inventory differs")?;
    let mut sites = HashSet::new();
    for call in calls {
        let site = number(call)?;
        let after = site.checked_add(5).ok_or("world call overflow")?;
        if !sites.insert(site) || site < caller || after > caller_end {
            return Err("world call repeats or lies outside caller".into());
        }
        let bytes = read_profile_function(site, 5)?;
        let displacement =
            i32::from_le_bytes(bytes[1..5].try_into().map_err(|_| "world call truncated")?);
        if bytes[0] != 0xe8
            || after.checked_add_signed(displacement as isize) != Some(roles["world"].0)
        {
            return Err("native world call differs".into());
        }
    }
    // The constructor installs the concrete Level class; its tick slot must
    // name this exact complete tick function. Do not invoke either pointer.
    let lea = number(&profile["levelVtableLeaRva"])?;
    let after = lea.checked_add(7).ok_or("level table LEA overflow")?;
    let (constructor, constructor_bytes) = roles["levelConstructor"];
    if lea < constructor
        || after
            > constructor
                .checked_add(constructor_bytes)
                .ok_or("level constructor extent overflow")?
    {
        return Err("level table LEA lies outside constructor".into());
    }
    let bytes = read_profile_function(lea, 7)?;
    let displacement = i32::from_le_bytes(
        bytes[3..7]
            .try_into()
            .map_err(|_| "level table LEA truncated")?,
    );
    if bytes[..3] != [0x48, 0x8d, 0x05]
        || after.checked_add_signed(displacement as isize) != Some(layout::VTABLE)
    {
        return Err("level constructor table differs".into());
    }
    let slot = layout::VTABLE
        .checked_add(layout::SLOT)
        .ok_or("level tick slot overflow")?;
    if slot.checked_add(8).is_none_or(|end| end > image_size) || slot & 7 != 0 {
        return Err("level tick table lies outside image".into());
    }
    let mut pointer = [0u8; 8];
    read(
        base.checked_add(slot)
            .ok_or("level tick table address overflow")?,
        &mut pointer,
    )?;
    let expected = base
        .checked_add(roles["tick"].0)
        .ok_or("level tick address overflow")?;
    if usize::try_from(u64::from_le_bytes(pointer)).ok() != Some(expected) {
        return Err("concrete Level tick table differs".into());
    }
    Ok((
        base,
        image_size,
        [roles["world"].0, roles["actor"].0, roles["tick"].0],
    ))
}
pub(super) fn entry_rvas(base: usize, image_size: usize) -> Result<[usize; 3], String> {
    if image_base() != Some(base) {
        return Err("engine entry image unavailable".into());
    }
    let (accepted, accepted_size, entries) =
        BINDING.get_or_init(|| resolve(base, image_size)).clone()?;
    if accepted != base || accepted_size != image_size {
        return Err("engine entry image changed after preflight".into());
    }
    Ok(entries)
}
