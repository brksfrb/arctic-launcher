//! Current native binding metadata carried to the recording list, not an
//! exclusive target lease or proof of physical descriptor/GPU validity.
use crate::{
    native_framebuffer_resources::{Attachments, Pair},
    native_graphics_state::{ListToken, State as Graphics},
    native_surface_views::Kind,
    render_bridge::FrameKey,
};
use hudhook::windows::Win32::Graphics::Direct3D12::*;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

pub(crate) const MAX_BINDINGS: usize = 16;
pub(crate) const MAX_ALLOCATION_BYTES: u64 = 256 * 1024 * 1024;
static NEXT: AtomicU64 = AtomicU64::new(1);
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct Epoch(u64);
pub(crate) struct Bound {
    epoch: Epoch,
    token: ListToken,
    attachments: Attachments,
    bytes: u64,
    pub binding_frame: FrameKey,
}
fn allocation(resource: &ID3D12Resource, desc: &D3D12_RESOURCE_DESC) -> Option<u64> {
    let mut device: Option<ID3D12Device> = None;
    unsafe { resource.GetDevice(&mut device) }.ok()?;
    let mut properties = D3D12_HEAP_PROPERTIES::default();
    unsafe { resource.GetHeapProperties(Some(&mut properties), None) }.ok()?;
    let info = unsafe { device?.GetResourceAllocationInfo(properties.CreationNodeMask, &[*desc]) };
    (info.SizeInBytes > 0 && info.SizeInBytes != u64::MAX).then_some(info.SizeInBytes)
}
impl Bound {
    pub(crate) fn copy(
        list: &ID3D12GraphicsCommandList,
        graphics: &Graphics,
        epoch: Epoch,
        frame: FrameKey,
        attachments: Attachments,
    ) -> Option<Arc<Self>> {
        let token = graphics.list_token(list)?;
        let pair = attachments.pair()?;
        let bytes = allocation(pair.color, &pair.color_desc)?
            .checked_add(allocation(pair.depth, &pair.depth_desc)?)?;
        if bytes > MAX_ALLOCATION_BYTES {
            return None;
        }
        Some(Arc::new(Self {
            epoch,
            token,
            attachments,
            bytes,
            binding_frame: frame,
        }))
    }
    pub(crate) fn pair(&self) -> Option<Pair<'_>> {
        self.attachments.pair()
    }
    pub(crate) fn allocation_bytes(&self) -> u64 {
        self.bytes
    }
    /// Strong owners for actual observed committed allocations only. This does
    /// not own mutable descriptors, restored GPU addresses or an exclusive phase.
    #[allow(dead_code)]
    pub(crate) fn committed_allocations(
        &self,
    ) -> Option<[Arc<crate::native_committed_resources::Owner>; 2]> {
        let pair = self.pair()?;
        Some([
            pair.color_allocation?.clone(),
            pair.depth_allocation?.clone(),
        ])
    }
}
#[derive(Default)]
pub(crate) struct State {
    epoch: Option<Epoch>,
    bound: Option<Arc<Bound>>,
    world_scope: Option<u64>,
}
impl State {
    pub(crate) fn begin_binding(&mut self) -> Option<Epoch> {
        self.invalidate();
        if !(crate::native_body::enabled() || crate::native_clock::probe_enabled()) {
            return None;
        }
        self.epoch = NEXT
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .ok()
            .map(Epoch);
        self.epoch
    }
    pub(crate) fn invalidate(&mut self) {
        self.bound = None;
        self.epoch = None;
        self.world_scope = None;
    }
    pub(crate) fn attach(&mut self, epoch: Epoch, bound: Arc<Bound>) -> bool {
        if self.epoch != Some(epoch) || bound.epoch != epoch {
            return false;
        }
        self.bound = Some(bound);
        self.world_scope = None;
        true
    }
    pub(crate) fn accepts(&self, epoch: Epoch) -> bool {
        self.epoch == Some(epoch)
    }
    pub(crate) fn bytes(&self) -> u64 {
        self.bound.as_ref().map_or(0, |bound| bound.bytes)
    }
    pub(crate) fn bound(&self) -> bool {
        self.bound.is_some()
    }
    pub(crate) fn scoped(&self) -> bool {
        self.world_scope.is_some()
    }
    /// Called after forwarding a nonempty actual public native draw. Private
    /// Arctic draws are excluded by the enclosing observer, not counted here.
    pub(crate) fn after_draw(&mut self, list: &ID3D12GraphicsCommandList, graphics: &Graphics) {
        self.world_scope = (|| {
            let bound = self.bound.as_ref()?;
            if self.epoch != Some(bound.epoch) || graphics.list_token(list) != Some(bound.token) {
                return None;
            }
            let scope = graphics.native_world_scope(list)?;
            let current = crate::native_batch::current_frame()?;
            (crate::render_bridge::producer_for_scope(scope) == Some(current)).then_some(scope)
        })();
    }
    pub(crate) fn capture(
        &self,
        list: &ID3D12GraphicsCommandList,
        graphics: &Graphics,
        scope: u64,
    ) -> Option<Arc<Bound>> {
        let bound = self.bound.as_ref()?;
        if self.world_scope != Some(scope)
            || self.epoch != Some(bound.epoch)
            || graphics.list_token(list) != Some(bound.token)
        {
            return None;
        }
        // The binding can persist across native batches on one open list; the
        // separate actual draw/batch/world join determines its current scope.
        bound.pair()?;
        Some(bound.clone())
    }
    pub(crate) fn transition(&mut self, resource: Option<&ID3D12Resource>) {
        let Some(bound) = self.bound.as_ref() else {
            return;
        };
        if resource.and_then(|resource| bound.attachments.references(resource)) != Some(false) {
            self.invalidate();
        }
    }
    pub(crate) fn clear(&mut self, kind: Kind, handle: usize) {
        let Some(bound) = self.bound.as_ref() else {
            return;
        };
        if bound.attachments.aliases_view(kind, handle) != Some(false) {
            self.world_scope = None;
        }
    }
}
