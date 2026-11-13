//! Exact Game/ClientInstance/Minecraft metadata family; no native getter calls.
use crate::native_scene_layout::{
    self as scene, client, client_map, game, game_session, minecraft,
};
use crate::world_hooks::{image_base, read, read_profile_function};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    sync::OnceLock,
};
const CATALOG: &str = include_str!("../inspection/live-client-reader-bindings.json");
static BINDING: OnceLock<Result<(usize, usize), String>> = OnceLock::new();
fn number(value: &Value) -> Result<usize, String> {
    value
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| "invalid live-client binding number".into())
}
fn resolve(base: usize, image_size: usize) -> Result<(usize, usize), String> {
    let catalog: Value = serde_json::from_str(CATALOG).map_err(|error| error.to_string())?;
    if catalog["schema"] != 1 {
        return Err("live-client reader schema differs".into());
    }
    let profiles = catalog["profiles"]
        .as_array()
        .ok_or("live-client profiles absent")?;
    let mut selected = profiles
        .iter()
        .filter(|p| p["productVersion"].as_str() == Some(scene::PRODUCT_VERSION));
    let profile = selected
        .next()
        .ok_or("live-client scene family unavailable")?;
    if selected.next().is_some() || number(&profile["imageSize"])? != image_size {
        return Err("live-client scene image differs".into());
    }
    for (key, expected) in [
        ("gameVtable", game::VTABLE),
        ("primaryClients", game::PRIMARY_CLIENTS),
        ("gameRenderer", game::RENDERER),
        ("mapHead", client_map::HEAD),
        ("mapCount", client_map::COUNT),
        ("nodeLeft", client_map::NODE_LEFT),
        ("nodeParent", client_map::NODE_PARENT),
        ("nodeNil", client_map::NODE_NIL),
        ("nodeKey", client_map::NODE_KEY),
        ("nodeClient", client_map::NODE_CLIENT),
        ("clientVtable", client::VTABLE),
        ("clientGame", client::GAME),
        ("clientMinecraft", client::MINECRAFT),
        ("clientLevelRenderer", client::LEVEL_RENDERER),
        ("localPlayerGetterSlot", client::LOCAL_PLAYER_GETTER_SLOT),
        ("localPlayerGetterRva", client::LOCAL_PLAYER_GETTER_RVA),
        ("cameraGetterSlot", client::CAMERA_GETTER_SLOT),
        ("cameraGetterRva", client::CAMERA_GETTER_RVA),
        ("weakCameraReference", client::WEAK_CAMERA_REFERENCE),
        ("weakPlayerReference", client::WEAK_PLAYER_REFERENCE),
        ("minecraftVtable", minecraft::VTABLE),
        ("minecraftSession", minecraft::SESSION),
        ("simulationTimer", minecraft::SIM_TIMER),
        ("realTimer", minecraft::REAL_TIMER),
        (
            "simulationPauseGetterSlot",
            minecraft::SIM_PAUSE_GETTER_SLOT,
        ),
        ("simulationPauseGetterRva", minecraft::SIM_PAUSE_GETTER_RVA),
        ("sessionAccessSlot", minecraft::SESSION_ACCESS_SLOT),
        ("sessionAccessRva", minecraft::SESSION_ACCESS_RVA),
        ("sessionReady", game_session::READY),
        ("sessionActiveOwner", game_session::ACTIVE_OWNER),
        ("sessionActive", game_session::ACTIVE),
        ("sessionLevel", game_session::LEVEL),
    ] {
        if number(&profile["layout"][key])? != expected {
            return Err(format!("live-client {key} differs"));
        }
    }
    if client_map::HEAD != 0
        || client_map::COUNT != 8
        || client_map::NODE_CLIENT > 120
        || client_map::NODE_NIL >= client_map::NODE_CLIENT
        || client_map::NODE_KEY >= client_map::NODE_CLIENT
        || client_map::NODE_PARENT
            .checked_add(8)
            .is_none_or(|n| n > client_map::NODE_NIL + 1)
        || client_map::NODE_LEFT
            .checked_add(8)
            .is_none_or(|n| n > client_map::NODE_CLIENT)
    {
        return Err("live-client map header bounds differ".into());
    }
    let functions = profile["functions"]
        .as_object()
        .filter(|f| f.len() == 11)
        .ok_or("live-client function inventory differs")?;
    let mut roles = HashMap::new();
    let mut starts = HashSet::new();
    let mut total = 0usize;
    for role in [
        "gameConstructor",
        "gameLookup",
        "gameRendererGetter",
        "clientConstructor",
        "localPlayerGetter",
        "cameraGetter",
        "clientRendererGetter",
        "clientLevelGetter",
        "minecraftConstructor",
        "simulationPauseGetter",
        "sessionPredicate",
    ] {
        let function = functions.get(role).ok_or("live-client function absent")?;
        let rva = number(&function["rva"])?;
        let length = number(&function["bytes"])?;
        total = total
            .checked_add(length)
            .filter(|n| *n <= 32 * 1024)
            .ok_or("live-client code budget exceeded")?;
        if !starts.insert(rva) {
            return Err("live-client function roles repeat".into());
        }
        let expected = function["sha256"]
            .as_str()
            .filter(|hash| hash.len() == 64)
            .ok_or("live-client function digest absent")?;
        if crate::native_hash::hex(&Sha256::digest(&read_profile_function(rva, length)?))
            != expected
        {
            return Err(format!("live-client {role} code differs"));
        }
        roles.insert(role, (rva, length));
    }
    if roles["localPlayerGetter"].0 != client::LOCAL_PLAYER_GETTER_RVA
        || roles["cameraGetter"].0 != client::CAMERA_GETTER_RVA
        || roles["simulationPauseGetter"].0 != minecraft::SIM_PAUSE_GETTER_RVA
        || roles["sessionPredicate"].0 != minecraft::SESSION_ACCESS_RVA
    {
        return Err("live-client reader and method entries differ".into());
    }
    let classes = profile["classes"]
        .as_array()
        .filter(|v| v.len() == 3)
        .ok_or("live-client class inventory differs")?;
    for (class, (role, table)) in classes.iter().zip([
        ("gameConstructor", game::VTABLE),
        ("clientConstructor", client::VTABLE),
        ("minecraftConstructor", minecraft::VTABLE),
    ]) {
        if class["role"].as_str() != Some(role) || number(&class["tableRva"])? != table {
            return Err("live-client class roles differ".into());
        }
        let (begin, length) = roles[role];
        let site = number(&class["siteRva"])?;
        let after = site
            .checked_add(7)
            .ok_or("live-client class LEA overflow")?;
        if site < begin
            || after
                > begin
                    .checked_add(length)
                    .ok_or("live-client constructor extent overflow")?
        {
            return Err("live-client class LEA lies outside constructor".into());
        }
        let bytes = read_profile_function(site, 7)?;
        let displacement = i32::from_le_bytes(
            bytes[3..7]
                .try_into()
                .map_err(|_| "live-client LEA truncated")?,
        );
        if bytes[0] & 0xf8 != 0x48
            || bytes[1] != 0x8d
            || bytes[2] & 0xc7 != 5
            || after.checked_add_signed(displacement as isize) != Some(table)
        {
            return Err("live-client constructor table differs".into());
        }
    }
    let methods = profile["methods"]
        .as_array()
        .filter(|v| v.len() == 7)
        .ok_or("live-client method inventory differs")?;
    let expected = [
        (
            game::VTABLE,
            game::RENDERER_GETTER_SLOT,
            "gameRendererGetter",
        ),
        (
            client::VTABLE,
            client::LOCAL_PLAYER_GETTER_SLOT,
            "localPlayerGetter",
        ),
        (client::VTABLE, client::CAMERA_GETTER_SLOT, "cameraGetter"),
        (
            client::VTABLE,
            client::LEVEL_RENDERER_GETTER_SLOT,
            "clientRendererGetter",
        ),
        (
            client::VTABLE,
            client::LEVEL_GETTER_SLOT,
            "clientLevelGetter",
        ),
        (
            minecraft::VTABLE,
            minecraft::SIM_PAUSE_GETTER_SLOT,
            "simulationPauseGetter",
        ),
        (
            minecraft::VTABLE,
            minecraft::SESSION_ACCESS_SLOT,
            "sessionPredicate",
        ),
    ];
    let mut slots = HashSet::new();
    for (method, (table, slot, role)) in methods.iter().zip(expected) {
        if number(&method["tableRva"])? != table
            || number(&method["slot"])? != slot
            || method["role"].as_str() != Some(role)
        {
            return Err("live-client method roles differ".into());
        }
        let at = slot
            .checked_mul(8)
            .and_then(|n| table.checked_add(n))
            .ok_or("live-client method slot overflow")?;
        if !slots.insert(at) || at & 7 != 0 || at.checked_add(8).is_none_or(|end| end > image_size)
        {
            return Err("live-client method repeats or lies outside image".into());
        }
        let mut bytes = [0u8; 8];
        read(
            base.checked_add(at)
                .ok_or("live-client method address overflow")?,
            &mut bytes,
        )?;
        if usize::try_from(u64::from_le_bytes(bytes)).ok() != base.checked_add(roles[role].0) {
            return Err("concrete live-client method table differs".into());
        }
    }
    Ok((base, image_size))
}
pub(super) fn install(base: usize, image_size: usize) -> Result<(), String> {
    if image_base() != Some(base) {
        return Err("live-client image unavailable".into());
    }
    let (accepted, size) = BINDING.get_or_init(|| resolve(base, image_size)).clone()?;
    if accepted != base || size != image_size {
        return Err("live-client image changed after preflight".into());
    }
    Ok(())
}
pub(super) fn base() -> Option<usize> {
    let &(base, _) = BINDING.get()?.as_ref().ok()?;
    (image_base() == Some(base)).then_some(base)
}
