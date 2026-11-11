//! A frame-local skin reference from the root model's selected controller list.
//! This is not a receipt that the skin was submitted. No pointer leaves the
//! enclosing ActorScope, and no previous frame's hierarchy is reused.
use crate::native_scene_layout::{model_body, skin_selection as layout};
use crate::world_hooks::read;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

#[path = "native_skin_selection_profile.rs"]
mod profile;

static COPIES: AtomicU64 = AtomicU64::new(0);
const LIMIT: usize = 64;
const STRIDE: usize = layout::PAIR_STRIDE;

struct Guard {
    address: usize,
    bytes: Vec<u8>,
}
struct Copies {
    guards: Vec<Guard>,
    bytes: usize,
    refs: Vec<(usize, bool)>,
}
impl Copies {
    fn read(&mut self, address: usize, length: usize) -> Option<Vec<u8>> {
        if address < 0x10000
            || address.checked_add(length).is_none()
            || self.guards.len() >= 256
            || self.bytes.checked_add(length)? > 32 * 1024
        {
            return None;
        }
        let mut bytes = vec![0; length];
        read(address, &mut bytes).ok()?;
        self.bytes += length;
        self.guards.push(Guard {
            address,
            bytes: bytes.clone(),
        });
        Some(bytes)
    }
    fn shared(&mut self, control: usize) -> Option<bool> {
        let live = strong(control)?;
        self.refs.push((control, live));
        Some(live)
    }
    fn unchanged(&self) -> Option<()> {
        for guard in &self.guards {
            let mut bytes = vec![0; guard.bytes.len()];
            read(guard.address, &mut bytes).ok()?;
            if bytes != guard.bytes {
                return None;
            }
        }
        // Native shared/weak_ptr copies change the positive reference count.
        // Its liveness must agree; its transient numeric value need not agree.
        for &(control, live) in &self.refs {
            if strong(control)? != live {
                return None;
            }
        }
        Some(())
    }
    fn name(&mut self, geometry: usize) -> Option<String> {
        let header = self.read(geometry.checked_add(model_body::GEOMETRY_NAME)?, 32)?;
        let text = header.as_slice();
        let length = word(text, 16)?;
        let capacity = word(text, 24)?;
        if length == 0 || length > 128 || capacity < length || capacity > 4096 {
            return None;
        }
        let bytes = if capacity < 16 {
            text.get(..length.checked_add(1)?)?.to_vec()
        } else {
            self.read(word(text, 0)?, length.checked_add(1)?)?
        };
        if bytes.last() != Some(&0) {
            return None;
        }
        Some(std::str::from_utf8(&bytes[..length]).ok()?.to_owned())
    }
}
fn word(bytes: &[u8], at: usize) -> Option<usize> {
    usize::try_from(u64::from_le_bytes(
        bytes.get(at..at.checked_add(8)?)?.try_into().ok()?,
    ))
    .ok()
}
fn strong(control: usize) -> Option<bool> {
    if control < 0x10000 {
        return None;
    }
    let mut count = [0; 4];
    read(control.checked_add(8)?, &mut count).ok()?;
    let count = i32::from_le_bytes(count);
    (count >= 0).then_some(count != 0)
}

pub(crate) struct Reference {
    pub pose: crate::native_body::SkinPose,
    actor: usize,
    context: usize,
    copies: Copies,
}
impl Reference {
    pub(crate) fn belongs(&self, actor: usize, context: usize) -> bool {
        self.actor == actor && self.context == context
    }
    pub(crate) fn valid(&self) -> bool {
        (|| {
            self.copies.unchanged()?;
            let pose = crate::animation_pose::actor(self.actor, self.context)?;
            if pose.update_frame != self.pose.animation_frame
                || crate::animation_pose::standard_parts(
                    &pose,
                    self.pose.hierarchy,
                    self.pose.native_slim,
                )? != self.pose.parts
            {
                return None;
            }
            self.copies.unchanged()
        })()
        .is_some()
    }
}

