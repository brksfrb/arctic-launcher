//! Exact-image observation of Model::dataDrivenDraw, after controller/bone work.
//! Replacement needs explicit normal/probe control and checked draw readiness.
use crate::native_scene_layout::model_body as body_layout;
use crate::world_hooks::{floats, read};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

// The three stack arguments remain pointers, including the final MERS input.
// Exact cached callers and both callee stack frames corroborate all seven.
type Draw = unsafe extern "system" fn(usize, usize, usize, usize, usize, usize, usize);
static ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static INSTALLED: AtomicBool = AtomicBool::new(false);
static BINDING: OnceLock<crate::world_hooks::ModelBodyBinding> = OnceLock::new();
static INSTALL: OnceLock<Result<(), String>> = OnceLock::new();
static CALLS: AtomicU64 = AtomicU64::new(0);
static FORWARDED: AtomicU64 = AtomicU64::new(0);
static ROOT_CALLS: AtomicU64 = AtomicU64::new(0);
static STEP: AtomicUsize = AtomicUsize::new(0);
static SAMPLES: Mutex<Vec<Sample>> = Mutex::new(Vec::new());
static PROBE_ENABLED: AtomicBool = AtomicBool::new(false);
static ISOLATED: AtomicBool = AtomicBool::new(false);
static USER_ENABLED: AtomicBool = AtomicBool::new(false);
static SUPPRESSED: AtomicU64 = AtomicU64::new(0);
static READY: Mutex<Option<Ready>> = Mutex::new(None);
static PLAN_ERROR: Mutex<Option<String>> = Mutex::new(None);
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Replacement {
    pub skin: String,
    pub slim: bool,
    pub geometry: String,
    pub animation_frame: u64,
    pub mesh_draws: u32,
    #[serde(default = "default_suppression_alpha")]
    pub suppression_alpha: f32,
    #[serde(default = "default_suppression_method")]
    pub suppression_method: String,
}
fn default_suppression_method() -> String {
    "bones".into()
}
fn default_suppression_alpha() -> f32 {
    0.0001
}
/// Copied native part-to-entity matrices for a standard skin hierarchy. The
/// Global::skin_pose field requires an actual draw; the selected-list reader
/// holds its separate, private reference without claiming a submission.
#[derive(Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SkinPose {
    pub geometry: String,
    pub hierarchy: u32,
    pub animation_frame: u64,
    pub native_skin: String,
    pub native_slim: bool,
    pub parts: [[f32; 12]; 6],
}
fn standard_geometry(name: &str) -> bool {
    matches!(
        name,
        "default"
            | "geometry.humanoid.custom"
            | "geometry.humanoid.customSlim"
            | "geometry.humanoid"
    )
}
#[derive(Clone, Deserialize)]
struct Skin {
    id: String,
    skin: String,
    slim: bool,
    #[serde(default, rename = "emoteToken")]
    emote_token: Option<String>,
    #[serde(default = "default_suppression_alpha", rename = "suppressionAlpha")]
    suppression_alpha: f32,
    #[serde(default = "default_suppression_method", rename = "suppressionMethod")]
    suppression_method: String,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Cape {
    id: String,
    cape: String,
    texture: String,
    native_cape: String,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HiddenCape {
    id: String,
    native_cape: String,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Elytra {
    id: String,
    cape: String,
    texture: String,
}
#[derive(Deserialize)]
struct Plan {
    schema: u32,
    session: String,
    players: Vec<Skin>,
    #[serde(default)]
    capes: Vec<Cape>,
    #[serde(default, rename = "hiddenCapes")]
    hidden_capes: Vec<HiddenCape>,
    #[serde(default)]
    elytras: Vec<Elytra>,
}
struct Ready {
    at: Instant,
    session: String,
    players: HashMap<String, Skin>,
    capes: HashMap<String, Cape>,
    hidden_capes: HashMap<String, HiddenCape>,
    elytras: HashMap<String, Elytra>,
}
impl Ready {
    fn skin(&self, id: &str, session: &str) -> Option<Skin> {
        if self.at.elapsed() > Duration::from_millis(250) || self.session != session {
            return None;
        }
        self.players.get(id).cloned()
    }
    fn cape(&self, id: &str, session: &str) -> Option<Cape> {
        if self.at.elapsed() > Duration::from_millis(250) || self.session != session {
            return None;
        }
        self.capes.get(id).cloned()
    }
    fn hidden_cape(&self, id: &str, session: &str) -> Option<HiddenCape> {
        if self.at.elapsed() > Duration::from_millis(250) || self.session != session {
            return None;
        }
        self.hidden_capes.get(id).cloned()
    }
    fn elytra(&self, id: &str, session: &str) -> Option<Elytra> {
        if self.at.elapsed() > Duration::from_millis(250) || self.session != session {
            return None;
        }
        self.elytras.get(id).cloned()
    }
}
fn parse_ready(input: &str) -> Result<Ready, String> {
    if input.len() > 128 * 1024 {
        return Err("body readiness exceeds limit".into());
    }
    let plan: Plan = serde_json::from_str(input).map_err(|e| e.to_string())?;
    if plan.schema != 1
        || plan.session.len() > 64
        || plan.players.len() > 128
        || plan.capes.len() > 128
        || plan.hidden_capes.len() > 128
        || plan.elytras.len() > 128
    {
        return Err("invalid body readiness header".into());
    }
    let mut players = HashMap::new();
    for p in plan.players {
        if !player_id(&p.id)
            || !p.suppression_alpha.is_finite()
            || !(0. ..=0.01).contains(&p.suppression_alpha)
            || !matches!(p.suppression_method.as_str(), "alpha" | "bones")
            || !texture_key(&p.skin)
            || p.emote_token
                .as_ref()
                .is_some_and(|token| !hex_hash(token, 64))
            || players.contains_key(&p.id)
        {
            return Err("invalid body readiness player".into());
        }
        players.insert(p.id.clone(), p);
    }
    let mut capes = HashMap::new();
    for cape in plan.capes {
        let root = format!("look:{}", cape.cape);
        let prefix = format!("{root}/");
        let texture_matches = cape.texture == root
            || cape.texture == format!("{root}/native")
            || cape.texture.strip_prefix(prefix.as_str()).is_some_and(|f| {
                !f.is_empty()
                    && f.len() <= 4
                    && f.bytes().all(|c| c.is_ascii_digit())
                    && f.parse::<u32>()
                        .is_ok_and(|n| n < 4096 && n.to_string() == f)
            });
        if !player_id(&cape.id)
            || !hex_hash(&cape.cape, 40)
            || !texture_matches
            || !cape
                .native_cape
                .strip_prefix("engine:cape/")
                .is_some_and(|h| hex_hash(h, 64))
            || capes.contains_key(&cape.id)
        {
            return Err("invalid cape readiness player or texture".into());
        }
        capes.insert(cape.id.clone(), cape);
    }
    let mut hidden_capes = HashMap::new();
    for cape in plan.hidden_capes {
        if !player_id(&cape.id)
            || !cape
                .native_cape
                .strip_prefix("engine:cape/")
                .is_some_and(|h| hex_hash(h, 64))
            || capes.contains_key(&cape.id)
            || hidden_capes.contains_key(&cape.id)
        {
            return Err("invalid or conflicting hidden cape readiness player".into());
        }
        hidden_capes.insert(cape.id.clone(), cape);
    }
    let mut elytras = HashMap::new();
    for wing in plan.elytras {
        let root = format!("look:{}", wing.cape);
        let prefix = format!("{root}/");
        let matches = wing.texture == root
            || wing.texture.strip_prefix(prefix.as_str()).is_some_and(|f| {
                !f.is_empty()
                    && f.len() <= 4
                    && f.bytes().all(|c| c.is_ascii_digit())
                    && f.parse::<u32>()
                        .is_ok_and(|n| n < 4096 && n.to_string() == f)
            });
        if !player_id(&wing.id)
            || !hex_hash(&wing.cape, 40)
            || !matches
            || elytras.contains_key(&wing.id)
        {
            return Err("invalid elytra readiness player or texture".into());
        }
        elytras.insert(wing.id.clone(), wing);
    }
    Ok(Ready {
        at: Instant::now(),
        session: plan.session,
        players,
        capes,
        hidden_capes,
        elytras,
    })
}
fn player_id(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(i, c)| {
            if [8, 13, 18, 23].contains(&i) {
                c == b'-'
            } else {
                c.is_ascii_hexdigit() && !c.is_ascii_uppercase()
            }
        })
}
fn hex_hash(hash: &str, size: usize) -> bool {
    hash.len() == size
        && hash
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}
fn texture_key(key: &str) -> bool {
    let hash = key
        .strip_prefix("look:")
        .map(|h| (h, 40))
        .or_else(|| key.strip_prefix("engine:skin/").map(|h| (h, 64)));
    hash.is_some_and(|(h, size)| hex_hash(h, size))
}
pub(crate) fn enabled() -> bool {
    PROBE_ENABLED.load(Ordering::Acquire) || user_enabled()
}
pub(crate) fn user_enabled() -> bool {
    USER_ENABLED.load(Ordering::Acquire)
}
/// Normal-use opt-in is separate from diagnostic modes and cannot enable body
/// writes in an isolated run whose probe configuration did not request them.
pub(crate) fn set_user_enabled(requested: bool) {
    let enabled = requested && !ISOLATED.load(Ordering::Acquire);
    let was_enabled = USER_ENABLED.swap(enabled, Ordering::AcqRel);
    if was_enabled && !enabled && !PROBE_ENABLED.load(Ordering::Acquire) {
        update_ready(None);
    }
}
/// Explicit probe policy, independent of the normal-use preference.
pub(crate) fn configure_probe(root: &std::path::Path, isolated: bool) {
    ISOLATED.store(isolated, Ordering::Release);
    USER_ENABLED.store(false, Ordering::Release);
    PROBE_ENABLED.store(false, Ordering::Release);
    if !isolated {
        crate::native_elytra::configure_probe(false);
        return;
    }
    let approved = || -> Option<(bool, bool)> {
        let path = root.join("arctic-offline-probe.json");
        if std::fs::metadata(&path).ok()?.len() > 4096 {
            return None;
        }
        let v: Value = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
        let state = crate::probe_isolation::status();
        Some((
            v["schema"] == 1
                && v["auto_world"] == true
                && v["body_probe"] == true
                && v["empty_user_provider"] == true
                && v["mode"] == "loopback-and-deny-user-provider"
                && state["userMode"] == "empty-provider"
                && state["networkMode"] == "loopback-only",
            v["elytra_probe"] == true,
        ))
    };
    let approved = approved().unwrap_or((false, false));
    PROBE_ENABLED.store(approved.0, Ordering::Release);
    crate::native_elytra::configure_probe(approved.0 && approved.1);
}
pub(crate) fn update_ready(input: Option<&str>) {
    let result = if enabled() {
        input.map(parse_ready).transpose()
    } else {
        Ok(None)
    };
    let mut ready = READY.lock().unwrap_or_else(|e| e.into_inner());
    match result {
        Ok(value) => {
            *ready = value;
        }
        Err(error) => {
            *ready = None;
            if let Ok(mut last) = PLAN_ERROR.lock() {
                *last = Some(error);
            }
        }
    }
}
fn skin_for(id: &str) -> Option<Skin> {
    if !enabled() {
        return None;
    }
    let (_, epoch) = crate::players::session()?;
    let ready = READY.try_lock().ok()?;
    let ready = ready.as_ref()?;
    ready.skin(id, &format!("native_{}_{}", std::process::id(), epoch))
}
fn cape_for(id: &str) -> Option<Cape> {
    if !enabled() {
        return None;
    }
    let (_, epoch) = crate::players::session()?;
    let ready = READY.try_lock().ok()?;
    ready
        .as_ref()?
        .cape(id, &format!("native_{}_{}", std::process::id(), epoch))
}
fn hidden_cape_for(id: &str) -> Option<HiddenCape> {
    if !enabled() {
        return None;
    }
    let (_, epoch) = crate::players::session()?;
    let ready = READY.try_lock().ok()?;
    ready
        .as_ref()?
        .hidden_cape(id, &format!("native_{}_{}", std::process::id(), epoch))
}
fn elytra_for(id: &str) -> Option<Elytra> {
    if !crate::native_elytra::enabled() {
        return None;
    }
    let (_, epoch) = crate::players::session()?;
    let ready = READY.try_lock().ok()?;
    ready
        .as_ref()?
        .elytra(id, &format!("native_{}_{}", std::process::id(), epoch))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Sample {
    #[serde(skip)]
    palette_binding: Option<crate::native_palette_binding::Binding>,
    #[serde(skip)]
    skin_reference: Option<std::sync::Arc<crate::native_skin_selection::Reference>>,
    partial: f32,
    actor_id: String,
    world_scope: u64,
    geometry: String,
    hierarchy: u32,
    animation_frame: u64,
    root_frame: u64,
    complete_standing_pose: bool,
    complete_standard_pose: bool,
    skin_pose: Option<SkinPose>,
    wing_pose: Option<crate::native_elytra::WingPose>,
    native_skin: Option<String>,
    native_slim: Option<bool>,
    native_cape: Option<String>,
    cape_mesh: bool,
    skin_mesh: bool,
    chest_empty: bool,
    cape_motion: Option<crate::native_capes::Motion>,
}
fn observe(model: usize, params: usize, controller: usize, geometry: usize) -> Option<Sample> {
    STEP.store(0, Ordering::Relaxed);
    let (address, context, global) = crate::native_transform::model_binding(model, params)?;
    STEP.store(1, Ordering::Relaxed);
    ROOT_CALLS.fetch_add(1, Ordering::Relaxed);
    // DataDrivenGeometry has a vptr, then a 48-byte HashedString. Its
    // hierarchy index at +0xe8 is used by this exact controller's bone lookup.
    let name = crate::players::string(geometry.checked_add(body_layout::GEOMETRY_NAME)?, 128)?;
    STEP.store(2, Ordering::Relaxed);
    let mut index = [0u8; 4];
    read(
        geometry.checked_add(body_layout::GEOMETRY_HIERARCHY)?,
        &mut index,
    )
    .ok()?;
    let hierarchy = u32::from_le_bytes(index);
    STEP.store(3, Ordering::Relaxed);
    let actor = crate::players::actor(address)?;
    let pose = crate::animation_pose::actor(address, context)?;
    STEP.store(4, Ordering::Relaxed);
    let selected = pose.hierarchies.iter().find(|h| h.index == hierarchy)?;
    let partial = floats::<1>(params.checked_add(body_layout::PARAMS_PARTIAL)?, 1.)?[0];
    if partial < 0. {
        return None;
    }
    let frame = crate::live_frame::player(&actor, &[0.; 3], partial, true, Some(&pose))?;
    let complete =
        frame["availableParts"] == 63 && frame["visible"] == true && actor.appearance.is_some();
    // Classify the geometry's actual native hierarchy, not a guessed logical
    // alias or any draw made while a player happened to be active.
    let cape_mesh = selected.bones.iter().any(|b| b.name == "cape")
        && selected
            .bones
            .iter()
            .all(|b| matches!(b.name.as_str(), "root" | "cape"));
    let skin_mesh = ["head", "body", "rightarm", "leftarm", "rightleg", "leftleg"]
        .iter()
        .all(|name| selected.bones.iter().any(|b| b.name == *name));
    let appearance = actor.appearance.as_ref();
    let skin_pose = if skin_mesh && standard_geometry(&name) {
        appearance.and_then(|appearance| {
            crate::animation_pose::standard_parts(&pose, hierarchy, appearance.slim).map(|parts| {
                SkinPose {
                    geometry: name.clone(),
                    hierarchy,
                    animation_frame: pose.update_frame,
                    native_skin: appearance.skin.clone(),
                    native_slim: appearance.slim,
                    parts,
                }
            })
        })
    } else {
        None
    };
    let visible = actor.components.as_ref().is_some_and(|c| !c.invisible());
    let complete_standard_pose = visible
        && (skin_pose.is_some()
            || global.skin_pose.as_ref().is_some_and(|p| {
                p.animation_frame == pose.update_frame
                    && appearance
                        .is_some_and(|a| a.skin == p.native_skin && a.slim == p.native_slim)
            }));
    // Seed an emote reference from the actual root-selected geometry list even
    // when this draw is equipment. A selected reference is not a skin draw:
    // replacement eligibility above still requires an observed skin submission.
    let skin_reference = if visible && enabled() && pose.update_frame == global.animation_frame {
        appearance.and_then(|appearance| {
            crate::native_skin_selection::copy(
                address, context, model, params, controller, geometry, &pose, appearance,
            )
        })
    } else {
        None
    };
    // WingsLayer does not share the skin/custom layer's invisibility gate.
    // Preserve actual attachable observations even when skin replacement is
    // ineligible. complete_standard_pose still governs suppression below.
    let wing_pose = if actor.chest_item.as_deref() == Some("minecraft:elytra") {
        crate::native_elytra::observe(&pose, hierarchy, &name)
    } else {
        None
    };
    STEP.store(5, Ordering::Relaxed);
    Some(Sample {
        palette_binding: if complete_standard_pose || skin_reference.is_some() {
            crate::native_palette_binding::copy(geometry, selected, pose.update_frame)
        } else {
            None
        },
        skin_reference,
        partial,
        native_skin: actor.appearance.as_ref().map(|a| a.skin.clone()),
        native_slim: actor.appearance.as_ref().map(|a| a.slim),
        native_cape: actor.appearance.as_ref().and_then(|a| a.cape.clone()),
        cape_mesh,
        skin_mesh,
        chest_empty: actor.chest_empty == Some(true),
        cape_motion: if cape_mesh && enabled() {
            crate::native_capes::sample_current(&actor, partial)
        } else {
            None
        },
        actor_id: actor.identity.id,
        world_scope: global.world_scope,
        geometry: name,
        hierarchy,
        animation_frame: pose.update_frame,
        root_frame: global.animation_frame,
        complete_standing_pose: complete,
        complete_standard_pose,
        skin_pose,
        wing_pose,
    })
}
fn remember(sample: Sample) {
    let Ok(mut samples) = SAMPLES.try_lock() else {
        return;
    };
    if let Some(previous) = samples.iter_mut().find(|s| s.geometry == sample.geometry) {
        *previous = sample;
    } else if samples.len() < 16 {
        samples.push(sample);
    }
}
unsafe extern "system" fn draw(
    model: usize,
    params: usize,
    metadata: usize,
    controller: usize,
    geometry: usize,
    textures: usize,
    mers: usize,
) {
    CALLS.fetch_add(1, Ordering::Relaxed);
    if !INSTALLED.load(Ordering::Acquire) {
        let original: Draw = unsafe { std::mem::transmute(ORIGINAL.load(Ordering::Acquire)) };
        unsafe {
            original(
                model, params, metadata, controller, geometry, textures, mers,
            )
        };
        FORWARDED.fetch_add(1, Ordering::Relaxed);
        return;
    }
    let prepared = std::panic::catch_unwind(|| {
        let Some(mut sample) = observe(model, params, controller, geometry) else {
            return crate::native_attachment_binding::prepare(model, params, controller, geometry);
        };
        let wings_accepted = sample
            .wing_pose
            .clone()
            .is_some_and(crate::native_transform::record_wing_pose);
        let reference_pose = sample
            .skin_reference
            .as_ref()
            .map(|reference| reference.pose.clone());
        // Never put the frame-local native metadata in persistent diagnostics.
        let reference_accepted = sample
            .skin_reference
            .take()
            .is_some_and(crate::native_transform::record_skin_reference);
        let pose_accepted = sample
            .skin_pose
            .clone()
            .is_some_and(crate::native_transform::record_skin_pose);
        let ready = skin_for(&sample.actor_id);
        let eligible = pose_accepted
            && sample.complete_standard_pose
            && sample.skin_mesh
            && sample.animation_frame == sample.root_frame
            && standard_geometry(&sample.geometry);
        if (eligible || reference_accepted)
            && sample.animation_frame == sample.root_frame
            && let Some(token) = ready.as_ref().and_then(|skin| skin.emote_token.as_deref())
            && let Some(pose) = sample.skin_pose.as_ref().or(reference_pose.as_ref())
            && let Some(target) =
                crate::native_emote_pose::sample(token, &sample.actor_id, sample.partial, pose)
        {
            crate::native_transform::record_emote_pose(&sample.actor_id, target);
        }
        let replacement = ready
            .filter(|skin| {
                eligible
                    // An engine key must belong to this live actor, including its arm
                    // model. A loaded skin from another player is never sufficient.
                    && (!skin.skin.starts_with("engine:")
                        || sample.native_skin.as_deref() == Some(skin.skin.as_str())
                            && sample.native_slim == Some(skin.slim))
            })
            .map(|skin| {
                crate::native_submit::Replacement::Body(Replacement {
                    skin: skin.skin,
                    slim: skin.slim,
                    geometry: sample.geometry.clone(),
                    animation_frame: sample.animation_frame,
                    mesh_draws: 1,
                    suppression_alpha: skin.suppression_alpha,
                    suppression_method: skin.suppression_method,
                })
            });
        let replacement = replacement.or_else(|| {
            if !sample.complete_standard_pose
                || !sample.cape_mesh
                || !sample.chest_empty
                || sample.animation_frame != sample.root_frame
            {
                return None;
            }
            if let Some(cape) = hidden_cape_for(&sample.actor_id) {
                if sample.native_cape.as_deref() != Some(cape.native_cape.as_str()) {
                    return None;
                }
                return Some(crate::native_submit::Replacement::HideCape(
                    crate::native_capes::Hidden {
                        native_cape: cape.native_cape,
                        geometry: sample.geometry.clone(),
                        animation_frame: sample.animation_frame,
                        mesh_draws: 1,
                    },
                ));
            }
            let cape = cape_for(&sample.actor_id)?;
            if sample.native_cape.as_deref() != Some(cape.native_cape.as_str()) {
                return None;
            }
            let motion = sample.cape_motion.clone()?;
            Some(crate::native_submit::Replacement::Cape(
                crate::native_capes::Replacement {
                    cape: cape.cape,
                    texture: cape.texture,
                    native_cape: cape.native_cape,
                    geometry: sample.geometry.clone(),
                    animation_frame: sample.animation_frame,
                    mesh_draws: 1,
                    motion,
                },
            ))
        });
        let replacement = replacement.or_else(|| {
            if !wings_accepted
                || !sample.complete_standard_pose
                || sample.animation_frame != sample.root_frame
            {
                return None;
            }
            let wing = elytra_for(&sample.actor_id)?;
            let pose = sample.wing_pose.as_ref()?;
            Some(crate::native_submit::Replacement::Elytra(
                crate::native_elytra::Replacement {
                    cape: wing.cape,
                    texture: wing.texture,
                    geometry: pose.geometry.clone(),
                    animation_frame: pose.animation_frame,
                    mesh_draws: 1,
                    wings: pose.wings,
                },
            ))
        });
        let tag = crate::native_submit::Tag {
            scope: sample.world_scope,
            actor: sample.actor_id.clone(),
            geometry: sample.geometry.clone(),
            replacement,
            role: if sample.skin_mesh {
                crate::native_submit::MeshRole::Skin
            } else if sample.cape_mesh {
                crate::native_submit::MeshRole::Cape {
                    animation_frame: sample.animation_frame,
                    native_cape: sample.native_cape.clone(),
                }
            } else if wings_accepted {
                crate::native_submit::MeshRole::Elytra {
                    animation_frame: sample.animation_frame,
                }
            } else {
                crate::native_submit::MeshRole::Other
            },
        };
        let binding = sample.palette_binding.take();
        remember(sample);
        Some((tag, binding))
    })
    .ok()
    .flatten();
    let (tag, binding) = prepared.map_or((None, None), |(tag, binding)| (Some(tag), binding));
    let binding = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        binding.map(|binding| binding.with_cache_owner(params))
    }))
    .ok()
    .flatten();
    let _palette_binding = crate::native_palette_binding::begin(binding, tag.as_ref());
    let _submission = crate::native_submit::begin(tag);
    unsafe {
        let original: Draw = std::mem::transmute(ORIGINAL.load(Ordering::Acquire));
        original(
            model, params, metadata, controller, geometry, textures, mers,
        );
    }
    FORWARDED.fetch_add(1, Ordering::Relaxed);
}
pub(crate) fn record_suppressed(body: Replacement) -> bool {
    if !enabled() || !crate::native_transform::record_body(body) {
        return false;
    }
    SUPPRESSED.fetch_add(1, Ordering::Relaxed);
    true
}
pub(crate) fn install() -> Result<(), String> {
    INSTALL
        .get_or_init(|| {
            let base = crate::world_hooks::image_base().ok_or("unvalidated body image")?;
            let binding = crate::world_hooks::model_body_binding()?;
            let target = base
                .checked_add(binding.rva)
                .ok_or("body observer address overflow")?;
            let expected = binding.prefix();
            let mut actual = vec![0u8; expected.len()];
            read(target, &mut actual)?;
            if actual.as_slice() != expected {
                return Err("body draw prologue differs from exact profile".into());
            }
            BINDING
                .set(binding)
                .map_err(|_| "body draw binding already published")?;
            // Admit this body's actual ABI/caller before installing dependent
            // readers or hooks. A mismatched body cannot partially activate them.
            crate::native_skin_selection::validate_profile(base)?;
            crate::native_attachment_binding::validate_profile(base)?;
            crate::native_expression_binding::install(base)?;
            crate::native_submit::install(base)?;
            crate::native_skinned_desc::install(base)?;
            use hudhook::mh::{MH_CreateHook, MH_EnableHook, MH_STATUS};
            let mut original = std::ptr::null_mut();
            let status =
                unsafe { MH_CreateHook(target as _, draw as *const () as _, &mut original) };
            if status != MH_STATUS::MH_OK {
                return Err(format!("create body draw observer: {status:?}"));
            }
            ORIGINAL.store(original as usize, Ordering::Release);
            let status = unsafe { MH_EnableHook(target as _) };
            if status != MH_STATUS::MH_OK {
                return Err(format!("enable body draw observer: {status:?}"));
            }
            INSTALLED.store(true, Ordering::Release);
            Ok(())
        })
        .clone()
}
pub(crate) fn status() -> Value {
    json!({"installed":INSTALLED.load(Ordering::Acquire),"hookRva":BINDING.get().map(|b|b.rva),
        "controllerRva":BINDING.get().map(|b|b.caller_rva),"rootModelDrawRva":BINDING.get().map(|b|b.root_rva),
        "bodyCallReturnRva":BINDING.get().map(|b|b.body_return_rva),
        "sevenPointerHookSourceImplemented":true,
        "calls":CALLS.load(Ordering::Relaxed),"forwarded":FORWARDED.load(Ordering::Relaxed),
        "rootCalls":ROOT_CALLS.load(Ordering::Relaxed),"samples":SAMPLES.try_lock().ok().map(|s|json!(*s)),
        "step":STEP.load(Ordering::Relaxed),
        "submission":crate::native_submit::status(),
        "geometryStream":crate::native_geometry_stream::status(),
        "skinnedDescription":crate::native_skinned_desc::status(),
        "skinSelection":crate::native_skin_selection::status(),
        "attachmentBinding":crate::native_attachment_binding::status(),
        "defaultGameMode":crate::native_game_mode::status(),
        "replacementEnabled":enabled(),"suppressed":SUPPRESSED.load(Ordering::Relaxed),
        "userEnabled":user_enabled(),"probeEnabled":PROBE_ENABLED.load(Ordering::Acquire),"isolated":ISOLATED.load(Ordering::Acquire),
        "readyPlayers":READY.try_lock().ok().and_then(|r|r.as_ref().map(|r|r.players.len())),
        "readyCapes":READY.try_lock().ok().and_then(|r|r.as_ref().map(|r|r.capes.len())),
        "readyHiddenCapes":READY.try_lock().ok().and_then(|r|r.as_ref().map(|r|r.hidden_capes.len())),
        "capeReplacement":crate::native_capes::replacement_status(),
        "elytraReplacement":crate::native_elytra::status(),
        "planError":PLAN_ERROR.try_lock().ok().and_then(|e|e.clone()),"bodyReplacementVerified":false})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn readiness_requires_unique_canonical_players_loaded_texture_keys_and_live_session() {
        let id = "b50ad385-829d-3141-a216-7e7d7539ba7f";
        let entry = json!({"id":id,"skin":format!("look:{}","a".repeat(40)),"slim":false});
        let mut input = json!({"schema":1,"session":"native_123_1","players":[entry.clone()]});
        let mut ready = parse_ready(&input.to_string()).unwrap();
        assert!(ready.skin(id, "native_123_1").is_some());
        assert!(ready.skin(id, "native_123_2").is_none());
        ready.at = Instant::now() - Duration::from_millis(251);
        assert!(ready.skin(id, "native_123_1").is_none());
        input["players"].as_array_mut().unwrap().push(entry);
        assert!(parse_ready(&input.to_string()).is_err());
        input["players"].as_array_mut().unwrap().pop();
        input["players"][0]["skin"] = json!("engine:arbitrary");
        assert!(parse_ready(&input.to_string()).is_err());
        input["players"][0]["skin"] = json!(format!("look:{}", "a".repeat(40)));
        input["players"][0]["id"] = json!("not-a-world-player");
        assert!(parse_ready(&input.to_string()).is_err());
        input["players"][0]["id"] = json!(id);
        for alpha in [-0.001, 0.1] {
            input["players"][0]["suppressionAlpha"] = json!(alpha);
            assert!(parse_ready(&input.to_string()).is_err());
        }
        input["players"][0]["suppressionAlpha"] = json!(0.);
        assert!(parse_ready(&input.to_string()).is_ok());
        for method in ["bones", "alpha"] {
            input["players"][0]["suppressionMethod"] = json!(method);
            let ready = parse_ready(&input.to_string()).unwrap();
            assert_eq!(
                ready.skin(id, "native_123_1").unwrap().suppression_method,
                method
            );
        }
        input["players"][0]["suppressionMethod"] = json!("arbitrary");
        assert!(parse_ready(&input.to_string()).is_err());
    }
    #[test]
    fn body_draw_preserves_all_three_stack_arguments_and_forwards_once() {
        static ARGS: Mutex<Vec<[usize; 7]>> = Mutex::new(Vec::new());
        unsafe extern "system" fn original(
            a: usize,
            b: usize,
            c: usize,
            d: usize,
            e: usize,
            f: usize,
            g: usize,
        ) {
            ARGS.lock().unwrap().push([a, b, c, d, e, f, g]);
        }
        ORIGINAL.store(original as *const () as usize, Ordering::Release);
        unsafe {
            draw(1, 2, 3, 4, 0x11223344, 0x55667788, 0x99aabbcc);
        }
        assert_eq!(
            *ARGS.lock().unwrap(),
            vec![[1, 2, 3, 4, 0x11223344, 0x55667788, 0x99aabbcc]]
        );
    }
}
