//! Heap ownership of cache inputs, separate from root/child retargeting roles.
//! Identities are comparison keys confined to ActorScope; no address is archived.
use crate::{native_attachment_binding::Copies, native_palette_writer::Stamp};

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Identity {
    component: usize,
    model: usize,
    geometry: usize,
    hierarchy: u32,
    frame: u64,
    reload: u64,
    slot: Option<u32>,
    setup: Option<u64>,
}
pub(crate) struct Binding {
    stamp: Stamp,
    identity: Identity,
    params: usize,
    bones: usize,
    count: usize,
    copies: Copies,
}
fn word(bytes: &[u8], at: usize) -> Option<usize> {
    usize::try_from(u64::from_le_bytes(
        bytes.get(at..at.checked_add(8)?)?.try_into().ok()?,
    ))
    .ok()
}
fn integer(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}
fn counter(copies: &mut Copies, address: usize) -> Option<u64> {
    Some(u64::from_le_bytes(
        copies.read(address, 8)?.try_into().ok()?,
    ))
}
impl Binding {
    pub(crate) fn copy(stamp: &Stamp, params: usize, geometry: usize) -> Option<Self> {
        if crate::native_transform::palette_writer_scope().as_ref() != Some(stamp) {
            return None;
        }
        let mut copies = Copies::new();
        let root = word(
            &copies.read(
                stamp
                    .actor
                    .checked_add(crate::native_scene_layout::actor::ANIMATION_COMPONENT)?,
                8,
            )?,
            0,
        )?;
        if counter(&mut copies, root.checked_add(0x368)?)? != stamp.frame {
            return None;
        }
        let component = params.checked_sub(0x88)?;
        let slot = if component == root {
            None
        } else {
            let (begin, count) = copies.vector(root.checked_add(0x300)?, 0x50, 7)?;
            if count != 7 {
                return None;
            }
            let mut selected = None;
            for index in 0..count {
                let entry = begin.checked_add(index.checked_mul(0x50)?)?;
                let shared = copies.read(entry.checked_add(0x40)?, 16)?;
                if word(&shared, 0)? != component {
                    continue;
                }
                if selected.is_some() {
                    return None;
                }
                copies.live(word(&shared, 8)?)?;
                // The slot key, not its mutable Molang variable map, is stable.
                copies.read(entry, 8)?;
                selected = Some(u32::try_from(index).ok()?);
            }
            Some(selected?)
        };
        let values = copies.read(params, 0x40)?;
        let model = word(&values, 0x20)?;
        if word(&values, 0)? != stamp.context
            || word(&values, 0x10)? != component
            || word(&values, 0x38)? != stamp.actor
            || model < 0x10000
        {
            return None;
        }
        copies.live(word(&values, 0x28)?)?;
        let item = word(&copies.read(params.checked_add(0x78)?, 8)?, 0)?;
        if let Some(slot) = slot {
            if word(&values, 0x18)? != root
                || item < 0x10000
                || integer(&copies.read(component.checked_add(0x284)?, 4)?, 0)? != slot
            {
                return None;
            }
        } else {
            if item != 0 {
                return None;
            }
            crate::native_transform::model_binding(model, params)?;
        }
        if copies.read(component.checked_add(0x358)?, 1)? != [1] {
            return None;
        }
        let frame = counter(&mut copies, component.checked_add(0x368)?)?;
        let reload = counter(&mut copies, component.checked_add(0x370)?)?;
        let setup = slot.and_then(|_| {
            crate::native_transform::attachment_setup_serial(component, model, reload)
        });
        if frame != stamp.frame
            && (slot.is_none()
                || crate::native_transform::attachment_setup_frame(component, model, reload)
                    != Some(frame))
        {
            return None;
        }
        // A geometry must be selected by this component, not merely have a
        // matching numeric hierarchy index in some other component's namespace.
        let (begin, count) = copies.vector(component.checked_add(0x380)?, 0x28, 64)?;
        let entries = copies.read(begin, count.checked_mul(0x28)?)?;
        let mut selected = false;
        for entry in entries.chunks_exact(0x28) {
            if word(entry, 0)? == 0 || word(entry, 16)? != geometry {
                continue;
            }
            copies.live(word(entry, 8)?)?;
            copies.live(word(entry, 24)?)?;
            selected = true;
        }
        if !selected {
            return None;
        }
        let hierarchy = integer(&copies.read(geometry.checked_add(0xe8)?, 4)?, 0)?;
        let (_, _, bones, count) = copies
            .hierarchies(component)?
            .into_iter()
            .find(|(_, index, _, _)| *index == hierarchy)?;
        let binding = Self {
            stamp: stamp.clone(),
            identity: Identity {
                component,
                model,
                geometry,
                hierarchy,
                frame,
                reload,
                slot,
                setup,
            },
            params,
            bones,
            count,
            copies,
        };
        binding.valid().then_some(binding)
    }
    pub(crate) fn identity(&self) -> Identity {
        self.identity.clone()
    }
    pub(crate) fn matches(&self, frame: u64, hierarchy: u32, geometry: usize) -> bool {
        self.stamp.frame == frame
            && self.identity.hierarchy == hierarchy
            && self.identity.geometry == geometry
    }
    pub(crate) fn params(&self) -> usize {
        self.params
    }
    /// The caller's span can be on its native stack. Copy it synchronously;
    /// retain only the owning heap vector's metadata in this binding.
    pub(crate) fn matches_span(&self, span: &[u8]) -> bool {
        span.len() == 16 && word(span, 0) == Some(self.count) && word(span, 8) == Some(self.bones)
    }
    pub(crate) fn valid(&self) -> bool {
        (|| {
            if crate::native_transform::palette_writer_scope().as_ref() != Some(&self.stamp) {
                return None;
            }
            self.copies.unchanged()?;
            if self.identity.slot.is_some()
                && crate::native_transform::attachment_setup_serial(
                    self.identity.component,
                    self.identity.model,
                    self.identity.reload,
                ) != self.identity.setup
            {
                return None;
            }
            if self.identity.frame != self.stamp.frame
                && crate::native_transform::attachment_setup_frame(
                    self.identity.component,
                    self.identity.model,
                    self.identity.reload,
                ) != Some(self.identity.frame)
            {
                return None;
            }
            self.copies.unchanged()?;
            (crate::native_transform::palette_writer_scope().as_ref() == Some(&self.stamp))
                .then_some(())
        })()
        .is_some()
    }
}
