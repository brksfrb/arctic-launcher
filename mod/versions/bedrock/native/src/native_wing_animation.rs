//! Java 26.2 ElytraAnimationState, driven by the existing completed-tick owner.
//! This is an independent animation filter, not a native animation receipt.
use crate::players::Actor;
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Snapshot {
    tick: u64,
    first_tick: u64,
    previous: [f32; 3],
    current: [f32; 3],
    crouching: bool,
}

#[derive(Clone, Default)]
pub(super) struct Filter {
    previous: [f32; 3],
    current: [f32; 3],
    crouching: bool,
    first_tick: Option<u64>,
    tick: Option<u64>,
}
impl Filter {
    pub(super) fn tick(&mut self, actor: &Actor, completed_tick: u64) -> Option<()> {
        let components = actor.components.as_ref()?;
        if components.id != actor.identity.id || actor.velocity.iter().any(|v| !v.is_finite()) {
            return None;
        }
        if self
            .tick
            .is_some_and(|previous| previous.checked_add(1) != Some(completed_tick))
        {
            return None;
        }
        let gliding = components.flags[0] & (1u64 << 32) != 0;
        let crouching = components.flags[0] & (1u64 << 1) != 0;
        let target = if gliding {
            let mut phase = 1f32;
            if actor.velocity[1] < 0. {
                // Vec3.normalize uses double left-associated squared length,
                // with ZERO below the exact double value of Java's 1e-5f.
                let [x, y, z] = actor.velocity.map(f64::from);
                let length = ((x * x + y * y) + z * z).sqrt();
                let normal_y = if length < f64::from(1e-5f32) {
                    0.
                } else {
                    y / length
                };
                phase = 1f32 - (-normal_y).powf(1.5) as f32;
            }
            [
                lerp(phase, 0.2617994f32, 0.34906584f32),
                0.,
                lerp(phase, -0.2617994f32, -1.5707964f32),
            ]
        } else if crouching {
            [0.6981317f32, 0.08726646f32, -std::f32::consts::FRAC_PI_4]
        } else {
            [0.2617994f32, 0., -0.2617994f32]
        };
        self.previous = self.current;
        for (current, target) in self.current.iter_mut().zip(target) {
            *current += (target - *current) * 0.3f32;
        }
        self.crouching = crouching;
        self.first_tick.get_or_insert(completed_tick);
        self.tick = Some(completed_tick);
        Some(())
    }
    pub(super) fn snapshot(&self, completed_tick: u64) -> Option<Snapshot> {
        if self.tick != Some(completed_tick) {
            return None;
        }
        Some(Snapshot {
            tick: completed_tick,
            first_tick: self.first_tick?,
            previous: self.previous,
            current: self.current,
            crouching: self.crouching,
        })
    }
}
fn lerp(phase: f32, a: f32, b: f32) -> f32 {
    a + phase * (b - a)
}
