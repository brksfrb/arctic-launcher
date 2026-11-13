//! Native renderer/matrix observations. No matrix, pose or engine result is changed.
use crate::world_hooks::{TransformBinding, TransformObservation};
use crate::world_hooks::{floats, pointer, read};
#[path = "native_transform_draw_frame.rs"]
mod draw_frame;
#[path = "native_draw_matrices.rs"]
mod draw_matrices;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
};

type GlobalFn = unsafe extern "system" fn(usize, usize, usize);
// The shared_ptr argument is passed indirectly under the Windows x64 ABI.
// Forward its native temporary exactly once; Rust never takes its ownership.
type DrawFn = unsafe extern "system" fn(usize, usize, usize, usize, usize);
static ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static DRAW_ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static TARGET: AtomicUsize = AtomicUsize::new(0);
static ENABLED: AtomicBool = AtomicBool::new(false);
static BINDING: OnceLock<TransformBinding> = OnceLock::new();
static INSTALL: OnceLock<Result<(), String>> = OnceLock::new();
static OBSERVATION_SERIAL: AtomicU64 = AtomicU64::new(0);
fn observation_serial() -> Option<u64> {
    OBSERVATION_SERIAL
        .try_update(Ordering::AcqRel, Ordering::Acquire, |old| {
            old.checked_add(1)
        })
        .ok()?
        .checked_add(1)
}
static MAP_ATTEMPTS: AtomicU64 = AtomicU64::new(0);
static CALLS: AtomicU64 = AtomicU64::new(0);
static COPIES: AtomicU64 = AtomicU64::new(0);
static RENDERER: OnceLock<Renderer> = OnceLock::new();
static LAST: Mutex<Option<Global>> = Mutex::new(None);
static STEP: AtomicUsize = AtomicUsize::new(0);
static CALLERS: OnceLock<Vec<usize>> = OnceLock::new();
static MAP_NAMES: Mutex<Vec<String>> = Mutex::new(Vec::new());
static DRAW_COPIES: AtomicU64 = AtomicU64::new(0);
static DRAW_STEP: AtomicUsize = AtomicUsize::new(0);
static BODY_FINISHED: AtomicU64 = AtomicU64::new(0);
static BODY_REJECTED: AtomicU64 = AtomicU64::new(0);
static CAPE_FINISHED: AtomicU64 = AtomicU64::new(0);
static CAPE_REJECTED: AtomicU64 = AtomicU64::new(0);
static LAST_BODY: Mutex<Option<Global>> = Mutex::new(None);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Renderer {
    table_rva: usize,
    render_rva: usize,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LayerColor {
    pub geometry: String,
    pub animation_frame: u64,
    pub native_cape: Option<String>,
    pub rgba: [f32; 4],
    #[serde(default)]
    pub source_dithering: Option<bool>,
    #[serde(default)]
    pub dithering_enabled: Option<bool>,
    #[serde(default)]
    pub shader_inputs: Option<crate::native_shader_inputs::ActorInputs>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Global {
    pub actor_id: String,
    pub world_scope: u64,
    pub animation_frame: u64,
    /// Legacy helper diagnostics. Draw-entry families copy the authoritative
    /// draw matrices without inventing an unavailable intermediate pair.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<[f32; 16]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<[f32; 16]>,
    #[serde(default)]
    pub observation: TransformObservation,
    pub draw: Option<Draw>,
    #[serde(default)]
    pub body: Option<crate::native_body::Replacement>,
    #[serde(default)]
    pub cape: Option<crate::native_capes::Replacement>,
    #[serde(default)]
    pub hidden_cape: Option<crate::native_capes::Hidden>,
    #[serde(default)]
    pub skin_pose: Option<crate::native_body::SkinPose>,
    #[serde(default)]
    pub skin_color: Option<[f32; 4]>,
    #[serde(default)]
    pub skin_color_conflict: bool,
    #[serde(default)]
    pub cape_color: Option<LayerColor>,
    #[serde(default)]
    pub cape_color_conflict: bool,
    #[serde(default)]
    pub cape_replay_color: Option<crate::native_attachment_replay::CapeColorReceipt>,
    #[serde(default)]
    pub cape_replay_color_conflict: bool,
    #[serde(default)]
    pub elytra_color: Option<LayerColor>,
    #[serde(default)]
    pub elytra_color_conflict: bool,
    #[serde(default)]
    pub elytra_replay_color: Option<crate::native_attachment_replay::CapeColorReceipt>,
    #[serde(default)]
    pub elytra_replay_color_conflict: bool,
    #[serde(default, skip_serializing)]
    pub emote_pose: Option<crate::native_emote_pose::Target>,
    #[serde(default)]
    pub emote_pose_conflict: bool,
    #[serde(default, skip_serializing)]
    pub attachment_samples: Option<crate::native_attachment_samples::Frame>,
    #[serde(default)]
    pub wing_pose: Option<crate::native_elytra::WingPose>,
    #[serde(default)]
    pub elytra: Option<crate::native_elytra::Replacement>,
}
pub(crate) fn record_emote_pose(actor: &str, target: crate::native_emote_pose::Target) -> bool {
    ACTORS
        .try_with(|s| {
            let Ok(mut s) = s.try_borrow_mut() else {
                return false;
            };
            let Some(entry) = s.last_mut() else {
                return false;
            };
            if entry.ambiguous || crate::world_hooks::current_pass() != Some(entry.scope) {
                return false;
            }
            if !emote_reference(entry).is_some_and(|skin| {
                skin.animation_frame == target.animation_frame
                    && skin.parts == target.reference_parts
            }) {
                return false;
            }
            let Some(global) = entry.observed.as_mut() else {
                return false;
            };
            if global.actor_id != actor
                || global.draw.is_none()
                || global.animation_frame != target.animation_frame
                || global.emote_pose_conflict
            {
                return false;
            }
            if let Some(previous) = global.emote_pose.as_ref() {
                if previous != &target {
                    global.emote_pose = None;
                    global.emote_pose_conflict = true;
                    return false;
                }
            } else {
                global.emote_pose = Some(target);
            }
            true
        })
        .unwrap_or(false)
}
pub(crate) fn record_elytra(replacement: crate::native_elytra::Replacement) -> bool {
    ACTORS
        .try_with(|s| {
            let Ok(mut s) = s.try_borrow_mut() else {
                return false;
            };
            let Some(entry) = s.last_mut() else {
                return false;
            };
            if entry.ambiguous || crate::world_hooks::current_pass() != Some(entry.scope) {
                return false;
            }
            let Some(global) = entry.observed.as_mut() else {
                return false;
            };
            if global.draw.is_none()
                || replacement.animation_frame != global.animation_frame
                || !global
                    .skin_pose
                    .as_ref()
                    .is_some_and(|p| p.animation_frame == replacement.animation_frame)
                || !global.wing_pose.as_ref().is_some_and(|p| {
                    p.animation_frame == replacement.animation_frame
                        && p.geometry == replacement.geometry
                        && p.wings == replacement.wings
                })
            {
                return false;
            }
            if let Some(previous) = global.elytra.as_mut() {
                if previous.cape != replacement.cape
                    || previous.texture != replacement.texture
                    || previous.geometry != replacement.geometry
                    || previous.animation_frame != replacement.animation_frame
                    || previous.wings != replacement.wings
                {
                    return false;
                }
                previous.mesh_draws = previous.mesh_draws.saturating_add(1);
            } else {
                global.elytra = Some(replacement);
            }
            true
        })
        .unwrap_or(false)
}
pub(crate) fn record_wing_pose(pose: crate::native_elytra::WingPose) -> bool {
    if !draw_frame::synchronize() {
        return false;
    }
    ACTORS
        .try_with(|s| {
            let Ok(mut s) = s.try_borrow_mut() else {
                return false;
            };
            let Some(entry) = s.last_mut() else {
                return false;
            };
            if entry.ambiguous || crate::world_hooks::current_pass() != Some(entry.scope) {
                return false;
            }
            let Some(global) = entry.observed.as_mut() else {
                return false;
            };
            if global.draw.is_none() || pose.animation_frame != global.animation_frame {
                return false;
            }
            if let Some(previous) = &global.wing_pose {
                return previous == &pose;
            }
            global.wing_pose = Some(pose);
            true
        })
        .unwrap_or(false)
}
pub(crate) fn record_skin_pose(pose: crate::native_body::SkinPose) -> bool {
    if !draw_frame::synchronize() {
        return false;
    }
    ACTORS
        .try_with(|s| {
            let Ok(mut s) = s.try_borrow_mut() else {
                return false;
            };
            let Some(entry) = s.last_mut() else {
                return false;
            };
            if entry.ambiguous || crate::world_hooks::current_pass() != Some(entry.scope) {
                return false;
            }
            let Some(global) = entry.observed.as_mut() else {
                return false;
            };
            if global.draw.is_none() || pose.animation_frame != global.animation_frame {
                return false;
            }
            if entry
                .skin_reference
                .as_ref()
                .is_some_and(|reference| reference.pose != pose || !reference.valid())
                || global
                    .skin_pose
                    .as_ref()
                    .is_some_and(|previous| previous != &pose)
            {
                global.emote_pose = None;
                global.emote_pose_conflict = true;
                global.skin_color = None;
                global.skin_color_conflict = true;
                return false;
            }
            if global.skin_pose.is_some() {
                return true;
            }
            global.skin_pose = Some(pose);
            true
        })
        .unwrap_or(false)
}
/// Original RGBA from an owned, root/color-matched skin description, before
/// suppression changes its alpha. Other geometry cannot supply this color.
pub(crate) fn record_skin_color(scope: u64, actor: &str, geometry: &str, color: [f32; 4]) -> bool {
    ACTORS
        .try_with(|s| {
            let Ok(mut s) = s.try_borrow_mut() else {
                return false;
            };
            let Some(entry) = s.last_mut() else {
                return false;
            };
            if entry.ambiguous
                || entry.scope != scope
                || crate::world_hooks::current_pass() != Some(scope)
            {
                return false;
            }
            let Some(global) = entry.observed.as_mut() else {
                return false;
            };
            if global.actor_id != actor
                || global.draw.is_none()
                || global.skin_color_conflict
                || !global.skin_pose.as_ref().is_some_and(|pose| {
                    pose.geometry == geometry && pose.animation_frame == global.animation_frame
                })
            {
                return false;
            }
            if color
                .iter()
                .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
                || global.skin_color.is_some_and(|previous| previous != color)
            {
                global.skin_color = None;
                global.skin_color_conflict = true;
                return false;
            }
            global.skin_color = Some(color);
            true
        })
        .unwrap_or(false)
}
pub(crate) fn record_draw_color(
    tag: &crate::native_submit::Tag,
    color: [f32; 4],
    source_dithering: Option<bool>,
    dithering_enabled: Option<bool>,
    shader_inputs: Option<crate::native_shader_inputs::ActorInputs>,
) -> bool {
    use crate::native_submit::MeshRole;
    if matches!(&tag.role, MeshRole::Skin) {
        return record_skin_color(tag.scope, &tag.actor, &tag.geometry, color);
    }
    let (frame, native_cape, wing) = match &tag.role {
        MeshRole::Cape {
            animation_frame,
            native_cape: Some(key),
        } => (*animation_frame, Some(key.clone()), false),
        MeshRole::Elytra { animation_frame } => (*animation_frame, None, true),
        _ => return false,
    };
    ACTORS
        .try_with(|s| {
            let Ok(mut s) = s.try_borrow_mut() else {
                return false;
            };
            let Some(entry) = s.last_mut() else {
                return false;
            };
            if entry.ambiguous
                || entry.scope != tag.scope
                || crate::world_hooks::current_pass() != Some(tag.scope)
            {
                return false;
            }
            let Some(global) = entry.observed.as_mut() else {
                return false;
            };
            if global.actor_id != tag.actor
                || global.draw.is_none()
                || global.animation_frame != frame
                || wing
                    && !global
                        .wing_pose
                        .as_ref()
                        .is_some_and(|p| p.geometry == tag.geometry && p.animation_frame == frame)
            {
                return false;
            }
            let (slot, conflict) = if wing {
                (&mut global.elytra_color, &mut global.elytra_color_conflict)
            } else {
                (&mut global.cape_color, &mut global.cape_color_conflict)
            };
            if *conflict {
                return false;
            }
            if color
                .iter()
                .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
                || slot.as_ref().is_some_and(|old| {
                    old.geometry != tag.geometry
                        || old.animation_frame != frame
                        || old.native_cape != native_cape
                        || old.rgba != color
                        || old.source_dithering != source_dithering
                        || old.dithering_enabled != dithering_enabled
                })
            {
                *slot = None;
                *conflict = true;
                if wing {
                    global.elytra_replay_color = None;
                    global.elytra_replay_color_conflict = true;
                } else {
                    global.cape_replay_color = None;
                    global.cape_replay_color_conflict = true;
                }
                return false;
            }
            // Every matching layer description must supply the same checked
            // cohort. One missing/changed cohort stays unknown for this frame,
            // without erasing otherwise usable original RGBA/dithering.
            let shader_inputs = shader_inputs
                .filter(|inputs| inputs.valid())
                .filter(|inputs| {
                    slot.as_ref().is_none_or(|old| {
                        old.shader_inputs
                            .as_ref()
                            .is_some_and(|old| old.same(inputs))
                    })
                });
            let shader_inputs_changed = slot
                .as_ref()
                .is_some_and(|old| old.shader_inputs != shader_inputs);
            *slot = Some(LayerColor {
                geometry: tag.geometry.clone(),
                animation_frame: frame,
                native_cape,
                rgba: color,
                source_dithering,
                dithering_enabled,
                shader_inputs,
            });
            if shader_inputs_changed {
                if wing {
                    global.elytra_replay_color = None;
                    global.elytra_replay_color_conflict = true;
                } else {
                    global.cape_replay_color = None;
                    global.cape_replay_color_conflict = true;
                }
            }
            true
        })
        .unwrap_or(false)
}
fn wing_layer(tag: &crate::native_submit::Tag) -> Option<bool> {
    match &tag.role {
        crate::native_submit::MeshRole::Cape {
            native_cape: Some(_),
            ..
        } => Some(false),
        crate::native_submit::MeshRole::Elytra { .. } if tag.geometry == "geometry.elytra" => {
            Some(true)
        }
        _ => None,
    }
}
pub(crate) fn invalidate_replay_layer_color(tag: &crate::native_submit::Tag) {
    let Some(wing) = wing_layer(tag) else {
        return;
    };
    let _ = ACTORS.try_with(|active| {
        let Ok(mut active) = active.try_borrow_mut() else {
            return;
        };
        let Some(entry) = active.last_mut() else {
            return;
        };
        if entry.ambiguous
            || entry.scope != tag.scope
            || crate::world_hooks::current_pass() != Some(tag.scope)
        {
            return;
        }
        let Some(global) = entry
            .observed
            .as_mut()
            .filter(|global| global.actor_id == tag.actor)
        else {
            return;
        };
        if wing {
            global.elytra_replay_color = None;
            global.elytra_replay_color_conflict = true;
        } else {
            global.cape_replay_color = None;
            global.cape_replay_color_conflict = true;
        }
    });
}
pub(crate) fn invalidate_layer_draw_color(tag: &crate::native_submit::Tag) {
    let Some(wing) = wing_layer(tag) else {
        return;
    };
    let _ = ACTORS.try_with(|active| {
        let Ok(mut active) = active.try_borrow_mut() else {
            return;
        };
        let Some(entry) = active.last_mut() else {
            return;
        };
        if !entry.ambiguous
            && entry.scope == tag.scope
            && crate::world_hooks::current_pass() == Some(tag.scope)
            && let Some(global) = entry
                .observed
                .as_mut()
                .filter(|global| global.actor_id == tag.actor)
        {
            if wing {
                global.elytra_color = None;
                global.elytra_color_conflict = true;
                global.elytra_replay_color = None;
                global.elytra_replay_color_conflict = true;
            } else {
                global.cape_color = None;
                global.cape_color_conflict = true;
                global.cape_replay_color = None;
                global.cape_replay_color_conflict = true;
            }
        }
    });
}
pub(crate) fn invalidate_attachment_samples(tag: &crate::native_submit::Tag) {
    let _ = ACTORS.try_with(|active| {
        let Ok(mut active) = active.try_borrow_mut() else {
            return;
        };
        let Some(entry) = active.last_mut() else {
            return;
        };
        if !entry.ambiguous
            && entry.scope == tag.scope
            && crate::world_hooks::current_pass() == Some(tag.scope)
            && entry
                .observed
                .as_ref()
                .is_some_and(|global| global.actor_id == tag.actor)
        {
            entry.attachment_samples.invalidate();
        }
    });
}
pub(crate) fn record_replay_layer_color(
    scope: u64,
    actor: &str,
    mut receipt: crate::native_attachment_replay::CapeColorReceipt,
) -> bool {
    let wing = receipt.slot == crate::native_attachment_samples::WING_SLOT;
    if !wing && receipt.slot != crate::native_attachment_samples::CAPE_SLOT
        || wing && (receipt.native_cape.is_some() || receipt.geometry != "geometry.elytra")
        || !wing && receipt.native_cape.is_none()
    {
        return false;
    }
    ACTORS.try_with(|active| {
        let Ok(mut active)=active.try_borrow_mut() else{return false;};let Some(entry)=active.last_mut() else{return false;};
        if entry.ambiguous||entry.scope!=scope||crate::world_hooks::current_pass()!=Some(scope){return false;}
        let Some(global)=entry.observed.as_mut() else{return false;};
        let (source,source_conflict,replay_conflict,suppressed)=if wing {
            (&global.elytra_color,global.elytra_color_conflict,global.elytra_replay_color_conflict,global.elytra.is_some())
        } else {(&global.cape_color,global.cape_color_conflict,global.cape_replay_color_conflict,global.cape.is_some()||global.hidden_cape.is_some())};
        if global.actor_id!=actor||global.draw.is_none()||global.animation_frame!=receipt.animation_frame
            ||source_conflict||replay_conflict||suppressed
            ||!receipt.replay.valid()||receipt.source_rgba.iter().chain(&receipt.rgba).any(|v|!v.is_finite()||!(0. ..=1.).contains(v))
            ||!source.as_ref().is_some_and(|source|source.geometry==receipt.geometry&&source.animation_frame==receipt.animation_frame
                &&source.native_cape==receipt.native_cape&&source.rgba.map(f32::to_bits)==receipt.source_rgba.map(f32::to_bits))
            // The source draw is captured before either owned output field
            // changes. The replay receipt independently carries the checked
            // post-copy flag, just as its RGBA differs from source_rgba.
            ||source.as_ref().is_some_and(|source|source.source_dithering!=receipt.source_dithering||source.dithering_enabled!=receipt.source_dithering)
            ||receipt.shader_inputs.as_ref().is_some_and(|inputs|!inputs.valid())
            ||receipt.source_shader_inputs.as_ref().is_some_and(|inputs|!inputs.valid())
            ||source.as_ref().is_some_and(|source|source.shader_inputs!=receipt.source_shader_inputs)
            ||receipt.shader_input_draws>1||receipt.shader_input_copies!=0||receipt.shader_inputs_applied
        {return false;}
        let (slot,conflict)=if wing {(&mut global.elytra_replay_color,&mut global.elytra_replay_color_conflict)}
            else {(&mut global.cape_replay_color,&mut global.cape_replay_color_conflict)};
        if let Some(old)=slot.as_ref() {
            let draws=receipt.shader_input_draws;
            let mut comparable=receipt.clone();comparable.shader_input_draws=old.shader_input_draws;
            comparable.shader_input_copies=old.shader_input_copies;comparable.shader_inputs_applied=old.shader_inputs_applied;
            if old!=&comparable||old.rgba.map(f32::to_bits)!=receipt.rgba.map(f32::to_bits)
                ||old.source_rgba.map(f32::to_bits)!=receipt.source_rgba.map(f32::to_bits)
                ||old.shader_input_draws.checked_add(draws).is_none_or(|n|n>4096) {
                *slot=None;*conflict=true;return false;
            }
            receipt.shader_input_draws=old.shader_input_draws+draws;receipt.shader_input_copies=old.shader_input_copies;
            receipt.shader_inputs_applied=receipt.shader_input_draws>0&&receipt.shader_input_copies==receipt.shader_input_draws;
        }
        *slot=Some(receipt);true
    }).unwrap_or(false)
}
/// Every prepared description needs its own matching synchronous local copy.
/// A later successful draw cannot erase an earlier unconsumed description.
pub(crate) fn record_replay_layer_shader_inputs(
    tag: &crate::native_submit::Tag,
    frame: u64,
    replay: &crate::native_emote_history::Replay,
    source: &crate::native_shader_inputs::ActorInputs,
    wanted: &crate::native_shader_inputs::ActorInputs,
) -> bool {
    let Some(wing) = wing_layer(tag) else {
        return false;
    };
    ACTORS
        .try_with(|active| {
            let Ok(mut active) = active.try_borrow_mut() else {
                return false;
            };
            let Some(entry) = active.last_mut() else {
                return false;
            };
            if entry.ambiguous
                || entry.scope != tag.scope
                || crate::world_hooks::current_pass() != Some(tag.scope)
            {
                return false;
            }
            let Some(global) = entry
                .observed
                .as_mut()
                .filter(|global| global.actor_id == tag.actor)
            else {
                return false;
            };
            let (slot, source_conflict, replay_conflict) = if wing {
                (
                    &mut global.elytra_replay_color,
                    global.elytra_color_conflict,
                    global.elytra_replay_color_conflict,
                )
            } else {
                (
                    &mut global.cape_replay_color,
                    global.cape_color_conflict,
                    global.cape_replay_color_conflict,
                )
            };
            if source_conflict || replay_conflict {
                return false;
            }
            let Some(receipt) = slot.as_mut() else {
                return false;
            };
            if receipt.geometry != tag.geometry
                || receipt.animation_frame != frame
                || receipt.replay != *replay
                || !receipt
                    .source_shader_inputs
                    .as_ref()
                    .is_some_and(|actual| actual.same(source))
                || !receipt
                    .shader_inputs
                    .as_ref()
                    .is_some_and(|actual| actual.same(wanted))
                || receipt.shader_input_copies >= receipt.shader_input_draws
            {
                return false;
            }
            receipt.shader_input_copies += 1;
            receipt.shader_inputs_applied =
                receipt.shader_input_copies == receipt.shader_input_draws;
            true
        })
        .unwrap_or(false)
}
pub(crate) fn record_cape(cape: crate::native_capes::Replacement) -> bool {
    ACTORS
        .try_with(|s| {
            let Ok(mut s) = s.try_borrow_mut() else {
                return false;
            };
            let Some(entry) = s.last_mut() else {
                return false;
            };
            if entry.ambiguous || crate::world_hooks::current_pass() != Some(entry.scope) {
                return false;
            }
            let Some(global) = entry.observed.as_mut() else {
                return false;
            };
            if global.hidden_cape.is_some()
                || cape.animation_frame != global.animation_frame
                || !global
                    .skin_pose
                    .as_ref()
                    .is_some_and(|p| p.animation_frame == cape.animation_frame)
            {
                return false;
            }
            if let Some(old) = global.cape.as_mut() {
                if old.cape != cape.cape
                    || old.texture != cape.texture
                    || old.native_cape != cape.native_cape
                    || old.geometry != cape.geometry
                    || old.animation_frame != cape.animation_frame
                    || old.motion != cape.motion
                {
                    return false;
                }
                old.mesh_draws = old.mesh_draws.saturating_add(1);
            } else {
                global.cape = Some(cape);
            }
            true
        })
        .unwrap_or(false)
}
pub(crate) fn record_hidden_cape(cape: crate::native_capes::Hidden) -> bool {
    ACTORS
        .try_with(|s| {
            let Ok(mut s) = s.try_borrow_mut() else {
                return false;
            };
            let Some(entry) = s.last_mut() else {
                return false;
            };
            if entry.ambiguous || crate::world_hooks::current_pass() != Some(entry.scope) {
                return false;
            }
            let Some(global) = entry.observed.as_mut() else {
                return false;
            };
            if global.cape.is_some()
                || cape.animation_frame != global.animation_frame
                || !global
                    .skin_pose
                    .as_ref()
                    .is_some_and(|p| p.animation_frame == cape.animation_frame)
            {
                return false;
            }
            if let Some(old) = global.hidden_cape.as_mut() {
                if old.native_cape != cape.native_cape
                    || old.geometry != cape.geometry
                    || old.animation_frame != cape.animation_frame
                {
                    return false;
                }
                old.mesh_draws = old.mesh_draws.saturating_add(1);
            } else {
                global.hidden_cape = Some(cape);
            }
            true
        })
        .unwrap_or(false)
}
pub(crate) fn record_body(body: crate::native_body::Replacement) -> bool {
    ACTORS
        .try_with(|s| {
            let Ok(mut s) = s.try_borrow_mut() else {
                return false;
            };
            let Some(entry) = s.last_mut() else {
                return false;
            };
            if entry.ambiguous || crate::world_hooks::current_pass() != Some(entry.scope) {
                return false;
            }
            let Some(global) = entry.observed.as_mut() else {
                return false;
            };
            if body.animation_frame != global.animation_frame
                || !global.skin_pose.as_ref().is_some_and(|p| {
                    p.animation_frame == body.animation_frame && p.geometry == body.geometry
                })
            {
                return false;
            }
            if let Some(old) = global.body.as_mut() {
                if old.skin != body.skin
                    || old.slim != body.slim
                    || old.geometry != body.geometry
                    || old.animation_frame != body.animation_frame
                    || old.suppression_alpha.to_bits() != body.suppression_alpha.to_bits()
                    || old.suppression_method != body.suppression_method
                {
                    return false;
                }
                old.mesh_draws = old.mesh_draws.saturating_add(1);
            } else {
                global.body = Some(body);
            }
            if let Ok(mut last) = LAST_BODY.try_lock() {
                *last = Some(global.clone());
            }
            true
        })
        .unwrap_or(false)
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Draw {
    pub matrix: [f32; 16],
    pub view: [f32; 16],
    pub projection: [f32; 16],
    pub model_scale: f32,
    pub translation: [f32; 3],
    pub camera_target: [f32; 3],
}
struct ActorScope {
    id: u64,
    observation: u64,
    rejection_epoch: u64,
    draw_call: Option<draw_frame::Call>,
    actor: usize,
    context: usize,
    scope: u64,
    observed: Option<Global>,
    ambiguous: bool,
    skin_reference: Option<std::sync::Arc<crate::native_skin_selection::Reference>>,
    palette_writes: crate::native_palette_writer::Ledger,
    frame_uploads: crate::native_emote_history::FrameUploads,
    expression_bindings: crate::native_expression_history::Ledger,
    attachment_samples: crate::native_attachment_samples::Ledger,
}
/// A selected skin reference authorizes only native palette retargeting. It
/// never fills Global::skin_pose or the body/cape replacement receipts.
pub(crate) fn record_skin_reference(
    reference: std::sync::Arc<crate::native_skin_selection::Reference>,
) -> bool {
    if SKIPPED_ACTORS.with(Cell::get) != 0 || !draw_frame::synchronize() || !reference.valid() {
        return false;
    }
    ACTORS
        .try_with(|s| {
            let Ok(mut s) = s.try_borrow_mut() else {
                return false;
            };
            let Some(entry) = s.last_mut() else {
                return false;
            };
            if entry.ambiguous
                || crate::world_hooks::current_pass() != Some(entry.scope)
                || !reference.belongs(entry.actor, entry.context)
            {
                return false;
            }
            let Some(global) = entry.observed.as_mut() else {
                return false;
            };
            if global.draw.is_none()
                || global.emote_pose_conflict
                || global.animation_frame != reference.pose.animation_frame
            {
                return false;
            }
            if entry
                .skin_reference
                .as_ref()
                .is_some_and(|previous| previous.pose != reference.pose || !previous.valid())
                || global
                    .skin_pose
                    .as_ref()
                    .is_some_and(|skin| skin != &reference.pose)
            {
                global.emote_pose = None;
                global.emote_pose_conflict = true;
                return false;
            }
            if entry.skin_reference.is_none() {
                entry.skin_reference = Some(reference);
            }
            true
        })
        .unwrap_or(false)
}
fn emote_reference(entry: &ActorScope) -> Option<&crate::native_body::SkinPose> {
    let global = entry.observed.as_ref()?;
    if global.emote_pose_conflict {
        return None;
    }
    if let Some(reference) = &entry.skin_reference {
        if !reference.belongs(entry.actor, entry.context)
            || !reference.valid()
            || global
                .skin_pose
                .as_ref()
                .is_some_and(|skin| skin != &reference.pose)
        {
            return None;
        }
        Some(&reference.pose)
    } else {
        global.skin_pose.as_ref()
    }
}
pub(crate) fn current_draw(scope: u64, actor: &str) -> Option<Draw> {
    if SKIPPED_ACTORS.with(Cell::get) != 0 || !draw_frame::synchronize() {
        return None;
    }
    ACTORS
        .try_with(|s| {
            let s = s.try_borrow().ok()?;
            let entry = s.last()?;
            let global = entry.observed.as_ref()?;
            if entry.ambiguous
                || entry.scope != scope
                || global.actor_id != actor
                || crate::world_hooks::current_pass() != Some(scope)
            {
                return None;
            }
            global.draw.clone()
        })
        .ok()
        .flatten()
}
pub(crate) fn current_emote(scope: u64, actor: &str) -> Option<crate::native_emote_pose::Target> {
    if SKIPPED_ACTORS.with(Cell::get) != 0 || !draw_frame::synchronize() {
        return None;
    }
    ACTORS
        .try_with(|s| {
            let s = s.try_borrow().ok()?;
            let entry = s.last()?;
            let global = entry.observed.as_ref()?;
            let target = global.emote_pose.as_ref()?;
            if entry.ambiguous
                || entry.scope != scope
                || global.world_scope != scope
                || global.actor_id != actor
                || global.draw.is_none()
                || global.emote_pose_conflict
                || crate::world_hooks::current_pass() != Some(scope)
                || global.animation_frame != target.animation_frame
                || !emote_reference(entry).is_some_and(|skin| {
                    skin.animation_frame == target.animation_frame
                        && skin.parts == target.reference_parts
                })
            {
                return None;
            }
            Some(target.clone())
        })
        .ok()
        .flatten()
}
pub(crate) fn model_binding(model: usize, params: usize) -> Option<(usize, usize, Global)> {
    if SKIPPED_ACTORS.with(Cell::get) != 0 || !draw_frame::synchronize() {
        return None;
    }
    ACTORS
        .try_with(|s| {
            let s = s.try_borrow().ok()?;
            let active = s.last()?;
            if active.ambiguous || crate::world_hooks::current_pass() != Some(active.scope) {
                return None;
            }
            let component = pointer(
                active
                    .actor
                    .checked_add(crate::native_scene_layout::actor::ANIMATION_COMPONENT)?,
            )?;
            if params != component.checked_add(0x88)?
                || pointer(params)? != active.context
                || pointer(params.checked_add(0x10)?)? != component
                || pointer(params.checked_add(0x20)?)? != model
                || pointer(params.checked_add(0x38)?)? != active.actor
                || word(params.checked_add(0x78)?)? != 0
            {
                return None;
            }
            let observed = active.observed.as_ref()?;
            observed.draw.as_ref()?;
            if word(component.checked_add(0x368)?)? != observed.animation_frame {
                return None;
            }
            let data = pointer(params.checked_add(0xa0)?)?;
            let mut ui = [0];
            read(data.checked_add(0x2e)?, &mut ui).ok()?;
            if pointer(data)? != active.actor
                || pointer(data.checked_add(0x38)?)? != component
                || word(data.checked_add(8)?)? != 0
                || ui[0] != 0
            {
                return None;
            }
            Some((active.actor, active.context, observed.clone()))
        })
        .ok()
        .flatten()
}
pub(crate) fn palette_writer_scope() -> Option<crate::native_palette_writer::Stamp> {
    if !crate::native_body::enabled()
        || SKIPPED_ACTORS.with(Cell::get) != 0
        || !draw_frame::synchronize()
    {
        return None;
    }
    ACTORS
        .try_with(|active| {
            let active = active.try_borrow().ok()?;
            let entry = active.last()?;
            let global = entry.observed.as_ref()?;
            if entry.ambiguous
                || global.draw.is_none()
                || global.world_scope != entry.scope
                || crate::world_hooks::current_pass() != Some(entry.scope)
            {
                return None;
            }
            let component = pointer(
                entry
                    .actor
                    .checked_add(crate::native_scene_layout::actor::ANIMATION_COMPONENT)?,
            )?;
            if pointer(component.checked_add(0x88)?)? != entry.context
                || pointer(component.checked_add(0xc0)?)? != entry.actor
                || word(component.checked_add(0x368)?)? != global.animation_frame
            {
                return None;
            }
            Some(crate::native_palette_writer::Stamp {
                actor: entry.actor,
                context: entry.context,
                scope: entry.scope,
                frame: global.animation_frame,
                observation: entry.observation,
                actor_id: global.actor_id.clone(),
            })
        })
        .ok()
        .flatten()
}
/// The root binding remains strict. A separate reader can use this identity to
/// prove a child slot without accepting arbitrary item/UI RenderParams here.
pub(crate) fn attachment_parent() -> Option<(usize, usize, Global)> {
    let stamp = palette_writer_scope()?;
    let root = pointer(
        stamp
            .actor
            .checked_add(crate::native_scene_layout::actor::ANIMATION_COMPONENT)?,
    )?;
    let params = root.checked_add(0x88)?;
    let model = pointer(params.checked_add(0x20)?)?;
    let result = model_binding(model, params)?;
    if palette_writer_scope().as_ref() != Some(&stamp) {
        return None;
    }
    Some(result)
}
pub(crate) fn begin_attachment_setup(
    stamp: &crate::native_palette_writer::Stamp,
    slot: u32,
) -> Option<u64> {
    if palette_writer_scope().as_ref() != Some(stamp) {
        return None;
    }
    ACTORS
        .try_with(|active| {
            let mut active = active.try_borrow_mut().ok()?;
            let last = active.len().checked_sub(1)?;
            // Nested native rendering can reuse component/model storage. Outer
            // selections must not remain capabilities merely because values match.
            for entry in &mut active[..last] {
                entry.expression_bindings.invalidate();
            }
            active[last].expression_bindings.begin(slot)
        })
        .ok()
        .flatten()
}
pub(crate) fn invalidate_attachment_setup(actor: usize, slot: Option<u32>) {
    let _ = ACTORS.try_with(|active| {
        let Ok(mut active) = active.try_borrow_mut() else {
            return;
        };
        let last = active.len().checked_sub(1);
        for (index, entry) in active.iter_mut().enumerate() {
            if Some(index) == last
                && entry.actor == actor
                && let Some(slot) = slot.filter(|slot| *slot < 7)
            {
                entry.expression_bindings.invalidate_slot(slot);
            } else {
                entry.expression_bindings.invalidate();
            }
        }
    });
}
pub(crate) fn finish_attachment_setup(
    stamp: &crate::native_palette_writer::Stamp,
    slot: u32,
    serial: u64,
    child: usize,
    model: usize,
    frame: u64,
    reload: u64,
    sources: Vec<crate::native_expression_history::Source>,
) -> bool {
    if palette_writer_scope().as_ref() != Some(stamp) {
        return false;
    }
    ACTORS
        .try_with(|active| {
            let Ok(mut active) = active.try_borrow_mut() else {
                return false;
            };
            let Some(entry) = active.last_mut() else {
                return false;
            };
            entry
                .expression_bindings
                .finish(slot, serial, child, model, frame, reload, sources)
        })
        .unwrap_or(false)
}
pub(crate) fn attachment_setup_frame(child: usize, model: usize, reload: u64) -> Option<u64> {
    let stamp = palette_writer_scope()?;
    let frame = ACTORS
        .try_with(|active| {
            active
                .try_borrow()
                .ok()?
                .last()?
                .expression_bindings
                .frame(child, model, reload)
        })
        .ok()
        .flatten()?;
    (palette_writer_scope().as_ref() == Some(&stamp)).then_some(frame)
}
pub(crate) fn attachment_setup_serial(child: usize, model: usize, reload: u64) -> Option<u64> {
    let stamp = palette_writer_scope()?;
    let serial = ACTORS
        .try_with(|active| {
            active
                .try_borrow()
                .ok()?
                .last()?
                .expression_bindings
                .serial(child, model, reload)
        })
        .ok()
        .flatten()?;
    (palette_writer_scope().as_ref() == Some(&stamp)).then_some(serial)
}
pub(crate) fn expression_binding(
    child: usize,
    model: usize,
    reload: u64,
    frame: u64,
    hierarchy: u32,
    bone: u32,
    expression: usize,
) -> Option<crate::native_expression_history::Source> {
    let stamp = palette_writer_scope()?;
    let source = ACTORS
        .try_with(|active| {
            active
                .try_borrow()
                .ok()?
                .last()?
                .expression_bindings
                .source(child, model, reload, frame, hierarchy, bone, expression)
        })
        .ok()
        .flatten()?;
    (palette_writer_scope().as_ref() == Some(&stamp)).then_some(source)
}
/// Copy the current native model space, without applying a transform. Child
/// attachments need their own space receipt instead of inheriting the skin's.
pub(crate) fn draw_space(context: usize) -> Option<[[f32; 16]; 3]> {
    let screen = pointer(context.checked_add(0x28)?)?;
    let camera = pointer(screen.checked_add(0x18)?)?;
    let result = [
        stack_top(camera.checked_add(0x40)?)?,
        stack_top(camera)?,
        stack_top(camera.checked_add(0x80)?)?,
    ];
    if pointer(context.checked_add(0x28)?)? != screen
        || pointer(screen.checked_add(0x18)?)? != camera
    {
        return None;
    }
    Some(result)
}
pub(crate) fn record_palette_write(
    stamp: &crate::native_palette_writer::Stamp,
    record: Option<crate::native_palette_writer::Record>,
) -> bool {
    if palette_writer_scope().as_ref() != Some(stamp) {
        return false;
    }
    ACTORS
        .try_with(|active| {
            let Ok(mut active) = active.try_borrow_mut() else {
                return false;
            };
            let Some(entry) = active.last_mut() else {
                return false;
            };
            entry.palette_writes.record(record)
        })
        .unwrap_or(false)
}
/// Uploads in a nested or unidentified actor scope cannot leave another scope's
/// old per-part result eligible for later native cache pairing.
pub(crate) fn invalidate_other_frame_uploads(stamp: Option<&crate::native_palette_writer::Stamp>) {
    let _ = ACTORS.try_with(|active| {
        let Ok(mut active) = active.try_borrow_mut() else {
            return;
        };
        let last = active.len().checked_sub(1);
        for (index, entry) in active.iter_mut().enumerate() {
            let current = Some(index) == last
                && stamp.is_some_and(|stamp| {
                    entry.actor == stamp.actor
                        && entry.context == stamp.context
                        && entry.scope == stamp.scope
                        && entry.observed.as_ref().is_some_and(|g| {
                            g.animation_frame == stamp.frame && g.actor_id == stamp.actor_id
                        })
                });
            if !current {
                entry.frame_uploads.invalidate();
            }
        }
    });
}
pub(crate) fn invalidate_frame_uploads(stamp: &crate::native_palette_writer::Stamp) {
    if palette_writer_scope().as_ref() != Some(stamp) {
        return;
    }
    let _ = ACTORS.try_with(|active| {
        let Ok(mut active) = active.try_borrow_mut() else {
            return;
        };
        if let Some(entry) = active.last_mut() {
            entry.frame_uploads.invalidate();
        }
    });
}
pub(crate) fn invalidate_owner_frame_uploads(owner: usize) {
    let _ = ACTORS.try_with(|active| {
        let Ok(mut active) = active.try_borrow_mut() else {
            return;
        };
        for entry in active.iter_mut().filter(|entry| entry.actor == owner) {
            entry.frame_uploads.invalidate();
        }
    });
}
fn attachment_ledger(
    stamp: &crate::native_palette_writer::Stamp,
    tag: &crate::native_submit::Tag,
    action: impl FnOnce(&mut crate::native_attachment_samples::Ledger) -> bool,
) -> bool {
    if palette_writer_scope().as_ref() != Some(stamp)
        || stamp.scope != tag.scope
        || stamp.actor_id != tag.actor
        || crate::world_hooks::current_pass() != Some(stamp.scope)
    {
        return false;
    }
    ACTORS
        .try_with(|active| {
            let Ok(mut active) = active.try_borrow_mut() else {
                return false;
            };
            let Some(entry) = active.last_mut() else {
                return false;
            };
            if entry.ambiguous
                || entry.actor != stamp.actor
                || entry.context != stamp.context
                || entry.scope != stamp.scope
                || !entry.observed.as_ref().is_some_and(|global| {
                    global.actor_id == tag.actor
                        && global.animation_frame == stamp.frame
                        && global.draw.is_some()
                })
            {
                return false;
            }
            action(&mut entry.attachment_samples)
        })
        .unwrap_or(false)
}
pub(crate) fn begin_attachment_sample(
    stamp: &crate::native_palette_writer::Stamp,
    tag: &crate::native_submit::Tag,
    named: &crate::native_palette_binding::Named,
    native: &[[f32; 16]],
    pending: bool,
) -> bool {
    if named.attachment_slot.is_none() {
        return true;
    }
    let sample = crate::native_attachment_samples::Palette::copy(named, &tag.geometry, native);
    attachment_ledger(stamp, tag, |ledger| match sample {
        Some(sample) => ledger.begin(stamp.frame, sample, pending),
        None => {
            ledger.invalidate();
            false
        }
    })
}
pub(crate) fn finish_attachment_sample(
    stamp: &crate::native_palette_writer::Stamp,
    tag: &crate::native_submit::Tag,
    named: &crate::native_palette_binding::Named,
    native: &[[f32; 16]],
    supplied: &[[f32; 16]],
    replay: Option<crate::native_emote_history::Replay>,
) -> bool {
    if named.attachment_slot.is_none() {
        return true;
    }
    let reference = crate::native_attachment_samples::Palette::copy(named, &tag.geometry, native);
    let supplied = crate::native_attachment_samples::Palette::copy(named, &tag.geometry, supplied)
        .map(|mut palette| {
            palette.replay = replay;
            palette
        });
    attachment_ledger(stamp, tag, |ledger| match (reference, supplied) {
        (Some(reference), Some(supplied)) => ledger.supply(stamp.frame, &reference, supplied),
        _ => {
            ledger.invalidate();
            false
        }
    })
}
pub(crate) fn record_frame_uploads(
    stamp: &crate::native_palette_writer::Stamp,
    named: &crate::native_palette_binding::Named,
    native: &[[f32; 16]],
    supplied: &[[f32; 16]],
    emote: Option<crate::native_emote_history::Emote>,
    replay: Option<crate::native_emote_history::Replay>,
) -> bool {
    if palette_writer_scope().as_ref() != Some(stamp) {
        return false;
    }
    let Some(owner) = named.cache_owner.as_ref() else {
        return false;
    };
    ACTORS
        .try_with(|active| {
            let Ok(mut active) = active.try_borrow_mut() else {
                return false;
            };
            let Some(entry) = active.last_mut() else {
                return false;
            };
            if native.is_empty()
                || native.len() > 8
                || named.bones.len() != native.len()
                || supplied.len() != native.len()
            {
                entry.frame_uploads.invalidate();
                return false;
            }
            for (index, bone) in named.bones.iter().enumerate() {
                if !entry.frame_uploads.record(
                    owner.clone(),
                    bone.part,
                    bone.cache.clone(),
                    crate::native_emote_history::Pose {
                        native: native[index],
                        supplied: supplied[index],
                        emote: emote.clone(),
                        replay: replay.clone(),
                    },
                ) {
                    return false;
                }
            }
            true
        })
        .unwrap_or(false)
}
/// Returns owned values only. The scalar part identity is never dereferenced.
pub(crate) fn frame_upload(
    stamp: &crate::native_palette_writer::Stamp,
    owner: &crate::native_cache_binding::Identity,
    part: usize,
    slot: &crate::native_emote_history::Slot,
    native: &[f32; 16],
) -> Option<crate::native_emote_history::Pose> {
    if palette_writer_scope().as_ref() != Some(stamp) {
        return None;
    }
    let pose = ACTORS
        .try_with(|active| {
            active
                .try_borrow()
                .ok()?
                .last()?
                .frame_uploads
                .get(owner, part, slot, native)
        })
        .ok()
        .flatten()?;
    (palette_writer_scope().as_ref() == Some(stamp)).then_some(pose)
}
pub(crate) fn palette_write_scopes_active() -> bool {
    crate::native_body::enabled()
        && ACTORS
            .try_with(|active| active.try_borrow().is_ok_and(|active| !active.is_empty()))
            .unwrap_or(false)
}
pub(crate) fn invalidate_shared_palette_writes(
    stamp: Option<&crate::native_palette_writer::Stamp>,
    key: Option<(usize, usize)>,
) {
    let _ = ACTORS.try_with(|active| {
        let Ok(mut active) = active.try_borrow_mut() else {
            return;
        };
        let last = active.len().checked_sub(1);
        for (index, entry) in active.iter_mut().enumerate() {
            let current = key.is_some()
                && Some(index) == last
                && stamp.is_some_and(|stamp| {
                    entry.actor == stamp.actor
                        && entry.context == stamp.context
                        && entry.scope == stamp.scope
                        && entry.observed.as_ref().is_some_and(|g| {
                            g.animation_frame == stamp.frame && g.actor_id == stamp.actor_id
                        })
                });
            if !current {
                entry.palette_writes.invalidate(key);
            }
        }
    });
}
pub(crate) fn previous_palette_is_current(
    tag: &crate::native_submit::Tag,
    frame: u64,
    current: usize,
    previous: usize,
    count: usize,
) -> bool {
    let Some(stamp) = palette_writer_scope() else {
        return false;
    };
    if stamp.scope != tag.scope || stamp.actor_id != tag.actor || stamp.frame != frame {
        return false;
    }
    let accepted = ACTORS
        .try_with(|active| {
            let Ok(active) = active.try_borrow() else {
                return false;
            };
            let Some(entry) = active.last() else {
                return false;
            };
            entry.palette_writes.proves(current, previous, count)
        })
        .unwrap_or(false);
    accepted && palette_writer_scope().as_ref() == Some(&stamp)
}
pub(crate) fn previous_palette(
    tag: &crate::native_submit::Tag,
    frame: u64,
    current: usize,
    previous: usize,
    count: usize,
) -> Option<Vec<crate::native_palette_writer::Prior>> {
    let stamp = palette_writer_scope()?;
    if stamp.scope != tag.scope || stamp.actor_id != tag.actor || stamp.frame != frame {
        return None;
    }
    let entries = ACTORS
        .try_with(|active| {
            active
                .try_borrow()
                .ok()?
                .last()?
                .palette_writes
                .previous(current, previous, count)
        })
        .ok()
        .flatten()?;
    (palette_writer_scope().as_ref() == Some(&stamp)).then_some(entries)
}
thread_local! {
    static ACTORS:RefCell<Vec<ActorScope>>=const {RefCell::new(Vec::new())};
    static SKIPPED_ACTORS: Cell<usize> = const { Cell::new(0) };
}
pub(crate) struct Guard {
    pushed: bool,
    skipped: bool,
}
impl Guard {
    fn skipped() -> Self {
        SKIPPED_ACTORS.with(|skipped| skipped.set(skipped.get().saturating_add(1)));
        Self {
            pushed: false,
            skipped: true,
        }
    }
    pub(crate) fn finish(mut self) -> Option<Global> {
        if !self.pushed {
            return None;
        }
        if !draw_frame::synchronize() {
            return None;
        }
        let mut entry = ACTORS
            .try_with(|s| s.try_borrow_mut().ok()?.pop())
            .ok()
            .flatten()?;
        // Disarm only after removal; a failed borrow must leave Drop able to
        // clean up the scope and its native palette metadata.
        self.pushed = false;
        if entry.observed.as_ref().is_some_and(|g| g.body.is_some()) {
            BODY_FINISHED.fetch_add(1, Ordering::Relaxed);
            if entry.ambiguous {
                BODY_REJECTED.fetch_add(1, Ordering::Relaxed);
            }
        }
        if entry.observed.as_ref().is_some_and(|g| g.cape.is_some()) {
            CAPE_FINISHED.fetch_add(1, Ordering::Relaxed);
            if entry.ambiguous {
                CAPE_REJECTED.fetch_add(1, Ordering::Relaxed);
            }
        }
        if entry.ambiguous || entry.draw_call.is_some() {
            return None;
        }
        let mut global = entry.observed?;
        if global.emote_pose_conflict || !crate::native_body::enabled() {
            entry.attachment_samples.invalidate();
        }
        global.attachment_samples = Some(entry.attachment_samples.finish(global.animation_frame));
        Some(global)
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        if self.pushed {
            let _ = ACTORS.try_with(|s| s.try_borrow_mut().ok().and_then(|mut s| s.pop()));
        }
        if self.skipped {
            SKIPPED_ACTORS.with(|skipped| skipped.set(skipped.get().saturating_sub(1)));
        }
    }
}
pub(crate) fn begin_actor(actor: usize, context: usize) -> Guard {
    if !ENABLED.load(Ordering::Acquire) {
        return Guard::skipped();
    }
    let Some(scope) = crate::world_hooks::current_pass() else {
        return Guard::skipped();
    };
    let Some(id) = observation_serial() else {
        return Guard::skipped();
    };
    let rejection_epoch = draw_frame::rejection_epoch();
    if rejection_epoch == 0 {
        return Guard::skipped();
    }
    let pushed = ACTORS
        .try_with(|s| {
            let Ok(mut s) = s.try_borrow_mut() else {
                return false;
            };
            if s.len() >= 8 {
                return false;
            }
            s.push(ActorScope {
                id,
                observation: id,
                rejection_epoch,
                draw_call: None,
                actor,
                context,
                scope,
                observed: None,
                ambiguous: false,
                skin_reference: None,
                palette_writes: crate::native_palette_writer::Ledger::default(),
                frame_uploads: crate::native_emote_history::FrameUploads::default(),
                expression_bindings: crate::native_expression_history::Ledger::default(),
                attachment_samples: crate::native_attachment_samples::Ledger::new(
                    crate::native_body::enabled(),
                ),
            });
            true
        })
        .unwrap_or(false);
    if pushed {
        Guard {
            pushed: true,
            skipped: false,
        }
    } else {
        Guard::skipped()
    }
}

fn word(address: usize) -> Option<u64> {
    let mut bytes = [0u8; 8];
    read(address, &mut bytes).ok()?;
    Some(u64::from_le_bytes(bytes))
}
fn renderer_entries(map: usize) -> Option<Vec<usize>> {
    let head = pointer(map.checked_add(8)?)?;
    let count = word(map.checked_add(16)?)? as usize;
    if count > 4096 {
        return None;
    }
    let mut node = pointer(head)?;
    let mut previous = head;
    let mut seen = HashSet::new();
    let mut found = Vec::new();
    for _ in 0..count {
        if node == head || !seen.insert(node) || pointer(node.checked_add(8)?)? != previous {
            return None;
        }
        // list links (16), HashedString (48), shared_ptr<ActorRenderer> (16).
        let name = crate::players::string(node.checked_add(24)?, 128)?;
        if let Ok(mut names) = MAP_NAMES.try_lock()
            && names.len() < 24
            && !names.contains(&name)
        {
            names.push(name.clone());
        }
        if matches!(name.as_str(), "player" | "minecraft:player") {
            found.push(pointer(node.checked_add(64)?)?);
        }
        previous = node;
        node = pointer(node)?;
    }
    if node != head
        || pointer(head.checked_add(8)?)? != previous
        || pointer(map.checked_add(8)?)? != head
        || word(map.checked_add(16)?)? as usize != count
    {
        return None;
    }
    Some(found)
}
pub(crate) fn observe_renderer(dispatcher: usize) {
    if RENDERER.get().is_some() {
        return;
    }
    MAP_ATTEMPTS.fetch_add(1, Ordering::Relaxed);
    let sample = || -> Option<Renderer> {
        let base = crate::world_hooks::image_base()?;
        let binding = BINDING.get()?;
        let mut matches = Vec::new();
        for offset in [8usize, 0x48] {
            for object in renderer_entries(dispatcher.checked_add(offset)?)? {
                let table = pointer(object)?;
                let render = pointer(table.checked_add(16)?)?;
                let table_rva = table.checked_sub(base)?;
                let render_rva = render.checked_sub(base)?;
                if table_rva != binding.table_rva || render_rva != binding.renderer_rva {
                    return None;
                }
                if !matches.contains(&(table_rva, render_rva)) {
                    matches.push((table_rva, render_rva));
                }
            }
        }
        if matches.len() != 1 {
            return None;
        }
        Some(Renderer {
            table_rva: matches[0].0,
            render_rva: matches[0].1,
        })
    };
    if let Some(renderer) = sample() {
        let _ = RENDERER.set(renderer);
    }
}

fn affine(matrix: &[f32; 16]) -> bool {
    matrix
        .iter()
        .all(|v| v.is_finite() && v.abs() <= 32_000_000.)
        && [3, 7, 11].iter().all(|i| matrix[*i].abs() < 0.0001)
        && (matrix[15] - 1.).abs() < 0.0001
}
fn actor_observation(data: usize, context: usize) -> Option<Global> {
    STEP.store(0, Ordering::Relaxed);
    STEP.store(1, Ordering::Relaxed);
    ACTORS
        .try_with(|active| {
            let active = active.try_borrow().ok()?;
            let entry = active.last()?;
            STEP.store(2, Ordering::Relaxed);
            if entry.context != context
                || crate::world_hooks::current_pass() != Some(entry.scope)
                || pointer(data)? != entry.actor
                || word(data.checked_add(8)?)? != 0
            {
                return None;
            }
            STEP.store(3, Ordering::Relaxed);
            let mut ui = [0];
            read(data.checked_add(0x2e)?, &mut ui).ok()?;
            if ui[0] != 0 {
                return None;
            }
            STEP.store(4, Ordering::Relaxed);
            let component = pointer(
                entry
                    .actor
                    .checked_add(crate::native_scene_layout::actor::ANIMATION_COMPONENT)?,
            )?;
            if pointer(data.checked_add(0x38)?)? != component
                || pointer(component.checked_add(0x88)?)? != context
                || pointer(component.checked_add(0xc0)?)? != entry.actor
            {
                return None;
            }
            STEP.store(5, Ordering::Relaxed);
            let actor = crate::players::actor(entry.actor)?;
            STEP.store(6, Ordering::Relaxed);
            if CALLERS.get().is_none() {
                let _ = CALLERS.set(crate::world_hooks::game_callers());
            }
            Some(Global {
                actor_id: actor.identity.id,
                world_scope: entry.scope,
                animation_frame: word(component.checked_add(0x368)?)?,
                before: None,
                after: None,
                observation: BINDING.get()?.observation,
                draw: None,
                body: None,
                cape: None,
                hidden_cape: None,
                skin_pose: None,
                skin_color: None,
                skin_color_conflict: false,
                cape_color: None,
                cape_color_conflict: false,
                cape_replay_color: None,
                cape_replay_color_conflict: false,
                elytra_color: None,
                elytra_color_conflict: false,
                elytra_replay_color: None,
                elytra_replay_color_conflict: false,
                emote_pose: None,
                emote_pose_conflict: false,
                attachment_samples: None,
                wing_pose: None,
                elytra: None,
            })
        })
        .ok()
        .flatten()
}
fn observation(data: usize, context: usize, before: [f32; 16], after: [f32; 16]) -> Option<Global> {
    if !affine(&before)
        || !affine(&after)
        || BINDING.get()?.observation != TransformObservation::HelperThenDraw
    {
        return None;
    }
    let mut value = actor_observation(data, context)?;
    value.before = Some(before);
    value.after = Some(after);
    Some(value)
}
unsafe extern "system" fn global_hook(data: usize, matrix: usize, context: usize) {
    CALLS.fetch_add(1, Ordering::Relaxed);
    let before = ENABLED
        .load(Ordering::Acquire)
        .then(|| floats::<16>(matrix, 32_000_000.))
        .flatten();
    // The selected SDK method is void(ActorRenderData*, Matrix&, Context&).
    // Only a verified exact-image profile can enable this pass-through hook.
    unsafe {
        let original: GlobalFn = std::mem::transmute(ORIGINAL.load(Ordering::Acquire));
        original(data, matrix, context);
    }
    let _ = std::panic::catch_unwind(|| {
        let Some(value) = before
            .zip(floats::<16>(matrix, 32_000_000.))
            .and_then(|(before, after)| observation(data, context, before, after))
        else {
            return;
        };
        COPIES.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut last) = LAST.try_lock() {
            *last = Some(value.clone());
        }
        let _ = ACTORS.try_with(|s| {
            let Ok(mut s) = s.try_borrow_mut() else {
                return;
            };
            let Some(entry) = s.last_mut() else {
                return;
            };
            if let Some(old) = &entry.observed {
                if old.actor_id != value.actor_id
                    || old.animation_frame != value.animation_frame
                    || old.observation != value.observation
                    || !old.after.zip(value.after).is_some_and(|(old, value)| {
                        old.iter().zip(value).all(|(a, b)| (a - b).abs() <= 0.0001)
                    })
                {
                    entry.ambiguous = true;
                }
            } else {
                entry.observed = Some(value);
            }
        });
    });
}

fn stack_top(stack: usize) -> Option<[f32; 16]> {
    draw_matrices::stack_top(stack)
}
fn draw_observation(
    model: usize,
    renderer: usize,
    data: usize,
    translation: usize,
    camera_target: usize,
) -> Option<draw_frame::Guard> {
    DRAW_STEP.store(0, Ordering::Relaxed);
    if !ENABLED.load(Ordering::Acquire) {
        return None;
    }
    let binding = BINDING.get()?;
    let context = ACTORS
        .try_with(|s| s.try_borrow().ok()?.last().map(|e| e.context))
        .ok()??;
    // Reuse the actor, native component, UI, item and world-scope checks.
    let identity = actor_observation(data, context)?;
    DRAW_STEP.store(1, Ordering::Relaxed);
    let selected = RENDERER.get()?;
    let base = crate::world_hooks::image_base()?;
    let renderer = pointer(renderer)?;
    let table = pointer(renderer)?;
    if selected.render_rva != binding.renderer_rva
        || selected.table_rva != binding.table_rva
        || table.checked_sub(base)? != binding.table_rva
        || pointer(table.checked_add(16)?)?.checked_sub(base)? != binding.renderer_rva
    {
        return None;
    }
    DRAW_STEP.store(2, Ordering::Relaxed);
    let component = pointer(data.checked_add(0x38)?)?;
    // RenderParams::mDataDrivenModel is its native weak_ptr at +0x20.
    if pointer(component.checked_add(0xa8)?)? != model {
        return None;
    }
    DRAW_STEP.store(3, Ordering::Relaxed);
    let matrices = draw_matrices::copy(context)?;
    let value = Draw {
        matrix: matrices.model,
        view: matrices.view,
        projection: matrices.projection,
        model_scale: floats::<1>(component.checked_add(0x198)?, 16.)?[0],
        translation: floats(translation, 32_000_000.)?,
        camera_target: floats(camera_target, 32_000_000.)?,
    };
    DRAW_STEP.store(4, Ordering::Relaxed);
    if !affine(&value.matrix)
        || !affine(&value.view)
        || value.model_scale <= 0.
        || value.model_scale > 1.
    {
        return None;
    }
    let checked = actor_observation(data, context)?;
    if checked.actor_id != identity.actor_id
        || checked.animation_frame != identity.animation_frame
        || checked.world_scope != identity.world_scope
    {
        return None;
    }
    // Own cleanup before publishing a call identity or allocating its Global.
    let mut boundary = draw_frame::Guard::new();
    ACTORS
        .try_with(|s| {
            let mut s = s.try_borrow_mut().ok()?;
            let entry = s.last_mut()?;
            if entry.ambiguous
                || entry.context != context
                || entry.scope != identity.world_scope
                || crate::world_hooks::current_pass() != Some(identity.world_scope)
            {
                return None;
            }
            // The exact draw boundary supplies this family's model/view/
            // projection matrices without requiring a diagnostic helper.
            if binding.observation == TransformObservation::DrawEntry {
                if entry.observed.is_some() {
                    entry.ambiguous = true;
                    return None;
                }
                boundary.bind(
                    entry.id,
                    &mut entry.draw_call,
                    draw_frame::Call::new(data, model, component),
                )?;
                entry.observed = Some(identity.clone());
            }
            let observed = entry.observed.as_mut()?;
            if entry.ambiguous
                || observed.draw.is_some()
                || observed.actor_id != identity.actor_id
                || observed.animation_frame != identity.animation_frame
                || observed.world_scope != identity.world_scope
            {
                entry.ambiguous = true;
                return None;
            }
            observed.draw = Some(value.clone());
            if let Ok(mut last) = LAST.try_lock() {
                *last = Some(observed.clone());
            }
            Some(())
        })
        .ok()??;
    DRAW_STEP.store(5, Ordering::Relaxed);
    Some(boundary)
}
#[cfg(test)]
const IDENTITY: [f32; 16] = [
    1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
];
unsafe extern "system" fn draw_hook(
    model: usize,
    renderer: usize,
    data: usize,
    translation: usize,
    camera_target: usize,
) {
    let _boundary = std::panic::catch_unwind(|| {
        let boundary = draw_observation(model, renderer, data, translation, camera_target)?;
        DRAW_COPIES.fetch_add(1, Ordering::Relaxed);
        Some(boundary)
    })
    .ok()
    .flatten();
    unsafe {
        let original: DrawFn = std::mem::transmute(DRAW_ORIGINAL.load(Ordering::Acquire));
        original(model, renderer, data, translation, camera_target);
    }
}

/// Fixed, typed observers for the independently hashed installed executable.
/// No supplied RVA, code bytes, or late-binding request is accepted.
pub(crate) fn install() -> Result<(), String> {
    INSTALL.get_or_init(||{
        let base=crate::world_hooks::image_base().ok_or("unvalidated native image")?;
        let binding=crate::world_hooks::transform_binding()?;
        let mut hooks:Vec<(usize,*const (),&AtomicUsize,&[u8])>=Vec::new();
        if let Some(rva)=binding.global_rva{
            hooks.push((rva,global_hook as *const (),&ORIGINAL,binding.global_prefix()));
        }
        hooks.push((binding.draw_rva,draw_hook as *const (),&DRAW_ORIGINAL,binding.draw_prefix()));
        let mut targets=Vec::with_capacity(hooks.len());
        // Preflight the complete selected group before creating any detour.
        for(rva,_,_,expected)in &hooks{
            if expected.is_empty(){return Err("native transform prefix missing".into());}
            let address=base.checked_add(*rva).ok_or("native transform address overflow")?;
            let mut actual=vec![0;expected.len()];read(address,&mut actual)?;
            if actual.as_slice()!=*expected{return Err(format!("native transform {rva:x} prologue differs"));}
            targets.push(address);
        }
        BINDING.set(binding).map_err(|_|"native transform binding already published")?;
        use hudhook::mh::{MH_CreateHook,MH_EnableHook,MH_DisableHook,MH_STATUS};
        for(i,(rva,callback,slot,_))in hooks.iter().enumerate(){
            let mut original=std::ptr::null_mut();
            let result=unsafe{MH_CreateHook(targets[i] as _,*callback as _,&mut original)};
            if result!=MH_STATUS::MH_OK{return Err(format!("create native transform {rva:x}: {result:?}"));}
            slot.store(original as usize,Ordering::Release);
        }
        // Every original is published before callbacks can run. Observations
        // remain disabled until all selected detours have been enabled.
        for(i,target)in targets.iter().enumerate(){
            let result=unsafe{MH_EnableHook(*target as _)};
            if result!=MH_STATUS::MH_OK{
                let mut failures=Vec::new();
                for old in targets[..i].iter().rev(){
                    let rollback=unsafe{MH_DisableHook(*old as _)};
                    if rollback!=MH_STATUS::MH_OK{failures.push(format!("{old:x}: {rollback:?}"));}
                }
                return Err(format!("enable native transform {target:x}: {result:?}; rollback failures: {failures:?}"));
            }
        }
        TARGET.store(binding.global_rva.unwrap_or(binding.draw_rva),Ordering::Release);
        ENABLED.store(true,Ordering::Release);Ok(())
    }).clone()
}
pub(crate) fn status() -> Value {
    json!({"rendererMapAttempts":MAP_ATTEMPTS.load(Ordering::Relaxed),"renderer":RENDERER.get(),
        "installed":ENABLED.load(Ordering::Acquire),"hookRva":TARGET.load(Ordering::Acquire),
        "drawHookRva":BINDING.get().map(|b|b.draw_rva),"globalHookRva":BINDING.get().and_then(|b|b.global_rva),
        "observationStrategy":BINDING.get().map(|b|b.observation),"drawEntryObservationSourceImplemented":true,
        "drawFrame":draw_frame::status(),
        "drawMatrices":draw_matrices::status(),
        "drawCopies":DRAW_COPIES.load(Ordering::Relaxed),"drawStep":DRAW_STEP.load(Ordering::Relaxed),"calls":CALLS.load(Ordering::Relaxed),"copies":COPIES.load(Ordering::Relaxed),
        "last":LAST.try_lock().ok().and_then(|v|v.clone()),
        "bodyFinished":BODY_FINISHED.load(Ordering::Relaxed),"bodyRejected":BODY_REJECTED.load(Ordering::Relaxed),
        "capeFinished":CAPE_FINISHED.load(Ordering::Relaxed),"capeRejected":CAPE_REJECTED.load(Ordering::Relaxed),
        "lastBody":LAST_BODY.try_lock().ok().and_then(|v|v.clone()),
        "observationStep":STEP.load(Ordering::Relaxed),"callers":CALLERS.get(),"rendererNames":MAP_NAMES.try_lock().ok().map(|v|v.clone()),"transformsVerified":false})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matrix_stack_uses_wrapped_back_and_rejects_invalid_ring_headers() {
        let matrix = IDENTITY;
        let mut slots = [0usize; 8];
        slots[0] = matrix.as_ptr() as usize;
        let mut deque = [0usize, slots.as_ptr() as usize, 8, 7, 2];
        let sample = |deque: &[usize; 5], slots: &[usize; 8]| {
            std::hint::black_box(slots);
            stack_top(deque.as_ptr() as usize)
        };
        assert_eq!(sample(&deque, &slots), Some(matrix));
        deque[4] = 0;
        assert!(sample(&deque, &slots).is_none());
        deque[4] = 9;
        assert!(sample(&deque, &slots).is_none());
        deque[4] = 2;
        deque[2] = 7;
        assert!(sample(&deque, &slots).is_none());
        deque[2] = 8;
        slots[0] = 0;
        assert!(sample(&deque, &slots).is_none());
    }
    #[test]
    fn model_draw_observer_forwards_the_fifth_stack_argument_once() {
        static ARGUMENTS: Mutex<Vec<[usize; 5]>> = Mutex::new(Vec::new());
        unsafe extern "system" fn original(a: usize, b: usize, c: usize, d: usize, e: usize) {
            ARGUMENTS.lock().unwrap().push([a, b, c, d, e]);
        }
        DRAW_ORIGINAL.store(original as *const () as usize, Ordering::Release);
        unsafe {
            draw_hook(1, 2, 3, 4, 0x12345678);
        }
        assert_eq!(*ARGUMENTS.lock().unwrap(), vec![[1, 2, 3, 4, 0x12345678]]);
    }
    #[test]
    fn matrix_observer_preserves_all_arguments_and_native_output() {
        static ARGUMENTS: Mutex<Option<(usize, usize)>> = Mutex::new(None);
        unsafe extern "system" fn original(data: usize, matrix: usize, context: usize) {
            *ARGUMENTS.lock().unwrap() = Some((data, context));
            unsafe {
                *(matrix as *mut f32) = 7.;
            }
        }
        ORIGINAL.store(original as *const () as usize, Ordering::Release);
        let mut matrix = [0f32; 16];
        unsafe {
            global_hook(0x1234, matrix.as_mut_ptr() as usize, 0x9876);
        }
        assert_eq!(matrix[0], 7.);
        assert_eq!(*ARGUMENTS.lock().unwrap(), Some((0x1234, 0x9876)));
        assert_eq!(COPIES.load(Ordering::Relaxed), 0);
    }
    #[test]
    fn renderer_map_checks_links_count_and_name_before_copying_object() {
        let mut head = [0usize; 2];
        let mut node = [0usize; 10];
        let mut map = [0usize; 8];
        let h = head.as_ptr() as usize;
        let n = node.as_ptr() as usize;
        head[0] = n;
        head[1] = n;
        node[0] = h;
        node[1] = h;
        let bytes = unsafe { std::slice::from_raw_parts_mut(node.as_mut_ptr().cast::<u8>(), 80) };
        bytes[24..30].copy_from_slice(b"player");
        bytes[40..48].copy_from_slice(&6usize.to_le_bytes());
        bytes[48..56].copy_from_slice(&15usize.to_le_bytes());
        bytes[64..72].copy_from_slice(&0x123456usize.to_le_bytes());
        map[1] = h;
        map[2] = 1;
        let sample = || {
            std::hint::black_box(&head);
            std::hint::black_box(&node);
            std::hint::black_box(&map);
            renderer_entries(map.as_ptr() as usize)
        };
        assert_eq!(sample(), Some(vec![0x123456]));
        map[2] = 2;
        std::hint::black_box(&map);
        assert!(renderer_entries(map.as_ptr() as usize).is_none());
    }
}
