//! Native View records and per-family observation without guessing a setter ABI.
use crate::native_scene_layout::{self as scene, frame_view as layout};
use crate::world_hooks::read_profile_function;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    sync::OnceLock,
};

const CATALOG: &str = include_str!("../inspection/view-reader-bindings.json");
#[derive(Clone, Copy)]
pub(super) struct Binding {
    pub(super) setter_rva: Option<usize>,
    pub(super) observation: &'static str,
}
static BINDING: OnceLock<Result<(usize, Binding), String>> = OnceLock::new();
fn number(value: &Value) -> Result<usize, String> {
    value
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| "invalid native View binding number".into())
}
fn resolve(base: usize) -> Result<(usize, Binding), String> {
    let catalog: Value = serde_json::from_str(CATALOG).map_err(|error| error.to_string())?;
    if catalog["schema"] != 1 {
        return Err("native View schema differs".into());
    }
    let profiles = catalog["profiles"]
        .as_array()
        .ok_or("native View profiles absent")?;
    let mut selected = profiles
        .iter()
        .filter(|profile| profile["productVersion"].as_str() == Some(scene::PRODUCT_VERSION));
    let profile = selected
        .next()
        .ok_or("native View scene family unavailable")?;
    if selected.next().is_some() || profile["observation"].as_str() != Some(layout::OBSERVATION) {
        return Err("native View observation differs".into());
    }
    let legacy = match (scene::PRODUCT_VERSION, layout::OBSERVATION) {
        ("1.26.5203.0", "setter8") => true,
        ("1.26.60.5", "frameSnapshots") => false,
        _ => return Err("native View observation lacks this family".into()),
    };
    for (key, expected) in [
        ("frameBase", layout::FRAME_BASE),
        ("count", layout::COUNT),
        ("stride", layout::STRIDE),
        ("rectangle", layout::RECTANGLE),
        ("view", layout::VIEW),
        ("projection", layout::PROJECTION),
        ("matrixBytes", layout::MATRIX_BYTES),
        ("contextViews", layout::CONTEXT_VIEWS),
        ("frameWidth", layout::FRAME_WIDTH),
        ("frameHeight", layout::FRAME_HEIGHT),
    ] {
        if number(&profile["layout"][key])? != expected {
            return Err(format!("native View {key} differs"));
        }
    }
    if layout::COUNT != 256
        || layout::MATRIX_BYTES != 64
        || layout::RECTANGLE
            .checked_add(8)
            .is_none_or(|n| n > layout::VIEW)
        || layout::VIEW
            .checked_add(64)
            .is_none_or(|n| n > layout::PROJECTION)
        || layout::PROJECTION
            .checked_add(64)
            .is_none_or(|n| n > layout::STRIDE)
        || layout::COUNT
            .checked_mul(layout::STRIDE)
            .is_none_or(|n| n > 128 * 1024)
    {
        return Err("native View reader bounds differ".into());
    }
    let functions = profile["functions"]
        .as_object()
        .filter(|f| f.len() == (if legacy { 3 } else { 2 }))
        .ok_or("native View function inventory differs")?;
    let mut roles = HashMap::new();
    let mut starts = HashSet::new();
    let mut total = 0usize;
    for role in ["matrixWriter", "frameCopy", "setter"] {
        if role == "setter" && !legacy {
            continue;
        }
        let function = functions.get(role).ok_or("native View function absent")?;
        let rva = number(&function["rva"])?;
        let length = number(&function["bytes"])?;
        total = total
            .checked_add(length)
            .filter(|n| *n <= 16 * 1024)
            .ok_or("native View code budget exceeded")?;
        if !starts.insert(rva) {
            return Err("native View function roles repeat".into());
        }
        let expected = function["sha256"]
            .as_str()
            .filter(|hash| hash.len() == 64)
            .ok_or("native View function digest absent")?;
        if crate::native_hash::hex(&Sha256::digest(&read_profile_function(rva, length)?))
            != expected
        {
            return Err(format!("native View {role} code differs"));
        }
        roles.insert(role, (rva, length));
    }
    if Some(roles["frameCopy"].0) != super::producer_profile::swap_rva() {
        return Err("native View copy is not admitted producer swap".into());
    }
    let copy = number(&profile["copyRva"])?;
    let _ = read_profile_function(copy, 1)?;
    let edges = profile["edges"]
        .as_array()
        .filter(|edges| edges.len() == (if legacy { 2 } else { 6 }))
        .ok_or("native View call inventory differs")?;
    let mut sites = HashSet::new();
    for (index, edge) in edges.iter().enumerate() {
        let (from, to) = if legacy {
            if index == 0 {
                ("matrixWriter", "setter")
            } else {
                ("frameCopy", "copy")
            }
        } else if index < 5 {
            ("matrixWriter", "copy")
        } else {
            ("frameCopy", "copy")
        };
        if edge["from"].as_str() != Some(from) || edge["to"].as_str() != Some(to) {
            return Err("native View call roles differ".into());
        }
        let &(begin, length) = roles.get(from).ok_or("native View caller absent")?;
        let target = if to == "copy" {
            copy
        } else {
            roles.get(to).ok_or("native View callee absent")?.0
        };
        let end = begin
            .checked_add(length)
            .ok_or("native View caller extent overflow")?;
        let site = number(&edge["siteRva"])?;
        let after = site.checked_add(5).ok_or("native View call overflow")?;
        if !sites.insert(site) || site < begin || after > end {
            return Err("native View call lies outside caller".into());
        }
        let bytes = read_profile_function(site, 5)?;
        let displacement = i32::from_le_bytes(
            bytes[1..5]
                .try_into()
                .map_err(|_| "native View call truncated")?,
        );
        if bytes[0] != 0xe8 || after.checked_add_signed(displacement as isize) != Some(target) {
            return Err("native View copy/call target differs".into());
        }
    }
    Ok((
        base,
        Binding {
            setter_rva: roles.get("setter").map(|entry| entry.0),
            observation: layout::OBSERVATION,
        },
    ))
}
pub(super) fn binding(base: usize) -> Result<Binding, String> {
    if crate::world_hooks::image_base() != Some(base) {
        return Err("native View image unavailable".into());
    }
    let (accepted, binding) = BINDING.get_or_init(|| resolve(base)).clone()?;
    if accepted != base {
        return Err("native View image changed after preflight".into());
    }
    Ok(binding)
}
pub(super) fn current() -> Option<Binding> {
    let &(base, binding) = BINDING.get()?.as_ref().ok()?;
    (crate::world_hooks::image_base() == Some(base)).then_some(binding)
}
