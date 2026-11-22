//! Version-tolerant core game-state bindings: whether a world is loaded,
//! whether a game screen is open, the local player's position, and the
//! game's own cursor grab/release. See `core_signatures` for the sources.
//!
//! These are independent of the exact-build cosmetics observers in
//! `world_hooks`: a minor game update that moves code only needs the
//! signatures to still match, and the self-check names each one that doesn't.
//! Every pointer read goes through `ReadProcessMemory`, and every object
//! pointer is checked against its MSVC RTTI class name before use.

use std::collections::{HashMap, VecDeque};
use std::panic::catch_unwind;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use hudhook::mh::{MH_CreateHook, MH_EnableHook, MH_STATUS};
use hudhook::windows::Win32::System::LibraryLoader::GetModuleHandleW;

use crate::core_signatures as sig;
use crate::sigscan::{self, Outcome, Pattern, Section};

/// Unknown calibration offset.
const UNSET: usize = usize::MAX;

struct Image {
    base: usize,
    size: usize,
    sections: Vec<Section>,
}

impl Image {
    fn contains(&self, address: usize) -> bool {
        address >= self.base && address < self.base + self.size
    }
    fn executable(&self, address: usize) -> bool {
        self.sections.iter().any(|s| {
            s.executable && address >= self.base + s.rva && address < self.base + s.rva + s.size
        })
    }
}

static IMAGE: OnceLock<Image> = OnceLock::new();
static SETUP_AND_RENDER: AtomicUsize = AtomicUsize::new(0);
static PROCESS_INPUT_QUEUE: AtomicUsize = AtomicUsize::new(0);
static GRAB_CURSOR: AtomicUsize = AtomicUsize::new(0);
static RELEASE_CURSOR: AtomicUsize = AtomicUsize::new(0);
static MOUSE_DEVICE: AtomicUsize = AtomicUsize::new(0);
static SCREEN_HOOK: AtomicBool = AtomicBool::new(false);
static QUEUE_HOOK: AtomicBool = AtomicBool::new(false);

/// Arctic's own menu is open (set from the render thread).
static MENU_OPEN: AtomicBool = AtomicBool::new(false);
/// 0 nothing, 1 release the cursor, 2 grab it back; run on the game's UI thread.
static CURSOR_REQUEST: AtomicU8 = AtomicU8::new(0);
/// We released a grabbed cursor and owe the game a grab.
static RELEASED_BY_US: AtomicBool = AtomicBool::new(false);
static CLIENT: AtomicUsize = AtomicUsize::new(0);

static CONTEXT_OFFSET: AtomicUsize = AtomicUsize::new(UNSET);
static NAME_PATH: Mutex<Option<(usize, usize, usize)>> = Mutex::new(None);
static STATE_VECTOR_OFFSET: AtomicUsize = AtomicUsize::new(UNSET);
static ROTATION_OFFSET: AtomicUsize = AtomicUsize::new(UNSET);
static GAME_OFFSET: AtomicUsize = AtomicUsize::new(UNSET);

struct Observed {
    /// Root control names seen recently, with when.
    screens: VecDeque<(String, Instant)>,
    /// Every distinct name ever seen (logged once each).
    logged: Vec<String>,
    position: Option<([f64; 5], Instant)>,
    cursor_grabbed: Option<bool>,
    rtti: HashMap<usize, Option<String>>,
    notes: Vec<String>,
}

static OBSERVED: Mutex<Option<Observed>> = Mutex::new(None);

fn with_observed<T>(run: impl FnOnce(&mut Observed) -> T) -> Option<T> {
    let mut guard = OBSERVED.lock().ok()?;
    let state = guard.get_or_insert_with(|| Observed {
        screens: VecDeque::new(),
        logged: Vec::new(),
        position: None,
        cursor_grabbed: None,
        rtti: HashMap::new(),
        notes: Vec::new(),
    });
    Some(run(state))
}

