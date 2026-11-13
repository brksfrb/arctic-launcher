//! Concrete ConstantExpression class and its immutable stored-value reader.
use crate::native_scene_layout::{self as scene, constant_expression as layout};
use crate::world_hooks::{read, read_profile_function};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::OnceLock;

const CATALOG: &str = include_str!("../inspection/constant-expression-reader-bindings.json");
static VALIDATED: OnceLock<Result<(usize, usize), String>> = OnceLock::new();
fn number(value: &Value) -> Result<usize, String> {
    value
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| "invalid constant expression binding number".into())
}
fn validate(base: usize) -> Result<(usize, usize), String> {
    let catalog: Value = serde_json::from_str(CATALOG).map_err(|error| error.to_string())?;
    if catalog["schema"] != 1 {
        return Err("constant expression schema differs".into());
    }
    let profiles = catalog["profiles"]
        .as_array()
        .ok_or("constant expression profiles absent")?;
    let mut selected = profiles
        .iter()
        .filter(|profile| profile["productVersion"].as_str() == Some(scene::PRODUCT_VERSION));
    let profile = selected
        .next()
        .ok_or("constant expression scene family unavailable")?;
    if selected.next().is_some() || number(&profile["tableRva"])? != layout::TABLE {
        return Err("constant expression class differs".into());
    }
    for (key, expected) in [
        ("objectReadBytes", layout::OBJECT_READ_BYTES),
        ("valueTagOffset", layout::VALUE_TAG),
        ("valueHashOffset", layout::VALUE_HASH),
        ("hashTag", layout::HASH_TAG as usize),
        ("expressionBytes", layout::EXPRESSION_BYTES),
        ("expressionVariantOffset", layout::EXPRESSION_VARIANT),
    ] {
        if number(&profile[key])? != expected {
            return Err(format!("constant expression {key} differs"));
        }
    }
    let table = base
        .checked_add(layout::TABLE)
        .ok_or("constant expression table overflow")?;
    let methods = profile["methods"]
        .as_array()
        .filter(|methods| methods.len() == 16)
        .ok_or("constant expression method inventory differs")?;
    let mut bytes = [0u8; 128];
    read(table, &mut bytes)?;
    for (index, method) in methods.iter().enumerate() {
        let actual = usize::try_from(u64::from_le_bytes(
            bytes[index * 8..index * 8 + 8]
                .try_into()
                .map_err(|_| "constant expression table width")?,
        ))
        .ok();
        if actual != base.checked_add(number(method)?) {
            return Err("constant expression method table differs".into());
        }
    }
    let clone = number(&profile["cloneRva"])?;
    let length = number(&profile["cloneBytes"])?;
    let eval = number(&profile["evalRva"])?;
    if !(1..=4096).contains(&length)
        || number(&methods[1])? != clone
        || number(&methods[2])? != eval
    {
        return Err("constant expression reader methods differ".into());
    }
    let expected = profile["cloneSha256"]
        .as_str()
        .filter(|hash| hash.len() == 64)
        .ok_or("constant expression clone digest absent")?;
    if crate::native_hash::hex(&Sha256::digest(&read_profile_function(clone, length)?)) != expected
    {
        return Err("constant expression clone code differs".into());
    }
    let site = number(&profile["tableLeaRva"])?;
    let end = clone
        .checked_add(length)
        .ok_or("constant expression clone extent overflow")?;
    let after = site
        .checked_add(7)
        .ok_or("constant expression table instruction overflow")?;
    if site < clone || after > end {
        return Err("constant expression table instruction lies outside clone".into());
    }
    let lea = read_profile_function(site, 7)?;
    let displacement = i32::from_le_bytes(
        lea[3..7]
            .try_into()
            .map_err(|_| "constant expression table instruction truncated")?,
    );
    if lea[..3] != [0x48, 0x8d, 0x05]
        || after.checked_add_signed(displacement as isize) != Some(layout::TABLE)
    {
        return Err("constant expression clone class address differs".into());
    }
    if read_profile_function(eval, 5)?.as_slice() != [0x48, 0x8d, 0x41, 0x08, 0xc3] {
        return Err("constant expression generic reader differs".into());
    }
    Ok((base, table))
}
pub(super) fn install(base: usize) -> Result<(), String> {
    if crate::world_hooks::image_base() != Some(base) {
        return Err("constant expression image unavailable".into());
    }
    let (accepted, _) = VALIDATED.get_or_init(|| validate(base)).clone()?;
    if accepted != base {
        return Err("constant expression image changed after preflight".into());
    }
    Ok(())
}
pub(super) fn table() -> Option<usize> {
    let &(base, table) = VALIDATED.get()?.as_ref().ok()?;
    (crate::world_hooks::image_base() == Some(base)).then_some(table)
}
