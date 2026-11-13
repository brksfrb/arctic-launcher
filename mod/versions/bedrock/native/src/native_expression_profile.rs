//! Exact equipment setup/evaluator entries and callsite-specific registers.
use crate::native_scene_layout::{self as scene, attachment_observer as layout};
use crate::world_hooks::{model_body_binding, read_profile_function};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    sync::OnceLock,
};

const CATALOG: &str = include_str!("../inspection/expression-observer-bindings.json");
#[derive(Clone, Copy)]
pub(super) enum ParentRegister {
    Rbx,
    R12,
}
#[derive(Clone, Copy)]
pub(super) struct Binding {
    pub(super) setup: usize,
    pub(super) evaluator: usize,
    pub(super) return_rva: usize,
    pub(super) parent: ParentRegister,
}
static BINDING: OnceLock<Result<(usize, Binding), String>> = OnceLock::new();
fn number(value: &Value) -> Result<usize, String> {
    value
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| "invalid expression observer binding number".into())
}
fn resolve(base: usize) -> Result<(usize, Binding), String> {
    let body = model_body_binding()?;
    let catalog: Value = serde_json::from_str(CATALOG).map_err(|error| error.to_string())?;
    if catalog["schema"] != 1 {
        return Err("expression observer schema differs".into());
    }
    let profiles = catalog["profiles"]
        .as_array()
        .ok_or("expression observer profiles absent")?;
    let mut selected = profiles
        .iter()
        .filter(|profile| profile["productVersion"].as_str() == Some(scene::PRODUCT_VERSION));
    let profile = selected
        .next()
        .ok_or("expression observer scene family unavailable")?;
    if selected.next().is_some()
        || profile["setupAbi"].as_str() != Some("modelItemSlotParamsActorFlags7")
        || profile["evalAbi"].as_str() != Some("expressionParamsResult2")
        || profile["parentRegister"].as_str() != Some(layout::PARENT_REGISTER)
        || profile["boneRegister"].as_str() != Some(layout::BONE_REGISTER)
        || number(&profile["setupStackBytes"])? != layout::SETUP_STACK_BYTES
        || number(&profile["evalVirtualSlotBytes"])? != layout::EVAL_VIRTUAL_SLOT_BYTES
    {
        return Err("expression observer ABI, registers or scene layout differ".into());
    }
    let parent = match (
        scene::PRODUCT_VERSION,
        layout::PARENT_REGISTER,
        layout::SETUP_STACK_BYTES,
    ) {
        ("1.26.5203.0", "rbx", 0x1b8) => ParentRegister::Rbx,
        ("1.26.60.5", "r12", 0x1c8) => ParentRegister::R12,
        _ => return Err("expression observer register strategy lacks this family".into()),
    };
    if layout::BONE_REGISTER != "rsi" {
        return Err("expression observer target bone register differs".into());
    }
    let functions = profile["functions"]
        .as_object()
        .filter(|functions| functions.len() == 3)
        .ok_or("expression observer function inventory differs")?;
    let mut roles = HashMap::new();
    let mut starts = HashSet::new();
    let mut total = 0usize;
    for role in ["setup", "wrapper", "evaluator"] {
        let function = functions
            .get(role)
            .ok_or("expression observer function absent")?;
        let rva = number(&function["rva"])?;
        let length = number(&function["bytes"])?;
        total = total
            .checked_add(length)
            .filter(|n| *n <= 32 * 1024)
            .ok_or("expression observer code budget exceeded")?;
        if !starts.insert(rva) || (role == "evaluator" && length != 17) {
            return Err("expression observer function shape differs".into());
        }
        let expected = function["sha256"]
            .as_str()
            .filter(|hash| hash.len() == 64)
            .ok_or("expression observer function digest absent")?;
        if crate::native_hash::hex(&Sha256::digest(&read_profile_function(rva, length)?))
            != expected
        {
            return Err(format!("expression observer {role} code differs"));
        }
        roles.insert(role, (rva, length));
    }
    let root = number(&profile["rootRva"])?;
    let root_bytes = number(&profile["rootBytes"])?;
    if root != body.root_rva || !(1..=65536).contains(&root_bytes) || starts.contains(&root) {
        return Err("expression observer root draw differs".into());
    }
    roles.insert("root", (root, root_bytes));
    let edges = profile["edges"]
        .as_array()
        .filter(|edges| edges.len() == 3)
        .ok_or("expression observer call inventory differs")?;
    let mut sites = HashSet::new();
    let mut return_rva = None;
    for (edge, (from, to)) in edges.iter().zip([
        ("root", "wrapper"),
        ("wrapper", "setup"),
        ("setup", "evaluator"),
    ]) {
        if edge["from"].as_str() != Some(from) || edge["to"].as_str() != Some(to) {
            return Err("expression observer call roles differ".into());
        }
        let &(begin, length) = roles.get(from).ok_or("expression observer caller absent")?;
        let &(target, _) = roles.get(to).ok_or("expression observer callee absent")?;
        let end = begin
            .checked_add(length)
            .ok_or("expression observer caller extent overflow")?;
        let site = number(&edge["siteRva"])?;
        let after = site
            .checked_add(5)
            .ok_or("expression observer call overflow")?;
        if !sites.insert(site) || site < begin || after > end {
            return Err("expression observer call lies outside caller".into());
        }
        let bytes = read_profile_function(site, 5)?;
        let displacement = i32::from_le_bytes(
            bytes[1..5]
                .try_into()
                .map_err(|_| "expression observer call truncated")?,
        );
        if bytes[0] != 0xe8 || after.checked_add_signed(displacement as isize) != Some(target) {
            return Err("expression observer direct call differs".into());
        }
        if to == "evaluator" {
            return_rva = Some(after);
        }
    }
    let return_rva = return_rva
        .filter(|rva| number(&profile["returnRva"]).ok() == Some(*rva))
        .ok_or("expression observer return site differs")?;
    Ok((
        base,
        Binding {
            setup: roles["setup"].0,
            evaluator: roles["evaluator"].0,
            return_rva,
            parent,
        },
    ))
}
pub(super) fn binding(base: usize) -> Result<Binding, String> {
    if crate::world_hooks::image_base() != Some(base) {
        return Err("expression observer image unavailable".into());
    }
    let (accepted, binding) = BINDING.get_or_init(|| resolve(base)).clone()?;
    if accepted != base {
        return Err("expression observer image changed after preflight".into());
    }
    Ok(binding)
}
pub(super) fn current() -> Option<Binding> {
    let &(base, binding) = BINDING.get()?.as_ref().ok()?;
    (crate::world_hooks::image_base() == Some(base)).then_some(binding)
}
