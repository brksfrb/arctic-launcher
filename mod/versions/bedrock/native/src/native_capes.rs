//! Java-style cape state driven only by completed ticks of the primary client
//! level. Rendering samples this state; it never advances the filter.
use crate::players::Actor;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicU64, Ordering},
};
use std::time::{Duration, Instant};
#[path = "native_wing_animation.rs"]
mod wing_animation;

/// A receipt for an owned native cape draw, separate from skin suppression.
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Replacement {
    pub cape: String,
    pub texture: String,
    pub native_cape: String,
    pub geometry: String,
    pub animation_frame: u64,
    pub mesh_draws: u32,
    pub motion: Motion,
}
/// Owned suppression for explicit recorded absence. No replacement texture or
/// synthetic cape hash is associated with this receipt.
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Hidden {
    pub native_cape: String,
    pub geometry: String,
    pub animation_frame: u64,
    pub mesh_draws: u32,
}
static SUPPRESSED: AtomicU64 = AtomicU64::new(0);
static HIDDEN: AtomicU64 = AtomicU64::new(0);
pub(crate) fn record_hidden(cape: Hidden) -> bool {
    if !crate::native_body::enabled() || !crate::native_transform::record_hidden_cape(cape) {
        return false;
    }
    HIDDEN.fetch_add(1, Ordering::Relaxed);
    true
}
pub(crate) fn record_suppressed(cape: Replacement) -> bool {
    if !crate::native_body::enabled() || !crate::native_transform::record_cape(cape) {
        return false;
    }
    SUPPRESSED.fetch_add(1, Ordering::Relaxed);
    true
}
pub(crate) fn replacement_status() -> serde_json::Value {
    serde_json::json!({"suppressed":SUPPRESSED.load(Ordering::Relaxed),"hidden":HIDDEN.load(Ordering::Relaxed),
        "nativeCapeReplacementVerified":false,"nativeCapeHidingVerified":false,"movement":crate::native_cape_movement::status(),
        "remotePhase":crate::native_cape_phase::status()})
}

