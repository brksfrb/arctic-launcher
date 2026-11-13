//! Owned CPU snapshots retained until their exact GPU scope arrives.
use super::{ActorSample, CameraSample, Observation};
use std::collections::VecDeque;
use std::time::Duration;

const MAX_FRAMES: usize = 32;
const MAX_BYTES: usize = 32 * 1024 * 1024;
pub(super) const MAX_AGE: Duration = Duration::from_millis(500);

fn actor_bytes(actor: &crate::players::Actor) -> usize {
    actor.identity.id.capacity()
        + actor.chest_item.as_ref().map_or(0, String::capacity)
        + actor.equipment.as_ref().map_or(0, |equipment| {
            equipment
                .armor
                .iter()
                .flat_map(|slots| slots.iter())
                .chain(equipment.hands.iter().flat_map(|slots| slots.iter()))
                .map(|slot| {
                    slot.item.as_ref().map_or(0, String::capacity)
                        + slot.icon_key.as_ref().map_or(0, String::capacity)
                })
                .sum::<usize>()
                + equipment.chest.as_ref().map_or(0, |slot| {
                    slot.item.as_ref().map_or(0, String::capacity)
                        + slot.icon_key.as_ref().map_or(0, String::capacity)
                })
        })
        + actor.identity.name.capacity()
        + actor.components.as_ref().map_or(0, |c| c.id.capacity())
        + actor.appearance.as_ref().map_or(0, |a| {
            a.skin.capacity() + a.cape.as_ref().map_or(0, String::capacity)
        })
}
fn sample_bytes(sample: &ActorSample) -> usize {
    sample.player.as_ref().map_or(0, actor_bytes)
        + sample.global_model.as_ref().map_or(0, |g| {
            g.actor_id.capacity()
                + g.body.as_ref().map_or(0, |body| {
                    body.skin.capacity()
                        + body.geometry.capacity()
                        + body.suppression_method.capacity()
                })
                + g.cape.as_ref().map_or(0, |cape| {
                    cape.cape.capacity()
                        + cape.texture.capacity()
                        + cape.native_cape.capacity()
                        + cape.geometry.capacity()
                })
                + g.skin_pose.as_ref().map_or(0, |pose| {
                    pose.geometry.capacity() + pose.native_skin.capacity()
                })
                + g.emote_pose
                    .as_ref()
                    .map_or(0, |pose| pose.token.capacity())
                + g.wing_pose
                    .as_ref()
                    .map_or(0, |pose| pose.geometry.capacity())
                + g.elytra.as_ref().map_or(0, |elytra| {
                    elytra.cape.capacity() + elytra.texture.capacity() + elytra.geometry.capacity()
                })
        })
        + sample.animation.as_ref().map_or(0, |pose| {
            pose.hierarchies.capacity() * std::mem::size_of::<crate::animation_pose::Hierarchy>()
                + pose
                    .hierarchies
                    .iter()
                    .map(|h| {
                        h.bones.capacity() * std::mem::size_of::<crate::animation_pose::Bone>()
                            + h.bones.iter().map(|b| b.name.capacity()).sum::<usize>()
                    })
                    .sum::<usize>()
        })
}
fn camera_bytes(camera: &CameraSample) -> usize {
    camera.local_player.as_ref().map_or(0, String::capacity)
        + camera.local_actor.as_ref().map_or(0, actor_bytes)
        + camera
            .resource_stack
            .as_ref()
            .map_or(0, crate::native_resource_packs::Snapshot::heap_bytes)
}
fn owned_bytes(frame: &Observation) -> usize {
    // Native pointers are never retained. Count all owned vector/string
    // capacities; GPU depth has a separate single-image bound below.
    std::mem::size_of::<Observation>()
        + frame.actors.capacity() * std::mem::size_of::<ActorSample>()
        + frame.actors.iter().map(sample_bytes).sum::<usize>()
        + frame.camera.as_ref().map_or(0, camera_bytes)
}
#[derive(Default)]
pub(super) struct History {
    frames: VecDeque<(Observation, usize)>,
    bytes: usize,
    epoch: Option<u64>,
    selected: Option<u64>,
    evicted: u64,
    rejected: u64,
}
impl History {
    pub(super) fn latest(&self) -> Option<&Observation> {
        self.frames.back().map(|f| &f.0)
    }
    fn pop(&mut self) {
        if let Some((old, bytes)) = self.frames.pop_front() {
            self.bytes -= bytes;
            if self.selected == Some(old.scope) {
                self.selected = None;
            }
            self.evicted += 1;
        }
    }
    pub(super) fn push(&mut self, frame: Observation) {
        let epoch = frame.camera.as_ref().and_then(|c| c.epoch);
        if epoch != self.epoch || epoch.is_none() {
            self.frames.clear();
            self.bytes = 0;
            self.selected = None;
            self.epoch = epoch;
        }
        while self
            .frames
            .front()
            .is_some_and(|f| f.0.at.elapsed() >= MAX_AGE)
        {
            self.pop();
        }
        let bytes = owned_bytes(&frame);
        if bytes > MAX_BYTES {
            self.frames.clear();
            self.bytes = 0;
            self.selected = None;
            self.rejected += 1;
            return;
        }
        self.bytes += bytes;
        self.frames.push_back((frame, bytes));
        while self.frames.len() > MAX_FRAMES || self.bytes > MAX_BYTES {
            self.pop();
        }
    }
    pub(super) fn get(&self, scope: u64) -> Option<&Observation> {
        self.frames.iter().map(|f| &f.0).find(|f| {
            f.scope == scope
                && f.at.elapsed() < MAX_AGE
                && self.epoch.is_some()
                && f.camera.as_ref().and_then(|c| c.epoch) == self.epoch
        })
    }
    pub(super) fn selected(&self) -> Option<&Observation> {
        self.get(self.selected?).filter(|f| f.depth.is_some())
    }
    pub(super) fn begin_frame(&mut self) {
        self.selected = None;
        for (frame, _) in &mut self.frames {
            frame.depth = None;
        }
    }
    pub(super) fn accept(&mut self, scope: u64, depth: crate::depth::Depth) -> bool {
        if self.get(scope).is_none()
            || depth.width == 0
            || depth.height == 0
            || depth.width as u64 * depth.height as u64 != depth.values.len() as u64
            || depth.values.len() > 64 * 1024 * 1024
        {
            return false;
        }
        // At most one native depth image survives; a queue of full-resolution
        // depth copies must not multiply GPU readback memory by MAX_FRAMES.
        for (frame, _) in &mut self.frames {
            frame.depth = None;
        }
        self.frames
            .iter_mut()
            .find(|f| f.0.scope == scope)
            .unwrap()
            .0
            .depth = Some(depth);
        self.selected = Some(scope);
        true
    }
    pub(super) fn status(&self) -> serde_json::Value {
        let body_scopes: Vec<_> = self
            .frames
            .iter()
            .filter(|(f, _)| {
                f.actors
                    .iter()
                    .any(|a| a.global_model.as_ref().is_some_and(|g| g.body.is_some()))
            })
            .map(|(f, _)| f.scope)
            .collect();
        serde_json::json!({"bodyScopes":body_scopes,"frames":self.frames.len(),"ownedBytes":self.bytes,"selectedScope":self.selected().map(|f|f.scope),
            "oldestScope":self.frames.front().map(|f|f.0.scope),"latestScope":self.latest().map(|f|f.scope),
            "evicted":self.evicted,"rejected":self.rejected})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;
    fn frame(scope: u64, epoch: u64) -> Observation {
        Observation {
            scope,
            actors: Vec::new(),
            rejected: 0,
            sequence: scope,
            tick_count: scope,
            at: Instant::now(),
            depth: None,
            camera: Some(CameraSample {
                resource_stack: None,
                render_clock: None,
                renderer_timing: None,
                real_timer: None,
                native_camera: None,
                origin: [scope as f32, 0., 0.],
                view: [0.; 16],
                projection: [0.; 16],
                partial: Some(0.5),
                ticks_per_second: Some(20.),
                client_ticks: Some(scope),
                client_elapsed_ms: Some(scope as f64 * 50.),
                local_player: None,
                local_actor: None,
                local_cape_motion: None,
                local_cape_state: None,
                epoch: Some(epoch),
            }),
        }
    }
    fn depth() -> crate::depth::Depth {
        crate::depth::Depth {
            width: 1,
            height: 1,
            reverse_z: false,
            values: vec![0.5],
        }
    }
    #[test]
    fn delayed_gpu_depth_selects_its_owned_camera_and_keeps_only_one_image() {
        let mut h = History::default();
        for scope in 100..=115 {
            h.push(frame(scope, 1));
        }
        assert!(h.accept(107, depth()));
        assert_eq!(h.latest().unwrap().scope, 115);
        assert_eq!(
            h.selected().unwrap().camera.as_ref().unwrap().origin[0],
            107.
        );
        assert!(!h.accept(99, depth()));
        assert!(h.accept(108, depth()));
        assert!(h.get(107).unwrap().depth.is_none());
        h.begin_frame();
        assert!(h.selected().is_none());
        assert!(h.get(108).unwrap().depth.is_none());
        h.push(frame(116, 2));
        assert!(!h.accept(108, depth()));
        assert!(h.selected().is_none());
    }
    #[test]
    fn old_evicted_and_oversized_snapshots_cannot_receive_depth() {
        let mut h = History::default();
        for scope in 1..=33 {
            h.push(frame(scope, 1));
        }
        assert_eq!(h.frames.len(), 32);
        assert!(!h.accept(1, depth()));
        h.frames.back_mut().unwrap().0.at = Instant::now() - Duration::from_secs(1);
        assert!(!h.accept(33, depth()));
        let mut large = frame(34, 1);
        large.camera.as_mut().unwrap().local_player = Some(String::with_capacity(MAX_BYTES + 1));
        h.push(large);
        assert!(h.frames.is_empty());
        assert_eq!(h.rejected, 1);
    }
}
