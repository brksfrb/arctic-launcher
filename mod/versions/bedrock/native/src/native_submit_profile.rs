//! Exact six-scalar encoder entry and its frame item/counter reader family.
use crate::native_scene_layout::{self as scene, encoder_submit as layout};
use crate::world_hooks::read_profile_function;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::OnceLock;

const CATALOG: &str = include_str!("../inspection/encoder-submit-bindings.json");
#[derive(Clone, Copy)]
pub(super) struct Binding {
    pub(super) rva: usize,
}
static BINDING: OnceLock<Result<(usize, Binding), String>> = OnceLock::new();
fn number(value: &Value) -> Result<usize, String> {
    value
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| "invalid encoder submission binding number".into())
}
fn resolve(base: usize) -> Result<(usize, Binding), String> {
    let catalog: Value = serde_json::from_str(CATALOG).map_err(|error| error.to_string())?;
    if catalog["schema"] != 1 {
        return Err("encoder submission schema differs".into());
    }
    let profiles = catalog["profiles"]
        .as_array()
        .ok_or("encoder submission profiles absent")?;
    let mut selected = profiles
        .iter()
        .filter(|profile| profile["productVersion"].as_str() == Some(scene::PRODUCT_VERSION));
    let profile = selected
        .next()
        .ok_or("encoder submission scene family unavailable")?;
    if selected.next().is_some()
        || profile["abi"].as_str() != Some("encoderViewProgramOcclusionDepthFlags6")
        || number(&profile["stackBytes"])? != layout::STACK_BYTES
    {
        return Err("encoder submission ABI differs".into());
    }
    for (key, expected) in [
        ("encoderFrame", layout::ENCODER_FRAME),
        ("itemsBase", layout::ITEMS_BASE),
        ("itemsCount", layout::ITEMS_COUNT),
        ("encoderSubmitted", layout::ENCODER_SUBMITTED),
        ("itemStride", layout::ITEM_STRIDE),
        ("itemIndices", layout::ITEM_INDICES),
        ("itemVertices", layout::ITEM_VERTICES),
        ("itemCapacity", layout::ITEM_CAPACITY),
    ] {
        if number(&profile["layout"][key])? != expected {
            return Err(format!("encoder submission {key} differs"));
        }
    }
    let functions = profile["functions"]
        .as_object()
        .filter(|functions| functions.len() == 2)
        .ok_or("encoder submission function inventory differs")?;
    let mut entries = Vec::with_capacity(2);
    let mut total = 0usize;
    for role in ["encoder", "caller"] {
        let function = functions
            .get(role)
            .ok_or("encoder submission function absent")?;
        let rva = number(&function["rva"])?;
        let length = number(&function["bytes"])?;
        total = total
            .checked_add(length)
            .filter(|n| *n <= 16 * 1024)
            .ok_or("encoder submission code budget exceeded")?;
        let expected = function["sha256"]
            .as_str()
            .filter(|hash| hash.len() == 64)
            .ok_or("encoder submission function digest absent")?;
        if crate::native_hash::hex(&Sha256::digest(&read_profile_function(rva, length)?))
            != expected
        {
            return Err(format!("encoder submission {role} code differs"));
        }
        entries.push((rva, length));
    }
    let (encoder, _) = entries[0];
    let (caller, length) = entries[1];
    let end = caller
        .checked_add(length)
        .ok_or("encoder submission caller extent overflow")?;
    let site = number(&profile["callRva"])?;
    let after = site
        .checked_add(5)
        .ok_or("encoder submission call overflow")?;
    if encoder == caller || site < caller || after > end {
        return Err("encoder submission call lies outside caller".into());
    }
    let bytes = read_profile_function(site, 5)?;
    let displacement = i32::from_le_bytes(
        bytes[1..5]
            .try_into()
            .map_err(|_| "encoder submission call truncated")?,
    );
    if bytes[0] != 0xe8 || after.checked_add_signed(displacement as isize) != Some(encoder) {
        return Err("encoder submission native call differs".into());
    }
    Ok((base, Binding { rva: encoder }))
}
pub(super) fn binding(base: usize) -> Result<Binding, String> {
    if crate::world_hooks::image_base() != Some(base) {
        return Err("encoder submission image unavailable".into());
    }
    let (accepted, binding) = BINDING.get_or_init(|| resolve(base)).clone()?;
    if accepted != base {
        return Err("encoder submission image changed after preflight".into());
    }
    Ok(binding)
}
pub(super) fn current() -> Option<Binding> {
    let &(base, binding) = BINDING.get()?.as_ref().ok()?;
    (crate::world_hooks::image_base() == Some(base)).then_some(binding)
}
