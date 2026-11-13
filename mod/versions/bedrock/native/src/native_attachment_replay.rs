//! Immutable replay-palette plans prepared on the Java client thread. Native
//! callbacks only match copied identities/math; no archive or UI calls occur.
use crate::{
    native_attachment_samples::{Frame, Palette},
    native_emote_history::Replay,
    native_palette_binding::Named,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
const MAX_INPUT: usize = 8 * 1024 * 1024;
const MAX_BYTES: usize = 16 * 1024 * 1024;
const LEASE: Duration = Duration::from_millis(250);
static STATE: Mutex<State> = Mutex::new(State {
    owner: None,
    ready: None,
    error: None,
    unknown: false,
});
static LAST: Mutex<Option<Value>> = Mutex::new(None);
static COPIES: AtomicU64 = AtomicU64::new(0);
static COLOR_COPIES: AtomicU64 = AtomicU64::new(0);
static WING_COLOR_COPIES: AtomicU64 = AtomicU64::new(0);
static REVISION: AtomicU64 = AtomicU64::new(1);
pub(crate) fn revision() -> u64 {
    REVISION.load(Ordering::Acquire)
}
fn changed() {
    let _ = REVISION.try_update(Ordering::AcqRel, Ordering::Acquire, |old| {
        Some(if old == 0 {
            0
        } else {
            old.checked_add(1).unwrap_or(0)
        })
    });
}
struct State {
    owner: Option<String>,
    ready: Option<Arc<Ready>>,
    error: Option<String>,
    unknown: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Plan {
    schema: u32,
    active: bool,
    #[serde(default)]
    session: String,
    #[serde(default)]
    manifest_hash: String,
    #[serde(default)]
    ready: bool,
    #[serde(default)]
    time_ms: u32,
    #[serde(default)]
    time_remainder: u32,
    #[serde(default = "one")]
    time_denominator: u32,
    #[serde(default)]
    native_tick: u64,
    #[serde(default)]
    partial: f32,
    #[serde(default)]
    camera_origin: Option<[f32; 3]>,
    #[serde(default)]
    marker: Option<Marker>,
    #[serde(default)]
    actors: Vec<Actor>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Actor {
    id: String,
    samples: Frame,
    #[serde(default)]
    root: Option<[f32; 12]>,
    #[serde(default)]
    cape_colors: Vec<CapeColor>,
    #[serde(default)]
    wing_colors: Vec<CapeColor>,
}
fn cape_slot() -> u32 {
    crate::native_attachment_samples::CAPE_SLOT
}
#[derive(Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CapeColor {
    geometry: String,
    #[serde(default = "cape_slot")]
    slot: u32,
    #[serde(default)]
    native_cape: Option<String>,
    rgba: [f32; 4],
    #[serde(default)]
    dithering_enabled: Option<bool>,
    #[serde(default)]
    shader_inputs: Option<crate::native_shader_inputs::ActorInputs>,
}
impl CapeColor {
    fn valid(&self, samples: &Frame) -> bool {
        [
            crate::native_attachment_samples::CAPE_SLOT,
            crate::native_attachment_samples::WING_SLOT,
        ]
        .contains(&self.slot)
            && self
                .rgba
                .iter()
                .all(|v| v.is_finite() && (0. ..=1.).contains(v))
            && self
                .shader_inputs
                .as_ref()
                .is_none_or(|inputs| inputs.valid())
            && samples.palettes.iter().any(|p| {
                p.slot == self.slot
                    && p.geometry == self.geometry
                    && p.native_cape == self.native_cape
            })
    }
    fn bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.geometry.capacity()
            + self.native_cape.as_ref().map_or(0, String::capacity)
            + self
                .shader_inputs
                .as_ref()
                .and_then(|value| value.material_paths.as_ref())
                .map_or(0, |value| {
                    value.sha256.capacity()
                        + value.variation_sha256.as_ref().map_or(0, String::capacity)
                })
    }
}
/// Owned copied-description provenance. It is not part of an appearance archive.
#[derive(Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CapeColorReceipt {
    pub geometry: String,
    #[serde(default = "cape_slot")]
    pub slot: u32,
    #[serde(default)]
    pub native_cape: Option<String>,
    pub animation_frame: u64,
    pub source_rgba: [f32; 4],
    pub rgba: [f32; 4],
    pub replay: Replay,
    #[serde(default)]
    pub source_dithering: Option<bool>,
    #[serde(default)]
    pub dithering_enabled: Option<bool>,
    #[serde(default)]
    pub shader_inputs: Option<crate::native_shader_inputs::ActorInputs>,
    #[serde(default)]
    pub source_shader_inputs: Option<crate::native_shader_inputs::ActorInputs>,
    #[serde(default)]
    pub shader_input_draws: u32,
    #[serde(default)]
    pub shader_input_copies: u32,
    #[serde(default)]
    pub shader_inputs_applied: bool,
}
fn actor_bytes(entry: &Actor) -> usize {
    entry.samples.bytes()
        + 2 * entry.id.capacity()
        + 256
        + (entry.cape_colors.capacity() + entry.wing_colors.capacity())
            * std::mem::size_of::<CapeColor>()
        + entry
            .cape_colors
            .iter()
            .chain(&entry.wing_colors)
            .map(CapeColor::bytes)
            .sum::<usize>()
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Marker {
    sequence: u64,
    request_id: String,
    marker: String,
    generation: u64,
    replay_sequence: u64,
    observation_sequence: u64,
}
fn one() -> u32 {
    1
}
fn token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
fn actor(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
            }
        })
}
impl Marker {
    fn valid(&self) -> bool {
        [self.sequence, self.generation, self.replay_sequence]
            .iter()
            .all(|n| (1..=1_000_000_000).contains(n))
            && token(&self.request_id)
            && self.marker.len() == 16
            && self.marker.starts_with("37c1")
            && self
                .marker
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            && self.observation_sequence > 0
    }
    fn matches(&self, value: &Value) -> bool {
        value["sequence"].as_u64() == Some(self.sequence)
            && value["requestId"].as_str() == Some(self.request_id.as_str())
            && value["marker"].as_str() == Some(self.marker.as_str())
            && value["generation"].as_u64() == Some(self.generation)
            && value["replaySequence"].as_u64() == Some(self.replay_sequence)
            && value["observationSequence"].as_u64() == Some(self.observation_sequence)
    }
}
struct Ready {
    at: Instant,
    session: String,
    identity: Replay,
    marker: Marker,
    native_tick: u64,
    partial: f32,
    camera_origin: [f32; 3],
    actors: HashMap<String, Actor>,
    bytes: usize,
}
impl Ready {
    fn clock(&self, timing: &Value) -> bool {
        let clock = &timing["captureControls"]["clock"];
        let packet = &timing["captureControls"]["packetDispatch"];
        let Some(tick) = timing["ticks"].as_u64().filter(|n| *n <= 1_000_000_000_000) else {
            return false;
        };
        let Some(partial) = timing["partial"]
            .as_f64()
            .filter(|n| n.is_finite() && (0. ..1.).contains(n))
        else {
            return false;
        };
        tick == self.native_tick
            && (partial as f32).to_bits() == self.partial.to_bits()
            && self.marker.matches(packet)
            && packet["session"].as_str() == Some(self.session.as_str())
            && packet["completion"].as_str() == Some("dispatch")
            && packet["stage"].as_str() == Some("dispatch_returned")
            && packet["dispatcherReturned"].as_bool() == Some(true)
            && packet["primaryClientThread"].as_bool() == Some(true)
            && packet["postClientTickCompleted"].as_bool() == Some(false)
            && packet["error"].is_null()
            && packet["dispatcherTick"].as_u64() == Some(tick)
            && clock["ticks"].as_u64() == Some(tick)
            && clock["realTimerControlled"].as_bool() == Some(true)
            && clock["sequence"]
                .as_u64()
                .is_some_and(|n| (1..=1_000_000_000).contains(&n))
            && clock["freezeSequence"]
                .as_u64()
                .is_some_and(|n| (1..=1_000_000_000).contains(&n))
            && clock["partial"].as_f64().is_some_and(|n| {
                n.is_finite() && (n as f32).to_bits() == (partial as f32).to_bits()
            })
            && packet["timeMs"].as_u64() == Some(u64::from(self.identity.time_ms))
            && packet["timeRemainder"]
                .as_u64()
                .or_else(|| packet.get("timeRemainder").is_none().then_some(0))
                == Some(u64::from(self.identity.time_remainder))
            && packet["timeDenominator"]
                .as_u64()
                .or_else(|| packet.get("timeDenominator").is_none().then_some(1))
                == Some(u64::from(self.identity.time_denominator))
    }
}
pub(crate) fn invalidate() {
    if let Ok(mut state) = STATE.lock() {
        changed();
        state.ready = None;
        state.unknown = true;
    }
}
pub(crate) fn active() -> bool {
    STATE.try_lock().map_or(true, |state| {
        state.owner.is_some() || state.unknown || revision() == 0
    })
}
pub(crate) fn update(input: Option<&str>) {
    let parsed = (|| -> Result<Plan, String> {
        let input = input.ok_or("attachment replay plan unavailable")?;
        if input.len() > MAX_INPUT {
            return Err("attachment replay plan exceeds 8 MiB".into());
        }
        let plan: Plan = serde_json::from_str(input).map_err(|e| e.to_string())?;
        if plan.schema != 1 {
            return Err("invalid attachment replay schema".into());
        }
        if !plan.active {
            if plan.ready || !plan.actors.is_empty() {
                return Err("inactive attachment replay has data".into());
            }
            return Ok(plan);
        }
        let identity = Replay {
            manifest_hash: plan.manifest_hash.clone(),
            time_ms: plan.time_ms,
            time_remainder: plan.time_remainder,
            time_denominator: plan.time_denominator,
        };
        if !token(&plan.session)
            || !identity.valid()
            || plan.actors.len() > 128
            || plan.ready && !plan.marker.as_ref().is_some_and(Marker::valid)
            || plan.native_tick > 1_000_000_000_000
            || !plan.partial.is_finite()
            || !(0. ..1.).contains(&plan.partial)
            || plan.ready
                && !plan.camera_origin.as_ref().is_some_and(|origin| {
                    origin
                        .iter()
                        .all(|v| v.is_finite() && v.abs() <= 32_000_000.)
                })
            || !plan.ready && !plan.actors.is_empty()
        {
            return Err("invalid attachment replay identity/clock".into());
        }
        let mut seen = std::collections::HashSet::new();
        let mut bytes = 1024;
        for entry in &plan.actors {
            let mut colors = std::collections::HashSet::new();
            if !actor(&entry.id)
                || !seen.insert(entry.id.as_str())
                || !entry.samples.complete
                || !entry.samples.valid()
                || entry.root.as_ref().is_some_and(|root| {
                    root.iter().any(|v| !v.is_finite() || v.abs() > 32_000_000.)
                })
                || entry.cape_colors.len() > 16
                || entry.cape_colors.iter().any(|color| {
                    !color.valid(&entry.samples)
                        || color.slot != crate::native_attachment_samples::CAPE_SLOT
                        || !colors.insert((color.slot, &color.geometry, &color.native_cape))
                })
                || entry.wing_colors.len() > 16
                || entry.wing_colors.iter().any(|color| {
                    !color.valid(&entry.samples)
                        || color.slot != crate::native_attachment_samples::WING_SLOT
                        || !colors.insert((color.slot, &color.geometry, &color.native_cape))
                })
            {
                return Err("invalid attachment replay actor/palette".into());
            }
            bytes += actor_bytes(entry);
            if bytes > MAX_BYTES {
                return Err("attachment replay plan exceeds retained data budget".into());
            }
        }
        Ok(plan)
    })();
    let Ok(mut state) = STATE.lock() else {
        return;
    };
    changed();
    match parsed {
        Err(error) => {
            state.ready = None;
            state.unknown = true;
            state.error = Some(error.chars().take(512).collect());
        }
        Ok(plan) if !plan.active => {
            state.owner = None;
            state.ready = None;
            state.error = None;
            state.unknown = false;
        }
        Ok(plan) => {
            state.owner = Some(plan.session.clone());
            state.ready = None;
            state.error = None;
            state.unknown = false;
            if plan.ready {
                let (Some(marker), Some(camera_origin)) = (plan.marker, plan.camera_origin) else {
                    return;
                };
                let bytes = 1024 + plan.actors.iter().map(actor_bytes).sum::<usize>();
                state.ready = Some(Arc::new(Ready {
                    at: Instant::now(),
                    session: plan.session,
                    identity: Replay {
                        manifest_hash: plan.manifest_hash,
                        time_ms: plan.time_ms,
                        time_remainder: plan.time_remainder,
                        time_denominator: plan.time_denominator,
                    },
                    marker,
                    native_tick: plan.native_tick,
                    partial: plan.partial,
                    camera_origin,
                    actors: plan
                        .actors
                        .into_iter()
                        .map(|entry| (entry.id.clone(), entry))
                        .collect(),
                    bytes,
                }));
            }
        }
    }
}
pub(crate) enum Query {
    Inactive,
    Blocked,
    Ready(Target),
}
pub(crate) struct Target {
    ready: Arc<Ready>,
    timing: Value,
    palette: Palette,
    actor: String,
    scope: u64,
    draw: Option<crate::native_transform::Draw>,
    pub matrices: Vec<[f32; 16]>,
}
fn current(
    ready: &Arc<Ready>,
    timing: &Value,
    scope: u64,
    actor: &str,
    draw: Option<&crate::native_transform::Draw>,
) -> bool {
    if ready.at.elapsed() > LEASE || revision() == 0 {
        return false;
    }
    let Some((session, now)) = crate::world_hooks::attachment_timing() else {
        return false;
    };
    if session != ready.session || now != *timing {
        return false;
    }
    let Some(origin) = crate::world_hooks::attachment_origin() else {
        return false;
    };
    if !origin
        .iter()
        .zip(ready.camera_origin)
        .all(|(a, b)| a.to_bits() == b.to_bits())
    {
        return false;
    }
    if let Some(draw) = draw {
        let Some(actual) = crate::native_transform::current_draw(scope, actor) else {
            return false;
        };
        if actual.matrix != draw.matrix
            || actual.view != draw.view
            || actual.projection != draw.projection
        {
            return false;
        }
    }
    STATE.try_lock().is_ok_and(|state| {
        !state.unknown
            && state.owner.as_deref() == Some(session.as_str())
            && state
                .ready
                .as_ref()
                .is_some_and(|live| Arc::ptr_eq(live, ready))
    })
}
impl Target {
    pub(crate) fn identity(&self) -> Replay {
        self.ready.identity.clone()
    }
    pub(crate) fn current(&self) -> bool {
        current(
            &self.ready,
            &self.timing,
            self.scope,
            &self.actor,
            self.draw.as_ref(),
        )
    }
    pub(crate) fn committed(&self, frame: u64) {
        COPIES.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut last) = LAST.try_lock() {
            *last = Some(
                json!({"actorId":self.actor,"manifestHash":self.ready.identity.manifest_hash,
            "timeMs":self.ready.identity.time_ms,"timeRemainder":self.ready.identity.time_remainder,"timeDenominator":self.ready.identity.time_denominator,
            "animationFrame":frame,"geometry":self.palette.geometry,"definition":self.palette.definition,"slot":self.palette.slot,
            "hierarchy":self.palette.hierarchy,"group":self.palette.group,"owner":self.palette.owner,"matrices":self.matrices.len(),
            "recordedModelRootApplied":self.draw.is_some(),
            "ownedCpuOutputCopied":true,"nativeEquipmentRenderingVerified":false,"deterministicVideoVerified":false}),
            );
        }
    }
}
/// Borrowed span headers last only through the enclosing synchronous native
/// submission. A prepared local material copy rechecks them before/after use;
/// they never enter a replay plan, archive or published receipt.
#[derive(Clone)]
pub(crate) struct ColorTarget {
    ready: Arc<Ready>,
    timing: Value,
    tag: crate::native_submit::Tag,
    frame: u64,
    draw: crate::native_transform::Draw,
    named: Named,
    sources: [usize; 2],
    color: CapeColor,
}
impl ColorTarget {
    pub(crate) fn rgba(&self) -> [f32; 4] {
        self.color.rgba
    }
    pub(crate) fn dithering(&self) -> Option<bool> {
        self.color.dithering_enabled
    }
    pub(crate) fn shader_inputs(&self) -> Option<crate::native_shader_inputs::ActorInputs> {
        self.color.shader_inputs.clone()
    }
    pub(crate) fn replay(&self) -> Replay {
        self.ready.identity.clone()
    }
    // A known archived cohort can differ from the live source only when the
    // separately pinned local material-copy hook is available. No global edit.
    pub(crate) fn can_prepare_shader_inputs(
        &self,
        inputs: Option<&crate::native_shader_inputs::ActorInputs>,
    ) -> bool {
        self.color.shader_inputs.as_ref().is_none_or(|wanted| {
            crate::native_shader_consumer::installed()
                && inputs.is_some_and(|actual| {
                    actual.valid()
                        && (wanted.material_context.is_none() || actual.material_context.is_some())
                        && wanted.material_paths.as_ref().is_none_or(|paths| {
                            actual
                                .material_paths
                                .as_ref()
                                .is_some_and(|actual| paths.matches(actual))
                        })
                })
        })
    }
    pub(crate) fn current(&self) -> bool {
        crate::native_body::enabled()
            && current(
                &self.ready,
                &self.timing,
                self.tag.scope,
                &self.tag.actor,
                Some(&self.draw),
            )
            && crate::native_transform::palette_writer_scope().is_some_and(|stamp| {
                stamp.scope == self.tag.scope
                    && stamp.actor_id == self.tag.actor
                    && stamp.frame == self.frame
            })
            && crate::native_palette_binding::current(self.sources, &self.tag, self.frame).as_ref()
                == Some(&self.named)
    }
    pub(crate) fn applied(
        &self,
        source_rgba: [f32; 4],
        source_dithering: Option<bool>,
        dithering_enabled: Option<bool>,
        shader_inputs: Option<crate::native_shader_inputs::ActorInputs>,
    ) -> bool {
        if !self.current()
            || self
                .color
                .dithering_enabled
                .is_some_and(|wanted| dithering_enabled != Some(wanted))
            || !self.can_prepare_shader_inputs(shader_inputs.as_ref())
        {
            return false;
        }
        let wanted = self.color.shader_inputs.clone();
        let receipt = CapeColorReceipt {
            geometry: self.color.geometry.clone(),
            native_cape: self.color.native_cape.clone(),
            slot: self.color.slot,
            animation_frame: self.frame,
            source_rgba,
            rgba: self.color.rgba,
            replay: self.ready.identity.clone(),
            source_dithering,
            dithering_enabled,
            source_shader_inputs: shader_inputs.clone(),
            shader_inputs: wanted.clone().or(shader_inputs),
            shader_input_draws: u32::from(wanted.is_some()),
            shader_input_copies: 0,
            shader_inputs_applied: false,
        };
        if !crate::native_transform::record_replay_layer_color(
            self.tag.scope,
            &self.tag.actor,
            receipt,
        ) {
            return false;
        }
        if self.color.slot == crate::native_attachment_samples::WING_SLOT {
            WING_COLOR_COPIES.fetch_add(1, Ordering::Relaxed);
        } else {
            COLOR_COPIES.fetch_add(1, Ordering::Relaxed);
        }
        true
    }
}
pub(crate) fn layer_color(
    tag: &crate::native_submit::Tag,
    frame: u64,
    sources: [usize; 2],
) -> Option<ColorTarget> {
    if tag.replacement.is_some() || revision() == 0 {
        return None;
    }
    let (animation_frame, slot, key) = match &tag.role {
        crate::native_submit::MeshRole::Cape {
            animation_frame,
            native_cape: Some(key),
        } => (
            animation_frame,
            crate::native_attachment_samples::CAPE_SLOT,
            Some(key),
        ),
        crate::native_submit::MeshRole::Elytra { animation_frame }
            if tag.geometry == "geometry.elytra" =>
        {
            (
                animation_frame,
                crate::native_attachment_samples::WING_SLOT,
                None,
            )
        }
        _ => return None,
    };
    if *animation_frame != frame {
        return None;
    }
    let named = crate::native_palette_binding::current(sources, tag, frame)?;
    if named.attachment_slot != Some(slot) || named.native_cape.as_ref() != key {
        return None;
    }
    let ready = {
        let state = STATE.try_lock().ok()?;
        if state.unknown {
            return None;
        }
        state.ready.clone()?
    };
    let (session, timing) = crate::world_hooks::attachment_timing()?;
    if session != ready.session || !ready.clock(&timing) {
        return None;
    }
    let entry = ready.actors.get(&tag.actor)?;
    if !entry.samples.palettes.iter().any(|p| {
        p.slot == slot
            && p.native_cape.as_ref() == key
            && p.geometry == tag.geometry
            && p.hierarchy == named.hierarchy
            && p.definition == named.definition
            && p.bones.len() == named.bones.len()
            && p.bones.iter().zip(&named.bones).all(|(a, b)| {
                a.name == b.name
                    && a.index == b.index
                    && a.palette == b.cache.palette
                    && p.group == b.cache.group
                    && p.owner == b.owner
            })
    }) {
        return None;
    }
    let colors = if slot == crate::native_attachment_samples::WING_SLOT {
        &entry.wing_colors
    } else {
        &entry.cape_colors
    };
    let color = colors
        .iter()
        .find(|color| {
            color.slot == slot
                && color.geometry == tag.geometry
                && color.native_cape.as_ref() == key
        })?
        .clone();
    let draw = crate::native_transform::current_draw(tag.scope, &tag.actor)?;
    let target = ColorTarget {
        ready,
        timing,
        tag: tag.clone(),
        frame,
        draw,
        named,
        sources,
        color,
    };
    target.current().then_some(target)
}
pub(crate) fn query(
    tag: &crate::native_submit::Tag,
    frame: u64,
    named: &Named,
    native: &[[f32; 16]],
) -> Query {
    let (scope, actor, geometry) = (tag.scope, tag.actor.as_str(), tag.geometry.as_str());
    if named.attachment_slot.is_none() {
        return Query::Inactive;
    }
    if revision() == 0 {
        return Query::Blocked;
    }
    let (owner, ready) = match STATE.try_lock() {
        Ok(state) if !state.unknown => (state.owner.clone(), state.ready.clone()),
        _ => return Query::Blocked,
    };
    let Some(owner) = owner else {
        return Query::Inactive;
    };
    let Some((session, timing)) = crate::world_hooks::attachment_timing() else {
        return Query::Blocked;
    };
    if owner != session {
        return Query::Inactive;
    }
    let Some(ready) = ready.filter(|ready| {
        ready.session == session && ready.at.elapsed() <= LEASE && ready.clock(&timing)
    }) else {
        return Query::Blocked;
    };
    let Some(origin) = crate::world_hooks::attachment_origin() else {
        return Query::Blocked;
    };
    if !origin
        .iter()
        .zip(ready.camera_origin)
        .all(|(a, b)| a.to_bits() == b.to_bits())
    {
        return Query::Blocked;
    }
    let Some(current) = Palette::copy(named, geometry, native) else {
        return Query::Blocked;
    };
    let Some(entry) = ready.actors.get(actor) else {
        return Query::Blocked;
    };
    let Some(recorded) = entry
        .samples
        .palettes
        .iter()
        .find(|p| p.matches_draw(&current))
    else {
        return Query::Blocked;
    };
    let Some(space) = named.space.as_ref() else {
        return Query::Blocked;
    };
    let (draw, recorded_space) = if let Some(model) = entry.root.as_ref() {
        let Some(draw) = crate::native_transform::current_draw(scope, actor) else {
            return Query::Blocked;
        };
        let Some(child) = crate::native_palette_binding::draw_matrix(tag, frame) else {
            return Query::Blocked;
        };
        let mut desired = [0f32; 16];
        desired[15] = 1.;
        for row in 0..3 {
            for column in 0..4 {
                desired[column * 4 + row] =
                    model[row * 4 + column] / (if column == 3 { 1. } else { 16. });
            }
        }
        // One checked conversion: inverse(current child model) * recorded root
        // model. Avoid rounding through an intermediate current-root palette.
        let Some(recorded_space) = crate::native_emote_space::Space::between(&desired, &child)
        else {
            return Query::Blocked;
        };
        (Some(draw), Some(recorded_space))
    } else {
        (None, None)
    };
    let selected_space = recorded_space.as_ref().unwrap_or(space);
    let Some(matrices) = recorded
        .bones
        .iter()
        .map(|bone| selected_space.child_matrix(&bone.matrix))
        .collect::<Option<Vec<_>>>()
    else {
        return Query::Blocked;
    };
    let target = Target {
        palette: recorded.clone(),
        ready,
        timing,
        actor: actor.into(),
        scope,
        draw,
        matrices,
    };
    if target.current() {
        Query::Ready(target)
    } else {
        Query::Blocked
    }
}
pub(crate) fn status() -> Value {
    let state = STATE.try_lock().ok();
    let ready = state.as_ref().and_then(|state| state.ready.as_ref());
    json!({"activeSession":state.as_ref().and_then(|state|state.owner.as_deref()),"controlUnknown":state.as_ref().is_none_or(|state|state.unknown),"planLive":ready.is_some_and(|ready|ready.at.elapsed()<=LEASE),
        "players":ready.map(|ready|ready.actors.len()),"retainedBytes":ready.map(|ready|ready.bytes),"ownedCpuCopies":COPIES.load(Ordering::Relaxed),
        "ownedCapeColorCopies":COLOR_COPIES.load(Ordering::Relaxed),"nativeCapeColorReplayVerified":false,
        "ownedWingColorCopies":WING_COLOR_COPIES.load(Ordering::Relaxed),"nativeWingColorReplayVerified":false,
        "wingPaletteReplaySourceImplemented":true,"wingMaterialReplaySourceImplemented":true,
        "error":state.as_ref().and_then(|state|state.error.as_deref()),"last":LAST.try_lock().ok().and_then(|last|last.clone()),
        "nativeEquipmentRenderingVerified":false,"deterministicVideoVerified":false})
}
