//! State-preserving transaction on an engine recording list. The engine's
//! surface/material/group producer is still separate; no packet is guessed
//! from presentation recency and no private list is submitted here.
use crate::{
    avatar_draw_order::{AvatarPhaseInputs, FeatureGroup, ModelViewPose, Phase, PhaseKey, Plan},
    avatar_geometry::PositionSpace,
    avatar_gpu_upload::Uploaded,
    avatar_material::{Constants, Pipelines, Target},
    native_bound_surfaces::{Bound, State as SurfaceState},
    native_graphics_state::{Snapshot, State},
};
use hudhook::windows::Win32::Graphics::Direct3D12::*;
use serde_json::{Value, json};
use std::{
    any::Any,
    collections::{HashMap, HashSet},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

pub(crate) struct Geometry {
    pub uploaded: Arc<Uploaded>,
    pub constants: Constants,
    pub pipelines: Arc<Pipelines>,
    pub groups: Vec<FeatureGroup>,
    pub uniform_indices: Vec<usize>,
    phase_groups: Option<Vec<(PhaseKey, Vec<usize>)>>,
}
impl Geometry {
    /// Consume the owned Java avatar collector's phase traversal together with
    /// actual adapter preparation/pose inputs. This supplies no native lease,
    /// foreign world features, insertion point or GPU resource owner.
    #[allow(dead_code)]
    pub(crate) fn from_owned_avatar_inputs(
        uploaded: impl Into<Arc<Uploaded>>,
        pipelines: Arc<Pipelines>,
        inputs: AvatarPhaseInputs,
    ) -> Result<Self, String> {
        let uploaded = uploaded.into();
        let phases =
            Phase::owned_avatar_ordered(uploaded.frame(), inputs).map_err(str::to_owned)?;
        let mut groups = Vec::new();
        let mut locations = Vec::new();
        for phase in phases {
            let start = groups.len();
            groups.extend(FeatureGroup::from_phases(vec![phase.phase]).map_err(str::to_owned)?);
            locations.push((phase.key, (start..groups.len()).collect()));
        }
        let mut geometry = Self::from_uploaded(uploaded, pipelines, groups)?;
        geometry.phase_groups = Some(locations);
        Ok(geometry)
    }
    /// Borrow the upload produced by the serialized client for this exact
    /// native surface/scope. Phase preparation/poses and native ownership still
    /// come from the real insertion adapter; this cannot select a recent world.
    #[allow(dead_code)]
    pub(crate) fn from_stream(
        scope: u64,
        width: u32,
        height: u32,
        pipelines: Arc<Pipelines>,
        inputs: AvatarPhaseInputs,
    ) -> Result<Self, String> {
        let reverse_z = pipelines.target().reverse_z;
        let uploaded = crate::native_geometry_stream::completed(scope, width, height, reverse_z)
            .ok_or("no completed geometry upload for this exact native world surface")?;
        let geometry = Self::from_owned_avatar_inputs(uploaded.clone(), pipelines, inputs)?;
        let current = crate::native_geometry_stream::completed(scope, width, height, reverse_z)
            .is_some_and(|current| Arc::ptr_eq(&current, &uploaded));
        if !current {
            return Err("geometry producer changed during phase preparation".into());
        }
        Ok(geometry)
    }
    /// Assemble real, already sorted phase output using Java's dispatcher
    /// grouping/strictness semantics before material allocation. This does not
    /// infer a phase from packet emission order or supply a live phase owner.
    #[allow(dead_code)]
    pub(crate) fn from_phase_outputs(
        uploaded: Uploaded,
        pipelines: Arc<Pipelines>,
        phases: Vec<Phase>,
    ) -> Result<Self, String> {
        let groups = FeatureGroup::from_phases(phases).map_err(str::to_owned)?;
        Self::from_uploaded(uploaded, pipelines, groups)
    }
    /// Assemble an already completed owned upload with packet-derived material
    /// constants. Real feature groups remain required from the owning adapter;
    /// texture identity or emission order cannot manufacture those groups.
    #[allow(dead_code)]
    pub(crate) fn from_uploaded(
        uploaded: impl Into<Arc<Uploaded>>,
        pipelines: Arc<Pipelines>,
        groups: Vec<FeatureGroup>,
    ) -> Result<Self, String> {
        let uploaded = uploaded.into();
        Plan::new(uploaded.frame(), &groups).map_err(str::to_owned)?;
        let target = pipelines.target();
        let device = pipelines.device()?;
        let (constants, uniform_indices) = Constants::from_frame(
            &device,
            target.node_mask,
            uploaded.frame(),
            target.reverse_z,
        )?;
        let geometry = Self {
            uploaded,
            constants,
            pipelines,
            groups,
            uniform_indices,
            phase_groups: None,
        };
        // Preflight the exact complete mapping before returning a recordable
        // owner. This records no commands and acquires no native scene lease.
        let plan = Plan::new(geometry.uploaded.frame(), &geometry.groups).map_err(str::to_owned)?;
        geometry.pipelines.prepare_ordered(
            &geometry.uploaded,
            &plan,
            &geometry.constants,
            &geometry.uniform_indices,
        )?;
        drop(plan);
        Ok(geometry)
    }
    fn groups_for_phase(&self, key: PhaseKey) -> Result<&[usize], String> {
        let phases = self
            .phase_groups
            .as_ref()
            .ok_or("entity phase locations are unknown")?;
        if ![(0, 0), (0, 7), (0, 8), (1, 8)].contains(&(key.order, key.stage)) {
            return Err("unsupported entity phase location".into());
        }
        Ok(phases
            .iter()
            .find(|(location, _)| *location == key)
            .map_or(&[][..], |(_, groups)| groups.as_slice()))
    }
}
/// Owns the actual glint pipeline/constants and its upload-complete base frame.
/// Construction/preflight does not prove selection, timing or phase ownership.
pub(crate) struct GlintGeometry {
    base: Arc<Geometry>,
    pipeline: Arc<crate::avatar_glint::Pipeline>,
    constants: Vec<crate::avatar_glint::Constants>,
    models: Vec<u16>,
    model_constants: HashMap<u16, usize>,
    schedule: crate::avatar_glint::WingSchedule,
}
impl GlintGeometry {
    /// Exclusive owned phase convenience. Shared phases use from_joint with
    /// the actual complete original participant sequence, including foreigners.
    #[allow(dead_code)]
    pub(crate) fn from_owned(
        base: Arc<Geometry>,
        pipeline: Arc<crate::avatar_glint::Pipeline>,
        inputs: &crate::avatar_glint::Inputs,
        poses: &[ModelViewPose],
    ) -> Result<Self, String> {
        let models = poses
            .iter()
            .map(|pose| {
                crate::avatar_glint::WingPhaseModel::Wing(ModelViewPose {
                    model: pose.model,
                    translation: pose.translation,
                })
            })
            .collect();
        Self::from_joint(base, pipeline, inputs, models)
    }
    #[allow(dead_code)]
    pub(crate) fn from_joint(
        base: Arc<Geometry>,
        pipeline: Arc<crate::avatar_glint::Pipeline>,
        inputs: &crate::avatar_glint::Inputs,
        participants: Vec<crate::avatar_glint::WingPhaseModel>,
    ) -> Result<Self, String> {
        if !same_target(base.pipelines.target(), pipeline.target()) {
            return Err("glint and base pipelines use different targets".into());
        }
        let schedule = crate::avatar_glint::WingSchedule::new(base.uploaded.frame(), participants)
            .map_err(str::to_owned)?;
        let models = schedule.wing_models();
        let device = base.pipelines.device()?;
        let constants = vec![crate::avatar_glint::Constants::for_uploaded(
            &device,
            pipeline.target(),
            &base.uploaded,
            inputs,
        )?];
        let model_constants = models.iter().map(|&model| (model, 0)).collect();
        let geometry = Self {
            base,
            pipeline,
            constants,
            models,
            model_constants,
            schedule,
        };
        geometry.prepare_models(&geometry.models)?;
        Ok(geometry)
    }
    /// Consume selected packet observations, retaining independent shader
    /// profiles per model. Unknown selection is an error; known absence is no
    /// pass. The caller still supplies the complete native foreign sequence.
    #[allow(dead_code)]
    pub(crate) fn from_packet(
        base: Arc<Geometry>,
        pipeline: Arc<crate::avatar_glint::Pipeline>,
        participants: Vec<crate::avatar_glint::PacketParticipant>,
    ) -> Result<Option<Self>, String> {
        if !same_target(base.pipelines.target(), pipeline.target()) {
            return Err("glint and base pipelines use different targets".into());
        }
        let (schedule, active) =
            crate::avatar_glint::packet_schedule(base.uploaded.frame(), participants)
                .map_err(str::to_owned)?;
        if active.is_empty() {
            return Ok(None);
        }
        let mut profiles = HashMap::new();
        let mut inputs = Vec::new();
        let mut model_constants = HashMap::new();
        for (model, uniforms) in active {
            let key = uniforms.shader_key();
            let index = if let Some(&index) = profiles.get(&key) {
                index
            } else {
                let index = inputs.len();
                inputs.push(crate::avatar_glint::Inputs::from_packet(uniforms));
                profiles.insert(key, index);
                index
            };
            model_constants.insert(model, index);
        }
        let device = base.pipelines.device()?;
        let constants = crate::avatar_glint::Constants::for_profiles(
            &device,
            pipeline.target(),
            &base.uploaded,
            &inputs,
        )?;
        let models = schedule.wing_models();
        let geometry = Self {
            base,
            pipeline,
            constants,
            models,
            model_constants,
            schedule,
        };
        geometry.prepare_models(&geometry.models)?;
        Ok(Some(geometry))
    }
    fn prepare_models<'a>(
        &'a self,
        models: &[u16],
    ) -> Result<Vec<crate::avatar_glint::Prepared<'a>>, String> {
        if models.len() > 2048 {
            return Err("glint model budget exceeded".into());
        }
        let mut seen = HashSet::new();
        for model in models {
            if !seen.insert(*model) || !self.model_constants.contains_key(model) {
                return Err("glint run repeats a model or has no selected profile".into());
            }
        }
        let mut result = Vec::new();
        let mut first = 0;
        while first < models.len() {
            let profile = self.model_constants[&models[first]];
            let mut end = first + 1;
            while end < models.len() && self.model_constants[&models[end]] == profile {
                end += 1;
            }
            let constants = self
                .constants
                .get(profile)
                .ok_or("glint model profile index exceeds its arena")?;
            result.push(self.pipeline.prepare_wings(
                &self.base.uploaded,
                constants,
                &models[first..end],
            )?);
            first = end;
        }
        Ok(result)
    }
}
/// A recording schedule cursor, not evidence that foreign GPU commands ran.
pub(crate) struct GlintCursor {
    geometry: Arc<GlintGeometry>,
    next: usize,
    poisoned: bool,
    binding: Option<(crate::native_graphics_state::ListToken, u64)>,
}
impl GlintCursor {
    pub(crate) fn new(geometry: Arc<GlintGeometry>) -> Self {
        Self {
            geometry,
            next: 0,
            poisoned: false,
            binding: None,
        }
    }
    pub(crate) fn next_step(&self) -> Result<Option<&crate::avatar_glint::WingPhaseStep>, String> {
        if self.poisoned {
            return Err("glint phase cursor failed; partial commands cannot be retried".into());
        }
        Ok(self.geometry.schedule.steps().get(self.next))
    }
    pub(crate) fn drained(&self) -> bool {
        !self.poisoned && self.next == self.geometry.schedule.steps().len()
    }
    fn check_binding(
        &self,
        list: &ID3D12GraphicsCommandList,
        state: &State,
        cohort: &Cohort,
    ) -> Result<(crate::native_graphics_state::ListToken, u64), String> {
        let token = state
            .list_token(list)
            .ok_or("glint phase recording-list identity is unknown")?;
        let binding = (token, cohort.scope);
        if self.binding.is_some_and(|previous| previous != binding) {
            return Err("glint phase changed recording lifetime or world scope".into());
        }
        Ok(binding)
    }
}
pub(crate) struct Cohort {
    pub scope: u64,
    pub width: u32,
    pub height: u32,
    pub target: Target,
    /// Actual native resource/descriptor owners for the bound scene and every
    /// restored native GPU address. COM target references alone are insufficient.
    pub native_owner: Arc<dyn Any + Send + Sync>,
}
pub(crate) struct Recorded {
    pub asset_generation: u64,
    pub scope: u64,
    pub sequence: u64,
    pub draw_ranges: usize,
}
struct Owners {
    _geometry: Arc<Geometry>,
    _glint: Option<Arc<GlintGeometry>>,
    _restore: Snapshot,
    _surfaces: Arc<Bound>,
    _native_owner: Arc<dyn Any + Send + Sync>,
}
static RECORDED: AtomicU64 = AtomicU64::new(0);
static GLINT_RECORDED: AtomicU64 = AtomicU64::new(0);
static RESTORE_FAILURES: AtomicU64 = AtomicU64::new(0);

