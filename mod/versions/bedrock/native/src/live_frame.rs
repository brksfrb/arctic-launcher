//! Build shared world/geometry input from one observed pass. Software rendering
//! separately requires its actual GPU depth. Native bodies stay enabled unless a scoped replacement receipt
//! attests suppression. Actual standard skin draws supply their selected native
//! matrices; the standing observation remains a restricted legacy fallback.
use crate::players::Actor;
use crate::world_hooks::Observation;
use serde_json::{Value, json};
use std::collections::HashSet;

fn interpolate(old: f32, current: f32, partial: f32) -> f32 {
    old + partial * (current - old)
}
fn angle(old: f32, current: f32, partial: f32) -> f32 {
    old + partial * ((current - old + 180.).rem_euclid(360.) - 180.)
}
fn multiply(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    std::array::from_fn(|index| {
        let row = index % 4;
        let column = index / 4;
        (0..4).map(|k| a[k * 4 + row] * b[column * 4 + k]).sum()
    })
}
/// GameRenderer retains OpenGL clip Z (-W..W); the installed BGFX views use
/// DirectX clip Z (0..W). Convert the entire row, including off-axis terms.
pub(crate) fn directx_clip_projection(native: &[f32; 16]) -> [f32; 16] {
    let mut result = *native;
    for column in 0..4 {
        result[column * 4 + 2] = (native[column * 4 + 2] + native[column * 4 + 3]) * 0.5;
    }
    result
}
/// The shared packet's forward-depth camera matrix, using precisely the same
/// float operation order as the live world packet producer.
pub(crate) fn packet_projection(
    camera: &crate::world_hooks::CameraSample,
    reverse_z: bool,
) -> [f32; 16] {
    let mut projection = multiply(&directx_clip_projection(&camera.projection), &camera.view);
    if reverse_z {
        for column in 0..4 {
            projection[column * 4 + 2] = projection[column * 4 + 3] - projection[column * 4 + 2];
        }
    }
    projection
}
/// Native model vertices and cached bone translations use pixels. The shared
/// avatar uses blocks, so convert the three basis columns and retain the native
/// camera-relative translation. Never reconstruct scale from an actor's AABB.
fn native_root(
    global: &crate::native_transform::Global,
    camera: &crate::world_hooks::CameraSample,
) -> Option<[f32; 12]> {
    let draw = global.draw.as_ref()?;
    let same = |a: &[f32; 16], b: &[f32; 16]| {
        a.iter()
            .zip(b)
            .all(|(a, b)| a.is_finite() && b.is_finite() && (a - b).abs() < 0.0001)
    };
    if !same(&draw.view, &camera.view)
        || !(same(&draw.projection, &camera.projection)
            || same(
                &draw.projection,
                &directx_clip_projection(&camera.projection),
            ))
    {
        return None;
    }
    let m = &draw.matrix;
    if m.iter().any(|v| !v.is_finite() || v.abs() > 32_000_000.)
        || [3, 7, 11].iter().any(|i| m[*i].abs() > 0.0001)
        || (m[15] - 1.).abs() > 0.0001
    {
        return None;
    }
    Some(std::array::from_fn(|i| {
        let row = i / 4;
        let column = i % 4;
        m[column * 4 + row] * if column == 3 { 1. } else { 16. }
    }))
}
fn flag(flags: &[u64; 3], bit: usize) -> bool {
    flags[bit / 64] & (1u64 << (bit % 64)) != 0
}

