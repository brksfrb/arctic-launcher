//! Instruction windows for the selected controller/weak geometry pair reader.
//! Whole-image acceptance and the body/controller/root join precede these reads.
use crate::native_scene_layout::{self as scene, model_body, skin_selection as layout};
use crate::world_hooks::{model_body_binding, read_profile_function};
use serde_json::Value;
use std::{collections::HashSet, sync::OnceLock};

const CATALOG: &str = include_str!("../inspection/skin-selection-reader-bindings.json");
static VALIDATED: OnceLock<Result<usize, String>> = OnceLock::new();
fn number(value: &Value) -> Result<usize, String> {
    value
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| "invalid skin reader binding number".into())
}
fn validate() -> Result<usize, String> {
    let base = crate::world_hooks::image_base().ok_or("skin reader image unavailable")?;
    let body = model_body_binding()?;
    let catalog: Value = serde_json::from_str(CATALOG).map_err(|error| error.to_string())?;
    if catalog["schema"] != 1 {
        return Err("skin reader binding schema differs".into());
    }
    let profiles = catalog["profiles"]
        .as_array()
        .ok_or("skin reader profiles absent")?;
    let mut selected = profiles
        .iter()
        .filter(|profile| profile["productVersion"].as_str() == Some(scene::PRODUCT_VERSION));
    let profile = selected
        .next()
        .ok_or("skin reader scene family unavailable")?;
    if selected.next().is_some()
        || number(&profile["actorComponent"])? != scene::actor::ANIMATION_COMPONENT
    {
        return Err("skin reader actor layout differs".into());
    }
    for (key, expected) in [
        ("componentParams", layout::COMPONENT_PARAMS),
        ("componentFrame", layout::COMPONENT_FRAME),
        ("componentPairs", layout::COMPONENT_PAIRS),
        ("pairStride", layout::PAIR_STRIDE),
        ("controller", layout::CONTROLLER),
        ("controllerControl", layout::CONTROLLER_CONTROL),
        ("geometry", layout::GEOMETRY),
        ("geometryControl", layout::GEOMETRY_CONTROL),
        ("geometryHierarchy", model_body::GEOMETRY_HIERARCHY),
    ] {
        if number(&profile["layout"][key])? != expected {
            return Err(format!("skin reader {key} differs"));
        }
    }
    for (role, expected) in [("root", body.root_rva), ("controller", body.caller_rva)] {
        if number(&profile["functions"][role]["rva"])? != expected {
            return Err("skin reader draw relationship differs".into());
        }
    }
    let sites = profile["sites"]
        .as_array()
        .filter(|sites| sites.len() == 10)
        .ok_or("skin reader site inventory differs")?;
    let roles = [
        ("componentParams", "root"),
        ("frameRead", "root"),
        ("pairBounds", "root"),
        ("pairAdvance", "root"),
        ("controllerBorrow", "root"),
        ("geometryWeakBorrow", "root"),
        ("geometryPromotion", "root"),
        ("controllerDispatch", "root"),
        ("hierarchySelection", "controller"),
        ("bodyDispatch", "controller"),
    ];
    let mut seen = HashSet::new();
    let mut total = 0usize;
    for (site, (role, owner)) in sites.iter().zip(roles) {
        if site["role"].as_str() != Some(role) || site["function"].as_str() != Some(owner) {
            return Err("skin reader site roles differ".into());
        }
        let function = &profile["functions"][owner];
        let begin = number(&function["rva"])?;
        let function_bytes = number(&function["bytes"])?;
        if !(1..=65536).contains(&function_bytes) {
            return Err("skin reader function size differs".into());
        }
        let end = begin
            .checked_add(function_bytes)
            .ok_or("skin reader function extent overflow")?;
        let rva = number(&site["rva"])?;
        let length = number(&site["bytes"])?;
        total = total
            .checked_add(length)
            .filter(|n| *n <= 1024)
            .ok_or("skin reader code budget exceeded")?;
        if !(1..=128).contains(&length)
            || rva < begin
            || rva.checked_add(length).is_none_or(|last| last > end)
            || !seen.insert(rva)
        {
            return Err("skin reader site lies outside selected draw".into());
        }
        let expected = site["hex"]
            .as_str()
            .filter(|hex| hex.len() == length * 2)
            .ok_or("skin reader instruction bytes absent")?;
        if crate::native_hash::hex(&read_profile_function(rva, length)?) != expected {
            return Err(format!("skin reader {role} instructions differ"));
        }
    }
    Ok(base)
}
pub(super) fn install(base: usize) -> Result<(), String> {
    if crate::world_hooks::image_base() != Some(base) {
        return Err("skin reader image unavailable".into());
    }
    let accepted = VALIDATED.get_or_init(validate).clone()?;
    if accepted != base {
        return Err("skin reader image changed after preflight".into());
    }
    Ok(())
}
pub(super) fn installed() -> bool {
    matches!(VALIDATED.get(), Some(Ok(_)))
}
