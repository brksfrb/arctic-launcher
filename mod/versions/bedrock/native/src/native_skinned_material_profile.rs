//! Concrete skinned MaterialPtr info/path/variation readers share a preflight.
//! Terrain layer validation remains separate from their native copy chain.
use crate::native_scene_layout::{
    self as scene, material as layout, material_variation as variation, skinned_description as desc,
};
use crate::world_hooks::{read, read_profile_function};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    sync::OnceLock,
};

const BINDINGS: &str = include_str!("../inspection/skinned-material-reader-bindings.json");
static INSTALL: OnceLock<Result<usize, String>> = OnceLock::new();
fn number(value: &Value) -> Result<usize, String> {
    value
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| "invalid material reader binding number".into())
}
fn table(base: usize, rva: usize, methods: &Value, count: usize) -> Result<Vec<usize>, String> {
    let methods = methods
        .as_array()
        .filter(|values| values.len() == count)
        .ok_or("material reader table shape differs")?;
    let mut result = Vec::with_capacity(count);
    for (index, method) in methods.iter().enumerate() {
        let method = number(method)?;
        let mut bytes = [0u8; 8];
        read(
            base.checked_add(rva)
                .and_then(|n| n.checked_add(index * 8))
                .ok_or("material reader table overflow")?,
            &mut bytes,
        )?;
        if usize::try_from(u64::from_le_bytes(bytes)).ok() != base.checked_add(method) {
            return Err("material reader concrete class differs".into());
        }
        result.push(method);
    }
    Ok(result)
}
fn variation_binding(profile: &Value, roles: &HashMap<&str, (usize, usize)>) -> Result<(), String> {
    for (key, expected) in [
        ("definesOffset", variation::DEFINES),
        ("stateMaskOffset", variation::STATE_MASK),
        ("setHead", variation::SET_HEAD),
        ("setCount", variation::SET_COUNT),
        ("fieldsBytes", variation::FIELDS_BYTES),
        ("sentinelBytes", variation::SENTINEL_BYTES),
        ("nodeBytes", variation::NODE_BYTES),
        ("nodeKeyOffset", variation::KEY),
        ("stringBytes", variation::STRING_BYTES),
        ("stringLengthOffset", variation::STRING_LENGTH),
        ("stringCapacityOffset", variation::STRING_CAPACITY),
        ("stringInlineCapacity", variation::STRING_INLINE_CAPACITY),
        ("nodeLeft", variation::LEFT),
        ("nodeParent", variation::PARENT),
        ("nodeRight", variation::RIGHT),
        ("nodeColor", variation::COLOR),
        ("nodeNil", variation::NIL),
    ] {
        if number(&profile["variationLayout"][key])? != expected {
            return Err(format!("material variation {key} differs"));
        }
    }
    let edges = profile["variationEdges"]
        .as_array()
        .filter(|edges| edges.len() == 5)
        .ok_or("material variation call inventory differs")?;
    let expected = [
        ("materialCopy", "defineSetCopy"),
        ("defineSetCopy", "defineTreeClone"),
        ("defineTreeClone", "defineNodeConstruct"),
        ("defineTreeClone", "defineTreeClone"),
        ("defineTreeClone", "defineTreeClone"),
    ];
    let mut seen = HashSet::new();
    for (edge, (from, to)) in edges.iter().zip(expected) {
        if edge["from"].as_str() != Some(from) || edge["to"].as_str() != Some(to) {
            return Err("material variation call roles differ".into());
        }
        let &(begin, length) = roles.get(from).ok_or("material variation caller absent")?;
        let &(target, _) = roles.get(to).ok_or("material variation callee absent")?;
        let site = number(&edge["siteRva"])?;
        let end = begin
            .checked_add(length)
            .ok_or("material variation function extent overflow")?;
        if !seen.insert(site) || site < begin || site.checked_add(5).is_none_or(|after| after > end)
        {
            return Err("material variation callsite differs".into());
        }
        let bytes = read_profile_function(site, 5)?;
        let displacement = i32::from_le_bytes(
            bytes[1..5]
                .try_into()
                .map_err(|_| "material variation call truncated")?,
        );
        if bytes[0] != 0xe8
            || site
                .checked_add(5)
                .and_then(|after| after.checked_add_signed(displacement as isize))
                != Some(target)
        {
            return Err("material variation direct call differs".into());
        }
    }
    Ok(())
}
fn validate(base: usize) -> Result<usize, String> {
    let catalog: Value = serde_json::from_str(BINDINGS).map_err(|error| error.to_string())?;
    if catalog["schema"] != 1 {
        return Err("material reader binding schema differs".into());
    }
    let profiles = catalog["profiles"]
        .as_array()
        .ok_or("material reader profiles absent")?;
    let mut selected = profiles
        .iter()
        .filter(|profile| profile["productVersion"].as_str() == Some(scene::PRODUCT_VERSION));
    let profile = selected
        .next()
        .ok_or("material reader scene family unavailable")?;
    if selected.next().is_some()
        || number(&profile["materialTableRva"])? != layout::VTABLE
        || number(&profile["controlTableRva"])? != layout::INFO_CONTROL_VTABLE
        || number(&profile["infoInControl"])? != layout::INFO_IN_CONTROL
        || number(&profile["renderMaterialInInfo"])? != layout::INFO_RENDER_MATERIAL
        || number(&profile["shaderPathsOffset"])? != layout::SHADER_PATHS
        || number(&profile["descriptionMaterialOffset"])? != desc::MATERIAL
    {
        return Err("material reader and selected scene layout differ".into());
    }
    let material = table(base, layout::VTABLE, &profile["materialMethods"], 5)?;
    let control = table(
        base,
        layout::INFO_CONTROL_VTABLE,
        &profile["controlMethods"],
        3,
    )?;
    if control.as_slice()
        != [
            layout::INFO_DESTROY_RVA,
            layout::INFO_DELETE_RVA,
            layout::INFO_DELETING_DTOR_RVA,
        ]
        .as_slice()
    {
        return Err("material reader info destructor family differs".into());
    }
    let functions = profile["functions"]
        .as_array()
        .filter(|values| values.len() == 11)
        .ok_or("material reader function inventory differs")?;
    let mut roles = HashMap::new();
    let mut total = 0usize;
    for function in functions {
        let role = function["role"]
            .as_str()
            .ok_or("material reader function role absent")?;
        let rva = number(&function["rva"])?;
        let length = number(&function["bytes"])?;
        total = total
            .checked_add(length)
            .filter(|total| *total <= 128 * 1024)
            .ok_or("material reader code budget exceeded")?;
        let expected = function["sha256"]
            .as_str()
            .filter(|hash| hash.len() == 64)
            .ok_or("material reader function digest absent")?;
        let bytes = read_profile_function(rva, length)?;
        if crate::native_hash::hex(&Sha256::digest(&bytes)) != expected {
            return Err(format!("material reader {role} code differs"));
        }
        if roles.insert(role, (rva, length)).is_some() {
            return Err("duplicate material reader function role".into());
        }
    }
    let description = crate::world_hooks::skinned_description_binding()?;
    let consumer = crate::world_hooks::skinned_material_consumer_binding()?;
    for (role, rva) in [
        ("description", description.rva),
        ("consumer", consumer.material_rva),
        ("infoDestroy", control[0]),
        ("infoDelete", control[1]),
        ("infoDeletingDestructor", control[2]),
        ("shaderPathParser", material[4]),
    ] {
        if roles.get(role).map(|entry| entry.0) != Some(rva) {
            return Err(format!("material reader {role} entry differs"));
        }
    }
    if !roles.contains_key("infoLoader") || !roles.contains_key("materialCopy") {
        return Err("material reader construction evidence absent".into());
    }
    variation_binding(profile, &roles)?;
    Ok(base)
}
pub(super) fn install(base: usize) -> Result<(), String> {
    if crate::world_hooks::image_base() != Some(base) {
        return Err("material reader image unavailable".into());
    }
    let accepted = INSTALL.get_or_init(|| validate(base)).clone()?;
    if accepted != base {
        return Err("material reader image changed after preflight".into());
    }
    Ok(())
}
