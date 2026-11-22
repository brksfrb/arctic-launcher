//! Signatures and layouts for the core game-state bindings (in a world, game
//! screen open, local player position, cursor grab, mouse queue).
//!
//! Signatures and field offsets are adapted from LatiteClient/Latite v2.9.1
//! (commit 0c32cbe035b3f4432e51cddadd2ae5a289bc7f89, GPL-3.0; it lists
//! "1.26.6x" as supported) `src/mc/Addresses.h` and its SDK headers. See
//! THIRD_PARTY.md. Unlike the exact-build cosmetics profile, nothing here is
//! gated on a PE timestamp or `.text` hash: each signature is scanned for and
//! logged as found/missing/ambiguous, and every field offset is checked
//! against RTTI class names or value invariants before it is trusted.

use crate::sigscan::Resolve;

pub const SCREEN_VIEW_SETUP_AND_RENDER: &str = "ScreenView::setupAndRender";
pub const CLIENT_INSTANCE_GRAB_CURSOR: &str = "ClientInstance::grabCursor";
pub const CLIENT_INSTANCE_RELEASE_CURSOR: &str = "ClientInstance::releaseCursor";
pub const MOUSE_PROCESS_INPUT_QUEUE: &str = "Mouse::processInputQueue";
pub const MOUSE_DEVICE_INSTANCE: &str = "MouseDevice::_instance";

/// (name, signature, how to resolve, what it is used for)
pub const SIGNATURES: &[(&str, &str, Resolve, &str)] = &[
    (
        SCREEN_VIEW_SETUP_AND_RENDER,
        "E8 ? ? ? ? 90 48 8B 4B ? 48 85 C9 74 ? 48 8B 01 48 8B 40 ? 48 89 FA FF 15 ? ? ? ? 90 48 8D 4D",
        Resolve::Relative { field: 1, end: 5 },
        "in-world and game-screen detection, ClientInstance",
    ),
    (
        CLIENT_INSTANCE_GRAB_CURSOR,
        "56 48 83 EC ? 48 89 CE 48 8B 01 48 8B 80 ? ? ? ? FF 15 ? ? ? ? 84 C0 74 ? 48 8B 8E ? ? ? ? 48 8B 01 48 8B 80 ? ? ? ? 48 8B 15 ? ? ? ? 48 83 C4 ? 5E 48 FF E2 48 83 C4 ? 5E C3 CC CC CC CC CC CC CC CC CC CC CC CC CC CC 56 48 83 EC",
        Resolve::Direct,
        "re-capture the cursor when the Arctic menu closes",
    ),
    (
        CLIENT_INSTANCE_RELEASE_CURSOR,
        "56 48 83 EC ? 48 89 CE 48 8B 01 48 8B 80 ? ? ? ? FF 15 ? ? ? ? 84 C0 74 ? 48 8B 8E ? ? ? ? 48 8B 01 48 8B 80 ? ? ? ? 48 8B 15 ? ? ? ? 48 83 C4 ? 5E 48 FF E2 48 83 C4 ? 5E C3 CC CC CC CC CC CC CC CC CC CC CC CC CC CC 56 53",
        Resolve::Direct,
        "release the cursor and stop mouse-look while the Arctic menu is open",
    ),
    (
        MOUSE_PROCESS_INPUT_QUEUE,
        "41 57 41 56 41 55 41 54 56 57 55 53 48 83 EC ? 44 0F 29 44 24 ? 0F 29 7C 24 ? 0F 29 74 24 ? 49 89 CE C7 05 ? ? ? ? FF FF FF FF 48 8B 0D ? ? ? ? 48 8B 05 ? ? ? ? 48 29 C1 48 C1 E9 02 69 C9 CD CC CC CC 89 4C 24 ? 85 C9",
        Resolve::Direct,
        "keep queued clicks away from the game while the Arctic menu is open",
    ),
    (
        MOUSE_DEVICE_INSTANCE,
        "89 15 ? ? ? ? C7 47",
        Resolve::Relative { field: 2, end: 6 },
        "the game's mouse event queue",
    ),
];

/// RTTI type names (MSVC TypeDescriptor names) used to verify pointers.
pub const RTTI_CLIENT_INSTANCE: &str = ".?AVClientInstance@@";
pub const RTTI_LOCAL_PLAYER: &str = ".?AVLocalPlayer@@";
pub const RTTI_MINECRAFT_GAME: &str = ".?AVMinecraftGame@@";