/// The original Model::draw has already selected these pairs before its first
/// dataDrivenDraw. Require the actual current controller/geometry pair in that
/// list, plus one explicitly named geometry matching SerializedSkinImpl. A
/// generic "default", an uninitialized index, or two candidate entries cannot
/// identify the skin before its own draw and therefore supplies no reference.
pub(crate) fn copy(
    actor: usize,
    context: usize,
    model: usize,
    params: usize,
    controller: usize,
    geometry: usize,
    pose: &crate::animation_pose::Pose,
    appearance: &crate::native_skins::Appearance,
) -> Option<Arc<Reference>> {
    if !profile::installed() {
        return None;
    }
    if !matches!(
        appearance.geometry.as_str(),
        "geometry.humanoid.custom" | "geometry.humanoid.customSlim" | "geometry.humanoid"
    ) {
        return None;
    }
    let mut copies = Copies {
        guards: Vec::new(),
        bytes: 0,
        refs: Vec::new(),
    };
    let component = word(
        &copies.read(
            actor.checked_add(crate::native_scene_layout::actor::ANIMATION_COMPONENT)?,
            8,
        )?,
        0,
    )?;
    if params != component.checked_add(layout::COMPONENT_PARAMS)? {
        return None;
    }
    for (offset, expected) in [
        (0, context),
        (0x10, component),
        (0x20, model),
        (0x38, actor),
    ] {
        if word(&copies.read(params.checked_add(offset)?, 8)?, 0)? != expected {
            return None;
        }
    }
    let frame = copies.read(component.checked_add(layout::COMPONENT_FRAME)?, 8)?;
    if u64::from_le_bytes(frame.try_into().ok()?) != pose.update_frame {
        return None;
    }
    let header = copies.read(component.checked_add(layout::COMPONENT_PAIRS)?, 24)?;
    let begin = word(&header, 0)?;
    let end = word(&header, 8)?;
    let capacity = word(&header, 16)?;
    let size = end.checked_sub(begin)?;
    if begin < 0x10000
        || size == 0
        || !size.is_multiple_of(STRIDE)
        || size / STRIDE > LIMIT
        || capacity < end
        || capacity.checked_sub(begin)? > 4096 * STRIDE
    {
        return None;
    }
    let entries = copies.read(begin, size)?;
    let mut current = 0usize;
    let mut selected = None;
    for entry in entries.chunks_exact(STRIDE) {
        let native_controller = word(entry, layout::CONTROLLER)?;
        if native_controller == 0 {
            continue;
        }
        if !copies.shared(word(entry, layout::CONTROLLER_CONTROL)?)? {
            return None;
        }
        let native_geometry = word(entry, layout::GEOMETRY)?;
        let control = word(entry, layout::GEOMETRY_CONTROL)?;
        if control == 0 || !copies.shared(control)? {
            continue;
        }
        if native_geometry < 0x10000 {
            return None;
        }
        if native_controller == controller && native_geometry == geometry {
            current += 1;
        }
        let name = copies.name(native_geometry)?;
        if name != appearance.geometry {
            continue;
        }
        let index = copies.read(
            native_geometry.checked_add(model_body::GEOMETRY_HIERARCHY)?,
            4,
        )?;
        let hierarchy = u32::from_le_bytes(index.try_into().ok()?);
        if hierarchy == u32::MAX || selected.is_some() {
            return None;
        }
        let parts = crate::animation_pose::standard_parts(pose, hierarchy, appearance.slim)?;
        selected = Some(crate::native_body::SkinPose {
            geometry: name,
            hierarchy,
            animation_frame: pose.update_frame,
            native_skin: appearance.skin.clone(),
            native_slim: appearance.slim,
            parts,
        });
    }
    if current != 1 {
        return None;
    }
    let reference = Arc::new(Reference {
        pose: selected?,
        actor,
        context,
        copies,
    });
    if !reference.valid() {
        return None;
    }
    COPIES.fetch_add(1, Ordering::Relaxed);
    Some(reference)
}

/// Admit the selected scene family's pair vector, weak geometry promotion and
/// hierarchy windows, joined to the exact body/controller/root draw chain.
/// Installation runs before the body callback can publish skin references.
pub(crate) fn validate_profile(base: usize) -> Result<(), String> {
    profile::install(base)
}
pub(crate) fn status() -> serde_json::Value {
    serde_json::json!({"copies":COPIES.load(Ordering::Relaxed),"selectedListReferenceImplemented":true,
        "readerProfileInstalled":profile::installed(),"readerProfileSourceImplemented":true,
        "skinSubmissionAttested":false,"equipmentRenderingVerified":false})
}
