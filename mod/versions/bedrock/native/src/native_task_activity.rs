//! Activity identities for pinned TaskGroup callbacks and object lifetime.
//! Group addresses are opaque bounded comparison keys, never dereferenced here.
use std::sync::{
    Mutex,
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};

const REGISTER: usize = 0x6b73d0;
const COMPLETE: usize = 0x6b7120;
const CONSTRUCT: usize = 0x6b4080;
const DESTRUCT: usize = 0x6b4320;
const MAX_GROUPS: usize = 256;
type Callback = unsafe extern "system" fn(usize, usize);
type Construct = unsafe extern "system" fn(usize, usize, usize, usize) -> usize;
type Destruct = unsafe extern "system" fn(usize);
static REGISTER_ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static COMPLETE_ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static CONSTRUCT_ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static DESTRUCT_ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static READY: AtomicBool = AtomicBool::new(false);
static FAILED: AtomicBool = AtomicBool::new(false);
static MUTATING: AtomicUsize = AtomicUsize::new(0);
static UNKNOWN: AtomicUsize = AtomicUsize::new(0);
static GENERATION: AtomicU64 = AtomicU64::new(1);
static LEDGER: Mutex<Ledger> = Mutex::new(Ledger {
    sequence: 0,
    len: 0,
    groups: [Entry {
        group: 0,
        sequence: 0,
        active: 0,
    }; MAX_GROUPS],
});

