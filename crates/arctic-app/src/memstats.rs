//! Where the launcher's memory goes, for screenshots runs (`ARCTIC_DEVSHOT_MEMORY=1`): heap in
//! use (debug builds count every allocation), egui's image caches and the GPU textures, biggest
//! first.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use eframe::egui;

const MB: f64 = 1024.0 * 1024.0;
/// Textures listed one by one.
const TOP_TEXTURES: usize = 12;

/// Debug builds: the system allocator, counting what's in use.
#[cfg_attr(not(debug_assertions), allow(dead_code))]
pub struct Counting;

static IN_USE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forwarded unchanged to the system allocator.
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            let now = IN_USE.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK.fetch_max(now, Ordering::Relaxed);
        }
        p
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: forwarded unchanged to the system allocator.
        unsafe { System.dealloc(ptr, layout) };
        IN_USE.fetch_sub(layout.size(), Ordering::Relaxed);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: forwarded unchanged to the system allocator.
        let p = unsafe { System.realloc(ptr, layout, new_size) };
        if !p.is_null() {
            IN_USE.fetch_sub(layout.size(), Ordering::Relaxed);
            let now = IN_USE.fetch_add(new_size, Ordering::Relaxed) + new_size;
            PEAK.fetch_max(now, Ordering::Relaxed);
        }
        p
    }
}

/// The process as the system sees it: (in RAM, committed), in MB.
#[cfg(windows)]
pub fn process() -> Option<(f64, f64)> {
    use windows_sys::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX};
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    let mut counters: PROCESS_MEMORY_COUNTERS_EX = unsafe { std::mem::zeroed() };
    counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
    // SAFETY: the process's own pseudo-handle and a correctly sized struct.
    let ok = unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), (&raw mut counters).cast(), counters.cb) };
    (ok != 0).then(|| (counters.WorkingSetSize as f64 / MB, counters.PrivateUsage as f64 / MB))
}

#[cfg(not(windows))]
pub fn process() -> Option<(f64, f64)> {
    None
}

/// The report, one line per part.
pub fn report(ctx: &egui::Context) -> String {
    let mut out = String::new();
    if let Some((ram, committed)) = process() {
        out += &format!("process: {ram:.1} MB in RAM, {committed:.1} MB committed\n");
    }
    if cfg!(debug_assertions) {
        out += &format!(
            "heap in use {:.1} MB (peak {:.1} MB)\n",
            IN_USE.load(Ordering::Relaxed) as f64 / MB,
            PEAK.load(Ordering::Relaxed) as f64 / MB
        );
    }
    let loaders = ctx.loaders();
    let sum = |sizes: Vec<usize>| sizes.iter().sum::<usize>() as f64 / MB;
    out += &format!(
        "image caches: file bytes {:.1} MB, decoded {:.1} MB, textures {:.1} MB\n",
        sum(loaders.bytes.lock().iter().map(|l| l.byte_size()).collect()),
        sum(loaders.image.lock().iter().map(|l| l.byte_size()).collect()),
        sum(loaders.texture.lock().iter().map(|l| l.byte_size()).collect()),
    );
    // Animation frames (`name#3`) counted together.
    let mut grouped = std::collections::BTreeMap::<String, usize>::new();
    for (_, meta) in ctx.tex_manager().read().allocated() {
        let name = meta.name.split('#').next().unwrap_or_default();
        *grouped.entry(format!("{name} {}x{}", meta.size[0], meta.size[1])).or_default() += meta.bytes_used();
    }
    let mut textures: Vec<(String, usize)> = grouped.into_iter().collect();
    textures.sort_by_key(|t| std::cmp::Reverse(t.1));
    out += &format!(
        "GPU textures: {} totaling {:.1} MB\n",
        textures.len(),
        textures.iter().map(|t| t.1).sum::<usize>() as f64 / MB
    );
    for (name, bytes) in textures.iter().take(TOP_TEXTURES) {
        out += &format!("  {:>7.2} MB  {name}\n", *bytes as f64 / MB);
    }
    out
}