/// Field offsets known for 1.26.6x (Latite SDK). Each has a calibration scan
/// range used when the known offset fails its check after an update.
pub struct Field {
    pub name: &'static str,
    pub known: usize,
    pub scan: (usize, usize),
}

/// MinecraftUIRenderContext -> ClientInstance*.
pub const CONTEXT_CLIENT: Field = Field {
    name: "MinecraftUIRenderContext.clientInstance",
    known: 0x8,
    scan: (0x0, 0x40),
};
/// ScreenView -> VisualTree*.
pub const VIEW_TREE: Field = Field {
    name: "ScreenView.visualTree",
    known: 0x50,
    scan: (0x8, 0xA0),
};
/// VisualTree -> root UIControl*.
pub const TREE_ROOT: Field = Field {
    name: "VisualTree.rootControl",
    known: 0x8,
    scan: (0x8, 0x20),
};
/// UIControl -> std::string name.
pub const CONTROL_NAME: Field = Field {
    name: "UIControl.name",
    known: 0x20,
    scan: (0x8, 0x60),
};
/// ClientInstance -> MinecraftGame*.
pub const CLIENT_GAME: Field = Field {
    name: "ClientInstance.minecraftGame",
    known: 0x1C8,
    scan: (0x100, 0x300),
};
/// MinecraftGame -> bool cursor grabbed.
pub const GAME_CURSOR_GRABBED: usize = 0x1F8;
/// Actor -> StateVectorComponent* (pos, posOld, velocity).
pub const ACTOR_STATE_VECTOR: Field = Field {
    name: "Actor.stateVector",
    known: 0x240,
    scan: (0x100, 0x400),
};
/// Actor -> ActorRotationComponent* (rotation, rotationOld; x pitch, y yaw).
pub const ACTOR_ROTATION: Field = Field {
    name: "Actor.actorRotation",
    known: 0x250,
    scan: (0x100, 0x400),
};
/// ClientInstance virtual getLocalPlayer() slot.
pub const CLIENT_GET_LOCAL_PLAYER_SLOT: usize = 0x1E;
/// Bedrock reports a player's position at eye height.
pub const PLAYER_EYE_HEIGHT: f64 = 1.62;

/// Screens that are always stacked over the world while playing: seeing them
/// does not mean the player opened a game screen.
pub const OVERLAY_SCREENS: &[&str] = &["hud_screen", "debug_screen", "toast_screen"];

/// A StateVectorComponent read (9 floats): position, previous position and
/// velocity must be finite, inside the world border range, and consistent.
pub fn plausible_state_vector(values: &[f32; 9]) -> bool {
    if !values.iter().all(|v| v.is_finite()) {
        return false;
    }
    let (pos, old, velocity) = (&values[0..3], &values[3..6], &values[6..9]);
    let inside = |v: &[f32]| v[0].abs() < 3.2e7 && v[2].abs() < 3.2e7 && v[1].abs() < 1.0e5;
    if !inside(pos) || !inside(old) || velocity.iter().any(|v| v.abs() > 100.0) {
        return false;
    }
    // A zeroed block is uninitialized storage, not a position.
    if pos.iter().chain(old).all(|v| *v == 0.0) {
        return false;
    }
    pos.iter().zip(old).all(|(a, b)| (a - b).abs() < 64.0)
}

/// An ActorRotationComponent read: pitch within ±90° (plus slack).
pub fn plausible_rotation(values: &[f32; 4]) -> bool {
    values.iter().all(|v| v.is_finite() && v.abs() < 1.0e6)
        && values[0].abs() <= 90.5
        && values[2].abs() <= 90.5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_vector_invariants() {
        let mut v = [10.5, 65.62, -3.0, 10.4, 65.62, -3.1, 0.1, 0.0, 0.1];
        assert!(plausible_state_vector(&v));
        v[0] = f32::NAN;
        assert!(!plausible_state_vector(&v));
        assert!(!plausible_state_vector(&[0.0; 9]));
        assert!(!plausible_state_vector(&[
            1.0, 2.0, 3.0, 500.0, 2.0, 3.0, 0.0, 0.0, 0.0
        ]));
    }

    #[test]
    fn rotation_invariants() {
        assert!(plausible_rotation(&[12.0, 270.0, 11.0, 268.0]));
        assert!(!plausible_rotation(&[120.0, 0.0, 0.0, 0.0]));
    }

    #[test]
    fn signature_names_are_unique() {
        let mut names: Vec<_> = SIGNATURES.iter().map(|s| s.0).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), SIGNATURES.len());
    }
}
