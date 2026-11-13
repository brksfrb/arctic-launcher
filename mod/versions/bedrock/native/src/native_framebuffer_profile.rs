//! Exact framebuffer setter and paged texture metadata reader family.
use crate::native_scene_layout::{self as scene, framebuffer as layout};
use crate::world_hooks::read_profile_function;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    sync::OnceLock,
};

const CATALOG: &str = include_str!("../inspection/framebuffer-reader-bindings.json");
#[derive(Clone, Copy)]
pub(super) struct Binding {
    pub(super) rva: usize,
}
static BINDING: OnceLock<Result<(usize, Binding), String>> = OnceLock::new();
fn number(value: &Value) -> Result<usize, String> {
    value
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| "invalid framebuffer reader binding number".into())
}
fn call(site: usize, target: usize, begin: usize, length: usize) -> Result<(), String> {
    let end = begin
        .checked_add(length)
        .ok_or("framebuffer caller extent overflow")?;
    let after = site.checked_add(5).ok_or("framebuffer call overflow")?;
    if site < begin || after > end {
        return Err("framebuffer call lies outside caller".into());
    }
    let bytes = read_profile_function(site, 5)?;
    let displacement = i32::from_le_bytes(
        bytes[1..5]
            .try_into()
            .map_err(|_| "framebuffer call truncated")?,
    );
    if bytes[0] != 0xe8 || after.checked_add_signed(displacement as isize) != Some(target) {
        return Err("framebuffer native call differs".into());
    }
    Ok(())
}
fn resolve(base: usize) -> Result<(usize, Binding), String> {
    let backend = crate::render_bridge::backend_binding(base)?;
    let catalog: Value = serde_json::from_str(CATALOG).map_err(|error| error.to_string())?;
    if catalog["schema"] != 1 {
        return Err("framebuffer reader schema differs".into());
    }
    let profiles = catalog["profiles"]
        .as_array()
        .ok_or("framebuffer reader profiles absent")?;
    let mut selected = profiles
        .iter()
        .filter(|profile| profile["productVersion"].as_str() == Some(scene::PRODUCT_VERSION));
    let profile = selected
        .next()
        .ok_or("framebuffer reader scene family unavailable")?;
    if selected.next().is_some()
        || profile["abi"].as_str() != Some("contextRendererDwordHandleDwordFlag4")
        || number(&profile["backendRva"])? != backend.submit_rva
    {
        return Err("framebuffer reader ABI or backend differs".into());
    }
    for (key, expected) in [
        ("headerBytes", layout::HEADER_BYTES),
        ("contextColors", layout::CONTEXT_COLORS),
        ("contextDepth", layout::CONTEXT_DEPTH),
        ("contextHandle", layout::CONTEXT_HANDLE),
        ("contextFlag", layout::CONTEXT_FLAG),
        ("contextList", layout::CONTEXT_LIST),
        ("framebuffers", layout::FRAMEBUFFERS),
        ("framebufferStride", layout::FRAMEBUFFER_STRIDE),
        ("framebufferCount", layout::FRAMEBUFFER_COUNT),
        ("framebufferDepth", layout::FRAMEBUFFER_DEPTH),
        ("textures", layout::TEXTURES),
        ("stateCache", layout::STATE_CACHE),
        ("textureStride", layout::TEXTURE_STRIDE),
        ("pageEntries", layout::PAGE_ENTRIES),
        ("retirementFlag", layout::RETIREMENT_FLAG),
        ("retirementMask", layout::RETIREMENT_MASK as usize),
    ] {
        if number(&profile["layout"][key])? != expected {
            return Err(format!("framebuffer reader {key} differs"));
        }
    }
    if layout::HEADER_BYTES != 32
        || layout::CONTEXT_COLORS
            .checked_add(8)
            .is_none_or(|n| n > layout::HEADER_BYTES)
        || layout::CONTEXT_DEPTH
            .checked_add(8)
            .is_none_or(|n| n > layout::HEADER_BYTES)
        || layout::FRAMEBUFFER_COUNT >= layout::FRAMEBUFFER_STRIDE
        || layout::FRAMEBUFFER_DEPTH
            .checked_add(2)
            .is_none_or(|n| n > layout::FRAMEBUFFER_STRIDE)
        || layout::RETIREMENT_FLAG >= layout::TEXTURE_STRIDE
        || layout::PAGE_ENTRIES != 4096
    {
        return Err("framebuffer reader bounds differ".into());
    }
    let functions = profile["functions"]
        .as_object()
        .filter(|f| f.len() == 5)
        .ok_or("framebuffer reader function inventory differs")?;
    let mut roles = HashMap::new();
    let mut starts = HashSet::new();
    let mut total = 0usize;
    for role in [
        "setter",
        "transition",
        "textureEnsure",
        "cleanup",
        "override",
    ] {
        let function = functions
            .get(role)
            .ok_or("framebuffer reader function absent")?;
        let rva = number(&function["rva"])?;
        let length = number(&function["bytes"])?;
        total = total
            .checked_add(length)
            .filter(|n| *n <= 16 * 1024)
            .ok_or("framebuffer reader code budget exceeded")?;
        if !starts.insert(rva) || rva == backend.submit_rva {
            return Err("framebuffer reader function roles repeat".into());
        }
        let expected = function["sha256"]
            .as_str()
            .filter(|hash| hash.len() == 64)
            .ok_or("framebuffer reader function digest absent")?;
        if crate::native_hash::hex(&Sha256::digest(&read_profile_function(rva, length)?))
            != expected
        {
            return Err(format!("framebuffer reader {role} code differs"));
        }
        roles.insert(role, (rva, length));
    }
    let rva = roles["setter"].0;
    let backend_call = number(&profile["callRva"])?;
    call(backend_call, rva, backend.submit_rva, backend.submit_bytes)?;
    let edges = profile["edges"]
        .as_array()
        .filter(|edges| edges.len() == 10)
        .ok_or("framebuffer reader call inventory differs")?;
    let expected = [
        ("setter", "transition"),
        ("setter", "textureEnsure"),
        ("setter", "textureEnsure"),
        ("setter", "transition"),
        ("setter", "textureEnsure"),
        ("setter", "textureEnsure"),
        ("setter", "transition"),
        ("setter", "transition"),
        ("override", "textureEnsure"),
        ("override", "cleanup"),
    ];
    let mut sites = HashSet::from([backend_call]);
    for (edge, (from, to)) in edges.iter().zip(expected) {
        if edge["from"].as_str() != Some(from) || edge["to"].as_str() != Some(to) {
            return Err("framebuffer reader call roles differ".into());
        }
        let &(begin, length) = roles.get(from).ok_or("framebuffer reader caller absent")?;
        let &(target, _) = roles.get(to).ok_or("framebuffer reader callee absent")?;
        let site = number(&edge["siteRva"])?;
        if !sites.insert(site) {
            return Err("framebuffer reader call sites repeat".into());
        }
        call(site, target, begin, length)?;
    }
    Ok((base, Binding { rva }))
}
pub(super) fn binding(base: usize) -> Result<Binding, String> {
    if crate::world_hooks::image_base() != Some(base) {
        return Err("framebuffer reader image unavailable".into());
    }
    let (accepted, binding) = BINDING.get_or_init(|| resolve(base)).clone()?;
    if accepted != base {
        return Err("framebuffer reader image changed after preflight".into());
    }
    Ok(binding)
}
pub(super) fn current() -> Option<Binding> {
    let &(base, binding) = BINDING.get()?.as_ref().ok()?;
    (crate::world_hooks::image_base() == Some(base)).then_some(binding)
}
