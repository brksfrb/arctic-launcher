//! Bounded native texture-slot copies joined to actual public barrier arguments.
//! Raw registry pointers are never invoked as COM objects. Only a live resource
//! supplied by the intercepted API is borrowed/cloned. A validated bounded
//! handoff may extend those references into the native list's binding state.
use crate::native_scene_layout::framebuffer as layout;
use crate::native_surface_views::{Kind, Snapshot};
use crate::native_world_work::Copies;
use hudhook::windows::{
    Win32::Graphics::Direct3D12::*,
    core::{IUnknown, Interface},
};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::sync::Arc;

const TEXTURES: usize = layout::TEXTURES;
const STATE_CACHE: usize = layout::STATE_CACHE;
const STRIDE: usize = layout::TEXTURE_STRIDE;
const PAGE_ENTRIES: usize = layout::PAGE_ENTRIES;
fn word(bytes: &[u8], at: usize) -> Option<usize> {
    usize::try_from(u64::from_le_bytes(
        bytes.get(at..at.checked_add(8)?)?.try_into().ok()?,
    ))
    .ok()
}
fn pointer(bytes: &[u8], at: usize) -> Option<usize> {
    word(bytes, at)
        .filter(|value| (0x10000..0x0000_8000_0000_0000).contains(value) && value.is_multiple_of(8))
}
struct Texture {
    resource: usize,
    retirement_bypass: bool,
    copies: Copies,
}
impl Texture {
    fn copy(registry: usize, handle: u16) -> Option<Self> {
        if handle == u16::MAX {
            return None;
        }
        let mut copies = Copies::default();
        let page_bytes = PAGE_ENTRIES * STRIDE;
        let extent = word(&copies.copy(registry.checked_add(page_bytes + 8)?, 8)?, 0)?;
        if !extent.is_multiple_of(STRIDE)
            || extent / STRIDE > 65536
            || usize::from(handle) >= extent / STRIDE
        {
            return None;
        }
        let index = usize::from(handle);
        let mut page = registry;
        if index >= PAGE_ENTRIES {
            // Native lookup follows a root pointer to its first extension page,
            // then one page-link at the inline-page byte extent per extra page.
            if copies.copy(registry.checked_add(page_bytes + 24)?, 1)?[0] != 0 {
                return None;
            }
            let root = pointer(&copies.copy(registry.checked_add(page_bytes)?, 8)?, 0)?;
            page = pointer(&copies.copy(root, 8)?, 0)?;
            let mut visited = HashSet::new();
            visited.insert(page);
            for _ in 1..index / PAGE_ENTRIES {
                page = pointer(&copies.copy(page.checked_add(page_bytes)?, 8)?, 0)?;
                if !visited.insert(page) {
                    return None;
                }
            }
        }
        let slot = page.checked_add((index % PAGE_ENTRIES).checked_mul(STRIDE)?)?;
        let resource = pointer(&copies.copy(slot, 8)?, 0)?;
        // Texture cleanup tests +0x9b bit 0x40 before its ordinary deferred
        // resource retirement path. Guard the whole byte as well as the pointer:
        // changing that path must invalidate an earlier ownership candidate.
        let retirement_bypass = copies.copy(slot.checked_add(layout::RETIREMENT_FLAG)?, 1)?[0]
            & layout::RETIREMENT_MASK
            != 0;
        copies.unchanged()?;
        Some(Self {
            resource,
            retirement_bypass,
            copies,
        })
    }
}
struct Witness {
    resource: ID3D12Resource,
    identity: usize,
    desc: D3D12_RESOURCE_DESC,
    allocation: Option<Arc<crate::native_committed_resources::Owner>>,
    before: D3D12_RESOURCE_STATES,
    after: D3D12_RESOURCE_STATES,
}
struct Slot {
    handle: u16,
    roles: u16,
    texture: Option<Texture>,
    witness: Option<Witness>,
    views: Vec<(u16, Option<Snapshot>)>,
}
impl Slot {
    fn view(&self) -> Option<&Snapshot> {
        if self.views.len() != 1 || self.views[0].0 != self.roles {
            return None;
        }
        self.views[0].1.as_ref()
    }
}
pub(crate) struct Attachments {
    copies: Copies,
    slots: Vec<Slot>,
    expected_views: usize,
    bound_views: Option<usize>,
}
pub(crate) struct Pair<'a> {
    pub color: &'a ID3D12Resource,
    pub depth: &'a ID3D12Resource,
    pub color_desc: D3D12_RESOURCE_DESC,
    pub depth_desc: D3D12_RESOURCE_DESC,
    pub color_view: crate::native_surface_views::Surface,
    pub depth_view: crate::native_surface_views::Surface,
    pub color_allocation: Option<&'a Arc<crate::native_committed_resources::Owner>>,
    pub depth_allocation: Option<&'a Arc<crate::native_committed_resources::Owner>>,
}
impl Attachments {
    pub(crate) fn copy(context: usize, renderer: usize, framebuffer: u16) -> Option<Self> {
        if !crate::native_framebuffer::reader_ready() {
            return None;
        }
        // The default/window framebuffer has a separate swap-chain path.
        if framebuffer == u16::MAX {
            return None;
        }
        let registry = renderer.checked_add(TEXTURES)?;
        let mut copies = Copies::default();
        if pointer(&copies.copy(context.checked_add(STATE_CACHE)?, 8)?, 0)? != registry {
            return None;
        }
        let entry = renderer
            .checked_add(layout::FRAMEBUFFERS)?
            .checked_add(usize::from(framebuffer).checked_mul(layout::FRAMEBUFFER_STRIDE)?)?;
        let count = usize::from(copies.copy(entry.checked_add(layout::FRAMEBUFFER_COUNT)?, 1)?[0]);
        if count > 8 {
            return None;
        }
        let colors = if count == 0 {
            Vec::new()
        } else {
            copies.copy(entry, count * 2)?
        };
        let depth = copies.copy(entry.checked_add(layout::FRAMEBUFFER_DEPTH)?, 2)?;
        let mut slots: Vec<Slot> = Vec::new();
        for index in 0..=count {
            let (handle, roles) = if index < count {
                (
                    u16::from_le_bytes(colors[index * 2..index * 2 + 2].try_into().ok()?),
                    1u16 << index,
                )
            } else {
                (
                    u16::from_le_bytes(depth.as_slice().try_into().ok()?),
                    1u16 << 8,
                )
            };
            if handle == u16::MAX {
                continue;
            }
            if let Some(slot) = slots.iter_mut().find(|slot| slot.handle == handle) {
                slot.roles |= roles;
                continue;
            }
            slots.push(Slot {
                handle,
                roles,
                texture: Texture::copy(registry, handle),
                witness: None,
                views: Vec::new(),
            });
        }
        let expected_views = slots
            .iter()
            .map(|slot| slot.roles.count_ones() as usize)
            .sum();
        copies.unchanged()?;
        Some(Self {
            copies,
            slots,
            expected_views,
            bound_views: None,
        })
    }
    pub(crate) fn transition(
        &mut self,
        resource: &ID3D12Resource,
        before: D3D12_RESOURCE_STATES,
        after: D3D12_RESOURCE_STATES,
    ) -> usize {
        let mut matched = 0;
        for slot in &mut self.slots {
            let Some(texture) = slot.texture.as_ref() else {
                continue;
            };
            if texture.resource != resource.as_raw() as usize
                || texture.copies.unchanged().is_none()
            {
                continue;
            }
            // This COM object comes from the actual API callback, never from a
            // guessed native slot dereference. A validated binding handoff may
            // extend this actual object's reference beyond the native call.
            let desc = unsafe { resource.GetDesc() };
            let Ok(identity) = resource.cast::<IUnknown>() else {
                continue;
            };
            let allocation = crate::native_committed_resources::matching(resource);
            slot.witness = Some(Witness {
                resource: resource.clone(),
                identity: identity.as_raw() as usize,
                desc,
                allocation,
                before,
                after,
            });
            matched += 1;
        }
        matched
    }
    pub(crate) fn observed(&self) -> usize {
        self.slots
            .iter()
            .filter(|slot| slot.witness.is_some())
            .count()
    }
    /// A complete, whole-surface pair for the current single-RTV material path.
    /// MRT/array/mip/plane/deferred-state paths need their own material contract.
    pub(crate) fn pair(&self) -> Option<Pair<'_>> {
        if self.expected_views != 2 || self.bound_views != Some(2) || self.slots.len() != 2 {
            return None;
        }
        self.copies.unchanged()?;
        let slot = |role: u16| self.slots.iter().find(|slot| slot.roles == role);
        let color = slot(1)?;
        let depth = slot(256)?;
        let validate = |slot: &Slot| {
            slot.texture.as_ref()?.copies.unchanged()?;
            let witness = slot.witness.as_ref()?;
            (unsafe { witness.resource.GetDesc() } == witness.desc).then_some(())
        };
        validate(color)?;
        validate(depth)?;
        let color_witness = color.witness.as_ref()?;
        let depth_witness = depth.witness.as_ref()?;
        if color_witness.identity == depth_witness.identity
            || color_witness.after != D3D12_RESOURCE_STATE_RENDER_TARGET
            || depth_witness.after != D3D12_RESOURCE_STATE_DEPTH_WRITE
        {
            return None;
        }
        let color_snapshot = color.view()?;
        let depth_snapshot = depth.view()?;
        let color_view = color_snapshot.surface()?;
        let depth_view = depth_snapshot.surface()?;
        if color_view.kind != Kind::Color
            || depth_view.kind != Kind::Depth
            || color_view.mip != 0
            || depth_view.mip != 0
            || color_view.plane != 0
            || depth_view.plane != 0
            || depth_view.flags != D3D12_DSV_FLAG_NONE
        {
            return None;
        }
        let shape = |desc: &D3D12_RESOURCE_DESC| {
            desc.Dimension == D3D12_RESOURCE_DIMENSION_TEXTURE2D
                && desc.DepthOrArraySize == 1
                && desc.MipLevels == 1
                && desc.Width > 0
                && desc.Height > 0
        };
        let c = color_witness.desc;
        let d = depth_witness.desc;
        if !shape(&c)
            || !shape(&d)
            || c.Width != d.Width
            || c.Height != d.Height
            || c.SampleDesc != d.SampleDesc
        {
            return None;
        }
        if !crate::native_surface_views::cohort_stable([color_snapshot, depth_snapshot].into_iter())
        {
            return None;
        }
        Some(Pair {
            color: &color_witness.resource,
            depth: &depth_witness.resource,
            color_desc: c,
            depth_desc: d,
            color_view,
            depth_view,
            color_allocation: color_witness.allocation.as_ref(),
            depth_allocation: depth_witness.allocation.as_ref(),
        })
    }
    pub(crate) fn references(&self, resource: &ID3D12Resource) -> Option<bool> {
        let identity = resource.cast::<IUnknown>().ok()?;
        Some(
            self.slots
                .iter()
                .filter_map(|slot| slot.witness.as_ref())
                .any(|witness| witness.identity == identity.as_raw() as usize),
        )
    }
    pub(crate) fn aliases_view(&self, kind: Kind, handle: usize) -> Option<bool> {
        let role = if kind == Kind::Color { 1 } else { 256 };
        let slot = self.slots.iter().find(|slot| slot.roles == role)?;
        slot.view()?.aliases(kind, handle)
    }
    /// Join descriptor metadata using the real list's device and actual API
    /// handles. A resource must already have a scoped public-barrier witness.
    pub(crate) fn binding(&mut self, list: usize, colors: &[usize], depth: Option<usize>) {
        self.bound_views = Some(colors.len() + usize::from(depth.is_some()));
        let raw = list as *mut core::ffi::c_void;
        let device = (|| {
            let list = unsafe { ID3D12GraphicsCommandList::from_raw_borrowed(&raw) }?;
            let mut device: Option<ID3D12Device> = None;
            unsafe { list.GetDevice(&mut device) }.ok()?;
            crate::native_surface_views::device_identity(&device?)
        })();
        for slot in &mut self.slots {
            for role in 0..9 {
                let mask = 1u16 << role;
                if slot.roles & mask == 0 {
                    continue;
                }
                let (kind, handle) = if role == 8 {
                    (Kind::Depth, depth)
                } else {
                    (Kind::Color, colors.get(role).copied())
                };
                let view = (|| {
                    let resource = &slot.witness.as_ref()?.resource;
                    Snapshot::matches(device?, kind, handle?, resource)
                })();
                slot.views.push((mask, view));
            }
        }
    }
    pub(crate) fn summary(&self) -> Value {
        let stable = self.copies.unchanged().is_some();
        let mut observed = 0;
        let mut mapped_views = 0;
        let slots:Vec<Value>=self.slots.iter().map(|slot| {
            let known=stable&&slot.texture.as_ref().is_some_and(|texture|texture.copies.unchanged().is_some());
            let witness=slot.witness.as_ref().filter(|witness|known&&unsafe {witness.resource.GetDesc()}==witness.desc);
            let mut item=json!({"nativeTextureHandle":slot.handle,"attachmentRoles":slot.roles,
                "registrySlotStable":known,"observedNativeBarrierResource":witness.is_some(),
                "observedCommittedResourceOwner":witness.is_some_and(|witness|witness.allocation.is_some()),
                "ordinaryRetirementBypassed":slot.texture.as_ref().filter(|_|known).map(|texture|texture.retirement_bypass)});
            let views:Vec<Value>=slot.views.iter().map(|(role,view)| {
                let view=view.as_ref().filter(|view|witness.is_some()&&view.stable());
                if view.is_some() {mapped_views+=1;}
                json!({"attachmentRole":role,"view":view.map(Snapshot::summary)})
            }).collect();
            item["views"]=json!(views);
            if let Some(witness)=witness {
                observed+=1;
                item["resource"]=json!({"dimension":witness.desc.Dimension.0,"width":witness.desc.Width,
                    "height":witness.desc.Height,"arrayOrDepth":witness.desc.DepthOrArraySize,"mips":witness.desc.MipLevels,
                    "format":witness.desc.Format.0,"samples":witness.desc.SampleDesc.Count,
                    "recordedStateBefore":witness.before.0,"recordedStateAfter":witness.after.0});
            }item
        }).collect();
        let cohort_stable = crate::native_surface_views::cohort_stable(
            self.slots
                .iter()
                .flat_map(|slot| slot.views.iter().filter_map(|(_, view)| view.as_ref())),
        );
        json!({"registryKnown":true,"framebufferHandlesStable":stable,"referencedTextures":self.slots.len(),
            "observedResources":observed,"allReferencedResourcesObserved":!self.slots.is_empty()&&observed==self.slots.len(),
            "mappedAttachmentViews":mapped_views,"expectedAttachmentViews":self.expected_views,
            "allAttachmentViewArgumentsMatched":stable&&cohort_stable&&self.expected_views>0&&mapped_views==self.expected_views
                &&self.bound_views==Some(self.expected_views),
            "descriptorCohortStable":cohort_stable&&self.expected_views>0&&mapped_views==self.expected_views,
            "slots":slots,"surfaceViewMappingVerified":false,"queuedGpuStateVerified":false,"targetLeaseVerified":false})
    }
}
