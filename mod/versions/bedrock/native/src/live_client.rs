//! Read the primary client and its active level/timer without invoking guessed
//! virtual functions. Client/renderer back-references must agree before use.
use crate::native_scene_layout::{client, client_map, game, game_session, minecraft};
use crate::world_hooks::{floats, pointer, read};
#[path = "native_live_client_profile.rs"]
mod profile;
/// Called only after the complete executable hash and code bounds are admitted.
pub(crate) fn bind_profile(base: usize, image_size: usize) -> Result<(), String> {
    profile::install(base, image_size)
}
fn bound_base() -> Option<Option<usize>> {
    if let Some(base) = profile::base() {
        return Some(Some(base));
    }
    // Data-only fixtures have no admitted game image. Production readers must
    // never fall through to that path after a failed family admission.
    crate::world_hooks::image_base().is_none().then_some(None)
}

pub struct Client {
    pub instance: usize,
    pub level: usize,
    pub sim_timer: usize,
    pub real_timer: Option<usize>,
    pub partial: f32,
    pub tps: f32,
    pub local_player: Option<usize>,
}

fn byte(address: usize) -> Option<u8> {
    let mut value = [0];
    read(address, &mut value).ok()?;
    Some(value[0])
}
pub(crate) fn primary(game: usize) -> Option<usize> {
    let base = bound_base()?;
    if let Some(base) = base
        && pointer(game)? != base.checked_add(game::VTABLE)?
    {
        return None;
    }
    let map = game.checked_add(game::PRIMARY_CLIENTS)?;
    const MAP_BYTES: usize = client_map::COUNT + 8;
    const NODE_BYTES: usize = client_map::NODE_CLIENT + 8;
    const HEAD_BYTES: usize = client_map::NODE_NIL + 1;
    let mut map_bytes = [0u8; MAP_BYTES];
    read(map, &mut map_bytes).ok()?;
    let head = copied_pointer(&map_bytes, client_map::HEAD)?;
    let count = usize::try_from(u64::from_le_bytes(
        map_bytes[client_map::COUNT..client_map::COUNT + 8]
            .try_into()
            .ok()?,
    ))
    .ok()?;
    if count == 0 || count > 16 {
        return None;
    }
    let mut head_bytes = [0u8; HEAD_BYTES];
    read(head, &mut head_bytes).ok()?;
    if head_bytes[client_map::NODE_NIL] != 1 {
        return None;
    }
    let mut node = copied_pointer(&head_bytes, client_map::NODE_PARENT)?;
    let mut parent = head;
    let mut copies = Vec::new();
    for _ in 0..count {
        if node == head || copies.iter().any(|(address, _)| *address == node) {
            return None;
        }
        let mut bytes = [0u8; NODE_BYTES];
        read(node, &mut bytes).ok()?;
        if bytes[client_map::NODE_NIL] != 0
            || copied_pointer(&bytes, client_map::NODE_PARENT)? != parent
        {
            return None;
        }
        let selected = (bytes[client_map::NODE_KEY] == 0)
            .then(|| copied_pointer(&bytes, client_map::NODE_CLIENT));
        let next = copied_pointer(&bytes, client_map::NODE_LEFT);
        copies.push((node, bytes));
        if let Some(selected) = selected {
            let selected = selected?;
            if let Some(base) = base
                && pointer(selected)? != base.checked_add(client::VTABLE)?
            {
                return None;
            }
            for (address, before) in copies {
                let mut after = [0u8; NODE_BYTES];
                read(address, &mut after).ok()?;
                if before != after {
                    return None;
                }
            }
            let mut after_head = [0u8; HEAD_BYTES];
            read(head, &mut after_head).ok()?;
            let mut after_map = [0u8; MAP_BYTES];
            read(map, &mut after_map).ok()?;
            if head_bytes != after_head || map_bytes != after_map {
                return None;
            }
            if let Some(base) = base
                && (pointer(game)? != base.checked_add(game::VTABLE)?
                    || pointer(selected)? != base.checked_add(client::VTABLE)?)
            {
                return None;
            }
            return Some(selected);
        }
        parent = node;
        node = next?;
    }
    None
}
fn copied_pointer(bytes: &[u8], offset: usize) -> Option<usize> {
    let value = usize::try_from(u64::from_le_bytes(
        bytes.get(offset..offset.checked_add(8)?)?.try_into().ok()?,
    ))
    .ok()?;
    (0x10000..0x8000_0000_0000)
        .contains(&value)
        .then_some(value)
}

