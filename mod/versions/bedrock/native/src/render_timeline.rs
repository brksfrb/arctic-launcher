//! Replay deltas mapped onto the renderer's existing scaled and wall-time epochs.
use serde::Serialize;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Timeline {
    pub replay_origin: f64,
    pub native_origin: [f32; 4],
    pub scaled_origin: f64,
    pub scaled_origin_replay: f64,
    pub scale: f32,
    pub last_replay: f64,
    pub value: [f32; 4],
}
fn valid_time(value: f64) -> bool {
    value.is_finite() && (0. ..=1_000_000_000.).contains(&value)
}
impl Timeline {
    pub fn start(replay: f64, native: [f32; 4], scale: f32) -> Option<Self> {
        if !valid_time(replay)
            || !valid_time(native[0] as f64)
            || !valid_time(native[3] as f64)
            || native.iter().any(|v| !v.is_finite())
            || !scale.is_finite()
            || !(0. ..=1000.).contains(&scale)
        {
            return None;
        }
        Some(Self {
            replay_origin: replay,
            native_origin: native,
            scaled_origin: native[0] as f64,
            scaled_origin_replay: replay,
            scale,
            last_replay: replay,
            value: [native[0], 0., 0., native[3]],
        })
    }
    pub fn sample(&self, replay: f64, scale: f32) -> Option<Self> {
        let delta = replay - self.last_replay;
        if !valid_time(replay)
            || !(0. ..=1000.).contains(&delta)
            || !scale.is_finite()
            || !(0. ..=1000.).contains(&scale)
        {
            return None;
        }
        let mut next = self.clone();
        if delta == 0. && scale.to_bits() == self.scale.to_bits() {
            next.value[1] = 0.;
            next.value[2] = 0.;
            return Some(next);
        }
        if scale.to_bits() != self.scale.to_bits() {
            // Preserve the established phase when native time scaling changes.
            next.scaled_origin = self.value[0] as f64;
            next.scaled_origin_replay = self.last_replay;
            next.scale = scale;
        }
        let scaled = next.scaled_origin + (replay - next.scaled_origin_replay) * f64::from(scale);
        let wall = self.native_origin[3] as f64 + (replay - self.replay_origin);
        if !valid_time(scaled) || !valid_time(wall) {
            return None;
        }
        let frame_delta = (delta * f64::from(scale)) as f32;
        next.last_replay = replay;
        next.value = [
            scaled as f32,
            frame_delta,
            frame_delta * frame_delta,
            wall as f32,
        ];
        next.value
            .iter()
            .all(|v| v.is_finite() && v.abs() <= 1_000_000_000.)
            .then_some(next)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn freeze_preserves_distinct_native_epochs_and_steps_by_replay_deltas() {
        let start = Timeline::start(22., [262.25, 0.03, 0.0009, 267.75], 1.).unwrap();
        assert_eq!(start.value, [262.25, 0., 0., 267.75]);
        let step = start.sample(22.5, 1.).unwrap();
        assert_eq!(step.value, [262.75, 0.5, 0.25, 268.25]);
        let held = step.sample(22.5, 1.).unwrap();
        assert_eq!(held.value, [262.75, 0., 0., 268.25]);
        let via = held.sample(23., 1.).unwrap();
        let direct = start.sample(23., 1.).unwrap();
        assert_eq!(via.value[0], direct.value[0]);
        assert_eq!(via.value[3], direct.value[3]);
        assert!(held.sample(22., 1.).is_none());
        assert!(held.sample(2000., 1.).is_none());
        assert!(Timeline::start(f64::NAN, [0.; 4], 1.).is_none());
    }
    #[test]
    fn native_scale_changes_rebase_scaled_phase_without_changing_wall_epoch() {
        let start = Timeline::start(1., [100., 0., 0., 200.], 0.5).unwrap();
        let step = start.sample(2., 0.5).unwrap();
        assert_eq!(step.value, [100.5, 0.5, 0.25, 201.]);
        let changed = step.sample(3., 2.).unwrap();
        assert_eq!(changed.value, [102.5, 2., 4., 202.]);
        let paused = changed.sample(4., 0.).unwrap();
        assert_eq!(paused.value, [102.5, 0., 0., 203.]);
        assert!(paused.sample(4., -1.).is_none());
    }
}
