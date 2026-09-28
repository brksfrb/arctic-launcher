//! Where a voice sits: louder when close, silent past the range, and
//! panned left or right by where the speaker is relative to where you look.

/// Full volume this close (blocks).
pub const NEAR: f64 = 4.0;
/// Silent this far (blocks), like Simple Voice Chat's default range.
pub const DEFAULT_RANGE: f64 = 48.0;

/// A position and, for the listener, which way they look (yaw in degrees,
/// Minecraft's: 0 = south (+z), 90 = west (-x)).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Place {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f64,
}

/// Left and right gains (0..1) for a speaker heard by a listener.
pub fn gains(listener: Place, speaker: Place, range: f64) -> (f32, f32) {
    let (dx, dy, dz) = (
        speaker.x - listener.x,
        speaker.y - listener.y,
        speaker.z - listener.z,
    );
    let distance = (dx * dx + dy * dy + dz * dz).sqrt();
    let volume = if distance <= NEAR {
        1.0
    } else if distance >= range {
        0.0
    } else {
        // A smooth fade: gentle near, quicker towards the edge.
        let t = (distance - NEAR) / (range - NEAR);
        (1.0 - t).powf(1.6)
    };
    if volume == 0.0 {
        return (0.0, 0.0);
    }
    // The listener's right-hand direction (Minecraft: yaw 0 looks at +z,
    // so right is -x).
    let yaw = listener.yaw.to_radians();
    let (right_x, right_z) = (-yaw.cos(), -yaw.sin());
    let horizontal = (dx * dx + dz * dz).sqrt();
    // -1 fully left .. 1 fully right; straight above/below is centred.
    let pan = if horizontal < 1e-6 {
        0.0
    } else {
        (dx * right_x + dz * right_z) / horizontal
    };
    // Constant-power pan, never fully silent on one side (sounds natural
    // on headphones).
    let angle = (pan * 0.8 + 1.0) * std::f64::consts::FRAC_PI_4;
    (
        (volume * angle.cos() * std::f64::consts::SQRT_2) as f32 * 0.707,
        (volume * angle.sin() * std::f64::consts::SQRT_2) as f32 * 0.707,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: f64, z: f64) -> Place {
        Place {
            x,
            y: 64.0,
            z,
            yaw: 0.0,
        }
    }

    #[test]
    fn louder_near_silent_far() {
        let me = at(0.0, 0.0);
        let (l, r) = gains(me, at(0.0, 2.0), DEFAULT_RANGE);
        assert!((l - r).abs() < 1e-3 && l > 0.6, "{l} {r}");
        let (l2, _) = gains(me, at(0.0, 30.0), DEFAULT_RANGE);
        assert!(l2 < l && l2 > 0.0);
        assert_eq!(gains(me, at(0.0, 60.0), DEFAULT_RANGE), (0.0, 0.0));
    }

    #[test]
    fn pans_by_direction() {
        // Looking south (+z): right-hand side is -x (west).
        let me = at(0.0, 0.0);
        let (l, r) = gains(me, at(-10.0, 0.0), DEFAULT_RANGE);
        assert!(r > l * 2.0, "west should be on the right: {l} {r}");
        let (l, r) = gains(me, at(10.0, 0.0), DEFAULT_RANGE);
        assert!(l > r * 2.0, "east should be on the left: {l} {r}");
        // Turned around (yaw 180, looking north), east is on the right.
        let back = Place { yaw: 180.0, ..me };
        let (l, r) = gains(back, at(10.0, 0.0), DEFAULT_RANGE);
        assert!(r > l * 2.0);
    }
}