fn getter(object: usize, index: usize) -> Option<usize> {
    let table = pointer(object)?;
    let mut code = pointer(table.checked_add(index.checked_mul(8)?)?)?;
    let base = bound_base()?.unwrap_or(0);
    if base != 0 {
        if table != base.checked_add(client::VTABLE)?
            || index != client::LOCAL_PLAYER_GETTER_SLOT
            || code.checked_sub(base)? != client::LOCAL_PLAYER_GETTER_RVA
        {
            return None;
        }
        return crate::ecs::weak_local_player(object.checked_add(client::WEAK_PLAYER_REFERENCE)?);
    }
    for _ in 0..3 {
        let mut bytes = [0u8; 16];
        read(code, &mut bytes).ok()?;
        if bytes[0] == 0xe9 {
            let displacement = i32::from_le_bytes(bytes[1..5].try_into().ok()?);
            code = code
                .checked_add(5)?
                .checked_add_signed(displacement as isize)?;
            continue;
        }
        // Unbound data fixtures may describe a plain pointer getter. Production
        // never falls back from a changed exact-family method to guessed code.
        let offset = if bytes[..3] == [0x48, 0x8b, 0x81] && bytes[7] == 0xc3 {
            u32::from_le_bytes(bytes[3..7].try_into().ok()?) as usize
        } else if bytes[..3] == [0x48, 0x8b, 0x41] && bytes[4] == 0xc3 {
            bytes[3] as usize
        } else {
            return None;
        };
        if offset > 0x10000 {
            return None;
        }
        return pointer(object.checked_add(offset)?);
    }
    None
}

/// Compare method addresses only. Neither getter nor the session predicate is
/// invoked. The exact-image base is published before production scene reads.
fn minecraft_identity(object: usize, base: usize) -> Option<()> {
    let table = pointer(object)?;
    if table != base.checked_add(minecraft::VTABLE)?
        || pointer(table.checked_add(minecraft::SIM_PAUSE_GETTER_SLOT.checked_mul(8)?)?)?
            != base.checked_add(minecraft::SIM_PAUSE_GETTER_RVA)?
        || pointer(table.checked_add(minecraft::SESSION_ACCESS_SLOT.checked_mul(8)?)?)?
            != base.checked_add(minecraft::SESSION_ACCESS_RVA)?
    {
        return None;
    }
    Some(())
}