/// Log once from any thread (the hook threads must not do file I/O inline
/// more than needed, so notes are queued and flushed by `state()`).
fn note(message: String) {
    with_observed(|o| {
        if o.notes.len() < 256 {
            o.notes.push(message)
        }
    });
}

fn read(address: usize, output: &mut [u8]) -> bool {
    crate::world_hooks::read(address, output).is_ok()
}
fn read_usize(address: usize) -> Option<usize> {
    let mut b = [0u8; 8];
    read(address, &mut b).then(|| u64::from_le_bytes(b) as usize)
}
fn read_u32(address: usize) -> Option<u32> {
    let mut b = [0u8; 4];
    read(address, &mut b).then(|| u32::from_le_bytes(b))
}
fn read_f32s<const N: usize>(address: usize) -> Option<[f32; N]> {
    let mut b = vec![0u8; N * 4];
    if !read(address, &mut b) {
        return None;
    }
    Some(std::array::from_fn(|i| {
        f32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().expect("four bytes"))
    }))
}
fn plausible_heap(pointer: usize) -> bool {
    (0x10000..0x0000_8000_0000_0000).contains(&pointer) && pointer.is_multiple_of(8)
}

/// MSVC RTTI: vtable[-1] -> CompleteObjectLocator -> TypeDescriptor name.
fn class_name(object: usize, cache: &mut HashMap<usize, Option<String>>) -> Option<String> {
    let image = IMAGE.get()?;
    let vtable = read_usize(object)?;
    if !image.contains(vtable) {
        return None;
    }
    if let Some(name) = cache.get(&vtable) {
        return name.clone();
    }
    let name = (|| {
        let locator = read_usize(vtable - 8)?;
        if !image.contains(locator) || read_u32(locator)? != 1 {
            return None;
        }
        let descriptor = image.base + read_u32(locator + 12)? as usize;
        if !image.contains(descriptor) {
            return None;
        }
        let mut raw = [0u8; 96];
        if !read(descriptor + 16, &mut raw) {
            return None;
        }
        let end = raw.iter().position(|b| *b == 0)?;
        String::from_utf8(raw[..end].to_vec()).ok()
    })();
    if cache.len() < 4096 {
        cache.insert(vtable, name.clone());
    }
    name
}

fn is_class(object: usize, rtti: &str) -> bool {
    plausible_heap(object)
        && with_observed(|o| class_name(object, &mut o.rtti).as_deref() == Some(rtti))
            .unwrap_or(false)
}

fn read_name(address: usize) -> Option<String> {
    let mut raw = [0u8; 32];
    if !read(address, &mut raw) {
        return None;
    }
    sigscan::msvc_string(&raw, 96, |at, len| {
        let mut out = vec![0u8; len];
        read(at, &mut out).then_some(out)
    })
    .filter(|n| sigscan::plausible_name(n))
}

/// ScreenView -> VisualTree -> root control -> name, calibrated once.
fn screen_name(view: usize) -> Option<String> {
    let walk = |(a, b, c): (usize, usize, usize)| -> Option<String> {
        let tree = read_usize(view + a).filter(|p| plausible_heap(*p))?;
        let root = read_usize(tree + b).filter(|p| plausible_heap(*p))?;
        read_name(root + c)
    };
    let known = *NAME_PATH.lock().ok()?;
    if let Some(path) = known {
        return walk(path);
    }
    let first = (
        sig::VIEW_TREE.known,
        sig::TREE_ROOT.known,
        sig::CONTROL_NAME.known,
    );
    let mut candidates = vec![first];
    for a in (sig::VIEW_TREE.scan.0..=sig::VIEW_TREE.scan.1).step_by(8) {
        for b in (sig::TREE_ROOT.scan.0..=sig::TREE_ROOT.scan.1).step_by(8) {
            for c in (sig::CONTROL_NAME.scan.0..=sig::CONTROL_NAME.scan.1).step_by(8) {
                if (a, b, c) != first {
                    candidates.push((a, b, c));
                }
            }
        }
    }
    if !scan_allowed("UIControl.name path") {
        return None;
    }
    for path in candidates {
        if let Some(name) = walk(path).filter(|n| n.contains("screen")) {
            *NAME_PATH.lock().ok()? = Some(path);
            note(format!(
                "self-check field ScreenView.visualTree/VisualTree.rootControl/UIControl.name: {} (+{:#x}/+{:#x}/+{:#x}), first screen {name:?}",
                if path == first {
                    "verified at the known offsets"
                } else {
                    "CALIBRATED to new offsets"
                },
                path.0,
                path.1,
                path.2
            ));
            return Some(name);
        }
    }
    None
}