pub(crate) fn player(
    actor: &Actor,
    origin: &[f32; 3],
    partial: f32,
    rendered: bool,
    pose: Option<&crate::animation_pose::Pose>,
) -> Option<Value> {
    let components = actor.components.as_ref()?;
    if components.id != actor.identity.id {
        return None;
    }
    let bounds = components.bounds;
    let feet: [f32; 3] = std::array::from_fn(|i| {
        let center = if i == 1 {
            bounds[1]
        } else {
            (bounds[i] + bounds[i + 3]) * 0.5
        };
        interpolate(actor.old_position[i], actor.position[i], partial) + center - actor.position[i]
    });
    let body = angle(components.body_yaw[1], components.body_yaw[0], partial);
    let head = angle(components.head_yaw[1], components.head_yaw[0], partial);
    let pitch = angle(actor.old_rotation[0], actor.rotation[0], partial);
    let (sine, cosine) = body.to_radians().sin_cos();
    let height = bounds[4] - bounds[1];
    let width = bounds[3] - bounds[0];
    let scale = height / 1.8;
    let standing = !components.dead
        && (0.25..=4.).contains(&scale)
        && (width / 0.6 - scale).abs() < 0.02
        && [1, 2, 11, 32, 57, 64, 76, 114]
            .iter()
            .all(|bit| !flag(&components.flags, *bit));
    let matrices = standing
        .then(|| {
            pose.and_then(|pose| {
                crate::animation_pose::standing_parts(
                    pose,
                    pitch.to_radians(),
                    (head - body).to_radians(),
                )
            })
        })
        .flatten();
    let available = if matrices.is_some() {
        63
    } else if standing {
        3
    } else {
        0
    };
    let root = [
        cosine * scale,
        0.,
        sine * scale,
        feet[0] - origin[0],
        0.,
        -scale,
        0.,
        feet[1] - origin[1] + 1.5 * scale,
        sine * scale,
        0.,
        -cosine * scale,
        feet[2] - origin[2],
    ];
    let mut rotations = [[0f32; 3]; 6];
    rotations[0] = [pitch.to_radians(), (head - body).to_radians(), 0.];
    let cape_absent = actor
        .appearance
        .as_ref()
        .is_some_and(|a| a.cape_absent && a.cape.is_none());
    let render_cape = matrices.is_some() && cape_absent && actor.chest_empty == Some(true);
    Some(json!({"id":actor.identity.id,"name":actor.identity.name,
        "position":[feet[0],feet[1],feet[2],head,pitch],"model":root,"modelSource":"bounds-estimate","partRotations":rotations,
        "partMatrices":matrices,"availableParts":available,"replaceBody":false,"replaceCape":render_cape,"nativeCapeAbsent":cape_absent,"chestEmpty":actor.chest_empty,
        "skin":actor.appearance.as_ref().map(|a|&a.skin),"cape":actor.appearance.as_ref().and_then(|a|a.cape.as_ref()),
        "nativeSkin":actor.appearance.as_ref().map(|a|&a.skin),"nativeSlim":actor.appearance.as_ref().map(|a|a.slim),
        "nativeCape":actor.appearance.as_ref().and_then(|a|a.cape.as_ref()),"nativeCapeSuppressed":false,"nativeCapeHidden":false,
        "elytra":flag(&components.flags,32),
        "chestItem":actor.chest_item,
        "equipment":actor.equipment,
        "elytraEquipped":if actor.chest_empty==Some(true) {Some(false)} else {actor.chest_item.as_deref().map(|id|id=="minecraft:elytra")},
        "movement":actor.movement,
        "javaLayerVisibility":{"schema":1,"spectator":components.spectator,"invisible":components.invisible(),"showCape":null},
        "visible":rendered&&available!=0&&!components.invisible(),"slim":actor.appearance.as_ref().is_some_and(|a|a.slim)}))
}

