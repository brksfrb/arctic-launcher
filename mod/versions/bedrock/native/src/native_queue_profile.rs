//! Exact queue consumer, fence poll and context recycling metadata family.
use crate::native_scene_layout::{self as scene, native_queue as layout};
use crate::world_hooks::read_profile_function;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    sync::OnceLock,
};

const CATALOG: &str = include_str!("../inspection/queue-retirement-reader-bindings.json");
#[derive(Clone, Copy)]
pub(super) struct Binding {
    pub(super) rva: usize,
}
static BINDING: OnceLock<Result<(usize, Binding), String>> = OnceLock::new();

fn number(value: &Value) -> Result<usize, String> {
    value
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| "invalid queue reader binding number".into())
}
fn resolve(base: usize) -> Result<(usize, Binding), String> {
    let catalog: Value = serde_json::from_str(CATALOG).map_err(|error| error.to_string())?;
    if catalog["schema"] != 1 {
        return Err("queue reader schema differs".into());
    }
    let profiles = catalog["profiles"]
        .as_array()
        .ok_or("queue reader profiles absent")?;
    let mut selected = profiles
        .iter()
        .filter(|p| p["productVersion"].as_str() == Some(scene::PRODUCT_VERSION));
    let profile = selected
        .next()
        .ok_or("queue reader scene family unavailable")?;
    let stack = number(&profile["stackBytes"])?;
    if selected.next().is_some()
        || profile["abi"].as_str() != Some("queueU64FenceU32TimeoutU8Return3")
        || !matches!(
            (scene::PRODUCT_VERSION, stack),
            ("1.26.5203.0", 0x918) | ("1.26.60.5", 0x908)
        )
    {
        return Err("queue reader ABI or stack differs".into());
    }
    for (key, expected) in [
        ("capacity", layout::CAPACITY),
        ("rendererQueue", layout::RENDERER_QUEUE),
        ("queueObject", layout::QUEUE_OBJECT),
        ("used", layout::USED),
        ("usedRead", layout::USED_READ),
        ("usedWrite", layout::USED_WRITE),
        ("free", layout::FREE),
        ("freeRead", layout::FREE_READ),
        ("freeWrite", layout::FREE_WRITE),
        ("fenceContext", layout::FENCE_CONTEXT),
        ("fenceObject", layout::FENCE_OBJECT),
        ("fenceCompleted", layout::FENCE_COMPLETED),
        ("contextList", layout::CONTEXT_LIST),
        ("contextFence", layout::CONTEXT_FENCE),
        ("contextSubmitted", layout::CONTEXT_SUBMITTED),
        ("contextDeferred", layout::CONTEXT_DEFERRED),
        ("deferredBytes", layout::DEFERRED_BYTES),
    ] {
        if number(&profile["layout"][key])? != expected {
            return Err(format!("queue reader {key} differs"));
        }
    }
    let ring_bytes = layout::CAPACITY
        .checked_mul(8)
        .ok_or("queue ring extent overflow")?;
    if layout::CAPACITY == 0
        || layout::CAPACITY > 512
        || layout::DEFERRED_BYTES != 24
        || layout::USED.checked_add(ring_bytes) != Some(layout::USED_READ)
        || layout::USED_READ.checked_add(8) != Some(layout::USED_WRITE)
        || layout::USED_WRITE.checked_add(8) != Some(layout::FREE)
        || layout::FREE.checked_add(ring_bytes) != Some(layout::FREE_READ)
        || layout::FREE_READ.checked_add(8) != Some(layout::FREE_WRITE)
        || layout::FREE_WRITE.checked_add(8) != Some(layout::FENCE_CONTEXT)
        || layout::CONTEXT_FENCE.checked_add(8) != Some(layout::CONTEXT_SUBMITTED)
        || layout::CONTEXT_SUBMITTED.checked_add(8) != Some(layout::CONTEXT_DEFERRED)
    {
        return Err("queue reader storage bounds differ".into());
    }
    let functions = profile["functions"]
        .as_object()
        .filter(|f| f.len() == 5)
        .ok_or("queue reader function inventory differs")?;
    let mut roles = HashMap::new();
    let mut starts = HashSet::new();
    let mut total = 0usize;
    for role in ["consumer", "caller", "poll", "submit", "allocator"] {
        let function = functions.get(role).ok_or("queue reader function absent")?;
        let rva = number(&function["rva"])?;
        let length = number(&function["bytes"])?;
        total = total
            .checked_add(length)
            .filter(|n| *n <= 8 * 1024)
            .ok_or("queue reader code budget exceeded")?;
        if !starts.insert(rva) {
            return Err("queue reader function roles repeat".into());
        }
        let expected = function["sha256"]
            .as_str()
            .filter(|hash| hash.len() == 64)
            .ok_or("queue reader function digest absent")?;
        if crate::native_hash::hex(&Sha256::digest(&read_profile_function(rva, length)?))
            != expected
        {
            return Err(format!("queue reader {role} code differs"));
        }
        roles.insert(role, (rva, length));
    }
    let edges = profile["edges"]
        .as_array()
        .filter(|edges| edges.len() == 5)
        .ok_or("queue reader call inventory differs")?;
    let expected = [
        ("consumer", "poll"),
        ("consumer", "poll"),
        ("caller", "consumer"),
        ("caller", "allocator"),
        ("allocator", "consumer"),
    ];
    let mut sites = HashSet::new();
    for (edge, (from, to)) in edges.iter().zip(expected) {
        if edge["fromRole"].as_str() != Some(from) || edge["toRole"].as_str() != Some(to) {
            return Err("queue reader call roles differ".into());
        }
        let (begin, length) = roles[from];
        let target = roles[to].0;
        let site = number(&edge["siteRva"])?;
        let end = begin
            .checked_add(length)
            .ok_or("queue caller extent overflow")?;
        let after = site.checked_add(5).ok_or("queue call overflow")?;
        if !sites.insert(site) || site < begin || after > end {
            return Err("queue call site repeats or lies outside caller".into());
        }
        let bytes = read_profile_function(site, 5)?;
        let displacement =
            i32::from_le_bytes(bytes[1..5].try_into().map_err(|_| "queue call truncated")?);
        if bytes[0] != 0xe8 || after.checked_add_signed(displacement as isize) != Some(target) {
            return Err("queue native call differs".into());
        }
    }
    Ok((
        base,
        Binding {
            rva: roles["consumer"].0,
        },
    ))
}
pub(super) fn binding(base: usize) -> Result<Binding, String> {
    if crate::world_hooks::image_base() != Some(base) {
        return Err("queue reader image unavailable".into());
    }
    let (accepted, binding) = BINDING.get_or_init(|| resolve(base)).clone()?;
    if accepted != base {
        return Err("queue reader image changed after preflight".into());
    }
    Ok(binding)
}
pub(super) fn current() -> Option<Binding> {
    let &(base, binding) = BINDING.get()?.as_ref().ok()?;
    (crate::world_hooks::image_base() == Some(base)).then_some(binding)
}