static SCAN_FAILURES: Mutex<Option<HashMap<&'static str, u32>>> = Mutex::new(None);
const SCAN_LIMIT: u32 = 600;

/// Count a calibration attempt for `name`; false once it has failed too often.
/// A success caches the offset, so only failures ever get here repeatedly.
fn scan_allowed(name: &'static str) -> bool {
    let Ok(mut guard) = SCAN_FAILURES.lock() else {
        return false;
    };
    let count = guard
        .get_or_insert_with(HashMap::new)
        .entry(name)
        .or_insert(0);
    *count += 1;
    if *count == SCAN_LIMIT {
        note(format!(
            "self-check field {name}: NOT FOUND after {SCAN_LIMIT} tries; scanning stopped"
        ));
    }
    *count < SCAN_LIMIT
}

/// Find a pointer field of `object` that `check` accepts, preferring `field.known`.
fn calibrate(
    object: usize,
    field: &sig::Field,
    cache: &AtomicUsize,
    check: impl Fn(usize) -> bool,
) -> Option<usize> {
    let cached = cache.load(Ordering::Acquire);
    if cached != UNSET {
        return read_usize(object + cached).filter(|p| check(*p));
    }
    // After many failed scans the layout has moved beyond the scan range:
    // stop scanning (it costs reads every frame) and say so once.
    if !scan_allowed(field.name) {
        return read_usize(object + field.known).filter(|p| plausible_heap(*p) && check(*p));
    }
    let order = std::iter::once(field.known).chain(
        (field.scan.0..=field.scan.1)
            .step_by(8)
            .filter(|o| *o != field.known),
    );
    for offset in order {
        if let Some(target) =
            read_usize(object + offset).filter(|p| plausible_heap(*p) && check(*p))
        {
            cache.store(offset, Ordering::Release);
            note(format!(
                "self-check field {}: {} (+{offset:#x})",
                field.name,
                if offset == field.known {
                    "verified at the known offset"
                } else {
                    "CALIBRATED to a new offset"
                }
            ));
            return Some(target);
        }
    }
    None
}

fn client_from_context(context: usize) -> Option<usize> {
    let cached = CONTEXT_OFFSET.load(Ordering::Acquire);
    if cached != UNSET {
        return read_usize(context + cached).filter(|p| is_class(*p, sig::RTTI_CLIENT_INSTANCE));
    }
    calibrate(context, &sig::CONTEXT_CLIENT, &CONTEXT_OFFSET, |p| {
        is_class(p, sig::RTTI_CLIENT_INSTANCE)
    })
}

fn local_player(client: usize) -> Option<usize> {
    let image = IMAGE.get()?;
    let vtable = read_usize(client)?;
    let function = read_usize(vtable + sig::CLIENT_GET_LOCAL_PLAYER_SLOT * 8)?;
    if !image.executable(function) {
        return None;
    }
    // SAFETY: an RTTI-verified ClientInstance's own virtual getter, called on
    // the game's UI thread inside its screen render (as Latite does each frame);
    // the result is only used after its RTTI name proves it is a LocalPlayer.
    let player = unsafe {
        let getter: unsafe extern "system" fn(usize) -> usize = std::mem::transmute(function);
        getter(client)
    };
    is_class(player, sig::RTTI_LOCAL_PLAYER).then_some(player)
}