fn same_target(a: Target, b: Target) -> bool {
    let node = |value| if value == 0 { 1 } else { value };
    a.color == b.color
        && a.depth == b.depth
        && a.samples == b.samples
        && a.reverse_z == b.reverse_z
        && a.front_counter_clockwise == b.front_counter_clockwise
        && node(a.node_mask) == node(b.node_mask)
}
fn resource_device(resource: &ID3D12Resource) -> Result<ID3D12Device, String> {
    let mut device = None;
    unsafe { resource.GetDevice(&mut device) }.map_err(|error| error.to_string())?;
    device.ok_or_else(|| "native recording resource has no device".into())
}
fn resource_node(
    resource: &ID3D12Resource,
    device: &ID3D12Device,
    node: u32,
) -> Result<(), String> {
    if *device != resource_device(resource)? {
        return Err("native recording resource belongs to another device".into());
    }
    let mut properties = D3D12_HEAP_PROPERTIES::default();
    unsafe { resource.GetHeapProperties(Some(&mut properties), None) }
        .map_err(|error| error.to_string())?;
    let visible = if properties.VisibleNodeMask == 0 {
        1
    } else {
        properties.VisibleNodeMask
    };
    if visible & node != node {
        return Err("native recording resource is not visible on the drawing node".into());
    }
    Ok(())
}
fn validate_device(
    list: &ID3D12GraphicsCommandList,
    geometry: &Geometry,
    target: Target,
) -> Result<(), String> {
    let mut device: Option<ID3D12Device> = None;
    unsafe { list.GetDevice(&mut device) }.map_err(|error| error.to_string())?;
    let device = device.ok_or("native recording list has no device")?;
    if unsafe { list.GetType() } != D3D12_COMMAND_LIST_TYPE_DIRECT
        || device != geometry.pipelines.device()?
    {
        return Err("native recording list/pipelines disagree on direct device".into());
    }
    let node = if target.node_mask == 0 {
        1
    } else {
        target.node_mask
    };
    let nodes = unsafe { device.GetNodeCount() };
    if !(1..=32).contains(&nodes) || node.count_ones() != 1 || (node as u64) >= (1u64 << nodes) {
        return Err("native recording node is outside its device".into());
    }
    for resource in geometry
        .uploaded
        .draw_resources()
        .chain(std::iter::once(geometry.constants.resource()))
    {
        resource_node(resource, &device, node)?;
    }
    let heap = geometry.uploaded.descriptor_heap();
    let mut heap_device: Option<ID3D12Device> = None;
    unsafe { heap.GetDevice(&mut heap_device) }.map_err(|error| error.to_string())?;
    let desc = unsafe { heap.GetDesc() };
    let heap_node = if desc.NodeMask == 0 { 1 } else { desc.NodeMask };
    if heap_device.as_ref() != Some(&device)
        || heap_node != node
        || desc.Type != D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV
        || desc.Flags.0 & D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE.0 == 0
    {
        return Err("native geometry descriptors disagree on device/node/type".into());
    }
    Ok(())
}
fn validate_surfaces(
    state: &State,
    geometry: &Geometry,
    cohort: &Cohort,
    surfaces: &Bound,
) -> Result<(), String> {
    let pair = surfaces
        .pair()
        .ok_or("native surface binding metadata changed")?;
    if pair.color_view.format != cohort.target.color
        || pair.depth_view.format != cohort.target.depth
        || pair.color_desc.Width != u64::from(cohort.width)
        || pair.color_desc.Height != cohort.height
        || pair.color_desc.SampleDesc != cohort.target.samples
        || !state.full_surface(cohort.width, cohort.height)
    {
        return Err(
            "native bound views, samples or raster bounds disagree with the draw cohort".into(),
        );
    }
    let device = geometry.pipelines.device()?;
    let node = if cohort.target.node_mask == 0 {
        1
    } else {
        cohort.target.node_mask
    };
    resource_node(pair.color, &device, node)?;
    resource_node(pair.depth, &device, node)
}
fn validate_world(
    list: &ID3D12GraphicsCommandList,
    state: &State,
    geometry: &Geometry,
    cohort: &Cohort,
) -> Result<(), String> {
    let frame = geometry.uploaded.frame();
    let backend = crate::native_batch::current_frame()
        .ok_or("native recording is outside a backend frame")?;
    if cohort.scope == 0
        || state.native_world_scope(list) != Some(cohort.scope)
        || crate::render_bridge::producer_for_scope(cohort.scope) != Some(backend)
    {
        return Err("native recording has no exact current world/batch binding".into());
    }
    if frame.width != cohort.width
        || frame.height != cohort.height
        || frame.space != PositionSpace::CameraRelative
        || !same_target(geometry.pipelines.target(), cohort.target)
    {
        return Err("native recording geometry disagrees with its bound scene".into());
    }
    let camera = crate::world_hooks::draw_camera(cohort.scope, cohort.target.reverse_z)
        .ok_or("native recording world observation expired")?;
    if frame.session != camera.session
        || frame.sequence != camera.sequence
        || frame.camera.map(f64::to_bits) != camera.origin.map(f64::to_bits)
        || frame.view_projection.map(f32::to_bits) != camera.projection.map(f32::to_bits)
    {
        return Err("native recording geometry belongs to another world/camera packet".into());
    }
    Ok(())
}
struct Restore<'a> {
    snapshot: &'a Snapshot,
    list: &'a ID3D12GraphicsCommandList,
    armed: bool,
}
impl Restore<'_> {
    fn finish(mut self) -> Result<(), String> {
        self.armed = false;
        let result = unsafe { self.snapshot.restore(self.list) };
        if result.is_err() {
            RESTORE_FAILURES.fetch_add(1, Ordering::Relaxed);
        }
        result
    }
}
impl Drop for Restore<'_> {
    fn drop(&mut self) {
        if self.armed && unsafe { self.snapshot.restore(self.list) }.is_err() {
            RESTORE_FAILURES.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// # Safety
/// The owning native adapter supplies the current native surface-state shadow,
/// winding/sample convention and native resource/descriptor ownership. Bound
/// views, recorded transitions and raster bounds are independently rechecked.
/// It serializes this exact live recording list, selects the correct Java
/// feature phase/order/material inputs, and pins all native resources/descriptor
/// contents referenced by the restored bindings through native retirement.
/// Uploaded geometry is upload-complete and immutable. Targets and every other
/// dynamic state outside the recorder's restore set remain unchanged. This
/// function validates observed identity/bindings; it does not discover ownership.
/// Caller keeps its graphics-state shadow locked/unchanged for this transaction.
pub(crate) unsafe fn record(
    list: &ID3D12GraphicsCommandList,
    state: &State,
    surface_state: &SurfaceState,
    geometry: &Arc<Geometry>,
    cohort: &Cohort,
) -> Result<Option<Recorded>, String> {
    if geometry
        .phase_groups
        .as_ref()
        .is_some_and(|phases| phases.len() > 1)
    {
        return Err(
            "owned avatar geometry spans multiple phases; record each phase separately".into(),
        );
    }
    validate_world(list, state, geometry, cohort)?;
    validate_device(list, geometry, cohort.target)?;
    let plan = Plan::new(geometry.uploaded.frame(), &geometry.groups).map_err(str::to_owned)?;
    let prepared = geometry.pipelines.prepare_ordered(
        &geometry.uploaded,
        &plan,
        &geometry.constants,
        &geometry.uniform_indices,
    )?;
    let emit = |list: &ID3D12GraphicsCommandList| unsafe { prepared.record(list) };
    unsafe {
        record_transaction(
            list,
            state,
            surface_state,
            geometry,
            cohort,
            None,
            prepared.ranges(),
            emit,
        )
    }
}

/// # Safety
/// Same recording/resource contracts as record, at this actual owned engine
/// phase. A PhaseKey validates source membership; it does not prove a callsite.
pub(crate) unsafe fn record_phase(
    list: &ID3D12GraphicsCommandList,
    state: &State,
    surface_state: &SurfaceState,
    geometry: &Arc<Geometry>,
    cohort: &Cohort,
    phase: PhaseKey,
) -> Result<Option<Recorded>, String> {
    let groups = geometry.groups_for_phase(phase)?;
    validate_world(list, state, geometry, cohort)?;
    validate_device(list, geometry, cohort.target)?;
    let plan = Plan::new(geometry.uploaded.frame(), &geometry.groups).map_err(str::to_owned)?;
    let prepared = geometry.pipelines.prepare_groups(
        &geometry.uploaded,
        &plan,
        &geometry.constants,
        &geometry.uniform_indices,
        groups,
    )?;
    let emit = |list: &ID3D12GraphicsCommandList| unsafe { prepared.record(list) };
    unsafe {
        record_transaction(
            list,
            state,
            surface_state,
            geometry,
            cohort,
            None,
            prepared.ranges(),
            emit,
        )
    }
}

/// # Safety
/// Same native ownership/list/state contracts as record. The caller is at
/// order 1 translucentModels phase 7 for the selected supported wing glint,
/// supplies an actual uninterrupted owned model delivery equivalent to this
/// local sort, and the exact base depth is already written. Shared foreign
/// model sorting/interleaving is not synthesized by this entry. Numeric phase
/// labels and a same-sized target do not prove a valid callsite.
pub(crate) unsafe fn record_glint(
    list: &ID3D12GraphicsCommandList,
    state: &State,
    surface_state: &SurfaceState,
    glint: &Arc<GlintGeometry>,
    cohort: &Cohort,
    phase: PhaseKey,
) -> Result<Option<Recorded>, String> {
    if glint.schedule.has_foreign() {
        return Err("joint glint phase must retain its foreign-delivery boundaries".into());
    }
    unsafe {
        record_glint_models(
            list,
            state,
            surface_state,
            glint,
            cohort,
            phase,
            &glint.models,
        )
    }
}
unsafe fn record_glint_models(
    list: &ID3D12GraphicsCommandList,
    state: &State,
    surface_state: &SurfaceState,
    glint: &Arc<GlintGeometry>,
    cohort: &Cohort,
    phase: PhaseKey,
    models: &[u16],
) -> Result<Option<Recorded>, String> {
    if phase != crate::avatar_glint::WING_PHASE {
        return Err("wing glint belongs to order 1 model phase 7".into());
    }
    let geometry = &glint.base;
    if !same_target(glint.pipeline.target(), cohort.target) {
        return Err("glint pipeline disagrees with the current scene target".into());
    }
    validate_world(list, state, geometry, cohort)?;
    validate_device(list, geometry, cohort.target)?;
    let prepared = glint.prepare_models(models)?;
    let draws = prepared.iter().map(|part| part.draws()).sum();
    let emit = |list: &ID3D12GraphicsCommandList| {
        for part in &prepared {
            unsafe { part.record(list) };
        }
    };
    unsafe {
        record_transaction(
            list,
            state,
            surface_state,
            geometry,
            cohort,
            Some(glint.clone()),
            draws,
            emit,
        )
    }
}

/// # Safety
/// The owner drives the complete actual order-1/model-7 phase. Every foreign
/// step is drawn by that owner outside this entry/tracker lock/private guard,
/// then acknowledged. Native selection, poses, targets, base depth and resource
/// contracts remain those of record_glint. This records only the next own run.
pub(crate) unsafe fn record_glint_step(
    list: &ID3D12GraphicsCommandList,
    state: &State,
    surface_state: &SurfaceState,
    cursor: &mut GlintCursor,
    cohort: &Cohort,
    phase: PhaseKey,
) -> Result<Option<Recorded>, String> {
    let result=(||->Result<(Option<Recorded>,(crate::native_graphics_state::ListToken,u64)),String> {
        let binding=cursor.check_binding(list,state,cohort)?;
        let models=match cursor.next_step()? {
            Some(crate::avatar_glint::WingPhaseStep::WingRun(models))=>models.as_slice(),
            Some(crate::avatar_glint::WingPhaseStep::Foreign(_))=>return Err("next glint phase step belongs to the native foreign owner".into()),
            None=>return Err("glint phase cursor is already drained".into()),
        };
        let recorded=unsafe {record_glint_models(list,state,surface_state,&cursor.geometry,cohort,phase,models)}?;
        Ok((recorded,binding))
    })();
    match result {
        Ok((recorded, binding)) => {
            cursor.binding = Some(binding);
            cursor.next += 1;
            Ok(recorded)
        }
        Err(error) => {
            cursor.poisoned = true;
            Err(error)
        }
    }
}

/// # Safety
/// The owning adapter actually delivered this exact foreign model step in the
/// selected phase/world/list, with its real resource owners. This advances
/// bookkeeping only; it does not execute/prove foreign rendering or completion.
pub(crate) unsafe fn acknowledge_glint_foreign(
    list: &ID3D12GraphicsCommandList,
    state: &State,
    cursor: &mut GlintCursor,
    cohort: &Cohort,
    phase: PhaseKey,
    token: u64,
) -> Result<(), String> {
    let result = (|| -> Result<(crate::native_graphics_state::ListToken, u64), String> {
        if phase != crate::avatar_glint::WING_PHASE {
            return Err("foreign delivery belongs to another glint phase".into());
        }
        validate_world(list, state, &cursor.geometry.base, cohort)?;
        let binding = cursor.check_binding(list, state, cohort)?;
        match cursor.next_step()? {
            Some(crate::avatar_glint::WingPhaseStep::Foreign(expected)) if *expected == token => {
                Ok(binding)
            }
            _ => Err("foreign glint phase delivery is missing, repeated or out of order".into()),
        }
    })();
    match result {
        Ok(binding) => {
            cursor.binding = Some(binding);
            cursor.next += 1;
            Ok(())
        }
        Err(error) => {
            cursor.poisoned = true;
            Err(error)
        }
    }
}

/// Common transaction for base phases and glint. The closure is invoked only
/// after native/Arctic owners have been retained and the state rechecked.
unsafe fn record_transaction(
    list: &ID3D12GraphicsCommandList,
    state: &State,
    surface_state: &SurfaceState,
    geometry: &Arc<Geometry>,
    cohort: &Cohort,
    glint: Option<Arc<GlintGeometry>>,
    draws: usize,
    emit: impl FnOnce(&ID3D12GraphicsCommandList),
) -> Result<Option<Recorded>, String> {
    if draws == 0 {
        return Ok(None);
    }
    validate_world(list, state, geometry, cohort)?;
    validate_device(list, geometry, cohort.target)?;
    let surfaces = surface_state
        .capture(list, state, cohort.scope)
        .ok_or("native recording lacks a current scoped surface binding")?;
    validate_surfaces(state, geometry, cohort, &surfaces)?;
    let snapshot = state
        .snapshot(list)
        .ok_or("native recording lacks a complete restorable state")?;
    let is_glint = glint.is_some();
    let owners = Owners {
        _geometry: geometry.clone(),
        _glint: glint,
        _restore: snapshot.clone(),
        _surfaces: surfaces.clone(),
        _native_owner: cohort.native_owner.clone(),
    };
    // Every owner is installed before the first command. These payloads contain
    // only clones: the borrowed geometry/cohort/state retain their original
    // owners, including across a failed reservation and its clone cleanup.
    let token = state
        .list_token(list)
        .ok_or("native recording list lifetime changed")?;
    let generation =
        match unsafe { crate::native_draw_assets::retain_recording(list, token, Box::new(owners)) }
        {
            Ok(value) => value,
            Err(error) => {
                let reason = error.reason;
                drop(error.assets);
                return Err(reason);
            }
        };
    // Registration performs no recording. Reject a changed native cohort before
    // any draw, while leaving the reservation safely pinned until reset/retire.
    validate_world(list, state, geometry, cohort)?;
    if surface_state
        .capture(list, state, cohort.scope)
        .is_none_or(|current| !Arc::ptr_eq(&current, &surfaces))
    {
        return Err("native surface cohort changed during recording preparation".into());
    }
    if !snapshot.matches(state, list) {
        return Err("native graphics state changed during recording preparation".into());
    }
    let _private = crate::depth_tracker::private_commands();
    let restore = Restore {
        snapshot: &snapshot,
        list,
        armed: true,
    };
    emit(list);
    restore
        .finish()
        .map_err(|error| format!("native draws recorded but state restoration failed: {error}"))?;
    RECORDED.fetch_add(1, Ordering::Relaxed);
    if is_glint {
        GLINT_RECORDED.fetch_add(1, Ordering::Relaxed);
    }
    Ok(Some(Recorded {
        asset_generation: generation,
        scope: cohort.scope,
        sequence: geometry.uploaded.frame().sequence,
        draw_ranges: draws,
    }))
}
pub(crate) fn status() -> Value {
    json!({"recordedTransactions":RECORDED.load(Ordering::Relaxed),"glintRecordedTransactions":GLINT_RECORDED.load(Ordering::Relaxed),
        "restoreFailures":RESTORE_FAILURES.load(Ordering::Relaxed),
        "nativeInsertionCallerImplemented":false,"liveInputsProducerImplemented":false,
        "trackedStateEntryImplemented":true,
        "geometryInputWithoutReadbackImplemented":true,
        "packetMaterialAssemblyImplemented":true,
        "javaFactoryReferenceInputsImplemented":true,
        "modelSubmissionBoundariesImplemented":true,
        "translucentModelPhaseSorterImplemented":true,
        "ownedAvatarSimplePhaseProducerImplemented":true,
        "ownedAvatarPhaseAssemblyImplemented":true,
        "serializedGeometryStreamSourceImplemented":true,
        "completedUploadGeometryAssemblyImplemented":true,
        "ownedAvatarSimplePhasesVerified":false,
        "armorGlintSourceImplemented":true,
        "armorGlintRecordingEntryImplemented":true,
        "armorGlintModelPhaseSorterImplemented":true,
        "jointGlintModelScheduleImplemented":true,
        "jointGlintCursorRecordingImplemented":true,
        "jointGlintNativeParticipantProducerImplemented":false,
        "packetGlintInputsImplemented":true,
        "packetGlintProfilesAssemblyImplemented":true,
        "entityPhaseRecordingEntryImplemented":true,
        "armorGlintNativeInsertionImplemented":false,
        "javaEquivalentModelSortingPoseProducerImplemented":false,
        "boundSurfaceAndRasterChecksImplemented":true,
        "committedRestoreBufferOwnersImplemented":true,
        "restoredDescriptorSpansImplemented":true,
        "publicDescriptorWriteTrackingImplemented":true,
        "retainedDescriptorResourcesImplemented":true,
        "restoredDescriptorContentsOwnerImplemented":false,
        "nativeDrawingVerified":false,"fullParity":false})
}
