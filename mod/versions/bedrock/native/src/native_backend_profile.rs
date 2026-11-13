//! Six-pointer backend entry and per-family reset ABI/batch packet readers.
use crate::native_scene_layout::{
    self as scene, backend_submit as backend, native_batch as layout,
};
use crate::world_hooks::read_profile_function;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    sync::OnceLock,
};

const CATALOG: &str = include_str!("../inspection/backend-batch-bindings.json");
#[derive(Clone, Copy)]
pub(crate) enum BatchResetAbi {
    FrameStereo3,
    Frame2,
}
#[derive(Clone, Copy)]
pub(crate) struct BackendBinding {
    pub(crate) submit_rva: usize,
    pub(crate) reset_rva: usize,
    pub(crate) fill_rva: usize,
    pub(crate) submit_bytes: usize,
    pub(crate) reset_return_rva: usize,
    pub(crate) reset_abi: BatchResetAbi,
}
static BINDING: OnceLock<Result<(usize, BackendBinding), String>> = OnceLock::new();
fn number(value: &Value) -> Result<usize, String> {
    value
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| "invalid backend batch binding number".into())
}
fn resolve(base: usize) -> Result<(usize, BackendBinding), String> {
    let catalog: Value = serde_json::from_str(CATALOG).map_err(|error| error.to_string())?;
    if catalog["schema"] != 1 {
        return Err("backend batch schema differs".into());
    }
    let profiles = catalog["profiles"]
        .as_array()
        .ok_or("backend batch profiles absent")?;
    let mut selected = profiles
        .iter()
        .filter(|profile| profile["productVersion"].as_str() == Some(scene::PRODUCT_VERSION));
    let profile = selected
        .next()
        .ok_or("backend batch scene family unavailable")?;
    if selected.next().is_some()
        || profile["submitAbi"].as_str() != Some("sixPointerVoid")
        || profile["fillAbi"].as_str() != Some("fourPointerU32")
        || profile["resetAbi"].as_str() != Some(backend::RESET_ABI)
        || number(&profile["stackBytes"])? != backend::STACK_BYTES
    {
        return Err("backend batch ABI or stack layout differs".into());
    }
    let reset_abi = match (
        scene::PRODUCT_VERSION,
        backend::RESET_ABI,
        backend::STACK_BYTES,
    ) {
        ("1.26.5203.0", "frameStereo3", 0x1a238) => BatchResetAbi::FrameStereo3,
        ("1.26.60.5", "frame2", 0xe238) => BatchResetAbi::Frame2,
        _ => return Err("backend batch reset ABI lacks this family".into()),
    };
    for (key, expected) in [
        ("resetInStack", layout::RESET_IN_STACK),
        ("viewInStack", layout::VIEW_IN_STACK),
        ("drawIndexHandle", layout::DRAW_INDEX_HANDLE),
        ("commandList", layout::COMMAND_LIST),
        ("nonIndexedCount", layout::NON_INDEXED_COUNT),
        ("nonIndexedArray", layout::NON_INDEXED_ARRAY),
        ("nonIndexedStride", layout::NON_INDEXED_STRIDE),
        ("nonIndexedCbv", layout::NON_INDEXED_CBV),
        ("indexedCount", layout::INDEXED_COUNT),
        ("indexedArray", layout::INDEXED_ARRAY),
        ("indexedStride", layout::INDEXED_STRIDE),
        ("indexedCbv", layout::INDEXED_CBV),
    ] {
        if number(&profile["layout"][key])? != expected {
            return Err(format!("backend batch {key} differs"));
        }
    }
    if layout::RESET_IN_STACK >= backend::STACK_BYTES
        || layout::VIEW_IN_STACK
            .checked_add(8)
            .is_none_or(|n| n > backend::STACK_BYTES)
        || layout::NON_INDEXED_CBV
            .checked_add(16)
            .is_none_or(|n| n > layout::NON_INDEXED_STRIDE)
        || layout::INDEXED_CBV
            .checked_add(16)
            .is_none_or(|n| n > layout::INDEXED_STRIDE)
    {
        return Err("backend batch reader bounds differ".into());
    }
    let functions = profile["functions"]
        .as_object()
        .filter(|f| f.len() == 4)
        .ok_or("backend batch function inventory differs")?;
    let mut roles = HashMap::new();
    let mut starts = HashSet::new();
    let mut total = 0usize;
    for role in ["submit", "caller", "reset", "fill"] {
        let function = functions.get(role).ok_or("backend batch function absent")?;
        let rva = number(&function["rva"])?;
        let length = number(&function["bytes"])?;
        total = total
            .checked_add(length)
            .filter(|n| *n <= 64 * 1024)
            .ok_or("backend batch code budget exceeded")?;
        if !starts.insert(rva) {
            return Err("backend batch function roles repeat".into());
        }
        let expected = function["sha256"]
            .as_str()
            .filter(|hash| hash.len() == 64)
            .ok_or("backend batch function digest absent")?;
        if crate::native_hash::hex(&Sha256::digest(&read_profile_function(rva, length)?))
            != expected
        {
            return Err(format!("backend batch {role} code differs"));
        }
        roles.insert(role, (rva, length));
    }
    let edges = profile["edges"]
        .as_array()
        .filter(|edges| edges.len() == 5)
        .ok_or("backend batch call inventory differs")?;
    let expected = [
        ("caller", "submit"),
        ("submit", "reset"),
        ("submit", "fill"),
        ("submit", "fill"),
        ("submit", "fill"),
    ];
    let mut sites = HashSet::new();
    let mut reset_return = None;
    for (edge, (from, to)) in edges.iter().zip(expected) {
        if edge["from"].as_str() != Some(from) || edge["to"].as_str() != Some(to) {
            return Err("backend batch call roles differ".into());
        }
        let &(begin, length) = roles.get(from).ok_or("backend batch caller absent")?;
        let &(target, _) = roles.get(to).ok_or("backend batch callee absent")?;
        let end = begin
            .checked_add(length)
            .ok_or("backend batch caller extent overflow")?;
        let site = number(&edge["siteRva"])?;
        let after = site.checked_add(5).ok_or("backend batch call overflow")?;
        if !sites.insert(site) || site < begin || after > end {
            return Err("backend batch call lies outside caller".into());
        }
        let bytes = read_profile_function(site, 5)?;
        let displacement = i32::from_le_bytes(
            bytes[1..5]
                .try_into()
                .map_err(|_| "backend batch call truncated")?,
        );
        if bytes[0] != 0xe8 || after.checked_add_signed(displacement as isize) != Some(target) {
            return Err("backend batch native call differs".into());
        }
        if to == "reset" {
            reset_return = Some(after);
        }
    }
    Ok((
        base,
        BackendBinding {
            submit_rva: roles["submit"].0,
            submit_bytes: roles["submit"].1,
            reset_rva: roles["reset"].0,
            fill_rva: roles["fill"].0,
            reset_return_rva: reset_return.ok_or("backend batch reset return absent")?,
            reset_abi,
        },
    ))
}
pub(super) fn binding(base: usize) -> Result<BackendBinding, String> {
    if crate::world_hooks::image_base() != Some(base) {
        return Err("backend batch image unavailable".into());
    }
    let (accepted, binding) = BINDING.get_or_init(|| resolve(base)).clone()?;
    if accepted != base {
        return Err("backend batch image changed after preflight".into());
    }
    Ok(binding)
}
pub(super) fn current() -> Option<BackendBinding> {
    let &(base, binding) = BINDING.get()?.as_ref().ok()?;
    (crate::world_hooks::image_base() == Some(base)).then_some(binding)
}
