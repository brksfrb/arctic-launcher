//! Pinned synchronous skinned-material input copies. Only a native caller's
//! checked stack-local vector fields are temporarily changed, never globals.
use crate::native_scene_layout::skinned_description as desc;
use crate::world_hooks::{floats, pointer, read};
use crate::{
    native_attachment_replay::ColorTarget,
    native_shader_inputs::ActorInputs,
    native_submit::{MeshRole, Tag},
};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
};

static BINDING: OnceLock<crate::world_hooks::SkinnedMaterialConsumerBinding> = OnceLock::new();
const PREFIX: usize = 0xb8;
// (local snapshot byte offset, ordered ActorInputs float offset, float count).
const FIELDS: &[(usize, usize, usize)] = &[
    (0x18, 0, 4),
    (0x38, 4, 4),
    (0x48, 8, 2),
    (0x50, 10, 4),
    (0x28, 14, 4),
    (0x08, 18, 4),
    (0xa4, 22, 4),
    (0x8c, 26, 2),
    (0x94, 28, 2),
    (0x9c, 30, 2),
];
const LEGACY_RANGES: &[(usize, usize)] = &[(8, 88), (0x8c, 40)];
const CONTEXT_FIELDS: &[(usize, usize, usize)] = &[
    (0x60, 0, 4),
    (0x70, 4, 4),
    (0x80, 8, 2),
    (0x88, 10, 1),
    (0xb4, 11, 1),
];
const FULL_RANGES: &[(usize, usize)] = &[(8, 176)];
type Insert = unsafe extern "system" fn(usize, usize);
type Material = unsafe extern "system" fn(usize, usize, usize, usize, usize);
static BASE: AtomicUsize = AtomicUsize::new(0);
static INSERT: AtomicUsize = AtomicUsize::new(0);
static MATERIAL: AtomicUsize = AtomicUsize::new(0);
static ENABLED: AtomicBool = AtomicBool::new(false);
static INSTALL: OnceLock<Result<(), String>> = OnceLock::new();
static CALLS: AtomicU64 = AtomicU64::new(0);
static COPIES: AtomicU64 = AtomicU64::new(0);
static REJECTED: AtomicU64 = AtomicU64::new(0);
static RESTORE_FAILURES: AtomicU64 = AtomicU64::new(0);
struct Prepared {
    description: usize,
    tag: Tag,
    target: ColorTarget,
    source: ActorInputs,
    wanted: ActorInputs,
    root: [f32; 16],
    flag: u8,
    material: crate::native_frame_material::SkinnedMaterial,
    used: AtomicBool,
}
thread_local! {
    static PENDING:RefCell<Vec<Option<Arc<Prepared>>>>=const{RefCell::new(Vec::new())};
    static PENDING_SKIPPED:Cell<usize>=const{Cell::new(0)};
    static CONSUMERS:RefCell<Vec<Option<Arc<Prepared>>>>=const{RefCell::new(Vec::new())};
    static CONSUMERS_SKIPPED:Cell<usize>=const{Cell::new(0)};
}
/// Carries no engine pointer beyond the exact native submission scope.
pub(crate) struct Scope(bool);
pub(crate) fn begin_scope() -> Scope {
    let pushed = PENDING.with(|state| {
        state.try_borrow_mut().ok().is_some_and(|mut state| {
            if state.len() < 8 {
                state.push(None);
                true
            } else {
                false
            }
        })
    });
    if !pushed {
        PENDING_SKIPPED.with(|n| n.set(n.get() + 1));
    }
    Scope(pushed)
}
impl Drop for Scope {
    fn drop(&mut self) {
        if self.0 {
            let retired = PENDING.with(|s| s.try_borrow_mut().ok().and_then(|mut s| s.pop()));
            // A final material-info reference can invoke an engine destructor.
            // Reentrant callbacks must not encounter our mutable state borrow.
            drop(retired);
        } else {
            PENDING_SKIPPED.with(|n| n.set(n.get().saturating_sub(1)));
        }
    }
}
pub(crate) fn reset_description() {
    if PENDING_SKIPPED.with(Cell::get) != 0 {
        return;
    }
    let retired = PENDING.with(|s| s.try_borrow_mut().ok()?.last_mut()?.take());
    drop(retired);
}
fn same_tag(a: &Tag, b: &Tag) -> bool {
    a.scope == b.scope
        && a.actor == b.actor
        && a.geometry == b.geometry
        && a.replacement.is_none()
        && b.replacement.is_none()
        && match (&a.role, &b.role) {
            (
                MeshRole::Cape {
                    animation_frame: a,
                    native_cape: Some(x),
                },
                MeshRole::Cape {
                    animation_frame: b,
                    native_cape: Some(y),
                },
            ) => a == b && x == y,
            (MeshRole::Elytra { animation_frame: x }, MeshRole::Elytra { animation_frame: y }) => {
                x == y && a.geometry == "geometry.elytra"
            }
            _ => false,
        }
}
pub(crate) fn installed() -> bool {
    ENABLED.load(Ordering::Acquire)
}
/// # Safety
/// description is owned by the active native constructor/submission scope.
/// Its live strong MaterialPtr permits acquiring the callback-thread info owner.
pub(crate) unsafe fn prepare(
    description: usize,
    tag: &Tag,
    target: ColorTarget,
    source: &ActorInputs,
    flag: u8,
) -> bool {
    let attempt = || -> Option<()> {
        if !installed()
            || PENDING_SKIPPED.with(Cell::get) != 0
            || !source.valid()
            || !target.current()
            || !crate::native_submit::active().is_some_and(|active| same_tag(&active, tag))
        {
            return None;
        }
        let wanted = target.shader_inputs()?;
        if !wanted.valid() {
            return None;
        }
        let root = floats::<16>(description, 1_000_000.)?;
        let material = unsafe { crate::native_frame_material::SkinnedMaterial::copy(description) }?;
        let paths = material.paths();
        if source
            .material_paths
            .as_ref()
            .is_some_and(|value| !value.matches(&paths))
            || wanted
                .material_paths
                .as_ref()
                .is_some_and(|value| !value.matches(&paths))
        {
            return None;
        }
        // Shared only on the render thread that prepared it.
        #[allow(clippy::arc_with_non_send_sync)]
        let prepared = Arc::new(Prepared {
            description,
            tag: tag.clone(),
            target,
            source: source.clone(),
            wanted,
            root,
            flag,
            material,
            used: AtomicBool::new(false),
        });
        let rejected = PENDING.with(|state| {
            let Ok(mut state) = state.try_borrow_mut() else {
                return Some(prepared);
            };
            let Some(last) = state.last_mut() else {
                return Some(prepared);
            };
            if last.is_some() {
                return Some(prepared);
            }
            *last = Some(prepared);
            None
        });
        if rejected.is_some() {
            drop(rejected);
            return None;
        }
        Some(())
    };
    attempt().is_some()
}
impl Prepared {
    fn current(&self) -> bool {
        let attempt = || -> Option<()> {
            if !self.target.current()
                || !self.material.current()
                || !crate::native_submit::active().is_some_and(|tag| same_tag(&tag, &self.tag))
            {
                return None;
            }
            let root = floats::<16>(self.description, 1_000_000.)?;
            if root.map(f32::to_bits) != self.root.map(f32::to_bits)
                || floats::<4>(self.description.checked_add(desc::COLOR)?, 100.)?.map(f32::to_bits)
                    != self.target.rgba().map(f32::to_bits)
            {
                return None;
            }
            let mut flag = [0];
            read(self.description.checked_add(desc::DITHERING)?, &mut flag).ok()?;
            (flag == [self.flag]).then_some(())
        };
        attempt().is_some()
    }
    fn reject(&self) {
        REJECTED.fetch_add(1, Ordering::Relaxed);
        crate::native_transform::invalidate_replay_layer_color(&self.tag);
    }
}
struct ConsumerScope(bool);
impl ConsumerScope {
    fn begin(entry: Option<Arc<Prepared>>) -> Self {
        let mut entry = entry;
        let pushed = CONSUMERS.with(|state| {
            state.try_borrow_mut().ok().is_some_and(|mut state| {
                if state.len() < 8 {
                    state.push(entry.take());
                    true
                } else {
                    false
                }
            })
        });
        drop(entry);
        if !pushed {
            CONSUMERS_SKIPPED.with(|n| n.set(n.get() + 1));
        }
        Self(pushed)
    }
}
impl Drop for ConsumerScope {
    fn drop(&mut self) {
        if self.0 {
            let retired = CONSUMERS.with(|s| s.try_borrow_mut().ok().and_then(|mut s| s.pop()));
            drop(retired);
        } else {
            CONSUMERS_SKIPPED.with(|n| n.set(n.get().saturating_sub(1)));
        }
    }
}
unsafe extern "system" fn insert(this: usize, variant: usize) {
    let candidate = std::panic::catch_unwind(|| {
        if !installed() || PENDING_SKIPPED.with(Cell::get) != 0 {
            return None;
        }
        let entry = PENDING.with(|s| s.try_borrow_mut().ok()?.last_mut()?.take())?;
        let mut tag = [0];
        read(variant.checked_add(8)?, &mut tag).ok()?;
        if tag != [7]
            || pointer(variant)? != entry.description
            || pointer(this)?
                != BASE
                    .load(Ordering::Acquire)
                    .checked_add(BINDING.get()?.vtable_rva)?
            || !entry.current()
        {
            entry.reject();
            return None;
        }
        Some(entry)
    })
    .ok()
    .flatten();
    // Unknown/non-skinned recursive calls mask a parent scope, too.
    let _scope = ConsumerScope::begin(candidate);
    let original: Insert = unsafe { std::mem::transmute(INSERT.load(Ordering::Acquire)) };
    unsafe { original(this, variant) };
}
fn stack_range(address: usize, bytes: usize) -> bool {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentThreadStackLimits(low: *mut usize, high: *mut usize);
    }
    let mut low = 0;
    let mut high = 0;
    unsafe { GetCurrentThreadStackLimits(&mut low, &mut high) };
    low != 0 && address >= low && address.checked_add(bytes).is_some_and(|end| end <= high)
}
fn inputs(bytes: &[u8; PREFIX]) -> Option<ActorInputs> {
    let mut values = [0f32; 32];
    for &(offset, index, count) in FIELDS {
        for i in 0..count {
            values[index + i] =
                f32::from_le_bytes(bytes[offset + i * 4..offset + i * 4 + 4].try_into().ok()?);
        }
    }
    let mut context = [0f32; 12];
    for &(offset, index, count) in CONTEXT_FIELDS {
        for i in 0..count {
            context[index + i] =
                f32::from_le_bytes(bytes[offset + i * 4..offset + i * 4 + 4].try_into().ok()?);
        }
    }
    let material_context = context
        .iter()
        .all(|v| v.is_finite() && v.abs() <= 1_000_000_000.)
        .then_some(context);
    let value = ActorInputs {
        values,
        material_context,
        material_paths: None,
    };
    value.valid().then_some(value)
}
fn overlaps(a: usize, n: usize, b: usize, m: usize) -> bool {
    match (a.checked_add(n), b.checked_add(m)) {
        (Some(end), Some(other)) => a < other && b < end,
        _ => true,
    }
}
struct Patch {
    address: usize,
    before: [u8; PREFIX],
    after: [u8; PREFIX],
    ranges: &'static [(usize, usize)],
    attempted: [bool; 2],
    restored: bool,
    entry: Arc<Prepared>,
}
impl Patch {
    fn begin(address: usize, entry: Arc<Prepared>) -> Option<Self> {
        if !stack_range(address, PREFIX)
            || overlaps(address, PREFIX, entry.description, 0x200)
            || !entry.current()
        {
            return None;
        }
        let mut before = [0; PREFIX];
        read(address, &mut before).ok()?;
        let original = inputs(&before)?;
        if pointer(address)? != entry.description
            || original.values.map(f32::to_bits) != entry.source.values.map(f32::to_bits)
            || entry.source.material_context.is_some_and(|known| {
                original.material_context.map(|v| v.map(f32::to_bits))
                    != Some(known.map(f32::to_bits))
            })
            || entry.wanted.material_context.is_some() && entry.source.material_context.is_none()
        {
            return None;
        }
        let mut after = before;
        for &(offset, index, count) in FIELDS {
            for i in 0..count {
                after[offset + i * 4..offset + i * 4 + 4]
                    .copy_from_slice(&entry.wanted.values[index + i].to_le_bytes());
            }
        }
        if let Some(context) = entry.wanted.material_context {
            for &(offset, index, count) in CONTEXT_FIELDS {
                for i in 0..count {
                    after[offset + i * 4..offset + i * 4 + 4]
                        .copy_from_slice(&context[index + i].to_le_bytes());
                }
            }
        }
        let ranges = if entry.wanted.material_context.is_some() {
            FULL_RANGES
        } else {
            LEGACY_RANGES
        };
        let mut patch = Self {
            address,
            before,
            after,
            ranges,
            attempted: [false; 2],
            restored: false,
            entry,
        };
        for (i, &(offset, bytes)) in patch.ranges.iter().enumerate() {
            patch.attempted[i] = true;
            if !crate::native_skinned_desc::write(
                address.checked_add(offset)?,
                &patch.after[offset..offset + bytes],
            ) {
                return None;
            }
        }
        let mut check = [0; PREFIX];
        read(address, &mut check).ok()?;
        (check == patch.after && patch.entry.current()).then_some(patch)
    }
    fn restore(&mut self) -> bool {
        let mut success = true;
        for (i, &(offset, bytes)) in self.ranges.iter().enumerate() {
            if self.attempted[i] {
                success &= crate::native_skinned_desc::write(
                    self.address + offset,
                    &self.before[offset..offset + bytes],
                );
            }
        }
        let mut check = [0; PREFIX];
        success &= read(self.address, &mut check).is_ok() && check == self.before;
        self.restored = success;
        success
    }
    fn complete(&mut self) -> bool {
        let mut actual = [0; PREFIX];
        let consumed =
            read(self.address, &mut actual).is_ok() && actual == self.after && self.entry.current();
        if !self.restore() {
            return false;
        }
        let animation_frame = match &self.entry.tag.role {
            MeshRole::Cape {
                animation_frame, ..
            }
            | MeshRole::Elytra { animation_frame } => *animation_frame,
            _ => return false,
        };
        consumed
            && crate::native_transform::record_replay_layer_shader_inputs(
                &self.entry.tag,
                animation_frame,
                &self.entry.target.replay(),
                &self.entry.source,
                &self.entry.wanted,
            )
    }
}
impl Drop for Patch {
    fn drop(&mut self) {
        if !self.restored && !self.restore() {
            RESTORE_FAILURES.fetch_add(1, Ordering::Relaxed);
            self.entry.reject();
        }
    }
}
unsafe extern "system" fn material(a: usize, b: usize, c: usize, d: usize, e: usize) {
    CALLS.fetch_add(1, Ordering::Relaxed);
    let entry = std::panic::catch_unwind(|| {
        if !installed() || CONSUMERS_SKIPPED.with(Cell::get) != 0 {
            return None;
        }
        CONSUMERS.with(|s| s.try_borrow().ok()?.last()?.clone())
    })
    .ok()
    .flatten();
    let mut patch = entry.as_ref().and_then(|entry| {
        if entry.used.swap(true, Ordering::AcqRel) {
            entry.reject();
            return None;
        }
        let value = std::panic::catch_unwind(|| Patch::begin(c, entry.clone()))
            .ok()
            .flatten();
        if value.is_none() {
            entry.reject();
        }
        value
    });
    let original: Material = unsafe { std::mem::transmute(MATERIAL.load(Ordering::Acquire)) };
    unsafe { original(a, b, c, d, e) };
    if let Some(patch) = patch.as_mut() {
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| patch.complete()))
            .unwrap_or(false)
        {
            COPIES.fetch_add(1, Ordering::Relaxed);
        } else {
            patch.entry.reject();
        }
    }
}
pub(crate) fn install(base: usize) -> Result<(), String> {
    INSTALL
        .get_or_init(|| {
            use hudhook::mh::{MH_CreateHook, MH_DisableHook, MH_EnableHook, MH_STATUS};
            if crate::world_hooks::image_base() != Some(base) {
                return Err("skinned consumer image unavailable".into());
            }
            crate::native_frame_material::install_skinned(base)?;
            let binding = crate::world_hooks::skinned_material_consumer_binding()?;
            let insert_prefix = [
                0x55, 0x56, 0x57, 0x53, 0x48, 0x81, 0xec, 0x08, 0x01, 0, 0, 0x48, 0x8d, 0xac, 0x24,
                0x80, 0, 0, 0,
            ];
            let mut material_prefix = [
                0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x81,
                0xec, 0, 0, 0, 0,
            ];
            material_prefix[15..].copy_from_slice(&binding.material_stack_bytes.to_le_bytes());
            let hooks = [
                (binding.insert_rva, insert as *const (), insert_prefix),
                (binding.material_rva, material as *const (), material_prefix),
            ];
            let mut targets = [0usize; 2];
            // Validate both complete prefixes before creating or enabling either
            // hook; the input copier and native consumer are one synchronous path.
            for (i, (rva, _, expected)) in hooks.iter().enumerate() {
                let target = base.checked_add(*rva).ok_or("material hook overflow")?;
                let mut actual = [0u8; 19];
                read(target, &mut actual)?;
                if actual != *expected {
                    return Err(format!("skinned consumer {rva:x} prologue differs"));
                }
                targets[i] = target;
            }
            crate::native_frame_material::install(base)?;
            BINDING
                .set(binding)
                .map_err(|_| "skinned consumer installation already attempted")?;
            BASE.store(base, Ordering::Release);
            for (i, (rva, callback, _)) in hooks.iter().enumerate() {
                let target = targets[i];
                let mut original = std::ptr::null_mut();
                let result = unsafe { MH_CreateHook(target as _, *callback as _, &mut original) };
                if result != MH_STATUS::MH_OK {
                    return Err(format!("create skinned consumer {rva:x}: {result:?}"));
                }
                (if i == 0 { &INSERT } else { &MATERIAL })
                    .store(original as usize, Ordering::Release);
            }
            // Publish both originals, then enable the consumer before its producer.
            let result = unsafe { MH_EnableHook(targets[1] as _) };
            if result != MH_STATUS::MH_OK {
                return Err(format!("enable skinned material consumer: {result:?}"));
            }
            let result = unsafe { MH_EnableHook(targets[0] as _) };
            if result != MH_STATUS::MH_OK {
                let rollback = unsafe { MH_DisableHook(targets[1] as _) };
                return Err(format!(
                    "enable skinned insertion: {result:?}; material rollback: {rollback:?}"
                ));
            }
            ENABLED.store(true, Ordering::Release);
            Ok(())
        })
        .clone()
}
pub(crate) fn status() -> Value {
    json!({"installed":installed(),"insertRva":BINDING.get().map(|b|b.insert_rva),"materialRva":BINDING.get().map(|b|b.material_rva),
        "materialStackBytes":BINDING.get().map(|b|b.material_stack_bytes),
        "calls":CALLS.load(Ordering::Relaxed),"localCopies":COPIES.load(Ordering::Relaxed),
        "rejected":REJECTED.load(Ordering::Relaxed),"restoreFailures":RESTORE_FAILURES.load(Ordering::Relaxed),
        "maximumCopiedVectorBytes":176,"legacyCopiedVectorBytes":128,"materialContextFloats":12,
        "materialReferenceGuardRequired":true,"concreteMaterialPathGuardRequired":true,
        "maximumGuardedMaterialBytesPerDescription":crate::native_frame_material::MAX_SKINNED_MATERIAL_BYTES,
        "maximumMaterialBlockReadsPerDescription":crate::native_frame_material::MAX_SKINNED_MATERIAL_BLOCK_READS,
        "materialReadBudgetScope":"copied metadata; owner acquisition and repeated endpoint guards are separate",
        "synchronousMaterialInfoOwner":crate::native_frame_material::owner_status(),
        "nativeMaterialLeaseTaken":false,"archivedMaterialIdentityMatched":false,
        "archivedDeclaredPathFingerprintMatchRequiredWhenPresent":true,
        "archivedDefineStateFingerprintMatchRequiredWhenPresent":true,"nativeDefineCacheModified":false,
        "globalsModified":false,"gpuExecutionVerified":false,"completeNativeMaterialReplayVerified":false})
}