pub(crate) fn build(
    observation: &Observation,
    width: u32,
    height: u32,
) -> Option<(String, Vec<f32>)> {
    let depth = observation.depth.as_ref()?;
    let json = geometry_input(observation, width, height, depth.reverse_z)?;
    Some((json, depth.downsample(width, height).ok()?))
}
/// Owned native geometry input without a CPU terrain image. The actual bound
/// surface producer supplies its depth convention independently of readback.
pub(crate) fn geometry_input(
    observation: &Observation,
    width: u32,
    height: u32,
    reverse_z: bool,
) -> Option<String> {
    if !(1..=3840).contains(&width) || !(1..=2160).contains(&height) {
        return None;
    }
    let camera = observation.camera.as_ref()?;
    let partial = camera.partial?;
    let tps = camera.ticks_per_second?;
    let elapsed = camera.client_elapsed_ms?;
    let epoch = camera.epoch?;
    // Camera matrices are camera-relative in this renderer. Keep model roots
    // relative as well, avoiding loss of precision from large world translations.
    let projection = packet_projection(camera, reverse_z);
    if camera.projection[11].abs() < 0.5 || camera.projection[5].abs() < 0.0001 {
        return None;
    }
    let fov = 2. * (1. / camera.projection[5].abs()).atan().to_degrees();
    if !(1.0..=179.).contains(&fov) {
        return None;
    }
    let direction = [camera.view[2], camera.view[6], camera.view[10]]
        .map(|v| v * camera.projection[11].signum());
    let length = direction.iter().map(|v| v * v).sum::<f32>().sqrt();
    if length < 0.001 {
        return None;
    }
    let yaw = (-direction[0]).atan2(direction[2]).to_degrees();
    let pitch = -(direction[1] / length).clamp(-1., 1.).asin().to_degrees();
    let mut ids = HashSet::new();
    let mut players = Vec::new();
    for sample in &observation.actors {
        let Some(actor) = sample.player.as_ref() else {
            continue;
        };
        if ids.contains(&actor.identity.id) {
            continue;
        }
        if let Some(mut player) = player(
            actor,
            &camera.origin,
            partial,
            true,
            sample.animation.as_ref(),
        ) {
            if let Some(motion) = &sample.cape_motion {
                player["capeMotion"] = serde_json::to_value(motion).ok()?;
            }
            if let Some(state) = &sample.cape_state {
                player["capeState"] = serde_json::to_value(state).ok()?;
            }
            if let Some(global) = &sample.global_model {
                player["nativeGlobal"] = serde_json::to_value(global).ok()?;
            }
            let root = sample.global_model.as_ref().and_then(|global| {
                (global.actor_id == actor.identity.id && global.world_scope == observation.scope())
                    .then(|| native_root(global, camera))
                    .flatten()
            });
            if let Some(root) = root {
                player["model"] = json!(root);
                player["modelSource"] = json!("native-model-draw");
                if let Some(pose) = sample
                    .global_model
                    .as_ref()
                    .and_then(|g| g.skin_pose.as_ref())
                    && sample
                        .animation
                        .as_ref()
                        .is_some_and(|p| p.update_frame == pose.animation_frame)
                    && actor
                        .appearance
                        .as_ref()
                        .is_some_and(|a| a.skin == pose.native_skin && a.slim == pose.native_slim)
                {
                    // These matrices came from the selected skin geometry in
                    // this very draw. They include posture/attack translations
                    // and rotations; do not replace them with standing pivots
                    // or rescale them using the actor's changing AABB height.
                    player["partMatrices"] = json!(pose.parts);
                    player["partRotations"] = Value::Null;
                    player["availableParts"] = json!(63);
                    if let Some(global) = sample.global_model.as_ref()
                        && !global.skin_color_conflict
                    {
                        player["nativeSkinColor"] = json!(global.skin_color);
                    }
                    if let Some(global) = sample.global_model.as_ref()
                        && !global.emote_pose_conflict
                        && let Some(target) = global.emote_pose.as_ref()
                        && target.animation_frame == pose.animation_frame
                        && target.reference_parts == pose.parts
                    {
                        player["emotePose"] = serde_json::to_value(target).ok()?;
                    }
                    if let Some(samples) = sample
                        .global_model
                        .as_ref()
                        .and_then(|global| global.attachment_samples.as_ref())
                        && samples.animation_frame == pose.animation_frame
                    {
                        player["attachmentPoses"] = serde_json::to_value(samples).ok()?;
                    }
                    player["visible"] =
                        json!(actor.components.as_ref().is_some_and(|c| !c.invisible()));
                    player["replaceCape"] = json!(
                        actor
                            .appearance
                            .as_ref()
                            .is_some_and(|a| a.cape_absent && a.cape.is_none())
                            && actor.chest_empty == Some(true)
                    );
                }
                if let Some(body) = sample.global_model.as_ref().and_then(|g| g.body.as_ref())
                    && player["availableParts"] == 63
                    && sample
                        .animation
                        .as_ref()
                        .is_some_and(|p| p.update_frame == body.animation_frame)
                {
                    player["replaceBody"] = json!(true);
                    player["skin"] = json!(body.skin);
                    player["slim"] = json!(body.slim);
                }
                if let Some(global) = sample.global_model.as_ref()
                    && !global.cape_color_conflict
                    && let Some(color) = global.cape_color.as_ref()
                    && player["availableParts"] == 63
                    && actor.appearance.as_ref().and_then(|a| a.cape.as_ref())
                        == color.native_cape.as_ref()
                    && color.native_cape.is_some()
                    && sample
                        .animation
                        .as_ref()
                        .is_some_and(|p| p.update_frame == color.animation_frame)
                {
                    player["nativeCapeDrawColor"] = serde_json::to_value(color).ok()?;
                }
                if let Some(global) = sample.global_model.as_ref()
                    && !global.cape_color_conflict
                    && !global.cape_replay_color_conflict
                    && global.cape.is_none()
                    && global.hidden_cape.is_none()
                    && let Some(color) = global.cape_replay_color.as_ref()
                    && player["availableParts"] == 63
                    && actor.appearance.as_ref().and_then(|a| a.cape.as_ref())
                        == color.native_cape.as_ref()
                    && color.native_cape.is_some()
                    && color.slot == crate::native_attachment_samples::CAPE_SLOT
                    && sample
                        .animation
                        .as_ref()
                        .is_some_and(|pose| pose.update_frame == color.animation_frame)
                {
                    player["nativeCapeReplayColor"] = serde_json::to_value(color).ok()?;
                }
                if let Some(pose) = sample
                    .global_model
                    .as_ref()
                    .and_then(|g| g.wing_pose.as_ref())
                    && actor.chest_item.as_deref() == Some("minecraft:elytra")
                    && sample
                        .animation
                        .as_ref()
                        .is_some_and(|p| p.update_frame == pose.animation_frame)
                {
                    // Observation only. Native wings remain visible unless a
                    // separate owned replacement receipt is present below.
                    player["wingMatrices"] = json!(pose.wings);
                    if let Some(global) = sample.global_model.as_ref()
                        && !global.elytra_color_conflict
                        && let Some(color) = global.elytra_color.as_ref()
                        && player["availableParts"] == 63
                        && color.geometry == pose.geometry
                        && color.animation_frame == pose.animation_frame
                    {
                        player["nativeWingDrawColor"] = serde_json::to_value(color).ok()?;
                    }
                    if let Some(global) = sample.global_model.as_ref()
                        && !global.elytra_color_conflict
                        && !global.elytra_replay_color_conflict
                        && global.elytra.is_none()
                        && let Some(color) = global.elytra_replay_color.as_ref()
                        && player["availableParts"] == 63
                        && color.slot == crate::native_attachment_samples::WING_SLOT
                        && color.native_cape.is_none()
                        && color.geometry == pose.geometry
                        && color.animation_frame == pose.animation_frame
                    {
                        player["nativeWingReplayColor"] = serde_json::to_value(color).ok()?;
                    }
                }
                if let Some(elytra) = sample.global_model.as_ref().and_then(|g| g.elytra.as_ref())
                    && player["availableParts"] == 63
                    && actor.chest_item.as_deref() == Some("minecraft:elytra")
                    && sample
                        .animation
                        .as_ref()
                        .is_some_and(|p| p.update_frame == elytra.animation_frame)
                    && sample
                        .global_model
                        .as_ref()
                        .and_then(|g| g.wing_pose.as_ref())
                        .is_some_and(|p| {
                            p.geometry == elytra.geometry
                                && p.animation_frame == elytra.animation_frame
                                && p.wings == elytra.wings
                        })
                {
                    player["replaceElytra"] = json!(true);
                    player["wingHash"] = json!(elytra.cape);
                    player["wingTexture"] = json!(elytra.texture);
                    player["wingMatrices"] = json!(elytra.wings);
                }
                if let Some(cape) = sample.global_model.as_ref().and_then(|g| g.cape.as_ref())
                    && player["availableParts"] == 63
                    && actor.chest_empty == Some(true)
                    && actor.appearance.as_ref().and_then(|a| a.cape.as_deref())
                        == Some(cape.native_cape.as_str())
                    && sample
                        .animation
                        .as_ref()
                        .is_some_and(|p| p.update_frame == cape.animation_frame)
                {
                    player["replaceCape"] = json!(true);
                    player["nativeCapeSuppressed"] = json!(true);
                    player["capeHash"] = json!(cape.cape);
                    player["cape"] = json!(cape.texture);
                    player["capeMotion"] = serde_json::to_value(&cape.motion).ok()?;
                }
                if let Some(cape) = sample
                    .global_model
                    .as_ref()
                    .and_then(|g| g.hidden_cape.as_ref())
                    && player["availableParts"] == 63
                    && player["replaceElytra"] != true
                    && actor.chest_empty == Some(true)
                    && actor.appearance.as_ref().and_then(|a| a.cape.as_deref())
                        == Some(cape.native_cape.as_str())
                    && sample
                        .animation
                        .as_ref()
                        .is_some_and(|p| p.update_frame == cape.animation_frame)
                {
                    player["replaceCape"] = json!(false);
                    player["nativeCapeHidden"] = json!(true);
                    player["cape"] = Value::Null;
                    player["capeHash"] = Value::Null;
                }
            } else {
                // A diagnostic bounds estimate must never become a fallback
                // attachment when the actual draw or its matching camera fails.
                player["visible"] = json!(false);
                player["replaceCape"] = json!(false);
                player["availableParts"] = json!(0);
            }
            ids.insert(actor.identity.id.clone());
            players.push(player);
        }
    }
    if let Some(actor) = camera.local_actor.as_ref()
        && !ids.contains(&actor.identity.id)
        && let Some(mut player) = player(actor, &camera.origin, partial, false, None)
    {
        player["capeMotion"] = serde_json::to_value(&camera.local_cape_motion).ok()?;
        player["capeState"] = serde_json::to_value(&camera.local_cape_state).ok()?;
        ids.insert(actor.identity.id.clone());
        players.push(player);
    }
    let local = camera.local_player.as_ref().filter(|id| ids.contains(*id));
    let dimension = local
        .and_then(|id| {
            let mut copies = observation
                .actors
                .iter()
                .filter_map(|sample| sample.player.as_ref())
                .chain(camera.local_actor.iter())
                .filter(|actor| &actor.identity.id == id);
            let first = copies.next()?.dimension?;
            copies
                .all(|actor| actor.dimension == Some(first))
                .then_some(first)
        })
        .unwrap_or("unknown");
    // Integrate completed ticks at their observed rates. Dividing all previous
    // ticks by today's TPS would retroactively jump emotes whenever TPS changes.
    let time = (elapsed + partial as f64 / tps as f64 * 1000.).floor();
    if !time.is_finite() || !(0.0..=i64::MAX as f64).contains(&time) {
        return None;
    }
    let session = crate::world_hooks::native_session(epoch);
    let controls = json!({
        "worldWork":camera.renderer_timing.as_ref().and_then(|v|v.get("worldWork")),
        "packetDispatch":camera.client_ticks.and_then(|tick|crate::native_packet::capture_sample(&session,tick)),
        "clock":camera.renderer_timing.as_ref().and_then(|v|v.get("captureClock")),
        "cameraTime":camera.renderer_timing.as_ref().and_then(|v|v.get("cameraTime")),
        "camera":crate::render_bridge::producer_for_scope(observation.scope())
            .and_then(|key|crate::native_camera_control::capture_sample(&session,key))});
    let constants = crate::native_gpu_constants::frame_sample(
        observation.scope(),
        &session,
        observation.sequence,
    );
    let json = json!({"schema":1,"session":session,"sequence":observation.sequence,
        "clockMs":time as u64,"nativeTiming":{"ticks":camera.client_ticks,"elapsedMs":elapsed,"partial":partial,"ticksPerSecond":tps,"rendererClock":camera.render_clock,"rendererState":camera.renderer_timing.as_ref(),"realTimer":camera.real_timer.as_ref(),"cameraComponent":camera.native_camera.as_ref(),"submittedShaderConstants":constants,"captureControls":controls},
        "dayTime":-1,"width":width,"height":height,"dimension":dimension,"server":null,
        "itemResources":camera.resource_stack.as_ref().map(crate::native_resource_packs::Snapshot::publish),
        "localPlayer":local,"viewProjection":projection,"cameraRelative":true,
        "projectionContext":{"type":"perspective","clipDepth":"negative_one_to_one",
            "view":camera.view,"matrix":camera.projection,"reverseZ":reverse_z},
        "camera":[camera.origin[0],camera.origin[1],camera.origin[2],yaw,pitch,fov],"players":players});
    Some(json.to_string())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::players::{Components, Identity};
    #[test]
    fn installed_model_draw_preserves_native_pixel_positions_and_rejects_foreign_camera() {
        let fixture: Value =
            serde_json::from_str(include_str!("../inspection/native-transform.json")).unwrap();
        let mut global: crate::native_transform::Global =
            serde_json::from_value(fixture["global"].clone()).unwrap();
        let mut camera = crate::world_hooks::CameraSample {
            resource_stack: None,
            render_clock: None,
            renderer_timing: None,
            real_timer: None,
            native_camera: None,
            origin: [0.; 3],
            view: std::array::from_fn(|i| fixture["camera"]["view"][i].as_f64().unwrap() as f32),
            projection: std::array::from_fn(|i| {
                fixture["camera"]["projection"][i].as_f64().unwrap() as f32
            }),
            partial: None,
            ticks_per_second: None,
            client_ticks: None,
            client_elapsed_ms: None,
            local_player: None,
            local_actor: None,
            local_cape_motion: None,
            local_cape_state: None,
            epoch: None,
        };
        let root = native_root(&global, &camera).unwrap();
        let native = &global.draw.as_ref().unwrap().matrix;
        // Compare actual native pixel vertices with the shared avatar's block
        // vertices, including both sides of the head and the bottom of a leg.
        for pixel in [[-4., -8., -4.], [4., 0., 4.], [-1.9, 24., 0.]] {
            for row in 0..3 {
                let native_position =
                    native[12 + row] + (0..3).map(|c| native[c * 4 + row] * pixel[c]).sum::<f32>();
                let avatar_position = root[row * 4 + 3]
                    + (0..3)
                        .map(|c| root[row * 4 + c] * pixel[c] / 16.)
                        .sum::<f32>();
                assert!((native_position - avatar_position).abs() < 0.000001);
            }
        }
        assert!((root[4..7].iter().map(|v| v * v).sum::<f32>().sqrt() - 0.9375).abs() < 0.00001);
        assert!((root[7] - native[13]).abs() < 0.000001);
        camera.view[12] += 0.01;
        assert!(native_root(&global, &camera).is_none());
        camera.view[12] -= 0.01;
        global.draw.as_mut().unwrap().matrix[3] = 0.5;
        assert!(native_root(&global, &camera).is_none());
    }
    #[test]
    fn installed_camera_projection_converts_to_the_observed_gpu_depth_range() {
        let fixture: Value =
            serde_json::from_str(include_str!("../inspection/world-projection.json")).unwrap();
        let matrix = |name: &str| -> [f32; 16] {
            std::array::from_fn(|i| fixture[name][i].as_f64().unwrap() as f32)
        };
        let source = matrix("native");
        let gpu = matrix("gpu");
        let view = matrix("view");
        let converted = directx_clip_projection(&source);
        for (a, b) in converted.iter().zip(gpu) {
            assert!((a - b).abs() < 0.000001);
        }
        for (a, b) in multiply(&converted, &view)
            .iter()
            .zip(multiply(&gpu, &view))
        {
            assert!((a - b).abs() < 0.000001);
        }
        let project = |distance: f32| {
            (converted[14] - converted[10] * distance) / (-converted[11] * distance)
        };
        assert!(project(0.025).abs() < 0.00001);
        assert!((project(2500.) - 1.).abs() < 0.00001);
        let mut off_axis = source;
        off_axis[3] = 0.2;
        off_axis[7] = 0.4;
        let d = directx_clip_projection(&off_axis);
        assert_eq!(d[2], 0.1);
        assert_eq!(d[6], 0.2);
    }
    pub(crate) fn fixture_actor() -> Actor {
        Actor {
            identity: Identity::fixture("b50ad385-829d-3141-a216-7e7d7539ba7f", "ArcticTest"),
            dimension: None,
            position: [100., 81.62, 200.],
            old_position: [98., 81.62, 200.],
            velocity: [0.; 3],
            rotation: [20., 90.],
            old_rotation: [0., 90.],
            components: Some(Components {
                id: "b50ad385-829d-3141-a216-7e7d7539ba7f".into(),
                body_yaw: [90., 90.],
                head_yaw: [110., 90.],
                on_ground: true,
                dead: false,
                spectator: None,
                flags: [0; 3],
                bounds: [99.7, 80., 199.7, 100.3, 81.8, 200.3],
            }),
            appearance: None,
            chest_empty: None,
            equipment: None,
            chest_item: None,
            movement: None,
        }
    }
    #[test]
    fn recorded_six_part_pose_reaches_world_input_only_with_matching_standing_actor() {
        let pose = serde_json::from_str(include_str!("../inspection/standing-pose.json")).unwrap();
        let mut actor = fixture_actor();
        actor.rotation = [0.; 2];
        actor.old_rotation = [0.; 2];
        let c = actor.components.as_mut().unwrap();
        c.body_yaw = [50.; 2];
        c.head_yaw = [0.; 2];
        let p = player(&actor, &[0.; 3], 0.5, true, Some(&pose)).unwrap();
        assert_eq!(p["availableParts"], 63);
        assert_eq!(p["partMatrices"].as_array().unwrap().len(), 6);
        assert_eq!(p["replaceBody"], false);
        assert_eq!(p["replaceCape"], false);
        actor.appearance = Some(crate::native_skins::Appearance {
            skin: "engine:skin/test".into(),
            geometry: "geometry.humanoid.custom".into(),
            cape: None,
            slim: false,
            cape_absent: true,
        });
        actor.chest_empty = Some(true);
        assert_eq!(
            player(&actor, &[0.; 3], 0.5, true, Some(&pose)).unwrap()["replaceCape"],
            true
        );
        actor.chest_empty = None;
        assert_eq!(
            player(&actor, &[0.; 3], 0.5, true, Some(&pose)).unwrap()["replaceCape"],
            false
        );
        actor.chest_empty = Some(true);
        actor.appearance.as_mut().unwrap().cape = Some("engine:cape/test".into());
        assert_eq!(
            player(&actor, &[0.; 3], 0.5, true, Some(&pose)).unwrap()["replaceCape"],
            false
        );
        actor.appearance = None;
        actor.components.as_mut().unwrap().head_yaw = [20.; 2];
        let mismatch = player(&actor, &[0.; 3], 0.5, true, Some(&pose)).unwrap();
        assert_eq!(mismatch["availableParts"], 3);
        assert!(mismatch["partMatrices"].is_null());
        actor.components.as_mut().unwrap().flags[0] |= 1 << 1;
        let crouch = player(&actor, &[0.; 3], 0.5, true, Some(&pose)).unwrap();
        assert_eq!(crouch["availableParts"], 0);
        assert!(crouch["partMatrices"].is_null());
    }
    #[test]
    fn standing_attachment_transform_uses_native_feet_head_look_and_camera_relative_origin() {
        let mut actor = fixture_actor();
        let p = player(&actor, &[96., 80., 195.], 0.5, true, None).unwrap();
        assert_eq!(p["availableParts"], 3);
        assert_eq!(p["replaceBody"], false);
        assert_eq!(p["replaceCape"], false);
        let root = p["model"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap() as f32)
            .collect::<Vec<_>>();
        assert!((root[3] - 3.).abs() < 0.0001);
        assert!((root[7] - 1.5).abs() < 0.0001);
        assert_eq!(root[11], 5.);
        // A Java model faces -Z: native yaw 90 must face world -X.
        assert!((-root[2] + 1.).abs() < 0.0001);
        assert!(root[10].abs() < 0.0001);
        assert!((p["partRotations"][0][0].as_f64().unwrap() - 10f64.to_radians()).abs() < 0.00001);
        actor.components.as_mut().unwrap().flags[0] |= 1 << 1;
        assert_eq!(
            player(&actor, &[0.; 3], 0.5, true, None).unwrap()["availableParts"],
            0
        );
        assert_eq!(
            player(&actor, &[0.; 3], 0.5, false, None).unwrap()["visible"],
            false
        );
    }
}
