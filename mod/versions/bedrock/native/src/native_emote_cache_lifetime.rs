//! Invalidate owned pose history before the actual actor/cache destructors.
//! This observes native destruction; it does not own actors or GPU resources.
use crate::world_hooks::{ActorCacheLifetimeBinding, read};
use std::sync::{
    OnceLock,
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};

type Destroy = unsafe extern "system" fn(usize);
static BINDING: OnceLock<ActorCacheLifetimeBinding> = OnceLock::new();
static INSTALL: OnceLock<Result<(), String>> = OnceLock::new();
static ACTOR: AtomicUsize = AtomicUsize::new(0);
static CACHE: AtomicUsize = AtomicUsize::new(0);
static ENABLED: AtomicBool = AtomicBool::new(false);
static ACTOR_CALLS: AtomicU64 = AtomicU64::new(0);
static CACHE_CALLS: AtomicU64 = AtomicU64::new(0);

fn invalidate(owner: Option<usize>) {
    // Only an opaque history-key comparison uses this native argument. Do not
    // dereference a partially destructed object or require an active draw scope.
    if let Some(owner) = owner.filter(|owner| (0x10000..0x0000_8000_0000_0000).contains(owner)) {
        crate::native_transform::invalidate_owner_frame_uploads(owner);
        crate::native_emote_cache_history::invalidate_owner(owner);
    } else {
        crate::native_transform::invalidate_other_frame_uploads(None);
        crate::native_emote_cache_history::invalidate();
    }
}
unsafe extern "system" fn actor_destroy(this: usize) {
    ACTOR_CALLS.fetch_add(1, Ordering::Relaxed);
    let _ = std::panic::catch_unwind(|| invalidate(Some(this)));
    let original: Destroy = unsafe { std::mem::transmute(ACTOR.load(Ordering::Acquire)) };
    unsafe { original(this) };
}
unsafe extern "system" fn cache_destroy(this: usize) {
    CACHE_CALLS.fetch_add(1, Ordering::Relaxed);
    let _ = std::panic::catch_unwind(|| {
        invalidate(
            BINDING
                .get()
                .and_then(|binding| this.checked_sub(binding.cache_offset)),
        )
    });
    let original: Destroy = unsafe { std::mem::transmute(CACHE.load(Ordering::Acquire)) };
    unsafe { original(this) };
}
pub(super) fn install(base: usize) -> Result<(), String> {
    INSTALL.get_or_init(||{
        if crate::world_hooks::image_base()!=Some(base){return Err("cache lifetime image unavailable".into());}
        let binding=crate::world_hooks::actor_cache_lifetime_binding()?;
        let actor_prefix=binding.actor_prefix()?;
        let hooks:[(usize,*const (),&AtomicUsize,&[u8]);2]=[
            (binding.actor_rva,actor_destroy as *const (),&ACTOR,&actor_prefix),
            (binding.cache_rva,cache_destroy as *const (),&CACHE,binding.cache_prefix()),
        ];
        let mut targets=[0usize;2];
        for(i,(rva,_,_,expected))in hooks.iter().enumerate(){
            let target=base.checked_add(*rva).ok_or("cache lifetime address overflow")?;
            let mut actual=vec![0u8;expected.len()];read(target,&mut actual)?;
            if actual.as_slice()!=*expected{return Err(format!("cache lifetime {rva:x} prefix differs"));}
            targets[i]=target;
        }
        BINDING.set(binding).map_err(|_|"cache lifetime installation already attempted")?;
        use hudhook::mh::{MH_CreateHook,MH_EnableHook,MH_DisableHook,MH_STATUS};
        for(i,(rva,callback,original,_))in hooks.iter().enumerate(){
            let mut trampoline=std::ptr::null_mut();
            let status=unsafe{MH_CreateHook(targets[i] as _,*callback as _,&mut trampoline)};
            if status!=MH_STATUS::MH_OK{return Err(format!("create cache lifetime {rva:x}: {status:?}"));}
            original.store(trampoline as usize,Ordering::Release);
        }
        // Both callbacks only invalidate history and forward their original.
        // Destruction during staged enablement must also invalidate old poses.
        for(i,target)in targets.iter().enumerate(){
            let status=unsafe{MH_EnableHook(*target as _)};
            if status!=MH_STATUS::MH_OK{
                let mut failures=Vec::new();
                for prior in targets[..i].iter().rev(){
                    let rollback=unsafe{MH_DisableHook(*prior as _)};
                    if rollback!=MH_STATUS::MH_OK{failures.push(format!("{prior:x}: {rollback:?}"));}
                }
                return Err(format!("enable cache lifetime {target:x}: {status:?}; rollback failures: {failures:?}"));
            }
        }
        ENABLED.store(true,Ordering::Release);Ok(())
    }).clone()
}
pub(super) fn status() -> serde_json::Value {
    serde_json::json!({"installed":ENABLED.load(Ordering::Acquire),
        "actorDestructorRva":BINDING.get().map(|b|b.actor_rva),
        "cacheDestructorRva":BINDING.get().map(|b|b.cache_rva),
        "cacheOwnerOffset":BINDING.get().map(|b|b.cache_offset),
        "actorDestructions":ACTOR_CALLS.load(Ordering::Relaxed),
        "cacheDestructions":CACHE_CALLS.load(Ordering::Relaxed),
        "invalidatesBeforeOriginal":true,"retainsNativeOwners":false,
        "flatMutationCoverageComplete":false,"runtimeVerified":false})
}
