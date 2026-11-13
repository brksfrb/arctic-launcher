//! Context/embedded Frame metadata readers, independent of backend detours.
use crate::native_scene_layout::{self as scene, producer_frame as layout};
use crate::world_hooks::read_profile_function;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    sync::OnceLock,
};

const CATALOG: &str = include_str!("../inspection/producer-frame-reader-bindings.json");
static VALIDATED: OnceLock<Result<(usize, usize), String>> = OnceLock::new();
fn number(value: &Value) -> Result<usize, String> {
    value
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| "invalid producer frame binding number".into())
}
fn resolve(base: usize) -> Result<(usize, usize), String> {
    let catalog: Value = serde_json::from_str(CATALOG).map_err(|error| error.to_string())?;
    if catalog["schema"] != 1 {
        return Err("producer frame schema differs".into());
    }
    let profiles = catalog["profiles"]
        .as_array()
        .ok_or("producer frame profiles absent")?;
    let mut selected = profiles
        .iter()
        .filter(|profile| profile["productVersion"].as_str() == Some(scene::PRODUCT_VERSION));
    let profile = selected
        .next()
        .ok_or("producer frame scene family unavailable")?;
    if selected.next().is_some() {
        return Err("duplicate producer frame scene family".into());
    }
    for (key, expected) in [
        ("contextGlobal", layout::CONTEXT_GLOBAL),
        ("contextEncoder", layout::CONTEXT_ENCODER),
        ("contextSubmit", layout::CONTEXT_SUBMIT),
        ("contextRender", layout::CONTEXT_RENDER),
        ("firstFrame", layout::FIRST_FRAME),
        ("frameStride", layout::FRAME_STRIDE),
        ("frameNumber", layout::FRAME_NUMBER),
    ] {
        if number(&profile["layout"][key])? != expected {
            return Err(format!("producer frame {key} differs"));
        }
    }
    if layout::FIRST_FRAME.checked_add(
        layout::FRAME_STRIDE
            .checked_mul(2)
            .ok_or("producer frame pair overflow")?,
    ) != Some(layout::CONTEXT_RENDER)
        || layout::CONTEXT_RENDER.checked_add(8) != Some(layout::CONTEXT_SUBMIT)
        || layout::CONTEXT_ENCODER
            .checked_add(8)
            .is_none_or(|end| end > layout::FIRST_FRAME)
        || layout::FRAME_NUMBER
            .checked_add(4)
            .is_none_or(|end| end > layout::FRAME_STRIDE)
    {
        return Err("producer embedded frame pair layout differs".into());
    }
    let functions = profile["functions"]
        .as_object()
        .filter(|functions| functions.len() == 5)
        .ok_or("producer frame function inventory differs")?;
    let mut roles = HashMap::new();
    let mut starts = HashSet::new();
    let mut total = 0usize;
    for role in [
        "frameStep",
        "swap",
        "contextConstructor",
        "frameConstructor",
        "globalAccessor",
    ] {
        let function = functions
            .get(role)
            .ok_or("producer frame function absent")?;
        let rva = number(&function["rva"])?;
        let length = number(&function["bytes"])?;
        total = total
            .checked_add(length)
            .filter(|n| *n <= 32 * 1024)
            .ok_or("producer frame code budget exceeded")?;
        if !starts.insert(rva) {
            return Err("producer frame function roles repeat".into());
        }
        let expected = function["sha256"]
            .as_str()
            .filter(|hash| hash.len() == 64)
            .ok_or("producer frame function digest absent")?;
        if crate::native_hash::hex(&Sha256::digest(&read_profile_function(rva, length)?))
            != expected
        {
            return Err(format!("producer frame {role} code differs"));
        }
        roles.insert(role, (rva, length));
    }
    let edges = profile["edges"]
        .as_array()
        .filter(|edges| edges.len() == 3)
        .ok_or("producer frame call inventory differs")?;
    let expected = [
        ("frameStep", "swap"),
        ("contextConstructor", "frameConstructor"),
        ("contextConstructor", "frameConstructor"),
    ];
    let mut sites = HashSet::new();
    for (edge, (from, to)) in edges.iter().zip(expected) {
        if edge["from"].as_str() != Some(from) || edge["to"].as_str() != Some(to) {
            return Err("producer frame call roles differ".into());
        }
        let &(begin, length) = roles.get(from).ok_or("producer frame caller absent")?;
        let &(target, _) = roles.get(to).ok_or("producer frame callee absent")?;
        let end = begin
            .checked_add(length)
            .ok_or("producer frame caller extent overflow")?;
        let site = number(&edge["siteRva"])?;
        let after = site.checked_add(5).ok_or("producer frame call overflow")?;
        if !sites.insert(site) || site < begin || after > end {
            return Err("producer frame call lies outside caller".into());
        }
        let bytes = read_profile_function(site, 5)?;
        let displacement = i32::from_le_bytes(
            bytes[1..5]
                .try_into()
                .map_err(|_| "producer frame call truncated")?,
        );
        if bytes[0] != 0xe8 || after.checked_add_signed(displacement as isize) != Some(target) {
            return Err("producer frame native call differs".into());
        }
    }
    let loads = profile["globalLoads"]
        .as_array()
        .filter(|loads| loads.len() == 2)
        .ok_or("producer frame global witness inventory differs")?;
    let &(begin, length) = roles
        .get("globalAccessor")
        .ok_or("producer frame global accessor absent")?;
    let end = begin
        .checked_add(length)
        .ok_or("producer frame accessor extent overflow")?;
    for load in loads {
        let site = number(&load["siteRva"])?;
        let after = site
            .checked_add(7)
            .ok_or("producer frame global witness overflow")?;
        if load["function"].as_str() != Some("globalAccessor")
            || !sites.insert(site)
            || site < begin
            || after > end
        {
            return Err("producer frame global witness lies outside accessor".into());
        }
        let bytes = read_profile_function(site, 7)?;
        let expected = load["prefix"]
            .as_str()
            .filter(|prefix| prefix.len() == 6)
            .ok_or("producer frame global load opcode absent")?;
        let displacement = i32::from_le_bytes(
            bytes[3..7]
                .try_into()
                .map_err(|_| "producer frame global witness truncated")?,
        );
        if bytes[0] & 0xf8 != 0x48
            || bytes[1] != 0x8b
            || bytes[2] & 0xc7 != 5
            || crate::native_hash::hex(&bytes[..3]) != expected
            || after.checked_add_signed(displacement as isize) != Some(layout::CONTEXT_GLOBAL)
        {
            return Err("producer frame native Context global differs".into());
        }
    }
    Ok((base, roles["swap"].0))
}
pub(super) fn install(base: usize) -> Result<(), String> {
    if crate::world_hooks::image_base() != Some(base) {
        return Err("producer frame image unavailable".into());
    }
    let (accepted, _) = VALIDATED.get_or_init(|| resolve(base)).clone()?;
    if accepted != base {
        return Err("producer frame image changed after preflight".into());
    }
    Ok(())
}
pub(super) fn base() -> Option<usize> {
    let &(base, _) = VALIDATED.get()?.as_ref().ok()?;
    (crate::world_hooks::image_base() == Some(base)).then_some(base)
}
pub(super) fn swap_rva() -> Option<usize> {
    let &(base, swap) = VALIDATED.get()?.as_ref().ok()?;
    (crate::world_hooks::image_base() == Some(base)).then_some(swap)
}