#[derive(Clone, Copy)]
struct Entry {
    group: usize,
    sequence: u64,
    active: u32,
}
struct Ledger {
    sequence: u64,
    len: usize,
    groups: [Entry; MAX_GROUPS],
}
impl Ledger {
    fn next(&mut self) -> Option<u64> {
        let sequence = self.sequence.checked_add(1).or_else(|| {
            FAILED.store(true, Ordering::SeqCst);
            None
        })?;
        self.sequence = sequence;
        Some(sequence)
    }
    fn index(&mut self, group: usize) -> Option<usize> {
        if let Some(index) = self.groups[..self.len]
            .iter()
            .position(|entry| entry.group == group)
        {
            return Some(index);
        }
        let index = if self.len < MAX_GROUPS {
            self.len
        } else {
            self.groups
                .iter()
                .enumerate()
                .filter(|(_, entry)| entry.active == 0)
                .min_by_key(|(_, entry)| entry.sequence)
                .map(|(index, _)| index)?
        };
        let sequence = self.next()?;
        if index == self.len {
            self.len += 1;
        }
        self.groups[index] = Entry {
            group,
            sequence,
            active: 0,
        };
        Some(index)
    }
}
fn increment(counter: &AtomicUsize) -> bool {
    let counted = counter
        .try_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
            value.checked_add(1)
        })
        .is_ok();
    if !counted {
        FAILED.store(true, Ordering::SeqCst);
    }
    counted
}
fn invalidate() {
    if GENERATION
        .try_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
            value.checked_add(1)
        })
        .is_err()
    {
        FAILED.store(true, Ordering::SeqCst);
    }
}
struct Mutation(bool);
impl Mutation {
    fn begin() -> Self {
        Self(increment(&MUTATING))
    }
}
impl Drop for Mutation {
    fn drop(&mut self) {
        if self.0 {
            MUTATING.fetch_sub(1, Ordering::SeqCst);
        }
    }
}
struct Activity {
    group: Option<usize>,
    unknown: bool,
}
impl Activity {
    fn begin(group: usize) -> Self {
        let mutation = Mutation::begin();
        if !mutation.0 || FAILED.load(Ordering::SeqCst) {
            return Self {
                group: None,
                unknown: false,
            };
        }
        // This mutex guards Arctic's tiny ledger only. It is always released
        // before an engine original, native memory read or callback is invoked.
        let tracked = (|| {
            let mut ledger = LEDGER.lock().ok().or_else(|| {
                FAILED.store(true, Ordering::SeqCst);
                None
            })?;
            if !(0x10000..0x0000_8000_0000_0000).contains(&group) {
                return None;
            }
            let index = ledger.index(group)?;
            let sequence = ledger.next()?;
            let active = ledger.groups[index].active.checked_add(1).or_else(|| {
                FAILED.store(true, Ordering::SeqCst);
                None
            })?;
            ledger.groups[index].sequence = sequence;
            ledger.groups[index].active = active;
            Some(group)
        })();
        let unknown = tracked.is_none() && increment(&UNKNOWN);
        if unknown {
            invalidate();
        }
        Self {
            group: tracked,
            unknown,
        }
    }
}
impl Drop for Activity {
    fn drop(&mut self) {
        let _mutation = Mutation::begin();
        if let Some(group) = self.group {
            let closed = (|| {
                let mut ledger = LEDGER.lock().ok()?;
                let index = ledger.groups[..ledger.len]
                    .iter()
                    .position(|entry| entry.group == group)?;
                let active = ledger.groups[index].active.checked_sub(1)?;
                let sequence = ledger.next()?;
                ledger.groups[index].active = active;
                ledger.groups[index].sequence = sequence;
                Some(())
            })();
            if closed.is_none() {
                FAILED.store(true, Ordering::SeqCst);
            }
        }
        if self.unknown {
            invalidate();
            UNKNOWN.fetch_sub(1, Ordering::SeqCst);
        }
    }
}
#[derive(PartialEq, Eq)]
pub(crate) struct Witness {
    group: usize,
    sequence: u64,
    generation: u64,
}
pub(crate) fn sample(group: usize) -> Option<Witness> {
    if !READY.load(Ordering::Acquire)
        || FAILED.load(Ordering::SeqCst)
        || MUTATING.load(Ordering::SeqCst) != 0
        || UNKNOWN.load(Ordering::SeqCst) != 0
    {
        return None;
    }
    let mut ledger = LEDGER.try_lock().ok()?;
    let generation = GENERATION.load(Ordering::SeqCst);
    let index = ledger.index(group)?;
    let entry = &ledger.groups[index];
    if entry.active != 0
        || FAILED.load(Ordering::SeqCst)
        || MUTATING.load(Ordering::SeqCst) != 0
        || UNKNOWN.load(Ordering::SeqCst) != 0
        || GENERATION.load(Ordering::SeqCst) != generation
    {
        return None;
    }
    Some(Witness {
        group,
        sequence: entry.sequence,
        generation,
    })
}
impl Witness {
    pub(crate) fn current(&self) -> bool {
        sample(self.group).as_ref() == Some(self)
    }
    pub(crate) fn sequence(&self) -> u64 {
        self.sequence
    }
    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }
}
fn observe(group: usize) -> Option<Activity> {
    std::panic::catch_unwind(|| Activity::begin(group))
        .map_err(|_| FAILED.store(true, Ordering::SeqCst))
        .ok()
}
unsafe extern "system" fn register(group: usize, task_reference: usize) {
    let _activity = observe(group);
    // The by-value shared_ptr arrives indirectly on Windows x64. Forward its
    // native storage once; never clone, consume or retain it in Rust.
    unsafe {
        let original: Callback = std::mem::transmute(REGISTER_ORIGINAL.load(Ordering::Acquire));
        original(group, task_reference);
    }
}
unsafe extern "system" fn complete(group: usize, task: usize) {
    let _activity = observe(group);
    // gsl::not_null<BackgroundTaskBase*> is the native raw pointer argument.
    unsafe {
        let original: Callback = std::mem::transmute(COMPLETE_ORIGINAL.load(Ordering::Acquire));
        original(group, task);
    }
}
unsafe extern "system" fn construct(
    group: usize,
    workers: usize,
    scheduler: usize,
    name: usize,
) -> usize {
    let _activity = observe(group);
    // Preserve the native this-return and indirect by-value string storage.
    unsafe {
        let original: Construct = std::mem::transmute(CONSTRUCT_ORIGINAL.load(Ordering::Acquire));
        original(group, workers, scheduler, name)
    }
}
unsafe extern "system" fn destruct(group: usize) {
    let _activity = observe(group);
    // The common body covers direct destruction of embedded TaskGroups as
    // well as the deleting-destructor wrapper's call. It does not free here.
    unsafe {
        let original: Destruct = std::mem::transmute(DESTRUCT_ORIGINAL.load(Ordering::Acquire));
        original(group);
    }
}
pub(crate) fn install(base: usize) -> Result<(), String> {
    if READY.load(Ordering::Acquire) {
        return Ok(());
    }
    if crate::world_hooks::image_base() != Some(base) {
        return Err("unvalidated task activity image".into());
    }
    let sites: &[(usize, &[u8])] = &[
        (
            CONSTRUCT,
            &[
                0x41, 0x57, 0x41, 0x56, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x83, 0xec, 0x48, 0x0f,
                0x29, 0x74, 0x24, 0x30,
            ],
        ),
        (
            DESTRUCT,
            &[
                0x55, 0x56, 0x57, 0x48, 0x83, 0xec, 0x70, 0x48, 0x8d, 0x6c, 0x24, 0x70, 0x48, 0xc7,
                0x45, 0xf8, 0xfe, 0xff, 0xff, 0xff,
            ],
        ),
        (
            REGISTER,
            &[
                0x55, 0x41, 0x56, 0x56, 0x57, 0x53, 0x48, 0x83, 0xec, 0x40, 0x48, 0x8d, 0x6c, 0x24,
                0x40, 0x0f, 0x29, 0x75, 0xf0,
            ],
        ),
        (
            COMPLETE,
            &[
                0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x83, 0xec,
                0x20, 0x49, 0x89, 0xd6, 0x48, 0x89, 0xcf, 0x48, 0x8d, 0x71, 0x60,
            ],
        ),
        (0x6b74c5, &[0x48, 0xff, 0x86, 0xc8, 0, 0, 0]),
        (0x6b728c, &[0x48, 0xff, 0x8f, 0xc8, 0, 0, 0]),
        (0x6b72e3, &[0x48, 0xff, 0x8f, 0xc8, 0, 0, 0]),
        (0x6b42b3, &[0x48, 0x89, 0xf8]),
        (0x6ba2cb, &[0xe8, 0x50, 0xa0, 0xff, 0xff]),
    ];
    for &(rva, expected) in sites {
        let mut actual = vec![0; expected.len()];
        crate::world_hooks::read(
            base.checked_add(rva)
                .ok_or("task activity address overflow")?,
            &mut actual,
        )?;
        if actual != expected {
            return Err(format!("task activity profile differs at {rva:x}"));
        }
    }
    for (offset, function) in [(0x18, REGISTER), (0x38, COMPLETE)] {
        if crate::world_hooks::pointer(
            base.checked_add(0xe76b260 + offset)
                .ok_or("task activity vtable overflow")?,
        ) != base.checked_add(function)
        {
            return Err("task activity vtable differs".into());
        }
    }
    use hudhook::mh::{MH_CreateHook, MH_EnableHook, MH_STATUS};
    for (rva, callback, slot) in [
        (REGISTER, register as *const () as usize, &REGISTER_ORIGINAL),
        (COMPLETE, complete as *const () as usize, &COMPLETE_ORIGINAL),
        (
            CONSTRUCT,
            construct as *const () as usize,
            &CONSTRUCT_ORIGINAL,
        ),
        (DESTRUCT, destruct as *const () as usize, &DESTRUCT_ORIGINAL),
    ] {
        let mut original = std::ptr::null_mut();
        let status = unsafe { MH_CreateHook((base + rva) as _, callback as _, &mut original) };
        if status != MH_STATUS::MH_OK {
            return Err(format!("create task activity observer: {status:?}"));
        }
        slot.store(original as usize, Ordering::Release);
    }
    for rva in [REGISTER, COMPLETE, CONSTRUCT, DESTRUCT] {
        let status = unsafe { MH_EnableHook((base + rva) as _) };
        if status != MH_STATUS::MH_OK {
            return Err(format!("enable task activity observer: {status:?}"));
        }
    }
    READY.store(true, Ordering::Release);
    Ok(())
}
pub(crate) fn status() -> serde_json::Value {
    let ledger = LEDGER.try_lock().ok().map(|ledger| {
        (
            ledger.len,
            ledger.sequence,
            ledger.groups[..ledger.len]
                .iter()
                .map(|entry| u64::from(entry.active))
                .sum::<u64>(),
        )
    });
    serde_json::json!({"installed":READY.load(Ordering::Acquire),"trackingFailed":FAILED.load(Ordering::SeqCst),
        "groups":ledger.map(|value|value.0),"activitySequence":ledger.map(|value|value.1),
        "activeCallbacks":ledger.map(|value|value.2),"maxGroups":MAX_GROUPS,
        "unknownCallbacks":UNKNOWN.load(Ordering::SeqCst),"generation":GENERATION.load(Ordering::SeqCst),
        "tracksConstructionAndDestruction":true,"retainsTaskPointers":false,
        "taskLifecycleVerified":false,"terrainReady":false})
}
