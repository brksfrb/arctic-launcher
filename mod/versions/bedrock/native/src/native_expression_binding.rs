//! Observe existing binding evaluations and seal their completed native copies.
//! Nothing evaluates Molang twice or changes its result. Pending native heap
//! borrows end with setupAttachableNoChecks; ActorScope retains owned outcomes.
use crate::native_attachment_binding::Copies;
use crate::native_palette_writer::Stamp;
use crate::native_scene_layout::{attachment as layout, skin_selection as selection};
use std::{
    cell::{Cell, RefCell},
    sync::{
        OnceLock,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
};

#[path = "native_expression_profile.rs"]
mod profile;

static INSTALL: OnceLock<Result<(), String>> = OnceLock::new();
static OBSERVING: AtomicBool = AtomicBool::new(false);
static SETUP_ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static EVAL_ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static EXPECTED_RETURN: AtomicUsize = AtomicUsize::new(0);
static SETUP_ACTIVE: AtomicBool = AtomicBool::new(false);
static EVAL_ACTIVE: AtomicBool = AtomicBool::new(false);
static CAPTURED: AtomicU64 = AtomicU64::new(0);
static SEALED: AtomicU64 = AtomicU64::new(0);
type SetupFn = unsafe extern "system" fn(usize, usize, usize, usize, usize, u8, u8);
type EvalFn = unsafe extern "system" fn(usize, usize) -> usize;

// This dispatcher thunk may be shared by unrelated virtual calls with other
// signatures. Forward those directly, without touching argument registers,
// vector arguments, stack slots or the native return path. Only this pinned
// two-argument call site enters the typed observer. The leaf has no stack frame.
#[cfg(target_arch = "x86_64")]
core::arch::global_asm!(
    ".p2align 4",
    ".global arctic_bedrock_parent_eval",
    "arctic_bedrock_parent_eval:",
    "mov r10, [rsp]",
    "cmp r10, qword ptr [rip + {expected}]",
    "jne .Larctic_parent_eval_forward",
    "mov r8, rbx",
    "mov r9, rsi",
    "jmp {callback}",
    ".Larctic_parent_eval_forward:",
    "jmp qword ptr [rip + {original}]",
    ".p2align 4",
    ".global arctic_bedrock_parent_eval_r12",
    "arctic_bedrock_parent_eval_r12:",
    "mov r10, [rsp]",
    "cmp r10, qword ptr [rip + {expected}]",
    "jne .Larctic_parent_eval_r12_forward",
    "mov r8, r12",
    "mov r9, rsi",
    "jmp {callback}",
    ".Larctic_parent_eval_r12_forward:",
    "jmp qword ptr [rip + {original}]",
    expected = sym EXPECTED_RETURN,
    original = sym EVAL_ORIGINAL,
    callback = sym evaluate,
);
#[cfg(target_arch = "x86_64")]
unsafe extern "system" {
    fn arctic_bedrock_parent_eval(_: usize, _: usize) -> usize;
    fn arctic_bedrock_parent_eval_r12(_: usize, _: usize) -> usize;
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
fn scalar(address: usize) -> Option<usize> {
    let mut bytes = [0; 8];
    crate::world_hooks::read(address, &mut bytes).ok()?;
    word(&bytes, 0)
}
fn frame(address: usize) -> Option<u64> {
    let mut bytes = [0; 8];
    crate::world_hooks::read(address, &mut bytes).ok()?;
    Some(u64::from_le_bytes(bytes))
}
fn child_slot(copies: &mut Copies, root: usize, slot: u32) -> Option<usize> {
    let (begin, count) = copies.vector(
        root.checked_add(layout::CHILDREN)?,
        layout::CHILD_STRIDE,
        layout::SLOTS,
    )?;
    if count != layout::SLOTS || usize::try_from(slot).ok()? >= layout::SLOTS {
        return None;
    }
    let mut children = Vec::with_capacity(count);
    for index in 0..count {
        let shared = copies.read(
            begin
                .checked_add(index.checked_mul(layout::CHILD_STRIDE)?)?
                .checked_add(layout::CHILD_SHARED)?,
            16,
        )?;
        children.push((word(&shared, 0)?, word(&shared, 8)?));
    }
    let &(child, control) = children.get(slot as usize)?;
    if children
        .iter()
        .filter(|(pointer, _)| *pointer == child)
        .count()
        != 1
    {
        return None;
    }
    copies.live(control)?;
    Some(child)
}
#[derive(Clone)]
struct Meta {
    stamp: Stamp,
    root: usize,
    slot: u32,
    serial: u64,
}
impl Meta {
    fn begin(model: usize, slot: usize, params: usize, actor: usize) -> Option<Self> {
        let stamp = crate::native_transform::palette_writer_scope()?;
        if actor != stamp.actor {
            return None;
        }
        crate::native_transform::model_binding(model, params)?;
        let root =
            scalar(actor.checked_add(crate::native_scene_layout::actor::ANIMATION_COMPONENT)?)?;
        if params != root.checked_add(selection::COMPONENT_PARAMS)? {
            return None;
        }
        let mut bytes = [0; 4];
        crate::world_hooks::read(slot, &mut bytes).ok()?;
        let slot = u32::from_le_bytes(bytes);
        if usize::try_from(slot).ok()? >= layout::SLOTS {
            return None;
        }
        let serial = crate::native_transform::begin_attachment_setup(&stamp, slot)?;
        Some(Self {
            stamp,
            root,
            slot,
            serial,
        })
    }
    fn child(&self, copies: &mut Copies) -> Option<(usize, usize)> {
        if crate::native_transform::palette_writer_scope().as_ref() != Some(&self.stamp)
            || word(
                &copies.read(
                    self.stamp
                        .actor
                        .checked_add(crate::native_scene_layout::actor::ANIMATION_COMPONENT)?,
                    8,
                )?,
                0,
            )? != self.root
        {
            return None;
        }
        let child = child_slot(copies, self.root, self.slot)?;
        if child == self.root {
            return None;
        }
        for (offset, value) in [
            (0x88, self.stamp.context),
            (0x98, child),
            (0xa0, self.root),
            (0xc0, self.stamp.actor),
        ] {
            if word(&copies.read(child.checked_add(offset)?, 8)?, 0)? != value {
                return None;
            }
        }
        let weak = copies.read(child.checked_add(0xa8)?, 16)?;
        copies.live(word(&weak, 8)?)?;
        Some((child, word(&weak, 0)?))
    }
}
struct Input {
    meta: Meta,
    child: usize,
    model: usize,
    bone: usize,
    hierarchy: u32,
    index: u32,
    expression: usize,
    source_hierarchy: u32,
    source_begin: usize,
    source_count: usize,
    copies: Copies,
}
impl Input {
    fn copy(
        meta: Meta,
        expression: usize,
        params: usize,
        parent_node: usize,
        bone: usize,
    ) -> Option<Self> {
        let mut copies = Copies::new();
        let (child, model) = meta.child(&mut copies)?;
        if params != child.checked_add(selection::COMPONENT_PARAMS)? {
            return None;
        }
        // The inspected branch copies the selected root matrix only when this
        // resource lock succeeds and its geometry range is nonempty. Otherwise
        // an old override flag/matrix could survive without a new copy.
        let resource = copies.read(meta.root.checked_add(0x2a8)?, 16)?;
        copies.live(word(&resource, 8)?)?;
        let resource = word(&resource, 0)?;
        let geometries = copies.read(resource.checked_add(0xb0)?, 16)?;
        if word(&geometries, 0)? == word(&geometries, 8)? {
            return None;
        }
        let mut target = None;
        for (_, hierarchy, begin, count) in copies.hierarchies(child)? {
            let Some(offset) = bone.checked_sub(begin) else {
                continue;
            };
            if offset.is_multiple_of(layout::BONE_BYTES) && offset / layout::BONE_BYTES < count {
                if target.is_some() {
                    return None;
                }
                target = Some((hierarchy, u32::try_from(offset / layout::BONE_BYTES).ok()?));
            }
        }
        let (hierarchy, index) = target?;
        let node = copies.read(bone.checked_add(layout::BONE_EXPRESSION)?, 16)?;
        if node[8] != 0
            || word(&node, 0)? != expression
            || copies.read(bone.checked_add(layout::BONE_METHOD)?, 1)?
                != [layout::EXPRESSION_METHOD]
        {
            return None;
        }
        copies.read(bone.checked_add(layout::BONE_HASH)?, 8)?;
        copies.read(expression, 8)?;
        let mut parent = None;
        for (node, hierarchy, begin, count) in copies.hierarchies(meta.root)? {
            if node == parent_node {
                parent = Some((hierarchy, begin, count));
            }
        }
        let (source_hierarchy, source_begin, source_count) = parent?;
        copies.unchanged()?;
        Some(Self {
            meta,
            child,
            model,
            bone,
            hierarchy,
            index,
            expression,
            source_hierarchy,
            source_begin,
            source_count,
            copies,
        })
    }
    fn complete(mut self, result: usize) -> Option<Pending> {
        // This is known child-owned heap storage. Arbitrary evaluator return
        // pointers can be native temporaries and must not outlive this call.
        if result != self.child.checked_add(0x210)? {
            return None;
        }
        self.copies.unchanged()?;
        let value = self.copies.read(result, 16)?;
        if integer(&value, 0)? != 1 {
            return None;
        }
        let hash = u64::from_le_bytes(value[8..16].try_into().ok()?);
        let mut source = None;
        for index in 0..self.source_count {
            let address = self
                .source_begin
                .checked_add(index.checked_mul(layout::BONE_BYTES)?)?;
            let key = self
                .copies
                .read(address.checked_add(layout::BONE_HASH)?, 8)?;
            if u64::from_le_bytes(key.try_into().ok()?) == hash {
                source = Some((index, address));
                break;
            }
        }
        let (index, address) = source?;
        let matrix = self
            .copies
            .read(address.checked_add(layout::BONE_MATRIX)?, 64)?;
        let matrix = std::array::from_fn(|index| {
            f32::from_le_bytes(
                matrix[index * 4..index * 4 + 4]
                    .try_into()
                    .expect("four bytes"),
            )
        });
        self.copies.unchanged()?;
        Some(Pending {
            source: crate::native_expression_history::Source {
                serial: self.meta.serial,
                child: self.child,
                model: self.model,
                slot: self.meta.slot,
                hierarchy: self.hierarchy,
                bone: self.index,
                expression: self.expression,
                source_hierarchy: self.source_hierarchy,
                source_bone: u32::try_from(index).ok()?,
                source_hash: hash,
                matrix,
            },
            input: self,
        })
    }
}
struct Pending {
    source: crate::native_expression_history::Source,
    input: Input,
}
impl Pending {
    fn seal(
        self,
        meta: &Meta,
        child: usize,
        model: usize,
    ) -> Option<crate::native_expression_history::Source> {
        if self.input.meta.serial != meta.serial
            || self.input.meta.stamp != meta.stamp
            || self.source.child != child
            || self.source.model != model
        {
            return None;
        }
        self.input.copies.unchanged()?;
        let mut flags = [0; 3];
        crate::world_hooks::read(
            self.input.bone.checked_add(layout::BONE_METHOD)?,
            &mut flags,
        )
        .ok()?;
        if flags[0] != layout::EXPRESSION_METHOD || flags[2] != 1 {
            return None;
        }
        let matrix = crate::world_hooks::floats::<16>(
            self.input.bone.checked_add(layout::BONE_MATRIX)?,
            1_000_000.,
        )?;
        if !matrix
            .iter()
            .zip(&self.source.matrix)
            .all(|(a, b)| a.to_bits() == b.to_bits())
        {
            return None;
        }
        self.input.copies.unchanged()?;
        Some(self.source)
    }
}
struct Scope {
    meta: Meta,
    pending: Vec<Pending>,
    bytes: usize,
    invalid: bool,
}
impl Scope {
    fn seal(self) {
        if self.invalid {
            return;
        }
        let mut copies = Copies::new();
        let Some((child, model)) = self.meta.child(&mut copies) else {
            return;
        };
        let Some(frame_address) = child.checked_add(selection::COMPONENT_FRAME) else {
            return;
        };
        let Some(reload_address) = child.checked_add(layout::COMPONENT_RELOAD) else {
            return;
        };
        let Some(initialized_address) = child.checked_add(0x358) else {
            return;
        };
        let Some(child_frame) = frame(frame_address) else {
            return;
        };
        let Some(reload) = frame(reload_address) else {
            return;
        };
        let mut initialized = [0];
        if crate::world_hooks::read(initialized_address, &mut initialized).is_err()
            || initialized != [1]
        {
            return;
        }
        let sources = self
            .pending
            .into_iter()
            .filter_map(|pending| pending.seal(&self.meta, child, model))
            .collect();
        if copies.unchanged().is_none()
            || frame(frame_address) != Some(child_frame)
            || frame(reload_address) != Some(reload)
        {
            return;
        }
        if crate::native_transform::finish_attachment_setup(
            &self.meta.stamp,
            self.meta.slot,
            self.meta.serial,
            child,
            model,
            child_frame,
            reload,
            sources,
        ) {
            SEALED.fetch_add(1, Ordering::Relaxed);
        }
    }
}
thread_local! {
    static ACTIVE:RefCell<Vec<Option<Scope>>>=const{RefCell::new(Vec::new())};
    static SKIPPED:Cell<usize>=const{Cell::new(0)};
}
struct Guard {
    pushed: bool,
    skipped: bool,
}
impl Guard {
    fn begin(meta: Option<Meta>) -> Self {
        let pushed = ACTIVE
            .try_with(|active| {
                let Ok(mut active) = active.try_borrow_mut() else {
                    return false;
                };
                if active.len() >= 8 {
                    return false;
                }
                active.push(meta.map(|meta| Scope {
                    meta,
                    pending: Vec::new(),
                    bytes: 0,
                    invalid: false,
                }));
                true
            })
            .unwrap_or(false);
        let skipped = !pushed
            && SKIPPED
                .try_with(|skip| skip.set(skip.get().saturating_add(1)))
                .is_ok();
        Self { pushed, skipped }
    }
    fn finish(mut self) -> Option<Scope> {
        if !self.pushed {
            return None;
        }
        let scope = ACTIVE
            .try_with(|active| active.try_borrow_mut().ok()?.pop())
            .ok()
            .flatten()?;
        self.pushed = false;
        scope
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        if self.pushed {
            let _ = ACTIVE.try_with(|active| {
                active
                    .try_borrow_mut()
                    .ok()
                    .and_then(|mut active| active.pop())
            });
        }
        if self.skipped {
            let _ = SKIPPED.try_with(|skip| skip.set(skip.get().saturating_sub(1)));
        }
    }
}
unsafe extern "system" fn setup(
    model: usize,
    item: usize,
    slot: usize,
    params: usize,
    actor: usize,
    first: u8,
    parent: u8,
) {
    if !OBSERVING.load(Ordering::Acquire) || !crate::native_body::enabled() {
        // Quarantine a nested setup even if the mode changes during the native
        // call; its evaluations must not borrow an older outer setup scope.
        let _guard = Guard::begin(None);
        unsafe {
            let original: SetupFn = std::mem::transmute(SETUP_ORIGINAL.load(Ordering::Acquire));
            original(model, item, slot, params, actor, first, parent);
        }
        return;
    }
    let _ = std::panic::catch_unwind(|| {
        let mut value = [0; 4];
        let slot = crate::world_hooks::read(slot, &mut value)
            .ok()
            .map(|_| u32::from_le_bytes(value));
        crate::native_transform::invalidate_attachment_setup(actor, slot);
    });
    let meta = std::panic::catch_unwind(|| Meta::begin(model, slot, params, actor))
        .ok()
        .flatten();
    let guard = Guard::begin(meta);
    unsafe {
        let original: SetupFn = std::mem::transmute(SETUP_ORIGINAL.load(Ordering::Acquire));
        original(model, item, slot, params, actor, first, parent);
    }
    let _ = std::panic::catch_unwind(|| {
        if let Some(scope) = guard.finish() {
            scope.seal();
        }
    });
}
unsafe extern "system" fn evaluate(
    expression: usize,
    params: usize,
    parent_node: usize,
    bone: usize,
) -> usize {
    let input = std::panic::catch_unwind(|| {
        if !OBSERVING.load(Ordering::Acquire)
            || !crate::native_body::enabled()
            || SKIPPED.try_with(Cell::get).unwrap_or(1) != 0
        {
            return None;
        }
        let meta = ACTIVE
            .try_with(|active| {
                active
                    .try_borrow()
                    .ok()?
                    .last()?
                    .as_ref()
                    .filter(|scope| !scope.invalid)
                    .map(|scope| scope.meta.clone())
            })
            .ok()
            .flatten()?;
        Input::copy(meta, expression, params, parent_node, bone)
    })
    .ok()
    .flatten();
    let result = unsafe {
        let original: EvalFn = std::mem::transmute(EVAL_ORIGINAL.load(Ordering::Acquire));
        original(expression, params)
    };
    let _ = std::panic::catch_unwind(|| {
        let Some(pending) = input.and_then(|input| input.complete(result)) else {
            return;
        };
        let bytes = pending.input.copies.retained_bytes()
            + std::mem::size_of::<Pending>()
            + pending.input.meta.stamp.actor_id.capacity();
        let _ = ACTIVE.try_with(|active| {
            let Ok(mut active) = active.try_borrow_mut() else {
                return;
            };
            let Some(scope) = active.last_mut().and_then(Option::as_mut) else {
                return;
            };
            if scope.invalid
                || scope.meta.serial != pending.input.meta.serial
                || scope.meta.stamp != pending.input.meta.stamp
            {
                return;
            }
            if scope.pending.len() >= 512
                || scope
                    .bytes
                    .checked_add(bytes)
                    .is_none_or(|size| size > 2 * 1024 * 1024)
            {
                scope.invalid = true;
                scope.pending.clear();
                return;
            }
            scope.bytes += bytes;
            scope.pending.push(pending);
            CAPTURED.fetch_add(1, Ordering::Relaxed);
        });
    });
    result
}
/// Admit immutable setup/evaluator bytes before any dependent reader or detour.
/// This returns the actual setup entry and never creates or enables a hook.
pub(crate) fn preflight(base: usize) -> Result<usize, String> {
    Ok(profile::binding(base)?.setup)
}
pub(crate) fn install(base: usize) -> Result<(), String> {
    // Install once for the exact image even when the preference starts off;
    // callbacks pass straight through until normal/probe mode enables them.
    #[cfg(not(target_arch = "x86_64"))]
    {
        let _ = base;
        return Err("native binding observer requires Windows x64".into());
    }
    #[cfg(target_arch = "x86_64")]
    {
        let binding = profile::binding(base)?;
        INSTALL
            .get_or_init(|| {
                let callback = match binding.parent {
                    profile::ParentRegister::Rbx => {
                        arctic_bedrock_parent_eval as *const () as usize
                    }
                    profile::ParentRegister::R12 => {
                        arctic_bedrock_parent_eval_r12 as *const () as usize
                    }
                };
                EXPECTED_RETURN.store(
                    base.checked_add(binding.return_rva)
                        .ok_or("binding return overflow")?,
                    Ordering::Release,
                );
                use hudhook::mh::{MH_CreateHook, MH_EnableHook, MH_STATUS};
                for (rva, callback, slot) in [
                    (binding.setup, setup as *const () as usize, &SETUP_ORIGINAL),
                    (binding.evaluator, callback, &EVAL_ORIGINAL),
                ] {
                    let mut original = std::ptr::null_mut();
                    let target = base
                        .checked_add(rva)
                        .ok_or("binding hook address overflow")?;
                    let status =
                        unsafe { MH_CreateHook(target as _, callback as _, &mut original) };
                    if status != MH_STATUS::MH_OK {
                        return Err(format!("create binding observer: {status:?}"));
                    }
                    slot.store(original as usize, Ordering::Release);
                }
                for (rva, active) in [
                    (binding.setup, &SETUP_ACTIVE),
                    (binding.evaluator, &EVAL_ACTIVE),
                ] {
                    let target = base
                        .checked_add(rva)
                        .ok_or("binding hook address overflow")?;
                    let status = unsafe { MH_EnableHook(target as _) };
                    if status != MH_STATUS::MH_OK {
                        return Err(format!("enable binding observer: {status:?}"));
                    }
                    active.store(true, Ordering::Release);
                }
                OBSERVING.store(true, Ordering::Release);
                Ok(())
            })
            .clone()
    }
}
pub(crate) fn status() -> serde_json::Value {
    serde_json::json!({"setupInstalled":SETUP_ACTIVE.load(Ordering::Acquire),
        "evaluatorInstalled":EVAL_ACTIVE.load(Ordering::Acquire),"captured":CAPTURED.load(Ordering::Relaxed),
        "observersEnabled":OBSERVING.load(Ordering::Acquire),"exactFamilyHookSourceImplemented":true,
        "setupRva":profile::current().map(|binding|binding.setup),
        "evaluatorRva":profile::current().map(|binding|binding.evaluator),
        "parentRegister":profile::current().map(|binding|match binding.parent{profile::ParentRegister::Rbx=>"rbx",profile::ParentRegister::R12=>"r12"}),
        "sealedSetups":SEALED.load(Ordering::Relaxed),"ownedScratchResultsOnly":true,
        "nativeDynamicParentBindingsVerified":false})
}
