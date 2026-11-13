//! Exact seven-pointer model draw, joined to controller and root draw calls.
//! Selecting this entry does not migrate the remaining skin/equipment readers.
use super::{exact_function_candidate, function_witness, read_profile_function};
use crate::native_scene_layout::model_body as layout;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::OnceLock;

#[derive(Clone, Copy)]
pub(crate) struct ModelBodyBinding {
    pub(crate) rva: usize,
    pub(crate) caller_rva: usize,
    pub(crate) root_rva: usize,
    pub(crate) body_return_rva: usize,
}
impl ModelBodyBinding {
    pub(crate) fn prefix(self) -> &'static [u8] {
        &[
            0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x55, 0x53, 0x48, 0x83,
            0xec, 0x58, 0x4c, 0x89, 0x4c, 0x24, 0x40, 0x4c, 0x89, 0x44, 0x24, 0x50,
        ]
    }
}
static BINDING: OnceLock<Result<(usize, ModelBodyBinding), String>> = OnceLock::new();
fn number(value: &Value) -> Result<usize, String> {
    value
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| "invalid model body binding number".into())
}
fn digest(rva: usize, length: usize, expected: &Value) -> Result<(), String> {
    let expected = expected
        .as_str()
        .filter(|hash| hash.len() == 64)
        .ok_or("model body function digest absent")?;
    let bytes = read_profile_function(rva, length)?;
    if crate::native_hash::hex(&Sha256::digest(&bytes)) != expected {
        return Err("model body function bytes differ".into());
    }
    Ok(())
}
fn resolve(base: usize) -> Result<(usize, ModelBodyBinding), String> {
    let (rva, entry, _) = exact_function_candidate("modelBodyDraw", 26)?;
    if entry["abi"].as_str() != Some("modelParamsMetadataControllerGeometryTexturesMers7")
        || entry["layout"].as_str() != Some(layout::LAYOUT)
        || number(&entry["stackBytes"])? != layout::STACK_BYTES
    {
        return Err("model body ABI or scene layout differs".into());
    }
    let caller_rva = number(&entry["callerRva"])?;
    let caller_bytes = number(&entry["callerBytes"])?;
    let caller_end = caller_rva
        .checked_add(caller_bytes)
        .ok_or("model body caller extent overflow")?;
    let witness = number(&entry["witnessRva"])?;
    if witness < caller_rva
        || witness
            .checked_add(5)
            .is_none_or(|after| after > caller_end)
    {
        return Err("model body call lies outside controller".into());
    }
    let root = super::transform_binding()?;
    let root_rva = number(&entry["rootDrawRva"])?;
    let root_end = number(&entry["rootDrawEndRva"])?;
    let root_call = number(&entry["rootCallRva"])?;
    let &(_, start, end) = super::CODE_IMAGE
        .get()
        .ok_or("model body image unavailable")?;
    if root_rva != root.draw_rva
        || root_end > end
        || root_rva < start
        || root_rva >= root_end
        || root_call < root_rva
        || root_call
            .checked_add(5)
            .is_none_or(|after| after > root_end)
        || rva == caller_rva
        || rva == root_rva
        || caller_rva == root_rva
    {
        return Err("model body controller/root relationship differs".into());
    }
    digest(rva, number(&entry["bytes"])?, &entry["sha256"])?;
    digest(caller_rva, caller_bytes, &entry["callerSha256"])?;
    function_witness(&entry, rva)?;
    let root_edge = serde_json::json!({"witnessRva":root_call,"witnessKind":"call"});
    function_witness(&root_edge, caller_rva)?;
    let body_return_rva = witness.checked_add(5).ok_or("model body return overflow")?;
    Ok((
        base,
        ModelBodyBinding {
            rva,
            caller_rva,
            root_rva,
            body_return_rva,
        },
    ))
}
pub(crate) fn model_body_binding() -> Result<ModelBodyBinding, String> {
    let base = super::image_base().ok_or("model body image unavailable")?;
    let (accepted, binding) = BINDING.get_or_init(|| resolve(base)).clone()?;
    if accepted != base {
        return Err("model body image changed after preflight".into());
    }
    Ok(binding)
}