fn player_position(player: usize) -> Option<[f64; 5]> {
    let state = calibrate(
        player,
        &sig::ACTOR_STATE_VECTOR,
        &STATE_VECTOR_OFFSET,
        |p| read_f32s::<9>(p).is_some_and(|v| sig::plausible_state_vector(&v)),
    )?;
    let v = read_f32s::<9>(state).filter(sig::plausible_state_vector)?;
    let rotation = calibrate(player, &sig::ACTOR_ROTATION, &ROTATION_OFFSET, |p| {
        p != state && read_f32s::<4>(p).is_some_and(|r| sig::plausible_rotation(&r))
    })
    .and_then(read_f32s::<4>)
    .filter(sig::plausible_rotation);
    let (pitch, yaw) = rotation.map_or((0.0, 0.0), |r| (f64::from(r[0]), f64::from(r[1])));
    Some([
        f64::from(v[0]),
        f64::from(v[1]) - sig::PLAYER_EYE_HEIGHT,
        f64::from(v[2]),
        yaw,
        pitch,
    ])
}

fn cursor_grabbed(client: usize) -> Option<bool> {
    let game = calibrate(client, &sig::CLIENT_GAME, &GAME_OFFSET, |p| {
        is_class(p, sig::RTTI_MINECRAFT_GAME)
    })?;
    let mut b = [0u8; 1];
    read(game + sig::GAME_CURSOR_GRABBED, &mut b).then_some(())?;
    match b[0] {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

fn call_cursor(function: &AtomicUsize, client: usize) -> bool {
    let address = function.load(Ordering::Acquire);
    if address == 0 || client == 0 {
        return false;
    }
    // SAFETY: a signature-resolved ClientInstance member taking only `this`,
    // invoked on the game's UI thread with an RTTI-verified ClientInstance.
    unsafe {
        let call: unsafe extern "system" fn(usize) = std::mem::transmute(address);
        call(client);
    }
    true
}

type SetupAndRenderFn = unsafe extern "system" fn(usize, usize);
unsafe extern "system" fn setup_and_render(view: usize, context: usize) {
    let original: SetupAndRenderFn =
        unsafe { std::mem::transmute(SETUP_AND_RENDER.load(Ordering::Acquire)) };
    unsafe { original(view, context) };
    let _ = catch_unwind(|| observe_screen(view, context));
}

fn observe_screen(view: usize, context: usize) {
    let name = screen_name(view);
    let client = client_from_context(context);
    if let Some(client) = client {
        CLIENT.store(client, Ordering::Release);
    }
    let hud = name.as_deref() == Some("hud_screen");
    let position = match (hud, client) {
        (true, Some(client)) => local_player(client).and_then(player_position),
        _ => None,
    };
    let grabbed = client.and_then(cursor_grabbed);
    // Cursor requests run here, on the game's own UI thread.
    if let Some(client) = client {
        match CURSOR_REQUEST.swap(0, Ordering::AcqRel) {
            1 => {
                if grabbed != Some(false) && call_cursor(&RELEASE_CURSOR, client) {
                    RELEASED_BY_US.store(true, Ordering::Release);
                }
            }
            2 => {
                // Only re-capture where the game itself had it: playing in a
                // world with no game screen (never on a menu). Other screens
                // render in the same frame; wait for the HUD pass to decide.
                if hud {
                    if RELEASED_BY_US.swap(false, Ordering::AcqRel) && !game_screen_open() {
                        call_cursor(&GRAB_CURSOR, client);
                    }
                } else if in_world_recently() {
                    let _ =
                        CURSOR_REQUEST.compare_exchange(0, 2, Ordering::AcqRel, Ordering::Acquire);
                } else {
                    RELEASED_BY_US.store(false, Ordering::Release);
                }
            }
            _ => {
                // The game may grab again on its own (focus, respawn) while
                // the Arctic menu is up: keep it released, as Latite does.
                if MENU_OPEN.load(Ordering::Acquire) && grabbed == Some(true) && hud {
                    call_cursor(&RELEASE_CURSOR, client);
                    RELEASED_BY_US.store(true, Ordering::Release);
                }
            }
        }
    }
    let now = Instant::now();
    with_observed(|o| {
        if let Some(name) = name {
            if !o.logged.contains(&name) && o.logged.len() < 128 {
                o.logged.push(name.clone());
                o.notes.push(format!("game screen seen: {name}"));
            }
            o.screens.push_back((name, now));
            while o.screens.len() > 64 {
                o.screens.pop_front();
            }
        }
        if let Some(position) = position {
            o.position = Some((position, now));
        }
        if grabbed.is_some() {
            o.cursor_grabbed = grabbed;
        }
    });
}

fn recent_screens(o: &Observed, within: Duration) -> Vec<&str> {
    let now = Instant::now();
    o.screens
        .iter()
        .filter(|(_, at)| now.duration_since(*at) <= within)
        .map(|(n, _)| n.as_str())
        .collect()
}

fn in_world_recently() -> bool {
    with_observed(|o| recent_screens(o, Duration::from_millis(250)).contains(&"hud_screen"))
        .unwrap_or(false)
}

fn game_screen_open() -> bool {
    with_observed(|o| {
        recent_screens(o, Duration::from_millis(250))
            .iter()
            .any(|n| !sig::OVERLAY_SCREENS.contains(n))
    })
    .unwrap_or(false)
}

/// Clear the game's queued mouse events before it handles them while the
/// Arctic menu is open (it consumes the queue; clicks would act on the world).
type ProcessInputQueueFn = unsafe extern "system" fn(usize, usize, usize) -> u8;
unsafe extern "system" fn process_input_queue(a: usize, b: usize, c: usize) -> u8 {
    if MENU_OPEN.load(Ordering::Acquire) {
        let _ = catch_unwind(clear_mouse_queue);
    }
    let original: ProcessInputQueueFn =
        unsafe { std::mem::transmute(PROCESS_INPUT_QUEUE.load(Ordering::Acquire)) };
    unsafe { original(a, b, c) }
}

fn clear_mouse_queue() {
    // MouseDevice: 8 x i16, bool[7], then std::vector<MouseAction> {begin, end, capacity}.
    let device = MOUSE_DEVICE.load(Ordering::Acquire);
    if device == 0 {
        return;
    }
    let vector = device + 24;
    let (Some(begin), Some(end), Some(capacity)) = (
        read_usize(vector),
        read_usize(vector + 8),
        read_usize(vector + 16),
    ) else {
        return;
    };
    // MouseAction is 20 bytes; anything else means the layout moved: leave it.
    if begin == 0
        || begin > end
        || end > capacity
        || (end - begin) % 20 != 0
        || (capacity - begin) % 20 != 0
    {
        return;
    }
    if end != begin {
        // SAFETY: the game's own static MouseDevice vector end pointer, with the
        // vector checked as {begin <= end <= capacity}; MouseAction is trivially
        // destructible, so shrinking it to empty is what clear() does.
        unsafe { std::ptr::write_volatile((vector + 8) as *mut usize, begin) };
    }
}

/// Result of the core bindings for one presented frame.
#[derive(Clone, Debug, Default)]
pub struct GameState {
    /// The screen hook is installed and has observed the game.
    pub bound: bool,
    pub in_world: bool,
    pub screen_open: bool,
    pub top_screen: Option<String>,
    pub position: Option<[f64; 5]>,
    pub cursor_grabbed: Option<bool>,
}

/// Current game state plus queued self-check notes for the log.
pub fn state(log: &dyn Fn(String)) -> GameState {
    if !SCREEN_HOOK.load(Ordering::Acquire) {
        return GameState::default();
    }
    let result = with_observed(|o| {
        for note in o.notes.drain(..) {
            log(note);
        }
        let recent = recent_screens(o, Duration::from_millis(250));
        let bound = !o.logged.is_empty();
        let in_world = recent.contains(&"hud_screen");
        let screen_open = recent.iter().any(|n| !sig::OVERLAY_SCREENS.contains(n));
        let top_screen = recent.last().map(|s| (*s).to_owned());
        let position = o
            .position
            .filter(|(_, at)| at.elapsed() < Duration::from_millis(500))
            .map(|(p, _)| p)
            .filter(|_| in_world);
        GameState {
            bound,
            in_world,
            screen_open,
            top_screen,
            position,
            cursor_grabbed: o.cursor_grabbed,
        }
    });
    result.unwrap_or_default()
}

pub fn cursor_binding() -> bool {
    RELEASE_CURSOR.load(Ordering::Acquire) != 0 && GRAB_CURSOR.load(Ordering::Acquire) != 0
}

/// The Arctic menu opened or closed: release the game's cursor (and keep it
/// released) while open, and give it back on close.
pub fn menu_changed(open: bool) {
    MENU_OPEN.store(open, Ordering::Release);
    CURSOR_REQUEST.store(if open { 1 } else { 2 }, Ordering::Release);
}

/// Scan, log the self-check and install the core hooks. Never gated on a PE
/// timestamp: each binding stands on its own signature.
pub fn install(log: &dyn Fn(String)) {
    let is_game = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_owned()))
        .is_some_and(|n| {
            n.to_string_lossy()
                .eq_ignore_ascii_case("Minecraft.Windows.exe")
        });
    if !is_game {
        log("core bindings: not running inside Minecraft.Windows.exe; skipped".into());
        return;
    }
    let Ok(module) = (unsafe { GetModuleHandleW(None) }) else {
        log("core bindings: no game module handle".into());
        return;
    };
    let base = module.0 as usize;
    let mut header = [0u8; 4096];
    if !read(base, &mut header) {
        log("core bindings: couldn't read the game's PE header".into());
        return;
    }
    let sections = match sigscan::sections(&header) {
        Ok(s) => s,
        Err(e) => {
            log(format!("core bindings: {e}"));
            return;
        }
    };
    let size = sections.iter().map(|s| s.rva + s.size).max().unwrap_or(0);
    let pe = u32::from_le_bytes(header[60..64].try_into().unwrap()) as usize;
    let timestamp = header
        .get(pe + 8..pe + 12)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .unwrap_or(0);
    log(format!(
        "core bindings: signature self-check on the loaded image (PE timestamp {timestamp}, {} sections, {:.1} MiB)",
        sections.len(),
        size as f64 / 1048576.0
    ));
    let _ = IMAGE.set(Image {
        base,
        size,
        sections: sections.clone(),
    });
    let started = Instant::now();
    let mut found = HashMap::new();
    for (name, text, how, purpose) in sig::SIGNATURES {
        let pattern = match Pattern::parse(text) {
            Ok(p) => p,
            Err(e) => {
                log(format!("self-check signature {name}: INVALID ({e})"));
                continue;
            }
        };
        let mut outcome = Outcome::Missing;
        let mut total = 0usize;
        for section in sections.iter().filter(|s| s.executable && s.size > 0) {
            // SAFETY: an executable section of the loaded main image is
            // committed and readable for the process lifetime.
            let data = unsafe {
                std::slice::from_raw_parts((base + section.rva) as *const u8, section.size)
            };
            match sigscan::resolve(&pattern, *how, data, section.rva) {
                Outcome::Missing => {}
                Outcome::Ambiguous(n) => total += n,
                hit @ Outcome::Found { .. } => {
                    total += 1;
                    outcome = hit;
                }
            }
        }
        if total > 1 {
            outcome = Outcome::Ambiguous(total);
        }
        let line = match outcome {
            Outcome::Found { matched, resolved } => {
                let address = base + resolved;
                let valid = match how {
                    sigscan::Resolve::Direct => true,
                    sigscan::Resolve::Relative { .. } if *name == sig::MOUSE_DEVICE_INSTANCE => {
                        IMAGE.get().unwrap().contains(address)
                    }
                    sigscan::Resolve::Relative { .. } => IMAGE.get().unwrap().executable(address),
                };
                if valid {
                    found.insert(*name, address);
                    format!("found at rva {resolved:#x} (match rva {matched:#x})")
                } else {
                    format!("REJECTED: resolves outside the image (rva {resolved:#x})")
                }
            }
            Outcome::Missing => "MISSING".into(),
            Outcome::Ambiguous(n) => format!("AMBIGUOUS ({n} matches)"),
        };
        log(format!(
            "self-check signature {name}: {line}; used for {purpose}"
        ));
    }
    log(format!(
        "core bindings: scan took {} ms",
        started.elapsed().as_millis()
    ));

    if let Some(target) = found.get(sig::SCREEN_VIEW_SETUP_AND_RENDER) {
        match unsafe {
            hook(
                *target,
                setup_and_render as *const () as usize,
                &SETUP_AND_RENDER,
            )
        } {
            Ok(()) => {
                SCREEN_HOOK.store(true, Ordering::Release);
                log("self-check hook ScreenView::setupAndRender: installed (in-world, game screens, position)".into());
            }
            Err(e) => log(format!(
                "self-check hook ScreenView::setupAndRender: FAILED ({e})"
            )),
        }
    } else {
        log("self-check hook ScreenView::setupAndRender: not installed; HUD falls back to the game's cursor state".into());
    }
    if let (Some(grab), Some(release)) = (
        found.get(sig::CLIENT_INSTANCE_GRAB_CURSOR),
        found.get(sig::CLIENT_INSTANCE_RELEASE_CURSOR),
    ) {
        GRAB_CURSOR.store(*grab, Ordering::Release);
        RELEASE_CURSOR.store(*release, Ordering::Release);
        log("self-check binding cursor grab/release: ready (needs the screen hook for a ClientInstance)".into());
    } else {
        log("self-check binding cursor grab/release: unavailable; Win32 fallback (unclip + show cursor)".into());
    }
    if let (Some(queue), Some(device)) = (
        found.get(sig::MOUSE_PROCESS_INPUT_QUEUE),
        found.get(sig::MOUSE_DEVICE_INSTANCE),
    ) {
        MOUSE_DEVICE.store(*device, Ordering::Release);
        match unsafe {
            hook(
                *queue,
                process_input_queue as *const () as usize,
                &PROCESS_INPUT_QUEUE,
            )
        } {
            Ok(()) => {
                QUEUE_HOOK.store(true, Ordering::Release);
                log("self-check hook Mouse::processInputQueue: installed (menu clicks stay out of the game)".into());
            }
            Err(e) => log(format!(
                "self-check hook Mouse::processInputQueue: FAILED ({e})"
            )),
        }
    } else {
        log(
            "self-check hook Mouse::processInputQueue: not installed; window-message blocking only"
                .into(),
        );
    }
}

unsafe fn hook(target: usize, detour: usize, original: &AtomicUsize) -> Result<(), String> {
    let mut trampoline = std::ptr::null_mut();
    let status = unsafe { MH_CreateHook(target as _, detour as _, &mut trampoline) };
    if status != MH_STATUS::MH_OK {
        return Err(format!("MH_CreateHook {status:?}"));
    }
    original.store(trampoline as usize, Ordering::Release);
    let status = unsafe { MH_EnableHook(target as _) };
    if status != MH_STATUS::MH_OK {
        return Err(format!("MH_EnableHook {status:?}"));
    }
    Ok(())
}

/// One-line binding status for the Arctic menu ("Bedrock" footer), e.g.
/// "screens, position, cursor, menu clicks" bound or the ones that fell back.
pub fn summary(bound: bool) -> String {
    let position = with_observed(|o| o.position.is_some()).unwrap_or(false);
    let part = |name: &str, ok: bool| format!("{name} {}", if ok { "ok" } else { "off" });
    [
        part("screens", SCREEN_HOOK.load(Ordering::Acquire) && bound),
        part("position", position),
        part("cursor", cursor_binding()),
        part("menu clicks", QUEUE_HOOK.load(Ordering::Acquire)),
    ]
    .join(" · ")
}
