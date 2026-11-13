//! Equipment metadata reader admission and actual body/draw return identities.
use crate::native_scene_layout::{self as scene, attachment as layout};
use crate::world_hooks::{model_body_binding, read_profile_function};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    sync::OnceLock,
};

const CATALOG: &str = include_str!("../inspection/attachment-reader-bindings.json");
#[derive(Clone, Copy)]
pub(super) struct Binding {
    pub(super) body_return: usize,
    pub(super) draw_return: usize,
}
static VALIDATED: OnceLock<Result<(usize, Binding), String>> = OnceLock::new();
fn number(value: &Value) -> Result<usize, String> {
    value
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| "invalid attachment reader binding number".into())
}
fn resolve(base: usize) -> Result<(usize, Binding), String> {
    let body = model_body_binding()?;
    let setup = crate::native_expression_binding::preflight(base)?;
    let catalog: Value = serde_json::from_str(CATALOG).map_err(|error| error.to_string())?;
    if catalog["schema"] != 1 {
        return Err("attachment reader schema differs".into());
    }
    let profiles = catalog["profiles"]
        .as_array()
        .ok_or("attachment reader profiles absent")?;
    let mut selected = profiles
        .iter()
        .filter(|profile| profile["productVersion"].as_str() == Some(scene::PRODUCT_VERSION));
    let profile = selected
        .next()
        .ok_or("attachment reader scene family unavailable")?;
    if selected.next().is_some()
        || number(&profile["rootRva"])? != body.root_rva
        || number(&profile["controllerRva"])? != body.caller_rva
        || number(&profile["setupRva"])? != setup
    {
        return Err("attachment reader native draw family differs".into());
    }
    for (key, expected) in [
        ("children", layout::CHILDREN),
        ("childStride", layout::CHILD_STRIDE),
        ("childShared", layout::CHILD_SHARED),
        ("slots", layout::SLOTS),
        ("slotInParams", layout::SLOT_IN_PARAMS),
        ("componentReload", layout::COMPONENT_RELOAD),
        ("hierarchyMap", layout::HIERARCHY_MAP),
        ("boneBytes", layout::BONE_BYTES),
        ("boneHash", layout::BONE_HASH),
        ("boneMatrix", layout::BONE_MATRIX),
        ("boneExpression", layout::BONE_EXPRESSION),
        ("boneMethod", layout::BONE_METHOD),
        ("boneOverride", layout::BONE_OVERRIDE),
        ("indexedMethod", layout::INDEXED_METHOD as usize),
        ("expressionMethod", layout::EXPRESSION_METHOD as usize),
    ] {
        if number(&profile["layout"][key])? != expected {
            return Err(format!("attachment reader {key} differs"));
        }
    }
    let functions = profile["functions"]
        .as_object()
        .filter(|functions| functions.len() == 3)
        .ok_or("attachment reader function inventory differs")?;
    let mut roles = HashMap::new();
    let mut starts = HashSet::new();
    let mut total = 0usize;
    for role in ["wrapper", "draw", "childGetter"] {
        let function = functions
            .get(role)
            .ok_or("attachment reader function absent")?;
        let rva = number(&function["rva"])?;
        let length = number(&function["bytes"])?;
        total = total
            .checked_add(length)
            .filter(|n| *n <= 32 * 1024)
            .ok_or("attachment reader code budget exceeded")?;
        if !starts.insert(rva) || rva == body.root_rva || rva == body.caller_rva || rva == setup {
            return Err("attachment reader native functions overlap".into());
        }
        let expected = function["sha256"]
            .as_str()
            .filter(|hash| hash.len() == 64)
            .ok_or("attachment reader function digest absent")?;
        if crate::native_hash::hex(&Sha256::digest(&read_profile_function(rva, length)?))
            != expected
        {
            return Err(format!("attachment reader {role} code differs"));
        }
        roles.insert(role, (rva, length));
    }
    let root_bytes = number(&profile["rootBytes"])?;
    if !(1..=65536).contains(&root_bytes) {
        return Err("attachment reader root extent differs".into());
    }
    roles.insert("root", (body.root_rva, root_bytes));
    let edges = profile["edges"]
        .as_array()
        .filter(|edges| edges.len() == 4)
        .ok_or("attachment reader call inventory differs")?;
    let expected = [
        ("root", "wrapper", "call"),
        ("wrapper", "draw", "jump"),
        ("draw", "childGetter", "call"),
        ("draw", "controller", "call"),
    ];
    let mut sites = HashSet::new();
    let mut draw_return = None;
    for (edge, (from, to, kind)) in edges.iter().zip(expected) {
        if edge["from"].as_str() != Some(from)
            || edge["to"].as_str() != Some(to)
            || edge["kind"].as_str() != Some(kind)
        {
            return Err("attachment reader call roles differ".into());
        }
        let &(begin, length) = roles.get(from).ok_or("attachment reader caller absent")?;
        let end = begin
            .checked_add(length)
            .ok_or("attachment reader caller extent overflow")?;
        let target = if to == "controller" {
            body.caller_rva
        } else {
            roles.get(to).ok_or("attachment reader callee absent")?.0
        };
        let site = number(&edge["siteRva"])?;
        let after = site
            .checked_add(5)
            .ok_or("attachment reader call overflow")?;
        if !sites.insert(site) || site < begin || after > end {
            return Err("attachment reader call lies outside caller".into());
        }
        let bytes = read_profile_function(site, 5)?;
        let displacement = i32::from_le_bytes(
            bytes[1..5]
                .try_into()
                .map_err(|_| "attachment reader call truncated")?,
        );
        let opcode = if kind == "call" { 0xe8 } else { 0xe9 };
        if bytes[0] != opcode || after.checked_add_signed(displacement as isize) != Some(target) {
            return Err("attachment reader call target differs".into());
        }
        if to == "controller" {
            draw_return = Some(after);
        }
    }
    Ok((
        base,
        Binding {
            body_return: body.body_return_rva,
            draw_return: draw_return.ok_or("attachment reader draw return absent")?,
        },
    ))
}
pub(super) fn install(base: usize) -> Result<(), String> {
    if crate::world_hooks::image_base() != Some(base) {
        return Err("attachment reader image unavailable".into());
    }
    let (accepted, _) = VALIDATED.get_or_init(|| resolve(base)).clone()?;
    if accepted != base {
        return Err("attachment reader image changed after preflight".into());
    }
    Ok(())
}
pub(super) fn binding() -> Option<Binding> {
    let &(base, binding) = VALIDATED.get()?.as_ref().ok()?;
    (crate::world_hooks::image_base() == Some(base)).then_some(binding)
}