#[derive(Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Motion {
    delta: [f64; 3],
    body_yaw: f32,
    walk_distance: f32,
    bob: f32,
    fall_flying_scale: f32,
}
struct Simulation {
    wings: wing_animation::Filter,
    position: [f64; 3],
    old_position: [f64; 3],
    cloak: [f64; 3],
    old_cloak: [f64; 3],
    walk: f32,
    old_walk: f32,
    bob: f32,
    old_bob: f32,
    flight_ticks: u32,
}
/// Owned endpoints of one completed native tick. Recording these preserves the
/// chasing filter's history; a new replay viewer's current filter is unrelated.
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    tick: u64,
    position: [f64; 3],
    old_position: [f64; 3],
    cloak: [f64; 3],
    old_cloak: [f64; 3],
    body_yaw: [f32; 2],
    walk: f32,
    old_walk: f32,
    bob: f32,
    old_bob: f32,
    flight_ticks: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    wing_state: Option<wing_animation::Snapshot>,
}
fn feet_position(actor: &Actor) -> Option<[f64; 3]> {
    let bounds = actor.components.as_ref()?.bounds;
    // Bedrock's state-vector Y includes its changing eye/stance offset. Java's
    // chasing cloak follows the player's feet; a camera-height change alone
    // must not produce vertical cape velocity.
    let p = [actor.position[0], bounds[1], actor.position[2]].map(f64::from);
    p.iter().all(|v| v.is_finite()).then_some(p)
}
/// Mth.length(float,float) widens each input, sums double squares, takes the
/// double square root, then narrows once. Native velocities are also f32, so
/// the same operation order supplies Vec3.horizontalDistance's bob input.
pub(crate) fn horizontal_length(x: f32, z: f32) -> f32 {
    let x = f64::from(x);
    let z = f64::from(z);
    (x * x + z * z).sqrt() as f32
}
impl Simulation {
    fn new(p: [f64; 3]) -> Self {
        Self {
            wings: wing_animation::Filter::default(),
            position: p,
            old_position: p,
            cloak: p,
            old_cloak: p,
            walk: 0.,
            old_walk: 0.,
            bob: 0.,
            old_bob: 0.,
            flight_ticks: 0,
        }
    }
    fn tick(&mut self, actor: &Actor, local: bool) -> Option<()> {
        self.tick_with_cloak_input(actor, local, None)
    }
    fn tick_with_cloak_input(
        &mut self,
        actor: &Actor,
        local: bool,
        cloak_input: Option<[f64; 3]>,
    ) -> Option<()> {
        let c = actor.components.as_ref()?;
        let p = feet_position(actor)?;
        self.old_position = self.position;
        self.position = p;
        self.old_cloak = self.cloak;
        let cloak_position = cloak_input.unwrap_or(p);
        for (i, value) in cloak_position.iter().enumerate() {
            let delta = *value - self.cloak[i];
            // ClientAvatarState.moveCloak snaps only the discontinuous axis.
            // Preserve independent follower axes and locomotion/filter state.
            if delta > 10. || delta < -10. {
                self.cloak[i] = *value;
                self.old_cloak[i] = *value;
            } else {
                self.cloak[i] += delta * 0.25;
            }
        }
        self.old_walk = self.walk;
        if local {
            let x = (p[0] - self.old_position[0]) as f32;
            let z = (p[2] - self.old_position[2]) as f32;
            self.walk += horizontal_length(x, z) * 0.6f32;
        }
        self.old_bob = self.bob;
        if flag(&c.flags, 2) {
            self.bob = 0.;
        } else {
            let target = if c.on_ground && !c.dead && !flag(&c.flags, 57) {
                horizontal_length(actor.velocity[0], actor.velocity[2]).min(0.1f32)
            } else {
                0.
            };
            self.bob += (target - self.bob) * 0.4;
        }
        self.flight_ticks = if flag(&c.flags, 32) {
            self.flight_ticks.saturating_add(1)
        } else {
            0
        };
        Some(())
    }
    fn motion(&self, actor: &Actor, partial: f32) -> Option<Motion> {
        if !partial.is_finite() || !(0.0..=1.0).contains(&partial) {
            return None;
        }
        let c = actor.components.as_ref()?;
        let delta = std::array::from_fn(|i| {
            self.old_cloak[i] + f64::from(partial) * (self.cloak[i] - self.old_cloak[i])
                - (self.old_position[i]
                    + f64::from(partial) * (self.position[i] - self.old_position[i]))
        });
        // A snapped cloak endpoint can be far from the previous actor origin.
        // Keep the bounded world-span displacement; the Java pose clamps angles.
        if delta
            .iter()
            .any(|v| !v.is_finite() || v.abs() > 64_000_000.)
        {
            return None;
        }
        // Mth.rotLerp uses signed remainder before the two wrap branches.
        // Adding/subtracting 180 first can round away a small captured turn.
        let mut turn = (c.body_yaw[0] - c.body_yaw[1]) % 360.;
        if turn >= 180. {
            turn -= 360.;
        }
        if turn < -180. {
            turn += 360.;
        }
        let body_yaw = c.body_yaw[1] + partial * turn;
        let flight = if self.flight_ticks > 0 {
            let flight = self.flight_ticks as f32 + partial;
            (flight * flight / 100.).min(1.)
        } else {
            0.
        };
        Some(Motion {
            delta,
            body_yaw,
            walk_distance: self.old_walk + partial * (self.walk - self.old_walk),
            bob: self.old_bob + partial * (self.bob - self.old_bob),
            fall_flying_scale: flight,
        })
    }
}
fn flag(flags: &[u64; 3], bit: usize) -> bool {
    flags[bit / 64] & (1u64 << (bit % 64)) != 0
}
struct Entry {
    address: usize,
    seen: Instant,
    ticked: Option<u64>,
    simulation: Simulation,
}
impl Entry {
    fn advance_with_inputs(
        &mut self,
        actor: &Actor,
        local: bool,
        completed_tick: u64,
        session: (usize, u64),
        movement: Option<&crate::native_cape_movement::Batch>,
        phases: Option<&crate::native_cape_phase::Batch>,
    ) -> Option<()> {
        let sequential = self.ticked.and_then(|tick| tick.checked_add(1)) == Some(completed_tick);
        let cohort = if local && sequential {
            movement.and_then(|value| {
                let increments = value.increments(
                    session,
                    actor,
                    completed_tick,
                    self.simulation.position,
                    feet_position(actor)?,
                )?;
                Some((increments, value.cloak_input(self.simulation.position)))
            })
        } else {
            None
        };
        let remote_input = if !local && sequential {
            phases.and_then(|value| {
                value.input(session, actor, completed_tick, self.simulation.position)
            })
        } else {
            None
        };
        let cloak_input = cohort
            .as_ref()
            .and_then(|(_, input)| *input)
            .or(remote_input);
        if cloak_input.is_some() {
            self.advance_with_cloak_input(actor, local, completed_tick, cloak_input)?;
        } else {
            self.advance(actor, local, completed_tick)?;
        }
        if let Some((increments, _)) = cohort {
            let mut walk = self.simulation.old_walk;
            for &increment in increments {
                walk += increment;
            }
            if !walk.is_finite() || walk.abs() > 1_000_000_000. {
                return None;
            }
            self.simulation.walk = walk;
            crate::native_cape_movement::applied(cloak_input.is_some());
        }
        if remote_input.is_some() {
            crate::native_cape_phase::applied();
        }
        Some(())
    }
    fn advance(&mut self, actor: &Actor, local: bool, completed_tick: u64) -> Option<()> {
        self.advance_with_cloak_input(actor, local, completed_tick, None)
    }
    fn advance_with_cloak_input(
        &mut self,
        actor: &Actor,
        local: bool,
        completed_tick: u64,
        cloak_input: Option<[f64; 3]>,
    ) -> Option<()> {
        if let Some(previous) = self.ticked {
            // Render/capture retries and a delayed callback must not integrate
            // the chasing filter twice, or move it backwards after a newer tick.
            if completed_tick <= previous {
                return Some(());
            }
            if previous.checked_add(1) != Some(completed_tick) {
                // No positions were recorded for the missing ticks. Starting
                // from today's actual feet is explicit discontinuity handling;
                // applying today's velocity repeatedly would invent history.
                self.simulation = Simulation::new(feet_position(actor)?);
            }
        }
        if cloak_input.is_some() {
            self.simulation
                .tick_with_cloak_input(actor, local, cloak_input)?;
        } else {
            self.simulation.tick(actor, local)?;
        }
        self.simulation.wings.tick(actor, completed_tick)?;
        self.ticked = Some(completed_tick);
        Some(())
    }
}
#[derive(Default)]
struct State {
    session: Option<(usize, u64)>,
    local: Option<String>,
    entries: HashMap<String, Entry>,
}
impl State {
    fn snapshot(&self, actor: &Actor, epoch: u64, completed_tick: u64) -> Option<Snapshot> {
        if self.session?.1 != epoch {
            return None;
        }
        let entry = self.entries.get(&actor.identity.id)?;
        if entry.ticked != Some(completed_tick)
            || entry.seen.elapsed() >= Duration::from_millis(500)
        {
            return None;
        }
        let s = &entry.simulation;
        Some(Snapshot {
            tick: completed_tick,
            position: s.position,
            old_position: s.old_position,
            cloak: s.cloak,
            old_cloak: s.old_cloak,
            body_yaw: actor.components.as_ref()?.body_yaw,
            walk: s.walk,
            old_walk: s.old_walk,
            bob: s.bob,
            old_bob: s.old_bob,
            flight_ticks: s.flight_ticks,
            wing_state: s.wings.snapshot(completed_tick),
        })
    }
    fn sample(
        &self,
        actor: &Actor,
        epoch: u64,
        partial: f32,
        completed_tick: u64,
    ) -> Option<Motion> {
        if self.session?.1 != epoch {
            return None;
        }
        let entry = self.entries.get(&actor.identity.id)?;
        // A paused client can keep drawing the same completed tick for an
        // arbitrary duration. Require its exact tick plus a recently observed
        // live actor, rather than expiring valid simulation by wall time.
        if entry.ticked != Some(completed_tick)
            || entry.seen.elapsed() >= Duration::from_millis(500)
        {
            return None;
        }
        entry.simulation.motion(actor, partial)
    }
}
static STATE: OnceLock<Mutex<State>> = OnceLock::new();
fn state() -> &'static Mutex<State> {
    STATE.get_or_init(Mutex::default)
}
fn sync(state: &mut State, session: (usize, u64)) {
    if state.session != Some(session) {
        state.entries.clear();
        state.local = None;
        state.session = Some(session);
    }
}
pub fn local(id: Option<&str>) {
    let Some(session) = crate::players::session() else {
        return;
    };
    if let Ok(mut state) = state().try_lock() {
        sync(&mut state, session);
        state.local = id.map(str::to_owned);
    }
}
pub fn remember(address: usize, actor: &Actor) {
    let Some(position) = feet_position(actor) else {
        return;
    };
    let Some(session) = crate::players::session() else {
        return;
    };
    if let Ok(mut state) = state().try_lock() {
        sync(&mut state, session);
        if let Some(entry) = state.entries.get_mut(&actor.identity.id) {
            if entry.address != address {
                entry.simulation = Simulation::new(position);
                entry.ticked = None;
            }
            entry.address = address;
            entry.seen = Instant::now();
            return;
        }
        if state.entries.len() >= 128 {
            return;
        }
        state.entries.insert(
            actor.identity.id.clone(),
            Entry {
                address,
                seen: Instant::now(),
                ticked: None,
                simulation: Simulation::new(position),
            },
        );
    }
}
pub fn tick(level: usize, completed_tick: u64) {
    tick_with_inputs(level, completed_tick, None, None);
}
pub(crate) fn remote_phase_targets(
    session: (usize, u64),
    previous: u64,
) -> Option<Vec<(String, usize)>> {
    let state = state().try_lock().ok()?;
    if state.session != Some(session) {
        return None;
    }
    let local = state.local.as_ref()?;
    Some(
        state
            .entries
            .iter()
            .filter(|(id, entry)| {
                *id != local
                    && entry.ticked == Some(previous)
                    && entry.seen.elapsed() < Duration::from_millis(500)
            })
            .map(|(id, entry)| (id.clone(), entry.address))
            .collect(),
    )
}
pub(crate) fn movement_target(session: (usize, u64), previous: u64) -> Option<(String, usize)> {
    let state = state().try_lock().ok()?;
    if state.session != Some(session) {
        return None;
    }
    let id = state.local.as_ref()?;
    let entry = state.entries.get(id)?;
    (entry.ticked == Some(previous) && entry.seen.elapsed() < Duration::from_millis(500))
        .then(|| (id.clone(), entry.address))
}
pub(crate) fn tick_with_inputs(
    level: usize,
    completed_tick: u64,
    movement: Option<&crate::native_cape_movement::Batch>,
    phases: Option<&crate::native_cape_phase::Batch>,
) {
    let Some(session) = crate::players::session().filter(|s| s.0 == level) else {
        return;
    };
    let candidates = if let Ok(mut state) = state().try_lock() {
        sync(&mut state, session);
        state
            .entries
            .retain(|_, entry| entry.seen.elapsed() < Duration::from_secs(30));
        state
            .entries
            .iter()
            .map(|(id, entry)| (id.clone(), entry.address))
            .collect::<Vec<_>>()
    } else {
        return;
    };
    // Read actors outside the simulation lock. Revalidate native type, UUID,
    // current level and entity generation on each tick; a remembered address
    // alone never authorizes using a destroyed/reused native entity.
    for (id, address) in candidates {
        let actor = crate::players::actor(address).filter(|a| a.identity.id == id);
        let Ok(mut state) = state().try_lock() else {
            continue;
        };
        if state.session != Some(session) {
            return;
        }
        let local = state.local.as_ref() == Some(&id);
        let Some(entry) = state.entries.get_mut(&id).filter(|e| e.address == address) else {
            continue;
        };
        if actor
            .as_ref()
            .and_then(|actor| {
                entry.advance_with_inputs(actor, local, completed_tick, session, movement, phases)
            })
            .is_none()
        {
            state.entries.remove(&id);
        }
    }
}
pub fn sample(actor: &Actor, epoch: u64, partial: f32, completed_tick: u64) -> Option<Motion> {
    let state = state().try_lock().ok()?;
    state.sample(actor, epoch, partial, completed_tick)
}
pub fn snapshot(actor: &Actor, epoch: u64, completed_tick: u64) -> Option<Snapshot> {
    state()
        .try_lock()
        .ok()?
        .snapshot(actor, epoch, completed_tick)
}
/// Resolve the completed client tick without advancing the cape simulation.
/// A recent render/readiness flag alone does not authorize suppressing a cape.
pub(crate) fn sample_current(actor: &Actor, partial: f32) -> Option<Motion> {
    let (level, epoch) = crate::players::session()?;
    let tick = crate::world_hooks::client_completed_tick(level)?;
    sample(actor, epoch, partial, tick)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paused_ticks_keep_cape_state_but_new_ticks_sessions_and_absent_actors_are_rejected() {
        let actor = crate::live_frame::tests::fixture_actor();
        let mut state = State {
            session: Some((123, 7)),
            ..State::default()
        };
        state.entries.insert(
            actor.identity.id.clone(),
            Entry {
                address: 1234,
                seen: Instant::now(),
                ticked: Some(42),
                simulation: Simulation::new(feet_position(&actor).unwrap()),
            },
        );
        let first = state.sample(&actor, 7, 0.25, 42).unwrap();
        let entry = state.entries.get_mut(&actor.identity.id).unwrap();
        entry.seen = Instant::now() - Duration::from_secs(60);
        assert!(state.sample(&actor, 7, 0.25, 42).is_none());
        state.entries.get_mut(&actor.identity.id).unwrap().seen = Instant::now();
        let still_paused = state.sample(&actor, 7, 0.25, 42).unwrap();
        assert_eq!(first.delta, still_paused.delta);
        assert_eq!(first.bob, still_paused.bob);
        assert!(state.sample(&actor, 7, 0.25, 43).is_none());
        assert!(state.sample(&actor, 8, 0.25, 42).is_none());
    }
    #[test]
    fn tick_filter_matches_java_displacement_and_render_sampling_never_advances_it() {
        let mut a = crate::live_frame::tests::fixture_actor();
        a.position = [0.; 3];
        a.old_position = [0.; 3];
        a.velocity = [0., 0., 1.];
        a.components.as_mut().unwrap().bounds[1] = 0.;
        let mut sim = Simulation::new([0.; 3]);
        a.position[2] = 1.;
        sim.tick(&a, true).unwrap();
        assert_eq!(sim.motion(&a, 1.).unwrap().delta, [0., 0., -0.75]);
        assert_eq!(sim.motion(&a, 0.5).unwrap().delta, [0., 0., -0.375]);
        assert_eq!(
            sim.motion(&a, 0.5).unwrap().delta,
            sim.motion(&a, 0.5).unwrap().delta
        );
        assert!((sim.motion(&a, 1.).unwrap().bob - 0.04).abs() < 1e-7);
        assert_eq!(sim.motion(&a, 1.).unwrap().walk_distance, 0.6);
        sim.tick(&a, true).unwrap();
        assert_eq!(sim.motion(&a, 1.).unwrap().delta[2], -0.5625);
        a.position[0] = 100.;
        sim.tick(&a, true).unwrap();
        // X snaps its cloak endpoints while the independent Z follower keeps
        // advancing. Actor position endpoints still interpolate across the jump.
        assert_eq!(sim.motion(&a, 0.).unwrap().delta, [100., 0., -0.5625]);
        assert_eq!(sim.motion(&a, 0.5).unwrap().delta, [50., 0., -0.4921875]);
        assert_eq!(sim.motion(&a, 1.).unwrap().delta, [0., 0., -0.421875]);
    }
    #[test]
    fn eye_height_changes_do_not_become_cloak_motion_but_feet_motion_does() {
        let mut actor = crate::live_frame::tests::fixture_actor();
        let feet = feet_position(&actor).unwrap();
        let mut sim = Simulation::new(feet);
        actor.position[1] -= 0.12;
        actor.components.as_mut().unwrap().bounds[4] -= 0.3;
        sim.tick(&actor, true).unwrap();
        assert_eq!(sim.motion(&actor, 1.).unwrap().delta, [0.; 3]);
        actor.components.as_mut().unwrap().bounds[1] += 1.;
        actor.position[1] += 1.;
        sim.tick(&actor, true).unwrap();
        assert_eq!(sim.motion(&actor, 1.).unwrap().delta, [0., -0.75, 0.]);
        assert_eq!(sim.motion(&actor, 0.5).unwrap().delta, [0., -0.375, 0.]);
    }
}