pub fn read_client(game: usize, renderer: usize) -> Option<Client> {
    let client = primary(game)?;
    if pointer(client.checked_add(client::GAME)?)? != game
        || pointer(client.checked_add(client::LEVEL_RENDERER)?)? != renderer
    {
        return None;
    }
    let minecraft = pointer(client.checked_add(client::MINECRAFT)?)?;
    let base = bound_base()?.unwrap_or(0);
    // Unbound data fixtures retain their existing path; all production scene
    // reads enter after bind_profile. This does not enable another image family.
    if base != 0 {
        minecraft_identity(minecraft, base)?;
    }
    let session = pointer(minecraft.checked_add(minecraft::SESSION)?)?;
    let active = pointer(session.checked_add(game_session::ACTIVE_OWNER)?)?;
    if byte(session.checked_add(game_session::READY)?)? != 1
        || byte(active.checked_add(game_session::ACTIVE)?)? != 1
    {
        return None;
    }
    let level = pointer(session.checked_add(game_session::LEVEL)?)?;
    let sim_timer = pointer(minecraft.checked_add(minecraft::SIM_TIMER)?)?;
    let timer = floats::<5>(sim_timer, 10000.)?;
    if !(1.0..=1000.).contains(&timer[0]) || !(0.0..=1.).contains(&timer[2]) {
        return None;
    }
    let real_candidate = pointer(minecraft.checked_add(minecraft::REAL_TIMER)?);
    let real_timer = real_candidate.filter(|p| {
        floats::<5>(*p, 10000.)
            .is_some_and(|v| (1. ..=1000.).contains(&v[0]) && (0. ..=1.).contains(&v[2]))
    });
    let local_player = getter(client, client::LOCAL_PLAYER_GETTER_SLOT)
        .filter(|actor| crate::ecs::actor_type(*actor) == Some(319));
    // A pointer graph read is not an ownership lease. Reject an observed world
    // transition instead of combining old timer values with a new session.
    if primary(game)? != client
        || pointer(client.checked_add(client::GAME)?)? != game
        || pointer(client.checked_add(client::LEVEL_RENDERER)?)? != renderer
        || pointer(client.checked_add(client::MINECRAFT)?)? != minecraft
        || pointer(minecraft.checked_add(minecraft::SESSION)?)? != session
        || pointer(minecraft.checked_add(minecraft::SIM_TIMER)?)? != sim_timer
        || pointer(minecraft.checked_add(minecraft::REAL_TIMER)?) != real_candidate
        || pointer(session.checked_add(game_session::ACTIVE_OWNER)?)? != active
        || pointer(session.checked_add(game_session::LEVEL)?)? != level
        || byte(session.checked_add(game_session::READY)?)? != 1
        || byte(active.checked_add(game_session::ACTIVE)?)? != 1
    {
        return None;
    }
    if floats::<5>(sim_timer, 10000.)?.map(f32::to_bits) != timer.map(f32::to_bits) {
        return None;
    }
    if base != 0 {
        minecraft_identity(minecraft, base)?;
    }
    Some(Client {
        instance: client,
        level,
        sim_timer,
        real_timer,
        partial: timer[2],
        tps: timer[0],
        local_player,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn point(bytes: &mut [u8], at: usize, value: usize) {
        bytes[at..at + 8].copy_from_slice(&(value as u64).to_le_bytes())
    }
    #[test]
    fn primary_client_requires_a_live_level_and_matching_renderer_then_copies_timer() {
        let mut game = vec![0u8; game::PRIMARY_CLIENTS + client_map::COUNT + 8];
        let mut head = vec![0u8; client_map::NODE_NIL + 1];
        let mut node = vec![0u8; client_map::NODE_CLIENT + 8];
        let mut client = vec![0u8; client::LEVEL_RENDERER + 8];
        let mut minecraft = vec![0u8; minecraft::REAL_TIMER + 8];
        let mut session = vec![0u8; game_session::LEVEL + 8];
        let active = 1u8;
        let level = Box::new(0u64);
        let timer = [20f32, 0., 0.25, 1., 0.];
        let real_timer = [20f32, 0., 0.5, 1., 0.];
        point(
            &mut game,
            game::PRIMARY_CLIENTS + client_map::HEAD,
            head.as_ptr() as usize,
        );
        point(&mut game, game::PRIMARY_CLIENTS + client_map::COUNT, 1);
        point(&mut head, client_map::NODE_PARENT, node.as_ptr() as usize);
        head[client_map::NODE_NIL] = 1;
        std::hint::black_box(&head);
        point(&mut node, client_map::NODE_PARENT, head.as_ptr() as usize);
        point(&mut node, client_map::NODE_LEFT, head.as_ptr() as usize);
        point(&mut node, client_map::NODE_CLIENT, client.as_ptr() as usize);
        point(&mut client, client::GAME, game.as_ptr() as usize);
        point(&mut client, client::LEVEL_RENDERER, 0x234567);
        point(&mut client, client::MINECRAFT, minecraft.as_ptr() as usize);
        point(
            &mut minecraft,
            minecraft::SESSION,
            session.as_ptr() as usize,
        );
        point(
            &mut minecraft,
            minecraft::SIM_TIMER,
            timer.as_ptr() as usize,
        );
        point(
            &mut minecraft,
            minecraft::REAL_TIMER,
            real_timer.as_ptr() as usize,
        );
        session[game_session::READY] = 1;
        point(
            &mut session,
            game_session::ACTIVE_OWNER,
            &active as *const u8 as usize,
        );
        point(
            &mut session,
            game_session::LEVEL,
            &*level as *const u64 as usize,
        );
        let copied = read_client(game.as_ptr() as usize, 0x234567).unwrap();
        assert_eq!(copied.partial, 0.25);
        assert_eq!(copied.tps, 20.);
        assert_eq!(copied.real_timer, Some(real_timer.as_ptr() as usize));
        assert!(read_client(game.as_ptr() as usize, 0x999999).is_none());
        session[game_session::READY] = 0;
        std::hint::black_box(&session);
        assert!(read_client(game.as_ptr() as usize, 0x234567).is_none());
        assert_eq!(copied.partial, 0.25);
    }
}
