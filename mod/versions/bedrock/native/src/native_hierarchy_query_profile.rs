//! Exact metadata getter admission. The native method itself is never invoked.
use crate::native_scene_layout::{self as scene, hierarchy_query as layout};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::OnceLock;

const CATALOG: &str = include_str!("../inspection/hierarchy-query-reader-bindings.json");
#[derive(Clone, Copy)]
pub(super) struct Binding {
    pub(super) method: usize,
}
static VALIDATED: OnceLock<Result<(usize, Binding), String>> = OnceLock::new();
fn number(value: &Value) -> Result<usize, String> {
    value
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| "invalid hierarchy query binding number".into())
}
fn validate(base: usize) -> Result<(usize, Binding), String> {
    let catalog: Value = serde_json::from_str(CATALOG).map_err(|error| error.to_string())?;
    if catalog["schema"] != 1 {
        return Err("hierarchy query schema differs".into());
    }
    let profiles = catalog["profiles"]
        .as_array()
        .ok_or("hierarchy query profiles absent")?;
    let mut selected = profiles
        .iter()
        .filter(|entry| entry["productVersion"].as_str() == Some(scene::PRODUCT_VERSION));
    let profile = selected
        .next()
        .ok_or("hierarchy query scene family unavailable")?;
    if selected.next().is_some()
        || profile["abi"].as_str() != Some("componentAndOutputSpan2")
        || number(&profile["slotBytes"])? != layout::SLOT_BYTES
        || number(&profile["vectorOffset"])? != layout::VECTOR_OFFSET
        || number(&profile["elementBytes"])? != layout::ELEMENT_BYTES
    {
        return Err("hierarchy query ABI or reader layout differs".into());
    }
    let rva = number(&profile["rva"])?;
    let length = number(&profile["bytes"])?;
    if length != 74 {
        return Err("hierarchy query getter extent differs".into());
    }
    let expected = profile["sha256"]
        .as_str()
        .filter(|hash| hash.len() == 64)
        .ok_or("hierarchy query digest absent")?;
    if crate::native_hash::hex(&Sha256::digest(&crate::world_hooks::read_profile_function(
        rva, length,
    )?)) != expected
    {
        return Err("hierarchy query getter code differs".into());
    }
    Ok((
        base,
        Binding {
            method: base
                .checked_add(rva)
                .ok_or("hierarchy query method overflow")?,
        },
    ))
}
pub(super) fn install(base: usize) -> Result<(), String> {
    if crate::world_hooks::image_base() != Some(base) {
        return Err("hierarchy query image unavailable".into());
    }
    let (accepted, _) = VALIDATED.get_or_init(|| validate(base)).clone()?;
    if accepted != base {
        return Err("hierarchy query image changed after preflight".into());
    }
    Ok(())
}
pub(super) fn binding() -> Option<Binding> {
    let &(base, binding) = VALIDATED.get()?.as_ref().ok()?;
    (crate::world_hooks::image_base() == Some(base)).then_some(binding)
}
